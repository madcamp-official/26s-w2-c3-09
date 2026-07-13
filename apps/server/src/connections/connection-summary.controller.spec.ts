import { ForbiddenException } from '@nestjs/common';
import type { AuthPrincipal } from '../auth/auth-principal';

jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

import { ConnectionSummaryController } from './connection-summary.controller';

describe('ConnectionSummaryController', () => {
  const firebasePrincipal: AuthPrincipal = {
    userId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
    authProviderUid: 'firebase-user',
    displayName: 'User',
    authType: 'FIREBASE',
    deviceId: null,
  };

  it('returns the lightweight connection summary for a Firebase user', async () => {
    const summary = { devices: [], rooms: [] };
    const service = { summary: jest.fn().mockResolvedValue(summary) };
    const controller = new ConnectionSummaryController(service as never);

    await expect(controller.summary(firebasePrincipal)).resolves.toBe(summary);
    expect(service.summary).toHaveBeenCalledWith(firebasePrincipal.userId);
  });

  it('rejects desktop device credentials on the mobile safety endpoint', () => {
    const service = { summary: jest.fn() };
    const controller = new ConnectionSummaryController(service as never);

    expect(() =>
      controller.summary({
        ...firebasePrincipal,
        deviceId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
        authType: 'DEVICE' as const,
      }),
    ).toThrow(ForbiddenException);
    expect(service.summary).not.toHaveBeenCalled();
  });
});
