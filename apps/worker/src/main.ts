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
  rooms,
  syncEvents,
  type Database,
} from "@housemouse/database";
import { and, eq, inArray, lte, max, sql } from "drizzle-orm";
import { loadWorkerConfig } from "./config";

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

await tick();
const timer = setInterval(() => void tick(), 30_000);

async function shutdown() {
  clearInterval(timer);
  await connection.close();
  process.exit(0);
}

process.on("SIGINT", () => void shutdown());
process.on("SIGTERM", () => void shutdown());
