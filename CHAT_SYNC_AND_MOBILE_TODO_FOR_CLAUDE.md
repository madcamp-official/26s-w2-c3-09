# MouseKeeper Chat Sync / Mobile Follow-up Todo

This document is a handoff note for continuing the chat/proposal/rule work.

## Current State

Desktop chat overlay has been heavily improved:

- `character-overlay` and `chat-overlay` are separated.
- Desktop chat can show command proposals and auto-cleanup proposals.
- Approving a desktop command draft now attempts: confirm draft -> precheck/proposal -> execute.
- Desktop chat now renders `RULE_DRAFT` messages with approve/reject buttons.
- Approving a rule draft on desktop calls the server and writes the confirmed rule into the bound managed root's local `.mousekeeper/rules.json`.
- Desktop background replay now applies server `rule.created` events to the local desktop rule file.
- `TRASH` is connected through the contract/server/mobile/desktop command path:
  - mobile sends `TRASH` command payloads,
  - server creates command drafts/commands,
  - desktop turns them into recoverable `.mousekeeper_trash` proposals,
  - execution uses the same safe local trash engine.

Important caveat:

- A lot of desktop chat UI state is still locally patched/persisted, for example submitted/approved/skipped button labels and local auto proposal cards.
- For mobile parity, the server should become the source of truth for chat messages, proposal states, execution results, and draft states.

## Goal

Make desktop chat and mobile chat behave as the same product surface:

- Same chat history.
- Same pending suggestions.
- Same approval buttons.
- Same execution/skipped/failed result messages.
- Same rule draft approval flow.
- Same automatic proposal notifications.

## Priority 1: Server as Source of Truth

Right now desktop carries too much local-only UI state. Move the durable state to the server.

Required behavior:

- Every proposal-like thing that appears in chat must be represented by a server chat message or server-backed pending action.
- Approval/submission/skipped/executed/failed state must be recoverable from server data after app restart.
- Desktop and mobile should render button labels from server state, not from local-only memory.

Likely files:

- `apps/server/src/chat/chat.service.ts`
- server command/proposal/execution services
- `packages/contracts/src/control-plane.ts`
- `packages/contracts/openapi.yaml`
- `apps/desktop/src/features/overlay/ChatOverlay.tsx`
- `apps/mobile/lib/features/chat/chat_page.dart`

Suggested contract shape:

- `COMMAND_DRAFT`
  - draft id
  - status: `DRAFT | CONFIRMED | REJECTED | EXPIRED`
  - command id when materialized
- `RULE_DRAFT`
  - draft id
  - status: `DRAFT | MATERIALIZED | REJECTED | EXPIRED`
  - rule id when materialized
- `PROPOSAL`
  - proposal id
  - status: `OPEN | DECIDED | EXECUTED | SKIPPED | FAILED`
  - item status summary
- `EXECUTION_RESULT`
  - executed count
  - skipped count
  - failed count
  - per-item summaries

## Priority 2: Auto Suggestions Must Become Server Chat Messages

Desktop currently detects auto-cleanup suggestions locally. Mobile cannot see those unless the desktop/server publishes them.

Required behavior:

1. Desktop auto-cleanup finds real proposals.
2. Desktop submits those proposals to the server.
3. Server creates or exposes a pending chat suggestion.
4. Desktop and mobile both receive the same suggestion in chat.
5. User presses approve once.
6. Server records the decision.
7. Desktop executes.
8. Server stores execution result.
9. Desktop and mobile both show the result in chat.

Important UX rule:

- Auto suggestions should behave like normal chat entries.
- They should not float to the bottom every time the user sends a new message.
- They should stay in chronological order as part of the conversation log.

Likely files:

- `apps/desktop/src-tauri/src/auto_cleanup_processor.rs`
- `apps/desktop/src-tauri/src/command_processor.rs`
- `apps/desktop/src-tauri/src/execution_processor.rs`
- `apps/server/src/chat/chat.service.ts`
- server proposal/decision/execution modules
- `apps/mobile/lib/core/sync/realtime_controller.dart`
- `apps/mobile/lib/features/chat/chat_page.dart`

## Priority 3: Mobile Chat Parity

Mobile already has partial support for chat, rule drafts, and file commands. It needs parity with desktop behavior.

Check/fix:

