# Manual vs Delegated File Work

HOUSEMOUSE desktop has two file-operation modes. They must stay separate.

## Manual Tools

Manual tools are started by the desktop user in the local file browser.

Current manual tools:

- `trash_file`
- `rename_file`
- `create_file`

Manual tools may execute immediately after local confirmation and local safety checks. They are not permissions for mobile, server, AI, or background agent work.

## Delegated Work

Delegated work comes from desktop requests, mobile commands, or AI command drafts.

Delegated work must use this path:

```text
command
-> proposal
-> approval
-> precheck
-> journal
-> execute
```

Delegated code must not call `trash_file`, `rename_file`, or `create_file` directly. Remote rename should be represented as a move proposal when supported. Remote create/write needs a separate proposal action and is deferred until the README/write design.

## Current Boundary

`apps/desktop/src-tauri/src/command_processor.rs` and `apps/desktop/src-tauri/src/execution_processor.rs` are the delegated boundary. `command_processor.rs` accepts cleanup commands and turns them into proposal submissions. `execution_processor.rs` accepts server-approved decisions, reconstructs the local proposal snapshot from the server's recorded items, and journals the execution through the same `file_engine_cli::execute` path the manual proposal UI uses — it never calls `trash_file`, `rename_file`, or `create_file` directly. Unsupported command or item types that look like direct file tools are skipped or failed explicitly instead of being executed.

## Auto Approval Is Not Delegation Permission

The desktop-local "Auto approve proposals" policy (`apps/desktop/src-tauri/src/storage/auto_approval.rs`) is a manual-tool convenience, not a delegation grant:

- It only pre-checks `ready` items in a proposal the local user already built through the manual UI (`propose_file_changes` → `auto_approve_file_changes`). It never calls `precheck_file_changes` or `execute_file_changes` itself — those still require an explicit user click, so a stale or newly-conflicting item is still caught before any file changes.
- Default is disabled. Enabling it requires an explicit action allowlist (`move`/`trash`) and a positive `max_files_per_run`; an empty allowlist or zero limit is rejected by both the store (`AutoApprovalStore::patch`) and the engine (`file_engine_cli::auto_approval::AutoApprovalPolicy::validate`).
- `command_processor.rs` and `execution_processor.rs` never read `AutoApprovalStore` and never call `auto_approve_decisions`. Delegated work — from mobile, the server, or a future AI command draft — only ever executes after an explicit `APPROVE` decision recorded by the server (see `execution_processor.rs`'s `pending_decisions`/`create_execution`/`update_execution` flow). A locally enabled auto-approval policy has zero effect on proposals that originated remotely; there is no shared code path between them.
- Direct manual tools (`trash_file`, `rename_file`, `create_file`) are never auto-approved — auto approval only ever fills in decision checkboxes for a proposal, and proposals never contain those actions directly.

## Action Normalization (rename / trash / create)

The three file actions are normalized so a manual operation and its delegated equivalent behave identically at the journal layer, and so create/write can never be delegated:

- **Trash → `QUARANTINE`.** Both the manual `trash_file` and a proposal-executed trash go through the same `file_engine_cli::trash::trash_file`, so both write the identical recoverable layout (`.housemouse_trash/<op>/file` plus an `original.json` sidecar) and journal a `Trash` action. `command_processor.rs` maps local `Trash → QUARANTINE`; `execution_processor.rs` maps `QUARANTINE → Trash`. Verified by `execute::tests::proposal_trash_matches_direct_trash_on_disk_structure`.
- **Rename → `MOVE`.** A rename is a move whose destination is a sibling of the source (`folder/old.txt -> folder/new.txt`). The manual `rename_file` already journals a `Move`, and a delegated rename arrives as a `MOVE` proposal item that `execution_processor.rs` executes through the same move path — so both are journaled as `Move` and stay undoable. No separate rename action exists. Verified by `execution_processor::tests::delegated_rename_executes_as_a_journaled_move`.
- **Create is manual-only.** `create_file` is a local manual tool. There is no local `ProposalAction` for creation, so `command_processor.rs` can never submit a create, and `execution_processor.rs` explicitly refuses `CREATE_DIR` and `README_WRITE` items ("delegated write action … is not allowed"). Arbitrary delegated writes remain deferred to the README/write design (plan step 12). Verified by `execution_processor::tests::delegated_create_and_write_actions_are_refused`.

`apps/desktop/src-tauri/src/commands/file_engine.rs` owns direct manual tools and local proposal execution. Keep new remote/mobile/AI commands out of direct operation functions unless they first pass through proposal approval.
