import { createAiProvider } from './ai.module';
import { OpenAiResponsesProvider } from './openai-responses.provider';
import { UnconfiguredAiProvider } from './unconfigured-ai.provider';

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
  FIREBASE_PROJECT_ID: 'test-project',
  FIREBASE_CLIENT_EMAIL: 'firebase-admin@example.com',
  FIREBASE_PRIVATE_KEY: 'test-private-key',
};

describe('createAiProvider', () => {
  it('defaults to the explicit UNCONFIGURED provider', () => {
    expect(createAiProvider(baseEnvironment)).toBeInstanceOf(
      UnconfiguredAiProvider,
    );
  });

  it('uses OpenAI Responses only when provider, key, and model are configured', () => {
    expect(
      createAiProvider({
        ...baseEnvironment,
        AI_PROVIDER: 'openai',
        AI_API_KEY: 'test-openai-key',
        AI_MODEL: 'gpt-test',
      }),
    ).toBeInstanceOf(OpenAiResponsesProvider);
  });

  it('keeps incomplete OpenAI configuration UNCONFIGURED', () => {
    expect(
      createAiProvider({
        ...baseEnvironment,
        AI_PROVIDER: 'openai',
        AI_API_KEY: 'test-openai-key',
      }),
    ).toBeInstanceOf(UnconfiguredAiProvider);
  });
});
