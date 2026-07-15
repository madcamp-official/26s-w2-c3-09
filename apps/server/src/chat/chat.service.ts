import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
  Optional,
} from '@nestjs/common';
import {
  createCommandDraftSchema,
  createFileBrowseRequestSchema,
  createFileTransferSchema,
  downloadCommandPayloadSchema,
  findCommandPayloadSchema,
  ruleDefinitionSchema,
  uploadCommandPayloadSchema,
} from '@mousekeeper/contracts';
import {
  cachedFiles,
  chatMessages,
  chatReadStates,
  chatSessions,
  commandDrafts,
  commands,
  devices,
  fileBrowseRequests,
  fileTransfers,
  proposals,
  ruleDrafts,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, asc, desc, eq, gt, inArray, isNull, ne, or, sql } from 'drizzle-orm';
import { z } from 'zod';
import { mapAiResultToCommandDraft } from '../ai/ai-command-draft.mapper';
import {
  AI_PROVIDER,
  type AiProvider,
  type AiQueryResult,
  type AiRuleDraftResult,
} from '../ai/ai.provider';
import { canonicalJson } from '../common/canonical-json';
import { DATABASE } from '../database/database.module';
import { FileBrowseService } from '../file-access/file-browse.service';
import { SyncService } from '../sync/sync.service';
import { TransfersService } from '../transfers/transfers.service';

const CHAT_SESSION_LIMIT = 5;
const DEFAULT_CHAT_TITLE = 'New chat';
const DEFAULT_DRAFT_TTL_MS = 10 * 60 * 1000;
const QUICK_CLEANUP_USER_MESSAGE = '빠른 정리 제안 요청';
const QUICK_CLEANUP_CONFIRMATION =
  'PC가 관리 루트를 분석해서 정리 제안을 만들도록 요청합니다. 파일 변경은 별도 제안 승인 전에는 실행되지 않습니다.';
const QUICK_VIEW_PROMPTS = [
  {
    id: 'find-recent-reports',
    label: 'Find reports',
    prompt: '최근 report PDF 파일 찾아줘',
    category: 'QUERY',
  },
  {
    id: 'clean-downloads',
    label: 'Clean Downloads',
    prompt: 'Downloads 폴더에서 정리할 것들을 빠르게 제안해줘',
    category: 'CLEANUP',
  },
  {
    id: 'archive-pdfs-rule',
    label: 'PDF rule',
    prompt: '앞으로 PDF는 Archive/PDF로 옮기는 규칙을 추가해줘',
    category: 'RULE',
  },
  {
    id: 'move-screenshots',
    label: 'Move screenshots',
    prompt: '스크린샷 파일들을 Screenshots 폴더로 옮겨줘',
    category: 'COMMAND',
  },
] as const;
type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
type DbExecutor = Database | Transaction;
type PublicChatMessage = {
  id: string;
  roomId: string;
  sessionId: string | null;
  senderType: string;
  messageType: string;
  content: string;
  structuredPayload: unknown;
  commandId: string | null;
  createdAt: Date;
};

