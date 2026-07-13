import { randomUUID } from 'node:crypto';
import {
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
import { ObjectStorageService } from './object-storage.service';
import { TransfersService } from './transfers.service';

const databaseUrl = process.env.DATABASE_URL;
const redisUrl = process.env.REDIS_URL;
const hasStorageConfig = Boolean(
  process.env.OBJECT_STORAGE_REGION && process.env.OBJECT_STORAGE_BUCKET,
);
const describeDatabase =
  databaseUrl &&
  redisUrl &&
  hasStorageConfig &&
  process.env.MOUSEKEEPER_RUN_DB_TESTS === 'true'
    ? describe
    : describe.skip;

describeDatabase('TransfersService PostgreSQL and Valkey integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let redis: Redis;
  let service: TransfersService;
  let userId: string;
  let deviceId: string;
  let otherDeviceId: string;
  let roomId: string;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    redis = new Redis(redisUrl!);
    service = new TransfersService(
      connection.db,
      redis,
      new SyncService(),
      new ObjectStorageService(),
    );
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `transfer-${randomUUID()}`,
          displayName: 'Transfer Integration User',
        })
        .returning()
    )[0]!;
    const createdDevices = await connection.db
      .insert(devices)
      .values([
        { userId: user.id, platform: 'WINDOWS', deviceName: 'Transfer PC' },
        { userId: user.id, platform: 'WINDOWS', deviceName: 'Other PC' },
      ])
      .returning();
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: createdDevices[0]!.id,
          name: 'Transfer Room',
          rootAlias: 'Downloads',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = createdDevices[0]!.id;
    otherDeviceId = createdDevices[1]!.id;
    roomId = room.id;
    await redis.set(`presence:${deviceId}`, 'ONLINE_IDLE', 'EX', 45);
  });

  it('binds idempotency and signed targets to the exact file, device, and TTL', async () => {
    const key = randomUUID();
    const created = await service.create(userId, roomId, key, {
      sourceRelativePath: 'reports/current.pdf',
    });
    const replay = await service.create(userId, roomId, key, {
      sourceRelativePath: 'reports/current.pdf',
    });
    expect(replay.id).toBe(created.id);
    await expect(
      service.create(userId, roomId, key, {
        sourceRelativePath: 'reports/different.pdf',
      }),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });

    const sourceVersion = {
      fileId: 'stable-file-id',
      sizeBytes: 1024,
      modifiedAt: new Date().toISOString(),
    };
    await expect(
      service.uploadTarget(userId, otherDeviceId, created.id, {
        sourceVersion,
      }),
    ).rejects.toMatchObject({ response: { code: 'NOT_FOUND' } });
    const target = await service.uploadTarget(userId, deviceId, created.id, {
      sourceVersion,
    });
    const uploadExpiry = Number(
      new URL(target.uploadUrl).searchParams.get('X-Amz-Expires'),
    );
    expect(uploadExpiry).toBeGreaterThan(0);
    expect(uploadExpiry).toBeLessThanOrEqual(600);
    expect(target.expiresAt.getTime()).toBeLessThanOrEqual(
      Date.now() + 600_000,
    );
    expect((target as Record<string, unknown>)['objectKey']).toBeUndefined();

    const cancelled = await service.cancel(userId, deviceId, created.id);
    expect(cancelled.status).toBe('CANCELLED');
    expect(
      await connection.db
        .select()
        .from(objectDeletionJobs)
        .where(eq(objectDeletionJobs.transferId, created.id)),
    ).toHaveLength(1);

    const failedTransfer = await service.create(userId, roomId, randomUUID(), {
      sourceRelativePath: 'reports/changed.pdf',
    });
    await service.uploadTarget(userId, deviceId, failedTransfer.id, {
      sourceVersion,
    });
    await expect(
      service.fail(userId, otherDeviceId, failedTransfer.id, {
        failureCode: 'SOURCE_CHANGED',
      }),
    ).rejects.toMatchObject({ response: { code: 'NOT_FOUND' } });
    const failed = await service.fail(userId, deviceId, failedTransfer.id, {
      failureCode: 'SOURCE_CHANGED',
    });
    expect(failed).toMatchObject({
      status: 'FAILED',
      failureCode: 'SOURCE_CHANGED',
    });
    expect(
      (
        await service.fail(userId, deviceId, failedTransfer.id, {
          failureCode: 'SOURCE_CHANGED',
        })
      ).id,
    ).toBe(failedTransfer.id);
    await expect(
      service.fail(userId, deviceId, failedTransfer.id, {
        failureCode: 'SOURCE_NOT_FOUND',
      }),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });
    expect(
      await connection.db
        .select()
        .from(objectDeletionJobs)
        .where(eq(objectDeletionJobs.transferId, failedTransfer.id)),
    ).toHaveLength(1);

    const ready = (
      await connection.db
        .insert(fileTransfers)
        .values({
          roomId,
          desktopDeviceId: deviceId,
          requestedByUserId: userId,
          sourceRelativePath: 'reports/ready.pdf',
          status: 'READY',
          objectKey: `transfers/${userId}/${randomUUID()}`,
          sizeBytes: 2048,
          sha256: 'a'.repeat(64),
          idempotencyKey: randomUUID(),
          expiresAt: new Date(Date.now() + 25_000),
        })
        .returning()
    )[0]!;
    const download = await service.download(userId, ready.id);
    const downloadExpiry = Number(
      new URL(download.downloadUrl).searchParams.get('X-Amz-Expires'),
    );
    expect(downloadExpiry).toBeGreaterThan(0);
    expect(downloadExpiry).toBeLessThanOrEqual(25);
  });

  afterAll(async () => {
    await redis.del(`presence:${deviceId}`);
    const transfers = await connection.db
      .select({ id: fileTransfers.id })
      .from(fileTransfers)
      .where(eq(fileTransfers.roomId, roomId));
    if (transfers.length) {
      await connection.db.delete(objectDeletionJobs).where(
        inArray(
          objectDeletionJobs.transferId,
          transfers.map((transfer) => transfer.id),
        ),
      );
    }
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
    await connection.db
      .delete(fileTransfers)
      .where(eq(fileTransfers.roomId, roomId));
    await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    await connection.db.delete(devices).where(eq(devices.userId, userId));
    await connection.db.delete(users).where(eq(users.id, userId));
    await redis.quit();
    await connection.close();
  });
});
