import { Inject, Injectable, NotFoundException } from '@nestjs/common';
import { auditEvents, rooms, type Database } from '@housemouse/database';
import { and, desc, eq } from 'drizzle-orm';
import { DATABASE } from '../database/database.module';

@Injectable()
export class AuditService {
  constructor(@Inject(DATABASE) private readonly db: Database) {}

  async forRoom(userId: string, roomId: string, limit: number) {
    const room = (
      await this.db
        .select({ id: rooms.id })
        .from(rooms)
        .where(and(eq(rooms.id, roomId), eq(rooms.userId, userId)))
        .limit(1)
    )[0];
    if (!room) throw new NotFoundException({ code: 'NOT_FOUND' });
    const events = await this.db
      .select()
      .from(auditEvents)
      .where(
        and(eq(auditEvents.userId, userId), eq(auditEvents.roomId, roomId)),
      )
      .orderBy(desc(auditEvents.occurredAt))
      .limit(limit);
    return events.map(({ userId: _, ...event }) => event);
  }
}
