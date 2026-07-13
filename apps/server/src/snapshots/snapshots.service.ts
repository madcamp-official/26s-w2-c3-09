import {
  ForbiddenException,
  Inject,
  Injectable,
  NotFoundException,
} from '@nestjs/common';
import { createRoomSnapshotSchema } from '@mousekeeper/contracts';
import { roomSnapshots, rooms, type Database } from '@mousekeeper/database';
import { and, desc, eq } from 'drizzle-orm';
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
    const room = await this.ownedRoom(userId, roomId);
    if (room.desktopDeviceId !== deviceId) {
      throw new ForbiddenException({ code: 'FORBIDDEN' });
    }
    return this.db.transaction(async (tx) => {
      const created = (
        await tx
          .insert(roomSnapshots)
          .values({
            roomId,
            score: body.score,
            metrics: body.metrics,
            calculatedAt: new Date(body.calculatedAt),
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
        payload: { snapshotId: created.id, roomId, score: created.score },
      });
      return created;
    });
  }
}
