import { createHmac } from 'node:crypto';
import type { FastifyInstance } from 'fastify';
import Redis from 'ioredis';

const WINDOW_SECONDS = 60;
const PAIRING_STATUS_PATH = /^\/v1\/pairing-sessions\/[^/]+\/status\/?$/;
const HEARTBEAT_PATH = /^\/v1\/devices\/[^/]+\/heartbeat\/?$/;
const DEVICE_AGENT_PATH =
  /^\/v1\/devices\/[^/]+\/(commands|agent-tools|decisions|file-browse-requests|file-transfers)(\/|$)/;
const ROOM_SNAPSHOT_PATH = /^\/v1\/rooms\/[^/]+\/snapshots\/?$/;

function isPairingStatusRequest(method: string, path: string) {
  return method.toUpperCase() === 'GET' && PAIRING_STATUS_PATH.test(path);
}

export function requestScope(path: string, method = 'GET') {
  if (isPairingStatusRequest(method, path)) return 'pairing-status';
  if (path.startsWith('/v1/pairing-sessions')) return 'pairing';
  if (path.includes('/file-transfers')) return 'transfers';
  if (HEARTBEAT_PATH.test(path) || path === '/v1/sync/events')
    return 'realtime';
  if (
    path.startsWith('/v1/agent/') ||
    DEVICE_AGENT_PATH.test(path) ||
    ROOM_SNAPSHOT_PATH.test(path)
  ) {
    return 'agent';
  }
  if (
    path.includes('/chat-sessions') ||
    /^\/v1\/rooms\/[^/]+\/(chat|commands)(\/|$)/.test(path)
  ) {
    return 'chat';
  }
  return 'api';
}

export function requestLimit(path: string, method = 'GET') {
  switch (requestScope(path, method)) {
    case 'pairing-status':
      return 60;
    case 'pairing':
      return 10;
    case 'transfers':
      return 30;
    case 'realtime':
    case 'agent':
      return 600;
    case 'chat':
      return 300;
    default:
      return 120;
  }
}

export function rateLimitKey(
  secret: string,
  ip: string,
  path: string,
  window: number,
  method = 'GET',
) {
  const subject = createHmac('sha256', secret).update(ip).digest('hex');
  const scope = requestScope(path, method);
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
