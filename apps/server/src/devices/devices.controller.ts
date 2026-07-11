import {
  Controller,
  Delete,
  Get,
  Inject,
  NotFoundException,
  Param,
  UseGuards,
} from '@nestjs/common';
import {
  cacheDeletionJobs,
  cacheReservationDeletionJobs,
  cacheUploadReservations,
  cachedFiles,
  devices,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  auditEvents,
  type Database,
} from '@housemouse/database';
import { and, eq, inArray } from 'drizzle-orm';
import Redis from 'ioredis';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';
import { SyncService } from '../sync/sync.service';

@Controller('v1/devices')
@UseGuards(FirebaseAuthGuard)
export class DevicesController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly sync: SyncService,
  ) {}
  @Get()
  async list(@CurrentPrincipal() p: AuthPrincipal) {
    const result = await this.db
      .select()
      .from(devices)
      .where(and(eq(devices.userId, p.userId), eq(devices.status, 'ACTIVE')));
    return result.map((device) => this.publicDevice(device));
  }
  @Delete(':id')
  async revoke(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    const revoked = await this.db.transaction(async (tx) => {
      const result = (
        await tx
          .update(devices)
          .set({ status: 'REVOKED' })
          .where(
            and(
              eq(devices.id, id),
              eq(devices.userId, p.userId),
              eq(devices.status, 'ACTIVE'),
            ),
          )
          .returning()
      )[0];
      if (!result) throw new NotFoundException({ code: 'NOT_FOUND' });
      const files = await tx
        .select({ file: cachedFiles })
        .from(cachedFiles)
        .innerJoin(
          rooms,
          and(eq(cachedFiles.roomId, rooms.id), eq(rooms.desktopDeviceId, id)),
        )
        .where(eq(cachedFiles.availabilityStatus, 'AVAILABLE'));
      for (const { file } of files) {
        await tx
          .update(cachedFiles)
          .set({ availabilityStatus: 'INVALIDATED', freshnessStatus: 'STALE' })
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
            eq(cacheUploadReservations.desktopDeviceId, id),
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
      const transfers = await tx
        .select()
        .from(fileTransfers)
        .where(
          and(
            eq(fileTransfers.desktopDeviceId, id),
            inArray(fileTransfers.status, ['REQUESTED', 'UPLOADING', 'READY']),
          ),
        );
      for (const transfer of transfers) {
        await tx
          .update(fileTransfers)
          .set({ status: 'CANCELLED', completedAt: new Date() })
          .where(eq(fileTransfers.id, transfer.id));
        if (transfer.objectKey) {
          await tx
            .insert(objectDeletionJobs)
            .values({ transferId: transfer.id, objectKey: transfer.objectKey })
            .onConflictDoNothing();
        }
      }
      await tx
        .update(rooms)
        .set({ status: 'REMOVED' })
        .where(and(eq(rooms.desktopDeviceId, id), eq(rooms.status, 'ACTIVE')));
      await tx.insert(auditEvents).values({
        userId: p.userId,
        deviceId: id,
        eventType: 'device.revoked',
        aggregateType: 'device',
        aggregateId: id,
        metadata: {},
      });
      await this.sync.append(tx, {
        userId: p.userId,
        deviceId: id,
        roomId: null,
        eventType: 'device.revoked',
        aggregateType: 'device',
        aggregateId: id,
        payload: { deviceId: id },
      });
      return result;
    });
    try {
      if (this.redis.status === 'wait') await this.redis.connect();
      await this.redis.del(`presence:${id}`);
      await this.redis.zrem('presence:known', id);
    } catch {
      console.error('DEVICE_PRESENCE_DELETE_FAILED');
    }
    return this.publicDevice(revoked);
  }

  private publicDevice(device: typeof devices.$inferSelect) {
    const { userId: _, publicKey: __, ...safe } = device;
    return safe;
  }
}
