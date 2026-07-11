import { eventEnvelopeSchema, relativePathSchema } from "./common";

describe("relativePathSchema", () => {
  it.each([
    "",
    "C:\\Users\\file.txt",
    "/etc/passwd",
    "../secret",
    "safe/../secret",
    "safe/./file.txt",
    "safe//file.txt",
  ])("rejects unsafe path %s", (value) =>
    expect(relativePathSchema.safeParse(value).success).toBe(false),
  );

  it("accepts a managed-root-relative path", () => {
    expect(relativePathSchema.parse("Archive/PDF/file.pdf")).toBe(
      "Archive/PDF/file.pdf",
    );
  });
});

describe("eventEnvelopeSchema", () => {
  it("requires version and correlation metadata", () => {
    const base = {
      eventId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a1",
      eventType: "command.updated",
      aggregateType: "command",
      aggregateId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a2",
      correlationId: "018f4c7b-1ad6-7c95-bf34-5e45881f98a2",
      schemaVersion: 1,
      deviceId: null,
      roomId: null,
      sequence: 1,
      occurredAt: new Date().toISOString(),
      payload: {},
    };
    expect(eventEnvelopeSchema.safeParse(base).success).toBe(true);
    const { correlationId: _, ...withoutCorrelation } = base;
    expect(eventEnvelopeSchema.safeParse(withoutCorrelation).success).toBe(
      false,
    );
  });
});
