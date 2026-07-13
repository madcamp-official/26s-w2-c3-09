# MOUSEKEEPER (26s-w2-c3-09)

## Recent implementation notes (2026-07-13)

- Latency hardening: mobile Home keeps the 5-second authoritative safety check for device/room liveness, but it no longer turns that loop into a full `/v1/home/summary` refresh.
- Realtime Home updates remain item-scoped where payloads are complete: `presence.updated` patches only the affected device, lifecycle events remove only affected devices/rooms, and execution status patches only the affected room.
- `execution.updated` events now carry `executionId`, `roomId`, and `status` in the payload itself, so mobile reducers can perform targeted result upserts without depending on envelope-only fields.
- Mobile Room now patches/upserts only the affected execution row for `execution.updated` realtime events and skips the redundant full room reload that follows from the generic revision signal.
- `command.updated` events now also carry `commandId`, `roomId`, and `status`; Mobile Room patches only the affected command row instead of reloading the entire room projection.
- `proposal.created` events now carry `proposalId`, `roomId`, `commandId`, `status`, `summary`, `itemCount`, and the authoritative `pendingProposalCount`; mobile patches the Home badge and Room proposal row without a full summary/room reload.
- `decision.created` events now carry `decisionId`, `proposalId`, `roomId`, `commandId`, final proposal/command statuses, and authoritative `pendingProposalCount`; mobile removes the closed proposal and patches the related command without reloading the room.
- `room.snapshot.updated` events now carry the full cleanliness snapshot projection; mobile patches Home cleanliness fields and Room `CleanlinessCard` only when the event is newer than the cached `calculatedAt`.
- Mobile realtime dispatch now suppresses generic `realtimeRevision` fan-out for complete item-scoped payloads, including presence, lifecycle removals, proposals, decisions, cleanliness snapshots, commands, and executions; incomplete or unknown projections still keep the full reconcile fallback.
- Mobile file downloads now listen for `file.transfer.updated` WebSocket events and wake only the matching active transfer, keeping REST status reads as a 15-second safety fallback instead of polling every 2 seconds.
- Desktop background runtime keeps heartbeat at 5 seconds, splits scheduled REST reconciliation into 15-second fast control-plane passes and 30-second heavy file-transfer/smart-cache passes, while Socket.IO wakeups still trigger an immediate full reconcile.
- Desktop full reconcile now rebuilds the SQLite browse/search index for active watched managed roots and recalculates the same cleanliness snapshot, so missed watcher events are repaired without turning mobile into a full-refresh polling client.
- File-engine SQLite `file_index` now stores nullable OS-backed `file_id` values: Windows uses volume serial + file index via Win32, Unix uses dev + inode, and unsupported platforms leave the field empty instead of inventing an identity.
- Pairing status polling uses the existing isolated 60/min rate-limit bucket and the desktop pairing UI keeps a 2-second polling cadence.
- Rule draft lifecycle now persists only validated `READY` AI rule drafts, keeps unconfigured AI as explicit `UNCONFIGURED` without fake rows, and requires explicit idempotent confirmation before creating a durable rule.
- OpenAI Responses provider is now configurable behind `AI_PROVIDER=openai`, `AI_API_KEY`, and `AI_MODEL`; model output is parsed as structured JSON and revalidated against MouseKeeper command/rule Zod contracts before any draft enters product logic. Missing or rejected credentials still return explicit `AI_PROVIDER_UNCONFIGURED`.
- Desktop command processing now accepts server `RENAME`, `MOVE`, `TRASH`, directory `CREATE`, and empty-file `CREATE` commands as proposal generation only: the agent validates the managed-root binding, source paths, filenames/destinations, parent directory safety, symlink/reparse safety, and destination conflicts, then submits `MOVE`, quarantine, `CREATE_DIR`, or `CREATE_FILE` proposal items without touching files before user approval.

> 구체적인 아키텍처와 개발 순서는 [구현 계획](IMPLEMENTATION_PLAN.md), 현재 완료/누락 판정은 [구현 이력 및 MVP 감사](HISTORY.md)를 기준으로 합니다.
>
> 현재 감사 기준: `B` (`origin/main` 기반, `2026-07-13`), A/B v1.4 통합 코드 포함

## 공통과제 II : 협업형 실전 산출물 제작 (2인 1팀)

