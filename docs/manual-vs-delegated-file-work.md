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

`apps/desktop/src-tauri/src/command_processor.rs` is the delegated boundary. It accepts cleanup commands and turns them into proposal submissions. Unsupported command types that look like direct file tools are skipped or failed explicitly instead of being executed.

`apps/desktop/src-tauri/src/commands/file_engine.rs` owns direct manual tools and local proposal execution. Keep new remote/mobile/AI commands out of direct operation functions unless they first pass through proposal approval.
