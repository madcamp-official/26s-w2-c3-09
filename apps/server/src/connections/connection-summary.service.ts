import { Inject, Injectable } from '@nestjs/common';
import { connectionSummaryResponseSchema } from '@mousekeeper/contracts';
import { devices, rooms, type Database } from '@mousekeeper/database';
import { and, asc, eq } from 'drizzle-orm';
import { DATABASE } from '../database/database.module';

@Injectable()
export class ConnectionSummaryService {
  constructor(@Inject(DATABASE) private readonly db: Database) {}

  async summary(userId: string) {
    const [deviceRows, roomRows] = await Promise.all([
      this.db
        .select()
        .from(devices)
        .where(and(eq(devices.userId, userId), eq(devices.status, 'ACTIVE')))
        .orderBy(asc(devices.createdAt)),
      this.db
        .select()
        .from(rooms)
        .where(and(eq(rooms.userId, userId), eq(rooms.status, 'ACTIVE')))
        .orderBy(asc(rooms.createdAt)),
    ]);

    const activeDeviceIds = new Set(deviceRows.map((device) => device.id));
    return connectionSummaryResponseSchema.parse({
      devices: deviceRows.map((device) => ({
        id: device.id,
        platform: device.platform,
        deviceName: device.deviceName,
        status: device.status,
        lastSeenAt: device.lastSeenAt?.toISOString() ?? null,
        createdAt: device.createdAt.toISOString(),
      })),
      rooms: roomRows
        .filter((room) => activeDeviceIds.has(room.desktopDeviceId))
        .map((room) => ({
          id: room.id,
          desktopDeviceId: room.desktopDeviceId,
          name: room.name,
          rootAlias: room.rootAlias,
          status: room.status,
          createdAt: room.createdAt.toISOString(),
        })),
    });
  }
}
