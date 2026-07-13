# Offline File Cache Plan

## Goal

When the desktop is offline, mobile should still show the last known file view for a room and clearly distinguish:

- last known folder listings
- files that are actually downloadable while the desktop is offline
- files that may be stale because the desktop has not revalidated them

This is not full folder sync. It is a bounded offline view plus optional cached file downloads.

## Current Gap

The current Files flow is request/response:

1. Mobile creates a file browse request.
2. Server queues it for the desktop.
3. Desktop reads the managed root and completes the request.
4. Mobile displays the returned page.

If the desktop is powered off, the server returns `DEVICE_OFFLINE`. The server does not currently keep a durable last-known file browse page, so mobile has nothing authoritative to show for a folder it did not already hold in memory.

Smart cache is separate. It can make selected file contents downloadable offline, but it does not currently provide a full folder browser fallback.

## Ownership

### A Responsibilities

A owns desktop-local correctness and source validation:

- produce safe managed-root-relative file browse pages
- include a desktop generation/source version for each page
- upload smart-cache file bodies only after server reservation
- verify source file identity, size, mtime, and checksum before upload
- report source changes so server can mark cached metadata stale
- keep local watcher/index behavior safe inside managed roots

Primary A paths:

- `apps/desktop/src-tauri/`
- `tools/file-engine-cli/`
- `test-fixtures/file-trees/`
- `test-fixtures/file-transfers/`
- `test-fixtures/smart-cache/usage-events/`

### B Responsibilities

B owns server/mobile offline experience:

- persist last-known browse pages or folder entries on the server
- expose fallback browse responses when desktop is offline
- show cached/stale/offline states in mobile Files UI
- support offline-downloadable cached files
- enforce quota, lifecycle cleanup, revoke, and deletion behavior

Primary B paths:

- `apps/server/`
- `apps/mobile/`
- `apps/worker/`
- `infra/`

### Shared Contract Responsibilities

Shared changes should be additive and coordinated:

- `packages/contracts/`
- `docs/`

Avoid breaking existing `file-browse` and `file-transfer` response fields.

## Proposed States

Use explicit display/download states instead of pretending the desktop is online.

- `LIVE`: returned by desktop during an online browse request.
- `LAST_KNOWN`: server/mobile fallback from the most recent successful browse page.
- `UNVERIFIED_OFFLINE`: last known metadata exists, but desktop is offline so freshness is unknown.
- `STALE`: desktop or watcher reported source changed after this metadata/cache object was produced.
- `AVAILABLE`: file body is cached in object storage and downloadable.
- `NOT_CACHED`: metadata may be visible, but file body is not downloadable while offline.

`AVAILABLE` must not mean fresh. Freshness and availability are separate.

## Contract Shape

Add fields to file browse responses rather than replacing existing fields:

```json
{
  "status": "READY",
  "source": "LIVE",
  "desktopGeneration": "root:123",
  "resultPage": {
    "entries": [
      {
        "name": "report.pdf",
        "relativePath": "docs/report.pdf",
        "type": "FILE",
        "sizeBytes": 1234,
        "modifiedAt": "2026-07-13T00:00:00.000Z",
        "fileId": "stable-local-file-id",
        "freshnessStatus": "UNVERIFIED_OFFLINE",
        "cacheStatus": "AVAILABLE",
        "cachedFileId": "optional-server-id"
      }
    ],
    "nextCursor": null
  }
}
```

Recommended additive enums:

- `source`: `LIVE | LAST_KNOWN`
- `freshnessStatus`: `FRESH | UNVERIFIED_OFFLINE | STALE`
- `cacheStatus`: `NOT_CACHED | AVAILABLE`

## Server Plan

1. Persist last successful browse pages.
   - Key by `roomId`, `relativeDirectory`, and cursor/page scope.
   - Store only managed-root-relative metadata.
   - Do not store absolute desktop paths.

2. On `DEVICE_OFFLINE`, return fallback when available.
   - Return `READY` with `source = LAST_KNOWN`.
   - Mark entries as `UNVERIFIED_OFFLINE`.
   - Attach cache status if a matching cached file exists.
   - If no fallback exists, return the current offline failure.

3. Join browse metadata with smart-cache metadata.
   - Match by `roomId`, `sourceRelativePath`, and source version where available.
   - Only expose download action when `cacheStatus = AVAILABLE`.

4. Invalidate stale cached files.
   - When desktop reports source changed, mark matching cached files `STALE` or `INVALIDATED`.
   - Worker deletes invalidated object storage keys according to lifecycle rules.

5. Tests.
   - Online browse stores fallback.
   - Offline browse returns `LAST_KNOWN`.
   - Offline browse without prior page returns `DEVICE_OFFLINE`.
   - Cached file shows `AVAILABLE`.
   - Source-changed cached file shows `STALE` and is not presented as fresh.

## Mobile Plan

1. Files page displays fallback source.
   - Show a non-blocking banner for `LAST_KNOWN`.
   - Keep folder navigation usable for directories that have last-known pages.
   - Show empty fallback only when no cached page exists.

2. Entry states.
   - `UNVERIFIED_OFFLINE`: visible but freshness unknown.
   - `STALE`: visible with warning, no fresh guarantee.
   - `AVAILABLE`: enable download from cached file endpoint.
   - `NOT_CACHED`: disable download while desktop is offline.

3. Local display cache.
   - Mobile may also cache received browse pages locally for faster app relaunch.
   - Treat local cache as display-only, not as download authority.
   - Server fallback remains the authoritative cross-device offline view.

4. Tests.
   - Files page keeps last-known entries on `DEVICE_OFFLINE`.
   - `LAST_KNOWN` banner appears.
   - Download button is enabled only for `AVAILABLE`.

## Desktop Plan

1. Keep browse responses stable and safe.
   - Return managed-root-relative entries only.
   - Include `desktopGeneration`.
   - Keep cursor invalidation explicit.

2. Improve source version metadata if needed.
   - Include file identity, size, and modified timestamp in browse and transfer validation.
   - Never expose absolute local paths to server/mobile.

3. Smart cache candidate/upload flow.
   - Use opt-in policy.
   - Validate reservation before upload.
   - Recheck source before completion.
   - Report completion through outbox for retry.

4. Watcher/source-change reporting.
   - On local source changes, let server know cached metadata may be stale.
   - Keep this best-effort and retryable through outbox.

## Rollout Order

1. Shared contract: add fallback/cache/freshness fields.
2. Server: persist last-known browse pages and return fallback on offline.
3. Mobile: render `LAST_KNOWN`, `UNVERIFIED_OFFLINE`, `STALE`, `AVAILABLE`.
4. Desktop: fill any missing source version fields and stale signals.
5. Worker: confirm lifecycle cleanup for invalidated cached objects.
6. E2E: desktop online browse, power off desktop, mobile reopens Files and sees last-known folder.

## Non-Goals

- No full automatic folder sync.
- No server storage of absolute local paths.
- No pretending offline metadata is fresh.
- No offline download unless file content has an `AVAILABLE` cached object.

## Demo Acceptance

1. Desktop online: mobile opens a room Files page and sees folder entries.
2. Desktop powers off.
3. Mobile reopens the same room and sees the last-known entries with offline/freshness warning.
4. Non-cached files are visible but not downloadable.
5. Cached `AVAILABLE` files can be downloaded from server object storage.
6. When desktop returns and source changed, stale entries are marked clearly.
