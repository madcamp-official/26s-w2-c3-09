import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { ConnectionsModule } from '../connections/connections.module';
import { RealtimeModule } from '../realtime/realtime.module';
import { SyncModule } from '../sync/sync.module';
import { RoomsController } from './rooms.controller';
@Module({
  imports: [
    DatabaseModule,
    AuthModule,
    ConnectionsModule,
    RealtimeModule,
    SyncModule,
  ],
  controllers: [RoomsController],
})
export class RoomsModule {}
