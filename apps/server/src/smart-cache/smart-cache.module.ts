import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { ObjectStorageService } from '../transfers/object-storage.service';
import { RedisModule } from '../presence/redis.module';
import { SyncModule } from '../sync/sync.module';
import { SmartCacheController } from './smart-cache.controller';
import { SmartCacheService } from './smart-cache.service';
@Module({
  imports: [DatabaseModule, AuthModule, RedisModule, SyncModule],
  controllers: [SmartCacheController],
  providers: [SmartCacheService, ObjectStorageService],
})
export class SmartCacheModule {}