- Mobile renders `COMMAND_DRAFT` cards with approve button.
- Mobile renders `RULE_DRAFT` cards with approve/reject buttons.
- Mobile approval updates button state immediately.
- Mobile approval survives leaving/reopening the chat.
- Mobile receives execution result messages.
- Mobile shows skipped state when proposals are already executed or no longer applicable.
- Mobile handles `RULE_DRAFT_PREVIEW_UNCONFIGURED` with a friendly message.
- Mobile does not mix pending state between rooms after room switching.
- Mobile realtime updates chat sessions, unread counts, and pending suggestion counts.

Likely files:

- `apps/mobile/lib/features/chat/chat_page.dart`
- `apps/mobile/lib/features/rooms/file_command_page.dart`
- `apps/mobile/lib/features/rules/rules_page.dart`
- `apps/mobile/lib/core/sync/realtime_controller.dart`
- `apps/mobile/test/chat_page_test.dart`
- `apps/mobile/test/file_command_page_test.dart`
- `apps/mobile/test/rules_page_test.dart`

## Priority 4: Trash / File Command End-to-End Verification

`TRASH` appears connected, but verify end-to-end from both clients.

Desktop path:

- Chat command says to trash a file.
- Server creates command draft.
- User approves once.
- Desktop processes command into proposal.
- Desktop executes proposal.
- File moves into `.mousekeeper_trash`.
- Chat receives execution result.

Mobile path:

- Mobile file command page submits `TRASH`.
- Mobile chat or command flow shows approval/pending state.
- Desktop receives command.
- Desktop creates proposal and executes after approval.
- Mobile sees execution result.

Existing confirmed local safety:

- `tools/file-engine-cli/src/trash.rs` uses recoverable root-local trash.
- `apps/desktop/src-tauri/src/command_processor.rs` builds trash proposal reports.
- `apps/desktop/src-tauri/src/execution_processor.rs` executes proposal `Trash`.

### Resolution (this pass)

Traced both paths end-to-end at the code level (no live server/DB available in this environment, so this is static verification + new regression tests, not a manual click-through). No bugs found in the happy path; found and closed real coverage gaps, and one product-behavior clarification.

