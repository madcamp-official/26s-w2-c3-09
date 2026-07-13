import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { AiModule } from '../ai/ai.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { RulesController } from './rules.controller';
import { RulesService } from './rules.service';

@Module({
  imports: [DatabaseModule, AuthModule, SyncModule, AiModule],
  controllers: [RulesController],
  providers: [RulesService],
})
export class RulesModule {}
