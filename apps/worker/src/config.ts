import { z } from "zod";

const optionalString = z.preprocess(
  (value) => (value === "" ? undefined : value),
  z.string().min(1).optional(),
);

const schema = z
  .object({
    DATABASE_URL: z.url(),
    OBJECT_STORAGE_ENDPOINT: z.preprocess(
      (value) => (value === "" ? undefined : value),
      z.url().optional(),
    ),
    OBJECT_STORAGE_REGION: z.string().min(1),
    OBJECT_STORAGE_BUCKET: z.string().min(1),
    OBJECT_STORAGE_ACCESS_KEY_ID: optionalString,
    OBJECT_STORAGE_SECRET_ACCESS_KEY: optionalString,
    FILE_TRANSFER_TTL_SECONDS: z.coerce.number().int().min(60).max(3600),
    FCM_ENABLED: z
      .enum(["true", "false"])
      .transform((value) => value === "true"),
    FIREBASE_SERVICE_ACCOUNT_PATH: optionalString,
    FIREBASE_PROJECT_ID: optionalString,
    FIREBASE_CLIENT_EMAIL: optionalString,
    FIREBASE_PRIVATE_KEY: optionalString,
    SENTRY_DSN: z.preprocess(
      (value) => (value === "" ? undefined : value),
      z.url().optional(),
    ),
  })
  .superRefine((value, context) => {
    const hasAccessKey = Boolean(value.OBJECT_STORAGE_ACCESS_KEY_ID);
    const hasSecretKey = Boolean(value.OBJECT_STORAGE_SECRET_ACCESS_KEY);
    if (hasAccessKey !== hasSecretKey) {
      context.addIssue({
        code: "custom",
        path: [
          hasAccessKey
            ? "OBJECT_STORAGE_SECRET_ACCESS_KEY"
            : "OBJECT_STORAGE_ACCESS_KEY_ID",
        ],
        message:
          "object storage static credentials must be configured together",
      });
    }
    if (
      value.FCM_ENABLED &&
      !value.FIREBASE_SERVICE_ACCOUNT_PATH &&
      (!value.FIREBASE_PROJECT_ID ||
        !value.FIREBASE_CLIENT_EMAIL ||
        !value.FIREBASE_PRIVATE_KEY)
    ) {
      context.addIssue({
        code: "custom",
        path: ["FCM_ENABLED"],
        message:
          "FCM requires a service account path or complete direct credentials",
      });
    }
  });

export function loadWorkerConfig(source: NodeJS.ProcessEnv = process.env) {
  const result = schema.safeParse(source);
  if (!result.success)
    throw new Error(
      `UNCONFIGURED: ${result.error.issues.map((i) => i.path.join(".")).join(", ")}`,
    );
  return result.data;
}
