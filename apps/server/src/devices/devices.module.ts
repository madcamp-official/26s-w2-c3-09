import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { ConnectionsModule } from '../connections/connections.module';
import { DevicesController } from './devices.controller';
@Module({
  imports: [DatabaseModule, AuthModule, ConnectionsModule],
  controllers: [DevicesController],
})
export class DevicesModule {}
