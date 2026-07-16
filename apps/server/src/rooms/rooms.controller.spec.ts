jest.mock('../auth/firebase-auth.guard', () => ({
  FirebaseAuthGuard: class FirebaseAuthGuard {},
}));
jest.mock('../connections/connection-lifecycle.service', () => ({
  ConnectionLifecycleService: class ConnectionLifecycleService {},
  mobileConnectionActor: jest.fn(),
}));
jest.mock('../realtime/realtime-dispatcher.service', () => ({
  RealtimeDispatcher: class RealtimeDispatcher {},
}));

import type { AuthPrincipal } from '../auth/auth-principal';
import { RoomsController } from './rooms.controller';

describe('RoomsController', () => {
  it('stores and immediately publishes a complete room.created projection', async () => {
    const deviceId = '33333333-3333-4333-8333-333333333333';
    const roomId = '22222222-2222-4222-8222-222222222222';
    const createdAt = new Date('2026-07-15T01:02:03.000Z');
    const device = {
      id: deviceId,
      userId: 'user-a',
      status: 'ACTIVE',
    };
    const created = {
      id: roomId,
      userId: 'user-a',
      desktopDeviceId: deviceId,
      name: 'Reports',
      rootAlias: 'reports',
      aiDocumentAnalysisConsent: false,
      status: 'ACTIVE',
      createdAt,
    };
    const tx = {
      select: jest.fn(() => ({
        from: () => ({
          where: () => ({
            for: () => ({ limit: async () => [device] }),
          }),
        }),
      })),
      insert: jest.fn(() => ({
        values: () => ({ returning: async () => [created] }),
      })),
    };
    const db = {
      transaction: jest.fn(
        async (work: (executor: typeof tx) => Promise<unknown>) => work(tx),
      ),
    };
    const sync = {
      append: jest.fn(async () => ({ id: 'event-a' })),
    };
    const realtime = { publishNow: jest.fn(async () => undefined) };
    const controller = new RoomsController(
      db as never,
      {} as never,
      sync as never,
      realtime as never,
    );
    const principal: AuthPrincipal = {
      userId: 'user-a',
      authProviderUid: 'provider-a',
      displayName: 'Desktop',
      authType: 'DEVICE',
      deviceId,
    };

    const response = await controller.create(principal, {
      desktopDeviceId: deviceId,
      name: 'Reports',
      rootAlias: 'reports',
    });

    expect(response).toMatchObject({
      id: roomId,
      status: 'ACTIVE',
      aiDocumentAnalysisConsent: false,
      createdAt: createdAt.toISOString(),
    });
    expect(sync.append).toHaveBeenCalledWith(
      tx,
      expect.objectContaining({
        eventType: 'room.created',
        aggregateId: roomId,
        payload: {
          roomId,
          status: 'ACTIVE',
          room: expect.objectContaining({
            id: roomId,
            name: 'Reports',
            aiDocumentAnalysisConsent: false,
          }),
        },
      }),
    );
    expect(realtime.publishNow).toHaveBeenCalledWith(['event-a']);
  });
});
