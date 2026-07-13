import { Injectable } from '@nestjs/common';
import {
  notificationJobs,
  syncEvents,
  type Database,
} from '@housemouse/database';
import { and, asc, eq, gt, max, sql } from 'drizzle-orm';
import { notificationForEvent } from '../notifications/notification-event';

type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];

@Injectable()
export class SyncService {
  async append(
    tx: Transaction,
    input: {
      userId: string;
      deviceId: string | null;
      roomId: string | null;
      eventType: string;
      schemaVersion?: number;
      correlationId?: string;
      aggregateType: string;
      aggregateId: string;
      payload: Record<string, unknown>;
    },
  ) {
    await tx.execute(
      sql`select pg_advisory_xact_lock(hashtext(${input.userId}))`,
    );
    const latest =
      (
        await tx
          .select({ value: max(syncEvents.sequence) })
          .from(syncEvents)
          .where(eq(syncEvents.userId, input.userId))
      )[0]?.value ?? 0;
    const event = (
      await tx
        .insert(syncEvents)
        .values({
          ...input,
          schemaVersion: input.schemaVersion ?? 1,
          correlationId: input.correlationId ?? input.aggregateId,
          sequence: Number(latest) + 1,
        })
        .returning()
    )[0];
    const notification = notificationForEvent(input.eventType, input.payload);
    if (notification) {
      await tx
        .insert(notificationJobs)
        .values({
          userId: input.userId,
          syncEventId: event.id,
          eventType: input.eventType,
          title: notification.title,
          body: notification.body,
        })
        .onConflictDoNothing();
    }
    return event;
  }

  async replay(db: Database, userId: string, after: number, limit: number) {
    const events = await db
      .select()
      .from(syncEvents)
      .where(and(eq(syncEvents.userId, userId), gt(syncEvents.sequence, after)))
      .orderBy(asc(syncEvents.sequence))
      .limit(limit);
    return events.map((event) => this.envelope(event));
  }

  envelope(event: typeof syncEvents.$inferSelect) {
    if (
      typeof event.payload !== 'object' ||
      event.payload === null ||
      Array.isArray(event.payload)
    )
      throw new Error('INVALID_EVENT_PAYLOAD');
    return {
      eventId: event.id,
      eventType: event.eventType,
      schemaVersion: event.schemaVersion,
      correlationId: event.correlationId,
      aggregateType: event.aggregateType,
      aggregateId: event.aggregateId,
      deviceId: event.deviceId,
      roomId: event.roomId,
      sequence: event.sequence,
      occurredAt: event.occurredAt.toISOString(),
      payload: event.payload as Record<string, unknown>,
    };
  }
}
