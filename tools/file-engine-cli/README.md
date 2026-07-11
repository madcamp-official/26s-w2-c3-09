# File Engine CLI

`file-engine-cli` is A-side local file safety tooling for HOUSEMOUSE. It analyzes a managed root, proposes safe file moves, applies user decisions, executes no-overwrite moves, writes an operation journal, and supports undo.

The CLI is intentionally JSON-first so the desktop app and future server contracts can reuse the same data shape.

## Commands

Run from `tools/file-engine-cli`:

```powershell
cargo run --quiet -- analyze <managed-root>
cargo run --quiet -- propose <managed-root>
cargo run --quiet -- precheck <managed-root> --proposal <proposal.json> --decision <decision.jsonl>
cargo run --quiet -- execute <managed-root> --proposal <proposal.json> --decision <decision.jsonl>
cargo run --quiet -- undo <managed-root>
```

On this machine, if PowerShell cannot find Cargo:

```powershell
$env:Path = "C:\Users\user\.cargo\bin;$env:Path"
```

## Flow

1. `analyze` lists files inside the managed root.
2. `propose` creates deterministic file-move proposals without changing files.
3. The user or app writes `decision.jsonl`.
4. `precheck` validates the saved proposal against the current filesystem state.
5. `execute` journals first, then performs no-overwrite moves.
6. `undo` uses `.housemouse/journal.jsonl` to restore executed moves.

## Decision JSONL

Each line is one decision.

Approved:

```json
{"proposal_id":"move:0123456789abcdef","decision":"approved"}
```

Rejected:

```json
{"proposal_id":"move:0123456789abcdef","decision":"rejected","reason":"keep this file in inbox"}
```

Rules:

- `proposal_id` must exist in the proposal file.
- A proposal can have only one decision.
- `rejected` decisions require a non-empty `reason`.
- Proposals without a decision are not executed.

## Output Contracts

### AnalyzeReport

```json
{
  "root": "C:\\managed-root",
  "files": [
    {
      "path": "inbox/note.md",
      "size_bytes": 12,
      "modified_unix_ms": 1783672855045
    }
  ]
}
```

### ProposalReport

```json
{
  "root": "C:\\managed-root",
  "proposals": [
    {
      "proposal_id": "move:0123456789abcdef",
      "action": "move",
      "from": "inbox/note.md",
      "to": "documents/note.md",
      "source_size_bytes": 12,
      "source_modified_unix_ms": 1783672855045,
      "reason": ".md files belong in documents/",
      "status": "ready"
    }
  ]
}
```

`proposal_id` is deterministic for the proposal action and normalized source/destination paths. It is stable enough for saved decisions, but the app should still use the full saved proposal file as the source of truth.

### PrecheckReport

```json
{
  "root": "C:\\managed-root",
  "checks": [
    {
      "from": "inbox/note.md",
      "to": "documents/note.md",
      "status": "ready",
      "reason": null
    }
  ]
}
```

Possible `status` values:

- `ready`
- `destination_exists`
- `missing_source`
- `source_changed`
- `rejected_path`

### ExecuteReport

```json
{
  "root": "C:\\managed-root",
  "journal_path": "C:\\managed-root\\.housemouse\\journal.jsonl",
  "executed_count": 1,
  "skipped_count": 0,
  "rejected_count": 0,
  "results": [
    {
      "from": "inbox/note.md",
      "to": "documents/note.md",
      "status": "executed",
      "reason": null
    }
  ]
}
```

Possible result `status` values:

- `executed`
- `skipped`
- `rejected`

### UndoReport

```json
{
  "root": "C:\\managed-root",
  "journal_path": "C:\\managed-root\\.housemouse\\journal.jsonl",
  "undone_count": 1,
  "skipped_count": 0,
  "results": [
    {
      "from": "documents/note.md",
      "to": "inbox/note.md",
      "status": "undone",
      "reason": null
    }
  ]
}
```

## Safety Invariants

- Only paths inside the managed root are accepted.
- Absolute input paths and parent traversal are rejected.
- Existing destination files are not overwritten.
- Execution writes journal entries before file mutation.
- Crash recovery can reconcile completed moves with missing journal completion records.
- Undo refuses to overwrite an existing original path.

## E2E Fixture

Run the fixture script from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\file-engine-cli\scripts\e2e-fixture.ps1
```

The script copies `test-fixtures/file-trees/basic` to a temp directory, runs `propose -> precheck -> execute -> undo`, and verifies that the file returns to its original path.

## Verification

```powershell
cargo fmt --check
cargo test
```
