import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { loadEnvironment } from './environment';

const baseEnvironment = {
  NODE_ENV: 'test',
  PORT: '3000',
  SERVER_HOST: '127.0.0.1',
  DATABASE_URL: 'postgresql://test:test@localhost:5432/test',
  REDIS_URL: 'redis://localhost:6379',
  WEB_ORIGIN: 'http://localhost:5173',
  JWT_OR_DEVICE_TOKEN_SECRET: 'a'.repeat(32),
  FILE_TRANSFER_MAX_BYTES: '1048576',
  FILE_TRANSFER_TTL_SECONDS: '600',
  SMART_CACHE_ENABLED: 'false',
  SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES: '10485760',
  SMART_CACHE_DEFAULT_MAX_FILE_BYTES: '1048576',
};

describe('loadEnvironment', () => {
  it('fails fast when required values are absent', () => {
    expect(() => loadEnvironment({})).toThrow('UNCONFIGURED');
  });

  it('accepts only explicit loopback or container bind hosts', () => {
    expect(() =>
      loadEnvironment({
        ...baseEnvironment,
        SERVER_HOST: '192.0.2.10',
        FIREBASE_PROJECT_ID: 'test-project',
        FIREBASE_CLIENT_EMAIL: 'firebase-admin@example.com',
        FIREBASE_PRIVATE_KEY: 'test-private-key',
      }),
    ).toThrow('UNCONFIGURED: SERVER_HOST');
  });

  it('loads Firebase credentials from an external service account file', () => {
    const directory = mkdtempSync(join(tmpdir(), 'mousekeeper-firebase-'));
    const path = join(directory, 'service-account.json');
    writeFileSync(
      path,
      JSON.stringify({
        type: 'service_account',
        project_id: 'test-project',
        client_email: 'firebase-admin@example.com',
        private_key: 'test-private-key',
      }),
    );

    try {
      const environment = loadEnvironment({
        ...baseEnvironment,
        FIREBASE_SERVICE_ACCOUNT_PATH: path,
      });
      expect(environment.FIREBASE_PROJECT_ID).toBe('test-project');
      expect(environment.FIREBASE_CLIENT_EMAIL).toBe(
        'firebase-admin@example.com',
      );
      expect(environment.FIREBASE_PRIVATE_KEY).toBe('test-private-key');
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('keeps direct Firebase environment variables for deployments', () => {
    const environment = loadEnvironment({
      ...baseEnvironment,
      FIREBASE_PROJECT_ID: 'render-project',
      FIREBASE_CLIENT_EMAIL: 'firebase-admin@example.com',
      FIREBASE_PRIVATE_KEY: 'escaped-private-key',
    });
    expect(environment.FIREBASE_PROJECT_ID).toBe('render-project');
  });

  it('defaults AI to an explicit unconfigured provider', () => {
    const environment = loadEnvironment({
      ...baseEnvironment,
      FIREBASE_PROJECT_ID: 'test-project',
      FIREBASE_CLIENT_EMAIL: 'firebase-admin@example.com',
      FIREBASE_PRIVATE_KEY: 'test-private-key',
    });
    expect(environment.AI_PROVIDER).toBe('unconfigured');
  });

  it('rejects unknown AI provider names instead of guessing', () => {
    expect(() =>
      loadEnvironment({
        ...baseEnvironment,
        FIREBASE_PROJECT_ID: 'test-project',
        FIREBASE_CLIENT_EMAIL: 'firebase-admin@example.com',
        FIREBASE_PRIVATE_KEY: 'test-private-key',
        AI_PROVIDER: 'some-random-provider',
      }),
    ).toThrow('UNCONFIGURED: AI_PROVIDER');
  });
});
