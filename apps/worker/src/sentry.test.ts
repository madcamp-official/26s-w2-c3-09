import assert from "node:assert/strict";
import test from "node:test";
import type { ErrorEvent } from "@sentry/node";
import { initializeWorkerSentry, scrubWorkerSentryEvent } from "./sentry";

test("worker Sentry stays disabled or fails explicit invalid config", () => {
  assert.equal(initializeWorkerSentry({}), false);
  assert.throws(
    () => initializeWorkerSentry({ SENTRY_DSN: "invalid" }),
    /UNCONFIGURED: SENTRY_DSN/,
  );
});

test("worker Sentry removes provider paths and payload context", () => {
  const scrubbed = scrubWorkerSentryEvent({
    message: "delete failed for /tmp/private-object",
    request: { url: "https://object.test/?token=secret" },
    extra: { objectKey: "private/key" },
  } as unknown as ErrorEvent);
  const encoded = JSON.stringify(scrubbed);
  assert.equal(encoded.includes("private-object"), false);
  assert.equal(encoded.includes("objectKey"), false);
});
