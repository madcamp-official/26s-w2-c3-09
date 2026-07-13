import {
  ConflictException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import {
  deviceRevokedEventPayloadSchema,
  roomRemovedEventPayloadSchema,
} from '@mousekeeper/contracts';
import {
  auditEvents,
  cacheDeletionJobs,
  cacheReservationDeletionJobs,
  cacheUploadReservations,
  cachedFiles,
  connectionMutationReceipts,
  devices,
  fileBrowseRequests,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, eq, inArray, sql } from 'drizzle-orm';
import Redis from 'ioredis';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';
import { RealtimeDispatcher } from '../realtime/realtime-dispatcher.service';
import { RealtimeGateway } from '../realtime/realtime.gateway';
import { SyncService } from '../sync/sync.service';

type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
type MutationOperation = 'DEVICE_REVOKE' | 'ROOM_REMOVE';

export type ConnectionMutationActor = {
  userId: string;
  actorDeviceId: string | null;
  actorScope: string;
};

type StoredResult = Record<string, unknown>;
type MutationOutcome = {
  response: StoredResult;
  eventIds: string[];
  disconnectDeviceId: string | null;
};

@Injectable()
export class ConnectionLifecycleService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly sync: SyncService,
    private readonly realtime: RealtimeDispatcher,
    private readonly gateway: RealtimeGateway,
  ) {}

  async revokeDevice(
    actor: ConnectionMutationActor,
    deviceId: string,
    idempotencyKey: string,
  ) {
    const outcome = await this.withReceipt(
      actor,
      idempotencyKey,
      'DEVICE_REVOKE',
      deviceId,
      async (tx) => {
        const device = (
          await tx
            .select()
            .from(devices)
            .where(
              and(
                eq(devices.id, deviceId),
                eq(devices.userId, actor.userId),
                eq(devices.status, 'ACTIVE'),
              ),
            )
            .for('update')
            .limit(1)
        )[0];
        if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });

        const deviceRooms = await tx
          .select()
          .from(rooms)
          .where(eq(rooms.desktopDeviceId, deviceId))
          .for('update');
        const activeRooms = deviceRooms.filter(
          (room) => room.status === 'ACTIVE',
        );

        await this.cancelResources(tx, {
          deviceId,
          roomIds: deviceRooms.map((room) => room.id),
          browseFailureCode: 'DEVICE_OFFLINE',
        });

        const revoked = (
          await tx
            .update(devices)
            .set({ status: 'REVOKED' })
            .where(and(eq(devices.id, deviceId), eq(devices.status, 'ACTIVE')))
            .returning()
        )[0];
        if (!revoked) throw new ConflictException({ code: 'CONFLICT' });

        if (activeRooms.length) {
          await tx
            .update(rooms)
            .set({ status: 'REMOVED' })
            .where(
              and(
                inArray(
                  rooms.id,
                  activeRooms.map((room) => room.id),
                ),
                eq(rooms.status, 'ACTIVE'),
              ),
            );
        }

        const eventIds: string[] = [];
        for (const room of activeRooms) {
          await tx.insert(auditEvents).values({
            userId: actor.userId,
            deviceId,
            roomId: room.id,
            eventType: 'room.removed',
            aggregateType: 'room',
            aggregateId: room.id,
            metadata: {},
          });
          const event = await this.sync.append(tx, {
            userId: actor.userId,
            deviceId,
            roomId: room.id,
            eventType: 'room.removed',
            aggregateType: 'room',
            aggregateId: room.id,
            payload: roomRemovedEventPayloadSchema.parse({
              roomId: room.id,
              status: 'REMOVED',
            }),
          });
          eventIds.push(event.id);
        }

        await tx.insert(auditEvents).values({
          userId: actor.userId,
          deviceId,
          eventType: 'device.revoked',
          aggregateType: 'device',
          aggregateId: deviceId,
          metadata: {},
        });
        const event = await this.sync.append(tx, {
          userId: actor.userId,
          deviceId,
          roomId: null,
          eventType: 'device.revoked',
          aggregateType: 'device',
          aggregateId: deviceId,
          payload: deviceRevokedEventPayloadSchema.parse({
            deviceId,
            status: 'REVOKED',
          }),
        });
        eventIds.push(event.id);

        return {
          response: this.publicDevice(revoked),
          eventIds,
          disconnectDeviceId: deviceId,
        };
      },
    );

    await this.clearPresence(deviceId);
    await this.publishAndDisconnect(outcome);
    return outcome.response;
  }

  async removeRoom(
    actor: ConnectionMutationActor,
    roomId: string,
    idempotencyKey: string,
  ) {
    const outcome = await this.withReceipt(
      actor,
      idempotencyKey,
      'ROOM_REMOVE',
      roomId,
      async (tx) => {
        const candidate = (
          await tx
            .select()
            .from(rooms)
            .where(
              and(
                eq(rooms.id, roomId),
                eq(rooms.userId, actor.userId),
                eq(rooms.status, 'ACTIVE'),
              ),
            )
            .limit(1)
        )[0];
        if (!candidate) throw new NotFoundException({ code: 'NOT_FOUND' });
        if (
          actor.actorDeviceId &&
          actor.actorDeviceId !== candidate.desktopDeviceId
        ) {
          throw new NotFoundException({ code: 'NOT_FOUND' });
        }

        // Every lifecycle-aware mutation locks device -> room -> work rows.
        // The device lock also prevents a room unlink from deadlocking with a
        // concurrent device revoke while event foreign keys are checked.
        const device = (
          await tx
            .select()
            .from(devices)
            .where(
              and(
                eq(devices.id, candidate.desktopDeviceId),
                eq(devices.userId, actor.userId),
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
                eq(rooms.userId, actor.userId),
                eq(rooms.status, 'ACTIVE'),
              ),
            )
            .for('update')
            .limit(1)
        )[0];
        if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });

        await this.cancelResources(tx, {
          roomIds: [roomId],
          browseFailureCode: 'OUTSIDE_MANAGED_ROOT',
        });

        const removed = (
          await tx
            .update(rooms)
            .set({ status: 'REMOVED' })
            .where(and(eq(rooms.id, roomId), eq(rooms.status, 'ACTIVE')))
            .returning()
        )[0];
        if (!removed) throw new ConflictException({ code: 'CONFLICT' });

        await tx.insert(auditEvents).values({
          userId: actor.userId,
          deviceId: room.desktopDeviceId,
          roomId,
          eventType: 'room.removed',
          aggregateType: 'room',
          aggregateId: roomId,
          metadata: {},
        });
        const event = await this.sync.append(tx, {
          userId: actor.userId,
          deviceId: room.desktopDeviceId,
          roomId,
          eventType: 'room.removed',
          aggregateType: 'room',
          aggregateId: roomId,
          payload: roomRemovedEventPayloadSchema.parse({
            roomId,
            status: 'REMOVED',
          }),
        });

        return {
          response: this.publicRoom(removed),
          eventIds: [event.id],
          disconnectDeviceId: null,
        };
      },
    );

    await this.publishAndDisconnect(outcome);
    return outcome.response;
  }

  private async withReceipt(
    actor: ConnectionMutationActor,
    idempotencyKey: string,
    operation: MutationOperation,
    aggregateId: string,
    mutate: (tx: Transaction) => Promise<MutationOutcome>,
  ): Promise<MutationOutcome> {
    return this.db.transaction(async (tx) => {
      // Serialize duplicate HTTP retries before any cleanup side effect occurs.
      await tx.execute(
        sql`select pg_advisory_xact_lock(hashtext(${`${actor.actorScope}:${idempotencyKey}`}))`,
      );
      const receipt = (
        await tx
          .select()
          .from(connectionMutationReceipts)
          .where(
            and(
              eq(connectionMutationReceipts.actorScope, actor.actorScope),
              eq(connectionMutationReceipts.idempotencyKey, idempotencyKey),
            ),
          )
          .limit(1)
      )[0];
      if (receipt) {
        if (
          receipt.operation !== operation ||
          receipt.aggregateId !== aggregateId
        ) {
          throw new ConflictException({ code: 'IDEMPOTENCY_CONFLICT' });
        }
        return {
          response: receipt.result as StoredResult,
          eventIds: [],
          // Re-run the harmless post-commit socket eviction on a self-revoke
          // retry in case the first HTTP response was lost mid-disconnect.
          disconnectDeviceId:
            operation === 'DEVICE_REVOKE' ? aggregateId : null,
        };
      }

      const outcome = await mutate(tx);
      await tx.insert(connectionMutationReceipts).values({
        actorScope: actor.actorScope,
        userId: actor.userId,
        actorDeviceId: actor.actorDeviceId,
        operation,
        aggregateId,
        idempotencyKey,
        result: outcome.response,
      });
      return outcome;
    });
  }

  private async cancelResources(
    tx: Transaction,
    scope: {
      deviceId?: string;
      roomIds: string[];
      browseFailureCode: 'DEVICE_OFFLINE' | 'OUTSIDE_MANAGED_ROOT';
    },
  ) {
    if (scope.roomIds.length) {
      const invalidated = await tx
        .update(cachedFiles)
        .set({ availabilityStatus: 'INVALIDATED', freshnessStatus: 'STALE' })
        .where(
          and(
            inArray(cachedFiles.roomId, scope.roomIds),
            eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
          ),
        )
        .returning();
      for (const file of invalidated) {
        await tx
          .insert(cacheDeletionJobs)
          .values({ cachedFileId: file.id, objectKey: file.objectKey })
          .onConflictDoNothing();
      }
    }

    const reservationScope = scope.deviceId
      ? eq(cacheUploadReservations.desktopDeviceId, scope.deviceId)
      : inArray(cacheUploadReservations.roomId, scope.roomIds);
    const reservations = await tx
      .update(cacheUploadReservations)
      .set({ status: 'CANCELLED' })
      .where(
        and(reservationScope, eq(cacheUploadReservations.status, 'RESERVED')),
      )
      .returning();
    for (const reservation of reservations) {
      await tx
        .insert(cacheReservationDeletionJobs)
        .values({
          reservationId: reservation.id,
          objectKey: reservation.objectKey,
          // A previously issued upload URL may remain valid until this time.
          // Deleting earlier could let a late upload recreate an orphan.
          nextAttemptAt: reservation.expiresAt,
        })
        .onConflictDoNothing();
    }

    const transferScope = scope.deviceId
      ? eq(fileTransfers.desktopDeviceId, scope.deviceId)
      : inArray(fileTransfers.roomId, scope.roomIds);
    const transfers = await tx
      .update(fileTransfers)
      .set({ status: 'CANCELLED', completedAt: new Date() })
      .where(
        and(
          transferScope,
          inArray(fileTransfers.status, ['REQUESTED', 'UPLOADING', 'READY']),
        ),
      )
      .returning();
    for (const transfer of transfers) {
      if (!transfer.objectKey) continue;
      await tx
        .insert(objectDeletionJobs)
        .values({
          transferId: transfer.id,
          objectKey: transfer.objectKey,
          nextAttemptAt: transfer.expiresAt,
        })
        .onConflictDoNothing();
    }

    const browseScope = scope.deviceId
      ? eq(fileBrowseRequests.desktopDeviceId, scope.deviceId)
      : inArray(fileBrowseRequests.roomId, scope.roomIds);
    await tx
      .update(fileBrowseRequests)
      .set({
        status: 'FAILED',
        failureCode: scope.browseFailureCode,
        query: null,
        resultPage: null,
      })
      .where(and(browseScope, eq(fileBrowseRequests.status, 'REQUESTED')));
    await tx
      .update(fileBrowseRequests)
      .set({ query: null, resultPage: null })
      .where(browseScope);
  }

  private async clearPresence(deviceId: string) {
    try {
      if (this.redis.status === 'wait') await this.redis.connect();
      await this.redis.del(`presence:${deviceId}`);
      await this.redis.zrem('presence:known', deviceId);
    } catch {
      console.error('DEVICE_PRESENCE_DELETE_FAILED');
    }
  }

  private async publishAndDisconnect(outcome: MutationOutcome) {
    try {
      await this.realtime.publishNow(outcome.eventIds);
    } catch {
      // The durable sync row remains unpublished for the dispatcher/replay path.
      console.error('CONNECTION_EVENT_IMMEDIATE_PUBLISH_FAILED');
    }
    if (outcome.disconnectDeviceId) {
      try {
        this.gateway.disconnectDevice(outcome.disconnectDeviceId);
      } catch {
        // The token is already revoked and the durable event will still make
        // clients converge even if the adapter rejects this immediate kick.
        console.error('DEVICE_SOCKET_DISCONNECT_FAILED');
      }
    }
  }

  private publicDevice(device: typeof devices.$inferSelect): StoredResult {
    const { userId: _, publicKey: __, ...safe } = device;
    void _;
    void __;
    return {
      ...safe,
      lastSeenAt: safe.lastSeenAt?.toISOString() ?? null,
      createdAt: safe.createdAt.toISOString(),
    };
  }

  private publicRoom(room: typeof rooms.$inferSelect): StoredResult {
    const { userId: _, ...safe } = room;
    void _;
    return { ...safe, createdAt: safe.createdAt.toISOString() };
  }
}

export function mobileConnectionActor(userId: string): ConnectionMutationActor {
  return { userId, actorDeviceId: null, actorScope: `USER:${userId}` };
}

export function agentConnectionActor(
  userId: string,
  deviceId: string,
): ConnectionMutationActor {
  return {
    userId,
    actorDeviceId: deviceId,
    actorScope: `DEVICE:${deviceId}`,
  };
}
