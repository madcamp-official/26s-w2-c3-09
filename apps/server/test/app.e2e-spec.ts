import { Test } from '@nestjs/testing';
import {
  FastifyAdapter,
  NestFastifyApplication,
} from '@nestjs/platform-fastify';
import { HealthController } from '../src/health/health.controller';
import { DATABASE } from '../src/database/database.module';
import { REDIS } from '../src/presence/redis.module';

describe('health endpoint (e2e)', () => {
  let app: NestFastifyApplication;

  beforeAll(async () => {
    Object.assign(process.env, {
      NODE_ENV: 'test',
      PORT: '3000',
      DATABASE_URL: 'postgresql://test:test@localhost:5432/test',
      REDIS_URL: 'redis://localhost:6379',
      WEB_ORIGIN: 'http://localhost:3000',
      FIREBASE_PROJECT_ID: 'test-project',
      FIREBASE_CLIENT_EMAIL: 'test@example.com',
      FIREBASE_PRIVATE_KEY: 'test-key',
      JWT_OR_DEVICE_TOKEN_SECRET: 'test-device-token-secret-32-bytes-minimum',
      FILE_TRANSFER_MAX_BYTES: '104857600',
      FILE_TRANSFER_TTL_SECONDS: '600',
      SMART_CACHE_ENABLED: 'false',
      SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES: '524288000',
      SMART_CACHE_DEFAULT_MAX_FILE_BYTES: '52428800',
    });
    const module = await Test.createTestingModule({
      controllers: [HealthController],
      providers: [
        {
          provide: DATABASE,
          useValue: { execute: jest.fn().mockResolvedValue([]) },
        },
        {
          provide: REDIS,
          useValue: {
            status: 'ready',
            ping: jest.fn().mockResolvedValue('PONG'),
          },
        },
      ],
    }).compile();
    app = module.createNestApplication<NestFastifyApplication>(
      new FastifyAdapter(),
    );
    await app.init();
    await app.getHttpAdapter().getInstance().ready();
  });

  it('returns a real process health response', async () => {
    const response = await app.inject({ method: 'GET', url: '/health' });
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({ status: 'ok' });
  });

  it('returns ready only after dependency checks', async () => {
    const response = await app.inject({ method: 'GET', url: '/ready' });
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({ status: 'ready' });
  });

  afterAll(async () => {
    await app.close();
  });
});
