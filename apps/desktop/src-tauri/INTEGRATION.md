# Desktop File Engine Integration

This document prepares the Tauri desktop side to call the A-owned file engine safely.

The shared implementation currently lives in `tools/file-engine-cli`. The CLI now exposes those modules through `src/lib.rs`, and the desktop crate calls the same code from `apps/desktop/src-tauri/src/commands/file_engine.rs`.

## Integration Goal

Expose local file operations to the desktop UI through explicit Tauri commands while preserving the same safety rules:

- user-selected managed roots only
- proposal before execution
- user approval before mutation
- precheck immediately before execution
- no overwrite
- journal before mutation
- undo visibility

## Implemented Command Module

The first desktop bridge is implemented in:

```text
apps/desktop/src-tauri/src/commands/file_engine.rs
```

It intentionally stays thin. Each command validates input through the existing file-engine functions instead of duplicating file logic in the Tauri layer.

Implemented commands:

```rust
register_managed_root(path: String) -> Result<ManagedRoot, String>
list_managed_roots() -> Result<Vec<ManagedRoot>, String>
analyze_root(root_id: String) -> Result<AnalyzeReport, String>
propose_file_changes(root_id: String) -> Result<ProposalReport, String>
precheck_file_changes(root_id: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> Result<PrecheckReport, String>
execute_file_changes(root_id: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> Result<ExecuteReport, String>
undo_last_file_operation(root_id: String) -> Result<UndoReport, String>
```

`register_managed_root` returns a `root_id`, the canonical managed-root path, and a display name. The frontend should pass the returned `root_id` into later file-engine commands instead of carrying the raw path.

The `tauri::command` macro is behind the `tauri-commands` feature so the module can be unit-tested before the full desktop shell is wired.

Study note: this is the Rust version of a controller layer. The command receives UI-friendly data, calls the domain function, and converts errors into strings that Tauri can return to the frontend.

The Tauri app entrypoint is now in `src/lib.rs` and registers the commands through `tauri::generate_handler!`. `ManagedRootStore` is attached with `Builder::manage(...)`, so command calls share one root registry for the running app process.

Managed roots are persisted to the app data directory as `managed-roots.json`. On startup, `setup(...)` loads this file into `ManagedRootStore`; on `register_managed_root`, the store writes the updated root list back to disk.

Study note: this keeps the UI from carrying raw folder-picker paths as the source of truth. The registered `root_id` becomes the app-facing handle, and later commands resolve it through `ManagedRootStore` before touching files.

The desktop frontend is a minimal React/Vite app under `apps/desktop/src`. It calls these commands through `@tauri-apps/api/core` from `features/files/fileEngineApi.ts`.

Current frontend behavior:

- `Browse` opens the OS folder picker through `@tauri-apps/plugin-dialog`.
- `Demo` fills the local UI fixture path for fast manual testing.
- rejected proposals require a non-empty reason before precheck or execute.
- execute runs precheck first, blocks non-ready items, and asks for final confirmation.
- execute and undo results are saved in browser `localStorage` and shown in the History panel.

## Proposed Tauri Commands

These command names are the app-facing bridge contract. They should return JSON-compatible structs matching `tools/file-engine-cli/README.md`.

```rust
register_managed_root(path: String) -> ManagedRoot
list_managed_roots() -> Vec<ManagedRoot>
analyze_root(root_id: String) -> AnalyzeReport
propose_file_changes(root_id: String) -> ProposalReport
precheck_file_changes(root_id: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> PrecheckReport
execute_file_changes(root_id: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> ExecuteReport
undo_last_file_operation(root_id: String) -> UndoReport
```

## Recommended Desktop Flow

1. User selects a folder with the OS folder picker.
2. Tauri canonicalizes and stores the managed root, returning `root_id`.
3. UI calls `analyze_root(root_id)` for a read-only preview.
4. UI calls `propose_file_changes(root_id)`.
5. UI renders proposals and collects approvals/rejections.
6. UI requires a reason for each rejection.
7. UI calls `precheck_file_changes(root_id, proposal, decisions)`.
8. If every approved item is `ready`, UI asks for final confirmation.
9. UI calls `execute_file_changes(root_id, proposal, decisions)`.
10. UI displays `executed`, `skipped`, and `rejected` results separately.
11. History screen calls `undo_last_file_operation(root_id)` when the user asks to undo.

## Data Mapping

Use these existing CLI structs as the initial app DTOs:

- `AnalyzeReport`
- `ProposalReport`
- `DecisionEntry`
- `PrecheckReport`
- `ExecuteReport`
- `UndoReport`

For the first desktop integration, avoid inventing a different UI DTO shape. Convert only at the UI boundary if needed.

## Error Handling

Do not turn file-engine errors into fake success states.

Recommended UI mapping:

| Engine condition | UI state |
|---|---|
| unknown proposal id | stale approval data; refresh proposals |
| duplicate decision | invalid approval state; block execution |
| rejected without reason | invalid rejection; request reason |
| destination exists | conflict; show target path |
| missing source | stale proposal; refresh |
| source changed | stale proposal; refresh |
| rejected path | unsafe path; block execution |
| journal read/parse error | recovery needed; block mutation |

## Implementation Notes

Short term:

- Keep using `tools/file-engine-cli` for verification.
- Keep Tauri command modules under `apps/desktop/src-tauri/src/commands/`.
- Keep shared file-engine behavior in the reusable `file-engine-cli` library until a dedicated core crate is worth splitting out.

Preferred medium-term structure:

```text
tools/file-engine-cli/
  src/lib.rs               # shared engine module exports
  src/main.rs              # CLI argument parsing only
  src/*.rs                 # engine modules

apps/desktop/src-tauri/
  src/commands/file_engine.rs # Tauri invoke handlers

packages or crates/
  file-engine-core/        # shared analyzer/proposal/precheck/execute/undo logic
```

The key refactor is to keep command parsing separate from file-engine behavior. The CLI and Tauri app should call the same core functions.

## Verification Before UI Wiring

From `tools/file-engine-cli`:

```powershell
cargo fmt --check
cargo test
```

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\file-engine-cli\scripts\e2e-fixture.ps1
```

Do not wire the UI to mutation commands until these pass.
