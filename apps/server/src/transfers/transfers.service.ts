import {
  ConflictException,
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  completeFileUploadSchema,
  createFileTransferSchema,
  failFileTransferSchema,
  requestUploadTargetSchema,
} from '@mousekeeper/contracts';
import {
  devices,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, asc, eq, gt } from 'drizzle-orm';
import { z } from 'zod';
import Redis from 'ioredis';
import { loadEnvironment } from '../config/environment';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';
import { SyncService } from '../sync/sync.service';
import { ObjectStorageService } from './object-storage.service';

type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];

@Injectable()
export class TransfersService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly sync: SyncService,
    private readonly storage: ObjectStorageService,
  ) {}
  async create(
    userId: string,
    roomId: string,
    key: string,
    body: z.infer<typeof createFileTransferSchema>,
  ) {
    return this.db.transaction((tx) =>
      this.createInTransaction(tx, userId, roomId, key, body),
    );
  }

  async createInTransaction(
    tx: Transaction,
    userId: string,
    roomId: string,
    key: string,
    body: z.infer<typeof createFileTransferSchema>,
  ) {
    this.storage.assertConfigured();
    if (this.redis.status === 'wait') await this.redis.connect();
    const ttl = loadEnvironment().FILE_TRANSFER_TTL_SECONDS;
    const candidate = (
      await tx
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
    if (!candidate) throw new NotFoundException({ code: 'NOT_FOUND' });
    const device = (
      await tx
        .select()
        .from(devices)
        .where(
          and(
            eq(devices.id, candidate.desktopDeviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });
    const room = (
      await tx
        .select()
        .from(rooms)
        .where(
          and(
            eq(rooms.id, roomId),
            eq(rooms.desktopDeviceId, device.id),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
    if (!(await this.redis.exists(`presence:${room.desktopDeviceId}`)))
      throw new ConflictException({ code: 'DEVICE_OFFLINE' });
    const inserted = (
      await tx
        .insert(fileTransfers)
        .values({
          roomId,
          desktopDeviceId: room.desktopDeviceId,
          requestedByUserId: userId,
          sourceRelativePath: body.sourceRelativePath,
          idempotencyKey: key,
          expiresAt: new Date(Date.now() + ttl * 1000),
        })
        .onConflictDoNothing()
        .returning()
    )[0];
    if (!inserted) {
      const existing = (
        await tx
          .select()
          .from(fileTransfers)
          .where(
            and(
              eq(fileTransfers.requestedByUserId, userId),
              eq(fileTransfers.idempotencyKey, key),
            ),
          )
          .limit(1)
      )[0];
      if (!existing) throw new ConflictException({ code: 'CONFLICT' });
      if (
        existing.roomId !== roomId ||
        existing.sourceRelativePath !== body.sourceRelativePath
      )
        throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
      return this.publicTransfer(existing);
    }
    const created = inserted;
    await this.sync.append(tx, {
      userId,
      deviceId: room.desktopDeviceId,
      roomId,
      eventType: 'file.transfer.requested',
      aggregateType: 'file_transfer',
      aggregateId: created.id,
      payload: { transferId: created.id },
    });
    return this.publicTransfer(created);
  }
  async pending(userId: string, deviceId: string) {
    return this.db.transaction(async (tx) => {
      const device = (
        await tx
          .select()
          .from(devices)
          .where(
            and(
              eq(devices.id, deviceId),
              eq(devices.userId, userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!device) throw new ForbiddenException({ code: 'FORBIDDEN' });
      const pending = await tx
        .select({ transfer: fileTransfers })
        .from(fileTransfers)
        .innerJoin(
          rooms,
          and(eq(fileTransfers.roomId, rooms.id), eq(rooms.status, 'ACTIVE')),
        )
        .where(
          and(
            eq(fileTransfers.desktopDeviceId, deviceId),
            eq(fileTransfers.status, 'REQUESTED'),
            gt(fileTransfers.expiresAt, new Date()),
          ),
        )
        .orderBy(asc(fileTransfers.createdAt));
      return pending.map(({ transfer }) => this.publicTransfer(transfer));
    });
  }
  async uploadTarget(
    userId: string,
    deviceId: string,
    transferId: string,
    body: z.infer<typeof requestUploadTargetSchema>,
  ) {
    if (
      body.sourceVersion.sizeBytes > loadEnvironment().FILE_TRANSFER_MAX_BYTES
    )
      throw new ConflictException({ code: 'SIZE_LIMIT_EXCEEDED' });
    const transfer = await this.owned(userId, transferId, deviceId);
    if (transfer.status !== 'REQUESTED' || transfer.expiresAt <= new Date())
      throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
    const objectKey = `transfers/${userId}/${transfer.id}`;
    const expiresIn = Math.max(
      1,
      Math.floor((transfer.expiresAt.getTime() - Date.now()) / 1000),
    );
    const uploadUrl = await this.storage.uploadUrl(objectKey, expiresIn);
    const updated = await this.db
      .update(fileTransfers)
      .set({
        status: 'UPLOADING',
        sourceVersion: body.sourceVersion,
        objectKey,
        sizeBytes: body.sourceVersion.sizeBytes,
      })
      .where(
        and(
          eq(fileTransfers.id, transferId),
          eq(fileTransfers.status, 'REQUESTED'),
          gt(fileTransfers.expiresAt, new Date()),
        ),
      )
      .returning({ id: fileTransfers.id });
    if (!updated[0])
      throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
    return { transferId, uploadUrl, expiresAt: transfer.expiresAt };
  }
  async complete(
    userId: string,
    deviceId: string,
    transferId: string,
    idempotencyKey: string,
    body: z.infer<typeof completeFileUploadSchema>,
  ) {
    const existing = (
      await this.db
        .select()
        .from(fileTransfers)
        .where(
          and(
            eq(fileTransfers.requestedByUserId, userId),
            eq(fileTransfers.uploadCompletionIdempotencyKey, idempotencyKey),
          ),
        )
        .limit(1)
    )[0];
    if (existing) {
      if (
        existing.id === transferId &&
        existing.desktopDeviceId === deviceId &&
        existing.status === 'READY' &&
        existing.sizeBytes === body.sizeBytes &&
        existing.sha256 === body.sha256
      )
        return this.publicTransfer(existing);
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    }
    const transfer = await this.owned(userId, transferId, deviceId);
    if (
      transfer.status !== 'UPLOADING' ||
      !transfer.objectKey ||
      transfer.sizeBytes !== body.sizeBytes
    )
      throw new ConflictException({ code: 'SOURCE_CHANGED' });
    const storedSize = await this.storage.size(transfer.objectKey);
    if (storedSize !== body.sizeBytes)
      throw new ConflictException({ code: 'SOURCE_CHANGED' });
    return this.db.transaction(async (tx) => {
      const current = await this.lockOwned(tx, userId, transferId, deviceId);
      if (
        current.status === 'READY' &&
        current.uploadCompletionIdempotencyKey === idempotencyKey &&
        current.sizeBytes === body.sizeBytes &&
        current.sha256 === body.sha256
      ) {
        return this.publicTransfer(current);
      }
      if (
        current.status !== 'UPLOADING' ||
        !current.objectKey ||
        current.sizeBytes !== body.sizeBytes ||
        current.objectKey !== transfer.objectKey
      ) {
        throw new ConflictException({ code: 'SOURCE_CHANGED' });
      }
      const updated = (
        await tx
          .update(fileTransfers)
          .set({
            status: 'READY',
            sha256: body.sha256,
            uploadCompletionIdempotencyKey: idempotencyKey,
          })
          .where(
            and(
              eq(fileTransfers.id, transferId),
              eq(fileTransfers.status, 'UPLOADING'),
            ),
          )
          .returning()
      )[0];
      if (!updated)
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: updated.roomId,
        eventType: 'file.transfer.updated',
        aggregateType: 'file_transfer',
        aggregateId: updated.id,
        payload: { transferId: updated.id, status: updated.status },
      });
      return this.publicTransfer(updated);
    });
  }

  async fail(
    userId: string,
    deviceId: string,
    transferId: string,
    body: z.infer<typeof failFileTransferSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      const transfer = await this.lockOwned(tx, userId, transferId, deviceId);
      if (transfer.status === 'FAILED') {
        if (transfer.failureCode === body.failureCode)
          return this.publicTransfer(transfer);
        throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
      }
      if (!['REQUESTED', 'UPLOADING'].includes(transfer.status))
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      const failed = (
        await tx
          .update(fileTransfers)
          .set({
            status: 'FAILED',
            failureCode: body.failureCode,
            completedAt: new Date(),
          })
          .where(
            and(
              eq(fileTransfers.id, transferId),
              eq(fileTransfers.status, transfer.status),
            ),
          )
          .returning()
      )[0];
      if (!failed) {
        const current = await this.lockOwned(tx, userId, transferId, deviceId);
        if (
          current.status === 'FAILED' &&
          current.failureCode === body.failureCode
        )
          return this.publicTransfer(current);
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      }
      if (failed.objectKey) {
        await tx
          .insert(objectDeletionJobs)
          .values({ transferId, objectKey: failed.objectKey })
          .onConflictDoNothing();
      }
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: failed.roomId,
        eventType: 'file.transfer.updated',
        aggregateType: 'file_transfer',
        aggregateId: failed.id,
        payload: {
          transferId: failed.id,
          status: failed.status,
          failureCode: failed.failureCode,
        },
      });
      return this.publicTransfer(failed);
    });
  }

  async download(userId: string, transferId: string) {
    return this.db.transaction(async (tx) => {
      const transfer = await this.lockOwned(tx, userId, transferId);
      if (
        transfer.status !== 'READY' ||
        !transfer.objectKey ||
        transfer.expiresAt <= new Date()
      )
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      return {
        downloadUrl: await this.storage.downloadUrl(
          transfer.objectKey,
          Math.max(
            1,
            Math.min(
              60,
              Math.floor((transfer.expiresAt.getTime() - Date.now()) / 1000),
            ),
          ),
        ),
        sizeBytes: transfer.sizeBytes,
        sha256: transfer.sha256,
        expiresAt: transfer.expiresAt,
      };
    });
  }
  async get(userId: string, transferId: string) {
    return this.publicTransfer(await this.owned(userId, transferId));
  }
  async ack(userId: string, transferId: string) {
    return this.db.transaction(async (tx) => {
      const transfer = await this.lockOwned(tx, userId, transferId);
      if (transfer.status === 'COMPLETED') return this.publicTransfer(transfer);
      if (transfer.status !== 'READY' || !transfer.objectKey)
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      const updated = (
        await tx
          .update(fileTransfers)
          .set({ status: 'COMPLETED', completedAt: new Date() })
          .where(
            and(
              eq(fileTransfers.id, transferId),
              eq(fileTransfers.status, 'READY'),
            ),
          )
          .returning()
      )[0];
      if (!updated)
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      await tx
        .insert(objectDeletionJobs)
        .values({ transferId, objectKey: transfer.objectKey })
        .onConflictDoNothing();
      await this.sync.append(tx, {
        userId,
        deviceId: transfer.desktopDeviceId,
        roomId: transfer.roomId,
        eventType: 'file.transfer.updated',
        aggregateType: 'file_transfer',
        aggregateId: transfer.id,
        payload: { transferId: transfer.id, status: updated.status },
      });
      return this.publicTransfer(updated);
    });
  }

  async cancel(
    userId: string,
    deviceId: string | undefined,
    transferId: string,
  ) {
    return this.db.transaction(async (tx) => {
      const transfer = await this.lockOwned(tx, userId, transferId, deviceId);
      if (transfer.status === 'CANCELLED') return this.publicTransfer(transfer);
      if (['COMPLETED', 'EXPIRED'].includes(transfer.status))
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      const updated = (
        await tx
          .update(fileTransfers)
          .set({ status: 'CANCELLED', completedAt: new Date() })
          .where(
            and(
              eq(fileTransfers.id, transferId),
              eq(fileTransfers.status, transfer.status),
            ),
          )
          .returning()
      )[0];
      if (!updated) {
        const current = await this.lockOwned(tx, userId, transferId, deviceId);
        if (current.status === 'CANCELLED') return this.publicTransfer(current);
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      }
      if (transfer.objectKey) {
        await tx
          .insert(objectDeletionJobs)
          .values({ transferId, objectKey: transfer.objectKey })
          .onConflictDoNothing();
      }
      await this.sync.append(tx, {
        userId,
        deviceId: transfer.desktopDeviceId,
        roomId: transfer.roomId,
        eventType: 'file.transfer.updated',
        aggregateType: 'file_transfer',
        aggregateId: transfer.id,
        payload: { transferId: transfer.id, status: updated.status },
      });
      return this.publicTransfer(updated);
    });
  }

  private async owned(userId: string, transferId: string, deviceId?: string) {
    const transfer = (
      await this.db
        .select({ transfer: fileTransfers })
        .from(fileTransfers)
        .innerJoin(
          rooms,
          and(
            eq(fileTransfers.roomId, rooms.id),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .where(
          and(
            eq(fileTransfers.id, transferId),
            eq(fileTransfers.requestedByUserId, userId),
            ...(deviceId ? [eq(fileTransfers.desktopDeviceId, deviceId)] : []),
          ),
        )
        .limit(1)
    )[0]?.transfer;
    if (!transfer) throw new NotFoundException({ code: 'NOT_FOUND' });
    return transfer;
  }

  private async lockOwned(
    tx: Transaction,
    userId: string,
    transferId: string,
    deviceId?: string,
  ) {
    const candidate = (
      await tx
        .select()
        .from(fileTransfers)
        .where(
          and(
            eq(fileTransfers.id, transferId),
            eq(fileTransfers.requestedByUserId, userId),
            ...(deviceId ? [eq(fileTransfers.desktopDeviceId, deviceId)] : []),
          ),
        )
        .limit(1)
    )[0];
    if (!candidate) throw new NotFoundException({ code: 'NOT_FOUND' });

    const device = (
      await tx
        .select()
        .from(devices)
        .where(
          and(
            eq(devices.id, candidate.desktopDeviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });

    const room = (
      await tx
        .select()
        .from(rooms)
        .where(
          and(
            eq(rooms.id, candidate.roomId),
            eq(rooms.desktopDeviceId, device.id),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .for('share')
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });

    const transfer = (
      await tx
        .select()
        .from(fileTransfers)
        .where(
          and(
            eq(fileTransfers.id, transferId),
            eq(fileTransfers.desktopDeviceId, device.id),
            eq(fileTransfers.roomId, room.id),
            eq(fileTransfers.requestedByUserId, userId),
          ),
        )
        .for('update')
        .limit(1)
    )[0];
    if (!transfer) throw new NotFoundException({ code: 'NOT_FOUND' });
    return transfer;
  }

  private publicTransfer(transfer: typeof fileTransfers.$inferSelect) {
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
