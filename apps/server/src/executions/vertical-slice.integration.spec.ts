import { randomUUID } from 'node:crypto';
import {
  affinityEvents,
  auditEvents,
  characterProfiles,
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
import { eq } from 'drizzle-orm';
import { AffinityService } from '../affinity/affinity.service';
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
  let userId: string;
  let roomId: string;
  let deviceId: string;
  let otherDeviceId: string;
  let createdProposalId: string;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    const sync = new SyncService();
    const affinity = new AffinityService();
    commandService = new CommandsService(connection.db, sync);
    proposalService = new ProposalsService(connection.db, sync);
    decisionService = new DecisionsService(connection.db, sync, affinity);
    executionService = new ExecutionsService(connection.db, sync, affinity);

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
    const completed = await executionService.update(
      userId,
      deviceId,
      execution.id,
      resultKey,
      { status: 'SUCCEEDED', resultSummary: { applied: 1 } },
    );
    expect(completed.status).toBe('SUCCEEDED');
    expect(
      (
        await executionService.update(
          userId,
          deviceId,
          execution.id,
          resultKey,
          { status: 'SUCCEEDED', resultSummary: { applied: 1 } },
        )
      ).id,
    ).toBe(execution.id);
    expect(await executionService.listForRoom(userId, roomId)).toHaveLength(1);
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
    await connection.db
      .delete(proposalItems)
      .where(eq(proposalItems.proposalId, createdProposalId));
    await connection.db.delete(proposals).where(eq(proposals.roomId, roomId));
    await connection.db.delete(commands).where(eq(commands.roomId, roomId));
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
