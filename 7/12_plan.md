# A-side MVP Plan - 2026-07-12

## MVP Goal

A-side MVP is complete when the desktop agent can safely run as a paired background agent:

```text
Desktop app install/start
-> Mobile pairing through QR/code
-> Desktop background heartbeat/poll/replay
-> Desktop or Mobile cleanup request
-> local file engine proposal
-> user approval
-> precheck
-> journal-before-write execution
-> result sync
-> undo visibility
```

The hard minimum is not "many file tools." It is one reliable end-to-end path with local file safety preserved.

## 0. Commit Current Baseline

Current uncommitted work:

- background runtime skeleton
- tray controls
- conflict resolution after pulling latest `origin/main`

Verification:

```powershell
cargo fmt
cargo check --features tauri-commands
```

Suggested commit:

```text
feat(desktop): add background runtime tray controls
```

Done when:

- working tree is clean
- latest `origin/main` is included
- future bug fixes are not mixed with merge/conflict cleanup

## 1. Stabilize Downloads Folder Browsing

B-side testing found that real Downloads folders can break file browsing. This must be fixed before deeper integration.

Likely real-world cases:

- active browser temp files such as `.crdownload` or `.tmp`
- `.lnk` files
- OneDrive/cloud placeholder files
- symlink, junction, or reparse point entries
- permission or metadata read failures
- Korean names, spaces, parentheses, long names
- large mixed folders

Work:

- make `browse_root` tolerate unreadable entries without failing the whole directory
- make `analyze_root` tolerate unreadable entries where safe
- make `file_index` reindex/search tolerate unreadable entries where safe
- return or log skipped entry details
- keep symlink/reparse escape protection strict
- keep write/mutation paths strict

Rule:

```text
Read/list operations may partially skip unsafe or unreadable entries.
Write/mutation operations must fail closed.
```

Verification:

```text
Downloads-like fixture
-> browse
-> analyze
-> reindex
-> search
-> propose
-> precheck
```

Manual verification:

```text
Register Windows Downloads
-> browse list
-> search
-> proposal
-> precheck
```

Done when:

- one unreadable Downloads entry does not break the whole browse/index flow
- root boundary checks still block traversal and reparse escapes

## 2. Complete QR/Code Pairing

The desktop background agent is useful only after it is paired with mobile/server.

Already present:

- pairing session REST request
- 6-digit code response
- status polling
- keychain device token storage
- manual heartbeat request

Missing:

- QR display UX
- final QR payload schema
- real Android claim E2E
- automatic transition after `CLAIMED`
- restart recovery from keychain credential
- expired/failed pairing UX

Work:

1. Desktop pairing UI
   - start pairing
   - show 6-digit code
   - show QR code
   - show expiry
   - show polling/claimed/expired/error states

2. QR payload
   - include only non-secret claim data
   - never include device token

Example payload shape:

```json
{
  "type": "housemouse_pairing",
  "sessionId": "...",
  "code": "123456",
  "serverBaseUrl": "http://127.0.0.1:3000"
}
```

3. Claimed transition

```text
CLAIMED
-> save credential to OS keychain
-> send initial heartbeat
-> show paired/online or paired/offline state
```

4. Restart recovery

```text
app start
-> load keychain credential
-> restore paired state
-> try heartbeat
```

E2E:

```text
Desktop QR displayed
-> Android scans/claims
-> Desktop receives CLAIMED
-> keychain save
-> heartbeat 201
-> Mobile shows Desktop online
```

Done when:

- mobile can pair with desktop through QR/code
- desktop stays paired after restart
- token is not exposed in UI/logs

## 3. Make Background Runtime Real

Current background runtime is a skeleton. It tracks status but does not run the agent loops.

Missing:

- heartbeat loop
- command polling loop
- sync replay loop
- durable outbox flush loop
- real pause/resume task cancellation
- tray state integration
- autostart recovery flow

Work:

1. Extend `BackgroundRuntime`
   - `start`
   - `pause`
   - `resume`
   - `stop`
   - task handle tracking
   - cancellation token
   - last tick/error/status

