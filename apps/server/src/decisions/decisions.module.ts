import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { AffinityModule } from '../affinity/affinity.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { DecisionsController } from './decisions.controller';
import { DecisionsService } from './decisions.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule, AffinityModule],
  controllers: [DecisionsController],
  providers: [DecisionsService],
})
export class DecisionsModule {}
