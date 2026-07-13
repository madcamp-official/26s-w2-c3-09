import {
  ConflictException,
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
  ServiceUnavailableException,
} from '@nestjs/common';
import {
  cacheCandidateBatchSchema,
  completeCacheUploadSchema,
  updateSmartCachePolicySchema,
} from '@mousekeeper/contracts';
import {
  cacheDeletionJobs,
  cacheCandidateBatches,
  cacheReservationDeletionJobs,
  cachedFiles,
  cacheUploadReservations,
  commands,
  devices,
  rooms,
  smartCachePolicies,
  type Database,
} from '@mousekeeper/database';
import { and, asc, eq, gt, inArray, sql, sum } from 'drizzle-orm';
import { createHash, randomUUID } from 'node:crypto';
import { z } from 'zod';
import Redis from 'ioredis';
import { loadEnvironment } from '../config/environment';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';
import { ObjectStorageService } from '../transfers/object-storage.service';
import { SyncService } from '../sync/sync.service';
import { canonicalJson } from '../common/canonical-json';
import { matchesExcludedPattern } from './cache-policy';

type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];

@Injectable()
export class SmartCacheService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly storage: ObjectStorageService,
    private readonly sync: SyncService,
  ) {}
  private enabled() {
    if (!loadEnvironment().SMART_CACHE_ENABLED)
      throw new ServiceUnavailableException({
        code: 'UNCONFIGURED',
        provider: 'SMART_CACHE',
      });
  }
  private async room(userId: string, roomId: string) {
    const room = (
      await this.db
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
  async getPolicy(userId: string, roomId: string) {
    await this.room(userId, roomId);
    const policy = (
      await this.db
        .select()
        .from(smartCachePolicies)
        .where(eq(smartCachePolicies.roomId, roomId))
        .limit(1)
    )[0];
    const env = loadEnvironment();
    return (
      policy ?? {
        roomId,
        enabled: false,
        quotaBytes: env.SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES,
        maxFileBytes: env.SMART_CACHE_DEFAULT_MAX_FILE_BYTES,
        excludedPatterns: [],
      }
    );
  }
  async updatePolicy(
    userId: string,
    roomId: string,
    body: z.infer<typeof updateSmartCachePolicySchema>,
  ) {
    const room = await this.room(userId, roomId);
    if (body.enabled) {
      this.enabled();
      this.storage.assertConfigured();
    }
    return this.db.transaction(async (tx) => {
      await tx.execute(sql`select pg_advisory_xact_lock(hashtext(${roomId}))`);
      await this.lockActiveRoom(tx, userId, room.desktopDeviceId, roomId);
      if (body.enabled) {
        const availableBytes = Number(
          (
            await tx
              .select({ value: sum(cachedFiles.sizeBytes) })
              .from(cachedFiles)
              .where(
                and(
                  eq(cachedFiles.roomId, roomId),
                  eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
                ),
              )
          )[0]?.value ?? 0,
        );
        const reservedBytes = Number(
          (
            await tx
              .select({ value: sum(cacheUploadReservations.reservedBytes) })
              .from(cacheUploadReservations)
              .where(
                and(
                  eq(cacheUploadReservations.roomId, roomId),
                  eq(cacheUploadReservations.status, 'RESERVED'),
                  gt(cacheUploadReservations.expiresAt, new Date()),
                ),
              )
          )[0]?.value ?? 0,
        );
        if (availableBytes + reservedBytes > body.quotaBytes)
          throw new ConflictException({ code: 'REJECTED_POLICY' });
      }
      const policy = (
        await tx
          .insert(smartCachePolicies)
          .values({ roomId, ...body })
          .onConflictDoUpdate({
            target: smartCachePolicies.roomId,
            set: { ...body, updatedAt: new Date() },
          })
          .returning()
      )[0];
      if (!body.enabled) {
        const files = await tx
          .select()
          .from(cachedFiles)
          .where(
            and(
              eq(cachedFiles.roomId, roomId),
              eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
            ),
          );
        for (const file of files) {
          await tx
            .update(cachedFiles)
            .set({
              availabilityStatus: 'INVALIDATED',
              freshnessStatus: 'STALE',
            })
            .where(eq(cachedFiles.id, file.id));
          await tx
            .insert(cacheDeletionJobs)
            .values({ cachedFileId: file.id, objectKey: file.objectKey })
            .onConflictDoNothing();
        }
        const reservations = await tx
          .select()
          .from(cacheUploadReservations)
          .where(
            and(
              eq(cacheUploadReservations.roomId, roomId),
              eq(cacheUploadReservations.status, 'RESERVED'),
            ),
          );
        for (const reservation of reservations) {
          await tx
            .update(cacheUploadReservations)
            .set({ status: 'CANCELLED' })
            .where(eq(cacheUploadReservations.id, reservation.id));
          await tx
            .insert(cacheReservationDeletionJobs)
            .values({
              reservationId: reservation.id,
              objectKey: reservation.objectKey,
            })
            .onConflictDoNothing();
        }
      }
      await this.sync.append(tx, {
        userId,
        deviceId: room.desktopDeviceId,
        roomId,
        eventType: 'smart-cache.updated',
        aggregateType: 'smart_cache_policy',
        aggregateId: roomId,
        payload: { roomId, enabled: body.enabled },
      });
      return policy;
    });
  }
  async list(userId: string, roomId: string) {
    const room = await this.room(userId, roomId);
    const files = await this.db
      .select()
      .from(cachedFiles)
      .where(
        and(
          eq(cachedFiles.roomId, roomId),
          eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
        ),
      );
    const pending = await this.db
      .select({ id: commands.id })
      .from(commands)
      .where(
        and(
          eq(commands.roomId, roomId),
          inArray(commands.status, ['QUEUED', 'DELIVERED', 'ANALYZING']),
        ),
      )
      .limit(1);
    if (this.redis.status === 'wait') await this.redis.connect();
    const online = Boolean(
      await this.redis.exists(`presence:${room.desktopDeviceId}`),
    );
    return {
      files: files.map((file) =>
        this.publicFile(file, {
          freshnessStatus:
            !online && file.freshnessStatus === 'VERIFIED_CURRENT'
              ? 'UNVERIFIED_OFFLINE'
              : file.freshnessStatus,
        }),
      ),
      pendingCommandWarning: pending.length > 0,
      desktopOnline: online,
    };
  }
  async submit(
    userId: string,
    deviceId: string,
    idempotencyKey: string,
    body: z.infer<typeof cacheCandidateBatchSchema>,
  ) {
    this.enabled();
    this.storage.assertConfigured();
    const room = await this.room(userId, body.roomId);
    if (room.desktopDeviceId !== deviceId)
      throw new ForbiddenException({ code: 'FORBIDDEN' });
    const requestHash = createHash('sha256')
      .update(canonicalJson(body))
      .digest('hex');
    const result = await this.db.transaction(async (tx) => {
      await tx.execute(
        sql`select pg_advisory_xact_lock(hashtext(${body.roomId}))`,
      );
      const activeDevice = (
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
      if (!activeDevice) throw new ForbiddenException({ code: 'FORBIDDEN' });
      const activeRoom = (
        await tx
          .select()
          .from(rooms)
          .where(
            and(
              eq(rooms.id, body.roomId),
              eq(rooms.desktopDeviceId, deviceId),
              eq(rooms.userId, userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!activeRoom) throw new NotFoundException({ code: 'NOT_FOUND' });
      const replay = (
        await tx
          .select()
          .from(cacheCandidateBatches)
          .where(
            and(
              eq(cacheCandidateBatches.userId, userId),
              eq(cacheCandidateBatches.idempotencyKey, idempotencyKey),
            ),
          )
          .limit(1)
      )[0];
      if (replay) {
        if (
          replay.desktopDeviceId !== deviceId ||
          replay.roomId !== body.roomId ||
          replay.requestHash !== requestHash
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        return {
          batch: replay,
          reservations: await tx
            .select()
            .from(cacheUploadReservations)
            .where(eq(cacheUploadReservations.batchId, replay.id)),
        };
      }
      const batch = (
        await tx
          .insert(cacheCandidateBatches)
          .values({
            userId,
            desktopDeviceId: deviceId,
            roomId: body.roomId,
            idempotencyKey,
            requestHash,
            candidateCount: body.candidates.length,
          })
          .returning()
      )[0];
      const policy = (
        await tx
          .select()
          .from(smartCachePolicies)
          .where(
            and(
              eq(smartCachePolicies.roomId, body.roomId),
              eq(smartCachePolicies.enabled, true),
            ),
          )
          .limit(1)
      )[0];
      if (!policy) throw new ConflictException({ code: 'REJECTED_POLICY' });
      const used = Number(
        (
          await tx
            .select({ value: sum(cachedFiles.sizeBytes) })
            .from(cachedFiles)
            .where(
              and(
                eq(cachedFiles.roomId, body.roomId),
                eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
              ),
            )
        )[0]?.value ?? 0,
      );
      const reserved = Number(
        (
          await tx
            .select({ value: sum(cacheUploadReservations.reservedBytes) })
            .from(cacheUploadReservations)
            .where(
              and(
                eq(cacheUploadReservations.roomId, body.roomId),
                eq(cacheUploadReservations.status, 'RESERVED'),
                gt(cacheUploadReservations.expiresAt, new Date()),
              ),
            )
        )[0]?.value ?? 0,
      );
      let allocated = used + reserved;
      const evictable = await tx
        .select()
        .from(cachedFiles)
        .where(
          and(
            eq(cachedFiles.roomId, body.roomId),
            eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
            eq(cachedFiles.manualPin, false),
          ),
        )
        .orderBy(
          asc(cachedFiles.usageScore),
          asc(cachedFiles.lastAccessedAt),
          asc(cachedFiles.cachedAt),
        );
      let evictionIndex = 0;
      const approved = [];
      for (const candidate of [...body.candidates].sort(
        (a, b) =>
          Number(b.manualPin) - Number(a.manualPin) ||
          b.usageScore - a.usageScore,
      )) {
        if (
          matchesExcludedPattern(
            candidate.sourceRelativePath,
            policy.excludedPatterns as string[],
          )
        )
          continue;
        if (candidate.sizeBytes > policy.maxFileBytes) continue;
        while (
          allocated + candidate.sizeBytes > policy.quotaBytes &&
          evictionIndex < evictable.length
        ) {
          const file = evictable[evictionIndex++];
          if (!candidate.manualPin && file.usageScore >= candidate.usageScore)
            break;
          const invalidated = (
            await tx
              .update(cachedFiles)
              .set({
                availabilityStatus: 'INVALIDATED',
                freshnessStatus: 'STALE',
              })
              .where(
                and(
                  eq(cachedFiles.id, file.id),
                  eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
                ),
              )
              .returning()
          )[0];
          if (!invalidated) continue;
          allocated -= invalidated.sizeBytes;
          await tx
            .insert(cacheDeletionJobs)
            .values({
              cachedFileId: invalidated.id,
              objectKey: invalidated.objectKey,
            })
            .onConflictDoNothing();
        }
        if (allocated + candidate.sizeBytes > policy.quotaBytes) continue;
        const id = randomUUID();
        const row = (
          await tx
            .insert(cacheUploadReservations)
            .values({
              id,
              batchId: batch.id,
              roomId: body.roomId,
              desktopDeviceId: deviceId,
              sourceRelativePath: candidate.sourceRelativePath,
              sourceVersion: candidate.sourceVersion,
              sourceVersionHash: candidate.sourceVersionHash,
              reservedBytes: candidate.sizeBytes,
              expiresAt: new Date(Date.now() + 10 * 60_000),
              objectKey: `cache/${userId}/${body.roomId}/${id}`,
            })
            .onConflictDoNothing()
            .returning()
        )[0];
        if (row) {
          approved.push(row);
          allocated += candidate.sizeBytes;
        }
      }
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: body.roomId,
        eventType: 'smart-cache.updated',
        aggregateType: 'cache_candidate_batch',
        aggregateId: batch.id,
        payload: {
          roomId: body.roomId,
          batchId: batch.id,
          approvedCount: approved.length,
        },
      });
      return { batch, reservations: approved };
    });
    const approvedReservations = result.reservations.filter(
      (reservation) =>
        reservation.status === 'COMPLETED' ||
        (reservation.status === 'RESERVED' &&
          reservation.expiresAt > new Date()),
    );
    return {
      batchId: result.batch.id,
      approved: await Promise.all(
        approvedReservations.map(async (reservation) => ({
          reservationId: reservation.id,
          status: reservation.status,
          sourceRelativePath: reservation.sourceRelativePath,
          sourceVersionHash: reservation.sourceVersionHash,
          sizeBytes: reservation.reservedBytes,
          ...(reservation.status === 'RESERVED'
            ? {
                uploadUrl: await this.storage.uploadUrl(
                  reservation.objectKey,
                  Math.max(
                    1,
                    Math.floor(
                      (reservation.expiresAt.getTime() - Date.now()) / 1000,
                    ),
                  ),
                ),
              }
            : {}),
          expiresAt: reservation.expiresAt,
        })),
      ),
      rejectedCount: result.batch.candidateCount - approvedReservations.length,
    };
  }
  async complete(
    userId: string,
    deviceId: string,
    reservationId: string,
    idempotencyKey: string,
    body: z.infer<typeof completeCacheUploadSchema>,
  ) {
    this.enabled();
    const replay = (
      await this.db
        .select({ reservation: cacheUploadReservations, room: rooms })
        .from(cacheUploadReservations)
        .innerJoin(rooms, eq(cacheUploadReservations.roomId, rooms.id))
        .where(
          and(
            eq(cacheUploadReservations.desktopDeviceId, deviceId),
            eq(
              cacheUploadReservations.completionIdempotencyKey,
              idempotencyKey,
            ),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (replay) {
      if (replay.reservation.id !== reservationId)
        throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
      const cached = await this.completedCache(replay.reservation, body);
      return this.publicFile(cached);
    }
    const reservation = (
      await this.db
        .select({ reservation: cacheUploadReservations, room: rooms })
        .from(cacheUploadReservations)
        .innerJoin(rooms, eq(cacheUploadReservations.roomId, rooms.id))
        .where(
          and(
            eq(cacheUploadReservations.id, reservationId),
            eq(cacheUploadReservations.desktopDeviceId, deviceId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0]?.reservation;
    if (
      !reservation ||
      reservation.status !== 'RESERVED' ||
      reservation.expiresAt <= new Date()
    )
      throw new NotFoundException({ code: 'RESERVATION_EXPIRED' });
    if (
      reservation.reservedBytes !== body.sizeBytes ||
      (await this.storage.size(reservation.objectKey)) !== body.sizeBytes
    )
      throw new ConflictException({ code: 'UPLOAD_FAILED' });
    return this.db.transaction(async (tx) => {
      const activeDevice = (
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
      if (!activeDevice) throw new ForbiddenException({ code: 'FORBIDDEN' });
      const activeRoom = (
        await tx
          .select()
          .from(rooms)
          .where(
            and(
              eq(rooms.id, reservation.roomId),
              eq(rooms.desktopDeviceId, deviceId),
              eq(rooms.userId, userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!activeRoom)
        throw new NotFoundException({ code: 'RESERVATION_EXPIRED' });
      const current = (
        await tx
          .select()
          .from(cacheUploadReservations)
          .where(eq(cacheUploadReservations.id, reservationId))
          .for('update')
          .limit(1)
      )[0];
      if (!current)
        throw new NotFoundException({ code: 'RESERVATION_EXPIRED' });
      if (current.status === 'COMPLETED') {
        if (current.completionIdempotencyKey !== idempotencyKey)
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        const cached = await tx
          .select()
          .from(cachedFiles)
          .where(
            and(
              eq(cachedFiles.roomId, current.roomId),
              eq(cachedFiles.sourceRelativePath, current.sourceRelativePath),
              eq(cachedFiles.sourceVersionHash, current.sourceVersionHash),
            ),
          )
          .limit(1);
        const result = cached[0];
        if (
          !result ||
          result.sizeBytes !== body.sizeBytes ||
          result.sha256 !== body.sha256 ||
          result.usageScore !== body.usageScore ||
          result.manualPin !== body.manualPin ||
          canonicalJson(result.encryptionMetadata) !==
            canonicalJson(body.encryptionMetadata)
        )
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        return this.publicFile(result);
      }
      if (current.status !== 'RESERVED' || current.expiresAt <= new Date())
        throw new NotFoundException({ code: 'RESERVATION_EXPIRED' });
      const old = await tx
        .select()
        .from(cachedFiles)
        .where(
          and(
            eq(cachedFiles.roomId, reservation.roomId),
            eq(cachedFiles.sourceRelativePath, reservation.sourceRelativePath),
            eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
          ),
        );
      for (const file of old) {
        await tx
          .update(cachedFiles)
          .set({ availabilityStatus: 'INVALIDATED', freshnessStatus: 'STALE' })
          .where(eq(cachedFiles.id, file.id));
        await tx
          .insert(cacheDeletionJobs)
          .values({ cachedFileId: file.id, objectKey: file.objectKey })
          .onConflictDoNothing();
      }
      const cached = (
        await tx
          .insert(cachedFiles)
          .values({
            roomId: reservation.roomId,
            sourceRelativePath: reservation.sourceRelativePath,
            sourceVersion: reservation.sourceVersion,
            sourceVersionHash: reservation.sourceVersionHash,
            usageScore: body.usageScore,
            manualPin: body.manualPin,
            objectKey: reservation.objectKey,
            sizeBytes: body.sizeBytes,
            sha256: body.sha256,
            encryptionMetadata: body.encryptionMetadata,
            lastVerifiedAt: new Date(),
          })
          .onConflictDoUpdate({
            target: [
              cachedFiles.roomId,
              cachedFiles.sourceRelativePath,
              cachedFiles.sourceVersionHash,
            ],
            set: {
              availabilityStatus: 'AVAILABLE',
              freshnessStatus: 'VERIFIED_CURRENT',
              sizeBytes: body.sizeBytes,
              sha256: body.sha256,
              usageScore: body.usageScore,
              manualPin: body.manualPin,
              encryptionMetadata: body.encryptionMetadata,
              lastVerifiedAt: new Date(),
            },
          })
          .returning()
      )[0];
      await tx
        .update(cacheUploadReservations)
        .set({
          status: 'COMPLETED',
          completionIdempotencyKey: idempotencyKey,
        })
        .where(eq(cacheUploadReservations.id, reservationId));
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: reservation.roomId,
        eventType: 'smart-cache.updated',
        aggregateType: 'cached_file',
        aggregateId: cached.id,
        payload: { cachedFileId: cached.id, status: 'AVAILABLE' },
      });
      return this.publicFile(cached);
    });
  }

  async cancelReservation(
    userId: string,
    deviceId: string,
    reservationId: string,
  ) {
    const reservation = (
      await this.db
        .select({ reservation: cacheUploadReservations, room: rooms })
        .from(cacheUploadReservations)
        .innerJoin(rooms, eq(cacheUploadReservations.roomId, rooms.id))
        .where(
          and(
            eq(cacheUploadReservations.id, reservationId),
            eq(cacheUploadReservations.desktopDeviceId, deviceId),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!reservation) throw new NotFoundException({ code: 'NOT_FOUND' });
    if (reservation.reservation.status === 'CANCELLED')
      return { reservationId, status: 'CANCELLED' };
    if (reservation.reservation.status !== 'RESERVED')
      throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
    return this.db.transaction(async (tx) => {
      await this.lockActiveRoom(
        tx,
        userId,
        deviceId,
        reservation.reservation.roomId,
      );
      const current = (
        await tx
          .select()
          .from(cacheUploadReservations)
          .where(
            and(
              eq(cacheUploadReservations.id, reservationId),
              eq(cacheUploadReservations.desktopDeviceId, deviceId),
              eq(
                cacheUploadReservations.roomId,
                reservation.reservation.roomId,
              ),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!current) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (current.status === 'CANCELLED') {
        return { reservationId, status: 'CANCELLED' };
      }
      if (current.status !== 'RESERVED') {
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      }
      const cancelled = (
        await tx
          .update(cacheUploadReservations)
          .set({ status: 'CANCELLED' })
          .where(
            and(
              eq(cacheUploadReservations.id, reservationId),
              eq(cacheUploadReservations.status, 'RESERVED'),
            ),
          )
          .returning()
      )[0];
      if (!cancelled)
        throw new ConflictException({ code: 'INVALID_STATE_TRANSITION' });
      await tx
        .insert(cacheReservationDeletionJobs)
        .values({
          reservationId,
          objectKey: cancelled.objectKey,
          nextAttemptAt: cancelled.expiresAt,
        })
        .onConflictDoNothing();
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId: cancelled.roomId,
        eventType: 'smart-cache.updated',
        aggregateType: 'cache_upload_reservation',
        aggregateId: reservationId,
        payload: { reservationId, status: 'CANCELLED' },
      });
      return { reservationId, status: cancelled.status };
    });
  }

  async remove(userId: string, cachedFileId: string) {
    const owned = (
      await this.db
        .select({ file: cachedFiles, room: rooms })
        .from(cachedFiles)
        .innerJoin(
          rooms,
          and(
            eq(cachedFiles.roomId, rooms.id),
            eq(rooms.userId, userId),
            eq(rooms.status, 'ACTIVE'),
          ),
        )
        .where(eq(cachedFiles.id, cachedFileId))
        .limit(1)
    )[0];
    if (!owned) throw new NotFoundException({ code: 'NOT_FOUND' });
    if (owned.file.availabilityStatus !== 'AVAILABLE')
      return this.publicFile(owned.file);
    return this.db.transaction(async (tx) => {
      await this.lockActiveRoom(
        tx,
        userId,
        owned.room.desktopDeviceId,
        owned.room.id,
      );
      const current = (
        await tx
          .select()
          .from(cachedFiles)
          .where(
            and(
              eq(cachedFiles.id, cachedFileId),
              eq(cachedFiles.roomId, owned.room.id),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!current) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (current.availabilityStatus !== 'AVAILABLE')
        return this.publicFile(current);
      const removed = (
        await tx
          .update(cachedFiles)
          .set({ availabilityStatus: 'INVALIDATED', freshnessStatus: 'STALE' })
          .where(eq(cachedFiles.id, cachedFileId))
          .returning()
      )[0];
      await tx
        .insert(cacheDeletionJobs)
        .values({ cachedFileId, objectKey: current.objectKey })
        .onConflictDoNothing();
      await this.sync.append(tx, {
        userId,
        deviceId: null,
        roomId: current.roomId,
        eventType: 'smart-cache.updated',
        aggregateType: 'cached_file',
        aggregateId: cachedFileId,
        payload: { cachedFileId, status: 'INVALIDATED' },
      });
      return this.publicFile(removed);
    });
  }

  async download(userId: string, cachedFileId: string) {
    this.enabled();
    return this.db.transaction(async (tx) => {
      const candidate = (
        await tx
          .select({ file: cachedFiles, room: rooms })
          .from(cachedFiles)
          .innerJoin(
            rooms,
            and(
              eq(cachedFiles.roomId, rooms.id),
              eq(rooms.userId, userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .where(eq(cachedFiles.id, cachedFileId))
          .limit(1)
      )[0];
      if (!candidate) throw new NotFoundException({ code: 'NOT_FOUND' });
      await this.lockActiveRoom(
        tx,
        userId,
        candidate.room.desktopDeviceId,
        candidate.room.id,
      );
      const owned = (
        await tx
          .select()
          .from(cachedFiles)
          .where(
            and(
              eq(cachedFiles.id, cachedFileId),
              eq(cachedFiles.roomId, candidate.room.id),
              eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
            ),
          )
          .for('update')
          .limit(1)
      )[0];
      if (!owned) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (!owned.sha256)
        throw new ConflictException({ code: 'CHECKSUM_UNAVAILABLE' });
      await tx
        .update(cachedFiles)
        .set({ lastAccessedAt: new Date() })
        .where(eq(cachedFiles.id, cachedFileId));
      return {
        downloadUrl: await this.storage.downloadUrl(owned.objectKey, 60),
        sizeBytes: owned.sizeBytes,
        sha256: owned.sha256,
        encryptionMetadata: owned.encryptionMetadata,
        freshnessStatus: owned.freshnessStatus,
        lastVerifiedAt: owned.lastVerifiedAt,
      };
    });
  }

  private async lockActiveRoom(
    tx: Transaction,
    userId: string,
    deviceId: string,
    roomId: string,
  ) {
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
    return room;
  }

  private async completedCache(
    reservation: typeof cacheUploadReservations.$inferSelect,
    body: z.infer<typeof completeCacheUploadSchema>,
  ) {
    const cached = (
      await this.db
        .select()
        .from(cachedFiles)
        .where(
          and(
            eq(cachedFiles.roomId, reservation.roomId),
            eq(cachedFiles.sourceRelativePath, reservation.sourceRelativePath),
            eq(cachedFiles.sourceVersionHash, reservation.sourceVersionHash),
          ),
        )
        .limit(1)
    )[0];
    if (
      !cached ||
      cached.sizeBytes !== body.sizeBytes ||
      cached.sha256 !== body.sha256 ||
      cached.usageScore !== body.usageScore ||
      cached.manualPin !== body.manualPin ||
      canonicalJson(cached.encryptionMetadata) !==
        canonicalJson(body.encryptionMetadata)
    )
      throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
    return cached;
  }

  private publicFile(
    file: typeof cachedFiles.$inferSelect,
    overrides: Partial<typeof cachedFiles.$inferSelect> = {},
  ) {
    const { objectKey: _, ...safe } = { ...file, ...overrides };
    void _;
    return safe;
  }
}
