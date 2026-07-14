# E2E Scenarios

## Release preflight

Run the preflight before claiming a release E2E pass:

```powershell
$env:MOUSEKEEPER_API_URL = "https://mousekeeper.madcamp-kaist.org"
pnpm e2e:preflight
```

For local development while the working tree is intentionally dirty:

```powershell
$env:MOUSEKEEPER_API_URL = "http://127.0.0.1:3000"
pnpm e2e:preflight -- -AllowDirty -RunLocalChecks
```

The preflight checks only prerequisites and never creates fake success data. A
missing API URL, Android device, `adb`, `flutter`, `cargo`, `/health`, or
`/ready` is reported as `UNCONFIGURED` or `FAIL`, and the scenarios below must
not be marked passed until those blockers are resolved.

This document defines repeatable end-to-end checks for MouseKeeper. It is a
checklist, not proof that a scenario has passed. Record the command output,
device IDs, room IDs, screenshots, and server logs in the release notes when a
scenario is executed.

## Shared prerequisites

- API server is reachable at the intended base URL.
- PostgreSQL migrations are applied.
- Redis/Valkey is reachable and `/ready` returns success.
- Android app uses package `com.mousekeeper.app`.
- Desktop agent is paired with the same Firebase user as the Android app.
- The managed root is a disposable fixture folder, not a real user folder.
- Object storage is configured for transfer/smart-cache scenarios.
- Missing providers remain explicit `UNCONFIGURED`; do not substitute local
  mock success paths.

## Scenario 1 — pairing and presence

Goal: prove mobile, server, Redis presence, Socket.IO, and desktop device token
work together.

1. Start the API server, worker, mobile app, and desktop app.
2. Start desktop pairing and enter the code on mobile.
3. Confirm the mobile Home screen shows the newly paired desktop.
4. Confirm desktop heartbeat changes Redis presence to an `ONLINE_*` value.
5. Stop the desktop process without revoking the device.
6. Confirm presence falls back to `OFFLINE` after TTL expiry.
7. Restart desktop and confirm the device returns online without creating a new
   room/device row.

Pass evidence:

- Pairing session status transitions to `CLAIMED`.
- Mobile Home updates from WebSocket or replay without full refresh polling.
- `presence.updated` carries only device-scoped payload.

## Scenario 2 — command to proposal to safe execution

Goal: prove a user-visible file-changing command still goes through approval,
precondition, journal, no-overwrite execution, and result sync.

1. Create a fixture managed root with at least three files and one destination
   conflict.
2. Send a supported mobile/server command for that room.
3. Wait for desktop command polling or realtime wakeup.
4. Confirm desktop creates a proposal without mutating files.
5. Approve the proposal on mobile.
6. Confirm desktop revalidates source identity, root boundary, symlink/reparse
   safety, and destination conflicts.
7. Confirm operation journal rows are written before mutation.
8. Confirm successful items move/quarantine/create safely and conflicted items
   are skipped without overwrite.
9. Confirm `execution.updated` appears on mobile by targeted upsert.
10. Run undo and verify only journaled created/moved paths are reversed.

Pass evidence:

- No file changes occur before approval.
- No absolute paths appear in API responses, Sentry events, or request logs.
- Duplicate approval with the same idempotency key does not execute twice.

## Scenario 3 — online browse and verified download

Goal: prove read-only file access is separated from file mutation.

1. Register a managed root containing nested folders and searchable filenames.
2. Browse the root from mobile.
3. Search with a two-character-or-longer query.
4. Request a file download.
5. Confirm desktop validates the path and source version before streaming.
6. Confirm mobile writes to a temporary `.part` path.
7. Confirm final save and ACK happen only after SHA-256 verification.
8. Expire or cancel a transfer and verify object cleanup is retried by worker.

Pass evidence:

- Browse rejects traversal, absolute, UNC, and drive-prefixed paths.
- Download does not create an operation journal entry.
- Mobile does not POST `DOWNLOAD_COMPLETED` before checksum verification.

## Scenario 4 — device and room disconnect

Goal: prove normal disconnect does not wait for heartbeat TTL.

1. Start from an active paired device with at least one active room.
2. Revoke the device from mobile.
3. Confirm server transaction marks device and bound rooms inactive.
4. Confirm server writes durable sync events before publishing.
5. Confirm desktop receives `device.revoked`, clears local device binding, and
   switches to pairing.
6. Confirm mobile removes the device and room from the active gate.
7. Re-pair the same desktop and register the same local folder.

Pass evidence:

- The re-paired folder receives a new room ID.
- Stale Drift cache cannot reopen removed room/device state.
- Original user files and `.mousekeeper_trash` are not deleted.

## Scenario 5 — offline replay and outbox recovery

Goal: prove missed Socket.IO events and temporary network failures converge via
durable replay/outbox paths.

1. Pair mobile and desktop.
2. Stop the mobile network or block Socket.IO while server state changes.
3. Re-enable network and confirm `/v1/sync/events` replays missed events.
4. Queue a mobile mutation while offline.
5. Restore network and confirm the mutation flushes once.
6. Restart the desktop during a pending command and confirm local outbox/replay
   resumes without corrupting journal state.

Pass evidence:

- Replay cursor advances monotonically per user/device.
- Old account events are discarded after logout/login account switch.
- Background refresh failure does not replace a usable cached screen with a
  full error page.

## Scenario 6 — smart-cache opt-in lifecycle

Goal: prove smart-cache remains explicit, encrypted, quota-bound, and stale-aware.

1. Enable smart-cache for one room.
2. Produce local usage events and run desktop smart-cache candidate processing.
3. Confirm server quota reservation approves only policy-compliant candidates.
4. Confirm desktop encrypts before signed PUT.
5. Confirm server stores ciphertext size/checksum and encryption metadata, not
   plaintext or raw key material.
6. Confirm mobile lists available cache metadata.
7. Modify the source file on desktop and confirm `smart-cache.updated` marks the
   affected item stale without Home summary reload.
8. Disable smart-cache and confirm object deletion jobs retry until complete.

Pass evidence:

- Feature remains opt-in.
- Mobile plaintext handoff verifies the encrypted object only after a real
  synced key is available; without key sync it remains
  `UNCONFIGURED: SMART_CACHE_DECRYPTION_KEY_SYNC`.
- Quota/LRU deletion does not remove unrelated room objects.

## Release evidence template

```text
Date:
Branch/commit:
API URL:
Android build:
Desktop build:
PostgreSQL migration revision:
Redis URL class: local | staging | production
Object storage bucket:
Scenario IDs run:
Pass/fail summary:
Attached logs/screenshots:
Known follow-ups:
```