2. Heartbeat loop

```text
paired credential exists
-> heartbeat every configured interval
-> success: online
-> failure: offline/degraded
```

3. Command polling loop

```text
poll pending commands
-> mark DELIVERED/ANALYZING
-> hand command to local handler
```

4. Replay loop

```text
startup or reconnect
-> /v1/sync/events after local cursor
-> apply events
-> update cursor
```

5. Outbox loop preparation
   - proposal/result sending can be added later
   - structure should support retry after restart

6. Tray connection
   - pause actually stops loops
   - resume restarts loops
   - close window hides to tray while background continues
   - quit shuts down gracefully

7. Autostart recovery

```text
Windows login
-> app start
-> credential load
-> background start
-> heartbeat/replay/poll
```

Done when:

- desktop keeps heartbeat while window is hidden
- paired desktop detects server commands without manual UI refresh
- pause/resume affects real background loops

## 4. Separate Manual Tools From Delegated Agent Work

Desktop has two different operation modes.

Manual file tools:

```text
user clicks file-browser rename/trash/create
-> local confirm
-> local safety checks
-> journal where applicable
-> execute
```

Delegated agent work:

```text
Desktop request / Mobile request / AI request
-> command
-> proposal
-> approval
-> precheck
-> journal
-> execute
```

Current state:

- manual `trash_file`, `rename_file`, `create_file` exist
- delegated `propose/precheck/execute` exists for move/trash cleanup
- rename/create are not yet delegated proposal actions

Work:

- keep direct operations as local manual tools
- forbid remote/AI/agent commands from calling direct operations
- route delegated work through proposal approval
- document this distinction in code/docs/UI

Done when:

- user can tell manual file tools apart from agent proposals
- delegated work always goes through proposal approval

## 5. Complete Desktop Delegated Proposal Flow

Desktop itself must support "ask the agent" flow, not only direct file operations.

Initial desktop delegated flow:

```text
managed root selected
-> build cleanup proposal
-> approve/reject items
-> require rejection reason
-> precheck
-> execute
-> history
-> undo
```

Work:

- make proposal UI the primary delegated cleanup path
- block mutation on journal corruption
- require precheck before execute
- keep proposal snapshot as source of truth
- show executed/skipped/rejected separately

Done when:

- Desktop alone can demonstrate cleanup proposal -> approval -> execute -> undo
- no files change before approval
- stale proposals fail precheck

## 6. Connect Mobile/Server Command To Desktop Proposal

This is the first P0 vertical slice.

Target flow:

```text
Mobile command
-> Server DB
-> Desktop background poll/replay
-> local proposal
-> Server proposal upload
-> Mobile approval
-> Desktop decision receive
-> precheck
-> journal-before-write execute
-> execution result upload
-> Mobile result
-> Desktop undo
```

A-side work:

- map server command payload to local root/rule/proposal
- verify command room/root/device match a registered managed root
- generate local proposal snapshot
- upload proposal items to server
- map local actions:
  - `move` -> `MOVE`
  - `trash` -> `QUARANTINE`
- receive decision
- match decision to local proposal snapshot
- execute approved items only
- upload execution result and skipped/stale reasons

Done when:

- mobile command produces a real desktop proposal
- mobile approval produces real local execution
- result reaches mobile/server
- undo remains visible on desktop

## 7. Clarify Auto Approval / Delegation Permission

Current auto approval is proposal-only:

```text
enabled
allowed_actions: move/trash
max_files_per_run
expires_unix_ms
```

Policy:

- auto approval is not direct file-operation permission
- it only approves ready proposal items
- direct rename/create/trash are not auto-approved
- default remains disabled
- action allowlist, count limit, and expiry are required
- precheck and journal remain mandatory

Work:

- UI wording: "Auto approve proposals"
- make sure remote/mobile/AI commands cannot bypass proposal
- decide whether mobile-originated proposal may use local auto approval
- record auto-approved decisions if practical

Done when:

- "always allow" cannot be confused with "agent may run arbitrary file operations"
- auto approval still goes through precheck/journal

