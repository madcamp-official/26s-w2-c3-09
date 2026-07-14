import { randomUUID } from 'node:crypto';
import {
  cachedFiles,
  chatMessages,
  chatReadStates,
  chatSessions,
  commandDrafts,
  commands,
  createDatabase,
  devices,
  fileBrowseRequests,
  fileTransfers,
  ruleDrafts,
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
import { TransfersService } from '../transfers/transfers.service';
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
      exists: jest.fn(async () => 1),
    };
    const storage = { assertConfigured: jest.fn() };
    service = new ChatService(
      connection.db,
      sync,
      new UnconfiguredAiProvider(),
      new FileBrowseService(connection.db, redis as never, sync),
      new TransfersService(
        connection.db,
        redis as never,
        sync,
        storage as never,
      ),
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
      unreadCount: 0,
      pendingActionCount: 0,
    });
  });

  it('stores and syncs conversational AI replies as assistant text', async () => {
    service = new ChatService(
      connection.db,
      new SyncService(),
      new ScriptedAiProvider({
        status: 'READY',
        kind: 'NO_ACTION',
        reply: '네, MouseKeeper AI가 연결되어 있어요.',
      }),
    );
    const session = await service.createSession(userId, roomId);
    const result = await service.createMessage(
      userId,
      session.id,
      '지금 인공지능이 되나?',
    );

    expect(result).toMatchObject({
      aiStatus: 'READY',
      ai: { status: 'READY', kind: 'NO_ACTION' },
      assistant: {
        senderType: 'ASSISTANT',
        messageType: 'TEXT',
        content: '네, MouseKeeper AI가 연결되어 있어요.',
      },
    });
    expect(
      (await service.listMessages(userId, session.id)).map((message) => ({
        senderType: message.senderType,
        content: message.content,
      })),
    ).toEqual([
      { senderType: 'USER', content: '지금 인공지능이 되나?' },
      {
        senderType: 'ASSISTANT',
        content: '네, MouseKeeper AI가 연결되어 있어요.',
      },
    ]);
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
    if (!result.assistant) throw new Error('Expected assistant draft message');

    const drafts = await connection.db
      .select()
      .from(commandDrafts)
      .where(eq(commandDrafts.sessionId, session.id));
    expect(drafts).toHaveLength(1);
    expect((await service.listSessions(userId, roomId))[0]).toMatchObject({
      id: session.id,
      unreadCount: 1,
      pendingActionCount: 1,
      lastReadMessageId: null,
      readAt: null,
    });

    const read = await service.markSessionRead(userId, session.id);
    expect(read).toMatchObject({
      id: session.id,
      unreadCount: 0,
      pendingActionCount: 1,
    });
    expect(read.lastReadMessageId).toEqual(result.assistant.id);
    expect(read.readAt).toEqual(expect.any(Date));
    expect((await service.listSessions(userId, roomId))[0]).toMatchObject({
      id: session.id,
      unreadCount: 0,
      pendingActionCount: 1,
      lastReadMessageId: result.assistant.id,
    });
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

  it('returns quick-view prompts, recent history, and pending suggestions', async () => {
    const session = await service.createSession(userId, roomId, 'Quick view');
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

    const quickView = await service.quickView(userId, roomId);

    expect(quickView.prompts.map((prompt) => prompt.category)).toEqual(
      expect.arrayContaining(['QUERY', 'COMMAND', 'RULE', 'CLEANUP']),
    );
    expect(quickView.sessions[0]).toMatchObject({
      id: session.id,
      unreadCount: 1,
      pendingActionCount: 1,
    });
    expect(quickView.unreadCount).toBeGreaterThanOrEqual(1);
    expect(quickView.pendingActionCount).toBe(1);
    expect(quickView.history.map((item) => item.messageId)).toEqual(
      expect.arrayContaining([source.message.id, draftResult.message.id]),
    );
    expect(quickView.pendingSuggestions).toEqual([
      expect.objectContaining({
        sessionId: session.id,
        messageType: 'COMMAND_DRAFT',
        draftId: draftResult.draft.id,
        status: 'DRAFT',
      }),
    ]);
  });

  it('creates quick cleanup as an ANALYZE command draft without queuing a command', async () => {
    const result = await service.createQuickCleanupSuggestion(userId, roomId);

    expect(result.session).toMatchObject({
      roomId,
      title: '빠른 정리 제안',
      unreadCount: 1,
      pendingActionCount: 1,
    });
    expect(result.message).toMatchObject({
      senderType: 'USER',
      messageType: 'TEXT',
      content: '빠른 정리 제안 요청',
      sessionId: result.session.id,
    });
    expect(result.assistant).toMatchObject({
      senderType: 'ASSISTANT',
      messageType: 'COMMAND_DRAFT',
      sessionId: result.session.id,
      structuredPayload: expect.objectContaining({
        intent: 'ANALYZE',
        status: 'DRAFT',
        commandId: null,
      }),
    });
    expect(result.ai).toMatchObject({
      status: 'READY',
      kind: 'COMMAND_DRAFT',
    });
    expect(
      await connection.db
        .select()
        .from(commandDrafts)
        .where(eq(commandDrafts.sessionId, result.session.id)),
    ).toEqual([
      expect.objectContaining({
        intent: 'ANALYZE',
        arguments: {},
        status: 'DRAFT',
        commandId: null,
      }),
    ]);
    expect(
      await connection.db
        .select()
        .from(commands)
        .where(eq(commands.roomId, roomId)),
    ).toHaveLength(0);
  });

  it('persists AI rule drafts as chat approval cards without materializing rules', async () => {
    service = new ChatService(
      connection.db,
      new SyncService(),
      new ScriptedAiProvider({
        status: 'READY',
        kind: 'RULE_DRAFT',
        draft: {
          name: 'PDF archive',
          definition: {
            match: 'ALL',
            conditions: [
              { field: 'extension', operator: 'IN', value: ['.pdf'] },
            ],
            action: { type: 'MOVE', destinationTemplate: 'Archive/PDF' },
          },
          explanation: 'Move PDF files into Archive/PDF.',
          ambiguities: [],
        },
      }),
    );
    const session = await service.createSession(userId, roomId);
    const result = await service.createMessage(
      userId,
      session.id,
      'PDF는 앞으로 Archive/PDF로 옮기는 규칙 추가해줘',
    );

    expect(result.aiStatus).toBe('READY');
    expect(result.ai).toMatchObject({
      status: 'READY',
      kind: 'RULE_DRAFT',
    });
    expect(result.assistant).toMatchObject({
      senderType: 'ASSISTANT',
      messageType: 'RULE_DRAFT',
      content: 'Move PDF files into Archive/PDF.',
      structuredPayload: expect.objectContaining({
        name: 'PDF archive',
        status: 'DRAFT',
        ruleId: null,
      }),
    });
    if (!result.assistant) throw new Error('Expected assistant rule draft message');

    const drafts = await connection.db
      .select()
      .from(ruleDrafts)
      .where(eq(ruleDrafts.sessionId, session.id));
    expect(drafts).toHaveLength(1);
    expect(drafts[0]).toMatchObject({
      sourceMessageId: result.message.id,
      roomId,
      createdByUserId: userId,
      name: 'PDF archive',
      status: 'DRAFT',
      ruleId: null,
    });
    expect((await service.listSessions(userId, roomId))[0]).toMatchObject({
      id: session.id,
      unreadCount: 1,
      pendingActionCount: 1,
    });
  });

  it('answers AI query results from cache while forcing live browse offline', async () => {
    const sync = new SyncService();
    const offlineRedis = {
      status: 'ready',
      connect: jest.fn(async () => undefined),
      get: jest.fn(async () => null),
    };
    service = new ChatService(
      connection.db,
      sync,
      new ScriptedAiProvider({
        status: 'READY',
        kind: 'QUERY',
        browse: {
          relativeDirectory: 'Documents',
          cursor: null,
          query: 'report',
          extensions: ['.pdf'],
          limit: 10,
          searchScope: 'MANAGED_ROOT',
        },
        responseSummary: 'Looking for report PDFs under Documents.',
      }),
      new FileBrowseService(connection.db, offlineRedis as never, sync),
      undefined,
    );
    await connection.db.insert(cachedFiles).values({
      roomId,
      sourceRelativePath: 'Documents/report.pdf',
      sourceVersion: { mtimeMs: 1, sizeBytes: 1024 },
      sourceVersionHash: 'a'.repeat(64),
      usageScore: 10,
      objectKey: 'cache/report.pdf',
      sizeBytes: 1024,
      sha256: 'b'.repeat(64),
      lastVerifiedAt: new Date(),
    });
    await connection.db.insert(cachedFiles).values({
      roomId,
      sourceRelativePath: 'Documents/photo.jpg',
      sourceVersion: { mtimeMs: 2, sizeBytes: 2048 },
      sourceVersionHash: 'c'.repeat(64),
      usageScore: 9,
      objectKey: 'cache/photo.jpg',
      sizeBytes: 2048,
      sha256: 'd'.repeat(64),
      lastVerifiedAt: new Date(),
    });

    const session = await service.createSession(userId, roomId);
    const result = await service.createMessage(
      userId,
      session.id,
      'Documents에서 report pdf 찾아줘',
    );

    expect(result.ai).toMatchObject({
      status: 'READY',
      kind: 'QUERY',
      cacheHitCount: 1,
    });
    expect(result.assistant).toMatchObject({
      senderType: 'ASSISTANT',
      messageType: 'QUERY_RESULT',
      structuredPayload: expect.objectContaining({
        cacheMode: 'CACHE_ONLY',
        cacheHitCount: 1,
        cacheEntries: [
          expect.objectContaining({ relativePath: 'Documents/report.pdf' }),
        ],
        liveBrowseRequest: expect.objectContaining({
          status: 'FAILED',
          failureCode: 'DEVICE_OFFLINE',
        }),
      }),
    });
    expect((await service.listSessions(userId, roomId))[0]).toMatchObject({
      id: session.id,
      unreadCount: 1,
      pendingActionCount: 0,
    });
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
    expect((await service.listSessions(userId, roomId))[0]).toMatchObject({
      id: session.id,
      pendingActionCount: 0,
    });

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

  it('materializes DOWNLOAD drafts as file transfer requests without creating commands', async () => {
    const session = await service.createSession(userId, roomId);
    const source = await service.createMessage(
      userId,
      session.id,
      'Download current report',
    );
    const draftResult = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'DOWNLOAD',
        payload: {
          rootId: 'Downloads',
          sourceRelativePath: 'reports/current.pdf',
        },
      },
      confirmationSummary: 'Download reports/current.pdf',
    });

    const materialized = await service.confirmCommandDraft(
      userId,
      draftResult.draft.id,
      'download-draft-key',
    );
    const fileTransfer = materialized.fileTransfer;
    if (!fileTransfer) {
      throw new Error('DOWNLOAD draft did not create a file transfer');
    }

    expect(materialized).not.toHaveProperty('command');
    expect(materialized.draft).toMatchObject({
      id: draftResult.draft.id,
      status: 'MATERIALIZED',
      commandId: null,
      fileTransferId: fileTransfer.id,
    });
    expect(fileTransfer).toMatchObject({
      roomId,
      desktopDeviceId: deviceId,
      sourceRelativePath: 'reports/current.pdf',
      status: 'REQUESTED',
    });
    expect(fileTransfer).not.toHaveProperty('objectKey');
    expect(fileTransfer).not.toHaveProperty('idempotencyKey');
    expect(fileTransfer).not.toHaveProperty('requestedByUserId');
    expect(
      await connection.db
        .select()
        .from(commands)
        .where(eq(commands.roomId, roomId)),
    ).toHaveLength(0);
    expect(
      await connection.db
        .select()
        .from(fileTransfers)
        .where(eq(fileTransfers.roomId, roomId)),
    ).toHaveLength(1);

    const replay = await service.confirmCommandDraft(
      userId,
      draftResult.draft.id,
      'download-draft-key',
    );
    const replayFileTransfer = replay.fileTransfer;
    if (!replayFileTransfer) {
      throw new Error('DOWNLOAD draft replay did not return a file transfer');
    }
    expect(replayFileTransfer.id).toBe(fileTransfer.id);
    await expect(
      service.confirmCommandDraft(userId, draftResult.draft.id, 'another-key'),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });

    const events = await connection.db
      .select()
      .from(syncEvents)
      .where(eq(syncEvents.roomId, roomId));
    expect(events.map((event) => event.eventType)).toEqual(
      expect.arrayContaining([
        'file.transfer.requested',
        'command.draft.updated',
      ]),
    );
  });

  it('rejects DOWNLOAD drafts with unsupported expected identity instead of ignoring it', async () => {
    const session = await service.createSession(userId, roomId);
    const source = await service.createMessage(
      userId,
      session.id,
      'Download exact report',
    );
    const draftResult = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'DOWNLOAD',
        payload: {
          rootId: 'Downloads',
          sourceRelativePath: 'reports/current.pdf',
          expectedIdentity: { sizeBytes: 1024 },
        },
      },
      confirmationSummary: 'Download reports/current.pdf if unchanged',
    });

    await expect(
      service.confirmCommandDraft(
        userId,
        draftResult.draft.id,
        'download-identity-key',
      ),
    ).rejects.toMatchObject({
      response: { code: 'EXPECTED_IDENTITY_UNSUPPORTED' },
    });
    expect(
      await connection.db
        .select()
        .from(fileTransfers)
        .where(eq(fileTransfers.roomId, roomId)),
    ).toHaveLength(0);
  });

  it('rejects UPLOAD drafts before they become unsupported desktop commands', async () => {
    const session = await service.createSession(userId, roomId);
    const source = await service.createMessage(
      userId,
      session.id,
      'Upload report',
    );
    const draftResult = await service.createCommandDraft(userId, session.id, {
      sourceMessageId: source.message.id,
      command: {
        intent: 'UPLOAD',
        payload: {
          rootId: 'Downloads',
          destinationRelativePath: 'incoming/report.pdf',
          transferId: randomUUID(),
          expectedSha256: 'a'.repeat(64),
          expectedSize: 1024,
        },
      },
      confirmationSummary: 'Upload report.pdf to incoming/report.pdf',
    });

    await expect(
      service.confirmCommandDraft(userId, draftResult.draft.id, 'upload-key'),
    ).rejects.toMatchObject({
      response: { code: 'UPLOAD_TRANSFER_UNCONFIGURED' },
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
        .from(fileTransfers)
        .where(eq(fileTransfers.roomId, roomId)),
    ).toHaveLength(0);
    expect(
      (
        await connection.db
          .select({
            status: commandDrafts.status,
            commandId: commandDrafts.commandId,
          })
          .from(commandDrafts)
          .where(eq(commandDrafts.id, draftResult.draft.id))
      )[0],
    ).toEqual({ status: 'DRAFT', commandId: null });
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
    await connection.db.delete(chatReadStates).where(eq(chatReadStates.userId, userId));
    await connection.db
      .delete(cachedFiles)
      .where(eq(cachedFiles.roomId, roomId));
    await connection.db
      .delete(ruleDrafts)
      .where(eq(ruleDrafts.roomId, roomId));
    await connection.db
      .delete(commandDrafts)
      .where(eq(commandDrafts.roomId, roomId));
    await connection.db
      .delete(fileBrowseRequests)
      .where(eq(fileBrowseRequests.roomId, roomId));
    await connection.db
      .delete(fileTransfers)
      .where(eq(fileTransfers.roomId, roomId));
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
