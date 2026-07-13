import { Controller, Delete, Headers, Param, UseGuards } from '@nestjs/common';
import { AgentOnly } from '../auth/agent-only.decorator';
import { requireAgentDevice } from '../auth/agent-device';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import {
  agentConnectionActor,
  ConnectionLifecycleService,
} from './connection-lifecycle.service';
import { requireIdempotencyKey } from './idempotency-key';

@Controller('v1/agent/rooms')
@UseGuards(FirebaseAuthGuard)
export class AgentRoomsController {
  constructor(private readonly lifecycle: ConnectionLifecycleService) {}

  @Delete(':id')
  @AgentOnly()
  remove(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('id') roomId: string,
    @Headers('idempotency-key') rawKey: string | undefined,
  ) {
    const deviceId = requireAgentDevice(principal);
    return this.lifecycle.removeRoom(
      agentConnectionActor(principal.userId, deviceId),
      roomId,
      requireIdempotencyKey(rawKey),
    );
  }
}
