import { randomUUID } from 'node:crypto';
import {
  auditEvents,
  connectionMutationReceipts,
  createDatabase,
  devices,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq, inArray } from 'drizzle-orm';
import Redis from 'ioredis';
import { SyncService } from '../sync/sync.service';
import {
  agentConnectionActor,
  ConnectionLifecycleService,
} from './connection-lifecycle.service';

jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

const databaseUrl = process.env.DATABASE_URL;
const redisUrl = process.env.REDIS_URL;
const describeDatabase =
  databaseUrl && redisUrl && process.env.MOUSEKEEPER_RUN_DB_TESTS === 'true'
    ? describe
    : describe.skip;

describeDatabase('ConnectionLifecycleService integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let redis: Redis;
  let userId: string;
  let deviceId: string;
  let roomIds: string[] = [];
  let transferId: string;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    redis = new Redis(redisUrl!, { lazyConnect: true });
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `connection-lifecycle-${randomUUID()}`,
          displayName: 'Connection Lifecycle Integration User',
        })
        .returning()
    )[0];
    userId = user.id;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId,
          platform: 'WINDOWS',
          deviceName: 'Connection Lifecycle PC',
        })
        .returning()
    )[0];
    deviceId = device.id;
    roomIds = (
      await connection.db
        .insert(rooms)
        .values([
          {
            userId,
            desktopDeviceId: deviceId,
            name: 'Documents',
            rootAlias: 'Documents',
          },
          {
            userId,
            desktopDeviceId: deviceId,
            name: 'Downloads',
            rootAlias: 'Downloads',
          },
        ])
        .returning({ id: rooms.id })
    ).map(({ id }) => id);
    transferId = (
      await connection.db
        .insert(fileTransfers)
        .values({
          roomId: roomIds[0],
          desktopDeviceId: deviceId,
          requestedByUserId: userId,
          sourceRelativePath: 'report.pdf',
          status: 'READY',
          objectKey: `transfers/${userId}/${randomUUID()}`,
          sizeBytes: 12,
          sha256: 'a'.repeat(64),
          expiresAt: new Date(Date.now() + 60_000),
          idempotencyKey: randomUUID(),
        })
        .returning({ id: fileTransfers.id })
    )[0].id;
    if (redis.status === 'wait') await redis.connect();
    await redis.set(`presence:${deviceId}`, 'ONLINE_IDLE', 'EX', 30);
    await redis.zadd('presence:known', Date.now() + 30_000, deviceId);
  });

  it('revokes all rooms once and replays without duplicate events or jobs', async () => {
    const realtime = { publishNow: jest.fn().mockResolvedValue(undefined) };
    const gateway = { disconnectDevice: jest.fn() };
    const service = new ConnectionLifecycleService(
      connection.db,
      redis,
      new SyncService(),
      realtime as never,
      gateway as never,
    );
    const actor = agentConnectionActor(userId, deviceId);
    const key = randomUUID();

    // Two transport retries may arrive before the first response is returned.
    // The receipt advisory lock must make both calls converge without a
    // transient deadlock, 500, or a post-revoke 404.
    const [first, replay] = await Promise.all([
      service.revokeDevice(actor, deviceId, key),
      service.revokeDevice(actor, deviceId, key),
    ]);

    expect(first).toEqual(replay);
    expect(first.status).toBe('REVOKED');
    expect(
      (
        await connection.db
          .select({ status: devices.status })
          .from(devices)
          .where(eq(devices.id, deviceId))
      )[0].status,
    ).toBe('REVOKED');
    expect(
      await connection.db
        .select({ id: rooms.id })
        .from(rooms)
        .where(inArray(rooms.id, roomIds)),
    ).toHaveLength(2);
    expect(
      (
        await connection.db
          .select({ status: rooms.status })
          .from(rooms)
          .where(inArray(rooms.id, roomIds))
      ).every(({ status }) => status === 'REMOVED'),
    ).toBe(true);

    const events = await connection.db
      .select()
      .from(syncEvents)
      .where(eq(syncEvents.userId, userId));
    const roomEvents = events.filter(
      (event) => event.eventType === 'room.removed',
    );
    const deviceEvents = events.filter(
      (event) => event.eventType === 'device.revoked',
    );
    expect(roomEvents).toHaveLength(2);
    expect(new Set(roomEvents.map((event) => event.aggregateId))).toEqual(
      new Set(roomIds),
    );
    expect(
      roomEvents.every(
        (event) => (event.payload as { status?: string }).status === 'REMOVED',
      ),
    ).toBe(true);
    expect(deviceEvents).toHaveLength(1);
    expect(deviceEvents[0].payload).toMatchObject({
      deviceId,
      status: 'REVOKED',
    });
    expect(
      await connection.db
        .select()
        .from(objectDeletionJobs)
        .where(eq(objectDeletionJobs.transferId, transferId)),
    ).toHaveLength(1);
    expect(
      await connection.db
        .select()
        .from(connectionMutationReceipts)
        .where(eq(connectionMutationReceipts.userId, userId)),
    ).toHaveLength(1);
    expect(realtime.publishNow).toHaveBeenCalledWith(
      expect.arrayContaining(events.map((event) => event.id)),
    );
    expect(realtime.publishNow).toHaveBeenCalledWith([]);
    expect(gateway.disconnectDevice).toHaveBeenCalledTimes(2);
    expect(await redis.exists(`presence:${deviceId}`)).toBe(0);
  });

  afterAll(async () => {
    await redis.del(`presence:${deviceId}`);
    await redis.zrem('presence:known', deviceId);
    await connection.db
      .delete(objectDeletionJobs)
      .where(eq(objectDeletionJobs.transferId, transferId));
    await connection.db
      .delete(fileTransfers)
      .where(eq(fileTransfers.id, transferId));
    await connection.db
      .delete(connectionMutationReceipts)
      .where(eq(connectionMutationReceipts.userId, userId));
    await connection.db
      .delete(auditEvents)
      .where(eq(auditEvents.userId, userId));
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
    await connection.db.delete(rooms).where(inArray(rooms.id, roomIds));
    await connection.db.delete(devices).where(eq(devices.id, deviceId));
    await connection.db.delete(users).where(eq(users.id, userId));
    await redis.quit();
    await connection.close();
  });
});
