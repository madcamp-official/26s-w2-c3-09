import {
  createCommandSchema,
  commandIntentSchema,
  createFileBrowseRequestSchema,
  createProposalSchema,
  createRoomSnapshotSchema,
  createRuleSchema,
  failFileTransferSchema,
  connectionSummaryResponseSchema,
  homeSummaryResponseSchema,
  registerPushNotificationTokenSchema,
  updateCharacterSchema,
  updateRuleSchema,
  characterStateSchema,
  devicePairedEventPayloadSchema,
  deviceRevokedEventPayloadSchema,
  roomRemovedEventPayloadSchema,
  MOUSEKEEPER_CLEANLINESS_FORMULA_VERSION,
  chatMessagesQuerySchema,
  completeCacheUploadSchema,
  createChatMessageResponseSchema,
  commandDraftSummarySchema,
  createChatSessionSchema,
  createCommandDraftSchema,
  updateChatSessionSchema,
  createRuleDraftRequestSchema,
  confirmRuleDraftResponseSchema,
  rejectRuleDraftResponseSchema,
  ruleDraftSummarySchema,
  ruleDraftResultSchema,
} from "./control-plane";

declare function require(path: string): unknown;

describe("push notification token contract", () => {
  it("accepts only bounded mobile provider tokens", () => {
    expect(
      registerPushNotificationTokenSchema.safeParse({
        token: "provider-token-with-enough-entropy",
        platform: "ANDROID",
      }).success,
    ).toBe(true);
    expect(
      registerPushNotificationTokenSchema.safeParse({
        token: "short",
        platform: "ANDROID",
      }).success,
    ).toBe(false);
    expect(
      registerPushNotificationTokenSchema.safeParse({
        token: "provider-token-with-enough-entropy",
        platform: "WINDOWS",
      }).success,
    ).toBe(false);
  });
});

describe("smart cache contracts", () => {
  it("requires encrypted completion metadata without key material", () => {
    const completion = {
      sizeBytes: 132,
      sha256: "a".repeat(64),
      usageScore: 10,
      manualPin: false,
      encryptionMetadata: {
        algorithm: "AES-256-GCM",
        format: "MKS1_NONCE_CIPHERTEXT_TAG",
        keyId: "key-20260714-abcdef",
        nonceHex: "b".repeat(24),
        plaintextSizeBytes: 100,
        plaintextSha256: "c".repeat(64),
      },
    };

    expect(completeCacheUploadSchema.parse(completion)).toEqual(completion);
    expect(
      completeCacheUploadSchema.safeParse({
        ...completion,
        encryptionMetadata: undefined,
      }).success,
    ).toBe(false);
    expect(
      completeCacheUploadSchema.safeParse({
        ...completion,
        encryptionMetadata: {
          ...completion.encryptionMetadata,
          key: "raw-secret",
        },
      }).success,
    ).toBe(false);
  });
});

describe("rule contracts", () => {
  it("accepts a deterministic extension move rule", () => {
    expect(
      createRuleSchema.safeParse({
        name: "PDF archive",
        definition: {
          match: "ALL",
          conditions: [{ field: "extension", operator: "IN", value: [".pdf"] }],
          action: { type: "MOVE", destinationTemplate: "Archive/PDF" },
        },
        priority: 100,
        enabled: true,
      }).success,
    ).toBe(true);
  });

  it("accepts additive rule DSL conditions and safe actions", () => {
    expect(
      createRuleSchema.safeParse({
        name: "Large old reports",
        definition: {
          match: "ALL",
          conditions: [
            { field: "modifiedAgeDays", operator: "GTE", value: 30 },
            { field: "createdAgeDays", operator: "GT", value: 7 },
            { field: "sizeBytes", operator: "LTE", value: 20_000_000 },
            {
              field: "relativePath",
              operator: "STARTS_WITH",
              value: "reports",
            },
            { field: "fileKind", operator: "EQ", value: "FILE" },
            { field: "name", operator: "CONTAINS", value: "draft" },
          ],
          action: { type: "TRASH" },
        },
        priority: 100,
        enabled: true,
      }).success,
    ).toBe(true);

    expect(
      createRuleSchema.safeParse({
        name: "Create archive directory",
        definition: {
          conditions: [
            { field: "relativePath", operator: "STARTS_WITH", value: "inbox" },
          ],
          action: { type: "CREATE_DIR", relativePath: "Archive/Reports" },
        },
        priority: 101,
      }).success,
    ).toBe(true);
  });

  it("rejects an unsafe destination and an empty version-only update", () => {
    const unsafe = createRuleSchema.safeParse({
      name: "Unsafe",
      definition: {
        conditions: [{ field: "ageDays", operator: "GTE", value: 30 }],
        action: { type: "MOVE", destinationTemplate: "../outside" },
      },
      priority: 1,
    });
    expect(unsafe.success).toBe(false);
    expect(updateRuleSchema.safeParse({ version: 1 }).success).toBe(false);
  });

  it("rejects unsafe expanded rule DSL paths and unsupported operators", () => {
    expect(
      createRuleSchema.safeParse({
        name: "Unsafe prefix",
        definition: {
          conditions: [
            {
              field: "relativePath",
              operator: "STARTS_WITH",
              value: "../outside",
            },
          ],
          action: { type: "TRASH" },
        },
        priority: 1,
      }).success,
    ).toBe(false);
    expect(
      createRuleSchema.safeParse({
        name: "Unsupported size operator",
        definition: {
          conditions: [{ field: "sizeBytes", operator: "GT", value: 1024 }],
          action: { type: "CREATE_DIR", relativePath: "Archive" },
        },
        priority: 1,
      }).success,
    ).toBe(false);
  });
});

