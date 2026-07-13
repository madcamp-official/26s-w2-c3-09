import { z } from "zod";
import {
  fileNameSchema,
  idempotencyKeySchema,
  relativeDirectorySchema,
  relativePathSchema,
  uuidSchema,
} from "./common";

export const platformSchema = z.enum(["WINDOWS", "MACOS", "ANDROID", "IOS"]);
export const registerPushNotificationTokenSchema = z
  .object({
    token: z.string().min(20).max(4096),
    platform: z.enum(["ANDROID", "IOS"]),
  })
  .strict();
export const presenceSchema = z.enum([
  "ONLINE_IDLE",
  "ONLINE_SCANNING",
  "ONLINE_EXECUTING",
  "DEGRADED",
  "OFFLINE",
]);
export const MOUSEKEEPER_CLEANLINESS_FORMULA_VERSION =
  "mousekeeper-cleanliness-v1" as const;
export const characterStateSchema = z.enum([
  "IDLE",
  "CONNECTING",
  "ANALYZING",
  "WAITING_APPROVAL",
  "WORKING",
  "SUCCESS",
  "ERROR",
  "USER_WORKING",
  "OFFLINE",
]);
export const characterEventSchema = z
  .object({
    eventId: uuidSchema,
    roomId: uuidSchema.nullable(),
    kind: characterStateSchema,
    progress: z.number().min(0).max(1).optional(),
    occurredAt: z.iso.datetime(),
  })
  .strict();
