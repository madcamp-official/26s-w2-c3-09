import { Inject, Module, OnApplicationShutdown } from '@nestjs/common';
import Redis from 'ioredis';
import { loadEnvironment } from '../config/environment';

export const REDIS = Symbol('REDIS');
@Module({
  providers: [
    {
      provide: REDIS,
      useFactory: () =>
        new Redis(loadEnvironment().REDIS_URL, {
          lazyConnect: true,
          maxRetriesPerRequest: 2,
        }),
    },
  ],
  exports: [REDIS],
})
export class RedisModule implements OnApplicationShutdown {
  constructor(@Inject(REDIS) private readonly redis: Redis) {}
  async onApplicationShutdown() {
    if (this.redis.status === 'wait') {
      this.redis.disconnect(false);
    } else if (this.redis.status !== 'end') {
      await this.redis.quit();
    }
  }
}
