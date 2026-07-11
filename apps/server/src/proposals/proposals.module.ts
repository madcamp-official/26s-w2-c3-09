import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { ProposalsController } from './proposals.controller';
import { ProposalsService } from './proposals.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule],
  controllers: [ProposalsController],
  providers: [ProposalsService],
})
export class ProposalsModule {}
