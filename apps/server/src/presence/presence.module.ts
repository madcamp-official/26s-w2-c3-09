import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { PresenceController } from './presence.controller';
import { RedisModule } from './redis.module';
import { RealtimeModule } from '../realtime/realtime.module';
import { PresenceMonitorService } from './presence-monitor.service';
@Module({
  imports: [DatabaseModule, AuthModule, RedisModule, RealtimeModule],
  controllers: [PresenceController],
  providers: [PresenceMonitorService],
})
export class PresenceModule {}
