import {
  createCommandSchema,
  createProposalSchema,
  createRoomSnapshotSchema,
  createRuleSchema,
  failFileTransferSchema,
  updateCharacterSchema,
  updateRuleSchema,
} from "./control-plane";

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
