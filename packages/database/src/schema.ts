import {
  bigint,
  boolean,
  check,
  index,
  integer,
  jsonb,
  pgEnum,
  pgTable,
  text,
  timestamp,
  uniqueIndex,
  uuid,
  varchar,
} from "drizzle-orm/pg-core";
import { sql } from "drizzle-orm";

export const platform = pgEnum("platform", [
  "WINDOWS",
  "MACOS",
  "ANDROID",
  "IOS",
]);
export const deviceStatus = pgEnum("device_status", ["ACTIVE", "REVOKED"]);
export const roomStatus = pgEnum("room_status", [
  "ACTIVE",
  "PAUSED",
  "REMOVED",
]);
export const commandStatus = pgEnum("command_status", [
  "QUEUED",
  "DELIVERED",
  "ANALYZING",
  "PROPOSAL_READY",
  "WAITING_APPROVAL",
  "APPROVED",
  "REJECTED",
  "EXPIRED",
  "EXECUTING",
  "SUCCEEDED",
  "PARTIALLY_SUCCEEDED",
  "FAILED",
  "STALE",
]);
export const proposalStatus = pgEnum("proposal_status", [
  "OPEN",
  "APPROVED",
  "REJECTED",
  "EXPIRED",
]);
export const executionStatus = pgEnum("execution_status", [
  "EXECUTING",
  "SUCCEEDED",
  "PARTIALLY_SUCCEEDED",
  "FAILED",
  "STALE",
  "ROLLED_BACK",
]);