describe("rule draft contracts", () => {
  it("validates natural-language rule draft requests and explicit provider states", () => {
    expect(
      createRuleDraftRequestSchema.parse({
        instruction: "오래된 PDF는 Archive로 보내줘",
      }),
    ).toEqual({ instruction: "오래된 PDF는 Archive로 보내줘" });
    expect(
      createRuleDraftRequestSchema.safeParse({ instruction: "" }).success,
    ).toBe(false);
    expect(
      ruleDraftResultSchema.parse({
        status: "UNCONFIGURED",
        code: "AI_PROVIDER_UNCONFIGURED",
      }),
    ).toEqual({
      status: "UNCONFIGURED",
      code: "AI_PROVIDER_UNCONFIGURED",
    });
    expect(
      ruleDraftResultSchema.parse({
        status: "READY",
        kind: "RULE_DRAFT",
        draft: {
          id: "018f4c7b-1ad6-7c95-bf34-5e45881f98a5",
          roomId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a6",
          name: "Old PDFs",
          definition: {
            match: "ALL",
            conditions: [
              { field: "modifiedAgeDays", operator: "GTE", value: 30 },
              { field: "extension", operator: "IN", value: [".pdf"] },
            ],
            action: { type: "MOVE", destinationTemplate: "Archive/PDF" },
          },
          explanation: "30일 이상 수정되지 않은 PDF를 보관 폴더로 이동합니다.",
          ambiguities: [],
          status: "DRAFT",
          expiresAt: "2026-07-13T10:00:00.000Z",
          ruleId: null,
        },
      }).status,
    ).toBe("READY");
  });
});

describe("rule draft lifecycle response contracts", () => {
  it("validates rule draft confirm and reject responses", () => {
    const draft = {
      id: "018f4c7b-1ad6-7c95-bf34-5e45881f98a5",
      roomId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a6",
      name: "Old PDFs",
      definition: {
        match: "ALL",
        conditions: [{ field: "extension", operator: "IN", value: [".pdf"] }],
        action: { type: "MOVE", destinationTemplate: "Archive/PDF" },
      },
      explanation: "Move old PDFs after explicit approval.",
      ambiguities: [],
      status: "MATERIALIZED",
      expiresAt: "2026-07-13T10:00:00.000Z",
      ruleId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a7",
    };
    const rule = {
      id: draft.ruleId,
      roomId: draft.roomId,
      name: draft.name,
      definition: draft.definition,
      priority: 100,
      enabled: true,
      version: 1,
      createdAt: "2026-07-13T10:00:00.000Z",
      updatedAt: "2026-07-13T10:00:00.000Z",
    };

    expect(ruleDraftSummarySchema.parse(draft)).toEqual(draft);
    expect(confirmRuleDraftResponseSchema.parse({ draft, rule })).toEqual({
      draft,
      rule,
    });
    expect(
      rejectRuleDraftResponseSchema.parse({
        draft: { ...draft, status: "REJECTED", ruleId: null },
      }).draft.status,
    ).toBe("REJECTED");
  });
});

