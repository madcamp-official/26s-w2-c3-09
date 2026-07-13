import { readFileSync } from "node:fs";
import {
  DeleteObjectCommand,
  ListObjectsV2Command,
  S3Client,
} from "@aws-sdk/client-s3";
import {
  cacheDeletionJobs,
  cacheReservationDeletionJobs,
  cacheUploadReservations,
  createDatabase,
  fileTransfers,
  objectDeletionJobs,
  notificationJobs,
  pushNotificationTokens,
  rooms,
  syncEvents,
  type Database,
} from "@housemouse/database";
import { and, asc, eq, inArray, lte, max, or, sql } from "drizzle-orm";
import { cert, getApps, initializeApp } from "firebase-admin/app";
import { getMessaging, type Messaging } from "firebase-admin/messaging";
import { loadWorkerConfig } from "./config";
import { isPermanentlyInvalidTokenError } from "./notification-delivery";

type Transaction = Parameters<Parameters<Database["transaction"]>[0]>[0];

const config = loadWorkerConfig();
const connection = createDatabase(config.DATABASE_URL);
const storage = new S3Client({
  region: config.OBJECT_STORAGE_REGION,
  ...(config.OBJECT_STORAGE_ENDPOINT
    ? {
        endpoint: config.OBJECT_STORAGE_ENDPOINT,
        forcePathStyle: true,
      }
    : {}),
  ...(config.OBJECT_STORAGE_ACCESS_KEY_ID &&
  config.OBJECT_STORAGE_SECRET_ACCESS_KEY
    ? {
        credentials: {
          accessKeyId: config.OBJECT_STORAGE_ACCESS_KEY_ID,
          secretAccessKey: config.OBJECT_STORAGE_SECRET_ACCESS_KEY,
        },
      }
    : {}),
});

function createMessaging(): Messaging | null {
  if (!config.FCM_ENABLED) return null;
  try {
    const credential = config.FIREBASE_SERVICE_ACCOUNT_PATH
      ? JSON.parse(readFileSync(config.FIREBASE_SERVICE_ACCOUNT_PATH, "utf8"))
      : {
          projectId: config.FIREBASE_PROJECT_ID,
          clientEmail: config.FIREBASE_CLIENT_EMAIL,
          privateKey: config.FIREBASE_PRIVATE_KEY?.replace(/\\n/g, "\n"),
        };
    const app = getApps()[0] ?? initializeApp({ credential: cert(credential) });
    return getMessaging(app);
  } catch {
    throw new Error("UNCONFIGURED: FIREBASE_SERVICE_ACCOUNT_PATH");
  }
}

const messaging = createMessaging();

async function deleteObject(key: string) {
  await storage.send(
    new DeleteObjectCommand({ Bucket: config.OBJECT_STORAGE_BUCKET, Key: key }),
  );
}

async function appendSyncEvent(
  tx: Transaction,
  input: {
    userId: string;
    deviceId: string | null;
    roomId: string | null;
    aggregateId: string;
    eventType: string;
    aggregateType: string;
    payload: Record<string, unknown>;
  },
) {
  await tx.execute(
    sql`select pg_advisory_xact_lock(hashtext(${input.userId}))`,
  );
  const latest =
    (
      await tx
        .select({ value: max(syncEvents.sequence) })
        .from(syncEvents)
        .where(eq(syncEvents.userId, input.userId))
    )[0]?.value ?? 0;
  await tx.insert(syncEvents).values({
    userId: input.userId,
    deviceId: input.deviceId,
    roomId: input.roomId,
    sequence: Number(latest) + 1,
    eventType: input.eventType,
    schemaVersion: 1,
    correlationId: input.aggregateId,
    aggregateType: input.aggregateType,
    aggregateId: input.aggregateId,
    payload: input.payload,
  });
}