**목적:** 실시간 인터랙션, LLM Wrapper, Cross-Platform 중 하나의 옵션을 선택해 구현하며, 선택한 기술을 실제로 동작하는 형태의 산출물로 완성한다.

**선택 옵션:**

| 옵션 | 설명 |
|---|---|
| 실시간 인터랙션 | 사용자 간 상태 변화, 실시간 데이터 흐름, 스트리밍 응답 등 실시간성이 드러나는 기능을 구현 |
| LLM Wrapper | LLM API를 활용하여 AI 기능이 포함된 산출물을 구현 |
| Cross-Platform | 하나의 산출물을 여러 실행 환경에서 사용할 수 있도록 구현* |

> *데스크톱 앱 ↔ 모바일 앱; 혹은 다른 폼팩터에서의 앱; 웹만/웹 기반 프레임워크(Electron, Tauri 등) 대신 다른 프레임워크를 시도해보는 것을 적극 권장

**결과물:** 선택한 옵션이 적용된 작동 가능한 산출물, 실행 가능한 코드, 시연 자료 및 관련 문서

---

## 팀원

| 이름 | 학교 | GitHub | 역할 |
|---|---|---|---|
| 김윤서 | 이화여자대학교 | westyoon | B — Product & Cloud: 모바일, 서버, worker, 인증·실시간 연결 |
| 임성진 | 고려대학교 | nounmoumn | A — Desktop Agent: 로컬 파일 안전, Tauri/Rust, watcher·실행·복구 |

---

## 선택 옵션

- [x] 실시간 인터랙션
- [x] LLM Wrapper
- [x] Cross-Platform

---

## 프로젝트 개요

MOUSEKEEPER는 사용자가 등록한 로컬 폴더를 데스크톱 에이전트가 안전하게 분석하고, 모바일에서 제안을 검토·승인한 뒤 파일 정리와 복구를 수행하는 local-first 크로스플랫폼 서비스다.

- **P0 정리 흐름:** 모바일 command → 서버 영속 저장 → 데스크톱 분석 → proposal → 사용자 승인 → 실행 직전 재검증 → journal → no-overwrite 실행 → 결과 동기화 → undo
- **P0 파일 접근:** 온라인 데스크톱 탐색 → managed root 재검증 → 만료형 전송 → 모바일 checksum 검증 → ACK/TTL 삭제
- **안전 원칙:** 등록 root 밖 접근 차단, symlink/junction/reparse point 우회 차단, 승인 없는 쓰기 금지, journal-before-write, 기존 파일 덮어쓰기 금지
- **실시간 원칙:** Socket.IO는 알림 수단이며 PostgreSQL과 replay cursor가 상태 복구의 기준이다.
- **외부 연동 원칙:** 미설정 provider는 성공으로 가장하지 않고 `UNCONFIGURED`로 처리한다.

## 시스템 구조

```text
Flutter Android
  ↕ REST + Socket.IO
NestJS/Fastify Server ─ PostgreSQL
  ├─ Redis/Valkey: presence TTL, rate limit, 짧은 lock
  ├─ PostgreSQL durable worker: 알림, 재시도, object 만료·삭제
  └─ S3-compatible Storage: P0 만료형 전송 / P1 opt-in cache
  ↕ REST + Socket.IO
Tauri Desktop
  ├─ React/Vite UI
  ├─ Rust file engine: path guard, scan, watcher, rule, journal, undo
  └─ SQLite WAL: managed roots, file index, operation history
```

| 경로 | 역할 |
|---|---|
| `apps/desktop` | A 소유 Tauri/React/Rust 데스크톱 앱 |
| `tools/file-engine-cli` | 데스크톱과 같은 Rust 파일 엔진을 검증하는 CLI |
| `apps/mobile` | B 소유 Flutter Android 앱 |
| `apps/server` | B 소유 REST, Socket.IO, 인증, 상태 머신과 영속 queue |
| `apps/worker` | 전송·캐시 object lifecycle worker |
| `packages/contracts` | OpenAPI, JSON Schema, Zod 공개 계약 |
| `packages/database` | Drizzle schema와 PostgreSQL migration |

## 현재 구현 현황

### A — Desktop Agent

구현된 코드:

