import { Module } from '@nestjs/common';
import { AiModule } from '../ai/ai.module';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { FileAccessModule } from '../file-access/file-access.module';
import { SyncModule } from '../sync/sync.module';
import { TransfersModule } from '../transfers/transfers.module';
import { RedisModule } from '../presence/redis.module';
import { ChatController } from './chat.controller';
import { ChatService } from './chat.service';
import { AgentRunsService } from './agent-runs.service';
@Module({
  imports: [
    DatabaseModule,
    AuthModule,
    SyncModule,
    AiModule,
    FileAccessModule,
    TransfersModule,
    RedisModule,
  ],
  controllers: [ChatController],
  providers: [ChatService, AgentRunsService],
  exports: [ChatService, AgentRunsService],
})
export class ChatModule {}