export const commandStatusSchema = z.enum([
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
export const commandIntentSchema = z.enum([
  "SCAN",
  "CREATE_RULE",
  "ANALYZE",
  "README",
  "RENAME",
  "MOVE",
  "ORGANIZE",
  "CREATE",
  "TRASH",
  "FIND",
  "DOWNLOAD",
  "UPLOAD",
]);

export const createPairingSessionSchema = z
  .object({
    deviceName: z.string().trim().min(1).max(120),
    platform: z.enum(["WINDOWS", "MACOS"]),
    publicKey: z.string().min(32).max(8192).optional(),
  })
  .strict();

export const claimPairingSessionSchema = z
  .object({
    code: z.string().regex(/^\d{6}$/),
  })
  .strict();

export const createRoomSchema = z
  .object({
    desktopDeviceId: uuidSchema,
    name: z.string().trim().min(1).max(120),
    rootAlias: z.string().trim().min(1).max(120),
  })
  .strict();

const extensionConditionSchema = z
  .object({
    field: z.literal("extension"),
    operator: z.literal("IN"),
    value: z
      .array(z.string().regex(/^\.[a-z0-9]+$/i))
      .min(1)
      .max(50),
  })
  .strict();

const ageConditionSchema = z
  .object({
    field: z.literal("ageDays"),
    operator: z.literal("GTE"),
    value: z.number().int().nonnegative().max(36500),
  })
  .strict();

const fileAgeConditionSchema = z
  .object({
    field: z.enum(["modifiedAgeDays", "createdAgeDays"]),
    operator: z.enum(["GTE", "GT"]),
    value: z.number().int().nonnegative().max(36500),
  })
  .strict();

const sizeConditionSchema = z
  .object({
    field: z.literal("sizeBytes"),
    operator: z.enum(["GTE", "LTE"]),
    value: z.number().int().nonnegative(),
  })
  .strict();

const relativePathConditionSchema = z
  .object({
    field: z.literal("relativePath"),
    operator: z.literal("STARTS_WITH"),
    value: relativePathSchema,
  })
  .strict();

const fileKindConditionSchema = z
  .object({
    field: z.literal("fileKind"),
    operator: z.literal("EQ"),
    value: z.enum(["FILE", "DIRECTORY"]),
  })
  .strict();

const nameConditionSchema = z
  .object({
    field: z.literal("name"),
    operator: z.enum(["CONTAINS", "STARTS_WITH", "ENDS_WITH"]),
    value: z.string().trim().min(1).max(120),
  })
  .strict();

export const ruleDefinitionSchema = z
  .object({
    match: z.enum(["ALL", "ANY"]).default("ALL"),
    conditions: z
      .array(
        z.discriminatedUnion("field", [
          extensionConditionSchema,
          ageConditionSchema,
          fileAgeConditionSchema,
          sizeConditionSchema,
          relativePathConditionSchema,
          fileKindConditionSchema,
          nameConditionSchema,
        ]),
      )
      .min(1)
      .max(10),
    action: z.discriminatedUnion("type", [
      z
        .object({
          type: z.literal("MOVE"),
          destinationTemplate: relativePathSchema,
        })
        .strict(),
      z.object({ type: z.literal("QUARANTINE") }).strict(),
      z.object({ type: z.literal("TRASH") }).strict(),
      z
        .object({
          type: z.literal("CREATE_DIR"),
          relativePath: relativePathSchema,
        })
        .strict(),
    ]),
  })
  .strict();

export const createRuleSchema = z
  .object({
    name: z.string().trim().min(1).max(120),
    definition: ruleDefinitionSchema,
    priority: z.number().int().min(0).max(10000),
    enabled: z.boolean().default(true),
  })
  .strict();

export const updateRuleSchema = z
  .object({
    name: z.string().trim().min(1).max(120).optional(),
    definition: ruleDefinitionSchema.optional(),
    priority: z.number().int().min(0).max(10000).optional(),
    enabled: z.boolean().optional(),
    version: z.number().int().positive(),
  })
  .strict()
  .refine(
    (value) => Object.keys(value).some((key) => key !== "version"),
    "At least one rule field must be updated",
  );

export const roomSnapshotMetricsSchema = z
  .object({
    totalFileCount: z.number().int().nonnegative(),
    managedFileCount: z.number().int().nonnegative(),
    unorganizedFileCount: z.number().int().nonnegative(),
    deductions: z
      .array(
        z
          .object({
            reasonCode: z.string().trim().min(1).max(100),
            count: z.number().int().nonnegative(),
            points: z.number().int().nonnegative().max(100),
          })
          .strict(),
      )
      .max(100),
  })
  .strict()
  .refine(
    (value) =>
      value.managedFileCount <= value.totalFileCount &&
      value.unorganizedFileCount <= value.totalFileCount,
    "File counts must not exceed totalFileCount",
  );

export const createRoomSnapshotSchema = z
  .object({
    score: z.number().int().min(0).max(100),
    metrics: roomSnapshotMetricsSchema,
    calculatedAt: z.iso.datetime(),
    formulaVersion: z
      .literal(MOUSEKEEPER_CLEANLINESS_FORMULA_VERSION)
      .default(MOUSEKEEPER_CLEANLINESS_FORMULA_VERSION),
  })
  .strict();

const emptyCommandPayloadSchema = z.object({}).strict();
const commandRootIdSchema = z
  .string()
  .trim()
  .min(1)
  .max(160)
  .refine((value) => !value.includes("\0"), "rootId must be opaque text");
const commandMetadataSchema = z
  .object({
    sessionId: uuidSchema.optional(),
    sourceMessageId: uuidSchema.optional(),
    idempotencyKey: idempotencyKeySchema.optional(),
    requiresApproval: z.boolean().optional(),
  })
  .strict();
export const readmeCommandPayloadSchema = z
  .object({
    purpose: z.string().trim().min(1).max(500),
    audience: z.string().trim().min(1).max(200),
    tone: z.enum(["concise", "friendly", "technical"]),
    sections: z.array(z.string().trim().min(1).max(120)).max(20),
  })
  .strict();
const commandEnvelope = <T extends z.ZodRawShape>(shape: T) =>
  z
    .object({
      ...shape,
      metadata: commandMetadataSchema.optional(),
    })
    .strict();

export const renameCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    sourceRelativePath: relativePathSchema,
    newName: fileNameSchema,
  })
  .strict();

export const moveCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    sourceRelativePaths: z.array(relativePathSchema).min(1).max(200),
    destinationRelativeDirectory: relativeDirectorySchema,
  })
  .strict();

export const organizeCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    scopeRelativePath: relativeDirectorySchema,
    instruction: z.string().trim().min(1).max(2000).optional(),
  })
  .strict();

export const createFileCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    kind: z.enum(["FILE", "DIRECTORY"]),
    relativePath: relativePathSchema,
    content: z.string().max(200_000).optional(),
  })
  .strict();

