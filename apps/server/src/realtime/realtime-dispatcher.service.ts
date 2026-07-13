import {
  Inject,
  Injectable,
  OnApplicationBootstrap,
  OnApplicationShutdown,
} from '@nestjs/common';
import { syncEvents, type Database } from '@mousekeeper/database';
import { and, asc, eq, inArray, isNull } from 'drizzle-orm';
import { DATABASE } from '../database/database.module';
import { RealtimeGateway } from './realtime.gateway';
import { SyncService } from '../sync/sync.service';
import { toCharacterEvent } from './character-event';

@Injectable()
export class RealtimeDispatcher
  implements OnApplicationBootstrap, OnApplicationShutdown
{
  private timer?: NodeJS.Timeout;
  private running = false;
  private publicationTail: Promise<void> = Promise.resolve();

  constructor(
    @Inject(DATABASE) private readonly db: Database,
    private readonly gateway: RealtimeGateway,
    private readonly sync: SyncService,
  ) {}

  onApplicationBootstrap() {
    this.timer = setInterval(() => void this.flush(), 500);
  }

  onApplicationShutdown() {
    if (this.timer) {
      clearInterval(this.timer);
    }
  }

  async flush() {
    if (this.running || !this.gateway.isReady()) {
      return;
    }
    this.running = true;
    try {
      await this.serializePublication(async () => {
        const events = await this.db
          .select()
          .from(syncEvents)
          .where(isNull(syncEvents.publishedAt))
          .orderBy(asc(syncEvents.occurredAt))
          .limit(100);
        await this.publishRows(events);
      });
    } finally {
      this.running = false;
    }
  }

  async publishNow(eventIds: string[]) {
    if (!eventIds.length || !this.gateway.isReady()) return;
    await this.serializePublication(async () => {
      const events = await this.db
        .select()
        .from(syncEvents)
        .where(
          and(inArray(syncEvents.id, eventIds), isNull(syncEvents.publishedAt)),
        )
        .orderBy(asc(syncEvents.sequence));
      await this.publishRows(events);
    });
  }

  private serializePublication(work: () => Promise<void>) {
    // Immediate lifecycle publishing and the periodic outbox flush used to
    // read the same unpublished row concurrently. Keep one publisher in this
    // process; eventId/sequence remain the cross-process deduplication key.
    const next = this.publicationTail.then(work, work);
    this.publicationTail = next.catch(() => undefined);
    return next;
  }

  private async publishRows(events: (typeof syncEvents.$inferSelect)[]) {
    for (const event of events) {
      const envelope = this.sync.envelope(event);
      this.gateway.publish({
        eventType: event.eventType,
        userId: event.userId,
        deviceId: event.deviceId,
        payload: envelope,
      });
      const characterEvent = toCharacterEvent(envelope);
      if (characterEvent) {
        this.gateway.publish({
          eventType: 'character.event',
          userId: event.userId,
          deviceId: event.deviceId,
          payload: characterEvent,
        });
      }
      await this.db
        .update(syncEvents)
        .set({ publishedAt: new Date() })
        .where(
          and(eq(syncEvents.id, event.id), isNull(syncEvents.publishedAt)),
        );
    }
  }
}
