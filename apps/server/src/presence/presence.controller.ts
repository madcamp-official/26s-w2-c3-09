import {
  Body,
  Controller,
  Get,
  Inject,
  NotFoundException,
  Param,
  Post,
  UseGuards,
} from '@nestjs/common';
import { heartbeatSchema } from '@housemouse/contracts';
import { devices, type Database } from '@housemouse/database';
import { and, eq } from 'drizzle-orm';
import Redis from 'ioredis';
import { z } from 'zod';
import { CurrentPrincipal } from '../auth/auth-principal';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { DATABASE } from '../database/database.module';
import { REDIS } from './redis.module';
import { RealtimeGateway } from '../realtime/realtime.gateway';

@Controller('v1/devices')
@UseGuards(FirebaseAuthGuard)
export class PresenceController {
  constructor(
    @Inject(DATABASE) private readonly db: Database,
    @Inject(REDIS) private readonly redis: Redis,
    private readonly realtime: RealtimeGateway,
  ) {}
  private async owned(userId: string, deviceId: string) {
    const device = (
      await this.db
        .select()
        .from(devices)
        .where(
          and(
            eq(devices.id, deviceId),
            eq(devices.userId, userId),
            eq(devices.status, 'ACTIVE'),
          ),
        )
        .limit(1)
    )[0];
    if (!device) throw new NotFoundException({ code: 'NOT_FOUND' });
    return device;
  }
  @Post(':id/heartbeat')
  @AgentOnly()
  async heartbeat(
    @CurrentPrincipal() p: AuthPrincipal,
    @Param('id') id: string,
    @Body(new ZodValidationPipe(heartbeatSchema))
    body: z.infer<typeof heartbeatSchema>,
  ) {
    requireAgentDevice(p, id);
    await this.owned(p.userId, id);
    if (this.redis.status === 'wait') await this.redis.connect();
    await this.redis
      .multi()
      .set(`presence:${id}`, body.presence, 'EX', 45)
      .zadd('presence:known', Date.now() + 45_000, id)
      .exec();
    await this.db
      .update(devices)
      .set({ lastSeenAt: new Date() })
      .where(eq(devices.id, id));
    const response = { deviceId: id, presence: body.presence, ttlSeconds: 45 };
    this.realtime.publish({
      eventType: 'presence.updated',
      userId: p.userId,
      deviceId: id,
      payload: response,
    });
    return response;
  }
  @Get(':id/presence')
  async get(@CurrentPrincipal() p: AuthPrincipal, @Param('id') id: string) {
    await this.owned(p.userId, id);
    if (this.redis.status === 'wait') await this.redis.connect();
    return {
      deviceId: id,
      presence: (await this.redis.get(`presence:${id}`)) ?? 'OFFLINE',
    };
  }
}
