import { z } from 'zod';

const schema = z.object({
  NODE_ENV: z.enum(['development', 'test', 'production']),
  PORT: z.coerce.number().int().min(1).max(65535),
  DATABASE_URL: z.url(),
  REDIS_URL: z.url(),
  WEB_ORIGIN: z.url(),
  FIREBASE_PROJECT_ID: z.string().min(1),
  FIREBASE_CLIENT_EMAIL: z.email(),
  FIREBASE_PRIVATE_KEY: z.string().min(1),
  JWT_OR_DEVICE_TOKEN_SECRET: z.string().min(32),
  FILE_TRANSFER_MAX_BYTES: z.coerce.number().int().positive(),
  FILE_TRANSFER_TTL_SECONDS: z.coerce.number().int().min(60).max(3600),
  SMART_CACHE_ENABLED: z
    .enum(['true', 'false'])
    .transform((value) => value === 'true'),
  SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES: z.coerce.number().int().positive(),
  SMART_CACHE_DEFAULT_MAX_FILE_BYTES: z.coerce.number().int().positive(),
});

export function loadEnvironment(source: NodeJS.ProcessEnv = process.env) {
  const result = schema.safeParse(source);
  if (!result.success) {
    throw new Error(
      `UNCONFIGURED: ${result.error.issues.map((i) => i.path.join('.')).join(', ')}`,
    );
  }
  return result.data;
}
