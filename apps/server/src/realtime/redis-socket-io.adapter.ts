import type { INestApplicationContext } from '@nestjs/common';
import { IoAdapter } from '@nestjs/platform-socket.io';
import { createAdapter } from '@socket.io/redis-adapter';
import Redis from 'ioredis';
import type { Server, ServerOptions } from 'socket.io';

type RedisAdapterFactory = ReturnType<typeof createAdapter>;

export class RedisSocketIoAdapter extends IoAdapter {
  private adapterFactory?: RedisAdapterFactory;
  private readonly pubSubClients: Redis[] = [];
  private closed = false;

  private constructor(
    app: INestApplicationContext,
    private readonly baseRedis: Redis,
  ) {
    super(app);
  }

  static async create(app: INestApplicationContext, baseRedis: Redis) {
    const adapter = new RedisSocketIoAdapter(app, baseRedis);
    await adapter.connectPubSubClients();
    return adapter;
  }

  private async connectPubSubClients() {
    const pubClient = this.baseRedis.duplicate();
    const subClient = this.baseRedis.duplicate();
    this.pubSubClients.push(pubClient, subClient);
    try {
      await Promise.all([
        connectIfLazy(pubClient),
        connectIfLazy(subClient),
      ]);
      this.adapterFactory = createAdapter(pubClient, subClient);
    } catch (error) {
      await this.closeRedisClients();
      throw error;
    }
  }

  override createIOServer(port: number, options?: ServerOptions): Server {
    const server = super.createIOServer(port, options) as Server;
    this.installAdapter(server);
    return server;
  }

  installAdapter(server: Pick<Server, 'adapter'>) {
    if (!this.adapterFactory) {
      throw new Error('UNCONFIGURED: Socket.IO Redis adapter is not connected');
    }
    server.adapter(this.adapterFactory);
  }

  override async close(server: Server) {
    await super.close(server);
    await this.closeRedisClients();
  }

  async closeRedisClients() {
    if (this.closed) return;
    this.closed = true;
    await Promise.allSettled(
      this.pubSubClients.map((client) => closeRedisClient(client)),
    );
    this.pubSubClients.length = 0;
  }
}

export async function connectIfLazy(client: Redis) {
  if (client.status === 'wait') {
    await client.connect();
  }
}

async function closeRedisClient(client: Redis) {
  if (client.status === 'wait') {
    client.disconnect(false);
    return;
  }
  if (client.status !== 'end') {
    await client.quit();
  }
}