async function scheduleExpired() {
  const transfers = await connection.db
    .select()
    .from(fileTransfers)
    .where(
      and(
        lte(fileTransfers.expiresAt, new Date()),
        inArray(fileTransfers.status, ["REQUESTED", "UPLOADING", "READY"]),
      ),
    );
  for (const transfer of transfers) {
    await connection.db.transaction(async (tx) => {
      const expired = (
        await tx
          .update(fileTransfers)
          .set({ status: "EXPIRED", completedAt: new Date() })
          .where(
            and(
              eq(fileTransfers.id, transfer.id),
              inArray(fileTransfers.status, [
                "REQUESTED",
                "UPLOADING",
                "READY",
              ]),
            ),
          )
          .returning()
      )[0];
      if (!expired) return;
      if (expired.objectKey) {
        await tx
          .insert(objectDeletionJobs)
          .values({ transferId: expired.id, objectKey: expired.objectKey })
          .onConflictDoNothing();
      }
      await appendSyncEvent(tx, {
        userId: expired.requestedByUserId,
        deviceId: expired.desktopDeviceId,
        roomId: expired.roomId,
        aggregateId: expired.id,
        eventType: "file.transfer.updated",
        aggregateType: "file_transfer",
        payload: { transferId: expired.id, status: expired.status },
      });
    });
  }

  const reservations = await connection.db
    .select({ reservation: cacheUploadReservations, userId: rooms.userId })
    .from(cacheUploadReservations)
    .innerJoin(rooms, eq(cacheUploadReservations.roomId, rooms.id))
    .where(
      and(
        lte(cacheUploadReservations.expiresAt, new Date()),
        eq(cacheUploadReservations.status, "RESERVED"),
      ),
    );
  for (const item of reservations) {
    const reservation = item.reservation;
    await connection.db.transaction(async (tx) => {
      const expired = (
        await tx
          .update(cacheUploadReservations)
          .set({ status: "EXPIRED" })
          .where(
            and(
              eq(cacheUploadReservations.id, reservation.id),
              eq(cacheUploadReservations.status, "RESERVED"),
            ),
          )
          .returning()
      )[0];
      if (!expired) return;
      await tx
        .insert(cacheReservationDeletionJobs)
        .values({ reservationId: expired.id, objectKey: expired.objectKey })
        .onConflictDoNothing();
      await appendSyncEvent(tx, {
        userId: item.userId,
        deviceId: expired.desktopDeviceId,
        roomId: expired.roomId,
        aggregateId: expired.id,
        eventType: "smart-cache.updated",
        aggregateType: "cache_upload_reservation",
        payload: { reservationId: expired.id, status: expired.status },
      });
    });
  }
}

async function claimTransferJob() {
  return connection.db.transaction(async (tx) => {
    const row = (
      await tx
        .select()
        .from(objectDeletionJobs)
        .where(
          and(
            eq(objectDeletionJobs.status, "PENDING"),
            lte(objectDeletionJobs.nextAttemptAt, new Date()),
          ),
        )
        .for("update", { skipLocked: true })
        .limit(1)
    )[0];
    if (row) {
      await tx
        .update(objectDeletionJobs)
        .set({
          status: "PROCESSING",
          attemptCount: sql`${objectDeletionJobs.attemptCount} + 1`,
        })
        .where(eq(objectDeletionJobs.id, row.id));
    }
    return row;
  });
}

async function processTransferJob() {
  const job = await claimTransferJob();
  if (!job) return false;
  try {
    await deleteObject(job.objectKey);
    await connection.db
      .update(objectDeletionJobs)
      .set({
        status: "COMPLETED",
        completedAt: new Date(),
        lastErrorCode: null,
      })
      .where(eq(objectDeletionJobs.id, job.id));
  } catch {
    await connection.db
      .update(objectDeletionJobs)
      .set({
        status: "PENDING",
        nextAttemptAt: new Date(Date.now() + 60_000),
        lastErrorCode: "OBJECT_DELETE_FAILED",
      })
      .where(eq(objectDeletionJobs.id, job.id));
  }
  return true;
}

