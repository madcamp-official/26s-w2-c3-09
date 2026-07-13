import {
  createCommandSchema,
  createFileBrowseRequestSchema,
  createProposalSchema,
  createRoomSnapshotSchema,
  createRuleSchema,
  failFileTransferSchema,
  registerPushNotificationTokenSchema,
  updateCharacterSchema,
  updateRuleSchema,
  characterStateSchema,
  devicePairedEventPayloadSchema,
  deviceRevokedEventPayloadSchema,
  roomRemovedEventPayloadSchema,
  MOUSEKEEPER_CLEANLINESS_FORMULA_VERSION,
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
