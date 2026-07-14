import { BadRequestException } from '@nestjs/common';
import { CommandsController } from './commands.controller';
import type { CommandsService } from './commands.service';
import type { AuthPrincipal } from '../auth/auth-principal';

jest.mock('firebase-admin/app', () => ({
  cert: jest.fn(),
  getApps: jest.fn(() => []),
  initializeApp: jest.fn(),
}));
jest.mock('firebase-admin/auth', () => ({
  getAuth: jest.fn(() => ({ verifyIdToken: jest.fn() })),
}));
jest.mock('jsonwebtoken', () => ({ verify: jest.fn() }));

describe('CommandsController', () => {
  const principal: AuthPrincipal = {
    userId: 'user-1',
    authProviderUid: 'firebase-user-1',
    displayName: 'User',
    authType: 'FIREBASE',
    deviceId: null,
  };

  it('rejects command metadata idempotency that differs from the header', () => {
    const service = {
      create: jest.fn(),
    } as unknown as CommandsService;
    const controller = new CommandsController(service);

    expect(() =>
      controller.create(principal, 'room-1', 'header-key-123', {
        intent: 'RENAME',
        payload: {
          rootId: 'root:downloads',
          sourceRelativePath: 'reports/old.pdf',
          newName: 'final.pdf',
        },
        metadata: {
          idempotencyKey: 'different-key-123',
          requiresApproval: true,
        },
      }),
    ).toThrow(BadRequestException);
    expect(service.create).not.toHaveBeenCalled();
  });

  it('passes matching command metadata through to the service', () => {
    const service = {
      create: jest.fn().mockReturnValue({ id: 'command-1' }),
    } as unknown as CommandsService;
    const controller = new CommandsController(service);
    const body = {
      intent: 'RENAME' as const,
      payload: {
        rootId: 'root:downloads',
        sourceRelativePath: 'reports/old.pdf',
        newName: 'final.pdf',
      },
      metadata: {
        idempotencyKey: 'header-key-123',
        requiresApproval: true,
      },
    };

    expect(
      controller.create(principal, 'room-1', 'header-key-123', body),
    ).toEqual({ id: 'command-1' });
    expect(service.create).toHaveBeenCalledWith(
      'user-1',
      'room-1',
      'header-key-123',
      body,
    );
  });
});
