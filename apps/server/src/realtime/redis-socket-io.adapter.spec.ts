import Redis from 'ioredis';
import type { Server } from 'socket.io';

const mockCreateAdapter = jest.fn((pubClient: Redis, subClient: Redis) => ({
  pubClient,
  subClient,
}));

jest.mock('@socket.io/redis-adapter', () => ({
  createAdapter: mockCreateAdapter,
}));

import {
  connectIfLazy,
  RedisSocketIoAdapter,
} from './redis-socket-io.adapter';

type FakeRedis = Redis & {
  status: Redis['status'];
  connect: jest.Mock<Promise<void>, []>;
  quit: jest.Mock<Promise<void>, []>;
  disconnect: jest.Mock<void, [boolean?]>;
  duplicate: jest.Mock<Redis, []>;
};

function redisClient(status: Redis['status']): FakeRedis {
  const client = {
    status,
    connect: jest.fn(async () => {
      client.status = 'ready';
    }),
    quit: jest.fn(async () => {
      client.status = 'end';
    }),
    disconnect: jest.fn((_: boolean = false) => {
      client.status = 'end';
    }),
    duplicate: jest.fn(),
  } as FakeRedis;
  return client;
}

describe('RedisSocketIoAdapter', () => {
  beforeEach(() => {
    mockCreateAdapter.mockClear();
  });

  it('connects dedicated Redis pub/sub clients and installs the Socket.IO adapter', async () => {
    const pubClient = redisClient('wait');
    const subClient = redisClient('ready');
    const baseRedis = redisClient('ready');
    baseRedis.duplicate
      .mockReturnValueOnce(pubClient)
      .mockReturnValueOnce(subClient);

    const adapter = await RedisSocketIoAdapter.create(
      { getHttpServer: jest.fn() } as never,
      baseRedis,
    );

    expect(baseRedis.duplicate).toHaveBeenCalledTimes(2);
    expect(pubClient.connect).toHaveBeenCalledTimes(1);
    expect(subClient.connect).not.toHaveBeenCalled();
    expect(mockCreateAdapter).toHaveBeenCalledWith(pubClient, subClient);

    const server = { adapter: jest.fn() } as unknown as Pick<
      Server,
      'adapter'
    >;
    adapter.installAdapter(server);

    expect(server.adapter).toHaveBeenCalledWith(
      mockCreateAdapter.mock.results[0].value,
    );
  });

  it('closes only adapter-owned pub/sub clients once', async () => {
    const pubClient = redisClient('wait');
    const subClient = redisClient('ready');
    const baseRedis = redisClient('ready');
    baseRedis.duplicate
      .mockReturnValueOnce(pubClient)
      .mockReturnValueOnce(subClient);

    const adapter = await RedisSocketIoAdapter.create(
      { getHttpServer: jest.fn() } as never,
      baseRedis,
    );
    await adapter.closeRedisClients();
    await adapter.closeRedisClients();

    expect(pubClient.quit).toHaveBeenCalledTimes(1);
    expect(subClient.quit).toHaveBeenCalledTimes(1);
    expect(baseRedis.quit).not.toHaveBeenCalled();
  });

  it('cleans up duplicated clients when Redis adapter bootstrap fails', async () => {
    const pubClient = redisClient('wait');
    const subClient = redisClient('wait');
    subClient.connect.mockRejectedValueOnce(new Error('redis unavailable'));
    const baseRedis = redisClient('ready');
    baseRedis.duplicate
      .mockReturnValueOnce(pubClient)
      .mockReturnValueOnce(subClient);

    await expect(
      RedisSocketIoAdapter.create(
        { getHttpServer: jest.fn() } as never,
        baseRedis,
      ),
    ).rejects.toThrow('redis unavailable');

    expect(pubClient.quit).toHaveBeenCalledTimes(1);
    expect(subClient.disconnect).toHaveBeenCalledWith(false);
    expect(baseRedis.quit).not.toHaveBeenCalled();
  });

  it('leaves already connected Redis clients alone', async () => {
    const client = redisClient('ready');

    await connectIfLazy(client);

    expect(client.connect).not.toHaveBeenCalled();
  });
});
