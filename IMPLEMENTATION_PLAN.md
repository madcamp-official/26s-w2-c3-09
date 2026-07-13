# 집쥐인(MOUSEKEEPER) MVP 구현 계획

이 문서는 루트의 `MOUSEKEEPER_PLAN.md` v1.3과 `maeari_ai_collaboration_principles.txt`를 실제 개발 순서로 옮긴 실행 가이드다. 제품 구조와 기술 선택은 MOUSEKEEPER 계획서를 우선하고, 협업 문서의 무가짜 구현·환경 변수 분리·fail-fast·계약 우선·교차 리뷰 원칙을 적용한다.

## 1. 핵심 구현 원칙

- 사용자가 명시적으로 등록한 managed root만 읽고 변경한다.
- 파일 쓰기는 제안 → 사용자 승인 → 실행 직전 재검증 → journal 기록 뒤 수행한다.
- 기존 파일을 자동으로 덮어쓰거나 모바일에서 영구 삭제하지 않는다.
- AI와 캐릭터는 파일 API를 직접 호출하지 않고 구조화된 명령 초안만 만든다.
- Socket.IO는 실시간 알림 수단이며 PostgreSQL이 명령과 결과의 진실의 원천이다.
- 서버는 전체 파일 인덱스와 사용자 절대 경로를 저장하지 않는다.
- 외부 연동이 없으면 fake 성공이나 임시 provider 대신 명확한 `UNCONFIGURED` 오류를 노출한다.
- MOUSEKEEPER 식별자와 휴지통 이름 등은 v1.3 명칭을 기준으로 통일한다.

## 2. 목표 아키텍처

```text
Flutter Mobile
  ↕ REST + Socket.IO
NestJS/Fastify Server ─ PostgreSQL
  ├─ Redis/Valkey: presence TTL, Socket.IO adapter, 짧은 lock
  ├─ BullMQ Worker: 알림, 재시도, 전송 object 만료 삭제
  └─ S3 호환 Storage: P0 만료형 전송 object와 opt-in P1 cache
  ↕ REST + Socket.IO
Tauri Desktop
  ├─ React/Vite: 관리 UI와 캐릭터 overlay
  ├─ Rust: path guard, scan, watcher, rule, operation, transfer
  └─ SQLite WAL: 파일 인덱스, operation journal, inbox/outbox
```

### 저장소 경계

- `apps/desktop`: Tauri 2 + React/Vite + Rust 파일 엔진. 로컬 파일 I/O는 Rust만 수행한다.
- `apps/mobile`: Flutter + Riverpod + Freezed + GoRouter + Drift 기반 Android 우선 앱이다.
- `apps/server`: NestJS/Fastify 모듈러 모놀리스로 인증, 권한, 영속 queue, REST와 WebSocket을 담당한다.
- `apps/worker`: BullMQ 작업자로 알림과 object lifecycle을 담당한다.
- `packages/contracts`: OpenAPI와 JSON Schema 기반 공개 계약이다.
- `packages/database`: Drizzle schema와 SQL migration의 서버 내부 전용 영역이다.
- `tools`: production 구현과 계약을 사용하는 file-engine CLI와 desktop-agent simulator다.
- `test-fixtures`: 경로 공격, 충돌, crash recovery, 전송 오류 회귀 테스트 입력이다.

## 3. 첫 번째 Vertical Slice

모든 화면을 동시에 구현하지 않고 다음 흐름을 가장 먼저 끝까지 연결한다.

```text
Android에서 정리 명령 생성
→ Server가 QUEUED 상태로 PostgreSQL에 저장
→ 온라인 Desktop이 명령 수신
→ 로컬 파일 분석 후 Proposal 생성
→ Android에서 전체 승인 또는 거절
→ Desktop이 source identity와 destination 충돌 재검증
→ operation journal 기록
→ no-overwrite 이동 또는 MOUSEKEEPER 휴지통 격리
→ 실행 결과와 청결도 동기화
→ Desktop에서 undo
```

첫 통합에서는 복합 규칙, 자유 채팅, 커스터마이징을 제외한다. 단일 확장자 또는 기간 규칙과 한 개의 room 시연에 집중하되 데이터 모델은 처음부터 여러 room을 지원한다.

## 4. 단계별 구현 순서

### Phase 0 — 계약과 파일 안전 POC

1. `Command`, `Proposal`, `Decision`, `ExecutionResult`, event envelope의 ID, 상태와 오류 코드를 공동 확정한다.
2. Rust path guard에서 canonical root, 상대 경로 정규화, `..`, 절대 경로, symlink, junction, reparse point 우회를 차단한다.
3. no-overwrite, journal-before-write, MOUSEKEEPER 휴지통, undo를 fixture로 검증한다.
4. 서버는 시작 시 필수 환경 변수를 검증하고 누락되면 즉시 중단한다.

완료 조건은 schema validation과 scan → proposal → execute → undo 테스트가 반복 실행되는 것이다.

### Phase 1 — 인증·페어링·Presence