export const users = pgTable("users", {
  id: uuid("id").primaryKey().defaultRandom(),
  authProviderUid: varchar("auth_provider_uid", { length: 128 })
    .notNull()
    .unique(),
  displayName: varchar("display_name", { length: 120 }).notNull(),
  createdAt: timestamp("created_at", { withTimezone: true })
    .notNull()
    .defaultNow(),
  deletedAt: timestamp("deleted_at", { withTimezone: true }),
});
export const devices = pgTable(
  "devices",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    platform: platform("platform").notNull(),
    deviceName: varchar("device_name", { length: 120 }).notNull(),
    publicKey: text("public_key"),
    status: deviceStatus("status").notNull().default("ACTIVE"),
    lastSeenAt: timestamp("last_seen_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("devices_user_status_idx").on(t.userId, t.status),
    index("devices_last_seen_idx").on(t.lastSeenAt),
  ],
);
export const pushNotificationTokens = pgTable(
  "push_notification_tokens",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    token: text("token").notNull(),
    tokenHash: varchar("token_hash", { length: 64 }).notNull(),
    platform: platform("platform").notNull(),
    status: varchar("status", { length: 30 }).notNull().default("ACTIVE"),
    lastSeenAt: timestamp("last_seen_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    revokedAt: timestamp("revoked_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("push_notification_tokens_hash_idx").on(t.tokenHash),
    index("push_notification_tokens_user_status_idx").on(t.userId, t.status),
  ],
);
export const pairingSessions = pgTable("pairing_sessions", {
  id: uuid("id").primaryKey().defaultRandom(),
  desktopNonce: varchar("desktop_nonce", { length: 128 }).notNull().unique(),
  pairingCodeHash: varchar("pairing_code_hash", { length: 128 })
    .notNull()
    .unique(),
  claimedByUserId: uuid("claimed_by_user_id").references(() => users.id),
  deviceName: varchar("device_name", { length: 120 }).notNull(),
  platform: platform("platform").notNull(),
  publicKey: text("public_key"),
  expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
  claimedAt: timestamp("claimed_at", { withTimezone: true }),
  claimedDeviceId: uuid("claimed_device_id").references(() => devices.id),
});
export const rooms = pgTable(
  "rooms",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    desktopDeviceId: uuid("desktop_device_id")
      .notNull()
      .references(() => devices.id),
    name: varchar("name", { length: 120 }).notNull(),
    rootAlias: varchar("root_alias", { length: 120 }).notNull(),
    status: roomStatus("status").notNull().default("ACTIVE"),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("rooms_user_created_idx").on(t.userId, t.createdAt),
    index("rooms_device_status_idx").on(t.desktopDeviceId, t.status),
  ],
);
export const connectionMutationReceipts = pgTable(
  "connection_mutation_receipts",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    actorScope: varchar("actor_scope", { length: 160 }).notNull(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    actorDeviceId: uuid("actor_device_id").references(() => devices.id),
    operation: varchar("operation", { length: 40 }).notNull(),
    aggregateId: uuid("aggregate_id").notNull(),
    idempotencyKey: varchar("idempotency_key", { length: 128 }).notNull(),
    result: jsonb("result").notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("connection_mutation_actor_key_idx").on(
      t.actorScope,
      t.idempotencyKey,
    ),
    index("connection_mutation_aggregate_idx").on(t.operation, t.aggregateId),
  ],
);
export const rules = pgTable(
  "rules",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    name: varchar("name", { length: 120 }).notNull(),
    definition: jsonb("definition").notNull(),
    priority: integer("priority").notNull(),
    enabled: boolean("enabled").notNull().default(true),
    version: integer("version").notNull().default(1),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("rules_room_enabled_priority_idx").on(
      t.roomId,
      t.enabled,
      t.priority,
    ),
  ],
);
export const commands = pgTable(
  "commands",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    targetDeviceId: uuid("target_device_id")
      .notNull()
      .references(() => devices.id),
    createdByUserId: uuid("created_by_user_id")
      .notNull()
      .references(() => users.id),
    intent: varchar("intent", { length: 40 }).notNull(),
    payload: jsonb("payload").notNull(),
    metadata: jsonb("metadata").notNull().default({}),
    status: commandStatus("status").notNull().default("QUEUED"),
    idempotencyKey: varchar("idempotency_key", { length: 128 }).notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    deliveredAt: timestamp("delivered_at", { withTimezone: true }),
    finishedAt: timestamp("finished_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("commands_user_idempotency_idx").on(
      t.createdByUserId,
      t.idempotencyKey,
    ),
    index("commands_device_status_created_idx").on(
      t.targetDeviceId,
      t.status,
      t.createdAt,
    ),
  ],
);
export const proposals = pgTable(
  "proposals",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    commandId: uuid("command_id")
      .notNull()
      .references(() => commands.id)
      .unique(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    status: proposalStatus("status").notNull().default("OPEN"),
    summary: jsonb("summary").notNull(),
    idempotencyKey: varchar("idempotency_key", { length: 128 }),
    expiresAt: timestamp("expires_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("proposals_idempotency_idx").on(t.idempotencyKey),
    index("proposals_room_status_idx").on(t.roomId, t.status),
  ],
);
export const proposalItems = pgTable(
  "proposal_items",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    proposalId: uuid("proposal_id")
      .notNull()
      .references(() => proposals.id),
    itemOrder: integer("item_order").notNull(),
    actionType: varchar("action_type", { length: 40 }).notNull(),
    sourceRelativePath: text("source_relative_path"),
    destinationRelativePath: text("destination_relative_path"),
    reasonCode: varchar("reason_code", { length: 100 }).notNull(),
    precondition: jsonb("precondition").notNull(),
    conflictState: varchar("conflict_state", { length: 40 }).notNull(),
  },
  (t) => [
    uniqueIndex("proposal_items_order_idx").on(t.proposalId, t.itemOrder),
  ],
);
export const decisions = pgTable(
  "decisions",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    proposalId: uuid("proposal_id")
      .notNull()
      .references(() => proposals.id),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    decisionType: varchar("decision_type", { length: 20 }).notNull(),
    approvedItemIds: jsonb("approved_item_ids").notNull(),
    idempotencyKey: varchar("idempotency_key", { length: 128 }).notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("decisions_user_idempotency_idx").on(
      t.userId,
      t.idempotencyKey,
    ),
    uniqueIndex("decisions_proposal_idx").on(t.proposalId),
  ],
);
export const executions = pgTable(
  "executions",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    proposalId: uuid("proposal_id")
      .notNull()
      .references(() => proposals.id),
    decisionId: uuid("decision_id")
      .notNull()
      .references(() => decisions.id),
    desktopDeviceId: uuid("desktop_device_id")
      .notNull()
      .references(() => devices.id),
    status: executionStatus("status").notNull().default("EXECUTING"),
    resultSummary: jsonb("result_summary"),
    idempotencyKey: varchar("idempotency_key", { length: 128 })
      .notNull()
      .unique(),
    resultIdempotencyKey: varchar("result_idempotency_key", { length: 128 }),
    startedAt: timestamp("started_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    finishedAt: timestamp("finished_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("executions_result_idempotency_idx").on(t.resultIdempotencyKey),
    index("executions_proposal_started_idx").on(t.proposalId, t.startedAt),
  ],
);
export const roomSnapshots = pgTable(
  "room_snapshots",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    score: integer("score").notNull(),
    metrics: jsonb("metrics").notNull(),
    formulaVersion: varchar("formula_version", { length: 80 })
      .notNull()
      .default("mousekeeper-cleanliness-v1"),
    calculatedAt: timestamp("calculated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    check("room_snapshots_score_check", sql`${t.score} between 0 and 100`),
    index("room_snapshots_room_time_idx").on(t.roomId, t.calculatedAt),
  ],
);
export const syncEvents = pgTable(
  "sync_events",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    deviceId: uuid("device_id").references(() => devices.id),
    roomId: uuid("room_id").references(() => rooms.id),
    sequence: bigint("sequence", { mode: "number" }).notNull(),
    eventType: varchar("event_type", { length: 100 }).notNull(),
    schemaVersion: integer("schema_version").notNull().default(1),
    correlationId: uuid("correlation_id").notNull().defaultRandom(),
    aggregateType: varchar("aggregate_type", { length: 50 }).notNull(),
    aggregateId: uuid("aggregate_id").notNull(),
    payload: jsonb("payload").notNull(),
    occurredAt: timestamp("occurred_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    publishedAt: timestamp("published_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("sync_events_user_sequence_idx").on(t.userId, t.sequence),
    index("sync_events_user_time_idx").on(t.userId, t.occurredAt),
  ],
);
export const notificationJobs = pgTable(
  "notification_jobs",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    syncEventId: uuid("sync_event_id")
      .notNull()
      .references(() => syncEvents.id),
    eventType: varchar("event_type", { length: 100 }).notNull(),
    title: varchar("title", { length: 120 }).notNull(),
    body: varchar("body", { length: 500 }).notNull(),
    status: varchar("status", { length: 30 }).notNull().default("PENDING"),
    attemptCount: integer("attempt_count").notNull().default(0),
    nextAttemptAt: timestamp("next_attempt_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    lastErrorCode: varchar("last_error_code", { length: 80 }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    completedAt: timestamp("completed_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("notification_jobs_sync_event_idx").on(t.syncEventId),
    index("notification_jobs_pending_idx").on(t.status, t.nextAttemptAt),
    index("notification_jobs_user_status_idx").on(t.userId, t.status),
  ],
);
export const auditEvents = pgTable(
  "audit_events",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id").references(() => users.id),
    deviceId: uuid("device_id").references(() => devices.id),
    roomId: uuid("room_id").references(() => rooms.id),
    eventType: varchar("event_type", { length: 100 }).notNull(),
    aggregateType: varchar("aggregate_type", { length: 50 }).notNull(),
    aggregateId: uuid("aggregate_id"),
    metadata: jsonb("metadata").notNull(),
    occurredAt: timestamp("occurred_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("audit_events_user_time_idx").on(t.userId, t.occurredAt),
    index("audit_events_room_time_idx").on(t.roomId, t.occurredAt),
  ],
);

