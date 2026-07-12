# B Handoff After A Step 13

This note is for B-side work after the A-side desktop branch reached the step-13 boundary.

## Current A-Side Status

A has the local desktop safety rail in place:

```text
server/mobile/AI-style command
-> desktop command processor
-> optional AI rule draft validation
-> deterministic local proposal
-> server proposal outbox
-> user approval decision
-> desktop precheck
-> journal-before-write execution
-> execution result outbox
-> desktop undo/history visibility
```

Implemented on the desktop side:

- QR/code pairing bridge and persisted device credential flow
- background heartbeat, command polling, sync replay, decision polling, and outbox flush
- Socket.IO wake-up as a latency hint, with REST replay/poll as the source of truth
- local managed-root registration, browse/index/search, proposal, precheck, execute, history, undo, and recovery
- manual `trash_file`, `rename_file`, `create_file` kept separate from delegated work
- delegated work normalized to proposal items only
- `MOVE` and `QUARANTINE` execution through the journaled file engine
- `CREATE_DIR` and `README_WRITE` refused by execution until the README/write design is implemented
- AI/rule-draft validation before filesystem access
- overlay shell and chat input bridge that can only emit a draft request, never mutate files

## What B Can Build Against Now

B can rely on this high-level flow:

```text
Mobile or server creates command
-> Desktop receives command
-> Desktop creates proposal
-> Mobile shows proposal
-> User approves or rejects
-> Desktop executes approved items
-> Mobile shows execution result
```

The currently reliable command path is cleanup/organize-style work that becomes local `MOVE` or `QUARANTINE` proposals.

For AI, B should treat the desktop as a validator and executor, not as the LLM host:

```text
chat message
-> server-side AI/provider call
-> validated command or Rule DSL draft
-> command persisted on server
-> desktop proposal flow
```

The desktop accepts an optional `ruleDraft` in organize command payloads. That draft is parsed as the local Rule DSL and rejected before any file read if malformed or semantically invalid.

## README Draft/Diff/Write Boundary

Step 13 is not a general write permission. README changes must also stay on the proposal rail.

Target flow:

```text
README command
-> AI/server draft text
-> server or desktop computes preview/diff
-> README_WRITE proposal item
-> user approval
-> desktop precheck
-> journal/write or explicit undo-limited state
-> execution result
```

Rules B should preserve:

- no README file write before approval
- no fake success when AI/provider is unconfigured
- show the user the proposed README text or diff before approval
- keep README output bounded by schema and size limits
- do not call desktop direct file APIs for README writing
- do not expose README writing as `create_file`

Current A behavior:

- `README_WRITE` is recognized as a server proposal action shape but is explicitly refused by the desktop executor.
- This is intentional until A adds the journaled README write path.
- B can design the UX and API around README draft/diff approval now, but should not expect desktop execution of `README_WRITE` yet.

## B-Side TODO

AI/chat:

- implement or finish `POST /v1/rooms/{roomId}/chat`
- call the configured LLM provider on the server, not from the desktop app
- return explicit `UNCONFIGURED` when no provider is configured
- convert natural language to a bounded command or Rule DSL draft
- validate AI output with contracts before persisting commands
- persist only commands/drafts, never direct file operations

Proposal UX:

- show open proposals from `GET /v1/rooms/{roomId}/proposals/open`
- show item reasons, conflicts, and pending approval state
- submit approve/reject through `POST /v1/proposals/{proposalId}/decisions`
- show execution history/result from room execution APIs

README UX:

- design README draft/diff review
- keep README write visibly approval-gated
- display unconfigured AI/provider state instead of pretending a draft exists

Overlay/character:

- B can replace the placeholder character stage in the desktop overlay.
- The overlay chat input is draft-only. It cannot invoke file mutation commands.
- Character events are state hints from the desktop background loop.

## A-Side TODO After This Handoff

Recommended next A-side order:

```text
13a. implement journaled README_WRITE execution, or keep it explicitly refused
14. online file browse adapter
15. FileTransfer P0
17. device revoke/unpair safety
18. Tauri window capability split
```

The next truly cross-team slice is step 14/15: mobile online file browse and file transfer.
