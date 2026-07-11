import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { RedisModule } from '../presence/redis.module';
import { SyncModule } from '../sync/sync.module';
import { DevicesController } from './devices.controller';
@Module({
  imports: [DatabaseModule, AuthModule, RedisModule, SyncModule],
  controllers: [DevicesController],
})
export class DevicesModule {}
