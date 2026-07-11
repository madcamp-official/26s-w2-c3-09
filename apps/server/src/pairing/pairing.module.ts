import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { SyncModule } from '../sync/sync.module';
import { PairingController } from './pairing.controller';
import { PairingService } from './pairing.service';
@Module({
  imports: [DatabaseModule, AuthModule, SyncModule],
  controllers: [PairingController],
  providers: [PairingService],
})
export class PairingModule {}
