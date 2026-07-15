import { readFileSync } from 'node:fs';
import { z } from 'zod';

const optionalString = z.preprocess(
  (value) => (value === '' ? undefined : value),
  z.string().min(1).optional(),
);
const optionalPositiveInteger = z.preprocess(
  (value) => (value === '' ? undefined : value),
  z.coerce.number().int().positive().optional(),
);
const aiProvider = z.preprocess(
  (value) => (value === '' || value === undefined ? 'unconfigured' : value),
  z.enum(['unconfigured', 'openai']),
);

const schema = z.object({
  NODE_ENV: z.enum(['development', 'test', 'production']),
  PORT: z.coerce.number().int().min(1).max(65535),
  SERVER_HOST: z.enum(['127.0.0.1', '0.0.0.0']),
  DATABASE_URL: z.url(),
  REDIS_URL: z.url(),
  WEB_ORIGIN: z.url(),
  SENTRY_DSN: z.preprocess(
    (value) => (value === '' ? undefined : value),
    z.url().optional(),
  ),
  FIREBASE_SERVICE_ACCOUNT_PATH: optionalString,
  FIREBASE_PROJECT_ID: optionalString,
  FIREBASE_CLIENT_EMAIL: optionalString,
  FIREBASE_PRIVATE_KEY: optionalString,
  AI_PROVIDER: aiProvider,
  AI_API_KEY: optionalString,
  AI_MODEL: optionalString,
  AI_CLASSIFIER_MODEL: optionalString,
  AI_AGENT_MODEL: optionalString,
  AI_TIMEOUT_MS: optionalPositiveInteger,
  AI_MAX_OUTPUT_TOKENS: optionalPositiveInteger,
  JWT_OR_DEVICE_TOKEN_SECRET: z.string().min(32),
  FILE_TRANSFER_MAX_BYTES: z.coerce.number().int().positive(),
  FILE_TRANSFER_TTL_SECONDS: z.coerce.number().int().min(60).max(3600),
  SMART_CACHE_ENABLED: z
    .enum(['true', 'false'])
    .transform((value) => value === 'true'),
  SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES: z.coerce.number().int().positive(),
  SMART_CACHE_DEFAULT_MAX_FILE_BYTES: z.coerce.number().int().positive(),
});

const serviceAccountSchema = z.object({
  type: z.literal('service_account'),
  project_id: z.string().min(1),
  client_email: z.email(),
  private_key: z.string().min(1),
});

function firebaseCredentials(environment: z.infer<typeof schema>) {
  if (environment.FIREBASE_SERVICE_ACCOUNT_PATH) {
    try {
      const account = serviceAccountSchema.parse(
        JSON.parse(
          readFileSync(environment.FIREBASE_SERVICE_ACCOUNT_PATH, 'utf8'),
        ),
      );
      return {
        FIREBASE_PROJECT_ID: account.project_id,
        FIREBASE_CLIENT_EMAIL: account.client_email,
        FIREBASE_PRIVATE_KEY: account.private_key,
      };
    } catch {
      throw new Error('UNCONFIGURED: FIREBASE_SERVICE_ACCOUNT_PATH');
    }
  }

  if (
    !environment.FIREBASE_PROJECT_ID ||
    !environment.FIREBASE_CLIENT_EMAIL ||
    !environment.FIREBASE_PRIVATE_KEY
  ) {
    throw new Error(
      'UNCONFIGURED: FIREBASE_SERVICE_ACCOUNT_PATH or ' +
        'FIREBASE_PROJECT_ID, FIREBASE_CLIENT_EMAIL, FIREBASE_PRIVATE_KEY',
    );
  }

  const direct = serviceAccountSchema
    .pick({
      project_id: true,
      client_email: true,
      private_key: true,
    })
    .safeParse({
      project_id: environment.FIREBASE_PROJECT_ID,
      client_email: environment.FIREBASE_CLIENT_EMAIL,
      private_key: environment.FIREBASE_PRIVATE_KEY,
    });
  if (!direct.success) {
    throw new Error(
      'UNCONFIGURED: FIREBASE_PROJECT_ID, FIREBASE_CLIENT_EMAIL, ' +
        'FIREBASE_PRIVATE_KEY',
    );
  }
  return {
    FIREBASE_PROJECT_ID: direct.data.project_id,
    FIREBASE_CLIENT_EMAIL: direct.data.client_email,
    FIREBASE_PRIVATE_KEY: direct.data.private_key,
  };
}

export function loadEnvironment(source: NodeJS.ProcessEnv = process.env) {
  const result = schema.safeParse(source);
  if (!result.success) {
    throw new Error(
      `UNCONFIGURED: ${result.error.issues.map((i) => i.path.join('.')).join(', ')}`,
    );
  }
  return { ...result.data, ...firebaseCredentials(result.data) };
}
