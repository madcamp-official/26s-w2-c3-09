import { randomUUID } from 'node:crypto';
import {
  auditEvents,
  cacheCandidateBatches,
  cacheDeletionJobs,
  cacheReservationDeletionJobs,
  cachedFiles,
  cacheUploadReservations,
  createDatabase,
  devices,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  smartCachePolicies,
  syncEvents,
  users,
} from '@housemouse/database';
import { eq, inArray } from 'drizzle-orm';
import Redis from 'ioredis';
import type { AuthPrincipal } from '../auth/auth-principal';
import { RoomsController } from '../rooms/rooms.controller';
import { SyncService } from '../sync/sync.service';
import { ObjectStorageService } from '../transfers/object-storage.service';
import { SmartCacheService } from './smart-cache.service';

jest.mock('../auth/firebase-auth.guard', () => ({
  FirebaseAuthGuard: class FirebaseAuthGuard {},
}));

const databaseUrl = process.env.DATABASE_URL;
const redisUrl = process.env.REDIS_URL;
const hasStorageConfig = [
  'OBJECT_STORAGE_ENDPOINT',
  'OBJECT_STORAGE_REGION',
  'OBJECT_STORAGE_BUCKET',
  'OBJECT_STORAGE_ACCESS_KEY_ID',
  'OBJECT_STORAGE_SECRET_ACCESS_KEY',
].every((key) => Boolean(process.env[key]));
const describeDatabase =
  databaseUrl &&
  redisUrl &&
  hasStorageConfig &&
  process.env.HOUSEMOUSE_RUN_DB_TESTS === 'true' &&
  process.env.SMART_CACHE_ENABLED === 'true'
    ? describe
    : describe.skip;