describe("room snapshot contract", () => {
  it("validates the shared v1.4 snapshot fixture without changing it", () => {
    const fixture = require("../fixtures/room-snapshot-v1.json");

    expect(createRoomSnapshotSchema.parse(fixture)).toEqual(fixture);
  });

  it("rejects file counts larger than the total", () => {
    const result = createRoomSnapshotSchema.safeParse({
      score: 75,
      calculatedAt: new Date().toISOString(),
      metrics: {
        totalFileCount: 10,
        managedFileCount: 11,
        unorganizedFileCount: 2,
        deductions: [],
      },
    });
    expect(result.success).toBe(false);
  });

  it("defaults legacy snapshots to the current formula version", () => {
    const result = createRoomSnapshotSchema.parse({
      score: 100,
      calculatedAt: new Date().toISOString(),
      metrics: {
        totalFileCount: 1,
        managedFileCount: 1,
        unorganizedFileCount: 0,
        deductions: [],
      },
    });

    expect(result.formulaVersion).toBe(MOUSEKEEPER_CLEANLINESS_FORMULA_VERSION);
  });

  it("rejects snapshots produced by an unknown formula", () => {
    expect(
      createRoomSnapshotSchema.safeParse({
        score: 100,
        calculatedAt: new Date().toISOString(),
        formulaVersion: "mousekeeper-cleanliness-v0",
        metrics: {
          totalFileCount: 1,
          managedFileCount: 1,
          unorganizedFileCount: 0,
          deductions: [],
        },
      }).success,
    ).toBe(false);
  });
});

describe("v1.4 connection and browse contracts", () => {
  it("accepts CONNECTING only as a known additive character state", () => {
    expect(characterStateSchema.parse("CONNECTING")).toBe("CONNECTING");
  });

  it("keeps legacy browse requests compatible and normalizes search input", () => {
    expect(
      createFileBrowseRequestSchema.parse({ relativeDirectory: "Documents" }),
    ).toEqual({
      relativeDirectory: "Documents",
      cursor: null,
      query: null,
      searchScope: "CURRENT_DIRECTORY",
    });
    expect(
      createFileBrowseRequestSchema.parse({
        relativeDirectory: "Documents",
        query: "  report  ",
        searchScope: "MANAGED_ROOT",
      }).query,
    ).toBe("report");
    expect(
      createFileBrowseRequestSchema.safeParse({
        relativeDirectory: "Documents",
        query: "   ",
      }).success,
    ).toBe(false);
    expect(
      createFileBrowseRequestSchema.safeParse({
        relativeDirectory: "Documents",
        query: "r",
      }).success,
    ).toBe(false);
    expect(
      createFileBrowseRequestSchema.safeParse({
        relativeDirectory: "Documents",
        query: "🐭",
      }).success,
    ).toBe(false);
    expect(
      createFileBrowseRequestSchema.safeParse({
        relativeDirectory: "x".repeat(1025),
      }).success,
    ).toBe(false);
    expect(
      createFileBrowseRequestSchema.safeParse({
        relativeDirectory: "..\\outside",
      }).success,
    ).toBe(false);

    const browseJsonSchema = require("../events/file-browse.schema.json") as {
      $defs: { SafeRelativePathOrRoot: { pattern: string } };
    };
    const jsonPathPattern = new RegExp(
      browseJsonSchema.$defs.SafeRelativePathOrRoot.pattern,
    );
    expect(
      ["", "Documents", "Documents/reports"].every((path) =>
        jsonPathPattern.test(path),
      ),
    ).toBe(true);
    expect(
      [".", "..", "a/./b", "a/../b", "a//b", "a/", "/a", "C:a", "a\0b"].some(
        (path) => jsonPathPattern.test(path),
      ),
    ).toBe(false);
  });

  it("validates the shared filename-search fixture", () => {
    const fixture = require("../fixtures/file-browse-search-request.json");

    expect(createFileBrowseRequestSchema.parse(fixture)).toEqual(fixture);
  });

  it("requires aggregate ids and final lifecycle states", () => {
    const deviceId = "33333333-3333-4333-8333-333333333333";
    const roomId = "22222222-2222-4222-8222-222222222222";

    expect(
      devicePairedEventPayloadSchema.parse({ deviceId, status: "ACTIVE" }),
    ).toEqual({ deviceId, status: "ACTIVE" });
    expect(
      deviceRevokedEventPayloadSchema.parse({ deviceId, status: "REVOKED" }),
    ).toEqual({ deviceId, status: "REVOKED" });
    expect(
      roomRemovedEventPayloadSchema.parse({ roomId, status: "REMOVED" }),
    ).toEqual({ roomId, status: "REMOVED" });
    expect(
      deviceRevokedEventPayloadSchema.safeParse({
        deviceId,
        status: "ACTIVE",
      }).success,
    ).toBe(false);
  });
});

