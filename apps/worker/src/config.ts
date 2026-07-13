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
  });

export function loadWorkerConfig(source: NodeJS.ProcessEnv = process.env) {
  const result = schema.safeParse(source);
  if (!result.success)
    throw new Error(
      `UNCONFIGURED: ${result.error.issues.map((i) => i.path.join(".")).join(", ")}`,
    );
  return result.data;
}
