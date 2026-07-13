import type { Server, Socket } from 'socket.io';

jest.mock('../auth/auth.service', () => ({
  AuthService: class AuthService {},
}));

import { RealtimeGateway } from './realtime.gateway';

describe('RealtimeGateway contract', () => {
  it('authenticates a device token and joins user/device rooms', async () => {
    const authenticateDevice = jest.fn().mockResolvedValue({
      userId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      deviceId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      authType: 'DEVICE',
    });
    const gateway = new RealtimeGateway({ authenticateDevice } as never);
    const join = jest.fn().mockResolvedValue(undefined);
    const disconnect = jest.fn();
    const client = {
      handshake: { auth: { token: 'mk_device_signed-token' } },
      join,
      disconnect,
      data: {},
    } as unknown as Socket;

    await gateway.handleConnection(client);

    expect(authenticateDevice).toHaveBeenCalledWith('signed-token');
    expect(join).toHaveBeenCalledWith(
      'user:018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
    );
    expect(join).toHaveBeenCalledWith(
      'device:018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
    );
    expect(disconnect).not.toHaveBeenCalled();
  });

  it('publishes once to the user room so device sockets do not receive duplicates', () => {
    const gateway = new RealtimeGateway({} as never);
    const emit = jest.fn();
    const to = jest.fn().mockReturnValue({ emit });
    gateway.server = { to } as unknown as Server;

    gateway.publish({
      eventType: 'command.updated',
      userId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      deviceId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
      payload: { eventId: '018f4c7b-1ad6-7c95-bf34-5e45881f98a3' },
    });

    expect(to).toHaveBeenCalledTimes(1);
    expect(to).toHaveBeenCalledWith(
      'user:018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
    );
    expect(emit).toHaveBeenCalledWith('command.updated', expect.any(Object));
  });

  it('force-disconnects every socket bound to a revoked device', () => {
    const gateway = new RealtimeGateway({} as never);
    const disconnectSockets = jest.fn();
    const inRoom = jest.fn().mockReturnValue({ disconnectSockets });
    gateway.server = { in: inRoom } as unknown as Server;

    gateway.disconnectDevice('018f4c7b-1ad6-7c95-bf34-5e45881f98a2');

    expect(inRoom).toHaveBeenCalledWith(
      'device:018f4c7b-1ad6-7c95-bf34-5e45881f98a2',
    );
    expect(disconnectSockets).toHaveBeenCalledWith(true);
  });
});
