import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { RedisModule } from '../presence/redis.module';
import { RealtimeModule } from '../realtime/realtime.module';
import { SyncModule } from '../sync/sync.module';
import { AgentDevicesController } from './agent-devices.controller';
import { AgentRoomsController } from './agent-rooms.controller';
import { ConnectionLifecycleService } from './connection-lifecycle.service';

@Module({
  imports: [
    AuthModule,
    DatabaseModule,
    RedisModule,
    RealtimeModule,
    SyncModule,
  ],
  controllers: [AgentDevicesController, AgentRoomsController],
  providers: [ConnectionLifecycleService],
  exports: [ConnectionLifecycleService],
})
export class ConnectionsModule {}
