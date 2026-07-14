# ADR 0001: Local-first file authority with durable cloud coordination

## Status

Accepted.

## Context

MouseKeeper lets mobile users request changes to files that physically live on a
desktop computer. The cloud service is necessary for auth, pairing, durable
queues, audit, realtime notifications, object lifecycle, and mobile UI state.
It must not become the authority that decides whether a local file path is safe
to mutate.

The implementation also differs from the original worker plan in one respect:
the current worker uses PostgreSQL-backed durable jobs and polling instead of
BullMQ. Redis/Valkey is still used for presence, rate limiting, short locks, and
Socket.IO fan-out.

## Decision

1. The desktop agent is the authority for local filesystem safety.
2. The server stores command/proposal/decision/execution state, but command
   arguments remain untrusted hints until the desktop validates them.
3. All file-changing operations must pass:
   - explicit managed-root binding;
   - relative-path validation;
   - canonical boundary check;
   - symlink/junction/reparse escape rejection;
   - source identity/precondition check;
   - no-overwrite destination check;
   - journal-before-write.
4. Socket.IO is a latency optimization. PostgreSQL sync events and cursor replay
   are the source of truth.
5. PostgreSQL durable jobs are acceptable for the current single-cluster MVP
   because they keep retry state, object deletion, notification jobs, and audit
   close to the same transactional store.
6. Redis-backed Socket.IO adapter is required for multi-process realtime room
   fan-out, but deployment topology still needs release E2E validation.
7. Missing external providers must surface `UNCONFIGURED` instead of local
   fallback success.

## Consequences

Positive:

- The server never needs absolute local paths.
- Mobile can recover from missed realtime events through replay.
- Desktop crash recovery and undo remain local and inspectable.
- A fake or malformed AI/server command cannot directly mutate files.
- The MVP can run with fewer moving infrastructure parts than a BullMQ-first
  worker topology.

Tradeoffs:

- Some operations require desktop availability or replay before mobile catches
  up.
- Release E2E must include a real desktop, mobile app, PostgreSQL, Redis, and
  object storage.
- PostgreSQL polling workers need careful indexes, leases, and retry limits as
  scale grows.
- A future BullMQ migration may still be useful when job volume exceeds the MVP
  deployment profile.

## Revisit triggers

- Worker queues begin causing database load or lock contention.
- More than one API process is deployed behind a load balancer.
- Smart-cache decryption key sync becomes production enabled.
- macOS/iOS support changes local authority or background execution behavior.
- A provider or AI feature needs new draft/approval state.

## Related documents

- [MVP scope](../MVP_SCOPE.md)
- [File safety invariants](../FILE_SAFETY_INVARIANTS.md)
- [File transfer threat model](../FILE_TRANSFER_THREAT_MODEL.md)
- [Threat model](../threat-model.md)
- [E2E scenarios](../e2e-scenarios.md)
