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

