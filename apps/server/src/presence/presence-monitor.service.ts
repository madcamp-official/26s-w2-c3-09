import {
  Inject,
  Injectable,
  OnApplicationBootstrap,
  OnApplicationShutdown,
} from '@nestjs/common';
import { devices, type Database } from '@mousekeeper/database';
import { and, eq } from 'drizzle-orm';
import Redis from 'ioredis';
import { DATABASE } from '../database/database.module';
import { RealtimeGateway } from '../realtime/realtime.gateway';
import { REDIS } from './redis.module';

const PRESENCE_MONITOR_INTERVAL_MS = 1_000;

@Injectable()
export class PresenceMonitorService
  implements OnApplicationBootstrap, OnApplicationShutdown
{
  private timer?: NodeJS.Timeout;
  private running = false;

  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly realtime: RealtimeGateway,
  ) {}

  onApplicationBootstrap() {
    this.timer = setInterval(
      () => void this.expire(),
      PRESENCE_MONITOR_INTERVAL_MS,
    );
  }

  onApplicationShutdown() {
    if (this.timer) clearInterval(this.timer);
  }

  async expire() {
    if (this.running) return;
    this.running = true;
    try {
      if (this.redis.status === 'wait') await this.redis.connect();
      const expired = await this.redis.zrangebyscore(
        'presence:known',
        0,
        Date.now(),
      );
      for (const deviceId of expired) {
        if (await this.redis.exists(`presence:${deviceId}`)) {
          await this.redis.zadd(
            'presence:known',
            Date.now() + PRESENCE_MONITOR_INTERVAL_MS,
            deviceId,
          );
          continue;
        }
        const device = (
          await this.db
            .select()
            .from(devices)
            .where(and(eq(devices.id, deviceId), eq(devices.status, 'ACTIVE')))
            .limit(1)
        )[0];
        await this.redis.zrem('presence:known', deviceId);
        if (!device) continue;
        this.realtime.publish({
          eventType: 'presence.updated',
          userId: device.userId,
          deviceId,
          payload: { deviceId, presence: 'OFFLINE', ttlSeconds: 0 },
        });
      }
    } catch {
      console.error('PRESENCE_MONITOR_FAILED');
    } finally {
      this.running = false;
    }
  }
}
