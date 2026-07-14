import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { RedisModule } from '../presence/redis.module';
import { ObjectStorageService } from './object-storage.service';
import { TransfersController } from './transfers.controller';
import { TransfersService } from './transfers.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule, RedisModule],
  controllers: [TransfersController],
  providers: [TransfersService, ObjectStorageService],
  exports: [TransfersService],
})
export class TransfersModule {}