## 8. Normalize Rename / Trash / Create Actions

Priority:

1. Trash
   - already exists as proposal action
   - align direct trash and proposal trash result/journal behavior
   - map to server `QUARANTINE`

2. Rename
   - delegated rename can be represented as a move:

```text
folder/old.txt -> folder/new.txt
```

   - keep direct rename as manual tool
   - remote/delegated rename must become a proposal

3. Create
   - current proposal shape expects a source path
   - create/write needs a separate proposal shape or action
   - defer until README/write design

Done when:

- MVP uses move/quarantine safely
- rename can be delegated as a proposal if needed
- create is not exposed as arbitrary delegated write

## 9. Add Socket.IO Notification Client

Socket.IO should not be the state source.

```text
Socket.IO = "new data exists"
REST replay = recovery/source of truth
```

Work:

- connect desktop Socket.IO client
- listen for command/decision/proposal-related events
- trigger REST replay or command poll on notification
- keep REST background loop as fallback

Done when:

- socket disconnect does not lose state
- socket improves responsiveness when available

## 10. Add Durable Desktop Outbox

Desktop-to-server mutations must survive network failure and process restart.

Outbox items:

- command status update
- proposal created
- execution result
- undo result
- later transfer/cache status updates

Work:

- SQLite outbox table
- idempotency key
- pending/sent/failed state
- retry count and last error
- background flush loop

Done when:

- execution result is not lost if network fails after local file mutation
- retry is idempotent on the server

## 11. Add Desktop Overlay Shell / CharacterEvent Bridge

Overlay and first-run onboarding are A/B boundaries. A owns the desktop native shell, event bridge, and app-level slots; B owns the character design, motion, onboarding copy, and product expression.

A-side work:

- add an app-level empty/onboarding slot for the first run before any managed root exists
- route the zero-root state to onboarding instead of only showing disabled file panels
- let B-owned onboarding/character UI prompt the user to pair and register a managed root
- create/show/hide the Tauri overlay window
- keep tray, close-to-tray, autostart, and overlay lifecycle compatible
- deliver background/proposal/decision/execution state as `CharacterEvent`
- provide an overlay chat/input bridge that can hand off to the AI command draft or proposal flow
- expose a stable bridge for B-owned character UI
- keep overlay code out of direct file-operation permission paths

Done when:

- first app launch with zero managed roots has a clear UI slot for B's onboarding design
- onboarding can hand off to pairing and managed-root registration without bypassing safety checks
- B can plug the character design into a working desktop overlay window
- proposal/execution state can trigger character events
- overlay chat can create only draft/proposal-bound requests, never direct file operations
- overlay cannot bypass file approval, precheck, journal, or execution boundaries

## 12. Add AI Command Draft

AI should be added after deterministic P0 proposal flow works.

Allowed role:

```text
natural language from mobile chat or desktop overlay chat
-> CommandDraft / Rule DSL draft
-> schema validation
-> deterministic file proposal
-> approval
-> execute
```

Desktop overlay chat boundary:

- desktop overlay chat is an input bridge, not a file-operation surface
- overlay chat submits a message to the AI command draft flow
- AI/provider code returns only a validated command/rule draft
- the draft must enter the same command/proposal pipeline as mobile/server commands
- the desktop file engine remains the only layer that computes concrete file targets
- chat output must never call `trash_file`, `rename_file`, `create_file`, or execution APIs directly

A/B split:

- B owns chat UI, AI provider calls, natural-language UX, and server-side command draft creation
- A owns the desktop overlay bridge, local validation, deterministic proposal generation, precheck, journal, execution, undo visibility, and outbox result delivery

Forbidden:

- direct file mutation
- bypassing approval
- root escape
- fake success

MVP minimum:

```text
"Clean up old PDFs in Downloads"
-> validated rule draft
-> proposal
```

Done when:

- invalid AI output is rejected before file logic
- deterministic Rust engine computes final targets
- desktop overlay chat can create only draft/proposal-bound requests
- desktop overlay chat cannot bypass approval, precheck, journal, execution, or outbox boundaries

