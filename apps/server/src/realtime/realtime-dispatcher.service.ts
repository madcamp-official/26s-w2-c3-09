import {
  Inject,
  Injectable,
  OnApplicationBootstrap,
  OnApplicationShutdown,
} from '@nestjs/common';
import { syncEvents, type Database } from '@housemouse/database';
import { asc, eq, isNull } from 'drizzle-orm';
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
    if (this.running || !this.gateway.server) {
      return;
    }
    this.running = true;
    try {
      const events = await this.db
        .select()
        .from(syncEvents)
        .where(isNull(syncEvents.publishedAt))
        .orderBy(asc(syncEvents.occurredAt))
        .limit(100);
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
          .where(eq(syncEvents.id, event.id));
      }
    } finally {
      this.running = false;
    }
  }
}