export const trashCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    sourceRelativePaths: z.array(relativePathSchema).min(1).max(200),
  })
  .strict();

export const findCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    query: z.string().trim().min(1).max(200),
    extensions: z
      .array(z.string().regex(/^\.[a-z0-9]+$/i))
      .min(1)
      .max(50)
      .optional(),
    scopeRelativePath: relativeDirectorySchema.optional(),
    limit: z.number().int().min(1).max(200),
  })
  .strict();

export const fileIdentitySchema = z
  .object({
    fileId: z.string().min(1).max(512).optional(),
    sizeBytes: z.number().int().nonnegative().optional(),
    modifiedAt: z.iso.datetime().optional(),
    sha256: z
      .string()
      .regex(/^[a-f0-9]{64}$/)
      .optional(),
  })
  .strict()
  .refine((value) => Object.keys(value).length > 0, "empty file identity");

export const downloadCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    sourceRelativePath: relativePathSchema,
    expectedIdentity: fileIdentitySchema.optional(),
  })
  .strict();

export const uploadCommandPayloadSchema = z
  .object({
    rootId: commandRootIdSchema,
    destinationRelativePath: relativePathSchema,
    transferId: uuidSchema,
    expectedSha256: z.string().regex(/^[a-f0-9]{64}$/),
    expectedSize: z.number().int().positive(),
  })
  .strict();

export const createCommandSchema = z.discriminatedUnion("intent", [
  z
    .object({
      intent: z.literal("SCAN"),
      payload: emptyCommandPayloadSchema,
      metadata: commandMetadataSchema.optional(),
    })
    .strict(),
  z
    .object({
      intent: z.literal("ANALYZE"),
      payload: emptyCommandPayloadSchema,
      metadata: commandMetadataSchema.optional(),
    })
    .strict(),
  z
    .object({
      intent: z.literal("CREATE_RULE"),
      payload: z.object({ rule: createRuleSchema }).strict(),
      metadata: commandMetadataSchema.optional(),
    })
    .strict(),
  z
    .object({
      intent: z.literal("README"),
      payload: readmeCommandPayloadSchema,
      metadata: commandMetadataSchema.optional(),
    })
    .strict(),
  commandEnvelope({
    intent: z.literal("RENAME"),
    payload: renameCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("MOVE"),
    payload: moveCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("ORGANIZE"),
    payload: organizeCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("CREATE"),
    payload: createFileCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("TRASH"),
    payload: trashCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("FIND"),
    payload: findCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("DOWNLOAD"),
    payload: downloadCommandPayloadSchema,
  }),
  commandEnvelope({
    intent: z.literal("UPLOAD"),
    payload: uploadCommandPayloadSchema,
  }),
]);

export const updateCommandStatusSchema = z
  .object({
    status: z.enum(["DELIVERED", "ANALYZING", "FAILED"]),
  })
  .strict();

export const proposalItemSchema = z
  .object({
    itemOrder: z.number().int().nonnegative(),
    actionType: z.enum(["MOVE", "QUARANTINE", "CREATE_DIR", "README_WRITE"]),
    sourceRelativePath: relativePathSchema.nullable(),
    destinationRelativePath: relativePathSchema.nullable(),
    reasonCode: z.string().min(1).max(100),
    precondition: z.record(z.string(), z.unknown()),
    conflictState: z.enum(["NONE", "NAME_CONFLICT", "UNSUPPORTED"]),
  })
  .strict();

export const createProposalSchema = z
  .object({
    commandId: uuidSchema,
    roomId: uuidSchema,
    summary: z
      .object({
        itemCount: z.number().int().nonnegative().optional(),
        readmeDraft: z.string().max(200_000).optional(),
        readmeDiff: z.string().max(200_000).optional(),
      })
      .catchall(z.unknown())
      .refine(
        (value) => JSON.stringify(value).length <= 256_000,
        "proposal summary too large",
      ),
    expiresAt: z.iso.datetime().nullable(),
    items: z.array(proposalItemSchema).min(1).max(200),
  })
  .strict()
  .superRefine((value, context) => {
    const seen = new Set<string>();
    value.items.forEach((item, index) => {
      if (!item.sourceRelativePath) return;
      const normalized = item.sourceRelativePath.replaceAll("\\", "/");
      if (seen.has(normalized)) {
        context.addIssue({
          code: "custom",
          path: ["items", index, "sourceRelativePath"],
          message: "A file may appear only once in a proposal",
        });
      }
      seen.add(normalized);
    });
  });