describe("character contract", () => {
  it("rejects an empty preference update", () => {
    expect(updateCharacterSchema.safeParse({}).success).toBe(false);
  });
});

describe("command contracts", () => {
  const rootId = "root:downloads";

  it("accepts a bounded structured README request", () => {
    expect(
      createCommandSchema.safeParse({
        intent: "README",
        payload: {
          purpose: "Team mobile application",
          audience: "New contributors",
          tone: "concise",
          sections: ["Setup", "Run"],
        },
      }).success,
    ).toBe(true);
  });

  it("accepts safe file command payloads without shell-shaped escape hatches", () => {
    expect(commandIntentSchema.parse("RENAME")).toBe("RENAME");
    expect(
      createCommandSchema.parse({
        intent: "RENAME",
        payload: {
          rootId,
          sourceRelativePath: "reports/old.pdf",
          newName: "final.pdf",
        },
        metadata: {
          sessionId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a1",
          sourceMessageId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a2",
          idempotencyKey: "rename-018f4c7b",
          requiresApproval: true,
        },
      }),
    ).toEqual({
      intent: "RENAME",
      payload: {
        rootId,
        sourceRelativePath: "reports/old.pdf",
        newName: "final.pdf",
      },
      metadata: {
        sessionId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a1",
        sourceMessageId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a2",
        idempotencyKey: "rename-018f4c7b",
        requiresApproval: true,
      },
    });
    expect(
      createCommandSchema.safeParse({
        intent: "MOVE",
        payload: {
          rootId,
          sourceRelativePaths: ["reports/final.pdf", "reports/notes.txt"],
          destinationRelativeDirectory: "Archive",
        },
      }).success,
    ).toBe(true);
    expect(
      createCommandSchema.safeParse({
        intent: "FIND",
        payload: {
          rootId,
          query: "invoice",
          extensions: [".pdf"],
          scopeRelativePath: "",
          limit: 50,
        },
      }).success,
    ).toBe(true);
    expect(
      createCommandSchema.safeParse({
        intent: "UPLOAD",
        payload: {
          rootId,
          destinationRelativePath: "incoming/photo.png",
          transferId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a3",
          expectedSha256:
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          expectedSize: 42,
        },
      }).success,
    ).toBe(true);
  });

  it("rejects unsafe file command paths, reserved names, and oversized searches", () => {
    expect(
      createCommandSchema.safeParse({
        intent: "RENAME",
        payload: {
          rootId,
          sourceRelativePath: "../outside.txt",
          newName: "safe.txt",
        },
      }).success,
    ).toBe(false);
    expect(
      createCommandSchema.safeParse({
        intent: "RENAME",
        payload: {
          rootId,
          sourceRelativePath: "reports/old.pdf",
          newName: "nested/final.pdf",
        },
      }).success,
    ).toBe(false);
    expect(
      createCommandSchema.safeParse({
        intent: "CREATE",
        payload: {
          rootId,
          kind: "FILE",
          relativePath: "CON.txt",
          content: "blocked reserved Windows name",
        },
      }).success,
    ).toBe(false);
    expect(
      createCommandSchema.safeParse({
        intent: "FIND",
        payload: {
          rootId,
          query: "x",
          limit: 201,
        },
      }).success,
    ).toBe(false);
  });

  it("rejects arbitrary payloads and invalid rule drafts", () => {
    expect(
      createCommandSchema.safeParse({
        intent: "ANALYZE",
        payload: { shell: "do-not-run" },
      }).success,
    ).toBe(false);
    expect(
      createCommandSchema.safeParse({
        intent: "CREATE_RULE",
        payload: {
          rule: {
            name: "Unsafe",
            definition: {
              conditions: [{ field: "ageDays", operator: "GTE", value: 1 }],
              action: { type: "MOVE", destinationTemplate: "../outside" },
            },
            priority: 1,
          },
        },
      }).success,
    ).toBe(false);
  });
});

describe("file transfer failure contract", () => {
  it("accepts only explicit safe failure codes", () => {
    expect(
      failFileTransferSchema.safeParse({ failureCode: "SOURCE_CHANGED" })
        .success,
    ).toBe(true);
    expect(
      failFileTransferSchema.safeParse({
        failureCode: "ARBITRARY_PROVIDER_ERROR",
      }).success,
    ).toBe(false);
  });
});

