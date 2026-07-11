import {
  Body,
  Controller,
  Delete,
  Get,
  Inject,
  NotFoundException,
  Param,
  Post,
  UseGuards,
} from '@nestjs/common';
import { createRoomSchema } from '@housemouse/contracts';
import {
  auditEvents,
  cacheDeletionJobs,
  cacheReservationDeletionJobs,
  cachedFiles,
  cacheUploadReservations,
  devices,
  fileTransfers,
  objectDeletionJobs,
  rooms,
  type Database,
} from '@housemouse/database';
import { and, eq, inArray } from 'drizzle-orm';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';

@Controller('v1/rooms')
@UseGuards(FirebaseAuthGuard)
export class RoomsController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}
  @Get()
  async list(@CurrentPrincipal() p: AuthPrincipal) {
    const result = await this.db
      .select()
      .from(rooms)
      .where(and(eq(rooms.userId, p.userId), eq(rooms.status, 'ACTIVE')));
    return result.map((room) => this.publicRoom(room));
  }
  @Post()
  @AgentOnly()
  async create(
    @CurrentPrincipal() p: AuthPrincipal,
    @Body(new ZodValidationPipe(createRoomSchema))
    body: z.infer<typeof createRoomSchema>,
  ) {
    requireAgentDevice(p, body.desktopDeviceId);
    const device = (
      await this.db
        .select()
        .from(devices)
        .where(
          and(
            eq(devices.id, body.desktopDeviceId),
            eq(devices.userId, p.userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!device)
      throw new NotFoundException({
        code: 'NOT_FOUND',
        message: 'Desktop device not found',
      });
    const created = (
      await this.db
        .insert(rooms)
        .values({ userId: p.userId, ...body })
        .returning()
    )[0];
    return this.publicRoom(created);
  }

  @Delete(':id')
  @AgentOnly()
  async remove(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    return this.db.transaction(async (tx) => {
      const room = (
        await tx
          .select()
          .from(rooms)
          .where(
            and(
              eq(rooms.id, id),
              eq(rooms.userId, p.userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .limit(1)
      )[0];
      if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
      requireAgentDevice(p, room.desktopDeviceId);

      const files = await tx
        .select()
        .from(cachedFiles)
        .where(
          and(
            eq(cachedFiles.roomId, id),
            eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
          ),
        );
      for (const file of files) {
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
            eq(cacheUploadReservations.roomId, id),
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
            eq(fileTransfers.roomId, id),
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

      const removed = (
        await tx
          .update(rooms)
          .set({ status: 'REMOVED' })
          .where(eq(rooms.id, id))
          .returning()
      )[0]!;
      await tx.insert(auditEvents).values({
        userId: p.userId,
        deviceId: room.desktopDeviceId,
        roomId: id,
        eventType: 'room.removed',
        aggregateType: 'room',
        aggregateId: id,
        metadata: {},
      });
      await this.sync.append(tx, {
        userId: p.userId,
        deviceId: room.desktopDeviceId,
        roomId: id,
        eventType: 'room.removed',
        aggregateType: 'room',
        aggregateId: id,
        payload: { roomId: id },
      });
      return this.publicRoom(removed);
    });
  }

  private publicRoom(room: typeof rooms.$inferSelect) {
    const { userId: _, ...safe } = room;
    return safe;
  }
}