export const createDecisionSchema = z
  .object({
    decisionType: z.enum(["APPROVE", "REJECT"]),
    approvedItemIds: z.array(uuidSchema).default([]),
  })
  .strict();

export const createExecutionSchema = z
  .object({
    proposalId: uuidSchema,
    decisionId: uuidSchema,
    desktopDeviceId: uuidSchema,
  })
  .strict();

export const updateExecutionSchema = z
  .object({
    status: z.enum([
      "SUCCEEDED",
      "PARTIALLY_SUCCEEDED",
      "FAILED",
      "STALE",
      "ROLLED_BACK",
    ]),
    resultSummary: z.record(z.string(), z.unknown()),
  })
  .strict();

export const heartbeatSchema = z
  .object({
    presence: presenceSchema.exclude(["OFFLINE"]),
  })
  .strict();

export const executionStatusSchema = z.enum([
  "EXECUTING",
  "SUCCEEDED",
  "PARTIALLY_SUCCEEDED",
  "FAILED",
  "STALE",
  "ROLLED_BACK",
]);

export const homeSummaryDeviceSchema = z
  .object({
    id: uuidSchema,
    platform: platformSchema,
    deviceName: z.string().min(1).max(120),
    status: z.literal("ACTIVE"),
    lastSeenAt: z.iso.datetime().nullable(),
    createdAt: z.iso.datetime(),
    presence: presenceSchema,
  })
  .strict();

export const homeSummaryRoomSchema = z
  .object({
    id: uuidSchema,
    desktopDeviceId: uuidSchema,
    name: z.string().min(1).max(120),
    rootAlias: z.string().min(1).max(120),
    status: z.literal("ACTIVE"),
    createdAt: z.iso.datetime(),
    pendingProposalCount: z.number().int().nonnegative(),
    latestExecutionStatus: executionStatusSchema.nullable(),
    cleanlinessScore: z.number().int().min(0).max(100).nullable(),
    cleanlinessFormulaVersion: z.string().min(1).max(80).nullable(),
    cleanlinessCalculatedAt: z.iso.datetime().nullable(),
  })
  .strict();

export const homeSummaryCharacterSchema = z
  .object({
    id: uuidSchema,
    appearance: z.record(z.string(), z.unknown()),
    roomTheme: z.string().max(80).nullable(),
    affinityTotal: z.number().int().nonnegative(),
    createdAt: z.iso.datetime(),
    updatedAt: z.iso.datetime(),
    unlockedItems: z.array(z.string().min(1).max(120)).max(100),
    nextUnlockAffinity: z.number().int().nonnegative().nullable(),
    riveAssetStatus: z.enum(["CONFIGURED", "UNCONFIGURED"]),
  })
  .strict();

export const homeSummaryResponseSchema = z
  .object({
    devices: z.array(homeSummaryDeviceSchema),
    rooms: z.array(homeSummaryRoomSchema),
    character: homeSummaryCharacterSchema,
  })
  .strict();

export type HomeSummaryResponse = z.infer<typeof homeSummaryResponseSchema>;

export const createFileBrowseRequestSchema = z
  .object({
    relativeDirectory: relativePathSchema.or(z.literal("")),
    cursor: z.string().max(512).nullable().default(null),
    query: z
      .string()
      .trim()
      .max(200)
      .refine((value) => {
        const codePointLength = Array.from(value).length;
        return codePointLength >= 2 && codePointLength <= 100;
      }, "File search query must contain 2 to 100 characters")
      .nullable()
      .default(null),
    searchScope: z
      .enum(["CURRENT_DIRECTORY", "MANAGED_ROOT"])
      .default("CURRENT_DIRECTORY"),
  })
  .strict();

