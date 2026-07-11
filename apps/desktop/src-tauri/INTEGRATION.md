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
analyze_root(root: String) -> Result<AnalyzeReport, String>
propose_file_changes(root: String) -> Result<ProposalReport, String>
precheck_file_changes(root: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> Result<PrecheckReport, String>
execute_file_changes(root: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> Result<ExecuteReport, String>
undo_last_file_operation(root: String) -> Result<UndoReport, String>
```

`register_managed_root` returns the canonical managed-root path and a display name. The frontend should pass the returned `root` value into later file-engine commands instead of reusing the raw folder-picker string.

The `tauri::command` macro is behind the `tauri-commands` feature so the module can be unit-tested before the full desktop shell is wired.

Study note: this is the Rust version of a controller layer. The command receives UI-friendly data, calls the domain function, and converts errors into strings that Tauri can return to the frontend.

The Tauri app entrypoint is now in `src/lib.rs` and registers the commands through `tauri::generate_handler!`. `ManagedRootStore` is attached with `Builder::manage(...)`, so command calls share one in-memory root registry for the running app process.

Current bootstrapping note: `tauri.conf.json` points `frontendDist` at `../src` only so the Rust/Tauri command layer can compile before a real frontend build output exists. Replace it with `../dist` once the desktop UI build is wired.

## Proposed Tauri Commands

These command names are the app-facing bridge contract. They should return JSON-compatible structs matching `tools/file-engine-cli/README.md`.

```rust
register_managed_root(path: String) -> ManagedRoot
analyze_root(root: String) -> AnalyzeReport
propose_file_changes(root: String) -> ProposalReport
precheck_file_changes(root: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> PrecheckReport
execute_file_changes(root: String, proposal: ProposalReport, decisions: Vec<DecisionEntry>) -> ExecuteReport
undo_last_file_operation(root: String) -> UndoReport
```

## Recommended Desktop Flow

1. User selects a folder with the OS folder picker.
2. Tauri canonicalizes and stores the managed root.
3. UI calls `analyze_root` for a read-only preview.
4. UI calls `propose_file_changes`.
5. UI renders proposals and collects approvals/rejections.
6. UI requires a reason for each rejection.
7. UI calls `precheck_file_changes`.
8. If every approved item is `ready`, UI asks for final confirmation.
9. UI calls `execute_file_changes`.
10. UI displays `executed`, `skipped`, and `rejected` results separately.
11. History screen calls `undo_last_file_operation` when the user asks to undo.

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
