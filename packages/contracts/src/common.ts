import { z } from "zod";

export const uuidSchema = z.uuid();
export const idempotencyKeySchema = z.string().min(8).max(128);
export const relativePathSchema = z
  .string()
  .min(1)
  .max(1024)
  .refine((value) => {
    const segments = value.split(/[\\/]/);
    return (
      !value.includes("\0") &&
      !value.startsWith("/") &&
      !value.startsWith("\\") &&
      !/^[A-Za-z]:/.test(value) &&
      segments.every(
        (segment) => segment.length > 0 && segment !== "." && segment !== "..",
      )
    );
  }, "Only managed-root-relative paths are allowed");

export const errorCodeSchema = z.enum([
  "UNAUTHENTICATED",
  "FORBIDDEN",
  "NOT_FOUND",
  "CONFLICT",
  "VALIDATION_FAILED",
  "INVALID_STATE_TRANSITION",
  "UNCONFIGURED",
  "DEVICE_OFFLINE",
  "TIMED_OUT",
  "CURSOR_INVALIDATED",
  "OUTSIDE_MANAGED_ROOT",
  "SOURCE_NOT_FOUND",
  "SOURCE_CHANGED",
  "CHECKSUM_MISMATCH",
  "SIZE_LIMIT_EXCEEDED",
  "IDEMPOTENCY_CONFLICT",
  "VERSION_CONFLICT",
  "RATE_LIMITED",
  "REJECTED_POLICY",
  "RESERVATION_EXPIRED",
  "UPLOAD_FAILED",
  "CHECKSUM_UNAVAILABLE",
  "DEPENDENCY_UNAVAILABLE",
  "FEATURE_LOCKED",
  "INTERNAL_ERROR",
]);

export const eventEnvelopeSchema = z
  .object({
    eventId: uuidSchema,
    eventType: z.string().min(1).max(100),
    schemaVersion: z.number().int().positive(),
    correlationId: uuidSchema,
    aggregateType: z.string().min(1).max(50),
    aggregateId: uuidSchema,
    deviceId: uuidSchema.nullable(),
    roomId: uuidSchema.nullable(),
    sequence: z.number().int().nonnegative(),
    occurredAt: z.iso.datetime(),
    payload: z.record(z.string(), z.unknown()),
  })
  .strict();

export type EventEnvelope = z.infer<typeof eventEnvelopeSchema>;