@Injectable()
export class ChatService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
    @Inject(AI_PROVIDER) private readonly ai: AiProvider,
    @Optional() private readonly browse?: FileBrowseService,
    @Optional() private readonly transfers?: TransfersService,
  ) {}

  async listSessions(userId: string, roomId: string) {
    await this.requireOwnedRoom(userId, roomId);
    const sessions = await this.db
      .select()
      .from(chatSessions)
      .where(
        and(
          eq(chatSessions.userId, userId),
          eq(chatSessions.roomId, roomId),
          eq(chatSessions.status, 'ACTIVE'),
        ),
      )
      .orderBy(desc(chatSessions.updatedAt), desc(chatSessions.createdAt));

    return this.withPreviews(this.db, sessions);
  }

  async quickView(userId: string, roomId: string) {
    await this.requireOwnedRoom(userId, roomId);
    const sessions = await this.listSessions(userId, roomId);
    const history = await this.quickHistory(userId, roomId);
    const pendingSuggestions = await this.quickPendingSuggestions(
      userId,
      roomId,
    );
    return {
      prompts: QUICK_VIEW_PROMPTS.map((prompt) => ({ ...prompt })),
      sessions: sessions.slice(0, CHAT_SESSION_LIMIT),
      history,
      pendingSuggestions,
      unreadCount: sessions.reduce(
        (sum, session) => sum + Number(session.unreadCount ?? 0),
        0,
      ),
      pendingActionCount: sessions.reduce(
        (sum, session) => sum + Number(session.pendingActionCount ?? 0),
        0,
      ),
    };
  }

  async createQuickCleanupSuggestion(userId: string, roomId: string) {
    return this.db.transaction(async (tx) => {
      await this.requireOwnedRoomIn(tx, userId, roomId);
      const session = await this.getOrCreateSystemSessionIn(
        tx,
        userId,
        roomId,
        '빠른 정리 제안',
      );
      const message = (
        await tx
          .insert(chatMessages)
          .values({
            roomId,
            sessionId: session.id,
            senderType: 'USER',
            messageType: 'TEXT',
            content: QUICK_CLEANUP_USER_MESSAGE,
          })
          .returning()
      )[0]!;
      const expiresAt = new Date(Date.now() + DEFAULT_DRAFT_TTL_MS);
      const draft = (
        await tx
          .insert(commandDrafts)
          .values({
            sessionId: session.id,
            sourceMessageId: message.id,
            roomId,
            createdByUserId: userId,
            intent: 'ANALYZE',
            arguments: {},
            confirmationSummary: QUICK_CLEANUP_CONFIRMATION,
            expiresAt,
          })
          .returning()
      )[0]!;
      const draftSummary = this.publicDraft(draft);
      const assistant = (
        await tx
          .insert(chatMessages)
          .values({
            roomId,
            sessionId: session.id,
            senderType: 'ASSISTANT',
            messageType: 'COMMAND_DRAFT',
            content: QUICK_CLEANUP_CONFIRMATION,
            structuredPayload: draftSummary,
          })
          .returning()
      )[0]!;
      const now = new Date();
      const updatedSession = (
        await tx
          .update(chatSessions)
          .set({ title: '빠른 정리 제안', updatedAt: now })
          .where(eq(chatSessions.id, session.id))
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: message.id,
        payload: {
          messageId: message.id,
          sessionId: session.id,
          senderType: message.senderType,
          messageType: message.messageType,
        },
      });
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: assistant.id,
        payload: {
          messageId: assistant.id,
          sessionId: session.id,
          commandDraftId: draft.id,
          senderType: assistant.senderType,
          messageType: assistant.messageType,
        },
      });
      return {
        session: this.publicSession(
          updatedSession,
          assistant.content.slice(0, 120),
          await this.sessionCounters(tx, updatedSession),
        ),
        message: this.publicMessage(message),
        assistant: this.publicMessage(assistant),
        aiStatus: 'READY' as const,
        ai: {
          status: 'READY' as const,
          kind: 'COMMAND_DRAFT' as const,
          commandDraftId: draft.id,
        },
      };
    });
  }

  async createSession(userId: string, roomId: string, title?: string) {
    return this.db.transaction(async (tx) => {
      await this.requireOwnedRoomIn(tx, userId, roomId);
      const active = await tx
        .select()
        .from(chatSessions)
        .where(
          and(
            eq(chatSessions.userId, userId),
            eq(chatSessions.roomId, roomId),
            eq(chatSessions.status, 'ACTIVE'),
          ),
        )
        .orderBy(desc(chatSessions.updatedAt), desc(chatSessions.createdAt))
        .for('update');

      if (active.length >= CHAT_SESSION_LIMIT) {
        throw new ConflictException({
          code: 'CHAT_SESSION_LIMIT_REACHED',
          limit: CHAT_SESSION_LIMIT,
          sessions: await this.withPreviews(tx, active),
        });
      }

      const created = (
        await tx
          .insert(chatSessions)
          .values({
            userId,
            roomId,
            title: title ?? DEFAULT_CHAT_TITLE,
          })
          .returning()
      )[0]!;
      return this.publicSession(created, '');
    });
  }

  async updateSession(userId: string, sessionId: string, title: string) {
    const current = await this.requireOwnedSession(userId, sessionId);
    const updated = (
      await this.db
        .update(chatSessions)
        .set({ title, updatedAt: new Date() })
        .where(eq(chatSessions.id, current.id))
        .returning()
    )[0];
    if (!updated) throw new NotFoundException({ code: 'NOT_FOUND' });
    return this.publicSession(
      updated,
      await this.latestPreview(this.db, updated.id),
      await this.sessionCounters(this.db, updated),
    );
  }

  async deleteSession(userId: string, sessionId: string) {
    const current = await this.requireOwnedSession(userId, sessionId);
    const updated = (
      await this.db
        .update(chatSessions)
        .set({
          status: 'DELETED',
          deletedAt: new Date(),
          updatedAt: new Date(),
        })
        .where(eq(chatSessions.id, current.id))
        .returning()
    )[0];
    if (!updated) throw new NotFoundException({ code: 'NOT_FOUND' });
    return this.publicSession(
      updated,
      await this.latestPreview(this.db, updated.id),
      await this.sessionCounters(this.db, updated),
    );
  }

  async markSessionRead(
    userId: string,
    sessionId: string,
    lastReadMessageId?: string,
  ) {
    return this.db.transaction(async (tx) => {
      const session = await this.requireOwnedSessionIn(tx, userId, sessionId);
      const readMessage = lastReadMessageId
        ? await this.requireSessionMessageIn(tx, session.id, lastReadMessageId)
        : await this.latestSessionMessageIn(tx, session.id);
      const now = new Date();
      const readState = (
        await tx
          .insert(chatReadStates)
          .values({
            userId,
            sessionId: session.id,
            lastReadMessageId: readMessage?.id ?? null,
            readAt: now,
            updatedAt: now,
          })
          .onConflictDoUpdate({
            target: [chatReadStates.userId, chatReadStates.sessionId],
            set: {
              lastReadMessageId: readMessage?.id ?? null,
              readAt: now,
              updatedAt: now,
            },
          })
          .returning()
      )[0]!;
      await tx
        .update(chatSessions)
        .set({ updatedAt: now })
        .where(eq(chatSessions.id, session.id));
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: session.roomId,
        eventType: 'chat.session.read',
        aggregateType: 'chat_session',
        aggregateId: session.id,
        payload: {
          sessionId: session.id,
          lastReadMessageId: readState.lastReadMessageId,
          readAt: readState.readAt.toISOString(),
        },
      });
      return this.publicSession(
        { ...session, updatedAt: now },
        await this.latestPreview(tx, session.id),
        {
          unreadCount: 0,
          pendingActionCount: await this.pendingActionCount(tx, session.id),
          lastReadMessageId: readState.lastReadMessageId,
          readAt: readState.readAt,
        },
      );
    });
  }

  async listMessages(
    userId: string,
    sessionId: string,
    cursor?: string,
    limit = 50,
  ) {
    const session = await this.requireOwnedSession(userId, sessionId);
    let afterCreatedAt: Date | null = null;
    if (cursor) {
      const cursorMessage = (
        await this.db
          .select()
          .from(chatMessages)
          .where(
            and(
              eq(chatMessages.id, cursor),
              eq(chatMessages.sessionId, session.id),
            ),
          )
          .limit(1)
      )[0];
      if (!cursorMessage) throw new NotFoundException({ code: 'NOT_FOUND' });
      afterCreatedAt = cursorMessage.createdAt;
    }

    const rows = await this.db
      .select()
      .from(chatMessages)
      .where(
        and(
          eq(chatMessages.sessionId, session.id),
          ...(afterCreatedAt
            ? [gt(chatMessages.createdAt, afterCreatedAt)]
            : []),
        ),
      )
      .orderBy(asc(chatMessages.createdAt), asc(chatMessages.id))
      .limit(limit);
    return this.hydrateMessages(this.db, rows);
  }

  async createMessage(userId: string, sessionId: string, content: string) {
    const session = await this.requireOwnedSession(userId, sessionId);
    return this.createMessageForSession(userId, session, content);
  }

  async createCommandDraft(
    userId: string,
    sessionId: string,
    body: z.infer<typeof createCommandDraftSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const session = await this.requireOwnedSessionIn(tx, userId, sessionId);
      const sourceMessage = (
        await tx
          .select()
          .from(chatMessages)
          .where(
            and(
              eq(chatMessages.id, body.sourceMessageId),
              eq(chatMessages.sessionId, session.id),
              eq(chatMessages.senderType, 'USER'),
            ),
          )
          .limit(1)
      )[0];
      if (!sourceMessage) throw new NotFoundException({ code: 'NOT_FOUND' });
      const expiresAt = body.expiresAt
        ? new Date(body.expiresAt)
        : new Date(Date.now() + DEFAULT_DRAFT_TTL_MS);
      const draft = (
        await tx
          .insert(commandDrafts)
          .values({
            sessionId: session.id,
            sourceMessageId: sourceMessage.id,
            roomId: session.roomId,
            createdByUserId: userId,
            intent: body.command.intent,
            arguments: body.command.payload,
            confirmationSummary: body.confirmationSummary,
            expiresAt,
          })
          .returning()
      )[0]!;
      const draftSummary = this.publicDraft(draft);
      const message = (
        await tx
          .insert(chatMessages)
          .values({
            roomId: session.roomId,
            sessionId: session.id,
            senderType: 'ASSISTANT',
            messageType: 'COMMAND_DRAFT',
            content: body.confirmationSummary,
            structuredPayload: draftSummary,
          })
          .returning()
      )[0]!;
      await tx
        .update(chatSessions)
        .set({ updatedAt: new Date() })
        .where(eq(chatSessions.id, session.id));
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: session.roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: message.id,
        payload: {
          messageId: message.id,
          sessionId: session.id,
          commandDraftId: draft.id,
          senderType: message.senderType,
          messageType: message.messageType,
        },
      });
      return { draft: draftSummary, message: this.publicMessage(message) };
    });
  }

  async confirmCommandDraft(userId: string, draftId: string, key: string) {
    return this.db.transaction(async (tx) => {
      const owned = await this.requireOwnedDraftIn(tx, userId, draftId);
      const draft = owned.draft;
      if (draft.status === 'MATERIALIZED') {
        if (draft.commandId) {
          const command = await this.requireMaterializedCommandIn(
            tx,
            draft.commandId,
            key,
          );
          return {
            draft: this.publicDraft(draft),
            command: this.publicCommand(command),
          };
        }
        if (draft.fileBrowseRequestId) {
          this.requireMaterializedDraftKey(draft, key);
          const fileBrowseRequest =
            await this.requireMaterializedFileBrowseRequestIn(
              tx,
              draft.fileBrowseRequestId,
            );
          return {
            draft: this.publicDraft(draft),
            fileBrowseRequest: this.publicFileBrowseRequest(fileBrowseRequest),
          };
        }
        if (draft.fileTransferId) {
          this.requireMaterializedDraftKey(draft, key);
          const fileTransfer = await this.requireMaterializedFileTransferIn(
            tx,
            draft.fileTransferId,
          );
          return {
            draft: this.publicDraft(draft),
            fileTransfer: this.publicFileTransfer(fileTransfer),
          };
        }
      }
      if (draft.status === 'REJECTED') {
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: draft.status,
          to: 'MATERIALIZED',
        });
      }
      if (draft.status === 'EXPIRED' || draft.expiresAt <= new Date()) {
        const expired =
          draft.status === 'EXPIRED'
            ? draft
            : (
                await tx
                  .update(commandDrafts)
                  .set({ status: 'EXPIRED' })
                  .where(eq(commandDrafts.id, draft.id))
                  .returning()
              )[0]!;
        throw new ConflictException({
          code: 'DRAFT_EXPIRED',
          draft: this.publicDraft(expired),
        });
      }
      if (draft.status !== 'DRAFT') {
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: draft.status,
          to: 'MATERIALIZED',
        });
      }
      await this.ensureConfirmKeyAvailableIn(tx, userId, key, draft.id);
      if (draft.intent === 'FIND') {
        return this.materializeFindDraftIn(tx, userId, owned, key);
      }
      if (draft.intent === 'DOWNLOAD') {
        return this.materializeDownloadDraftIn(tx, userId, owned, key);
      }
      if (draft.intent === 'UPLOAD') {
        return this.materializeUploadDraftIn(tx, userId, owned);
      }
      const device = await this.requireActiveRoomDeviceIn(
        tx,
        userId,
        owned.session.roomId,
      );
      const metadata = {
        sessionId: owned.session.id,
        sourceMessageId: draft.sourceMessageId,
        commandDraftId: draft.id,
        idempotencyKey: key,
        requiresApproval: true,
      };
      const created = (
        await tx
          .insert(commands)
          .values({
            roomId: owned.session.roomId,
            targetDeviceId: device.id,
            createdByUserId: userId,
            intent: draft.intent,
            payload: draft.arguments,
            metadata,
            idempotencyKey: key,
          })
          .onConflictDoNothing()
          .returning()
      )[0];
      const command =
        created ??
        (await this.requireExistingCommandForDraftIn(
          tx,
          userId,
          key,
          owned.session.roomId,
          draft,
          metadata,
        ));
      const updated = (
        await tx
          .update(commandDrafts)
          .set({
            status: 'MATERIALIZED',
            commandId: command.id,
            confirmIdempotencyKey: key,
          })
          .where(eq(commandDrafts.id, draft.id))
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: device.id,
        roomId: owned.session.roomId,
        eventType: 'command.available',
        aggregateType: 'command',
        aggregateId: command.id,
        payload: { commandId: command.id, commandDraftId: draft.id },
      });
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: owned.session.roomId,
        eventType: 'command.draft.updated',
        aggregateType: 'command_draft',
        aggregateId: draft.id,
        payload: {
          commandDraftId: draft.id,
          status: updated.status,
          commandId: command.id,
        },
      });
      return {
        draft: this.publicDraft(updated),
        command: this.publicCommand(command),
      };
    });
  }

  private async materializeFindDraftIn(
    tx: Transaction,
    userId: string,
    owned: {
      draft: typeof commandDrafts.$inferSelect;
      session: typeof chatSessions.$inferSelect;
    },
    key: string,
  ) {
    if (!this.browse) {
      throw new ConflictException({ code: 'FILE_BROWSE_UNCONFIGURED' });
    }
    const parsed = findCommandPayloadSchema.safeParse(owned.draft.arguments);
    if (!parsed.success) {
      throw new ConflictException({ code: 'INVALID_DRAFT_ARGUMENTS' });
    }
    const room = await this.requireOwnedRoomIn(
      tx,
      userId,
      owned.session.roomId,
    );
    if (parsed.data.rootId !== room.rootAlias) {
      throw new ConflictException({ code: 'ROOT_MISMATCH' });
    }
    const browseBody = createFileBrowseRequestSchema.parse({
      relativeDirectory: parsed.data.scopeRelativePath ?? '',
      cursor: null,
      query: parsed.data.query,
      extensions: parsed.data.extensions ?? [],
      limit: parsed.data.limit,
      searchScope: parsed.data.scopeRelativePath
        ? 'CURRENT_DIRECTORY'
        : 'MANAGED_ROOT',
    });
    const fileBrowseRequest = await this.browse.createInTransaction(
      tx,
      userId,
      owned.session.roomId,
      browseBody,
    );
    const updated = (
      await tx
        .update(commandDrafts)
        .set({
          status: 'MATERIALIZED',
          fileBrowseRequestId: fileBrowseRequest.id,
          confirmIdempotencyKey: key,
        })
        .where(eq(commandDrafts.id, owned.draft.id))
        .returning()
    )[0]!;
    await this.sync.append(tx, {
      userId,
      deviceId: null,
      roomId: owned.session.roomId,
      eventType: 'command.draft.updated',
      aggregateType: 'command_draft',
      aggregateId: owned.draft.id,
      payload: {
        commandDraftId: owned.draft.id,
        status: updated.status,
        fileBrowseRequestId: fileBrowseRequest.id,
      },
    });
    return {
      draft: this.publicDraft(updated),
      fileBrowseRequest: this.publicFileBrowseRequest(fileBrowseRequest),
    };
  }

  private async materializeDownloadDraftIn(
    tx: Transaction,
    userId: string,
    owned: {
      draft: typeof commandDrafts.$inferSelect;
      session: typeof chatSessions.$inferSelect;
    },
    key: string,
  ) {
    if (!this.transfers) {
      throw new ConflictException({ code: 'FILE_TRANSFER_UNCONFIGURED' });
    }
    const parsed = downloadCommandPayloadSchema.safeParse(
      owned.draft.arguments,
    );
    if (!parsed.success) {
      throw new ConflictException({ code: 'INVALID_DRAFT_ARGUMENTS' });
    }
    const room = await this.requireOwnedRoomIn(
      tx,
      userId,
      owned.session.roomId,
    );
    if (parsed.data.rootId !== room.rootAlias) {
      throw new ConflictException({ code: 'ROOT_MISMATCH' });
    }
    if (parsed.data.expectedIdentity) {
      throw new ConflictException({ code: 'EXPECTED_IDENTITY_UNSUPPORTED' });
    }

    const transferBody = createFileTransferSchema.parse({
      sourceRelativePath: parsed.data.sourceRelativePath,
    });
    const fileTransfer = await this.transfers.createInTransaction(
      tx,
      userId,
      owned.session.roomId,
      key,
      transferBody,
    );
    const updated = (
      await tx
        .update(commandDrafts)
        .set({
          status: 'MATERIALIZED',
          fileTransferId: fileTransfer.id,
          confirmIdempotencyKey: key,
        })
        .where(eq(commandDrafts.id, owned.draft.id))
        .returning()
    )[0]!;
    await this.sync.append(tx, {
      userId,
      deviceId: null,
      roomId: owned.session.roomId,
      eventType: 'command.draft.updated',
      aggregateType: 'command_draft',
      aggregateId: owned.draft.id,
      payload: {
        commandDraftId: owned.draft.id,
        status: updated.status,
        fileTransferId: fileTransfer.id,
      },
    });
    return {
      draft: this.publicDraft(updated),
      fileTransfer,
    };
  }

  private async materializeUploadDraftIn(
    tx: Transaction,
    userId: string,
    owned: {
      draft: typeof commandDrafts.$inferSelect;
      session: typeof chatSessions.$inferSelect;
    },
  ): Promise<never> {
    const parsed = uploadCommandPayloadSchema.safeParse(owned.draft.arguments);
    if (!parsed.success) {
      throw new ConflictException({ code: 'INVALID_DRAFT_ARGUMENTS' });
    }
    const room = await this.requireOwnedRoomIn(
      tx,
      userId,
      owned.session.roomId,
    );
    if (parsed.data.rootId !== room.rootAlias) {
      throw new ConflictException({ code: 'ROOT_MISMATCH' });
    }

    throw new ConflictException({
      code: 'UPLOAD_TRANSFER_UNCONFIGURED',
      message:
        'UPLOAD drafts require a mobile-to-desktop transfer state machine and are not materialized as desktop commands.',
    });
  }

  async rejectCommandDraft(userId: string, draftId: string) {
    return this.db.transaction(async (tx) => {
      const owned = await this.requireOwnedDraftIn(tx, userId, draftId);
      const draft = owned.draft;
      if (draft.status === 'REJECTED') {
        return { draft: this.publicDraft(draft) };
      }
      if (draft.status === 'MATERIALIZED') {
        throw new ConflictException({
          code: 'INVALID_STATE_TRANSITION',
          from: draft.status,
          to: 'REJECTED',
        });
      }
      const nextStatus = draft.expiresAt <= new Date() ? 'EXPIRED' : 'REJECTED';
      const updated = (
        await tx
          .update(commandDrafts)
          .set({ status: nextStatus })
          .where(eq(commandDrafts.id, draft.id))
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: owned.session.roomId,
        eventType: 'command.draft.updated',
        aggregateType: 'command_draft',
        aggregateId: draft.id,
        payload: {
          commandDraftId: draft.id,
          status: updated.status,
          commandId: updated.commandId,
        },
      });
      return { draft: this.publicDraft(updated) };
    });
  }

  async listLegacyRoomMessages(userId: string, roomId: string) {
    await this.requireOwnedRoom(userId, roomId);
    const rows = await this.db
      .select({ message: chatMessages })
      .from(chatMessages)
      .leftJoin(chatSessions, eq(chatMessages.sessionId, chatSessions.id))
      .where(
        and(
          eq(chatMessages.roomId, roomId),
          or(isNull(chatMessages.sessionId), eq(chatSessions.status, 'ACTIVE')),
        ),
      )
      .orderBy(asc(chatMessages.createdAt), asc(chatMessages.id))
      .limit(200);
    return this.hydrateMessages(
      this.db,
      rows.map(({ message }) => message),
    );
  }

  async createLegacyRoomMessage(
    userId: string,
    roomId: string,
    content: string,
  ) {
    const created = await this.db.transaction(async (tx) => {
      await this.requireOwnedRoomIn(tx, userId, roomId);
      let session = (
        await tx
          .select()
          .from(chatSessions)
          .where(
            and(
              eq(chatSessions.userId, userId),
              eq(chatSessions.roomId, roomId),
              eq(chatSessions.status, 'ACTIVE'),
            ),
          )
          .orderBy(desc(chatSessions.updatedAt), desc(chatSessions.createdAt))
          .limit(1)
      )[0];
      if (!session) {
        session = (
          await tx
            .insert(chatSessions)
            .values({ userId, roomId, title: titleFromContent(content) })
            .returning()
        )[0]!;
      }
      const result = await this.createMessageForSessionIn(
        tx,
        userId,
        session,
        content,
      );
      return { session, message: result.message };
    });
    return this.withAiResult(userId, created.session, created.message);
  }

  private async createMessageForSession(
    userId: string,
    session: typeof chatSessions.$inferSelect,
    content: string,
  ) {
    const result = await this.db.transaction((tx) =>
      this.createMessageForSessionIn(tx, userId, session, content),
    );
    return this.withAiResult(userId, session, result.message);
  }

  private async createMessageForSessionIn(
    tx: Transaction,
    userId: string,
    session: typeof chatSessions.$inferSelect,
    content: string,
  ) {
    const message = (
      await tx
        .insert(chatMessages)
        .values({
          roomId: session.roomId,
          sessionId: session.id,
          senderType: 'USER',
          messageType: 'TEXT',
          content,
        })
        .returning()
    )[0]!;
    const nextTitle =
      session.title === DEFAULT_CHAT_TITLE
        ? titleFromContent(content)
        : session.title;
    await tx
      .update(chatSessions)
      .set({ title: nextTitle, updatedAt: new Date() })
      .where(eq(chatSessions.id, session.id));
    await this.sync.append(tx, {
      userId,
      deviceId: null,
      roomId: session.roomId,
      eventType: 'chat.message.created',
      aggregateType: 'chat_message',
      aggregateId: message.id,
      payload: {
        messageId: message.id,
        sessionId: session.id,
        senderType: message.senderType,
        messageType: message.messageType,
      },
    });
    return {
      message: this.publicMessage(message),
    };
  }

  private async withAiResult(
    userId: string,
    session: typeof chatSessions.$inferSelect,
    message: PublicChatMessage,
  ) {
    const ai = await this.ai.classifyAndRespond({
      userId,
      roomId: session.roomId,
      sessionId: session.id,
      sourceMessage: {
        id: message.id,
        content: message.content,
      },
    });
    if (ai.status === 'READY' && ai.kind === 'NO_ACTION') {
      const assistant = await this.createAiTextAssistantMessage(
        userId,
        session,
        ai.reply,
      );
      return {
        message,
        assistant,
        aiStatus: 'READY' as const,
        ai: {
          status: 'READY' as const,
          kind: 'NO_ACTION' as const,
        },
      };
    }
    if (ai.status === 'READY' && ai.kind === 'RULE_DRAFT') {
      const draft = await this.createRuleDraftFromAi(userId, session, message, ai);
      if (!draft) {
        return {
          message,
          assistant: null,
          aiStatus: 'INVALID' as const,
          ai: {
            status: 'INVALID' as const,
            code: 'AI_OUTPUT_INVALID' as const,
          },
        };
      }
      return {
        message,
        assistant: draft.message,
        aiStatus: 'READY' as const,
        ai: {
          status: 'READY' as const,
          kind: 'RULE_DRAFT' as const,
          ruleDraftId: draft.draft.id,
        },
      };
    }
    if (ai.status === 'READY' && ai.kind === 'QUERY') {
      const query = await this.createQueryResultFromAi(userId, session, ai);
      return {
        message,
        assistant: query.message,
        aiStatus: 'READY' as const,
        ai: {
          status: 'READY' as const,
          kind: 'QUERY' as const,
          fileBrowseRequestId: query.fileBrowseRequest?.id ?? null,
          cacheHitCount: query.cacheEntries.length,
        },
      };
    }
    const mapped = mapAiResultToCommandDraft(message.id, ai);
    if (mapped.kind === 'NO_DRAFT' || mapped.kind === 'INVALID') {
      return {
        message,
        assistant: null,
        aiStatus: mapped.aiStatus,
        ai: mapped.ai,
      };
    }
    const draft = await this.createCommandDraft(
      userId,
      session.id,
      mapped.draftInput,
    );
    return {
      message,
      assistant: draft.message,
      aiStatus: 'READY' as const,
      ai: {
        status: 'READY' as const,
        kind: 'COMMAND_DRAFT' as const,
        commandDraftId: draft.draft.id,
      },
    };
  }

  private async createQueryResultFromAi(
    userId: string,
    session: typeof chatSessions.$inferSelect,
    ai: AiQueryResult,
  ) {
    const browseBody = createFileBrowseRequestSchema.safeParse(ai.browse);
    if (!browseBody.success) {
      return this.createInvalidAiAssistantMessage(userId, session);
    }
    return this.db.transaction(async (tx) => {
      const currentSession = await this.requireOwnedSessionIn(
        tx,
        userId,
        session.id,
      );
      const cacheEntries = await this.cachedBrowseEntriesIn(
        tx,
        currentSession.roomId,
        browseBody.data,
      );
      const fileBrowseRequest = this.browse
        ? await this.browse.createInTransaction(
            tx,
            userId,
            currentSession.roomId,
            browseBody.data,
          )
        : null;
      const payload = {
        kind: 'QUERY_RESULT',
        cacheMode:
          fileBrowseRequest == null || fileBrowseRequest.status === 'FAILED'
            ? 'CACHE_ONLY'
            : 'CACHE_AND_LIVE',
        cacheEntries,
        cacheHitCount: cacheEntries.length,
        liveBrowseRequest: fileBrowseRequest
          ? this.publicFileBrowseRequest(fileBrowseRequest)
          : null,
      };
      const message = (
        await tx
          .insert(chatMessages)
          .values({
            roomId: currentSession.roomId,
            sessionId: currentSession.id,
            senderType: 'ASSISTANT',
            messageType: 'QUERY_RESULT',
            content: queryResultContent(
              ai.responseSummary,
              cacheEntries.length,
              fileBrowseRequest?.status ?? null,
            ),
            structuredPayload: payload,
          })
          .returning()
      )[0]!;
      await tx
        .update(chatSessions)
        .set({ updatedAt: new Date() })
        .where(eq(chatSessions.id, currentSession.id));
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: currentSession.roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: message.id,
        payload: {
          messageId: message.id,
          sessionId: currentSession.id,
          fileBrowseRequestId: fileBrowseRequest?.id ?? null,
          senderType: message.senderType,
          messageType: message.messageType,
        },
      });
      return {
        message: this.publicMessage(message),
        fileBrowseRequest,
        cacheEntries,
      };
    });
  }

  private async createInvalidAiAssistantMessage(
    userId: string,
    session: typeof chatSessions.$inferSelect,
  ) {
    return this.db.transaction(async (tx) => {
      const currentSession = await this.requireOwnedSessionIn(
        tx,
        userId,
        session.id,
      );
      const message = (
        await tx
          .insert(chatMessages)
          .values({
            roomId: currentSession.roomId,
            sessionId: currentSession.id,
            senderType: 'ASSISTANT',
            messageType: 'TEXT',
            content: 'AI output failed schema validation, so no query was run.',
            structuredPayload: {
              status: 'INVALID',
              code: 'AI_OUTPUT_INVALID',
            },
          })
          .returning()
      )[0]!;
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: currentSession.roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: message.id,
        payload: {
          messageId: message.id,
          sessionId: currentSession.id,
          senderType: message.senderType,
          messageType: message.messageType,
        },
      });
      return {
        message: this.publicMessage(message),
        fileBrowseRequest: null,
        cacheEntries: [],
      };
    });
  }

  private async createAiTextAssistantMessage(
    userId: string,
    session: typeof chatSessions.$inferSelect,
    content: string,
  ) {
    return this.db.transaction(async (tx) => {
      const currentSession = await this.requireOwnedSessionIn(
        tx,
        userId,
        session.id,
      );
      const message = (
        await tx
          .insert(chatMessages)
          .values({
            roomId: currentSession.roomId,
            sessionId: currentSession.id,
            senderType: 'ASSISTANT',
            messageType: 'TEXT',
            content,
          })
          .returning()
      )[0]!;
      await tx
        .update(chatSessions)
        .set({ updatedAt: new Date() })
        .where(eq(chatSessions.id, currentSession.id));
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: currentSession.roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: message.id,
        payload: {
          messageId: message.id,
          sessionId: currentSession.id,
          senderType: message.senderType,
          messageType: message.messageType,
        },
      });
      return this.publicMessage(message);
    });
  }

  private async createRuleDraftFromAi(
    userId: string,
    session: typeof chatSessions.$inferSelect,
    sourceMessage: PublicChatMessage,
    ai: AiRuleDraftResult,
  ) {
    const definition = ruleDefinitionSchema.safeParse(ai.draft.definition);
    if (!definition.success) {
      return null;
    }
    return this.db.transaction(async (tx) => {
      const currentSession = await this.requireOwnedSessionIn(
        tx,
        userId,
        session.id,
      );
      const currentSource = await this.requireSessionMessageIn(
        tx,
        currentSession.id,
        sourceMessage.id,
      );
      const draft = (
        await tx
          .insert(ruleDrafts)
          .values({
            sessionId: currentSession.id,
            sourceMessageId: currentSource.id,
            roomId: currentSession.roomId,
            createdByUserId: userId,
            name: ai.draft.name,
            definition: definition.data,
            explanation: ai.draft.explanation,
            ambiguities: ai.draft.ambiguities,
            expiresAt: new Date(Date.now() + DEFAULT_DRAFT_TTL_MS),
          })
          .returning()
      )[0]!;
      const draftSummary = this.publicRuleDraft(draft);
      const message = (
        await tx
          .insert(chatMessages)
          .values({
            roomId: currentSession.roomId,
            sessionId: currentSession.id,
            senderType: 'ASSISTANT',
            messageType: 'RULE_DRAFT',
            content: ai.draft.explanation,
            structuredPayload: draftSummary,
          })
          .returning()
      )[0]!;
      await tx
        .update(chatSessions)
        .set({ updatedAt: new Date() })
        .where(eq(chatSessions.id, currentSession.id));
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: currentSession.roomId,
        eventType: 'rule.draft.created',
        aggregateType: 'rule_draft',
        aggregateId: draft.id,
        payload: {
          ruleDraftId: draft.id,
          sessionId: currentSession.id,
          sourceMessageId: currentSource.id,
          status: draft.status,
        },
      });
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: currentSession.roomId,
        eventType: 'chat.message.created',
        aggregateType: 'chat_message',
        aggregateId: message.id,
        payload: {
          messageId: message.id,
          sessionId: currentSession.id,
          ruleDraftId: draft.id,
          senderType: message.senderType,
          messageType: message.messageType,
        },
      });
      return { draft: draftSummary, message: this.publicMessage(message) };
    });
  }

  private async requireOwnedRoom(userId: string, roomId: string) {
    return this.requireOwnedRoomIn(this.db, userId, roomId);
  }

  private async requireOwnedRoomIn(
    db: DbExecutor,
    userId: string,
    roomId: string,
  ) {
    const room = (
      await db
        .select()
        .from(rooms)
        .where(
          and(
            eq(rooms.id, roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
    return room;
  }

  private async requireOwnedSession(userId: string, sessionId: string) {
    return this.requireOwnedSessionIn(this.db, userId, sessionId);
  }

  private async requireOwnedSessionIn(
    db: DbExecutor,
    userId: string,
    sessionId: string,
  ) {
    const row = (
      await db
        .select({ session: chatSessions })
        .from(chatSessions)
        .innerJoin(
          rooms,
          and(
            eq(rooms.id, chatSessions.roomId),
            eq(rooms.status, 'ACTIVE'),
            eq(rooms.userId, userId),
          ),
        )
        .where(
          and(
            eq(chatSessions.id, sessionId),
            eq(chatSessions.userId, userId),
            eq(chatSessions.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!row) throw new NotFoundException({ code: 'NOT_FOUND' });
    return row.session;
  }

  private async requireOwnedDraftIn(
    tx: Transaction,
    userId: string,
    draftId: string,
  ) {
    const row = (
      await tx
        .select({ draft: commandDrafts, session: chatSessions })
        .from(commandDrafts)
        .innerJoin(
          chatSessions,
          and(
            eq(chatSessions.id, commandDrafts.sessionId),
            eq(chatSessions.userId, userId),
            eq(chatSessions.status, 'ACTIVE'),
          ),
        )
        .innerJoin(
          rooms,
          and(
            eq(rooms.id, chatSessions.roomId),
            eq(rooms.status, 'ACTIVE'),
            eq(rooms.userId, userId),
          ),
        )
        .where(
          and(
            eq(commandDrafts.id, draftId),
            eq(commandDrafts.createdByUserId, userId),
          ),
        )
        .for('update')
        .limit(1)
    )[0];
    if (!row) throw new NotFoundException({ code: 'NOT_FOUND' });
    return row;
  }

  private async requireActiveRoomDeviceIn(
    tx: Transaction,
    userId: string,
    roomId: string,
  ) {
    const row = (
      await tx
        .select({ device: devices })
        .from(rooms)
        .innerJoin(
          devices,
          and(
            eq(devices.id, rooms.desktopDeviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .where(
          and(
            eq(rooms.id, roomId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!row) throw new NotFoundException({ code: 'NOT_FOUND' });
    return row.device;
  }

  private async requireExistingCommandForDraftIn(
    tx: Transaction,
    userId: string,
    key: string,
    roomId: string,
    draft: typeof commandDrafts.$inferSelect,
    metadata: Record<string, unknown>,
  ) {
    const existing = (
      await tx
        .select()
        .from(commands)
        .where(
          and(
            eq(commands.createdByUserId, userId),
            eq(commands.idempotencyKey, key),
          ),
        )
        .limit(1)
    )[0];
    if (!existing) throw new ConflictException({ code: 'CONFLICT' });
    if (
      existing.roomId !== roomId ||
      existing.intent !== draft.intent ||
      canonicalJson(existing.payload) !== canonicalJson(draft.arguments) ||
      canonicalJson(existing.metadata) !== canonicalJson(metadata)
    ) {
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
    return existing;
  }

  private async requireMaterializedCommandIn(
    tx: Transaction,
    commandId: string,
    key: string,
  ) {
    const command = (
      await tx
        .select()
        .from(commands)
        .where(eq(commands.id, commandId))
        .limit(1)
    )[0];
    if (!command) throw new ConflictException({ code: 'CONFLICT' });
    if (command.idempotencyKey !== key) {
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
    return command;
  }

  private requireMaterializedDraftKey(
    draft: typeof commandDrafts.$inferSelect,
    key: string,
  ) {
    if (draft.confirmIdempotencyKey !== key) {
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
  }

  private async requireMaterializedFileBrowseRequestIn(
    tx: Transaction,
    requestId: string,
  ) {
    const request = (
      await tx
        .select()
        .from(fileBrowseRequests)
        .where(eq(fileBrowseRequests.id, requestId))
        .limit(1)
    )[0];
    if (!request) throw new ConflictException({ code: 'CONFLICT' });
    return request;
  }

  private async requireMaterializedFileTransferIn(
    tx: Transaction,
    transferId: string,
  ) {
    const transfer = (
      await tx
        .select()
        .from(fileTransfers)
        .where(eq(fileTransfers.id, transferId))
        .limit(1)
    )[0];
    if (!transfer) throw new ConflictException({ code: 'CONFLICT' });
    return transfer;
  }

  private async ensureConfirmKeyAvailableIn(
    tx: Transaction,
    userId: string,
    key: string,
    draftId: string,
  ) {
    const existingCommand = (
      await tx
        .select({ id: commands.id })
        .from(commands)
        .where(
          and(
            eq(commands.createdByUserId, userId),
            eq(commands.idempotencyKey, key),
          ),
        )
        .limit(1)
    )[0];
    if (existingCommand) {
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
    const existingTransfer = (
      await tx
        .select({ id: fileTransfers.id })
        .from(fileTransfers)
        .where(
          and(
            eq(fileTransfers.requestedByUserId, userId),
            eq(fileTransfers.idempotencyKey, key),
          ),
        )
        .limit(1)
    )[0];
    if (existingTransfer) {
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
    const existingDraft = (
      await tx
        .select({ id: commandDrafts.id })
        .from(commandDrafts)
        .where(
          and(
            eq(commandDrafts.createdByUserId, userId),
            eq(commandDrafts.confirmIdempotencyKey, key),
            ne(commandDrafts.id, draftId),
          ),
        )
        .limit(1)
    )[0];
    if (existingDraft) {
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
  }

  private async withPreviews(
    db: DbExecutor,
    sessions: (typeof chatSessions.$inferSelect)[],
  ) {
    const result = [];
    for (const session of sessions) {
      result.push(
        this.publicSession(
          session,
          await this.latestPreview(db, session.id),
          await this.sessionCounters(db, session),
        ),
      );
    }
    return result;
  }

  private async getOrCreateSystemSessionIn(
    tx: Transaction,
    userId: string,
    roomId: string,
    title: string,
  ) {
    const latest = (
      await tx
        .select()
        .from(chatSessions)
        .where(
          and(
            eq(chatSessions.userId, userId),
            eq(chatSessions.roomId, roomId),
            eq(chatSessions.status, 'ACTIVE'),
          ),
        )
        .orderBy(desc(chatSessions.updatedAt), desc(chatSessions.createdAt))
        .limit(1)
    )[0];
    if (latest) return latest;
    return (
      await tx
        .insert(chatSessions)
        .values({
          userId,
          roomId,
          title,
        })
        .returning()
    )[0]!;
  }

  /**
   * Resolves the chat session a system-generated message (proposal, execution
   * result) should be posted into. Reuses the session the originating command
   * was drafted from (via `commands.metadata.sessionId`, set by
   * `confirmCommandDraft`) when available, otherwise falls back to the room's
   * latest active session so commands created outside of chat (e.g. mobile's
   * direct `POST /v1/rooms/:roomId/commands`) still surface in chat.
   */
  async resolveSessionForCommandIn(
    tx: Transaction,
    userId: string,
    roomId: string,
    commandMetadata: unknown,
  ) {
    const metadataSessionId =
      commandMetadata &&
      typeof commandMetadata === 'object' &&
      !Array.isArray(commandMetadata) &&
      typeof (commandMetadata as Record<string, unknown>).sessionId ===
        'string'
        ? ((commandMetadata as Record<string, unknown>).sessionId as string)
        : null;
    if (metadataSessionId) {
      const session = (
        await tx
          .select()
          .from(chatSessions)
          .where(
            and(
              eq(chatSessions.id, metadataSessionId),
              eq(chatSessions.userId, userId),
              eq(chatSessions.roomId, roomId),
              eq(chatSessions.status, 'ACTIVE'),
            ),
          )
          .limit(1)
      )[0];
      if (session) return session;
    }
    return this.getOrCreateSystemSessionIn(tx, userId, roomId, '자동 제안');
  }

  /** Inserts an assistant chat message and emits `chat.message.created`, for use by services outside the chat module (proposals, executions). */
  async postSystemMessageIn(
    tx: Transaction,
    params: {
      userId: string;
      roomId: string;
      sessionId: string;
      messageType: string;
      content: string;
      structuredPayload: Record<string, unknown>;
      commandId?: string | null;
    },
  ) {
    const message = (
      await tx
        .insert(chatMessages)
        .values({
          roomId: params.roomId,
          sessionId: params.sessionId,
          senderType: 'ASSISTANT',
          messageType: params.messageType,
          content: params.content,
          structuredPayload: params.structuredPayload,
          commandId: params.commandId ?? null,
        })
        .returning()
    )[0]!;
    await tx
      .update(chatSessions)
      .set({ updatedAt: new Date() })
      .where(eq(chatSessions.id, params.sessionId));
    await this.sync.append(tx, {
      userId: params.userId,
      deviceId: null,
      roomId: params.roomId,
      eventType: 'chat.message.created',
      aggregateType: 'chat_message',
      aggregateId: message.id,
      payload: {
        messageId: message.id,
        sessionId: params.sessionId,
        senderType: message.senderType,
        messageType: message.messageType,
      },
    });
    return this.publicMessage(message);
  }

  /**
   * Overlays live status from `commandDrafts`/`ruleDrafts`/`proposals` onto
   * the frozen `structuredPayload` stored on each message row. The stored
   * payload only ever reflects the state at message-creation time (confirm
   * /reject/decide/execute never rewrite the chat_messages row), so without
   * this step a re-fetched message list (or the pending-suggestions list)
   * would show a draft as perpetually pending even after it was resolved.
   */
  private async hydrateMessages(
    db: DbExecutor,
    rows: (typeof chatMessages.$inferSelect)[],
  ): Promise<PublicChatMessage[]> {
    const commandDraftIds = new Set<string>();
    const ruleDraftIds = new Set<string>();
    const proposalIds = new Set<string>();
    for (const row of rows) {
      const summary = draftSummaryFromPayload(row.structuredPayload);
      if (!summary) continue;
      if (row.messageType === 'COMMAND_DRAFT') commandDraftIds.add(summary.id);
      else if (row.messageType === 'RULE_DRAFT') ruleDraftIds.add(summary.id);
      else if (row.messageType === 'PROPOSAL') proposalIds.add(summary.id);
    }
    const [liveCommandDrafts, liveRuleDrafts, liveProposals] =
      await Promise.all([
        commandDraftIds.size
          ? db
              .select()
              .from(commandDrafts)
              .where(inArray(commandDrafts.id, [...commandDraftIds]))
          : Promise.resolve([]),
        ruleDraftIds.size
          ? db
              .select()
              .from(ruleDrafts)
              .where(inArray(ruleDrafts.id, [...ruleDraftIds]))
          : Promise.resolve([]),
        proposalIds.size
          ? db
              .select()
              .from(proposals)
              .where(inArray(proposals.id, [...proposalIds]))
          : Promise.resolve([]),
      ]);
    const commandDraftById = new Map(liveCommandDrafts.map((d) => [d.id, d]));
    const ruleDraftById = new Map(liveRuleDrafts.map((d) => [d.id, d]));
    const proposalById = new Map(liveProposals.map((p) => [p.id, p]));
    return rows.map((row) => {
      const message = this.publicMessage(row);
      const summary = draftSummaryFromPayload(row.structuredPayload);
      const payload = message.structuredPayload;
      if (
        !summary ||
        payload === null ||
        typeof payload !== 'object' ||
        Array.isArray(payload)
      ) {
        return message;
      }
      const base = payload as Record<string, unknown>;
      if (row.messageType === 'COMMAND_DRAFT') {
        const live = commandDraftById.get(summary.id);
        if (live) {
          message.structuredPayload = {
            ...base,
            status: live.status,
            commandId: live.commandId,
            fileBrowseRequestId: live.fileBrowseRequestId,
            fileTransferId: live.fileTransferId,
          };
        }
      } else if (row.messageType === 'RULE_DRAFT') {
        const live = ruleDraftById.get(summary.id);
        if (live) {
          message.structuredPayload = {
            ...base,
            status: live.status,
            ruleId: live.ruleId,
          };
        }
      } else if (row.messageType === 'PROPOSAL') {
        const live = proposalById.get(summary.id);
        if (live) {
          message.structuredPayload = {
            ...base,
            status: live.status,
          };
        }
      }
      return message;
    });
  }

  private async quickHistory(userId: string, roomId: string) {
    const rows = await this.db
      .select({ message: chatMessages, session: chatSessions })
      .from(chatMessages)
      .innerJoin(
        chatSessions,
        and(
          eq(chatSessions.id, chatMessages.sessionId),
          eq(chatSessions.userId, userId),
          eq(chatSessions.roomId, roomId),
          eq(chatSessions.status, 'ACTIVE'),
        ),
      )
      .where(eq(chatMessages.roomId, roomId))
      .orderBy(desc(chatMessages.createdAt), desc(chatMessages.id))
      .limit(8);
    return rows.map(({ message, session }) => ({
      messageId: message.id,
      sessionId: session.id,
      sessionTitle: session.title,
      senderType: message.senderType,
      messageType: message.messageType,
      content: message.content,
      createdAt: message.createdAt,
    }));
  }

  private async quickPendingSuggestions(userId: string, roomId: string) {
    const rows = await this.db
      .select({ message: chatMessages, session: chatSessions })
      .from(chatMessages)
      .innerJoin(
        chatSessions,
        and(
          eq(chatSessions.id, chatMessages.sessionId),
          eq(chatSessions.userId, userId),
          eq(chatSessions.roomId, roomId),
          eq(chatSessions.status, 'ACTIVE'),
        ),
      )
      .where(
        and(
          eq(chatMessages.roomId, roomId),
          eq(chatMessages.senderType, 'ASSISTANT'),
          or(
            eq(chatMessages.messageType, 'COMMAND_DRAFT'),
            eq(chatMessages.messageType, 'RULE_DRAFT'),
            eq(chatMessages.messageType, 'PROPOSAL'),
          ),
        ),
      )
      .orderBy(desc(chatMessages.createdAt), desc(chatMessages.id))
      .limit(24);
    const hydrated = await this.hydrateMessages(
      this.db,
      rows.map(({ message }) => message),
    );
    const sessionByMessageId = new Map(
      rows.map(({ message, session }) => [message.id, session]),
    );
    const pendingStatusByType: Record<string, string> = {
      COMMAND_DRAFT: 'DRAFT',
      RULE_DRAFT: 'DRAFT',
      PROPOSAL: 'OPEN',
    };
    const suggestions = [];
    for (const message of hydrated) {
      const draft = draftSummaryFromPayload(message.structuredPayload);
      const pendingStatus = pendingStatusByType[message.messageType];
      if (!draft || !pendingStatus || draft.status !== pendingStatus) continue;
      const session = sessionByMessageId.get(message.id)!;
      suggestions.push({
        messageId: message.id,
        sessionId: session.id,
        sessionTitle: session.title,
        messageType: message.messageType,
        content: message.content,
        draftId: draft.id,
        status: draft.status,
        createdAt: message.createdAt,
      });
      if (suggestions.length >= 12) break;
    }
    return suggestions;
  }

  private async latestPreview(db: DbExecutor, sessionId: string) {
    const latest = (
      await db
        .select({ content: chatMessages.content })
        .from(chatMessages)
        .where(eq(chatMessages.sessionId, sessionId))
        .orderBy(desc(chatMessages.createdAt), desc(chatMessages.id))
        .limit(1)
    )[0];
    return latest?.content.slice(0, 120) ?? '';
  }

  private async latestSessionMessageIn(db: DbExecutor, sessionId: string) {
    return (
      await db
        .select()
        .from(chatMessages)
        .where(eq(chatMessages.sessionId, sessionId))
        .orderBy(desc(chatMessages.createdAt), desc(chatMessages.id))
        .limit(1)
    )[0];
  }

  private async requireSessionMessageIn(
    db: DbExecutor,
    sessionId: string,
    messageId: string,
  ) {
    const message = (
      await db
        .select()
        .from(chatMessages)
        .where(
          and(eq(chatMessages.id, messageId), eq(chatMessages.sessionId, sessionId)),
        )
        .limit(1)
    )[0];
    if (!message) throw new NotFoundException({ code: 'NOT_FOUND' });
    return message;
  }

  private async sessionCounters(
    db: DbExecutor,
    session: typeof chatSessions.$inferSelect,
  ) {
    const readState = (
      await db
        .select()
        .from(chatReadStates)
        .where(
          and(
            eq(chatReadStates.userId, session.userId),
            eq(chatReadStates.sessionId, session.id),
          ),
        )
        .limit(1)
    )[0];
    let unreadCount = 0;
    if (readState?.lastReadMessageId) {
      const readMessage = await this.requireSessionMessageIn(
        db,
        session.id,
        readState.lastReadMessageId,
      );
      unreadCount = await this.unreadCountAfter(db, session.id, readMessage.createdAt);
    } else {
      unreadCount = await this.unreadCountAfter(db, session.id, null);
    }
    return {
      unreadCount,
      pendingActionCount: await this.pendingActionCount(db, session.id),
      lastReadMessageId: readState?.lastReadMessageId ?? null,
      readAt: readState?.readAt ?? null,
    };
  }

  private async unreadCountAfter(
    db: DbExecutor,
    sessionId: string,
    afterCreatedAt: Date | null,
  ) {
    const row = (
      await db
        .select({ value: sql<number>`count(*)::int` })
        .from(chatMessages)
        .where(
          and(
            eq(chatMessages.sessionId, sessionId),
            ne(chatMessages.senderType, 'USER'),
            ...(afterCreatedAt ? [gt(chatMessages.createdAt, afterCreatedAt)] : []),
          ),
        )
    )[0];
    return Number(row?.value ?? 0);
  }

  private async pendingActionCount(db: DbExecutor, sessionId: string) {
    const commandRow = (
      await db
        .select({ value: sql<number>`count(*)::int` })
        .from(commandDrafts)
        .where(
          and(
            eq(commandDrafts.sessionId, sessionId),
            eq(commandDrafts.status, 'DRAFT'),
          ),
        )
    )[0];
    const ruleRow = (
      await db
        .select({ value: sql<number>`count(*)::int` })
        .from(ruleDrafts)
        .where(
          and(eq(ruleDrafts.sessionId, sessionId), eq(ruleDrafts.status, 'DRAFT')),
        )
    )[0];
    return Number(commandRow?.value ?? 0) + Number(ruleRow?.value ?? 0);
  }

  private publicSession(
    session: typeof chatSessions.$inferSelect,
    messagePreview: string,
    counters: {
      unreadCount: number;
      pendingActionCount: number;
      lastReadMessageId: string | null;
      readAt: Date | null;
    } = {
      unreadCount: 0,
      pendingActionCount: 0,
      lastReadMessageId: null,
      readAt: null,
    },
  ) {
    return {
      id: session.id,
      roomId: session.roomId,
      title: session.title,
      summary: session.summary,
      status: session.status,
      createdAt: session.createdAt,
      updatedAt: session.updatedAt,
      deletedAt: session.deletedAt,
      messagePreview,
      unreadCount: counters.unreadCount,
      pendingActionCount: counters.pendingActionCount,
      lastReadMessageId: counters.lastReadMessageId,
      readAt: counters.readAt,
    };
  }

  private publicMessage(message: typeof chatMessages.$inferSelect) {
    return {
      id: message.id,
      roomId: message.roomId,
      sessionId: message.sessionId,
      senderType: message.senderType,
      messageType: message.messageType,
      content: message.content,
      structuredPayload: message.structuredPayload,
      commandId: message.commandId,
      createdAt: message.createdAt,
    };
  }

  private publicDraft(draft: typeof commandDrafts.$inferSelect) {
    return {
      id: draft.id,
      intent: draft.intent,
      confirmationSummary: draft.confirmationSummary,
      status: draft.status,
      expiresAt: draft.expiresAt,
      commandId: draft.commandId,
      fileBrowseRequestId: draft.fileBrowseRequestId,
      fileTransferId: draft.fileTransferId,
    };
  }

  private publicRuleDraft(draft: typeof ruleDrafts.$inferSelect) {
    return {
      id: draft.id,
      roomId: draft.roomId,
      name: draft.name,
      definition: draft.definition,
      explanation: draft.explanation,
      ambiguities: Array.isArray(draft.ambiguities) ? draft.ambiguities : [],
      status: draft.status,
      expiresAt: draft.expiresAt,
      ruleId: draft.ruleId,
    };
  }

  private publicCommand(command: typeof commands.$inferSelect) {
    const { createdByUserId: _, idempotencyKey: __, ...safe } = command;
    return safe;
  }

  private async cachedBrowseEntriesIn(
    tx: Transaction,
    roomId: string,
    browse: z.infer<typeof createFileBrowseRequestSchema>,
  ) {
    const directory = normalizeQueryDirectory(browse.relativeDirectory);
    const query = browse.query?.trim().toLowerCase() ?? null;
    const extensions = new Set(
      browse.extensions.map((extension) => extension.toLowerCase()),
    );
    const limit = Math.min(Math.max(browse.limit, 1), 200);
    const rows = await tx
      .select()
      .from(cachedFiles)
      .where(
        and(
          eq(cachedFiles.roomId, roomId),
          eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
          ne(cachedFiles.freshnessStatus, 'STALE'),
        ),
      )
      .orderBy(desc(cachedFiles.usageScore), desc(cachedFiles.cachedAt))
      .limit(400);
    const entries = [];
    for (const file of rows) {
      const path = file.sourceRelativePath;
      if (!isPathInQueryScope(path, directory, browse.searchScope)) continue;
      if (query && !path.toLowerCase().includes(query)) continue;
      if (extensions.size > 0 && !extensions.has(fileExtension(path))) {
        continue;
      }
      entries.push({
        id: file.id,
        relativePath: path,
        sizeBytes: file.sizeBytes,
        freshnessStatus: file.freshnessStatus,
        cachedAt: file.cachedAt,
        lastVerifiedAt: file.lastVerifiedAt,
      });
      if (entries.length >= limit) break;
    }
    return entries;
  }

  private publicFileBrowseRequest(
    request: typeof fileBrowseRequests.$inferSelect,
  ) {
    return request;
  }

  private publicFileTransfer(transfer: typeof fileTransfers.$inferSelect) {
    const {
      objectKey: _,
      idempotencyKey: __,
      uploadCompletionIdempotencyKey: ___,
      requestedByUserId: ____,
      ...safe
    } = transfer;
    void _;
    void __;
    void ___;
    void ____;
    return safe;
  }
}

function titleFromContent(content: string) {
  const title = content.trim().replace(/\s+/g, ' ').slice(0, 120);
  return title.length > 0 ? title : DEFAULT_CHAT_TITLE;
}

function queryResultContent(
  summary: string,
  cacheHitCount: number,
  liveStatus: string | null,
) {
  const trimmed = summary.trim();
  const prefix = trimmed.length > 0 ? `${trimmed}\n` : '';
  if (liveStatus === 'REQUESTED') {
    return `${prefix}캐시에서 ${cacheHitCount}개를 찾았고, PC에 최신 탐색도 요청했어요. 결과가 도착하면 이어서 보여줄게요.`;
  }
  if (liveStatus === 'FAILED') {
    return `${prefix}지금 PC 탐색은 사용할 수 없어서 캐시 기준으로 ${cacheHitCount}개를 찾았어요.`;
  }
  return `${prefix}캐시 기준으로 ${cacheHitCount}개를 찾았어요.`;
}

function normalizeQueryDirectory(value: string) {
  return value.trim().replace(/^\/+|\/+$/g, '');
}

function isPathInQueryScope(
  path: string,
  directory: string,
  scope: 'CURRENT_DIRECTORY' | 'MANAGED_ROOT',
) {
  if (directory === '') return true;
  if (path === directory) return true;
  if (!path.startsWith(`${directory}/`)) return false;
  if (scope === 'MANAGED_ROOT') return true;
  const rest = path.slice(directory.length + 1);
  return !rest.includes('/');
}

function fileExtension(path: string) {
  const name = path.split('/').pop() ?? path;
  const index = name.lastIndexOf('.');
  return index >= 0 ? name.slice(index).toLowerCase() : '';
}

function draftSummaryFromPayload(
  value: unknown,
): { id: string; status: string } | null {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    return null;
  }
  const candidate = value as Record<string, unknown>;
  return typeof candidate.id === 'string' && typeof candidate.status === 'string'
    ? { id: candidate.id, status: candidate.status }
    : null;
}
