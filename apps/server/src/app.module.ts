import { Module } from '@nestjs/common';
import { HealthModule } from './health/health.module';
import { AuditModule } from './audit/audit.module';
import { CommandsModule } from './commands/commands.module';
import { CharacterModule } from './character/character.module';
import { ChatModule } from './chat/chat.module';
import { DecisionsModule } from './decisions/decisions.module';
import { DevicesModule } from './devices/devices.module';
import { ExecutionsModule } from './executions/executions.module';
import { FileAccessModule } from './file-access/file-access.module';
import { PairingModule } from './pairing/pairing.module';
import { PresenceModule } from './presence/presence.module';
import { ProposalsModule } from './proposals/proposals.module';
import { RealtimeModule } from './realtime/realtime.module';
import { RoomsModule } from './rooms/rooms.module';
import { RulesModule } from './rules/rules.module';
import { SmartCacheModule } from './smart-cache/smart-cache.module';
import { SnapshotsModule } from './snapshots/snapshots.module';
import { SyncModule } from './sync/sync.module';
import { TransfersModule } from './transfers/transfers.module';
import { NotificationsModule } from './notifications/notifications.module';

@Module({
  imports: [
    AuditModule,
    CommandsModule,
    CharacterModule,
    ChatModule,
    DecisionsModule,
    DevicesModule,
    ExecutionsModule,
    FileAccessModule,
    HealthModule,
    NotificationsModule,
    PairingModule,
    PresenceModule,
    ProposalsModule,
    RealtimeModule,
    RoomsModule,
    RulesModule,
    SmartCacheModule,
    SnapshotsModule,
    SyncModule,
    TransfersModule,
  ],
})
export class AppModule {}
