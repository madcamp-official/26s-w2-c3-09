# Project Memory for Codex

## User Role

- The user is responsible for role **A** in this project.
- Treat "my role", "A side", and "what I should do" as the A ownership area unless the user says otherwise.
- In CODEOWNERS, A corresponds to `@nounmoumn`.

## Must-Read Context

- Read `MOUSEKEEPER_PLAN.md` for the overall MVP plan, ownership, schedules, contracts, and A/B responsibilities.
- Read `AI_implement_rule.txt` for AI implementation rules and project engineering constraints.
- If these files appear garbled because of encoding, still use the stable structural signals from headings, paths, CODEOWNERS, and existing repository layout.

## A Ownership Summary

A owns the Desktop Agent and local file safety domain:

- Tauri desktop shell and native bridge
- Rust local file engine
- Windows/macOS filesystem abstraction
- managed root registration, canonicalization, and boundary checks
- blocking traversal, symlink, junction, and reparse point escape paths
- watcher, scan, reconcile, and file index
- rule evaluator / Rule DSL
- proposal generation and precondition checks
- no-overwrite file operations
- operation journal, crash recovery, quarantine, and undo
- README generation/edit proposals and local diff application
- local history/logging
- system tray, autostart, Windows build
- online file browse adapter
- file transfer validation, chunking, checksum, cancellation, and source-change handling
- local usage event aggregation for later smart-cache work

## A Primary Paths

- `apps/desktop/src-tauri/`
- `apps/desktop/src/features/files/`
- `apps/desktop/src/features/admin/`
- `apps/desktop/src/features/overlay/` for native shell/bridge concerns
- `packages/rule-fixtures/`
- `test-fixtures/file-trees/`
- `test-fixtures/file-transfers/`
- `test-fixtures/smart-cache/usage-events/`
- `tools/file-engine-cli/`

## A Edit Boundaries for Easier Merges

### A Can Freely Edit

These paths are A-owned. Prefer keeping A feature work inside these folders when possible:

- `apps/desktop/src-tauri/`
- `apps/desktop/src/features/files/`
- `apps/desktop/src/features/admin/`
- `packages/rule-fixtures/`
- `test-fixtures/file-trees/`
- `test-fixtures/file-transfers/`
- `test-fixtures/smart-cache/usage-events/`
- `tools/file-engine-cli/`

### A Can Edit With Care

These paths are shared or have mixed ownership. Keep changes small, contract-focused, and easy to review:

- `apps/desktop/src/features/overlay/`
  - A may edit native shell, bridge, window wiring, event plumbing, and safety-related integration.
  - Avoid character presentation, motion, and product UI decisions unless coordinated with B.
- `packages/contracts/`
  - Edit only when an A feature needs a schema/API/event contract change.
  - Coordinate with B before breaking or renaming fields.
- `docs/`
  - A may add ADRs, safety docs, file-engine docs, E2E notes, and implementation notes.
- `.env.example`
  - Edit only for required environment key additions/removals.
  - Keep values empty; do not add fake placeholders or secrets.
- `README.md`
  - Keep edits narrow and factual.
  - Avoid large rewrites while B is also changing product/server docs.
- `package.json` and `pnpm-workspace.yaml`
  - Edit only when adding real workspace packages, scripts, or dependencies needed by A-owned code.
  - Avoid formatting-only churn.

### Avoid Editing Unless Explicitly Needed

These paths are B-owned or likely to cause merge friction:

- `apps/mobile/`
- `apps/server/`
- `apps/worker/`
- `infra/`
- `apps/desktop/src/features/character/`
- `packages/character-assets/`
- `packages/design-tokens/`
- B-owned product UX, character state/motion, server control plane, auth, API implementation, WebSocket server, object lifecycle, deployment config, and mobile screens.

### Merge-Friendly Habits

- Keep A branches focused on one vertical slice or one local-file safety concern.
- Avoid drive-by formatting in shared files.
- Put cross-boundary changes in separate commits when practical.
- For contract changes, update schema, fixture, and docs together.
- Before editing shared files, check the latest `origin/main` and current local diff.
- Prefer additive contract changes over breaking renames/deletions.
- Do not move files across A/B ownership boundaries without explicit coordination.

## Shared / Contract Paths

- `packages/contracts/`
- `docs/`
- `.env.example`
- `README.md`

Coordinate contract changes with B. Do not make breaking contract changes casually.

## B Ownership Summary

B owns product/cloud/mobile/character/server areas:

- `apps/mobile/`
- `apps/server/`
- `apps/worker/`
- `infra/`
- `apps/desktop/src/features/character/`
- character UI, motion, mobile UX, server control plane, auth, API, WebSocket, heartbeat, deployment, and object lifecycle

## Engineering Rules from `AI_implement_rule.txt`

- No fake success paths.
- No production mock files just to satisfy structure.
- No hardcoded dummy data in app logic.
- No fake API responses such as unconditional `{ success: true }`.
- No secrets, API keys, tokens, or provider credentials in code.
- `.env.example` should list keys only, without real values or placeholder secrets.
- Missing required environment variables should fail fast with explicit errors.
- Unconfigured provider features should surface explicit `UNCONFIGURED` or equivalent states, not pretend to work.
- AI-generated or AI-assisted outputs must pass schema/DTO validation before entering product logic.
- User approval must not be bypassed for file/database write operations.

## Default Working Style

- Prefer implementing A-owned local safety and file-engine work first when the user asks for A-side progress.
- Keep changes scoped to the ownership boundaries above.
- For file-operation features, include fixture or integration tests when practical.
- Preserve local file safety invariants: explicit managed roots, boundary validation, no overwrites, journal before mutation, recoverable operations, and undo visibility.
- When adding or changing code, include a brief study-oriented explanation for the user.
- Keep explanations concise: what changed, why it is needed, and which local pattern or safety rule it follows.
- For non-obvious code, add a short code comment in the file. For obvious code, explain it in the response instead of cluttering the source.
- When introducing a new concept, name the concept plainly and connect it to the exact file or function changed.
