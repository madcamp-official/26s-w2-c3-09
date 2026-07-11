# Contracts

Shared API and event contracts for HOUSEMOUSE.

## Command Contract

`events/command.schema.json` describes durable server-to-desktop commands. The server persists and delivers these commands; the desktop agent must still validate all local safety rules before doing work.

Current command types:

- `organize_files`: ask the desktop to scan a managed root and return file-move proposals.
- `apply_decisions`: apply approved/rejected proposal decisions after a fresh precondition check.
- `browse_files`: list one managed-root directory page for the mobile file browser.
- `download_file`: upload one verified source file into a server-created transfer session.
- `cancel_transfer`: stop an in-progress file transfer.

Safety assumptions:

- Commands address a `roomId`; they never carry an absolute local path.
- `relativePath` values are only hints until the desktop validates them against the managed root.
- The desktop must reject traversal, symlink, junction, and reparse-point escapes at execution time.
- File writes still require approval and a local journal before mutation.

Example commands live in `fixtures/command-*.json`.