describe("chat session contracts", () => {
  it("validates session mutations and bounded message pagination", () => {
    expect(
      createChatSessionSchema.parse({ title: "  Inbox cleanup  " }),
    ).toEqual({ title: "Inbox cleanup" });
    expect(updateChatSessionSchema.safeParse({}).success).toBe(false);
    expect(updateChatSessionSchema.parse({ title: "Reports" })).toEqual({
      title: "Reports",
    });
    expect(chatMessagesQuerySchema.parse({ limit: "25" })).toEqual({
      limit: 25,
    });
    expect(chatMessagesQuerySchema.safeParse({ limit: "101" }).success).toBe(
      false,
    );
  });

  it("validates chat message responses with explicit AI unconfigured status", () => {
    const response = {
      message: {
        id: "018f4c7b-1ad6-7c95-bf34-5e45881f98a1",
        roomId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a2",
        sessionId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a3",
        senderType: "USER",
        messageType: "TEXT",
        content: "Rename the old report",
        structuredPayload: null,
        commandId: null,
        createdAt: "2026-07-13T01:02:03.000Z",
      },
      assistant: null,
      aiStatus: "UNCONFIGURED",
      ai: {
        status: "UNCONFIGURED",
        code: "AI_PROVIDER_UNCONFIGURED",
      },
    };

    expect(createChatMessageResponseSchema.parse(response)).toEqual(response);
    const draftResponse = {
      ...response,
      assistant: {
        ...response.message,
        id: "018f4c7b-1ad6-7c95-bf34-5e45881f98a4",
        senderType: "ASSISTANT",
        messageType: "COMMAND_DRAFT",
        content: "Rename reports/old.pdf to reports/final.pdf",
        structuredPayload: {
          id: "018f4c7b-1ad6-7c95-bf34-5e45881f98a5",
          intent: "RENAME",
          confirmationSummary: "Rename reports/old.pdf to reports/final.pdf",
          status: "DRAFT",
          expiresAt: "2026-07-13T01:12:03.000Z",
          commandId: null,
        },
      },
      aiStatus: "READY",
      ai: {
        status: "READY",
        kind: "COMMAND_DRAFT",
        commandDraftId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a5",
      },
    };
    expect(createChatMessageResponseSchema.parse(draftResponse)).toEqual(
      draftResponse,
    );
    expect(
      createChatMessageResponseSchema.safeParse({
        ...response,
        assistant: {
          ...response.message,
          senderType: "ASSISTANT",
          content: "Pretend success",
        },
        aiStatus: "READY",
      }).success,
    ).toBe(false);
  });

  it("validates command draft summaries without command arguments or secrets", () => {
    const draft = {
      id: "018f4c7b-1ad6-7c95-bf34-5e45881f98a1",
      intent: "RENAME",
      confirmationSummary: "Rename reports/old.pdf to final.pdf",
      status: "DRAFT",
      expiresAt: "2026-07-13T01:02:03.000Z",
      commandId: null,
    };

    expect(commandDraftSummarySchema.parse(draft)).toEqual(draft);
    expect(
      commandDraftSummarySchema.safeParse({ ...draft, arguments: {} }).success,
    ).toBe(false);
  });

  it("validates command draft creation while reserving metadata for the server", () => {
    const sourceMessageId = "018f4c7b-1ad6-7c95-bf34-5e45881f98a2";
    expect(
      createCommandDraftSchema.safeParse({
        sourceMessageId,
        command: {
          intent: "RENAME",
          payload: {
            rootId: "root:downloads",
            sourceRelativePath: "reports/old.pdf",
            newName: "final.pdf",
          },
        },
        confirmationSummary: "Rename reports/old.pdf to final.pdf",
      }).success,
    ).toBe(true);
    expect(
      createCommandDraftSchema.safeParse({
        sourceMessageId,
        command: {
          intent: "RENAME",
          payload: {
            rootId: "root:downloads",
            sourceRelativePath: "reports/old.pdf",
            newName: "final.pdf",
          },
          metadata: {
            idempotencyKey: "client-key",
          },
        },
        confirmationSummary: "Rename reports/old.pdf to final.pdf",
      }).success,
    ).toBe(false);
  });
});