async function processCacheFileJob() {
  const job = await connection.db.transaction(async (tx) => {
    const row = (
      await tx
        .select()
        .from(cacheDeletionJobs)
        .where(
          and(
            eq(cacheDeletionJobs.status, "PENDING"),
            lte(cacheDeletionJobs.nextAttemptAt, new Date()),
          ),
        )
        .for("update", { skipLocked: true })
        .limit(1)
    )[0];
    if (row) {
      await tx
        .update(cacheDeletionJobs)
        .set({
          status: "PROCESSING",
          attemptCount: sql`${cacheDeletionJobs.attemptCount} + 1`,
        })
        .where(eq(cacheDeletionJobs.id, row.id));
    }
    return row;
  });
  if (!job) return false;
  try {
    await deleteObject(job.objectKey);
    await connection.db
      .update(cacheDeletionJobs)
      .set({
        status: "COMPLETED",
        completedAt: new Date(),
        lastErrorCode: null,
      })
      .where(eq(cacheDeletionJobs.id, job.id));
  } catch {
    await connection.db
      .update(cacheDeletionJobs)
      .set({
        status: "PENDING",
        nextAttemptAt: new Date(Date.now() + 60_000),
        lastErrorCode: "CACHE_DELETE_FAILED",
      })
      .where(eq(cacheDeletionJobs.id, job.id));
  }
  return true;
}

async function processReservationJob() {
  const job = await connection.db.transaction(async (tx) => {
    const row = (
      await tx
        .select()
        .from(cacheReservationDeletionJobs)
        .where(
          and(
            eq(cacheReservationDeletionJobs.status, "PENDING"),
            lte(cacheReservationDeletionJobs.nextAttemptAt, new Date()),
          ),
        )
        .for("update", { skipLocked: true })
        .limit(1)
    )[0];
    if (row) {
      await tx
        .update(cacheReservationDeletionJobs)
        .set({
          status: "PROCESSING",
          attemptCount: sql`${cacheReservationDeletionJobs.attemptCount} + 1`,
        })
        .where(eq(cacheReservationDeletionJobs.id, row.id));
    }
    return row;
  });
  if (!job) return false;
  try {
    await deleteObject(job.objectKey);
    await connection.db
      .update(cacheReservationDeletionJobs)
      .set({
        status: "COMPLETED",
        completedAt: new Date(),
        lastErrorCode: null,
      })
      .where(eq(cacheReservationDeletionJobs.id, job.id));
  } catch {
    await connection.db
      .update(cacheReservationDeletionJobs)
      .set({
        status: "PENDING",
        nextAttemptAt: new Date(Date.now() + 60_000),
        lastErrorCode: "RESERVATION_DELETE_FAILED",
      })
      .where(eq(cacheReservationDeletionJobs.id, job.id));
  }
  return true;
}

async function claimNotificationJob() {
  const now = new Date();
  return connection.db.transaction(async (tx) => {
    const row = (
      await tx
        .select()
        .from(notificationJobs)
        .where(
          and(
            lte(notificationJobs.nextAttemptAt, now),
            or(
              eq(notificationJobs.status, "PENDING"),
              eq(notificationJobs.status, "PROCESSING"),
            ),
          ),
        )
        .orderBy(asc(notificationJobs.nextAttemptAt))
        .for("update", { skipLocked: true })
        .limit(1)
    )[0];
    if (row) {
      await tx
        .update(notificationJobs)
        .set({
          status: "PROCESSING",
          attemptCount: sql`${notificationJobs.attemptCount} + 1`,
          // nextAttemptAt doubles as a lease so a crashed worker cannot strand a job.
          nextAttemptAt: new Date(Date.now() + 5 * 60_000),
        })
        .where(eq(notificationJobs.id, row.id));
    }
    return row;
  });
}

async function completeNotificationJob(
  jobId: string,
  lastErrorCode: string | null,
) {
  await connection.db
    .update(notificationJobs)
    .set({
      status: "COMPLETED",
      completedAt: new Date(),
      lastErrorCode,
    })
    .where(eq(notificationJobs.id, jobId));
}