export const fileBrowseRequests = pgTable(
  "file_browse_requests",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    desktopDeviceId: uuid("desktop_device_id")
      .notNull()
      .references(() => devices.id),
    relativeDirectory: text("relative_directory").notNull(),
    cursor: varchar("cursor", { length: 512 }),
    query: varchar("query", { length: 100 }),
    extensions: jsonb("extensions").notNull().default([]),
    limit: integer("limit").notNull().default(200),
    searchScope: varchar("search_scope", { length: 30 })
      .notNull()
      .default("CURRENT_DIRECTORY"),
    status: varchar("status", { length: 40 }).notNull().default("REQUESTED"),
    failureCode: varchar("failure_code", { length: 60 }),
    resultPage: jsonb("result_page"),
    desktopGeneration: varchar("desktop_generation", { length: 128 }),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("browse_device_status_idx").on(
      t.desktopDeviceId,
      t.status,
      t.createdAt,
    ),
  ],
);

export const fileTransfers = pgTable(
  "file_transfers",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    desktopDeviceId: uuid("desktop_device_id")
      .notNull()
      .references(() => devices.id),
    requestedByUserId: uuid("requested_by_user_id")
      .notNull()
      .references(() => users.id),
    sourceRelativePath: text("source_relative_path").notNull(),
    sourceVersion: jsonb("source_version"),
    status: varchar("status", { length: 40 }).notNull().default("REQUESTED"),
    failureCode: varchar("failure_code", { length: 60 }),
    objectKey: text("object_key"),
    sizeBytes: bigint("size_bytes", { mode: "number" }),
    sha256: varchar("sha256", { length: 64 }),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    completedAt: timestamp("completed_at", { withTimezone: true }),
    idempotencyKey: varchar("idempotency_key", { length: 128 }).notNull(),
    uploadCompletionIdempotencyKey: varchar(
      "upload_completion_idempotency_key",
      { length: 128 },
    ),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("transfers_user_idempotency_idx").on(
      t.requestedByUserId,
      t.idempotencyKey,
    ),
    uniqueIndex("transfers_upload_completion_idempotency_idx").on(
      t.requestedByUserId,
      t.uploadCompletionIdempotencyKey,
    ),
    index("transfers_device_status_idx").on(
      t.desktopDeviceId,
      t.status,
      t.createdAt,
    ),
    index("transfers_expiry_idx").on(t.expiresAt, t.status),
  ],
);