export const devicePairedEventPayloadSchema = z
  .object({
    deviceId: uuidSchema,
    status: z.literal("ACTIVE"),
  })
  .strict();

export const deviceRevokedEventPayloadSchema = z
  .object({
    deviceId: uuidSchema,
    status: z.literal("REVOKED"),
  })
  .strict();

export const roomRemovedEventPayloadSchema = z
  .object({
    roomId: uuidSchema,
    status: z.literal("REMOVED"),
  })
  .strict();
export const fileBrowseEntrySchema = z
  .object({
    name: z.string().min(1).max(255),
    relativePath: relativePathSchema,
    type: z.enum(["FILE", "DIRECTORY"]),
    sizeBytes: z.number().int().nonnegative().nullable(),
    modifiedAt: z.iso.datetime(),
    fileId: z.string().min(1).max(512),
  })
  .strict();
export const completeFileBrowseSchema = z
  .object({
    entries: z.array(fileBrowseEntrySchema).max(200),
    nextCursor: z.string().max(512).nullable(),
    desktopGeneration: z.string().min(1).max(128),
  })
  .strict();
export const failFileBrowseSchema = z
  .object({
    failureCode: z.enum([
      "DEVICE_OFFLINE",
      "TIMED_OUT",
      "CURSOR_INVALIDATED",
      "OUTSIDE_MANAGED_ROOT",
    ]),
  })
  .strict();

export const createFileTransferSchema = z
  .object({ sourceRelativePath: relativePathSchema })
  .strict();
export const requestUploadTargetSchema = z
  .object({
    sourceVersion: z
      .object({
        fileId: z.string().min(1).max(512),
        sizeBytes: z.number().int().positive(),
        modifiedAt: z.iso.datetime(),
      })
      .strict(),
  })
  .strict();
export const completeFileUploadSchema = z
  .object({
    sizeBytes: z.number().int().positive(),
    sha256: z.string().regex(/^[a-f0-9]{64}$/),
  })
  .strict();
export const transferFailureCodeSchema = z.enum([
  "SOURCE_NOT_FOUND",
  "SOURCE_CHANGED",
  "OUTSIDE_MANAGED_ROOT",
  "SIZE_LIMIT_EXCEEDED",
  "CHECKSUM_MISMATCH",
]);
export const failFileTransferSchema = z
  .object({ failureCode: transferFailureCodeSchema })
  .strict();
export const updateCharacterSchema = z
  .object({
    appearance: z
      .object({
        furVariant: z.enum(["brown", "cream"]).optional(),
        accessory: z.enum(["none", "scarf"]).optional(),
        animationsEnabled: z.boolean().optional(),
      })
      .strict()
      .refine((value) => Object.keys(value).length > 0, "empty appearance")
      .optional(),
    roomTheme: z.enum(["warm", "forest"]).nullable().optional(),
  })
  .strict()
  .refine((value) => Object.keys(value).length > 0, "empty update");

export const chatSessionStatusSchema = z.enum(["ACTIVE", "DELETED"]);
export const chatMessageSenderSchema = z.enum(["USER", "ASSISTANT", "SYSTEM"]);
export const chatMessageTypeSchema = z.enum([
  "TEXT",
  "COMMAND_DRAFT",
  "RULE_DRAFT",
  "EXECUTION_RESULT",
]);
export const chatMessageSchema = z
  .object({
    id: uuidSchema,
    roomId: uuidSchema,
    sessionId: uuidSchema.nullable(),
    senderType: chatMessageSenderSchema,
    messageType: chatMessageTypeSchema,
    content: z.string().min(1),
    structuredPayload: z.record(z.string(), z.unknown()).nullable(),
    commandId: uuidSchema.nullable(),
    createdAt: z.iso.datetime(),
  })
  .strict();
export const aiProviderUnavailableSchema = z
  .object({
    status: z.literal("UNCONFIGURED"),
    code: z.literal("AI_PROVIDER_UNCONFIGURED"),
  })
  .strict();
export const aiProviderInvalidSchema = z
  .object({
    status: z.literal("INVALID"),
    code: z.literal("AI_OUTPUT_INVALID"),
  })
  .strict();
export const aiProviderNoActionSchema = z
  .object({
    status: z.literal("READY"),
    kind: z.literal("NO_ACTION"),
  })
  .strict();