async function processNotificationJob() {
  if (!messaging) return false;
  const job = await claimNotificationJob();
  if (!job) return false;
  const tokens = await connection.db
    .select()
    .from(pushNotificationTokens)
    .where(
      and(
        eq(pushNotificationTokens.userId, job.userId),
        eq(pushNotificationTokens.status, "ACTIVE"),
      ),
    )
    .orderBy(asc(pushNotificationTokens.createdAt));
  if (tokens.length === 0) {
    await completeNotificationJob(job.id, "NO_ACTIVE_TOKENS");
    return true;
  }

  let retryRequired = false;
  for (let offset = 0; offset < tokens.length; offset += 500) {
    const batch = tokens.slice(offset, offset + 500);
    try {
      const response = await messaging.sendEachForMulticast({
        tokens: batch.map((token) => token.token),
        notification: { title: job.title, body: job.body },
        data: { eventType: job.eventType, syncEventId: job.syncEventId },
      });
      const invalidIds: string[] = [];
      response.responses.forEach((result, index) => {
        if (result.success) return;
        if (isPermanentlyInvalidTokenError(result.error)) {
          invalidIds.push(batch[index].id);
        } else {
          retryRequired = true;
        }
      });
      if (invalidIds.length > 0) {
        await connection.db
          .update(pushNotificationTokens)
          .set({ status: "REVOKED", revokedAt: new Date() })
          .where(inArray(pushNotificationTokens.id, invalidIds));
      }
    } catch {
      retryRequired = true;
    }
  }

  if (retryRequired) {
    await connection.db
      .update(notificationJobs)
      .set({
        status: "PENDING",
        nextAttemptAt: new Date(Date.now() + 60_000),
        lastErrorCode: "FCM_SEND_FAILED",
      })
      .where(eq(notificationJobs.id, job.id));
  } else {
    await completeNotificationJob(job.id, null);
  }
  return true;
}

async function sweepOrphanTransfers() {
  const cutoff = Date.now() - config.FILE_TRANSFER_TTL_SECONDS * 2 * 1000;
  let continuationToken: string | undefined;
  do {
    const page = await storage.send(
      new ListObjectsV2Command({
        Bucket: config.OBJECT_STORAGE_BUCKET,
        Prefix: "transfers/",
        ContinuationToken: continuationToken,
      }),
    );
    for (const object of page.Contents ?? []) {
      if (
        !object.Key ||
        !object.LastModified ||
        object.LastModified.getTime() > cutoff
      )
        continue;
      const referenced = (
        await connection.db
          .select({ id: fileTransfers.id })
          .from(fileTransfers)
          .where(eq(fileTransfers.objectKey, object.Key))
          .limit(1)
      )[0];
      if (!referenced) await deleteObject(object.Key);
    }
    continuationToken = page.IsTruncated
      ? page.NextContinuationToken
      : undefined;
  } while (continuationToken);
}

let running = false;
let lastOrphanSweepAt = 0;

async function tick() {
  if (running) return;
  running = true;
  try {
    await scheduleExpired();
    while (await processTransferJob()) {}
    while (await processCacheFileJob()) {}
    while (await processReservationJob()) {}
    while (await processNotificationJob()) {}
    if (Date.now() - lastOrphanSweepAt >= 60 * 60 * 1000) {
      await sweepOrphanTransfers();
      lastOrphanSweepAt = Date.now();
    }
  } catch {
    console.error("WORKER_TICK_FAILED");
  } finally {
    running = false;
  }
}

async function start() {
  await tick();
  const timer = setInterval(() => void tick(), 30_000);

  async function shutdown() {
    clearInterval(timer);
    await connection.close();
    process.exit(0);
  }

  process.on("SIGINT", () => void shutdown());
  process.on("SIGTERM", () => void shutdown());
}

void start();
