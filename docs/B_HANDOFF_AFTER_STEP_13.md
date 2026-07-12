# B Handoff: A-Side Steps 1-13

Date: 2026-07-13

This is the handoff from A after completing the desktop-side work through plan step 13. It is written for B so mobile/server/character/AI work can attach to the desktop agent without guessing which file-operation paths are safe.

## One-Line Status

A has a working desktop safety rail for cleanup, AI rule drafts, and proposal-gated README writes:

```text
server/mobile/AI command
-> desktop command processor
-> optional Rule DSL draft validation
-> deterministic local proposal
-> proposal upload through desktop outbox
-> user approval decision
-> desktop precheck
-> journal-before-write execution
-> execution result upload through desktop outbox
-> desktop history/undo visibility
```

The next cross-team area is step 14/15: mobile online file browse and file transfer.

## Implemented By A

### 1. Downloads Browse / Index Stabilization

Implemented:

- read/list paths tolerate unsafe or unreadable entries by skipping them
- browse/analyze/index/search expose skipped-entry information
- symlink, junction, and reparse escapes remain blocked
- mutation paths still fail closed

B impact:

- real Downloads-like folders should no longer break the whole desktop browse/index flow because of one bad entry
- mobile file browsing still needs step 14; this is only local desktop browse/index stability

### 2. QR / Code Pairing

Implemented:

- desktop can create pairing sessions
- desktop shows QR/code flow in AgentPanel
- QR payload contains only non-secret claim data
- desktop polls pairing status
- claimed device token is stored locally and not shown in UI
- paired state can start background runtime

B impact:

- Android/mobile should claim the pairing code/QR through server APIs
- B should verify the Android scan/claim E2E with the current QR payload

Desktop QR payload shape:

```json
{
  "type": "housemouse_pairing",
  "version": 1,
  "code": "123456",
  "sessionId": "...",
  "serverBaseUrl": "http://127.0.0.1:3000",
  "expiresAt": "..."
}
```

### 3. Background Runtime

Implemented:

- background runtime start/pause/status commands
- heartbeat loop
- sync replay cursor
- command polling
- decision polling
- outbox flush
- Socket.IO wake hints integrated with REST fallback
- close-to-tray/background behavior scaffold

Key rule:

```text
Socket.IO = latency hint
REST replay/poll = source of truth
```

### 4. Manual vs Delegated Work

Implemented:

- manual desktop tools remain direct local tools:
  - `trash_file`
  - `rename_file`
  - `create_file`
- delegated work must use:

```text
command -> proposal -> approval -> precheck -> journal -> execute
```

Important B rule:

- mobile/server/AI must not ask desktop to call direct file APIs
- delegated file changes must become proposal items

### 5. Desktop Delegated Proposal Flow

Implemented:

- desktop can build cleanup proposals
- proposal approval/rejection path exists
- precheck is required before execution
- execution writes journal before mutation
- stale or changed files are skipped instead of overwritten
- history and undo are visible on desktop

### 6. Mobile / Server Command To Desktop Proposal

Implemented:

- desktop polls server commands
- desktop maps room to managed root
- desktop verifies root is registered/enabled
- desktop generates local proposal
- desktop submits proposal through durable outbox
- desktop polls approved decisions
- desktop reconstructs local proposal snapshot from server proposal items
- desktop executes approved items only
- desktop uploads execution result through durable outbox

Currently reliable delegated actions:

```text
MOVE
QUARANTINE
README_WRITE
```

Still refused:

```text
CREATE_DIR
arbitrary create/write outside README_WRITE
```

### 7. Auto Approval Permission Boundary

Implemented:

- auto approval is local proposal-only convenience
- default is disabled
- it has action allowlist, max count, and expiry
- it does not grant mobile/server/AI permission to execute arbitrary file operations
- remote delegated flow does not read `AutoApprovalStore`

B impact:

- do not label this as "agent can always edit files"
- correct UX wording is closer to "auto approve eligible proposals"

### 8. Rename / Trash / Create Normalization