- managed root canonicalization, overlap·traversal·symlink/junction/reparse point 방어
- SQLite WAL 기반 managed root, file index, operation journal과 auto-approval 설정 저장
- Tauri 비동기 background loop에서 SQLite 동기 경계를 안전하게 넘는 runtime bridge
- scan, 검색, paginated browse, watcher debounce·reconcile·startup 복원
- Rule DSL, proposal, 사용자 decision, 실행 직전 precheck
- journal-before-write, no-overwrite move, 복구형 trash, create/rename, history, undo, journal recovery
- React 파일 관리 UI와 Tauri invoke bridge
- 실제 REST agent transport: pairing 코드 생성·2초 status polling, 5초 heartbeat, 15초 fast REST reconcile, 30초 heavy transfer/smart-cache reconcile, pending command 조회·상태 전이
- device token의 OS keychain 저장과 UI·로그 비노출, 잘못된 서버 응답 schema 검증
- device별 SQLite sync cursor와 `/v1/sync/events` REST replay
- Desktop Agent 연결·pairing·replay 상태 UI
- managed root 등록 시 서버 room 자동 생성·중복 조회와 수동 `Sync to mobile` 재시도
- system tray, 사용자가 직접 설정하는 autostart, MSI/NSIS Windows bundle 구성
- Desktop·Flutter가 함께 사용하는 8종 픽셀풍 MouseKeeper PNG 상태 모션
- Windows에서 Cargo object 파일 잠금으로 Vite가 종료되지 않도록 `src-tauri/target` 감시 제외
- overlay window/event bridge skeleton
- 서버 `RENAME`·`MOVE`·`TRASH`·directory `CREATE`·empty-file `CREATE` command를 직접 파일 변경이 아닌 승인 대기 `MOVE`/격리/`CREATE_DIR`/`CREATE_FILE` proposal로 변환하는 Desktop processor 경로
- 파일 엔진용 OpenAPI 외부 schema 6개와 fixture

아직 완료되지 않은 범위:

- 새 Android identifier용 Firebase debug build 완료, Google login·FCM 실기기 재검증
- Android release signing, updater signing과 rename 이후 Windows installer 재검증
- 실제 Rive animation/interaction과 schema-validated 자연어 command provider
- Wiring indexed file ID into proposal/precondition/transfer source identity, plus release-grade three-party E2E automation for delegated create operations
- Desktop smart-cache client-side encryption과 key lifecycle
- Android ↔ server ↔ Desktop 전체 P0 release E2E 자동화

서버 URL 또는 pairing이 없으면 agent transport는 fake online을 만들지 않고 명시적으로 `UNCONFIGURED`를 반환한다. pairing 뒤에도 heartbeat나 인증 요청이 성공하기 전에는 `offline`이며, device token은 React 계층으로 전달되지 않는다.

### B — Product & Cloud

- Firebase Android 및 Google 로그인, Firebase Admin token 검증 경로
- PostgreSQL/Drizzle 17개 migration과 Redis/Valkey local compose
- pairing, device, room, heartbeat/presence, command/proposal/decision/execution API
- Socket.IO `/realtime`, idempotency, audit, cursor replay
- Flutter home/room/rule partial-update·expanded DSL form/proposal/result/chat session/files/smart-cache UI와 Drift cache/outbox
- P0 browse/transfer control plane과 P1 smart-cache quota/reservation/lifecycle control plane
- FCM token API·outbox·worker 전송과 모바일 foreground/background 수신 경로
- worker, AWS EC2 systemd/Nginx 구성, Render blueprint, 실제 backup/restore drill 및 안전 문서
- server·worker·mobile Sentry SDK와 request/path/token redaction 경계

AWS EC2 API는 `https://mousekeeper.madcamp-kaist.org`에서 `/health`, `/ready`까지 운영 검증됐다. Private S3 bucket과 최소 권한 EC2 IAM role의 LIST/PUT/HEAD/GET/DELETE, FileTransfer와 암호화 smart-cache object lifecycle, worker 삭제를 실제 검증했다. FCM worker는 기존 Firebase service account로 운영 중이고 PostgreSQL backup/격리 restore drill도 통과했다. Rive asset, Android release keystore, Sentry DSN이 필요한 최종 검증은 계속 `UNCONFIGURED` 또는 검증 대기 상태다.

### Phase별 판정

