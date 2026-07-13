import { createHash, randomUUID } from 'node:crypto';
import {
  createDatabase,
  devices,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  syncEvents,
  users,
} from '@housemouse/database';
import { eq, inArray } from 'drizzle-orm';
import Redis from 'ioredis';
import { SyncService } from '../sync/sync.service';
import { ObjectStorageService } from './object-storage.service';
import { TransfersService } from './transfers.service';

const databaseUrl = process.env.DATABASE_URL;
const redisUrl = process.env.REDIS_URL;
const hasObjectStorage = Boolean(
  process.env.OBJECT_STORAGE_REGION && process.env.OBJECT_STORAGE_BUCKET,
);
const describeObjectStorage =
  databaseUrl &&
  redisUrl &&
  hasObjectStorage &&
  process.env.HOUSEMOUSE_RUN_OBJECT_STORAGE_E2E === 'true'
    ? describe
    : describe.skip;

describeObjectStorage('FileTransfer production object lifecycle', () => {
  jest.setTimeout(90_000);

  let connection: ReturnType<typeof createDatabase>;
  let redis: Redis;
  let storage: ObjectStorageService;
  let service: TransfersService;
  let userId: string;
  let deviceId: string;
  let roomId: string;
  let objectKey: string | undefined;

  beforeAll(async () => {
    connection = createDatabase(databaseUrl!);
    redis = new Redis(redisUrl!);
    storage = new ObjectStorageService();
    service = new TransfersService(
      connection.db,
      redis,
      new SyncService(),
      storage,
    );
    const user = (
      await connection.db
        .insert(users)
        .values({
          authProviderUid: `transfer-object-e2e-${randomUUID()}`,
          displayName: 'Transfer Object E2E',
        })
        .returning()
    )[0]!;
    const device = (
      await connection.db
        .insert(devices)
        .values({
          userId: user.id,
          platform: 'WINDOWS',
          deviceName: 'Transfer Object E2E',
        })
        .returning()
    )[0]!;
    const room = (
      await connection.db
        .insert(rooms)
        .values({
          userId: user.id,
          desktopDeviceId: device.id,
          name: 'Transfer Object E2E',
          rootAlias: 'E2E',
        })
        .returning()
    )[0]!;
    userId = user.id;
    deviceId = device.id;
    roomId = room.id;
    await redis.set(`presence:${deviceId}`, 'ONLINE_IDLE', 'EX', 90);
  });

  it('uploads, verifies, downloads, acknowledges, and deletes the object', async () => {
    const content = Buffer.from(
      `housemouse-transfer-object-e2e:${randomUUID()}`,
      'utf8',
    );
    const sha256 = createHash('sha256').update(content).digest('hex');
    const created = await service.create(userId, roomId, randomUUID(), {
      sourceRelativePath: 'e2e/transfer-object.txt',
    });
    objectKey = `transfers/${userId}/${created.id}`;

    const upload = await service.uploadTarget(userId, deviceId, created.id, {
      sourceVersion: {
        fileId: randomUUID(),
        sizeBytes: content.length,
        modifiedAt: new Date().toISOString(),
      },
    });
    const uploadResponse = await fetch(upload.uploadUrl, {
      method: 'PUT',
      body: content,
    });
    expect(uploadResponse.status).toBe(200);

    const ready = await service.complete(
      userId,
      deviceId,
      created.id,
      randomUUID(),
      { sizeBytes: content.length, sha256 },
    );
    expect(ready).toMatchObject({ status: 'READY', sizeBytes: content.length });

    const target = await service.download(userId, created.id);
    const downloadResponse = await fetch(target.downloadUrl);
    expect(downloadResponse.status).toBe(200);
    const downloaded = Buffer.from(await downloadResponse.arrayBuffer());
    expect(downloaded).toEqual(content);
    expect(createHash('sha256').update(downloaded).digest('hex')).toBe(sha256);

    const completed = await service.ack(userId, created.id);
    expect(completed.status).toBe('COMPLETED');

    let deletionCompleted = false;
    for (let attempt = 0; attempt < 20; attempt++) {
      const job = (
        await connection.db
          .select()
          .from(objectDeletionJobs)
          .where(eq(objectDeletionJobs.transferId, created.id))
          .limit(1)
      )[0];
      if (job?.status === 'COMPLETED') {
        deletionCompleted = true;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 2_000));
    }
    expect(deletionCompleted).toBe(true);
    expect((await fetch(target.downloadUrl)).status).toBe(404);
  });

  afterAll(async () => {
    if (objectKey) await storage.delete(objectKey).catch(() => undefined);
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
