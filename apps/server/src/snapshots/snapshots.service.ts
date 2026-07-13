import {
  ConflictException,
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import { createRoomSnapshotSchema } from '@mousekeeper/contracts';
import {
  devices,
  roomSnapshots,
  rooms,
  type Database,
} from '@mousekeeper/database';
import { and, desc, eq, sql } from 'drizzle-orm';
import { z } from 'zod';
import { DATABASE } from '../database/database.module';
import { SyncService } from '../sync/sync.service';

@Injectable()
export class SnapshotsService {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly sync: SyncService,
  ) {}

  private async ownedRoom(userId: string, roomId: string) {
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
    if (!room) {
      throw new NotFoundException({
        code: 'NOT_FOUND',
        message: 'Room not found',
      });
    }
    return room;
  }

  async latest(userId: string, roomId: string) {
    await this.ownedRoom(userId, roomId);
    return (
      (
        await this.db
          .select()
          .from(roomSnapshots)
          .where(eq(roomSnapshots.roomId, roomId))
          .orderBy(desc(roomSnapshots.calculatedAt))
          .limit(1)
      )[0] ?? null
    );
  }

  async create(
    userId: string,
    deviceId: string,
    roomId: string,
    body: z.infer<typeof createRoomSnapshotSchema>,
  ) {
    return this.db.transaction(async (tx) => {
      await tx.execute(sql`select pg_advisory_xact_lock(hashtext(${roomId}))`);
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
              eq(rooms.userId, userId),
              eq(rooms.status, 'ACTIVE'),
            ),
          )
          .for('share')
          .limit(1)
      )[0];
      if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
      if (room.desktopDeviceId !== device.id) {
        throw new ForbiddenException({ code: 'FORBIDDEN' });
      }
      const calculatedAt = new Date(body.calculatedAt);
      const latest = (
        await tx
          .select({ calculatedAt: roomSnapshots.calculatedAt })
          .from(roomSnapshots)
          .where(eq(roomSnapshots.roomId, roomId))
          .orderBy(desc(roomSnapshots.calculatedAt))
          .limit(1)
      )[0];
      if (latest && calculatedAt <= latest.calculatedAt) {
        throw new ConflictException({
          code: 'VERSION_CONFLICT',
          message: 'Snapshot calculatedAt must advance monotonically',
        });
      }
      const created = (
        await tx
          .insert(roomSnapshots)
          .values({
            roomId,
            score: body.score,
            metrics: body.metrics,
            formulaVersion: body.formulaVersion,
            calculatedAt,
          })
          .returning()
      )[0];
      await this.sync.append(tx, {
        userId,
        deviceId,
        roomId,
        eventType: 'room.snapshot.updated',
        aggregateType: 'room_snapshot',
        aggregateId: created.id,
        payload: { snapshotId: created.id },
      });
      return created;
    });
  }
}