export const objectDeletionJobs = pgTable(
  "object_deletion_jobs",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    transferId: uuid("transfer_id")
      .notNull()
      .references(() => fileTransfers.id),
    objectKey: text("object_key").notNull(),
    status: varchar("status", { length: 30 }).notNull().default("PENDING"),
    attemptCount: integer("attempt_count").notNull().default(0),
    nextAttemptAt: timestamp("next_attempt_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    lastErrorCode: varchar("last_error_code", { length: 80 }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    completedAt: timestamp("completed_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("deletion_jobs_transfer_idx").on(t.transferId),
    index("deletion_jobs_pending_idx").on(t.status, t.nextAttemptAt),
  ],
);

export const characterProfiles = pgTable("character_profiles", {
  id: uuid("id").primaryKey().defaultRandom(),
  userId: uuid("user_id")
    .notNull()
    .references(() => users.id)
    .unique(),
  appearance: jsonb("appearance").notNull().default({}),
  roomTheme: varchar("room_theme", { length: 80 }),
  affinityTotal: integer("affinity_total").notNull().default(0),
  createdAt: timestamp("created_at", { withTimezone: true })
    .notNull()
    .defaultNow(),
  updatedAt: timestamp("updated_at", { withTimezone: true })
    .notNull()
    .defaultNow(),
});
export const affinityEvents = pgTable(
  "affinity_events",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    characterProfileId: uuid("character_profile_id")
      .notNull()
      .references(() => characterProfiles.id),
    eventType: varchar("event_type", { length: 80 }).notNull(),
    delta: integer("delta").notNull(),
    sourceDecisionId: uuid("source_decision_id").references(() => decisions.id),
    sourceExecutionId: uuid("source_execution_id").references(
      () => executions.id,
    ),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("affinity_decision_event_idx").on(
      t.sourceDecisionId,
      t.eventType,
    ),
    uniqueIndex("affinity_execution_event_idx").on(
      t.sourceExecutionId,
      t.eventType,
    ),
  ],
);

export const chatSessions = pgTable(
  "chat_sessions",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    title: varchar("title", { length: 120 }).notNull(),
    summary: text("summary"),
    status: varchar("status", { length: 30 }).notNull().default("ACTIVE"),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    deletedAt: timestamp("deleted_at", { withTimezone: true }),
  },
  (t) => [
    index("chat_sessions_user_room_status_updated_idx").on(
      t.userId,
      t.roomId,
      t.status,
      t.updatedAt,
    ),
  ],
);
export const chatMessages = pgTable(
  "chat_messages",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    sessionId: uuid("session_id").references(() => chatSessions.id),
    senderType: varchar("sender_type", { length: 30 }).notNull(),
    messageType: varchar("message_type", { length: 30 })
      .notNull()
      .default("TEXT"),
    content: text("content").notNull(),
    structuredPayload: jsonb("structured_payload"),
    commandId: uuid("command_id").references(() => commands.id),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("chat_room_time_idx").on(t.roomId, t.createdAt),
    index("chat_session_time_idx").on(t.sessionId, t.createdAt, t.id),
  ],
);