- **Desktop path confirmed working end-to-end**, including this session's PROPOSAL/EXECUTION_RESULT chat work: `confirmCommandDraft` has no special-case for `TRASH` (unlike FIND/DOWNLOAD/UPLOAD), so it falls through to the generic command-creation path and does carry `metadata.sessionId` — meaning a chat-initiated TRASH gets its PROPOSAL and EXECUTION_RESULT cards posted into the same session as the original request, same as any other intent. Added a test proving this specifically for TRASH (`chat.service.integration.spec.ts`, `'materializes a TRASH command draft with its chat session on the command metadata'`), since previously only `RENAME` was covered.
- **Wire-protocol naming note**: `proposalItemSchema.actionType` (`packages/contracts/src/control-plane.ts`) has no `TRASH` value — a chat/rule TRASH action is submitted to the server as `actionType: "QUARANTINE"` (`command_processor.rs`'s `proposal_item()`), distinguishable from other quarantine sources only by `reasonCode` (`USER_REQUESTED_TRASH_REASON` vs `RULE_QUARANTINE`). This is intentional (the local engine only has Move/Trash actions; nothing behaves differently for a "true quarantine"), not a bug — flagging so a future client doesn't assume `actionType` alone tells TRASH apart from other quarantine-labeled proposals.
- **Closed a real test gap**: `execution_processor.rs` had zero coverage of the delegated Trash execution path (a server-approved QUARANTINE decision actually calling `trash_file` and landing the file in `.mousekeeper_trash`) — exactly what this checklist asks to verify. Added `approved_decision_executes_trash_and_moves_file_into_mousekeeper_trash`. Writing it surfaced that trashed files land under a per-operation subfolder of `.mousekeeper_trash` (`trash.rs`'s `trash_file`), not flatly at `.mousekeeper_trash/<name>` — worth knowing for any future UI that tries to link directly to a trashed file's path.
- **Mobile path: `FileCommandPage` (the manual TRASH/MOVE/RENAME/CREATE form) is unreachable from the running app** — no navigation to it exists anywhere in `apps/mobile/lib`; `RoomPage` explicitly hides manual command entry and routes users to chat instead ("수동 규칙/커맨드 선택은 숨겼습니다. AI 대화에서 실제 가능한 작업만 초안으로 제안합니다"). **Confirmed with the user this is intentional** — mobile TRASH is chat-only by design, not a missing wiring bug. The doc's "Mobile file command page submits TRASH" step describes a surface that exists in code (and is still directly test-covered) but is deliberately not user-reachable; mobile TRASH goes through the same chat command-draft path as desktop and is covered by the point above.
- **Closed a related test gap in the fallback-session logic**: the only existing test for a chat-less (non-drafted) proposal's session routing (`vertical-slice.integration.spec.ts`) only covered the "room has zero active sessions → create a fresh one" branch of `getOrCreateSystemSessionIn`. Added `'routes a chat-less proposal into the room's existing active session instead of a fresh one'` to cover the more common real-world case (a session already exists and gets reused) — this is the actual behavior a mobile-submitted, chat-less TRASH command would hit in practice, per this session's earlier finding that a room almost always has ≥1 active session by the time a proposal is created.
- **Known, not fixed this pass** (flagged, not addressed — would need a product decision, not a bug fix): mobile shows the same open proposal in two separate session-bar tabs (the always-present "승인 대기방" pending-approval tab, and whichever real chat session the PROPOSAL card landed in) — no literal duplicate stacking since they're different tabs, but a user could see the same card twice. Also, there is no real push notification for `PROPOSAL`/`EXECUTION_RESULT` arrival — only an in-app SnackBar that only fires if the app is foregrounded with a live socket connection at that moment; if missed, the only breadcrumb is a non-tappable command-status line in `RoomPage`'s history list.

## Priority 5: Rule Draft Flow Completion

Desktop now confirms rule drafts and applies them locally, but preview is still incomplete.

Current server behavior:

- `POST /v1/rule-drafts/:draftId/preview` can return `RULE_DRAFT_PREVIEW_UNCONFIGURED`.

Required work:

- Either implement desktop dry-run transport for rule preview, or
- Make preview optional and show a friendly "preview unavailable, approve to add rule" UX.

Also verify:

- Rule approved on desktop appears on mobile.
- Rule approved on mobile emits `rule.created`.
- Desktop event replay applies the mobile-created rule into `.mousekeeper/rules.json`.
- Invalid local `rules.json` is not silently overwritten.

Likely files:

- `apps/server/src/rules/rules.service.ts`
- `apps/server/src/rules/rules.controller.ts`
- `apps/desktop/src-tauri/src/commands/agent.rs`
- `tools/file-engine-cli/src/rules/definition.rs`
- `apps/mobile/lib/features/rules/rules_page.dart`

### Resolution (this pass)

Decision: deferred full dry-run preview, closed the rest of the checklist.

- **Preview**: intentionally left as the existing fail-closed `RULE_DRAFT_PREVIEW_UNCONFIGURED` stub. Confirmed a real dry-run needs a synchronous server↔desktop request/response bridge that doesn't exist anywhere in the codebase today (everything else is fire-and-forget/poll-based) — building that is out of scope for this pass. The file-engine evaluation logic itself (`propose_for_root_with_rule_set`, `rule_set_from_draft_value`) already exists and is reused elsewhere, so a future implementation is mostly transport plumbing, not new engine work. Mobile's `rules_page.dart` already shows the friendly Korean fallback message for this error; `chat_page.dart` never calls preview at all, so it's unaffected either way.
- **Cross-device sync speed**: `rule.created`/`rule.draft.updated` were missing from `apps/desktop/src-tauri/src/background.rs`'s `REALTIME_WAKE_EVENTS`, so a rule confirmed on mobile only reached desktop on the next scheduled background tick (~15-20s), not on the realtime push. Added both event names so it now wakes the reconcile loop immediately, same as command/proposal/decision events.
- **Invalid local `rules.json` protection**: already safe as implemented — `apply_server_rule_to_local_root` (`agent.rs`) fails closed and never writes if the existing local file is corrupt or fails validation. Added a regression test (`corrupt_local_rules_json_blocks_server_rule_merge_without_overwriting` in `agent.rs`) proving this at the desktop-integration level; previously only covered at the file-engine-cli unit level.
- **Mobile/desktop confirm parity**: verified `rules.controller.ts` has no device-type branching — both clients hit the identical `RulesController`/`RulesService` path, so "rule approved on desktop appears on mobile" and vice versa were already structurally guaranteed, not something to build.

### Rule MOVE Destination Folder UX

Example user request:

> 앞으로 pdf 확장자의 파일을 pdf 폴더로 옮겨줘

Expected rule definition:

```json
{
  "match": "ALL",
  "conditions": [
    { "field": "extension", "operator": "IN", "value": [".pdf"] }
  ],
  "action": {
    "type": "MOVE",
    "destinationTemplate": "pdf"
  }
}
```

Important distinction:

- Rule draft creation should not fail just because the destination folder does not exist yet.
- Rule draft validation only needs to verify that `destinationTemplate` is a safe relative path.
- Actual execution of a future move can fail or skip if the destination parent folder is missing.

Current UX problem:

- The desktop chat can show `AI_OUTPUT_INVALID` for a simple rule request like "move PDF files to pdf folder".
- That error is probably not caused by the missing `pdf` folder. It means the AI-produced Rule DSL failed schema validation.
- Even after a valid rule is created, future execution may fail if the destination folder does not exist.

Desired behavior:

1. The AI should reliably produce valid Rule DSL for simple extension-to-folder rules.
2. If the destination folder does not exist, the product should handle it explicitly instead of failing mysteriously.
3. Good options:
   - create a companion `CREATE_DIR` proposal for the missing destination folder,
   - or ask "pdf 폴더가 없어요. 먼저 만들까요?",
   - or safely create the destination directory during approved execution if that is accepted by the safety model.
4. Whatever option is chosen, the behavior must be the same on desktop and mobile.

Implementation notes:

- Do not make rule draft validation depend on live desktop filesystem state unless a dry-run/preview transport is explicitly implemented.
- Improve AI validation logging so failed Rule DSL includes the exact Zod error in server logs.
- Consider adding a deterministic fallback for common rule requests:
  - extension rule: `pdf`, `.pdf`, `PDF` -> condition `{ field: "extension", operator: "IN", value: [".pdf"] }`
  - destination folder phrase: "pdf 폴더" -> `destinationTemplate: "pdf"`
- If using companion folder creation, represent it as a proposal/chat pending action so the user still approves writes.

## Priority 6: Remove Desktop-Only Chat State Where Possible

Desktop currently persists local chat action state for button labels.

Examples:

- approved draft ids
- submitted draft ids
- skipped proposal ids
- local auto proposal keys
- dismissed local proposal keys

This was useful for fast desktop fixes, but it should not be the long-term source of truth.

Target:

- Keep only transient UI state locally, such as "currently clicking/approving".
- Durable state should come from server messages, draft status, proposal status, and execution result status.

Likely file:

- `apps/desktop/src/features/overlay/ChatOverlay.tsx`

## Recommended Implementation Order

1. Define/confirm server contract for chat pending actions and execution result payloads.
2. Make server chat history include durable proposal/draft/result state.
3. Update desktop chat to render from server state instead of local-only persisted state.
4. Update mobile chat to render the same server-backed cards.
5. Publish desktop auto-cleanup suggestions to the server as chat-visible pending actions.
6. Add mobile realtime refresh for new suggestions, unread count, pending count, and execution results.
7. Verify `TRASH`, `MOVE`, `RENAME`, `CREATE`, `ORGANIZE`, and `RULE_DRAFT` on both desktop and mobile.
8. Decide whether rule preview is implemented or intentionally treated as unavailable.

## Validation Checklist

Desktop:

- `corepack pnpm --filter @mousekeeper/desktop typecheck`
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --features tauri-commands -j 1`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml command_processor::tests::trash_command -- -q`
- `cargo test --manifest-path tools/file-engine-cli/Cargo.toml trash -- -q`

Server:

- `corepack pnpm --filter @mousekeeper/server test`
- Add/extend tests for:
  - command draft chat approval
  - rule draft chat approval
  - execution result message creation
  - auto suggestion chat message creation
  - trash command draft payload

Mobile:

- Run Flutter tests for:
  - chat page command draft rendering
  - chat page rule draft rendering
  - file command page trash payload
  - realtime pending suggestion update

## Known Risk

The biggest risk is keeping duplicated state in desktop local storage and server chat state at the same time. That will cause mobile and desktop to disagree. Prefer server truth, then make desktop/mobile render that truth.
