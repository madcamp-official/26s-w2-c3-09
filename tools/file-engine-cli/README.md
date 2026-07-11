# File Engine CLI

`file-engine-cli` is A-side local file safety tooling for HOUSEMOUSE. It analyzes a managed root, proposes safe file moves, applies user decisions, executes no-overwrite moves, writes an operation journal, and supports undo.

The CLI is intentionally JSON-first so the desktop app and future server contracts can reuse the same data shape.

## Commands

Run from `tools/file-engine-cli`:

```powershell
cargo run --quiet -- analyze <managed-root>
cargo run --quiet -- browse <managed-root> [--path <relative-path>]
cargo run --quiet -- propose <managed-root>
cargo run --quiet -- precheck <managed-root> --proposal <proposal.json> --decision <decision.jsonl>
cargo run --quiet -- execute <managed-root> --proposal <proposal.json> --decision <decision.jsonl>
cargo run --quiet -- undo <managed-root>
cargo run --quiet -- recover-journal <managed-root>
```

On this machine, if PowerShell cannot find Cargo:

```powershell
$env:Path = "C:\Users\user\.cargo\bin;$env:Path"
```

## Flow

1. `analyze` lists files inside the managed root.
2. `browse` lists one directory level (folders then files) so a UI can navigate the tree without a full recursive scan.
3. `propose` creates deterministic file-move proposals without changing files.
4. The user or app writes `decision.jsonl`.
5. `precheck` validates the saved proposal against the current filesystem state.
6. `execute` journals first, then performs no-overwrite moves.
7. `undo` uses `.housemouse/journal.jsonl` to restore executed moves.
8. If a journal line is unparseable, history reporting stays available (it reports everything before the bad line plus where it broke) but `execute`/`undo` refuse to run until `recover-journal` quarantines the broken file and starts a fresh one.

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

### BrowseReport

```json
{
  "root": "C:\\managed-root",
  "path": "inbox",
  "entries": [
    {
      "name": "attachments",
      "path": "inbox/attachments",
      "is_dir": true,
      "size_bytes": null,
      "modified_unix_ms": null
    },
    {
      "name": "note.md",
      "path": "inbox/note.md",
      "is_dir": false,
      "size_bytes": 12,
      "modified_unix_ms": 1783672855045
    }
  ]
}
```

Omit `--path` (or pass an empty string) to browse the managed root itself. `entries` is sorted directories-first, then alphabetically. Directory entries always carry `size_bytes: null`.

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

### JournalRecoveryReport

```json
{
  "root": "C:\\managed-root",
  "journal_path": "C:\\managed-root\\.housemouse\\journal.jsonl",
  "quarantined_path": "C:\\managed-root\\.housemouse\\journal.jsonl.corrupted-1783672855045"
}
```

`recover-journal` refuses with an error if the journal has no unparseable line, so it cannot be used to silently discard a healthy history. Operations recorded before the corrupt line are no longer undoable through the app once the file is quarantined; the quarantined file itself is kept on disk, not deleted.

## Safety Invariants

- Only paths inside the managed root are accepted.
- Absolute input paths and parent traversal are rejected.
- Existing destination files are not overwritten.
- Execution writes journal entries before file mutation.
- Crash recovery can reconcile completed moves with missing journal completion records.
- Undo refuses to overwrite an existing original path.
- A journal with an unparseable line blocks `execute`/`undo` until `recover-journal` quarantines it; history reporting still shows everything recorded before the bad line.

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
