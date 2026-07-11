import { ForbiddenException } from '@nestjs/common';
import type { AuthPrincipal } from './auth-principal';
export function requireAgentDevice(
  principal: AuthPrincipal,
  expectedDeviceId?: string,
) {
  if (
    principal.authType !== 'DEVICE' ||
    !principal.deviceId ||
    (expectedDeviceId && principal.deviceId !== expectedDeviceId)
  ) {
    throw new ForbiddenException({
      code: 'FORBIDDEN',
      message: 'Desktop device identity mismatch',
    });
  }
  return principal.deviceId;
}
