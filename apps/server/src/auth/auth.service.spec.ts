import { UnauthorizedException } from '@nestjs/common';
import { verify } from 'jsonwebtoken';

import { AuthService } from './auth.service';

jest.mock('jsonwebtoken', () => ({ verify: jest.fn() }));
jest.mock('firebase-admin/app', () => ({
  cert: jest.fn(),
  getApps: jest.fn(() => []),
  initializeApp: jest.fn(),
}));
jest.mock('firebase-admin/auth', () => ({
  getAuth: jest.fn(),
}));
jest.mock('../config/environment', () => ({
  loadEnvironment: () => ({ JWT_OR_DEVICE_TOKEN_SECRET: 'test-only-secret' }),
}));

const deviceId = '018f4c7b-1ad6-7c95-bf34-5e45881f98a2';
const userId = '018f4c7b-1ad6-7c95-bf34-5e45881f98a1';

function databaseWithStatus(status: 'ACTIVE' | 'REVOKED' | null) {
  const row =
    status === null
      ? []
      : [
          {
            device: { id: deviceId, status },
            user: {
              id: userId,
              authProviderUid: 'firebase-user',
              displayName: 'User',
            },
          },
        ];
  const limit = jest.fn().mockResolvedValue(row);
  const where = jest.fn().mockReturnValue({ limit });
  const innerJoin = jest.fn().mockReturnValue({ where });
  const from = jest.fn().mockReturnValue({ innerJoin });
  const select = jest.fn().mockReturnValue({ from });
  return { database: { select }, select };
}

async function expectUnauthorizedCode(promise: Promise<unknown>, code: string) {
  try {
    await promise;
    throw new Error('Expected authentication to fail');
  } catch (error) {
    expect(error).toBeInstanceOf(UnauthorizedException);
    expect((error as UnauthorizedException).getResponse()).toMatchObject({
      code,
    });
  }
}

describe('AuthService desktop device status', () => {
  beforeEach(() => {
    jest.mocked(verify).mockReturnValue({ sub: deviceId } as never);
  });

  it('accepts only an ACTIVE device for normal agent requests', async () => {
    const { database } = databaseWithStatus('ACTIVE');
    const service = new AuthService(database as never);

    await expect(service.authenticateDevice('signed-token')).resolves.toEqual({
      userId,
      authProviderUid: 'firebase-user',
      displayName: 'User',
      authType: 'DEVICE',
      deviceId,
    });
  });

  it('returns DEVICE_REVOKED only for a valid token whose row is revoked', async () => {
    const { database } = databaseWithStatus('REVOKED');
    const service = new AuthService(database as never);

    await expectUnauthorizedCode(
      service.authenticateDevice('signed-token'),
      'DEVICE_REVOKED',
    );
    await expect(
      service.authenticateDeviceForRevocation('signed-token'),
    ).resolves.toMatchObject({ deviceId });
  });

  it('does not label an invalid signature or missing subject as revoked', async () => {
    const { database, select } = databaseWithStatus(null);
    const service = new AuthService(database as never);
    jest.mocked(verify).mockImplementationOnce(() => {
      throw new Error('bad signature');
    });

    await expectUnauthorizedCode(
      service.authenticateDevice('invalid-token'),
      'UNAUTHENTICATED',
    );
    expect(select).not.toHaveBeenCalled();

    jest.mocked(verify).mockReturnValueOnce({ sub: deviceId } as never);
    await expectUnauthorizedCode(
      service.authenticateDevice('unknown-device'),
      'UNAUTHENTICATED',
    );
  });
});