describe("proposal contract", () => {
  it("rejects duplicate source files in one proposal", () => {
    const item = {
      itemOrder: 0,
      actionType: "QUARANTINE" as const,
      sourceRelativePath: "reports/old.pdf",
      destinationRelativePath: null,
      reasonCode: "AGE_RULE_MATCH",
      precondition: {},
      conflictState: "NONE" as const,
    };
    const result = createProposalSchema.safeParse({
      commandId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a1",
      roomId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a2",
      summary: {},
      expiresAt: null,
      items: [item, { ...item, itemOrder: 1 }],
    });
    expect(result.success).toBe(false);
  });
});

describe("home summary contract", () => {
  const deviceId = "018f4c7b-1ad6-7c95-bf34-5e45881f98a1";
  const roomId = "018f4c7b-1ad6-7c95-bf34-5e45881f98a2";
  const characterId = "018f4c7b-1ad6-7c95-bf34-5e45881f98a3";
  const timestamp = "2026-07-13T01:02:03.000Z";

  it("validates the aggregated mobile home response", () => {
    const response = {
      devices: [
        {
          id: deviceId,
          platform: "WINDOWS",
          deviceName: "Desktop",
          status: "ACTIVE",
          lastSeenAt: timestamp,
          createdAt: timestamp,
          presence: "ONLINE_IDLE",
        },
      ],
      rooms: [
        {
          id: roomId,
          desktopDeviceId: deviceId,
          name: "Downloads",
          rootAlias: "Downloads",
          status: "ACTIVE",
          createdAt: timestamp,
          pendingProposalCount: 1,
          latestExecutionStatus: "SUCCEEDED",
          cleanlinessScore: 87,
          cleanlinessFormulaVersion: "mousekeeper-cleanliness-v2",
          cleanlinessCalculatedAt: timestamp,
        },
      ],
      character: {
        id: characterId,
        appearance: {},
        roomTheme: null,
        affinityTotal: 2,
        createdAt: timestamp,
        updatedAt: timestamp,
        unlockedItems: ["fur:brown"],
        nextUnlockAffinity: 3,
        riveAssetStatus: "UNCONFIGURED",
      },
    };

    expect(homeSummaryResponseSchema.parse(response)).toEqual(response);
  });

  it("rejects private database fields in the public response", () => {
    expect(
      homeSummaryResponseSchema.safeParse({
        devices: [],
        rooms: [],
        character: {
          id: characterId,
          userId: deviceId,
          appearance: {},
          roomTheme: null,
          affinityTotal: 0,
          createdAt: timestamp,
          updatedAt: timestamp,
          unlockedItems: [],
          nextUnlockAffinity: 3,
          riveAssetStatus: "UNCONFIGURED",
        },
      }).success,
    ).toBe(false);
  });
});

describe("connection summary contract", () => {
  const deviceId = "018f4c7b-1ad6-7c95-bf34-5e45881f98a1";
  const roomId = "018f4c7b-1ad6-7c95-bf34-5e45881f98a2";
  const timestamp = "2026-07-13T01:02:03.000Z";

  it("keeps the five-second safety check scoped to active connection data", () => {
    const response = {
      devices: [
        {
          id: deviceId,
          platform: "WINDOWS",
          deviceName: "Desktop",
          status: "ACTIVE",
          lastSeenAt: timestamp,
          createdAt: timestamp,
        },
      ],
      rooms: [
        {
          id: roomId,
          desktopDeviceId: deviceId,
          name: "Downloads",
          rootAlias: "Downloads",
          status: "ACTIVE",
          createdAt: timestamp,
        },
      ],
    };

    expect(connectionSummaryResponseSchema.parse(response)).toEqual(response);
  });

  it("rejects home projection fields and private database fields", () => {
    expect(
      connectionSummaryResponseSchema.safeParse({
        devices: [
          {
            id: deviceId,
            platform: "WINDOWS",
            deviceName: "Desktop",
            status: "ACTIVE",
            lastSeenAt: timestamp,
            createdAt: timestamp,
            presence: "ONLINE_IDLE",
          },
        ],
        rooms: [],
      }).success,
    ).toBe(false);
    expect(
      connectionSummaryResponseSchema.safeParse({
        devices: [
          {
            id: deviceId,
            userId: roomId,
            platform: "WINDOWS",
            deviceName: "Desktop",
            status: "ACTIVE",
            lastSeenAt: timestamp,
            createdAt: timestamp,
          },
        ],
        rooms: [],
      }).success,
    ).toBe(false);
  });
});
