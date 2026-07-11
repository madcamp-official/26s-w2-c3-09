import {
  Controller,
  Get,
  Inject,
  ServiceUnavailableException,
} from '@nestjs/common';
import type { Database } from '@housemouse/database';
import { sql } from 'drizzle-orm';
import Redis from 'ioredis';
import { DATABASE } from '../database/database.module';
import { REDIS } from '../presence/redis.module';

@Controller()
export class HealthController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
  ) {}

  @Get('health')
  check() {
    return { status: 'ok' as const };
  }

  @Get('ready')
  async ready() {
    try {
      await this.db.execute(sql`select 1`);
      if (this.redis.status === 'wait') await this.redis.connect();
      if ((await this.redis.ping()) !== 'PONG') throw new Error('Redis ping');
      return { status: 'ready' as const };
    } catch {
      throw new ServiceUnavailableException({
        code: 'DEPENDENCY_UNAVAILABLE',
      });
    }
  }
}