| Phase | 현재 판정 | 남은 완료 조건 |
|---|---|---|
| 0 계약·파일 안전 POC | 완료 | Rust 안전 회귀와 fixture E2E 유지 |
| 1 로그인·페어링·Presence | 코드 경로와 새 Firebase debug build | Google login과 background/terminated FCM 수신 확인 |
| 2 관리 폴더·스캔·청결도 | room/snapshot/watcher 연결, watcher 누락 보정용 30초 full-pass index reconcile, SQLite OS file ID 저장 | file ID를 source precondition/transfer 검증에 연결 |
| 3 규칙·명령·제안 | Desktop/server/mobile processor 연결, 확장 Rule DSL 서버 계약 | 확장 DSL Desktop evaluator 통합과 실제 3자 release E2E |
| 4 실행·Undo·README·파일 전달 | MOVE/격리/README/transfer/CREATE_DIR/CREATE_FILE 구현 | 실제 3자 transfer E2E와 create release E2E |
| 5 캐릭터·채팅 | PNG 상태/metadata/overlay shell, 모바일 chat session UI·cursor pagination, AI provider 경계, AI command draft schema 재검증과 `UNCONFIGURED` 응답 계약 | Rive asset과 실제 자연어 명령 provider |
| 6 오프라인·재접속 | 양쪽 outbox·cursor replay, pairing gate pixel-fill loading 구현 | 강제 종료·서버 재시작 E2E |
| 7 하드닝·배포 | EC2·private S3·FCM worker·DB restore drill | rename migration, signed release, Sentry DSN |
| 8 P1 스마트 캐시 | quota/reservation/usage score/lifecycle | Desktop client encryption과 key lifecycle |

### v1.4 — 청결도·연결 해제·모바일 파일 접근

- 청결도는 Rust의 `mousekeeper-cleanliness-v1` 공식이 만든 하나의 snapshot만 사용한다. Desktop은 그 객체를 표시·queue하고 서버는 재계산 없이 저장하며 모바일은 score, 감점 code/count/points, 계산 시각, 공식 버전을 그대로 표시한다.
- device와 room 연결 해제는 모바일·Desktop 양쪽에서 시작할 수 있다. 서버는 멱등 transaction으로 ACTIVE 상태와 진행 작업을 종료하고 durable event를 기록한 뒤 즉시 publish한다. device 해제는 socket과 연결 room을 함께 정리한다.
- 모바일은 로그인 직후 서버의 ACTIVE device/room을 확인하는 Pairing Gate를 통과해야 main navigation을 만든다. stale Drift cache는 gate를 열 수 없고 revoke/remove event와 replay가 관련 cache·outbox를 계단식으로 제거한다. event 유실 시에도 5초 간격의 직렬 authoritative safety reconcile로 device/room 연결 상태만 보정하며, 홈 전체 데이터는 `/v1/home/summary`를 재호출하지 않고 완전한 WebSocket payload 단위로 갱신한다.
- `CharacterState` 9종과 lifecycle 최종 상태는 TypeScript·JSON Schema·OpenAPI·Rust·Flutter에서 같은 대문자 wire value를 사용한다. 모바일은 잘못된 상태·ID·sequence를 fail-closed로 폐기하고 상대경로는 모든 계약에서 같은 1,024자·경계 규칙을 적용한다.
- 모바일 realtime replay와 mutation queue는 시작 시점 UID·generation에 고정된다. 로그아웃 뒤 다른 계정으로 전환되면 이전 계정의 늦은 socket 응답·cursor·cache write·ACK·outbox 갱신을 모두 폐기하며, 계정별 flush는 서로 막지 않는다.
- room 연결 해제 시 Desktop watcher와 disposable index만 정리한다. managed root, 실제 파일, `.mousekeeper_trash`, operation journal과 undo 가능 기록은 삭제하지 않는다.
- 모바일 Files 화면은 연결된 folder 선택, breadcrumb 탐색, metadata, pagination, filename 검색과 검증 다운로드를 제공한다. 검색은 300ms debounce·2자 제한·generation cursor를 사용하며, 다운로드는 `.part`에 받은 뒤 SHA-256이 일치해야 최종 저장 및 ACK한다.
- 호감도 숫자와 ledger는 유지하지만 appearance/accessory/room theme 해금 UI와 mutation 호출은 제거했다. MVP 화면은 affinity와 무관한 고정 외형·테마를 사용한다.

연결 해제와 file access의 DB 통합 suite는 PostgreSQL·Redis가 실행되는 환경에서 opt-in으로 동작한다. 로컬 Docker daemon이 꺼진 검증에서는 이를 성공으로 가장하지 않고 skip으로 기록하며, 배포 전 migration 적용과 실제 revoke/search/download 경합 E2E가 필요하다.

