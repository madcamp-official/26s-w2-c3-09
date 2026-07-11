import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { RedisModule } from '../presence/redis.module';
import { FileBrowseController } from './file-browse.controller';
import { FileBrowseService } from './file-browse.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule, RedisModule],
  controllers: [FileBrowseController],
  providers: [FileBrowseService],
})
export class FileAccessModule {}
