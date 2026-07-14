import {
  createCipheriv,
  createDecipheriv,
  createHash,
  randomBytes,
  randomUUID,
} from 'node:crypto';
import {
  cacheCandidateBatches,
  cacheDeletionJobs,
  cacheReservationDeletionJobs,
  cachedFiles,
  cacheUploadReservations,
  createDatabase,
  devices,
  rooms,
  smartCachePolicies,
  syncEvents,
  users,
} from '@mousekeeper/database';
import { eq, inArray } from 'drizzle-orm';
import Redis from 'ioredis';
import { SyncService } from '../sync/sync.service';
import { ObjectStorageService } from '../transfers/object-storage.service';
import { SmartCacheService } from './smart-cache.service';

const databaseUrl = process.env.DATABASE_URL;
const redisUrl = process.env.REDIS_URL;
const hasObjectStorage = Boolean(
  process.env.OBJECT_STORAGE_REGION && process.env.OBJECT_STORAGE_BUCKET,
);
const describeObjectStorage =
  databaseUrl &&
  redisUrl &&
  hasObjectStorage &&
  process.env.SMART_CACHE_ENABLED === 'true' &&
  process.env.MOUSEKEEPER_RUN_OBJECT_STORAGE_E2E === 'true'
    ? describe
    : describe.skip;

describeObjectStorage('Smart cache production object lifecycle', () => {
  jest.setTimeout(90_000);

  let connection: ReturnType<typeof createDatabase>;
  let redis: Redis;
  let storage: ObjectStorageService;
  let service: SmartCacheService;
  let userId: string;
  let deviceId: string;
  let roomId: string;
  let objectKey: string | undefined;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    redis = new Redis(redisUrl!, { lazyConnect: true });
    storage = new ObjectStorageService();
    service = new SmartCacheService(
      connection.db,
      redis,
      storage,
      new SyncService(),
    );
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `cache-object-e2e-${randomUUID()}`,
          displayName: 'Cache Object E2E',
        })
        .returning()
    )[0]!;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Cache Object E2E',
        })
        .returning()
    )[0]!;
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Cache Object E2E',
          rootAlias: 'E2E',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = device.id;
    roomId = room.id;
    await service.updatePolicy(userId, roomId, {
      enabled: true,
      quotaBytes: 1_048_576,
      maxFileBytes: 1_048_576,
      excludedPatterns: [],
    });
  });

  it('stores ciphertext, downloads it, and deletes it after opt-out', async () => {
    const plaintext = Buffer.from(
      `mousekeeper-smart-cache-object-e2e:${randomUUID()}`,
      'utf8',
    );
    const key = randomBytes(32);
    const iv = randomBytes(12);
    const cipher = createCipheriv('aes-256-gcm', key, iv);
    const encrypted = Buffer.concat([cipher.update(plaintext), cipher.final()]);
    const ciphertext = Buffer.concat([
      Buffer.from('MKS1'),
      iv,
      encrypted,
      cipher.getAuthTag(),
    ]);
    const sha256 = createHash('sha256').update(ciphertext).digest('hex');
    const plaintextSha256 = createHash('sha256').update(plaintext).digest('hex');
    const sourceVersionHash = createHash('sha256')
      .update(plaintext)
      .digest('hex');
    const encryptionMetadata = {
      algorithm: 'AES-256-GCM' as const,
      format: 'MKS1_NONCE_CIPHERTEXT_TAG' as const,
      keyId: `mks1-${randomBytes(16).toString('hex')}`,
      nonceHex: iv.toString('hex'),
      plaintextSizeBytes: plaintext.length,
      plaintextSha256,
    };

    const batch = await service.submit(userId, deviceId, randomUUID(), {
      roomId,
      candidates: [
        {
          sourceRelativePath: 'e2e/encrypted-cache.txt',
          sourceVersion: { revision: randomUUID() },
          sourceVersionHash,
          sizeBytes: ciphertext.length,
          usageScore: 100,
          manualPin: false,
        },
      ],
    });
    expect(batch.approved).toHaveLength(1);
    const reservation = batch.approved[0]!;
    objectKey = `cache/${userId}/${roomId}/${reservation.reservationId}`;
    const uploadResponse = await fetch(reservation.uploadUrl!, {
      method: 'PUT',
      body: ciphertext,
    });
    expect(uploadResponse.status).toBe(200);

    const cached = await service.complete(
      userId,
      deviceId,
      reservation.reservationId,
      randomUUID(),
      {
        sizeBytes: ciphertext.length,
        sha256,
        usageScore: 100,
        manualPin: false,
        encryptionMetadata,
      },
    );
    const serialized = JSON.stringify(cached);
    expect(serialized).not.toContain(key.toString('hex'));
    expect(serialized).not.toContain(plaintext.toString('utf8'));
    expect(cached.encryptionMetadata).toEqual(encryptionMetadata);

    const target = await service.download(userId, cached.id);
    expect(target.encryptionMetadata).toEqual(encryptionMetadata);
    const downloadResponse = await fetch(target.downloadUrl);
    expect(downloadResponse.status).toBe(200);
    const downloaded = Buffer.from(await downloadResponse.arrayBuffer());
    expect(createHash('sha256').update(downloaded).digest('hex')).toBe(sha256);
    expect(downloaded.subarray(0, 4).toString('utf8')).toBe('MKS1');
    const downloadedIv = downloaded.subarray(4, 16);
    const encryptedBody = downloaded.subarray(16, downloaded.length - 16);
    const tag = downloaded.subarray(downloaded.length - 16);
    const decipher = createDecipheriv('aes-256-gcm', key, downloadedIv);
    decipher.setAuthTag(tag);
    expect(
      Buffer.concat([decipher.update(encryptedBody), decipher.final()]),
    ).toEqual(plaintext);

    await service.updatePolicy(userId, roomId, {
      enabled: false,
      quotaBytes: 1_048_576,
      maxFileBytes: 1_048_576,
      excludedPatterns: [],
    });
    let deletionCompleted = false;
    for (let attempt = 0; attempt < 20; attempt++) {
      const job = (
        await connection.db
          .select()
          .from(cacheDeletionJobs)
          .where(eq(cacheDeletionJobs.cachedFileId, cached.id))
          .limit(1)
      )[0];
      if (job?.status === 'COMPLETED') {
        deletionCompleted = true;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 2_000));
    }
    expect(deletionCompleted).toBe(true);
    // Without ListBucket on the cache prefix, S3 intentionally hides a missing
    // key as 403. Both responses prove the previously readable object is gone.
    expect([403, 404]).toContain((await fetch(target.downloadUrl)).status);
  });

  afterAll(async () => {
    if (objectKey) await storage.delete(objectKey).catch(() => undefined);
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
    await connection.db.delete(syncEvents).where(eq(syncEvents.userId, userId));
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