## 검증 기록 (`2026-07-13`)

| 검사 | 결과 |
|---|---|
| `pnpm check:contracts` | 서버 controller 62개와 OpenAPI 일치 |
| `pnpm typecheck` | Node/React workspace 전체 통과 |
| Desktop `tsc + vite build` | production web bundle 성공 |
| contracts test | 25개 통과 |
| server test | 일반 모드 40개 통과·외부 DB/인프라 통합 7개 skip, 실제 S3 lifecycle E2E는 이전 운영 검증 유지 |
| worker test | IAM role·FCM config·영구 무효 token·Sentry redaction 포함 8개 통과 |
| desktop-agent-simulator test | 1개 통과 |
| `flutter analyze` | 오류 0개 |
| Flutter test | Pairing Gate·계정 전환 race·typed event·검색·검증 다운로드 포함 78개 통과 |
| Android debug APK | Firebase Messaging·Sentry SDK 포함 빌드 성공 |
| Rust file-engine CLI | unit 106개 + integration 3개, 총 109개 통과 |
| Rust Desktop/Tauri core | 기본 feature 142개 통과 |
| Rust fixture E2E | proposal → precheck → execute → undo 통과 |
| Tauri feature test | `cargo test --features tauri-commands` 114개 및 Clippy `-D warnings` 통과 |
| Windows release bundle | 실제 앱 feature를 포함한 MSI 6.82MB, NSIS 4.81MB 생성 성공 |
| Desktop ↔ server presence | `ONLINE_IDLE` heartbeat `201`, REST replay `200` 확인 |
| Android ↔ server | SM S901N `adb reverse` 연결, devices/rooms/presence API `200` 확인 |
| Managed root ↔ mobile room | Desktop `POST /v1/rooms` `201`, 자동 동기화 계약 test 통과 |
| AWS production | DNS·TLS·HTTP→HTTPS·`/health=ok`·`/ready=ready` 통과, 3000·5432·6379 외부 차단 |
| AWS S3 production | EC2 IAM role로 LIST·PUT·HEAD·GET·DELETE 및 삭제 후 404 통과, worker 30초 주기 무오류 실행 |
| FileTransfer production E2E | signed PUT → HEAD/complete → GET/SHA-256 → ACK → worker delete 통과 |
| Smart cache production E2E | client-side AES-256-GCM ciphertext → HEAD/SHA → download/decrypt → disable/delete 통과; key/plaintext 서버 미저장 확인 |
| PostgreSQL recovery | root-only custom dump 생성·검증, 임시 DB restore와 필수 schema 검증, 임시 DB 삭제 통과 |
| FCM/Sentry runtime | Firebase Admin 기반 FCM worker active; Sentry SDK는 DSN 미설정으로 전송 검증 대기 |

CI는 `dev` push를 검사하며 Windows에서 두 Rust crate의 format/test와 Tauri feature check를 실행한다.

## 다음 구현 순서

1. Android USB를 다시 연결해 새 package 빌드로 Google 로그인, FCM token 등록, background/terminated 알림 수신을 확인한다.
2. Desktop smart-cache 업로더에 authenticated encryption과 OS 보안 저장소 key lifecycle을 구현한다.
3. Indexed file ID를 proposal/precondition/transfer source identity에 연결하고 CREATE_DIR/CREATE_FILE journal/undo 경로를 release E2E에서 검증한다.
4. command/proposal/execute/undo와 browse/transfer를 한 managed root·실기기에서 왕복 검증한다.
5. 새 EC2 unit/path와 Firebase package로 rename migration을 실행하고 rollback을 확인한다.
6. 실제 `.riv`와 overlay shell을 연결하고 artboard/state machine/input을 실기기·Desktop에서 확인한다.
7. Android release keystore와 Sentry DSN을 secret으로 주입해 signed AAB와 redacted event 수신을 검증한다.

## 실행 방법

필수 환경 변수는 [.env.example](.env.example)을 기준으로 현재 shell 또는 secret manager에 주입한다. 실제 secret과 서비스 계정 원문은 Git에 넣지 않는다.