## 13. README Draft/Diff/Write

README write is a file mutation and must be proposal-gated.

Flow:

```text
README command
-> draft
-> diff
-> README_WRITE proposal
-> approval
-> precheck
-> journal/write
-> history/undo or recovery visibility
```

Rules:

- no overwrite without explicit approved diff
- no write before approval
- no fake success if AI/provider is unconfigured

Done when:

- user sees diff before write
- write is journaled/recoverable or visibly undo-limited

## 14. Add Online File Browse Adapter

Mobile file access starts with online browsing before transfer. The server may relay browse requests and cache display state, but the desktop remains the source of truth for managed-root path validation and live directory listings.

Target flow:

```text
Mobile Files screen
-> Server browse request for room/path/page
-> Desktop background poll/replay or socket wake
-> room maps to one registered managed root
-> relative path is validated inside that root
-> directory page is listed with safe skips
-> result is uploaded to server
-> Mobile displays the current page
-> user selects a file
-> FileTransfer P0 starts
```

A-side work:

- define the desktop browse request/response adapter for server-originated file browse
- map `room_id` to a registered, enabled managed root before reading anything
- validate relative directory paths with the same root boundary rules as local browse
- block traversal, absolute paths, symlink, junction, and reparse point escapes
- skip unreadable or unsafe entries for read/list operations without failing the whole page
- exclude `.housemouse`, `.housemouse_trash`, temp/lock/credential-like files from remote browse results
- support pagination or cursor inputs without treating stale pages as current truth
- return structured errors for offline, timeout, permission denied, invalid path, cursor invalidated, and root disabled
- avoid uploading absolute local paths or file contents in browse responses

Done when:

- mobile can request a live directory page from an online desktop through the server
- invalid paths and root escapes are rejected before filesystem reads outside the root
- one unreadable entry does not break the whole mobile browse page
- browse responses expose only relative paths and safe metadata
- FileTransfer can start from a browsed relative file path without reusing unvalidated mobile input

## 15. FileTransfer P0

This is the second P0 vertical slice after cleanup execution.

Flow:

```text
Mobile file request
-> Desktop validates managed-root source
-> source version check
-> upload target
-> chunk upload
-> SHA-256 complete
-> Mobile checksum
-> ACK/TTL cleanup
```

A-side work:

- source relative path validation
- managed root boundary check
- symlink/reparse block
- source version: file id, size, modified time
- source changed detection
- chunk reading
- SHA-256
- cancellation
- failure code mapping

Done when:

- root escape is blocked
- source changed is reported distinctly
- checksum is produced before completion

## 16. Offline Smart Cache / P1 File Cache

Offline file caching is not part of the first cleanup P0, but it must remain in the roadmap. It should start only after both P0 slices are stable:

1. cleanup command/proposal/execute/undo
2. online file browse/transfer

Purpose:

```text
When the PC is offline, mobile can access only explicitly cached, limited, freshness-labeled files.
```

A-side responsibilities:

- local file access event aggregation
- usage score calculation
- manual pin/exclude metadata
- source version hashing
- stale detection when original file changes
- candidate batch submission to server
- encrypt/upload only after server quota reservation and approved upload target
- never upload before user opt-in
- never present cached file as definitely fresh while PC is offline

Important states:

```text
AVAILABLE
UNVERIFIED_OFFLINE
STALE
EVICTED
UPLOAD_PENDING
UPLOAD_FAILED
```

Safety rules:

- opt-in per room
- quota enforced by server reservation
- file size limit
- exclude `.housemouse`, `.housemouse_trash`, temp/lock/credential-like files
- device revoke/cache disable must delete server objects
- pending queued commands should warn that cached file list may be outdated

Done when:

- cache candidates are local-score based
- uploads happen only after explicit server approval
- stale/offline freshness is visible
- disabling cache or revoking device removes cached objects

## 17. Add Device Revoke / Unpair Safety

Pairing is not complete unless the desktop also stops safely when the server or mobile revokes the device. A local "forget pairing" button is not enough; remote revoke must prevent future background work and file mutations.

