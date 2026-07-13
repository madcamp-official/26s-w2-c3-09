import { createHmac } from 'node:crypto';
import type { FastifyInstance } from 'fastify';
import Redis from 'ioredis';

const WINDOW_SECONDS = 60;
const PAIRING_STATUS_PATH = /^\/v1\/pairing-sessions\/[^/]+\/status\/?$/;

function isPairingStatusRequest(method: string, path: string) {
  return method.toUpperCase() === 'GET' && PAIRING_STATUS_PATH.test(path);
}

export function requestLimit(path: string, method = 'GET') {
  if (isPairingStatusRequest(method, path)) return 60;
  if (path.startsWith('/v1/pairing-sessions')) return 10;
  if (path.includes('/file-transfers')) return 30;
  return 120;
}

export function rateLimitKey(
  secret: string,
  ip: string,
  path: string,
  window: number,
  method = 'GET',
) {
  const subject = createHmac('sha256', secret).update(ip).digest('hex');
  const scope = isPairingStatusRequest(method, path)
    ? 'pairing-status'
    : path.startsWith('/v1/pairing-sessions')
      ? 'pairing'
      : path.includes('/file-transfers')
        ? 'transfers'
        : 'api';
  return `rate-limit:${scope}:${window}:${subject}`;
}

export async function registerRateLimit(
  server: FastifyInstance,
  redis: Redis,
  secret: string,
) {
  if (redis.status === 'wait') await redis.connect();
  server.addHook('onRequest', async (request, reply) => {
    const path = request.url.split('?', 1)[0] ?? request.url;
    if (path === '/health') return;
    const limit = requestLimit(path, request.method);
    const window = Math.floor(Date.now() / (WINDOW_SECONDS * 1000));
    const key = rateLimitKey(secret, request.ip, path, window, request.method);
    const result = await redis
      .multi()
      .incr(key)
      .expire(key, WINDOW_SECONDS + 1)
      .exec();
    const count = Number(result?.[0]?.[1] ?? limit + 1);
    reply.header('X-RateLimit-Limit', limit);
    reply.header('X-RateLimit-Remaining', Math.max(0, limit - count));
    if (count > limit) {
      reply.header('Retry-After', WINDOW_SECONDS);
      await reply.code(429).send({ code: 'RATE_LIMITED' });
    }
  });
}
