import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  chatMessages,
  chatSessions,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, asc, desc, eq, gt, isNull, or } from 'drizzle-orm';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';

const CHAT_SESSION_LIMIT = 5;
const DEFAULT_CHAT_TITLE = 'New chat';
type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
type DbExecutor = Database | Transaction;

@Injectable()
export class ChatService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
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
    );
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
    return this.db.transaction(async (tx) => {
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
      return this.createMessageForSessionIn(tx, userId, session, content);
    });
  }

  private async createMessageForSession(
    userId: string,
    session: typeof chatSessions.$inferSelect,
    content: string,
  ) {
    return this.db.transaction((tx) =>
      this.createMessageForSessionIn(tx, userId, session, content),
    );
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
      assistant: null,
      aiStatus: 'UNCONFIGURED' as const,
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
    const row = (
      await this.db
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

  private async withPreviews(
    db: DbExecutor,
    sessions: (typeof chatSessions.$inferSelect)[],
  ) {
    const result = [];
    for (const session of sessions) {
      result.push(
        this.publicSession(session, await this.latestPreview(db, session.id)),
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

  private publicSession(
    session: typeof chatSessions.$inferSelect,
    messagePreview: string,
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
}

function titleFromContent(content: string) {
  const title = content.trim().replace(/\s+/g, ' ').slice(0, 120);
  return title.length > 0 ? title : DEFAULT_CHAT_TITLE;
}
