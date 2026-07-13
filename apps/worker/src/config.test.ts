import assert from "node:assert/strict";
import test from "node:test";
import { loadWorkerConfig } from "./config";

const base = {
  DATABASE_URL: "postgresql://mousekeeper@localhost:5432/mousekeeper",
  OBJECT_STORAGE_REGION: "ap-southeast-2",
  OBJECT_STORAGE_BUCKET: "mousekeeper-private",
  FILE_TRANSFER_TTL_SECONDS: "600",
  FCM_ENABLED: "false",
};

test("worker accepts the EC2 instance role credential mode", () => {
  const config = loadWorkerConfig(base);
  assert.equal(config.OBJECT_STORAGE_ENDPOINT, undefined);
  assert.equal(config.OBJECT_STORAGE_ACCESS_KEY_ID, undefined);
  assert.equal(config.OBJECT_STORAGE_SECRET_ACCESS_KEY, undefined);
});

test("worker rejects enabled FCM without credentials", () => {
  assert.throws(
    () => loadWorkerConfig({ ...base, FCM_ENABLED: "true" }),
    /UNCONFIGURED: FCM_ENABLED/,
  );
});

test("worker accepts enabled FCM with a service account path", () => {
  const config = loadWorkerConfig({
    ...base,
    FCM_ENABLED: "true",
    FIREBASE_SERVICE_ACCOUNT_PATH: "/etc/mousekeeper/firebase.json",
  });
  assert.equal(config.FCM_ENABLED, true);
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
