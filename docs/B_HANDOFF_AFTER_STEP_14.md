# B Handoff After Step 14: Online File Browse

## What A Implemented

- Desktop now polls server-originated file browse requests:
  - `GET /v1/devices/{deviceId}/file-browse-requests/pending`
- Desktop maps each request `roomId` back to a registered managed root before touching the filesystem.
- Disabled or missing roots fail the browse request instead of reading local files.
- Relative directory paths are passed through the same local `browse_root` guard used by the desktop file browser.
- Traversal, absolute paths, symlink/junction/reparse escapes, and non-directory targets are rejected before exposing results.
- Unreadable entries are skipped by the local browse engine; one bad Downloads entry does not fail the whole page.
- Remote browse results exclude:
  - `.housemouse`
  - `.housemouse_trash`
  - temp/partial/lock-like files
  - credential-like filenames such as token/password/secret/key/pem/p12/pfx
- Desktop uploads only managed-root-relative metadata:
  - `name`
  - `relativePath`
  - `type`
  - `sizeBytes`
  - `modifiedAt`
  - `fileId`
- Desktop never uploads absolute local paths or file contents for browse.
- Pagination uses offset cursors:
  - first page: `cursor: null`
  - next page example: `cursor: "offset:200"`
- Background runtime now processes file browse requests during its normal tick and wakes faster on the `file.browse.requested` realtime hint.
- The desktop Agent panel has a manual debug action: `Process file browse`.

## B-Side Expected Flow

```text
Mobile Files screen
-> POST /v1/rooms/{roomId}/file-browse-requests
-> poll or subscribe for file.browse.ready / file.browse.failed
-> GET /v1/file-browse-requests/{requestId}
-> render resultPage.entries
-> when user selects a FILE entry, start FileTransfer P0 with that relativePath
```

## Request Body From Mobile/Server

```json
{
  "relativeDirectory": "",
  "cursor": null
}
```

For a subdirectory:

```json
{
  "relativeDirectory": "Documents/Reports",
  "cursor": null
}
```

For the next page:

```json
{
  "relativeDirectory": "Documents/Reports",
  "cursor": "offset:200"
}
```

## Desktop Result Shape

The desktop sends this to the server result endpoint:

```json
{
  "entries": [
    {
      "name": "report.pdf",
      "relativePath": "Documents/report.pdf",
      "type": "FILE",
      "sizeBytes": 12345,
      "modifiedAt": "2026-07-13T12:00:00.000Z",
      "fileId": "hm:..."
    }
  ],
  "nextCursor": null,
  "desktopGeneration": "..."
}
```

Server stores it as `resultPage.entries` and `resultPage.nextCursor`.

## Failure Mapping

The current server contract only accepts:

- `DEVICE_OFFLINE`
- `TIMED_OUT`
- `CURSOR_INVALIDATED`
- `OUTSIDE_MANAGED_ROOT`

A maps local root/path/browse safety failures to `OUTSIDE_MANAGED_ROOT` for now because the contract does not yet expose separate `ROOT_DISABLED`, `PERMISSION_DENIED`, or `NOT_A_DIRECTORY` browse failure codes.

## Not Implemented Yet

- File contents transfer. That starts in step 15.
- Offline cache browse. That remains step 16.
- Richer browse failure codes. This needs a contract/server change with B.
- Mobile UI rendering and polling/subscription UX.
