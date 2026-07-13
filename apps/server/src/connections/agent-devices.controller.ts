import {
  Controller,
  Delete,
  Headers,
  UnauthorizedException,
} from '@nestjs/common';
import { requireAgentDevice } from '../auth/agent-device';
import { AuthService } from '../auth/auth.service';
import {
  agentConnectionActor,
  ConnectionLifecycleService,
} from './connection-lifecycle.service';
import { requireIdempotencyKey } from './idempotency-key';

@Controller('v1/agent/devices')
export class AgentDevicesController {
  constructor(
    private readonly auth: AuthService,
    private readonly lifecycle: ConnectionLifecycleService,
  ) {}

  @Delete('self')
  async revokeSelf(
    @Headers('authorization') authorization: string | undefined,
    @Headers('idempotency-key') rawKey: string | undefined,
  ) {
    const prefix = 'Bearer mk_device_';
    if (!authorization?.startsWith(prefix)) {
      throw new UnauthorizedException({ code: 'UNAUTHENTICATED' });
    }
    // This verifier permits a revoked device only on this endpoint so a lost
    // first response can replay its durable idempotency receipt safely.
    const principal = await this.auth.authenticateDeviceForRevocation(
      authorization.slice(prefix.length),
    );
    const deviceId = requireAgentDevice(principal);
    return this.lifecycle.revokeDevice(
      agentConnectionActor(principal.userId, deviceId),
      deviceId,
      requireIdempotencyKey(rawKey),
    );
  }
}
