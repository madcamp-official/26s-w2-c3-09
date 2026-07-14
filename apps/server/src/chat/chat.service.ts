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
  uploadCommandPayloadSchema,
} from '@mousekeeper/contracts';
import {
  chatMessages,
  chatReadStates,
  chatSessions,
  commandDrafts,
  commands,
  devices,
  fileBrowseRequests,
  fileTransfers,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, asc, desc, eq, gt, isNull, ne, or, sql } from 'drizzle-orm';
import { z } from 'zod';
import { mapAiResultToCommandDraft } from '../ai/ai-command-draft.mapper';
import { AI_PROVIDER, type AiProvider } from '../ai/ai.provider';
import { canonicalJson } from '../common/canonical-json';
import { DATABASE } from '../database/database.module';
import { FileBrowseService } from '../file-access/file-browse.service';
import { SyncService } from '../sync/sync.service';
import { TransfersService } from '../transfers/transfers.service';

const CHAT_SESSION_LIMIT = 5;
const DEFAULT_CHAT_TITLE = 'New chat';
const DEFAULT_DRAFT_TTL_MS = 10 * 60 * 1000;
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
    return rows.map((message) => this.publicMessage(message));
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
    return rows.map(({ message }) => this.publicMessage(message));
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
    const row = (
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
    return Number(row?.value ?? 0);
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

  private publicCommand(command: typeof commands.$inferSelect) {
    const { createdByUserId: _, idempotencyKey: __, ...safe } = command;
    return safe;
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