Firebase ID token을 서버에서 검증하고 Desktop device token은 OS keychain에 보관한다. heartbeat TTL은 Redis/Valkey에 두고 device와 pairing 기록은 PostgreSQL에 저장한다. Socket.IO event 유실 후 REST cursor replay가 가능해야 한다.

### Phase 2 — Managed Root·Scan·청결도

네이티브 폴더 선택기로 등록한 canonical root만 SQLite에 기록한다. 최초 scan 뒤 watcher를 시작하고 overflow와 누락은 주기 reconcile scan으로 복구한다. 서버에는 room metadata와 집계된 snapshot만 전송한다.

### Phase 3 — 제안·승인·안전 실행

결정론적 Rule DSL로 proposal을 만들고 승인에는 idempotency key를 사용한다. 로컬 실행 상태는 `JOURNALED → APPLIED → VERIFIED`로 관리한다. 승인 뒤 원본이 달라졌으면 `STALE`, 목적지가 존재하면 `CONFLICT`로 종료하고 파일은 변경하지 않는다.

### Phase 4 — P0 온라인 파일 가져오기

파일 정리와 별도의 읽기 전용 `FileTransfer` 상태 머신을 사용한다. browse와 transfer 요청마다 managed root와 file identity를 재검증하고 chunk 전송 후 SHA-256을 확인한다. 모바일 ACK 또는 TTL 만료 시 worker가 object를 삭제한다. 연결 단절은 `DEVICE_OFFLINE`, 원본 변경은 `SOURCE_CHANGED`, cursor 무효화는 `CURSOR_INVALIDATED`로 명시한다.

### Phase 5 — 제품 경험·오프라인·배포

캐릭터 상태는 실제 domain event에서만 파생한다. 모바일에 loading, empty, offline, error 상태를 구현한다. offline command replay, device revoke, 로그 redaction, Windows installer, Android build와 동일 환경 E2E를 검증한다.

### Phase 6 — P1 스마트 캐시

P0 출시 뒤 feature flag 기본값 `false`로 시작한다. 사용자 opt-in → 후보 metadata → 서버 quota 예약 → 승인된 upload target → 암호화 → checksum/version 검증 순서를 강제한다. `AVAILABLE`과 최신성을 분리하며 PC offline에서는 `last_verified_at`과 `UNVERIFIED_OFFLINE`을 표시한다.

## 5. 데이터 저장 원칙

- PostgreSQL: users, devices, rooms, commands, proposals, decisions, executions, transfer metadata, audit summary.
- Desktop SQLite: canonical roots, file index, operation journal, inbox/outbox, 로컬 transfer 상태.
- Redis/Valkey: presence TTL, Socket.IO adapter, 단기 lock. 영속 명령 저장소로 사용하지 않는다.
- Object storage: P0 임시 object와 사용자가 켠 P1 암호화 object만 저장한다.
- Mobile Drift: 표시 cache와 mutation outbox. 서버 성공을 임의로 가정하지 않는다.

모든 mutation은 `id`, `userId`, `deviceId`, `roomId`, `idempotencyKey`, `createdAt`을 가진다. 모든 event는 `eventId`, `eventType`, `schemaVersion`, `sequence`, `occurredAt`, `correlationId`, `payload` envelope를 사용한다.

## 6. 담당과 협업 방식

A는 `apps/desktop/src-tauri`, desktop files/admin, file-engine fixture를 소유한다. B는 mobile, server, worker, character UI를 소유한다. overlay와 `packages/contracts`는 공동 리뷰한다.

API 변경은 계약을 먼저 수정하고 양쪽 합의 뒤 구현한다. 공개 필드는 즉시 제거하지 않고 deprecated 병행 기간을 둔다. 가짜 API, dummy/seed data, 동작하지 않는 성공 placeholder를 만들지 않는다. 프레임워크 코드는 공식 생성기로 초기화하고 실제 기능과 테스트를 함께 커밋한다.

## 7. 품질 게이트

- 공통: `pnpm typecheck`, `pnpm test`, `pnpm build`
- Rust: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
- Flutter: `flutter analyze`, `flutter test`
- 파일 작업 PR: path guard, 충돌, journal, undo fixture 테스트 필수
- API PR: OpenAPI/event schema와 idempotency 테스트 필수
- UI PR: loading, empty, offline, error 상태 필수

출시 전 승인 없는 쓰기, root 이탈, reparse 우회, overwrite, journal 없는 쓰기, 중복 승인, crash recovery, offline queue, room 간 접근, checksum 불일치와 TTL object 삭제를 자동 회귀 테스트로 차단한다.

## 8. 바로 시작할 작업

1. `CODEOWNERS`의 GitHub 계정과 실제 역할을 확인한다.
2. 공식 생성기로 Tauri, Flutter, NestJS 프로젝트를 해당 디렉터리에 초기화한다.
3. `packages/contracts`에서 command vertical slice 계약을 공동 동결한다.
4. A는 path guard와 operation journal POC를 구현한다.
5. B는 PostgreSQL command queue와 desktop-agent simulator를 구현한다.
6. 온라인 command 한 건의 저장 → 수신 → 결과 replay를 첫 통합 목표로 삼는다.
