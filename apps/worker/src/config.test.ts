import assert from "node:assert/strict";
import test from "node:test";
import { loadWorkerConfig } from "./config";

const base = {
  DATABASE_URL: "postgresql://housemouse@localhost:5432/housemouse",
  OBJECT_STORAGE_REGION: "ap-southeast-2",
  OBJECT_STORAGE_BUCKET: "housemouse-private",
  FILE_TRANSFER_TTL_SECONDS: "600",
};

test("worker accepts the EC2 instance role credential mode", () => {
  const config = loadWorkerConfig(base);
  assert.equal(config.OBJECT_STORAGE_ENDPOINT, undefined);
  assert.equal(config.OBJECT_STORAGE_ACCESS_KEY_ID, undefined);
  assert.equal(config.OBJECT_STORAGE_SECRET_ACCESS_KEY, undefined);
});

test("worker accepts a complete static credential pair", () => {
  const config = loadWorkerConfig({
    ...base,
    OBJECT_STORAGE_ENDPOINT: "https://objects.example.com",
    OBJECT_STORAGE_ACCESS_KEY_ID: "access-key",
    OBJECT_STORAGE_SECRET_ACCESS_KEY: "secret-key",
  });
  assert.equal(config.OBJECT_STORAGE_ENDPOINT, "https://objects.example.com");
  assert.equal(config.OBJECT_STORAGE_ACCESS_KEY_ID, "access-key");
});

test("worker rejects an incomplete static credential pair", () => {
  assert.throws(
    () =>
      loadWorkerConfig({
        ...base,
        OBJECT_STORAGE_SECRET_ACCESS_KEY: "secret-key",
      }),
    /UNCONFIGURED: OBJECT_STORAGE_ACCESS_KEY_ID/,
  );
});
