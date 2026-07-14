import { ForbiddenException } from '@nestjs/common';
import type { AuthPrincipal } from '../auth/auth-principal';

jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

import { HomeController } from './home.controller';
import type { HomeService } from './home.service';

const firebasePrincipal: AuthPrincipal = {
  userId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
  authProviderUid: 'firebase-user',
  displayName: 'Mobile user',
  authType: 'FIREBASE',
  deviceId: null,
};

describe('HomeController', () => {
  it('returns the aggregate for a Firebase-authenticated user', async () => {
    const summary = { devices: [], rooms: [], character: {} };
    const summaryMock = jest.fn().mockResolvedValue(summary);
    const service = {
      summary: summaryMock,
    } as unknown as HomeService;
    const controller = new HomeController(service);

    await expect(controller.summary(firebasePrincipal)).resolves.toBe(summary);
    expect(summaryMock).toHaveBeenCalledWith(firebasePrincipal.userId);
  });

  it('rejects a desktop device token from the mobile aggregate', () => {
    const summaryMock = jest.fn();
    const service = {
      summary: summaryMock,
    } as unknown as HomeService;
    const controller = new HomeController(service);
    const devicePrincipal: AuthPrincipal = {
      ...firebasePrincipal,
      authType: 'DEVICE',
      deviceId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
    };

    expect(() => controller.summary(devicePrincipal)).toThrow(
      ForbiddenException,
    );
    expect(summaryMock).not.toHaveBeenCalled();
  });
});
