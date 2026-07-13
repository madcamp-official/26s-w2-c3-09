# File Engine CLI

`file-engine-cli` is A-side local file safety tooling for MOUSEKEEPER. It analyzes a managed root, proposes safe file moves, applies user decisions, executes no-overwrite moves, writes an operation journal, and supports undo.

The CLI is intentionally JSON-first so the desktop app and future server contracts can reuse the same data shape.

## Commands

Run from `tools/file-engine-cli`:

```powershell
cargo run --quiet -- analyze <managed-root>
cargo run --quiet -- browse <managed-root> [--path <relative-path>]
cargo run --quiet -- index <managed-root>
cargo run --quiet -- search <managed-root> <query>
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
3. `rules` prints the rule set that `propose` will apply (the root's `.mousekeeper/rules.json`, or a built-in default when that file is absent).
4. `propose` evaluates those rules against the files and creates deterministic file-move proposals without changing files.
5. The user or app writes `decision.jsonl`.
6. `precheck` validates the saved proposal against the current filesystem state.
7. `execute` journals first, then performs no-overwrite moves.
8. `undo` uses the operation journal to restore executed moves.
9. If a journal row is unreadable, history reporting stays available (it reports everything before the bad row plus where it broke) but `execute`/`undo` refuse to run until `recover-journal` quarantines the journal and starts a fresh one.

## Storage

Per-root state lives in a SQLite database at `<managed-root>/.mousekeeper/mousekeeper.db`:

- `file_index` — the scanned file metadata `index` populates and `search` reads.
- `operation_journal` — the append-only execute/undo event log (formerly `journal.jsonl`).

`index` rebuilds `file_index` from a fresh scan; `search` does a case-insensitive substring match over the indexed relative paths. On the desktop app, the managed-root registry lives in a separate app-level `managed-roots.db`.

## Rule DSL

Organizing rules are **data, not code**. Each managed root may carry `.mousekeeper/rules.json`; when absent, a built-in default (extension buckets: documents/images/archives) is used. Because a rule can come from an untrusted source (e.g. a natural-language request translated to JSON server-side, or a per-user file), the rule set is validated before it is ever applied.

```json
{
  "version": 1,
  "rules": [
    {
      "id": "old-invoices",
      "priority": 5,
      "when": {
        "extension_in": ["pdf"],
        "older_than_days": 30,
        "name_matches": "*invoice*"
      },
      "then": { "move_to": "documents/billing" }
    }
  ]
}
```

- `version` must equal the schema version the build supports (currently `1`); a mismatch is rejected, not reinterpreted.
- `when` conditions are ANDed. A rule must have at least one condition — an empty `when` is rejected so it can never mean "move everything".
  - `extension_in`: bare, lowercase extensions (`"pdf"`, not `".pdf"`).
  - `older_than_days`: file modified time must be at least this many days in the past. A file with no known modified time never matches.
  - `name_matches`: case-insensitive glob against the file name (`*` any run, `?` one char).
- `then.move_to` is a directory relative to the managed root. Absolute paths and `..` traversal are rejected at load time (same boundary as `PathGuard`).
- `priority` orders evaluation (lower first, ties broken by `id`); the **first fully-matching rule wins** for each file, keeping results deterministic.
- Unknown JSON fields are rejected. A present-but-invalid `rules.json` is a hard error — the engine will not silently fall back to the default and hide a misconfiguration.

Run `file-engine-cli rules <managed-root>` to see the effective rule set (default or loaded) as JSON.

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
  ],
  "skipped_entries": []
}
```

Real-world folders such as Downloads can contain unreadable, temporary, or reparse-point entries.
Read-only scans skip those entries and report them in `skipped_entries`; mutation commands still
fail closed before changing files.

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
  ],
  "skipped_entries": []
}
```

Omit `--path` (or pass an empty string) to browse the managed root itself. `entries` is sorted directories-first, then alphabetically. Directory entries always carry `size_bytes: null`.
Entries that cannot be inspected safely are omitted from `entries` and listed in `skipped_entries`.

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
  "journal_path": "C:\\managed-root\\.mousekeeper\\mousekeeper.db",
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
  "journal_path": "C:\\managed-root\\.mousekeeper\\mousekeeper.db",
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
  "journal_path": "C:\\managed-root\\.mousekeeper\\mousekeeper.db",
  "quarantined_path": "C:\\managed-root\\.mousekeeper\\journal.jsonl.corrupted-1783672855045"
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
