# Desktop File Engine Integration

This document prepares the Tauri desktop side to call the A-owned file engine safely.

The current implementation lives in `tools/file-engine-cli`. Until the desktop Rust crate is fully wired, the app should treat the CLI JSON contracts as the reference behavior.

## Integration Goal

Expose local file operations to the desktop UI through explicit Tauri commands while preserving the same safety rules:

- user-selected managed roots only
- proposal before execution
- user approval before mutation
- precheck immediately before execution
- no overwrite
- journal before mutation
- undo visibility

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
- Add Tauri command modules under `apps/desktop/src-tauri/src/commands/`.
- Move shared file-engine logic into a reusable Rust module or crate before duplicating CLI logic.

Preferred medium-term structure:

```text
tools/file-engine-cli/
  src/main.rs              # CLI argument parsing only
  src/*.rs                 # temporary engine modules

apps/desktop/src-tauri/
  src/commands/files.rs    # Tauri invoke handlers

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