```powershell
# Node workspace
pnpm install --frozen-lockfile

# PostgreSQL + Redis/Valkey
docker compose up -d
pnpm --filter @mousekeeper/database db:migrate

# Server
pnpm --filter @mousekeeper/server start:dev

# Desktop: Rust stable/MSVC toolchain 필요
$env:MOUSEKEEPER_SERVER_BASE_URL = "http://127.0.0.1:3000"
pnpm --filter @mousekeeper/desktop tauri:dev

# Windows MSI/NSIS 생성
pnpm --filter @mousekeeper/desktop tauri:build
```

Android 실기기에서 USB로 로컬 서버를 사용할 때:

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe" reverse tcp:3000 tcp:3000

Set-Location apps/mobile
flutter run `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=MOUSEKEEPER_API_URL=http://127.0.0.1:3000 `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<Google-Web-OAuth-Client-ID>
```

### AWS EC2 운영 주소

운영 주소는 `https://mousekeeper.madcamp-kaist.org`다. 2026-07-13 기준 공용 DNS, 80 → 443 redirect, TLS, `/health=ok`, PostgreSQL·Valkey를 포함한 `/ready=ready`를 확인했다. 3000·5432·6379는 외부에서 닫혀 있다.

EC2의 Nginx·systemd·IAM role·TLS 설정은 [AWS EC2 배포 절차](docs/AWS_EC2_DEPLOYMENT.md)를 따른다. 다음 명령은 `/health=ok`, `/ready=ready`를 모두 확인한다.

```powershell
.\scripts\check-production-endpoint.ps1 `
  -BaseUrl https://mousekeeper.madcamp-kaist.org
```

Desktop과 모바일에는 HTTPS URL을 환경으로 주입한다. 운영 주소에서는 `adb reverse`를 사용하지 않는다.

핵심 검증 명령:

```powershell
pnpm check:contracts
pnpm typecheck
pnpm test

