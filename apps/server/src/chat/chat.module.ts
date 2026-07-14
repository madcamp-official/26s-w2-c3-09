import { Module } from '@nestjs/common';
import { AiModule } from '../ai/ai.module';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { FileAccessModule } from '../file-access/file-access.module';
import { SyncModule } from '../sync/sync.module';
import { TransfersModule } from '../transfers/transfers.module';
import { ChatController } from './chat.controller';
import { ChatService } from './chat.service';
@Module({
  imports: [
    DatabaseModule,
    AuthModule,
    SyncModule,
    AiModule,
    FileAccessModule,
    TransfersModule,
  ],
  controllers: [ChatController],
  providers: [ChatService],
})
export class ChatModule {}