export const chatReadStates = pgTable(
  "chat_read_states",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    sessionId: uuid("session_id")
      .notNull()
      .references(() => chatSessions.id),
    lastReadMessageId: uuid("last_read_message_id").references(
      () => chatMessages.id,
    ),
    readAt: timestamp("read_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("chat_read_states_user_session_idx").on(t.userId, t.sessionId),
    index("chat_read_states_user_updated_idx").on(t.userId, t.updatedAt),
  ],
);

export const commandDrafts = pgTable(
  "command_drafts",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    sessionId: uuid("session_id")
      .notNull()
      .references(() => chatSessions.id),
    sourceMessageId: uuid("source_message_id")
      .notNull()
      .references(() => chatMessages.id),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    createdByUserId: uuid("created_by_user_id")
      .notNull()
      .references(() => users.id),
    intent: varchar("intent", { length: 40 }).notNull(),
    arguments: jsonb("arguments").notNull(),
    confirmationSummary: text("confirmation_summary").notNull(),
    status: varchar("status", { length: 30 }).notNull().default("DRAFT"),
    commandId: uuid("command_id").references(() => commands.id),
    fileBrowseRequestId: uuid("file_browse_request_id").references(
      () => fileBrowseRequests.id,
    ),
    fileTransferId: uuid("file_transfer_id").references(() => fileTransfers.id),
    confirmIdempotencyKey: varchar("confirm_idempotency_key", { length: 128 }),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("command_drafts_session_status_idx").on(t.sessionId, t.status),
    index("command_drafts_room_status_idx").on(t.roomId, t.status),
    index("command_drafts_user_status_idx").on(t.createdByUserId, t.status),
  ],
);

export const ruleDrafts = pgTable(
  "rule_drafts",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    createdByUserId: uuid("created_by_user_id")
      .notNull()
      .references(() => users.id),
    name: varchar("name", { length: 120 }).notNull(),
    definition: jsonb("definition").notNull(),
    explanation: text("explanation").notNull(),
    ambiguities: jsonb("ambiguities").notNull().default([]),
    status: varchar("status", { length: 30 }).notNull().default("DRAFT"),
    ruleId: uuid("rule_id").references(() => rules.id),
    confirmIdempotencyKey: varchar("confirm_idempotency_key", { length: 128 }),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("rule_drafts_room_status_idx").on(t.roomId, t.status),
    index("rule_drafts_user_status_idx").on(t.createdByUserId, t.status),
    uniqueIndex("rule_drafts_confirm_key_idx").on(
      t.createdByUserId,
      t.confirmIdempotencyKey,
    ),
  ],
);