Target flow:

```text
Mobile/server revokes desktop device
-> Desktop heartbeat/poll/replay/socket sees 401, 403, or revoke event
-> background runtime enters suspended/revoked state
-> Socket.IO disconnects
-> pending command and decision processing stops
-> device token is deleted or marked invalid
-> UI asks the user to pair again
```

A-side work:

- treat `UNAUTHENTICATED`, `FORBIDDEN`, and explicit revoke events as terminal pairing failures
- stop command polling, decision execution, and realtime reconnect after revoke is detected
- disconnect realtime Socket.IO and prevent reconnect with the revoked token
- delete the device token from keychain or mark the local credential invalid
- preserve managed roots, file index, journal, and operation history; do not delete user file state
- check authenticated device state before claiming executions or starting file transfer work
- record a clear local status such as `revoked` or `pairing_required`
- make the UI expose re-pairing without pretending the agent is online
- handle revoke during an in-progress local execution without corrupting journal recovery

Done when:

- a revoked desktop cannot receive new commands or execute approved decisions
- background loops stop retrying with an invalid token
- local managed roots and recovery history remain available
- the user sees a clear re-pairing path
- revoke after a local file mutation cannot produce fake success or duplicate execution

## 18. Add Tauri Window Capability / Permission Split

The desktop will have at least a main app window and a character overlay window. They must not share the same file-operation permissions. The overlay is a character/chat surface, not a privileged file manager.

Window roles:

```text
main window
-> pairing, managed roots, browse/search
-> proposal/precheck/execute
-> manual trash/rename/create
-> history/recovery/background controls

character-overlay window
-> receive CharacterEvent
-> show character/onboarding/chat UI
-> send chat text or draft requests
-> show/hide/focus overlay
-> never invoke direct file mutations
```

A-side work:

- define separate Tauri capabilities for the main window and `character-overlay`
- allow file-engine mutation commands only from the main window
- keep overlay permissions limited to overlay status, character events, and chat/draft bridge commands
- prevent overlay access to `trash_file`, `rename_file`, `create_file`, `execute_file_changes`, `recover_journal`, and auto-approval mutation commands
- add Rust-side window-label checks for high-risk commands where practical
- document which commands each window may invoke
- add a test or manual verification checklist proving overlay cannot invoke file mutation commands
- keep overlay chat handoff proposal-bound even if B-owned UI changes

Done when:

- the overlay window can render and chat without file-operation privileges
- direct file mutation commands fail from the overlay window
- main-window file workflows still work normally
- capability rules match the manual/delegated file-operation boundary
- a frontend bug in overlay UI cannot bypass approval, precheck, journal, or execution boundaries

## 19. Windows Release Hardening

Work:

- autostart final verification
- tray status final verification
- updater
- release signing
- clean install test
- no secrets in logs
- avoid leaking absolute paths to server logs/crash reports
- demo recovery checklist

Done when:

- clean Windows install can pair, run background, execute demo, undo, and quit safely

## Final Implementation Order

```text
0. Commit current baseline
1. Downloads browsing/indexing stabilization
2. QR/code pairing completion
3. Real background heartbeat/poll/replay loops
4. Manual tools vs delegated agent separation
5. Desktop delegated proposal flow
6. Mobile/server command -> Desktop proposal
7. Auto approval permission clarification
8. Rename/trash/create action normalization
9. Socket.IO notification client
10. Durable outbox
11. Desktop overlay shell / CharacterEvent bridge
12. AI command draft
13. README write proposal
14. Online file browse adapter
15. FileTransfer P0
16. Offline smart cache / P1 file cache
17. Device revoke / unpair safety
18. Tauri window capability / permission split
19. Windows release hardening
```

## Minimum MVP Cut

If time is tight, keep only:

```text
Downloads stable browse/index
QR pairing
background heartbeat/poll
mobile command -> desktop proposal
mobile approval -> journaled execute
result sync
undo
```

AI, README write, FileTransfer, rename/create delegation, and offline smart cache can be layered after that core path is reliable.