export const aiProviderCommandDraftSchema = z
  .object({
    status: z.literal("READY"),
    kind: z.literal("COMMAND_DRAFT"),
    commandDraftId: uuidSchema,
  })
  .strict();
export const chatAiResultSchema = z.union([
  aiProviderUnavailableSchema,
  aiProviderInvalidSchema,
  aiProviderNoActionSchema,
  aiProviderCommandDraftSchema,
]);
export const createChatMessageResponseSchema = z
  .object({
    message: chatMessageSchema,
    assistant: chatMessageSchema.nullable(),
    aiStatus: z.enum(["UNCONFIGURED", "READY", "INVALID"]),
    ai: chatAiResultSchema,
  })
  .strict()
  .superRefine((value, context) => {
    if (value.aiStatus !== value.ai.status) {
      context.addIssue({
        code: "custom",
        path: ["aiStatus"],
        message: "aiStatus must match ai.status",
      });
    }
  });
export const commandDraftStatusSchema = z.enum([
  "DRAFT",
  "APPROVED",
  "REJECTED",
  "EXPIRED",
  "MATERIALIZED",
]);
export const createChatSessionSchema = z
  .object({
    title: z.string().trim().min(1).max(120).optional(),
  })
  .strict();
export const updateChatSessionSchema = z
  .object({
    title: z.string().trim().min(1).max(120).optional(),
  })
  .strict()
  .refine((value) => Object.keys(value).length > 0, "empty update");
export const createChatMessageSchema = z
  .object({ content: z.string().trim().min(1).max(2000) })
  .strict();
export const chatMessagesQuerySchema = z
  .object({
    cursor: uuidSchema.optional(),
    limit: z.coerce.number().int().min(1).max(100).default(50),
  })
  .strict();
export const createCommandDraftSchema = z
  .object({
    sourceMessageId: uuidSchema,
    command: createCommandSchema,
    confirmationSummary: z.string().trim().min(1).max(4000),
    expiresAt: z.iso.datetime().optional(),
  })
  .strict()
  .superRefine((value, context) => {
    if (value.command.metadata) {
      context.addIssue({
        code: "custom",
        path: ["command", "metadata"],
        message:
          "Command draft metadata is assigned by the server during confirmation",
      });
    }
  });
export const commandDraftSummarySchema = z
  .object({
    id: uuidSchema,
    intent: commandIntentSchema,
    confirmationSummary: z.string().min(1).max(4000),
    status: commandDraftStatusSchema,
    expiresAt: z.iso.datetime(),
    commandId: uuidSchema.nullable(),
  })
  .strict();
export const confirmCommandDraftSchema = z.object({}).strict();
export const rejectCommandDraftSchema = z.object({}).strict();
export const updateSmartCachePolicySchema = z
  .object({
    enabled: z.boolean(),
    quotaBytes: z.number().int().positive(),
    maxFileBytes: z.number().int().positive(),
    excludedPatterns: z.array(z.string().min(1).max(255)).max(100),
  })
  .strict()
  .refine(
    (value) => value.maxFileBytes <= value.quotaBytes,
    "maxFileBytes must not exceed quotaBytes",
  );
export const cacheCandidateSchema = z
  .object({
    sourceRelativePath: relativePathSchema,
    sourceVersion: z.record(z.string(), z.unknown()),
    sourceVersionHash: z.string().regex(/^[a-f0-9]{64}$/),
    sizeBytes: z.number().int().positive(),
    usageScore: z.number().int(),
    manualPin: z.boolean(),
  })
  .strict();
export const cacheCandidateBatchSchema = z
  .object({
    roomId: uuidSchema,
    candidates: z.array(cacheCandidateSchema).min(1).max(200),
  })
  .strict();
export const completeCacheUploadSchema = z
  .object({
    sizeBytes: z.number().int().positive(),
    sha256: z.string().regex(/^[a-f0-9]{64}$/),
    usageScore: z.number().int(),
    manualPin: z.boolean(),
  })
  .strict();

export const mutationHeadersSchema = z
  .object({
    idempotencyKey: idempotencyKeySchema,
  })
  .strict();
