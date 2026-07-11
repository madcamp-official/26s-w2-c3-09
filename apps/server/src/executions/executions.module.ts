import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { AffinityModule } from '../affinity/affinity.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { ExecutionsController } from './executions.controller';
import { ExecutionsService } from './executions.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule, AffinityModule],
  controllers: [ExecutionsController],
  providers: [ExecutionsService],
})
export class ExecutionsModule {}
