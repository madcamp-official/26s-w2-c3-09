import { randomUUID } from 'node:crypto';
import {
  chatMessages,
  chatSessions,
  commandDrafts,
  commands,
  createDatabase,
  devices,
  fileBrowseRequests,
  rooms,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
import type {
  AiProvider,
  AiProviderResult,
  ChatContext,
  RuleDraftResult,
  RuleTranslationContext,
} from '../ai/ai.provider';
import { UnconfiguredAiProvider } from '../ai/unconfigured-ai.provider';
import { FileBrowseService } from '../file-access/file-browse.service';
import { SyncService } from '../sync/sync.service';
import { ChatService } from './chat.service';

const databaseUrl = process.env.DATABASE_URL;
const describeDatabase =
  databaseUrl && process.env.MOUSEKEEPER_RUN_DB_TESTS === 'true'
    ? describe
    : describe.skip;

describeDatabase('ChatService PostgreSQL integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let service: ChatService;
  let userId: string;
  let deviceId: string;
  let roomId: string;

  beforeEach(async () => {
    connection = createDatabase(databaseUrl!);
    const sync = new SyncService();
    const redis = {
      status: 'ready',
      connect: jest.fn(async () => undefined),
      get: jest.fn(async () => 'ONLINE_IDLE'),
    };
    service = new ChatService(
      connection.db,
      sync,
      new UnconfiguredAiProvider(),
      new FileBrowseService(connection.db, redis as never, sync),
    );
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `chat-${randomUUID()}`,
          displayName: 'Chat User',
        })
        .returning()
    )[0]!;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Chat PC',
        })
        .returning()
    )[0]!;
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Chat Room',
          rootAlias: 'Downloads',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = device.id;
    roomId = room.id;
  });

  it('enforces five active chat sessions without auto-deleting history', async () => {
    const sessions = [];
    for (let index = 0; index < 5; index += 1) {
      sessions.push(
        await service.createSession(userId, roomId, `Session ${index + 1}`),
      );
    }

    await expect(
      service.createSession(userId, roomId, 'Sixth session'),
    ).rejects.toMatchObject({
      response: {
        code: 'CHAT_SESSION_LIMIT_REACHED',
        limit: 5,
        sessions: expect.arrayContaining([
          expect.objectContaining({ id: sessions[0].id }),
        ]),
      },
    });

    await service.deleteSession(userId, sessions[0].id);
    const replacement = await service.createSession(
      userId,
      roomId,
      'Replacement session',
    );
    expect(replacement.title).toBe('Replacement session');
    expect(await service.listSessions(userId, roomId)).toHaveLength(5);
  });

  it('stores session messages and keeps AI unconfigured instead of faking replies', async () => {
    const session = await service.createSession(userId, roomId);
    const result = await service.createMessage(
      userId,
      session.id,
      'Please clean the Downloads folder',
    );

    expect(result.aiStatus).toBe('UNCONFIGURED');
    expect(result.ai).toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
    expect(result.assistant).toBeNull();
    expect(result.message).toMatchObject({
      sessionId: session.id,
      senderType: 'USER',
      messageType: 'TEXT',
      content: 'Please clean the Downloads folder',
    });
    expect(
      (await service.listMessages(userId, session.id)).map((m) => m.id),
    ).toEqual([result.message.id]);
    expect((await service.listSessions(userId, roomId))[0]).toMatchObject({
      id: session.id,
      title: 'Please clean the Downloads folder',
      messagePreview: 'Please clean the Downloads folder',
    });
  });

  it('keeps legacy room chat compatible while storing through a session', async () => {
    const result = await service.createLegacyRoomMessage(
      userId,
      roomId,
      'Legacy mobile message',
    );

    expect(result.aiStatus).toBe('UNCONFIGURED');
    expect(result.ai).toEqual({
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    });
    const legacyMessages = await service.listLegacyRoomMessages(userId, roomId);
    expect(legacyMessages).toHaveLength(1);
    expect(legacyMessages[0]).toMatchObject({
      id: result.message.id,
      content: 'Legacy mobile message',
    });
    expect(legacyMessages[0].sessionId).toEqual(expect.any(String));

    await service.deleteSession(userId, legacyMessages[0].sessionId!);
    expect(await service.listLegacyRoomMessages(userId, roomId)).toHaveLength(
      0,
    );
  });

  it('materializes scripted AI command output only as a confirmation draft', async () => {
    service = new ChatService(
      connection.db,
      new SyncService(),
      new ScriptedAiProvider({
        status: 'READY',
        kind: 'COMMAND_DRAFT',
        command: {
          intent: 'RENAME',
          payload: {
            rootId: 'root:downloads',
            sourceRelativePath: 'reports/old.pdf',
            newName: 'final.pdf',
          },
        },
        confirmationSummary: 'Rename reports/old.pdf to final.pdf',
      }),
    );
    const session = await service.createSession(userId, roomId);
    const result = await service.createMessage(
      userId,
      session.id,
      'Rename old report to final.pdf',
    );

    expect(result.aiStatus).toBe('READY');
    expect(result.ai).toMatchObject({
      status: 'READY',
      kind: 'COMMAND_DRAFT',
    });
    expect(result.assistant).toMatchObject({
      senderType: 'ASSISTANT',
      messageType: 'COMMAND_DRAFT',
      content: 'Rename reports/old.pdf to final.pdf',
    });

    const drafts = await connection.db
      .select()
      .from(commandDrafts)
      .where(eq(commandDrafts.sessionId, session.id));
    expect(drafts).toHaveLength(1);
    expect(drafts[0]).toMatchObject({
      status: 'DRAFT',
      intent: 'RENAME',
      commandId: null,
    });
    expect(
      await connection.db
        .select()
        .from(commands)
        .where(eq(commands.roomId, roomId)),
    ).toHaveLength(0);
  });

  it('materializes a command draft only after confirmation with an idempotency key', async () => {
    const session = await service.createSession(userId, roomId);
    const source = await service.createMessage(
      userId,
      session.id,
      'Rename old report',
    );
    const draftResult = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'RENAME',
        payload: {
          rootId: 'root:downloads',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        },
      },
      confirmationSummary: 'Rename reports/old.pdf to final.pdf',
    });

    expect(draftResult.draft).toMatchObject({
      intent: 'RENAME',
      status: 'DRAFT',
      commandId: null,
    });
    expect(draftResult.message).toMatchObject({
      senderType: 'ASSISTANT',
      messageType: 'COMMAND_DRAFT',
      structuredPayload: expect.objectContaining({
        id: draftResult.draft.id,
        status: 'DRAFT',
      }),
    });
    expect(
      await connection.db
        .select()
        .from(commands)
        .where(eq(commands.roomId, roomId)),
    ).toHaveLength(0);

    const materialized = await service.confirmCommandDraft(
      userId,
      draftResult.draft.id,
      'confirm-draft-key',
    );
    const command = materialized.command;
    if (!command) throw new Error('RENAME draft did not create a command');

    expect(materialized.draft).toMatchObject({
      id: draftResult.draft.id,
      status: 'MATERIALIZED',
      commandId: command.id,
    });
    expect(command).toMatchObject({
      roomId,
      targetDeviceId: deviceId,
      intent: 'RENAME',
      status: 'QUEUED',
      payload: {
        rootId: 'root:downloads',
        sourceRelativePath: 'reports/old.pdf',
        newName: 'final.pdf',
      },
      metadata: expect.objectContaining({
        sessionId: session.id,
        sourceMessageId: source.message.id,
        commandDraftId: draftResult.draft.id,
        idempotencyKey: 'confirm-draft-key',
        requiresApproval: true,
      }),
    });
    expect(command).not.toHaveProperty('idempotencyKey');
    expect(command).not.toHaveProperty('createdByUserId');

    const replay = await service.confirmCommandDraft(
      userId,
      draftResult.draft.id,
      'confirm-draft-key',
    );
    const replayCommand = replay.command;
    if (!replayCommand)
      throw new Error('RENAME draft replay did not return a command');
    expect(replayCommand.id).toBe(command.id);
    await expect(
      service.confirmCommandDraft(userId, draftResult.draft.id, 'another-key'),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });
  });

  it('materializes FIND drafts as file browse requests without creating commands', async () => {
    const session = await service.createSession(userId, roomId);
    const source = await service.createMessage(
      userId,
      session.id,
      'Find report PDFs',
    );
    const draftResult = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'FIND',
        payload: {
          rootId: 'Downloads',
          query: 'report',
          extensions: ['.pdf'],
          scopeRelativePath: 'Documents',
          limit: 25,
        },
      },
      confirmationSummary: 'Find report PDFs under Documents',
    });

    const materialized = await service.confirmCommandDraft(
      userId,
      draftResult.draft.id,
      'find-draft-key',
    );
    const fileBrowseRequest = materialized.fileBrowseRequest;
    if (!fileBrowseRequest) {
      throw new Error('FIND draft did not create a file browse request');
    }

    expect(materialized).not.toHaveProperty('command');
    expect(materialized.draft).toMatchObject({
      id: draftResult.draft.id,
      status: 'MATERIALIZED',
      commandId: null,
      fileBrowseRequestId: fileBrowseRequest.id,
    });
    expect(fileBrowseRequest).toMatchObject({
      roomId,
      desktopDeviceId: deviceId,
      relativeDirectory: 'Documents',
      query: 'report',
      extensions: ['.pdf'],
      limit: 25,
      searchScope: 'CURRENT_DIRECTORY',
      status: 'REQUESTED',
    });
    expect(
      await connection.db
        .select()
        .from(commands)
        .where(eq(commands.roomId, roomId)),
    ).toHaveLength(0);
    expect(
      await connection.db
        .select()
        .from(fileBrowseRequests)
        .where(eq(fileBrowseRequests.roomId, roomId)),
    ).toHaveLength(1);

    const replay = await service.confirmCommandDraft(
      userId,
      draftResult.draft.id,
      'find-draft-key',
    );
    const replayFileBrowseRequest = replay.fileBrowseRequest;
    if (!replayFileBrowseRequest) {
      throw new Error('FIND draft replay did not return a file browse request');
    }
    expect(replayFileBrowseRequest.id).toBe(fileBrowseRequest.id);
    await expect(
      service.confirmCommandDraft(userId, draftResult.draft.id, 'another-key'),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });

    const events = await connection.db
      .select()
      .from(syncEvents)
      .where(eq(syncEvents.roomId, roomId));
    expect(events.map((event) => event.eventType)).toEqual(
      expect.arrayContaining([
        'file.browse.requested',
        'command.draft.updated',
      ]),
    );
  });

  it('rejects and expires command drafts without creating commands', async () => {
    const session = await service.createSession(userId, roomId);
    const source = await service.createMessage(
      userId,
      session.id,
      'Trash temp',
    );
    const rejected = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'TRASH',
        payload: {
          rootId: 'root:downloads',
          sourceRelativePaths: ['tmp/noise.log'],
        },
      },
      confirmationSummary: 'Move tmp/noise.log to trash',
    });

    expect(
      (await service.rejectCommandDraft(userId, rejected.draft.id)).draft,
    ).toMatchObject({ status: 'REJECTED' });
    await expect(
      service.confirmCommandDraft(userId, rejected.draft.id, 'reject-key'),
    ).rejects.toMatchObject({
      response: { code: 'INVALID_STATE_TRANSITION' },
    });

    const expired = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'MOVE',
        payload: {
          rootId: 'root:downloads',
          sourceRelativePaths: ['reports/old.pdf'],
          destinationRelativeDirectory: 'Archive',
        },
      },
      confirmationSummary: 'Move reports/old.pdf to Archive',
      expiresAt: '2026-01-01T00:00:00.000Z',
    });
    await expect(
      service.confirmCommandDraft(userId, expired.draft.id, 'expired-key'),
    ).rejects.toMatchObject({ response: { code: 'DRAFT_EXPIRED' } });
    expect(
      await connection.db
        .select()
        .from(commands)
        .where(eq(commands.roomId, roomId)),
    ).toHaveLength(0);
  });

  afterEach(async () => {
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
    await connection.db
      .delete(commandDrafts)
      .where(eq(commandDrafts.roomId, roomId));
    await connection.db
      .delete(fileBrowseRequests)
      .where(eq(fileBrowseRequests.roomId, roomId));
    await connection.db
      .delete(chatMessages)
      .where(eq(chatMessages.roomId, roomId));
    await connection.db.delete(commands).where(eq(commands.roomId, roomId));
    await connection.db
      .delete(chatSessions)
      .where(eq(chatSessions.roomId, roomId));
    await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    await connection.db.delete(devices).where(eq(devices.id, deviceId));
    await connection.db.delete(users).where(eq(users.id, userId));
    await connection.close();
  });
});

class ScriptedAiProvider implements AiProvider {
  constructor(private readonly result: AiProviderResult) {}

  async classifyAndRespond(_input: ChatContext): Promise<AiProviderResult> {
    return this.result;
  }

  async translateRule(
    _input: RuleTranslationContext,
  ): Promise<RuleDraftResult> {
    return {
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    };
  }
}
