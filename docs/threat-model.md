# Threat Model

MouseKeeper controls local files through a paired desktop agent. The safest
default is to treat every network payload as untrusted until the local agent
revalidates it against the managed root and current filesystem state.

## Assets

- User files inside explicitly registered managed roots.
- Files outside managed roots, which must remain unreachable.
- Desktop device token and mobile Firebase session.
- Operation journal, trash metadata, undo history, and local SQLite indexes.
- PostgreSQL command/proposal/decision/execution state.
- Transfer and smart-cache objects in S3-compatible storage.
- Smart-cache encryption keys and metadata.
- Chat messages and AI-generated command/rule drafts.

## Trust boundaries

| Boundary | Trusted side | Untrusted input |
|---|---|---|
| Mobile → server | Authenticated API handlers | Request body, idempotency key, room/device IDs |
| Server → desktop | Device-authenticated command queue | Command arguments and relative paths |
| Desktop → filesystem | Path guard after canonicalization | Symlinks, junctions, reparse points, renamed files |
| Desktop → object storage | Verified source file stream | Signed URL, object key, network interruption |
| AI provider → server | Zod/DTO validation layer | Model JSON, tool arguments, natural language |
| Socket.IO → clients | Durable replay cursor | Event delivery timing and duplicates |

## Threats and required controls

| Threat | Control |
|---|---|
| Path traversal with `..`, absolute path, drive prefix, or UNC path | Shared relative-path schema and desktop canonical root boundary check |
| Symlink/junction/reparse escape | Desktop refuses link/reparse traversal before browse, proposal, transfer, and execution |
| Source swap after approval | Source identity, size, and mtime are rechecked immediately before mutation/streaming |
| Destination overwrite | Execution uses no-overwrite checks; conflicts are skipped, not auto-renamed |
| Duplicate approval or retry | Idempotency keys and durable execution state prevent second mutation |
| Socket.IO event loss | PostgreSQL sync events and replay cursor are the source of truth |
| Multi-process realtime split brain | Socket.IO Redis adapter propagates room broadcasts across API processes |
| Device revoke race | Revocation writes durable events and disconnects device sockets; clients replay on reconnect |
| Object storage leak | Private bucket/IAM role, signed URL TTL, checksum/HEAD validation, worker cleanup retries |
| Smart-cache plaintext exposure | Desktop-side AES-256-GCM before upload; server stores ciphertext metadata only |
| AI invents executable action | AI output is only a draft; server validates schema and user must approve before desktop precheck |
| Secret or path leakage in logs | Request logging and Sentry redaction remove tokens, request bodies, and absolute paths |

## Explicit non-goals for MVP

- The server does not directly access a user's local filesystem.
- AI does not receive shell/file execution tools.
- Socket.IO does not guarantee durable delivery.
- Smart-cache is not full-folder sync.
- Missing Sentry, Rive, OpenAI, release keystore, or mobile key-sync secrets do
  not produce fake success states.

## Open risks

- Android release signing, production Google login, and terminated/background
  FCM still need release-key validation.
- Mobile smart-cache decryption key sync and AES-GCM tag verification remain
  `UNCONFIGURED`.
- Full Android ↔ server ↔ Desktop release E2E automation is not yet complete.
- Load balancer/WebSocket topology should be validated in the deployment
  environment even though the server now installs the Redis adapter.
- Long-running watcher overflow and 100,000-entry performance soak tests remain
  to be recorded.

## Security review checklist

- New API accepts only IDs and relative paths, never absolute local paths.
- New mutation has idempotency and owner/room/device authorization checks.
- New file write path journals before mutation and has undo visibility.
- New realtime event has replayable PostgreSQL state behind it.
- New provider integration has an `UNCONFIGURED` state and no committed secrets.
- New logs, metrics, and errors avoid user path/token/body disclosure.
