import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { CommandsController } from './commands.controller';
import { CommandsService } from './commands.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule],
  controllers: [CommandsController],
  providers: [CommandsService],
})
export class CommandsModule {}
