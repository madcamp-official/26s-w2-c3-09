import { randomUUID } from 'node:crypto';
import {
  chatMessages,
  chatSessions,
  createDatabase,
  devices,
  rooms,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq } from 'drizzle-orm';
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
    service = new ChatService(connection.db, new SyncService());
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

  afterEach(async () => {
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
    await connection.db
      .delete(chatMessages)
      .where(eq(chatMessages.roomId, roomId));
    await connection.db
      .delete(chatSessions)
      .where(eq(chatSessions.roomId, roomId));
    await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    await connection.db.delete(devices).where(eq(devices.id, deviceId));
    await connection.db.delete(users).where(eq(users.id, userId));
    await connection.close();
  });
});
