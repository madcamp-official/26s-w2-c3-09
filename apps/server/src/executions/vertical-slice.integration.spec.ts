import { randomUUID } from 'node:crypto';
import {
  affinityEvents,
  auditEvents,
  characterProfiles,
  chatMessages,
  chatSessions,
  commands,
  createDatabase,
  decisions,
  devices,
  executions,
  proposalItems,
  proposals,
  rooms,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq, inArray } from 'drizzle-orm';
import { AffinityService } from '../affinity/affinity.service';
import { ChatService } from '../chat/chat.service';
import { UnconfiguredAiProvider } from '../ai/unconfigured-ai.provider';
import { CommandsService } from '../commands/commands.service';
import { DecisionsService } from '../decisions/decisions.service';
import { ProposalsService } from '../proposals/proposals.service';
import { SyncService } from '../sync/sync.service';
import { ExecutionsService } from './executions.service';

const databaseUrl = process.env.DATABASE_URL;
const describeDatabase =
  databaseUrl && process.env.MOUSEKEEPER_RUN_DB_TESTS === 'true'
    ? describe
    : describe.skip;

describeDatabase('command to execution PostgreSQL integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let commandService: CommandsService;
  let proposalService: ProposalsService;
  let decisionService: DecisionsService;
  let executionService: ExecutionsService;
  let chat: ChatService;
  let userId: string;
  let roomId: string;
  let deviceId: string;
  let otherDeviceId: string;
  let createdProposalId: string;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    const sync = new SyncService();
    const affinity = new AffinityService();
    chat = new ChatService(connection.db, sync, new UnconfiguredAiProvider());
    commandService = new CommandsService(connection.db, sync);
    proposalService = new ProposalsService(connection.db, sync, chat);
    decisionService = new DecisionsService(connection.db, sync, affinity);
    executionService = new ExecutionsService(
      connection.db,
      sync,
      affinity,
      chat,
    );

    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `vertical-${randomUUID()}`,
          displayName: 'Vertical Slice User',
        })
        .returning()
    )[0]!;
    const createdDevices = await connection.db
      .insert(devices)
      .values([
        { userId: user.id, platform: 'WINDOWS', deviceName: 'Target PC' },
        { userId: user.id, platform: 'WINDOWS', deviceName: 'Other PC' },
      ])
      .returning();
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: createdDevices[0]!.id,
          name: 'Vertical Room',
          rootAlias: 'Downloads',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = createdDevices[0]!.id;
    otherDeviceId = createdDevices[1]!.id;
    roomId = room.id;
  });

  it('isolates approval and execution to the command target device', async () => {
    const command = await commandService.create(userId, roomId, randomUUID(), {
      intent: 'ANALYZE',
      payload: {},
    });
    await commandService.update(userId, deviceId, command.id, {
      status: 'DELIVERED',
    });
    await commandService.update(userId, deviceId, command.id, {
      status: 'ANALYZING',
    });
    const proposalKey = randomUUID();
    const proposal = await proposalService.create(
      userId,
      deviceId,
      proposalKey,
      {
        commandId: command.id,
        roomId,
        summary: { itemCount: 1 },
        expiresAt: null,
        items: [
          {
            itemOrder: 0,
            actionType: 'QUARANTINE',
            sourceRelativePath: 'old.pdf',
            destinationRelativePath: null,
            reasonCode: 'RULE_MATCH',
            precondition: {},
            conflictState: 'NONE',
          },
        ],
      },
    );
    expect(
      (
        await proposalService.create(userId, deviceId, proposalKey, {
          commandId: command.id,
          roomId,
          summary: { itemCount: 1 },
          expiresAt: null,
          items: [
            {
              itemOrder: 0,
              actionType: 'QUARANTINE',
              sourceRelativePath: 'old.pdf',
              destinationRelativePath: null,
              reasonCode: 'RULE_MATCH',
              precondition: {},
              conflictState: 'NONE',
            },
          ],
        })
      ).id,
    ).toBe(proposal.id);

    // The command was created directly (not via a chat command draft), so the
    // proposal must fall back to a room-level chat session and post itself
    // there as a PROPOSAL card — this is what lets a proposal with no chat
    // origin still show up in chat.
    expect(proposal.sessionId).toEqual(expect.any(String));
    expect(proposal.chatMessageId).toEqual(expect.any(String));
    const messagesAfterProposal = await chat.listMessages(
      userId,
      proposal.sessionId!,
    );
    const proposalCards = messagesAfterProposal.filter(
      (message) => message.messageType === 'PROPOSAL',
    );
    expect(proposalCards).toHaveLength(1);
    expect(proposalCards[0]?.structuredPayload).toMatchObject({
      id: proposal.id,
      status: 'OPEN',
      itemCount: 1,
    });
    const quickViewBeforeDecision = await chat.quickView(userId, roomId);
    expect(
      quickViewBeforeDecision.pendingSuggestions.some(
        (suggestion) => suggestion.draftId === proposal.id,
      ),
    ).toBe(true);

    const decisionKey = randomUUID();
    const decision = await decisionService.create(
      userId,
      proposal.id,
      decisionKey,
      {
        decisionType: 'APPROVE',
        approvedItemIds: proposal.items.map((item) => item.id),
      },
    );
    expect(
      (
        await decisionService.create(userId, proposal.id, decisionKey, {
          decisionType: 'APPROVE',
          approvedItemIds: proposal.items.map((item) => item.id),
        })
      ).id,
    ).toBe(decision.id);
    expect(decision).not.toHaveProperty('idempotencyKey');
    expect(decision).not.toHaveProperty('userId');
    await expect(
      decisionService.create(userId, proposal.id, decisionKey, {
        decisionType: 'REJECT',
        approvedItemIds: [],
      }),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });
    createdProposalId = proposal.id;

    // Deciding never touches chat_messages directly; the PROPOSAL card's
    // structuredPayload.status must be resolved live from the `proposals`
    // table on the next read, not served stale from the row written at
    // creation time.
    const messagesAfterDecision = await chat.listMessages(
      userId,
      proposal.sessionId!,
    );
    expect(
      messagesAfterDecision.find(
        (message) => message.messageType === 'PROPOSAL',
      )?.structuredPayload,
    ).toMatchObject({ status: 'APPROVED' });
    const quickViewAfterDecision = await chat.quickView(userId, roomId);
    expect(
      quickViewAfterDecision.pendingSuggestions.some(
        (suggestion) => suggestion.draftId === proposal.id,
      ),
    ).toBe(false);

    expect(await decisionService.pending(userId, otherDeviceId)).toHaveLength(
      0,
    );
    expect(await decisionService.pending(userId, deviceId)).toHaveLength(1);

    await expect(
      executionService.create(userId, randomUUID(), {
        proposalId: proposal.id,
        decisionId: decision.id,
        desktopDeviceId: otherDeviceId,
      }),
    ).rejects.toMatchObject({ response: { code: 'FORBIDDEN' } });

    const execution = await executionService.create(userId, randomUUID(), {
      proposalId: proposal.id,
      decisionId: decision.id,
      desktopDeviceId: deviceId,
    });
    expect(await decisionService.pending(userId, deviceId)).toHaveLength(0);
    await expect(
      executionService.update(
        userId,
        otherDeviceId,
        execution.id,
        randomUUID(),
        {
          status: 'SUCCEEDED',
          resultSummary: { applied: 1 },
        },
      ),
    ).rejects.toMatchObject({ response: { code: 'NOT_FOUND' } });
    const resultKey = randomUUID();
    const resultSummary = {
      root: 'Downloads',
      journal_path: 'journal.log',
      executed_count: 1,
      skipped_count: 0,
      rejected_count: 0,
      results: [
        {
          action: 'quarantine',
          from: 'old.pdf',
          to: '.mousekeeper_trash/old.pdf',
          status: 'executed',
          reason: null,
        },
      ],
    };
    const completed = await executionService.update(
      userId,
      deviceId,
      execution.id,
      resultKey,
      { status: 'SUCCEEDED', resultSummary },
    );
    expect(completed.status).toBe('SUCCEEDED');
    expect(
      (
        await executionService.update(
          userId,
          deviceId,
          execution.id,
          resultKey,
          { status: 'SUCCEEDED', resultSummary },
        )
      ).id,
    ).toBe(execution.id);
    expect(await executionService.listForRoom(userId, roomId)).toHaveLength(1);

    // Execution results land as their own EXECUTION_RESULT chat entry (not a
    // patch on the proposal card), and the idempotent replay above must not
    // have posted a second one.
    const messagesAfterExecution = await chat.listMessages(
      userId,
      proposal.sessionId!,
    );
    const resultCards = messagesAfterExecution.filter(
      (message) => message.messageType === 'EXECUTION_RESULT',
    );
    expect(resultCards).toHaveLength(1);
    expect(resultCards[0]?.structuredPayload).toMatchObject({
      status: 'SUCCEEDED',
      executedCount: 1,
      skippedCount: 0,
      rejectedCount: 0,
    });
  });

  it('routes a chat-less proposal into the room\'s existing active session instead of a fresh one', async () => {
    // getOrCreateSystemSessionIn only creates a brand-new "자동 제안" session
    // when the room has zero active sessions. The previous test already left
    // one behind, but that's incidental — create our own here so this
    // assertion doesn't depend on test execution order, and so it directly
    // proves the "reuse the most recently active session" branch rather than
    // the "create a fresh one" branch already covered above.
    const existingSession = await chat.createSession(userId, roomId);

    const command = await commandService.create(userId, roomId, randomUUID(), {
      intent: 'ANALYZE',
      payload: {},
    });
    await commandService.update(userId, deviceId, command.id, {
      status: 'DELIVERED',
    });
    await commandService.update(userId, deviceId, command.id, {
      status: 'ANALYZING',
    });
    const proposal = await proposalService.create(
      userId,
      deviceId,
      randomUUID(),
      {
        commandId: command.id,
        roomId,
        summary: { itemCount: 1 },
        expiresAt: null,
        items: [
          {
            itemOrder: 0,
            actionType: 'QUARANTINE',
            sourceRelativePath: 'reused-session.pdf',
            destinationRelativePath: null,
            reasonCode: 'RULE_MATCH',
            precondition: {},
            conflictState: 'NONE',
          },
        ],
      },
    );

    expect(proposal.sessionId).toBe(existingSession.id);
    const messages = await chat.listMessages(userId, existingSession.id);
    expect(
      messages.some(
        (message) =>
          message.messageType === 'PROPOSAL' &&
          (message.structuredPayload as { id?: string } | null)?.id ===
            proposal.id,
      ),
    ).toBe(true);
  });

  afterAll(async () => {
    const profile = (
      await connection.db
        .select()
        .from(characterProfiles)
        .where(eq(characterProfiles.userId, userId))
        .limit(1)
    )[0];
    if (profile) {
      await connection.db
        .delete(affinityEvents)
        .where(eq(affinityEvents.characterProfileId, profile.id));
    }
    await connection.db
      .delete(auditEvents)
      .where(eq(auditEvents.userId, userId));
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
    await connection.db
      .delete(executions)
      .where(eq(executions.proposalId, createdProposalId));
    await connection.db.delete(decisions).where(eq(decisions.userId, userId));
    // Two tests in this file create proposals in the same room (the second
    // asserts session-reuse, not execution), so proposal items must be
    // cleared for every proposal in the room, not just the first one.
    const roomProposalIds = (
      await connection.db
        .select({ id: proposals.id })
        .from(proposals)
        .where(eq(proposals.roomId, roomId))
    ).map((row) => row.id);
    if (roomProposalIds.length > 0) {
      await connection.db
        .delete(proposalItems)
        .where(inArray(proposalItems.proposalId, roomProposalIds));
    }
    await connection.db.delete(proposals).where(eq(proposals.roomId, roomId));
    await connection.db.delete(commands).where(eq(commands.roomId, roomId));
    await connection.db
      .delete(chatMessages)
      .where(eq(chatMessages.roomId, roomId));
    await connection.db
      .delete(chatSessions)
      .where(eq(chatSessions.roomId, roomId));
    if (profile) {
      await connection.db
        .delete(characterProfiles)
        .where(eq(characterProfiles.id, profile.id));
    }
    await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    await connection.db.delete(devices).where(eq(devices.userId, userId));
    await connection.db.delete(users).where(eq(users.id, userId));
    await connection.close();
  });
});