Implemented:

- `QUARANTINE` uses the same recoverable trash behavior as manual trash
- delegated rename is represented as `MOVE`
- direct `create_file` remains manual-only
- `CREATE_DIR` remains refused in delegated execution

### 9. Socket.IO Notification Client

Implemented:

- desktop Socket.IO client connects when paired
- command/proposal/decision events wake the background REST pass
- disconnect does not lose state because REST replay/poll still reconciles

### 10. Durable Desktop Outbox

Implemented:

- local SQLite outbox
- idempotency key per mutation
- pending/sent/failed states
- retry count and last error code
- background flush loop

Currently queued through outbox:

```text
proposal created
execution result
command FAILED status
```

Notes:

- `ANALYZING` command claim is intentionally sent synchronously to avoid duplicate command processing
- undo result is not yet a separate server/outbox mutation

### 11. Desktop Overlay Shell / CharacterEvent Bridge

Implemented:

- Tauri overlay window show/hide/status commands
- `CharacterEvent` bridge from background activity
- overlay chat input surface
- overlay chat can only emit a draft request event
- overlay does not import or call file-engine APIs

Overlay event:

```text
overlay:draft-request
```

Payload:

```ts
type OverlayDraftRequest = {
  text: string;
};
```

B impact:

- B can replace the placeholder character stage
- B can consume character state hints
- overlay chat still needs server/AI command-draft integration

### 12. AI Command Draft Boundary

Implemented on A:

- desktop accepts optional AI/server `ruleDraft` in organize command payloads
- `ruleDraft` is strictly parsed as local Rule DSL
- invalid AI output is rejected before filesystem reads
- deterministic Rust engine computes concrete file targets
- Tauri command `validate_rule_draft` exists for local validation

Organize command payload shape expected by desktop:

```json
{
  "relativePath": "",
  "maxProposals": 50,
  "ruleMode": "managed_root_rules",
  "userIntent": "Clean up old PDFs in Downloads",
  "ruleDraft": {
    "version": 1,
    "rules": [
      {
        "id": "cleanup-pdfs",
        "when": { "extension_in": ["pdf"] },
        "then": { "trash": true }
      }
    ]
  }
}
```

Rules:

- AI creates drafts, not file mutations
- server should validate AI output before persisting commands
- desktop validates again before local file logic
- no direct AI call from desktop is required for MVP

B owns:

- chat UX
- server-side LLM/provider calls
- natural language to command/rule draft
- `UNCONFIGURED` state when provider is missing

### 13. README Draft / Diff / Write

Implemented on A:

- `README_WRITE` can now execute as a delegated proposal item
- execution is limited to root-level `README.md`
- desktop requires approved full content in proposal item precondition
- desktop prechecks the live README before write
- desktop rejects symlink/junction/reparse README targets
- desktop writes journal before mutation
- desktop stores backup under `.housemouse/readme_backups/`
- undo restores the old README, or removes README if it did not exist before
- `CREATE_DIR` remains refused

README_WRITE proposal item shape:

```json
{
  "itemOrder": 0,
  "actionType": "README_WRITE",
  "sourceRelativePath": null,
  "destinationRelativePath": "README.md",
  "reasonCode": "README_WRITE",
  "precondition": {
    "sourceSizeBytes": 1234,
    "sourceModifiedUnixMs": 1780000000000,
    "content": "# Project Title\n\nGenerated README..."
  },
  "conflictState": "NONE"
}
```

If README does not exist yet:

```json
{
  "sourceSizeBytes": 0,
  "sourceModifiedUnixMs": null,
  "content": "# New README\n\n..."
}
```

Rules:

- B/server/AI generates README content
- mobile or overlay must show preview/diff before approval
- desktop writes only the approved content snapshot
- no fake README draft if AI/provider is unconfigured
- do not represent README write as `create_file`

## B-Facing Server API Flow

B does not call Tauri APIs directly. B calls server APIs; desktop also talks to the server.

Primary cleanup/readme flow:

```text
B/mobile: POST /v1/rooms/{roomId}/commands
Desktop: GET /v1/devices/{deviceId}/commands/pending
Desktop: PATCH /v1/devices/{deviceId}/commands/{commandId}/status
Desktop: POST /v1/agent/proposals
B/mobile: GET /v1/rooms/{roomId}/proposals/open
B/mobile: POST /v1/proposals/{proposalId}/decisions
Desktop: GET /v1/devices/{deviceId}/decisions/pending
Desktop: POST /v1/agent/executions
Desktop: PATCH /v1/agent/executions/{executionId}
B/mobile: GET /v1/rooms/{roomId}/executions
```

Pairing and presence:

```text
Desktop: POST /v1/pairing-sessions
B/mobile: POST /v1/pairing-sessions/claim
Desktop: GET /v1/pairing-sessions/{pairingSessionId}/status
Desktop: POST /v1/devices/{deviceId}/heartbeat
B/mobile: GET /v1/devices/{deviceId}/presence
```

Chat/AI:

```text
B/mobile: POST /v1/rooms/{roomId}/chat
B/server: LLM/provider call
B/server: command or rule draft creation
B/server: POST /v1/rooms/{roomId}/commands
```

## Desktop Tauri APIs Added / Relevant

These are desktop-internal APIs, mostly for A UI and debugging:

```text
get_agent_connection_status
start_agent_pairing
poll_agent_pairing
send_agent_heartbeat
poll_agent_commands
process_agent_commands
process_agent_decisions
flush_agent_outbox
ensure_agent_room
replay_agent_events
update_agent_command_status
forget_agent_device
```

File engine:

```text
register_managed_root
list_managed_roots
update_managed_root_state
analyze_root
browse_root_tree
reindex_managed_root
search_managed_root
propose_file_changes
validate_rule_draft
precheck_file_changes
execute_file_changes
trash_file
create_file
rename_file
undo_operation
list_operation_history
recover_journal
```

Overlay:

```text
get_overlay_status
show_overlay
hide_overlay
emit_character_event
overlay:draft-request
```

## Not Done Yet

Step 14 and later are not implemented on A desktop:

- online file browse adapter for mobile Files screen
- pending file-browse request polling/result upload
- FileTransfer P0 source validation, upload target, chunk/checksum, cancel/failure handling
- offline smart cache candidate/upload flow
- remote device revoke cleanup/suspension
- Tauri capability split that blocks overlay window from invoking privileged commands at the capability layer
- Windows release hardening

Known partial items:

- Android pairing E2E should be verified by B
- overlay chat only emits local draft event; it does not yet submit to server chat/AI
- undo result is not uploaded as a separate server mutation
- `ANALYZING` command claim is synchronous by design, not outboxed

## B TODO

Immediate B-side work:

- verify Android QR/code claim E2E
- ensure mobile can show desktop online/offline presence
- create cleanup commands from mobile/server
- show open proposals with action, reason, conflict state, and README preview/diff
- approve/reject proposals
- show execution results and stale/skipped reasons
- implement or finish chat endpoint and AI provider flow
- convert natural language to validated command/rule draft
- show explicit `UNCONFIGURED` when AI provider is missing
- design README preview/diff UX before approval
- plug character UI into the overlay stage

For README_WRITE specifically:

- generate final README content on server/AI side
- include approved content in `precondition.content` or `precondition.readmeContent`
- include live README precondition metadata
- keep content under 200KB
- never send README_WRITE for paths other than `README.md`

## Recommended Next Order

A:

```text
14. online file browse adapter
15. FileTransfer P0
17. device revoke / unpair safety
18. Tauri capability split
19. Windows release hardening
```

B:

```text
proposal/decision/result UX
chat/AI command draft
README preview/diff UX
mobile Files browse UX for step 14
file transfer UX for step 15
character overlay UI
```

## Verification Run Before Handoff

Latest local verification:

```text
cargo test -q
cargo check -q --features tauri-commands
pnpm --filter @housemouse/desktop typecheck
```

All passed after step 13.