export const smartCachePolicies = pgTable("smart_cache_policies", {
  roomId: uuid("room_id")
    .primaryKey()
    .references(() => rooms.id),
  enabled: boolean("enabled").notNull().default(false),
  quotaBytes: bigint("quota_bytes", { mode: "number" }).notNull(),
  maxFileBytes: bigint("max_file_bytes", { mode: "number" }).notNull(),
  excludedPatterns: jsonb("excluded_patterns").notNull().default([]),
  pinnedPatterns: jsonb("pinned_patterns").notNull().default([]),
  updatedAt: timestamp("updated_at", { withTimezone: true })
    .notNull()
    .defaultNow(),
});
export const cacheCandidateBatches = pgTable(
  "cache_candidate_batches",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id),
    desktopDeviceId: uuid("desktop_device_id")
      .notNull()
      .references(() => devices.id),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    idempotencyKey: varchar("idempotency_key", { length: 128 }).notNull(),
    requestHash: varchar("request_hash", { length: 64 }).notNull(),
    candidateCount: integer("candidate_count").notNull(),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("cache_candidate_batches_user_key_idx").on(
      t.userId,
      t.idempotencyKey,
    ),
  ],
);
export const cacheUploadReservations = pgTable(
  "cache_upload_reservations",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    batchId: uuid("batch_id").references(() => cacheCandidateBatches.id),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    desktopDeviceId: uuid("desktop_device_id")
      .notNull()
      .references(() => devices.id),
    sourceRelativePath: text("source_relative_path").notNull(),
    sourceVersion: jsonb("source_version").notNull(),
    sourceVersionHash: varchar("source_version_hash", { length: 64 }).notNull(),
    reservedBytes: bigint("reserved_bytes", { mode: "number" }).notNull(),
    status: varchar("status", { length: 30 }).notNull().default("RESERVED"),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    objectKey: text("object_key").notNull(),
    completionIdempotencyKey: varchar("completion_idempotency_key", {
      length: 128,
    }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("cache_reservation_version_idx").on(
      t.roomId,
      t.sourceRelativePath,
      t.sourceVersionHash,
    ),
    uniqueIndex("cache_reservation_completion_key_idx").on(
      t.desktopDeviceId,
      t.completionIdempotencyKey,
    ),
    index("cache_reservation_batch_idx").on(t.batchId),
    index("cache_reservation_quota_idx").on(t.roomId, t.status, t.expiresAt),
  ],
);
export const cachedFiles = pgTable(
  "cached_files",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    roomId: uuid("room_id")
      .notNull()
      .references(() => rooms.id),
    sourceRelativePath: text("source_relative_path").notNull(),
    sourceVersion: jsonb("source_version").notNull(),
    sourceVersionHash: varchar("source_version_hash", { length: 64 }).notNull(),
    usageScore: integer("usage_score").notNull(),
    manualPin: boolean("manual_pin").notNull().default(false),
    objectKey: text("object_key").notNull(),
    sizeBytes: bigint("size_bytes", { mode: "number" }).notNull(),
    sha256: varchar("sha256", { length: 64 }),
    encryptionMetadata: jsonb("encryption_metadata"),
    availabilityStatus: varchar("availability_status", { length: 30 })
      .notNull()
      .default("AVAILABLE"),
    freshnessStatus: varchar("freshness_status", { length: 30 })
      .notNull()
      .default("VERIFIED_CURRENT"),
    cachedAt: timestamp("cached_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    lastVerifiedAt: timestamp("last_verified_at", {
      withTimezone: true,
    }).notNull(),
    lastAccessedAt: timestamp("last_accessed_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("cached_file_version_idx").on(
      t.roomId,
      t.sourceRelativePath,
      t.sourceVersionHash,
    ),
    index("cached_files_room_state_idx").on(
      t.roomId,
      t.availabilityStatus,
      t.freshnessStatus,
    ),
  ],
);
export const cacheDeletionJobs = pgTable(
  "cache_deletion_jobs",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    cachedFileId: uuid("cached_file_id")
      .notNull()
      .references(() => cachedFiles.id),
    objectKey: text("object_key").notNull(),
    status: varchar("status", { length: 30 }).notNull().default("PENDING"),
    attemptCount: integer("attempt_count").notNull().default(0),
    nextAttemptAt: timestamp("next_attempt_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    lastErrorCode: varchar("last_error_code", { length: 80 }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    completedAt: timestamp("completed_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("cache_deletion_file_idx").on(t.cachedFileId),
    index("cache_deletion_pending_idx").on(t.status, t.nextAttemptAt),
  ],
);
export const cacheReservationDeletionJobs = pgTable(
  "cache_reservation_deletion_jobs",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    reservationId: uuid("reservation_id")
      .notNull()
      .references(() => cacheUploadReservations.id),
    objectKey: text("object_key").notNull(),
    status: varchar("status", { length: 30 }).notNull().default("PENDING"),
    attemptCount: integer("attempt_count").notNull().default(0),
    nextAttemptAt: timestamp("next_attempt_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    lastErrorCode: varchar("last_error_code", { length: 80 }),
    completedAt: timestamp("completed_at", { withTimezone: true }),
  },
  (t) => [
    uniqueIndex("cache_reservation_deletion_idx").on(t.reservationId),
    index("cache_reservation_deletion_pending_idx").on(
      t.status,
      t.nextAttemptAt,
    ),
  ],
);