Set-Location apps/mobile
flutter analyze
flutter test
```

---

## 회고 문서

> [KPT 방법론 참고](https://velog.io/@habwa/%EB%8B%A8%EA%B8%B0-%ED%94%84%EB%A1%9C%EC%A0%9D%ED%8A%B8-%ED%9A%8C%EA%B3%A0-KPT-%EB%B0%A9%EB%B2%95%EB%A1%A0)

### Keep — 잘 된 점, 다음에도 유지할 것

-
-
-

### Problem — 아쉬웠던 점, 개선이 필요한 것

-
-
-

### Try — 다음번에 시도해볼 것

-
-
-

### 팀원별 소감

**김윤서:**

> 

**임성진:**

> 

---

## 참고 자료

### 실시간 인터랙션

**WebSocket**
- https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API
- https://techblog.woowahan.com/5268/
- https://tech.kakao.com/posts/391
- https://daleseo.com/websocket/
- https://kakaoentertainment-tech.tistory.com/110

**Socket.IO**
- https://socket.io/docs/v4/
- https://inpa.tistory.com/entry/SOCKET-%F0%9F%93%9A-Namespace-Room-%EA%B8%B0%EB%8A%A5
- https://adjh54.tistory.com/549
- https://fred16157.github.io/node.js/nodejs-socketio-communication-room-and-namespace/

**SSE (Server-Sent Events)**
- https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events
- https://developer.mozilla.org/ko/docs/Web/API/Server-sent_events/Using_server-sent_events
- https://api7.ai/ko/blog/what-is-sse

**TCP / UDP Socket**
- https://docs.python.org/3/library/socket.html
- https://inpa.tistory.com/entry/NW-%F0%9F%8C%90-%EC%95%84%EC%A7%81%EB%8F%84-%EB%AA%A8%ED%98%B8%ED%95%9C-TCP-UDP-%EA%B0%9C%EB%85%90-%E2%9D%93-%EC%89%BD%EA%B2%8C-%EC%9D%B4%ED%95%B4%ED%95%98%EC%9E%90

**gRPC Streaming**
- https://grpc.io/docs/what-is-grpc/core-concepts/
- https://tech.ktcloud.com/entry/gRPC%EC%9D%98-%EB%82%B4%EB%B6%80-%EA%B5%AC%EC%A1%B0-%ED%8C%8C%ED%97%A4%EC%B9%98%EA%B8%B0-HTTP2-Protobuf-%EA%B7%B8%EB%A6%AC%EA%B3%A0-%EC%8A%A4%ED%8A%B8%EB%A6%AC%EB%B0%8D
- https://tech.ktcloud.com/entry/gRPC%EC%9D%98-%EB%82%B4%EB%B6%80-%EA%B5%AC%EC%A1%B0-%ED%8C%8C%ED%97%A4%EC%B9%98%EA%B8%B02-Channel-Stub
- https://inspirit941.tistory.com/371
- https://devocean.sk.com/blog/techBoardDetail.do?ID=167433

**WebRTC**
- https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API
- https://webrtc.org/getting-started/overview
- https://web.dev/articles/webrtc-basics?hl=ko
- https://devocean.sk.com/blog/techBoardDetail.do?ID=164885
- https://beomkey-nkb.github.io/%EA%B0%9C%EB%85%90%EC%A0%95%EB%A6%AC/webRTC%EC%A0%95%EB%A6%AC/
- https://gh402.tistory.com/45
- https://on.com2us.com/tech/webrtc-coturn-turn-stun-server-setup-guide/

**QUIC / WebTransport**
- https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API
- https://datatracker.ietf.org/doc/html/rfc9000
- https://news.hada.io/topic?id=13888

#### KCLOUD VM / Cloudflare Tunnel 환경별 주의사항

| 환경 | 사용 가능(권장) 기술 | 포트/조건 | 주의할 기술 |
|---|---|---|---|
| **로컬 / 일반 VM** | HTTP/REST, WebSocket, Socket.IO, SSE, TCP Socket, gRPC Streaming, WebRTC, QUIC/WebTransport 등 대부분 가능 | 직접 포트 개방 가능. 예: 3000, 5000, 8000, 8080, 9000 등. 외부 공개 시 방화벽/보안그룹/공인 IP 설정 필요 | WebRTC는 STUN/TURN 필요 가능. QUIC/WebTransport는 HTTP/3 · UDP 지원 필요 |
| **KCLOUD VM (VPN 내부)** | HTTP/REST, WebSocket, Socket.IO, SSE, WebRTC 시그널링 | 접속 기기 VPN 필요. 기본 허용 포트: **22, 80, 443**. 개발 포트(3000, 8000, 8080 등)는 직접 접근 제한 가능 | TCP Socket은 포트 제한 있음. gRPC는 HTTP/2 설정 필요. WebRTC 미디어·UDP·QUIC/WebTransport 비권장 |
| **KCLOUD VM + Tunnel** | HTTP/REST, WebSocket, Socket.IO, SSE, WebRTC 시그널링 | VM의 `localhost:<port>`를 도메인에 연결. `localPort`는 **1024~65535**. 예: 3000, 8000, 8080 가능 | 순수 TCP Socket, UDP, WebRTC 미디어/DataChannel, QUIC/WebTransport 불가. gRPC 보장 어려움 |
| **외부 서비스 + 우리 도메인** | HTTP/REST, WebSocket, Socket.IO, SSE, WebRTC 시그널링 | Vercel/Netlify/Railway/Render/AWS/GCP 등에 배포 후 CNAME/A 레코드 연결. 보통 외부는 **443** 사용 | WebSocket/gRPC/TCP/UDP는 플랫폼 지원 여부 확인 필요. 서버리스 플랫폼은 장시간 연결 제한 가능 |
| **서버 없이 외부 SaaS 사용** | Supabase Realtime, Firebase, Pusher/Ably, LLM API Streaming | 직접 포트 관리 불필요. 각 서비스 SDK/API 사용 | 커스텀 TCP/UDP 서버 구현 불가. WebRTC는 STUN/TURN 필요할 수 있음 |

### LLM Wrapper

- https://github.com/teddylee777/openai-api-kr
- https://github.com/teddylee777/langchain-kr
- https://devocean.sk.com/blog/techBoardDetail.do?ID=167407
- https://mastra.ai/docs

### Cross-Platform

- https://flutter.dev/
- https://reactnative.dev/
- https://docs.expo.dev/
- https://kotlinlang.org/multiplatform/

---

## A File Engine Notes

A-side local file engine documentation:

- CLI usage and JSON contracts: [`docs/FILE_ENGINE_CLI.md`](docs/FILE_ENGINE_CLI.md)
- Shared server/Desktop contracts: [`docs/CONTRACTS.md`](docs/CONTRACTS.md)
- Desktop/Tauri integration plan: [`apps/desktop/src-tauri/INTEGRATION.md`](apps/desktop/src-tauri/INTEGRATION.md)

Current safe flow:

```text
propose -> decision.jsonl -> precheck -> execute -> undo
```
