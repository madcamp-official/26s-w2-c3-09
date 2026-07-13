import { UnauthorizedException } from '@nestjs/common';

jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

import { AgentDevicesController } from './agent-devices.controller';

describe('AgentDevicesController', () => {
  const principal = {
    userId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
    deviceId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
    authProviderUid: 'desktop-device',
    displayName: 'Desktop',
    authType: 'DEVICE' as const,
  };

  it('authenticates the self token and delegates a device-scoped retry key', async () => {
    const auth = {
      authenticateDeviceForRevocation: jest.fn().mockResolvedValue(principal),
    };
    const lifecycle = {
      revokeDevice: jest.fn().mockResolvedValue({
        id: principal.deviceId,
        status: 'REVOKED',
      }),
    };
    const controller = new AgentDevicesController(
      auth as never,
      lifecycle as never,
    );

    await expect(
      controller.revokeSelf(
        'Bearer mk_device_signed-token',
        'disconnect-attempt-1',
      ),
    ).resolves.toMatchObject({ status: 'REVOKED' });
    expect(auth.authenticateDeviceForRevocation).toHaveBeenCalledWith(
      'signed-token',
    );
    expect(lifecycle.revokeDevice).toHaveBeenCalledWith(
      {
        userId: principal.userId,
        actorDeviceId: principal.deviceId,
        actorScope: `DEVICE:${principal.deviceId}`,
      },
      principal.deviceId,
      'disconnect-attempt-1',
    );
  });

  it('does not accept a Firebase bearer token on the agent-only route', async () => {
    const controller = new AgentDevicesController({} as never, {} as never);
    await expect(
      controller.revokeSelf('Bearer firebase-token', 'disconnect-attempt-1'),
    ).rejects.toBeInstanceOf(UnauthorizedException);
  });
});
