import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { RealtimeDispatcher } from './realtime-dispatcher.service';
import { RealtimeGateway } from './realtime.gateway';
import { SyncModule } from '../sync/sync.module';

@Module({
  imports: [DatabaseModule, AuthModule, SyncModule],
  providers: [RealtimeGateway, RealtimeDispatcher],
  exports: [RealtimeGateway],
})
export class RealtimeModule {}