describeDatabase('SmartCacheService PostgreSQL quota integration', () => {
  let connection: ReturnType<typeof createDatabase>;
  let redis: Redis;
  let service: SmartCacheService;
  let userId: string;
  let deviceId: string;
  let roomId: string;

  const candidate = (path: string, hash: string, sizeBytes: number) => ({
    roomId,
    candidates: [
      {
        sourceRelativePath: path,
        sourceVersion: { revision: hash.slice(0, 8) },
        sourceVersionHash: hash,
        sizeBytes,
        usageScore: 100,
        manualPin: false,
      },
    ],
  });

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    redis = new Redis(redisUrl!, { lazyConnect: true });
    service = new SmartCacheService(
      connection.db,
      redis,
      new ObjectStorageService(),
      new SyncService(),
    );
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `smart-cache-${randomUUID()}`,
          displayName: 'Smart Cache Integration User',
        })
        .returning()
    )[0]!;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Smart Cache Integration PC',
        })
        .returning()
    )[0]!;
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Smart Cache Integration Room',
          rootAlias: 'Documents',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = device.id;
    roomId = room.id;
    await service.updatePolicy(userId, roomId, {
      enabled: true,
      quotaBytes: 100,
      maxFileBytes: 100,
      excludedPatterns: ['excluded/**'],
    });
  });

  it('serializes quota, replays batches, releases expiry, and writes tombstones', async () => {
    const firstKey = randomUUID();
    const secondKey = randomUUID();
    const firstBody = candidate('first.pdf', 'a'.repeat(64), 60);
    const secondBody = candidate('second.pdf', 'b'.repeat(64), 60);
    const [first, second] = await Promise.all([
      service.submit(userId, deviceId, firstKey, firstBody),
      service.submit(userId, deviceId, secondKey, secondBody),
    ]);

    expect(first.approved.length + second.approved.length).toBe(1);
    expect(first.rejectedCount + second.rejectedCount).toBe(1);

    const accepted = first.approved.length ? first : second;
    const acceptedKey = first.approved.length ? firstKey : secondKey;
    const acceptedBody = first.approved.length ? firstBody : secondBody;
    const replay = await service.submit(
      userId,
      deviceId,
      acceptedKey,
      acceptedBody,
    );
    expect(replay.batchId).toBe(accepted.batchId);
    expect(replay.approved[0]!.reservationId).toBe(
      accepted.approved[0]!.reservationId,
    );
    await expect(
      service.submit(
        userId,
        deviceId,
        acceptedKey,
        candidate('changed.pdf', 'c'.repeat(64), 60),
      ),
    ).rejects.toMatchObject({ response: { code: 'IDEMPOTENCY_CONFLICT' } });

    await connection.db
      .update(cacheUploadReservations)
      .set({ expiresAt: new Date(Date.now() - 1_000) })
      .where(
        eq(cacheUploadReservations.id, accepted.approved[0]!.reservationId),
      );
    const replacement = await service.submit(
      userId,
      deviceId,
      randomUUID(),
      candidate('replacement.pdf', 'd'.repeat(64), 100),
    );
    expect(replacement.approved).toHaveLength(1);
    await service.cancelReservation(
      userId,
      deviceId,
      replacement.approved[0]!.reservationId,
    );
    expect(
      await connection.db
        .select()
        .from(cacheReservationDeletionJobs)
        .where(
          eq(
            cacheReservationDeletionJobs.reservationId,
            replacement.approved[0]!.reservationId,
          ),
        ),
    ).toHaveLength(1);

    const lowScore = (
      await connection.db
        .insert(cachedFiles)
        .values({
          roomId,
          sourceRelativePath: 'low-score.pdf',
          sourceVersion: { revision: 'low' },
          sourceVersionHash: 'e'.repeat(64),
          usageScore: 1,
          objectKey: `cache/${userId}/${roomId}/low-score`,
          sizeBytes: 60,
          lastVerifiedAt: new Date(),
        })
        .returning()
    )[0]!;
    const higherScore = await service.submit(
      userId,
      deviceId,
      randomUUID(),
      candidate('higher-score.pdf', 'f'.repeat(64), 60),
    );
    expect(higherScore.approved).toHaveLength(1);
    expect(
      (
        await connection.db
          .select()
          .from(cachedFiles)
          .where(eq(cachedFiles.id, lowScore.id))
      )[0]!.availabilityStatus,
    ).toBe('INVALIDATED');
    expect(
      await connection.db
        .select()
        .from(cacheDeletionJobs)
        .where(eq(cacheDeletionJobs.cachedFileId, lowScore.id)),
    ).toHaveLength(1);

    const disableTarget = (
      await connection.db
        .insert(cachedFiles)
        .values({
          roomId,
          sourceRelativePath: 'disable-target.pdf',
          sourceVersion: { revision: 'disable' },
          sourceVersionHash: '1'.repeat(64),
          usageScore: 200,
          manualPin: true,
          objectKey: `cache/${userId}/${roomId}/disable-target`,
          sizeBytes: 20,
          lastVerifiedAt: new Date(),
        })
        .returning()
    )[0]!;
    await expect(
      service.updatePolicy(userId, roomId, {
        enabled: true,
        quotaBytes: 50,
        maxFileBytes: 50,
        excludedPatterns: [],
      }),
    ).rejects.toMatchObject({ response: { code: 'REJECTED_POLICY' } });
    await service.updatePolicy(userId, roomId, {
      enabled: false,
      quotaBytes: 100,
      maxFileBytes: 100,
      excludedPatterns: [],
    });
    expect(
      (
        await connection.db
          .select()
          .from(cachedFiles)
          .where(eq(cachedFiles.id, disableTarget.id))
      )[0]!.availabilityStatus,
    ).toBe('INVALIDATED');
    expect(
      await connection.db
        .select()
        .from(cacheDeletionJobs)
        .where(eq(cacheDeletionJobs.cachedFileId, disableTarget.id)),
    ).toHaveLength(1);
    expect(
      (
        await connection.db
          .select()
          .from(cacheUploadReservations)
          .where(
            eq(
              cacheUploadReservations.id,
              higherScore.approved[0]!.reservationId,
            ),
          )
      )[0]!.status,
    ).toBe('CANCELLED');
    expect(
      await connection.db
        .select()
        .from(cacheReservationDeletionJobs)
        .where(
          eq(
            cacheReservationDeletionJobs.reservationId,
            higherScore.approved[0]!.reservationId,
          ),
        ),
    ).toHaveLength(1);

    await service.updatePolicy(userId, roomId, {
      enabled: true,
      quotaBytes: 100,
      maxFileBytes: 100,
      excludedPatterns: ['excluded/**'],
    });
    const removeTarget = (
      await connection.db
        .insert(cachedFiles)
        .values({
          roomId,
          sourceRelativePath: 'room-remove-target.pdf',
          sourceVersion: { revision: 'room-remove' },
          sourceVersionHash: '2'.repeat(64),
          usageScore: 300,
          manualPin: true,
          objectKey: `cache/${userId}/${roomId}/room-remove-target`,
          sizeBytes: 10,
          lastVerifiedAt: new Date(),
        })
        .returning()
    )[0]!;
    const excluded = await service.submit(
      userId,
      deviceId,
      randomUUID(),
      candidate('excluded/private.pdf', '5'.repeat(64), 20),
    );
    expect(excluded.approved).toHaveLength(0);
    expect(excluded.rejectedCount).toBe(1);
    const roomRemovalReservation = await service.submit(
      userId,
      deviceId,
      randomUUID(),
      candidate('room-remove-reservation.pdf', '4'.repeat(64), 20),
    );
    expect(roomRemovalReservation.approved).toHaveLength(1);
    const transfer = (
      await connection.db
        .insert(fileTransfers)
        .values({
          roomId,
          desktopDeviceId: deviceId,
          requestedByUserId: userId,
          sourceRelativePath: 'transfer.pdf',
          status: 'READY',
          objectKey: `transfers/${userId}/${randomUUID()}`,
          sizeBytes: 10,
          sha256: '3'.repeat(64),
          idempotencyKey: randomUUID(),
          expiresAt: new Date(Date.now() + 60_000),
        })
        .returning()
    )[0]!;
    const principal: AuthPrincipal = {
      userId,
      deviceId,
      authProviderUid: 'integration-device',
      displayName: 'Integration Device',
      authType: 'DEVICE',
    };
    const removed = await new RoomsController(
      connection.db,
      new SyncService(),
    ).remove(principal, roomId);
    expect(removed.status).toBe('REMOVED');
    expect(
      (
        await connection.db
          .select()
          .from(cachedFiles)
          .where(eq(cachedFiles.id, removeTarget.id))
      )[0]!.availabilityStatus,
    ).toBe('INVALIDATED');
    expect(
      await connection.db
        .select()
        .from(cacheDeletionJobs)
        .where(eq(cacheDeletionJobs.cachedFileId, removeTarget.id)),
    ).toHaveLength(1);
    expect(
      (
        await connection.db
          .select()
          .from(cacheUploadReservations)
          .where(
            eq(
              cacheUploadReservations.id,
              roomRemovalReservation.approved[0]!.reservationId,
            ),
          )
      )[0]!.status,
    ).toBe('CANCELLED');
    expect(
      await connection.db
        .select()
        .from(cacheReservationDeletionJobs)
        .where(
          eq(
            cacheReservationDeletionJobs.reservationId,
            roomRemovalReservation.approved[0]!.reservationId,
          ),
        ),
    ).toHaveLength(1);
    expect(
      (
        await connection.db
          .select()
          .from(fileTransfers)
          .where(eq(fileTransfers.id, transfer.id))
      )[0]!.status,
    ).toBe('CANCELLED');
    expect(
      await connection.db
        .select()
        .from(objectDeletionJobs)
        .where(eq(objectDeletionJobs.transferId, transfer.id)),
    ).toHaveLength(1);
    expect(
      (
        await connection.db
          .select()
          .from(syncEvents)
          .where(eq(syncEvents.userId, userId))
      ).filter((event) => event.eventType === 'smart-cache.updated').length,
    ).toBeGreaterThanOrEqual(5);
  });

  afterAll(async () => {
    const files = await connection.db
      .select({ id: cachedFiles.id })
      .from(cachedFiles)
      .where(eq(cachedFiles.roomId, roomId));
    const reservations = await connection.db
      .select({ id: cacheUploadReservations.id })
      .from(cacheUploadReservations)
      .where(eq(cacheUploadReservations.roomId, roomId));
    if (files.length) {
      await connection.db.delete(cacheDeletionJobs).where(
        inArray(
          cacheDeletionJobs.cachedFileId,
          files.map((file) => file.id),
        ),
      );
    }
    if (reservations.length) {
      await connection.db.delete(cacheReservationDeletionJobs).where(
        inArray(
          cacheReservationDeletionJobs.reservationId,
          reservations.map((reservation) => reservation.id),
        ),
      );
    }
    await connection.db
      .delete(auditEvents)
      .where(eq(auditEvents.userId, userId));
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
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
    await connection.db
      .delete(fileTransfers)
      .where(eq(fileTransfers.roomId, roomId));
    await connection.db
      .delete(cachedFiles)
      .where(eq(cachedFiles.roomId, roomId));
    await connection.db
      .delete(cacheUploadReservations)
      .where(eq(cacheUploadReservations.roomId, roomId));
    await connection.db
      .delete(cacheCandidateBatches)
      .where(eq(cacheCandidateBatches.roomId, roomId));
    await connection.db
      .delete(smartCachePolicies)
      .where(eq(smartCachePolicies.roomId, roomId));
    await connection.db.delete(rooms).where(eq(rooms.id, roomId));
    await connection.db.delete(devices).where(eq(devices.id, deviceId));
    await connection.db.delete(users).where(eq(users.id, userId));
    if (redis.status === 'wait') redis.disconnect();
    else if (redis.status !== 'end') await redis.quit();
    await connection.close();
  });
});
