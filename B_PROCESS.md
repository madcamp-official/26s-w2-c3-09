# B 개발 진행 기록

이 문서는 B(Product & Cloud) 영역의 결정, 구현 변경, 검증 결과와 외부 설정 의존성을 기록한다. 기준 문서는 `HOUSEMOUSE_PLAN.md`, `IMPLEMENTATION_PLAN.md`, `AI_implement_rule.txt`이다.

## 적용 원칙

- `HOUSEMOUSE_PLAN.md` v1.3의 기술 스택과 상태 머신을 제품 결정의 최우선 기준으로 사용한다.
- 가짜 API, dummy/seed 데이터, 성공을 가장하는 fallback을 만들지 않는다.
- Firebase, object storage 등 외부 provider가 없으면 `UNCONFIGURED` 또는 시작 단계 fail-fast로 처리한다.
- 중요한 명령과 결과는 PostgreSQL commit 후에만 알림을 발행한다.
- 공개 계약은 `packages/contracts`, DB 물리 모델은 `packages/database`에 분리한다.
- 파일 원문과 절대 경로를 서버에 저장하거나 로그로 남기지 않는다.
- A 소유 영역인 `apps/desktop/src-tauri`, files/admin, `tools/file-engine-cli`는 수정하지 않는다.

## 2026-07-11 — 초기 상태 감사

### 확인된 환경

- 브랜치: `B`
- Node.js: 설치 완료
- pnpm: 설치 완료
- Flutter/Android toolchain: `flutter doctor` 통과
- Docker: 설치 완료
- Android application ID: `com.housemouse.app`

### 기존 구현

- `apps/mobile`: Flutter 공식 프로젝트, Firebase 미설정 `UNCONFIGURED` 화면과 위젯 테스트
- `apps/server`: NestJS/Fastify 골격, 환경 변수 Zod fail-fast, health endpoint
- 초기 Drizzle 테이블: users, devices, rooms, commands
- `compose.yaml`: PostgreSQL 17, Valkey 8

### 감사에서 확인한 미구현 범위

- 공개 TypeScript/Zod 계약 패키지
- 계획서 전체 P0 PostgreSQL schema와 migration
- Firebase Admin 인증 Guard와 내부 `AuthPrincipal`
- pairing/device/room/command/proposal/decision/execution API
- DB 기반 idempotency, sync event replay, audit
- Redis presence TTL
- desktop-agent-simulator
- 모바일 Google 로그인, API 계층, Drift outbox, 핵심 화면
- P0 파일 browse/transfer와 object lifecycle worker
- 캐릭터/호감도 및 P1 스마트 캐시

### 이번 작업 순서

1. 계약과 DB 경계 정리
2. P0 control plane vertical slice
3. replay/presence/simulator
4. 모바일 실제 API 흐름
5. 파일 전달/worker
6. 제품 경험과 후속 P1

### 보존한 사용자 변경

- `apps/mobile/android/gradle/wrapper/gradle-wrapper.properties`의 기존 수정은 B 구현과 무관하므로 유지한다.

## 2026-07-11 — Control Plane 1차 구현

### 공개 계약

- `packages/contracts` TypeScript/Zod 패키지를 추가했다.
- command, proposal item, decision, execution, pairing, room, heartbeat 계약을 strict schema로 정의했다.
- 상대 경로 계약이 drive 절대 경로, `/`, `\\`, `..` segment를 거절하도록 테스트했다.
- mutation idempotency key는 8~128자로 제한했다.

### PostgreSQL

- `packages/database`로 DB 물리 모델을 분리했다.
- users, devices, pairing_sessions, rooms, rules, commands, proposals, proposal_items, decisions, executions, room_snapshots, sync_events, audit_events를 구현했다.
- 사용자별 idempotency, proposal 단일 decision, command 단일 proposal, device별 pending 조회용 index를 추가했다.
- 최초 Drizzle migration `0000_blue_mesmero.sql`을 생성했다.
- Docker Desktop이 처음 꺼져 있어 1회 실패했고 백그라운드 시작 후 PostgreSQL 17 임시 volume에 migration을 실제 적용했다. 검증 뒤 container와 volume은 삭제했다.

### 인증과 API

- Firebase Admin은 실제 ID token을 verify하고 revoke 여부까지 확인한다.
- 검증 성공 시 Firebase UID 기준 내부 user를 생성 또는 조회한다. 우회 로그인은 없다.
- pairing session은 암호학적 nonce, 6자리 code의 SHA-256 hash, 10분 expiry를 사용한다.
- device 목록/revoke와 room 생성/목록을 구현했다.
- Command → Proposal → 전체 승인/거절 → Execution 결과 흐름을 transaction으로 구현했다.
- 잘못된 command 상태 건너뛰기와 terminal replay를 거절한다.
- 중요 mutation은 같은 transaction에서 sync event와 최소 audit summary를 기록한다.
- sync replay는 사용자별 증가 sequence와 cursor를 사용한다.

### Presence

- heartbeat 간격 정책에 맞춰 Redis/Valkey key TTL을 45초로 저장한다.
- TTL key가 없으면 `OFFLINE`을 반환하고 PostgreSQL에는 `last_seen_at`을 보조 기록한다.

### 검증

- contracts: 5 tests 통과
- server: 3 tests 통과
- server TypeScript typecheck 통과
- NestJS production build 통과
- PostgreSQL migration Docker E2E 통과

## 2026-07-11 — Simulator, Mobile, File Access

### Desktop agent simulator

- 실제 API URL, Firebase ID token, device UUID가 없으면 `UNCONFIGURED`로 종료한다.
- pending command를 REST로 조회하고 `QUEUED → DELIVERED → ANALYZING`까지만 수행한다.
- 파일 엔진 결과가 없으면 proposal이나 성공 결과를 만들지 않고 ANALYZING 상태를 유지한다.
- config 단위 테스트와 TypeScript typecheck를 통과했다.

### Flutter 모바일

- `--dart-define`의 `FIREBASE_ENABLED`, `HOUSEMOUSE_API_URL`, 선택적 `GOOGLE_SERVER_CLIENT_ID`를 설정 경계로 사용한다.
- Firebase native 설정이 없거나 초기화에 실패하면 Google 로그인 대신 `UNCONFIGURED`를 표시한다.
- Google Sign-In → Firebase credential 로그인과 Firebase ID token API header 주입을 구현했다.
- 홈 화면에 장치·방의 loading, empty, network error 상태와 새로고침을 구현했다.
- 방 화면에 ANALYZE command 생성, 최근 command 상태, open proposal 목록을 구현했다.
- proposal 상세는 상대 경로·작업을 보여주고 MVP 범위의 전체 승인 또는 전체 거절만 전송한다.
- Drift에 devices, rooms, commands, proposals cache와 mutation outbox, sync cursor를 생성했다.
- Flutter analyze 0건, widget test 통과 상태다.

### P0 파일 탐색·전송

- 상대 경로만 허용하는 browse/transfer Zod 계약을 추가했다.
- file_browse_requests, file_transfers, object_deletion_jobs를 DB에 추가하고 migration `0001`, cleanup unique index migration `0002`를 생성했다.
- browse request/pending/result/failure API와 최대 200개 page 계약을 구현했다.
- transfer session은 DB에 먼저 저장하고 사용자·room·device ownership과 크기·TTL을 검증한다.
- S3 호환 storage가 없으면 signed URL endpoint는 `UNCONFIGURED`를 반환하며 로컬 디스크 fallback은 없다.
- upload object의 실제 크기를 HEAD로 재검증한 뒤에만 READY로 전환한다.
- 모바일 ACK 뒤 object deletion job을 DB에 영속 저장한다.
- worker는 만료 transfer를 EXPIRED로 전환하고 `FOR UPDATE SKIP LOCKED`로 삭제 job을 처리하며 실패 시 재시도한다.
- server/worker typecheck와 server production build를 통과했다.

## 2026-07-11 — Device 인증 하드닝

- 감사에서 agent endpoint가 같은 사용자의 모바일 Firebase token으로도 호출될 수 있는 문제를 발견했다.
- pairing session 응답에 Desktop만 아는 256-bit nonce를 추가했다.
- 모바일 claim이 완료되면 pairing session에 claimed device ID를 원자적으로 연결한다.
- Desktop은 session ID + nonce로 claim 상태를 조회하고 90일 만료 device JWT를 받는다.
- device token은 `hm_device_` prefix, server issuer, desktop audience를 검증하고 DB의 ACTIVE device 여부를 매 요청 재확인한다.
- `@AgentOnly` metadata와 guard를 추가해 command 수신/상태, proposal 제출, execution, heartbeat, browse 결과, transfer upload endpoint에서 Firebase 사용자 token을 거절한다.
- device revoke 뒤 기존 JWT도 DB 검사에서 즉시 거절된다.
- simulator도 Firebase ID token 대신 실제 pairing device token만 받도록 변경했다.

## 2026-07-11 — Character, P1 Cache, Deploy/CI

### 캐릭터·호감도·채팅

- character_profiles와 append-only affinity_events를 추가했다.
- proposal 전체 승인 시 +1, execution 성공 시 +2를 source decision/execution unique 제약과 함께 정확히 한 번 반영한다.
- 캐릭터 profile API는 Rive asset이 제공되지 않은 현재 상태를 `UNCONFIGURED`로 명시한다.
- chat message는 사용자 원문만 영속하며 AI adapter 미설정 상태에서 assistant 성공 응답을 만들지 않고 `aiStatus: UNCONFIGURED`를 반환한다.

### P1 스마트 캐시

- global feature flag 기본 false와 room별 opt-in policy를 분리했다.
- AVAILABLE byte와 유효 RESERVED byte를 함께 계산하고 room advisory lock 안에서 quota를 예약한다.
- manual pin과 usage score 우선순위로 승인하며 서버가 만든 object key와 signed UploadTarget만 반환한다.
- upload 완료 시 object HEAD 크기, reservation, device/room ownership을 검증한다.
- availability와 freshness를 별도 필드로 저장하고 이전 버전은 INVALIDATED + STALE로 전환한다.
- 같은 room에 미처리 command가 있으면 cached-files 응답에 목록 변경 warning을 포함한다.
- policy disable, device revoke, reservation expiry, 이전 버전 교체는 각각 durable deletion job을 생성한다.
- worker는 transfer/cache/reservation 삭제 job을 `SKIP LOCKED`로 가져와 재시도하며 로그/DB에 provider 오류 원문을 저장하지 않는다.

### 배포와 CI

- Render 공식 Blueprint 규격을 확인해 `render.yaml`에 web server, worker, PostgreSQL 17, Key Value를 정의했다.
- 실제 Firebase/object storage 값은 `sync: false` secret로 유지하고 device JWT secret만 Render가 생성한다.
- GitHub Actions는 공식 checkout/setup-node 최신 major와 pnpm, Flutter stable을 사용한다.
- Node typecheck/test/build, migration drift, Flutter codegen/analyze/test를 PR gate로 추가했다.

## 2026-07-11 — Realtime, Rule, Cleanliness, Offline Recovery

### 실시간 알림과 replay

- Socket.IO namespace `/realtime`을 추가하고 Firebase 사용자 token 또는 `hm_device_` device token을 handshake에서 실제 검증한다.
- 브라우저 Origin은 `WEB_ORIGIN`과 정확히 일치할 때만 허용하고, native client처럼 Origin header가 없는 연결은 인증 token 검증 뒤 허용한다.
- `sync_events.published_at`과 migration `0007_ambiguous_wong.sql`을 추가했다.
- PostgreSQL에 먼저 저장된 미발행 event를 dispatcher가 읽어 user room에 알린 뒤 published 시각을 기록한다.
- 같은 device socket을 user room과 device room 양쪽으로 emit해 중복 전달하던 가능성을 제거하고 user room에 한 번만 전송한다.
- 실시간 알림은 최적화 계층으로만 사용한다. Flutter는 Drift의 user cursor 이후 `/v1/sync/events`를 200개씩 replay하며, live/replay 경쟁이 있어도 cursor가 뒤로 가지 않도록 transaction에서 최댓값만 저장한다.
- Flutter는 event 수신 또는 reconnect replay 뒤 화면 provider를 갱신한다. Socket event가 유실돼도 PostgreSQL replay로 복구한다.

### 결정론적 규칙

- 확장자 `IN`, 지난 일수 `GTE`, 이름 `CONTAINS/STARTS_WITH/ENDS_WITH` 조건과 `ALL/ANY` 조합을 strict Zod 계약으로 정의했다.
- 동작은 관리 root 상대 경로만 받는 `MOVE` 또는 `QUARANTINE`만 허용한다. 절대 경로와 `..` 이동은 기존 path guard 계약에서 거절한다.
- room 소유권을 검사하는 규칙 목록/생성/수정 API를 구현했다.
- 수정은 client가 보낸 `version`이 DB 현재 version과 일치할 때만 적용하고, 충돌 시 `VERSION_CONFLICT`를 반환한다.
- rule 생성/수정은 같은 transaction에서 desktop 대상 sync event를 기록한다.
- 모바일 규칙 화면에서 MVP 범위인 단일 확장자 또는 기간 조건의 이동 규칙을 만들고 활성화 상태를 변경할 수 있다.
- create/update rule과 snapshot 계약을 OpenAPI에도 반영했다.

### 청결도 snapshot

- Desktop이 계산한 0~100 점수와 total/managed/unorganized 파일 수, 감점 reason 목록만 저장하는 strict 계약을 추가했다.
- 파일 수가 전체 파일 수보다 큰 잘못된 metric은 API 진입 전에 거절한다.
- snapshot 저장은 해당 room을 실제로 소유한 desktop device token만 가능하다.
- 최신 snapshot 조회 API와 `room.snapshot.updated` replay event를 구현했다.
- 모바일 방 화면은 점수 등급, 원형 진행률, 정리되지 않은 파일 수를 표시하고 snapshot 전에는 계산 전 상태를 명확히 표시한다.

### 모바일 mutation outbox

- Drift `mutation_outbox`에 `PENDING/FAILED`, attempt count, next retry, provider 원문을 포함하지 않는 error code를 추가하고 schema version 2 migration을 작성했다.
- command 생성과 proposal 승인/거절은 connection/timeout/5xx일 때 동일 idempotency key와 payload를 outbox에 저장한다.
- reconnect 또는 홈 진입 때 backoff 대상 mutation을 다시 전송하며, 서버 성공 ACK 뒤에만 row를 삭제한다.
- 4xx 같은 terminal 응답은 무한 재시도하지 않고 `FAILED`와 최소 HTTP code만 남긴다.

### 검증 결과

- contracts: 2 suites, 8 tests 통과. unsafe rule destination, version-only patch, 잘못된 snapshot count를 포함한다.
- 전체 Node workspace typecheck 통과.
- server/worker/contracts/database/simulator production build 통과.
- server unit 3 tests 통과, DB integration 1 test는 일반 test에서는 환경 flag가 없어 의도적으로 skip된다.
- simulator unit 1 test 통과.
- Flutter analyze 0건, widget test 1건 통과.
- Docker 임시 PostgreSQL 17과 Valkey 8을 healthy 상태로 띄운 뒤 migration 0000~0007과 실제 command DB integration test를 통과했다. 검증 뒤 container/network/volume을 삭제했다.
- Android 첫 build는 SDK Build-Tools 36, Platform 34/35, CMake 3.22.1 설치 때문에 5분 명령 제한을 넘었지만 Gradle 자식 프로세스가 정상 완료했고 `apps/mobile/build/app/outputs/flutter-apk/app-debug.apk`가 생성됐다.
- Android `namespace`와 `applicationId`는 모두 `com.housemouse.app`이다.
- 저장소 전체에서 README는 root `README.md` 하나만 존재한다.

### 외부 설정이 남은 항목

- Firebase native configuration과 server service account가 없으므로 Google 로그인과 FCM은 계속 `UNCONFIGURED`이다.
- AI provider, object storage, Rive asset도 실제 설정/asset이 없으므로 성공을 가장하지 않고 각각 `UNCONFIGURED` 경계를 유지한다.
- `flutter_application_1`은 내부 파일이 전혀 없는 빈 폴더지만, 현재 해당 폴더를 연 VS Code 프로세스가 Windows directory handle을 잡고 있어 삭제가 거절됐다. 해당 VS Code 창을 닫으면 폴더 자체를 제거할 수 있다. 실제 Flutter 프로젝트와 빌드는 `apps/mobile`만 사용한다.

## 2026-07-11 — B 전체 범위 재감사와 최종 하드닝

이번 단계에서는 `HOUSEMOUSE_PLAN.md`의 B-P0~B-P3, Phase 1~8, B DoD와 `IMPLEMENTATION_PLAN.md`, `AI_implement_rule.txt`를 다시 대조했다. 외부 secret이나 asset 없이 검증 가능한 B 구현은 계속 진행하고, 실제 provider가 필요한 항목은 성공으로 가장하지 않고 아래의 외부 의존성으로 분리했다.

### 계약과 데이터 계층

- 상대 경로 계약은 NUL, 절대/drive 경로, 빈 segment, `.`과 `..`를 거절한다.
- command, proposal, decision, execution, rule, snapshot, browse, transfer, audit, sync, CharacterEvent, smart-cache 계약과 오류 catalog를 `packages/contracts`에 모았다.
- EventEnvelope는 `schemaVersion`과 `correlationId`를 필수로 가지며 PostgreSQL sequence 순서로 replay한다.
- OpenAPI에 smart-cache policy, candidate batch, reservation 완료·취소, cached-file 목록·삭제·download 경로와 request schema를 추가했다.
- OpenAPI는 Prettier 검사와 YAML parse/reference 검사에 통과했다. 현재 51개 path, 58개 controller method와 29개 공개 schema가 일치한다.
- PostgreSQL schema는 25개 table이며 migration은 `0000`부터 `0014_loud_adam_destine.sql`까지 순서대로 생성했다.
- 후속 migration에는 transfer upload 완료 멱등성, sync envelope 필드, audit index, cached SHA-256, proposal/execution 결과 멱등성, cache candidate batch ledger와 reservation batch/completion key가 포함된다.

### 인증·명령·실시간 제어면

- Firebase user token과 90일 `hm_device_` token을 분리하고, device token은 요청마다 DB의 `ACTIVE` 상태와 정확한 device ownership을 다시 검사한다.
- pairing code는 HMAC 기반 6자리 값, 256-bit nonce, 10분 TTL을 사용하며 claim한 device와만 token을 교환한다.
- command는 DB durable queue에 먼저 저장하고, proposal/decision/execution은 각각 멱등 key와 대상 device를 격리한다.
- 승인된 decision 중 아직 execution이 claim하지 않은 항목만 Desktop pending API로 반환한다.
- audit summary는 사용자용 최소 정보만 저장하고 provider 원문, token, 절대 경로를 남기지 않는다.
- Socket.IO는 인증된 user room에 한 번만 emit하며, 유실 복구는 `/v1/sync/events` cursor replay가 담당한다.
- presence는 Redis TTL과 known-device sorted set으로 관리한다. 만료 monitor가 OFFLINE을 한 번만 emit하고 PostgreSQL을 현재 온라인 상태의 진실 원천으로 사용하지 않는다.
- `/health`는 process liveness, `/ready`는 PostgreSQL `select 1`과 Redis `PING`을 직접 확인하는 readiness로 분리했다.
- pairing/IP, transfer, 기본 API rate limit을 Redis에 두고 production에서는 proxy 정보를 신뢰하도록 설정했다.
- request 완료 로그는 JSON으로 기록하며 correlation ID, method, route template, status와 duration만 포함한다. 실제 path parameter, query, request body는 기록하지 않는다.

### P0 browse와 만료형 transfer

- offline desktop의 browse/transfer 생성은 장기 queue에 넣지 않고 `DEVICE_OFFLINE`으로 종료한다.
- browse는 60초가 지나면 `TIMED_OUT`을 영속하고, 모바일은 이미 받은 page를 유지하면서 `DEVICE_OFFLINE`, `TIMED_OUT`, `CURSOR_INVALIDATED`를 구분한다.
- transfer는 source metadata, room/device ownership, 크기, TTL과 상태 전이를 검증한다. signed URL 수명은 session의 남은 수명을 넘지 않는다.
- upload 완료는 object HEAD 크기와 idempotency key를 확인하고, 모바일은 `.part`로 받은 뒤 SHA-256 확인 후 충돌 없는 이름으로 전환하며 그 뒤에만 ACK한다.
- ACK, 취소, 실패, 만료는 durable deletion job을 남긴다. worker는 `SKIP LOCKED` 재시도와 2배 TTL grace 뒤 orphan sweep을 수행한다.
- object storage가 미설정된 상태에서 transfer/cache row를 먼저 만들고 signed URL 단계에서 실패할 수 있던 오염 경로를 발견했다. 이제 DB mutation 전에 `assertConfigured()`가 `UNCONFIGURED`를 발생시킨다.

### P1 스마트 캐시

- global feature flag 기본값은 false이고 room policy는 명시적 opt-in이다.
- 후보 batch는 canonical request hash와 user별 idempotency key로 정확히 한 번만 처리한다.
- PostgreSQL advisory transaction lock 안에서 `AVAILABLE + 만료 전 RESERVED`를 합산해 quota를 예약한다.
- manual pin과 높은 usage score를 우선하며, 낮은 score·오래 미사용한 non-pinned cache를 먼저 INVALIDATED하고 deletion tombstone을 만든다.
- reservation 완료는 object 크기, SHA-256, 버전, completion idempotency key를 검증한 뒤에만 AVAILABLE로 전환한다.
- reservation 취소, cache disable, cache delete와 device revoke는 durable deletion job을 생성한다.
- Desktop room 해제는 room을 `REMOVED`로 전환하고 활성 transfer, cache reservation, AVAILABLE cache를 한 transaction에서 취소·무효화한 뒤 각각의 deletion tombstone과 audit/sync event를 남긴다. device revoke도 연결된 ACTIVE room을 함께 `REMOVED`로 전환한다.
- 모바일은 availability, freshness, cached-at, last-verified-at을 분리하고 desktop offline의 `VERIFIED_CURRENT`를 응답 시 `UNVERIFIED_OFFLINE`으로 방어 표시한다.
- 같은 room에 처리 중 command가 있으면 cached-file 목록 변경 가능 경고를 노출한다.
- 실제 PostgreSQL 통합 테스트에서 동시 60-byte 후보 2개가 100-byte quota를 초과하지 않음, batch replay와 payload 충돌, 만료 reservation quota 해제, 취소 tombstone, 낮은 점수 LRU 퇴출, policy disable tombstone을 검증했다.

### 모바일 오프라인·표시 계층

- Drift schema version 3에서 devices, rooms, commands, proposals, executions, snapshots, outbox와 sync cursor를 Firebase owner UID별로 격리했다.
- 이전 사용자 범위가 없던 표시 cache는 migration에서 제거·재생성하여 계정 전환 시 데이터가 섞이지 않게 했다.
- home과 room은 성공 응답을 실제로 Drift에 저장하고 네트워크 실패 시 cache를 표시한다.
- command와 승인/거절 mutation은 connection/timeout/5xx에서 같은 idempotency key로 outbox에 보존한다. 서버 ACK 뒤에만 삭제하며 terminal 4xx는 `FAILED`로 남겨 무한 재시도를 막는다.
- home은 pending/failed outbox 수와 실패 항목 폐기 기능, 여러 room의 pending badge, presence 불빛, 청결도, 최근 execution과 캐릭터 상태를 표시한다.
- loading, empty, offline, error 상태와 다시 시도를 home/room/files/proposal/smart-cache 화면에서 명시한다.
- 실시간 live/replay sequence는 owner별 cursor transaction에서 단조 증가시키고 이미 처리한 sequence의 UI invalidation을 억제한다.
- 규칙 생성·수정·활성화와 optimistic version conflict 문구, 제안 파일별 상대 경로·이유·목적지·충돌, 실행의 STALE/ROLLED_BACK/부분 성공 표현을 연결했다.
- chat은 사용자 메시지만 영속하고 AI provider가 없는 현재 assistant 성공을 만들지 않으며 `UNCONFIGURED`를 표시한다.
- 캐릭터 설정 화면에서 털 색상, 액세서리와 방 테마를 저장한다. Rive asset이 없는 현재도 설정 누락을 숨기지 않고 `UNCONFIGURED` 안내와 선택 metadata만 안전하게 저장한다.

### 운영·문서·빌드

- Render Blueprint에 server, worker, PostgreSQL 17과 Valkey를 정의하고 secret은 `sync: false`로 외부 주입한다.
- DB backup/restore PowerShell script는 `DATABASE_URL`만 받고 backup SHA-256과 명시적 restore `-Apply`를 요구한다.
- `docs/RECOVERY_RUNBOOK.md`에 장애별 replay·worker·restore·rollback 절차를 기록했다.
- `docs/MVP_SCOPE.md`와 `docs/FILE_SAFETY_INVARIANTS.md`를 추가해 B scope, zero-fake 경계, 경로·승인·transfer/cache 불변식을 고정했다.
- Android release signing은 debug key fallback을 제거했다. release task는 실제 keystore path, alias, store password, key password가 없으면 `UNCONFIGURED`로 즉시 중단한다.
- README는 저장소 루트 `README.md` 하나만 존재한다.

### 최신 검증 결과

- contracts: 2 suites, 17 tests 통과.
- server 일반 모드: 10 suites, 19 tests 통과. 외부 DB가 필요한 4 suites는 의도적으로 skip된다.
- 임시 PostgreSQL 17·Valkey 8 환경: migration `0000~0014`, server 14 suites, 23 tests 전부 통과. room removal과 transfer failure lifecycle도 이 환경에서 검증했다.
- 실제 server runtime: `/ready=200`, `/health=200`, pairing rate limit 11번째 요청 `429`, redacted JSON request log 생성을 확인했다.
- worker typecheck 통과.
- desktop simulator: 1 test와 typecheck 통과.
- Flutter: `flutter analyze` 0건, cache/outbox/browse fallback/실시간 알림/스마트 캐시/README/캐릭터 widget·unit tests 19개 통과.
- 최신 Android debug APK 생성 성공: `apps/mobile/build/app/outputs/flutter-apk/app-debug.apk`, 183,707,412 bytes, SHA-256 `C96A1514AB4915D4D400CF3C86D681E9F40BC3030DAB04DD40AA583A49B6BAA3`.
- Android release dry-run은 실제 signing 환경 변수 4개가 없을 때 `UNCONFIGURED: HOUSEMOUSE_ANDROID_KEYSTORE_PATH...`로 중단되는 것을 확인했다.
- 검증에 사용한 임시 server process, PostgreSQL·Valkey container와 일회성 key 파일은 모두 종료·삭제했다.

### 실제 외부 입력이 필요한 잔여 항목

- Firebase Android native 설정과 server service account가 들어와야 Google 로그인과 FCM 실기기 테스트를 완료할 수 있다.
- Rive asset과 state machine 이름이 들어와야 저장된 외형·테마 선택을 실제 animation에 반영할 수 있다. 현재 CharacterEvent, affinity, 설정 API와 모바일 선택 UI까지 동작한다.
- 실제 private S3-compatible bucket과 lifecycle policy가 있어야 upload/download/HEAD/delete 및 provider-side orphan lifecycle E2E를 완료할 수 있다.
- Android release keystore 4개 값이 있어야 서명된 APK/AAB를 만들 수 있다. debug APK는 release 산출물이 아니다.
- Render 운영 secret과 배포 권한이 있어야 production deploy와 dashboard를 확인할 수 있다.
- Sentry DSN·프로젝트가 없어 crash dashboard는 아직 연결하지 않았다.
- `flutter_application_1` 빈 폴더는 해당 폴더를 연 VS Code 창이 Windows handle을 유지해 삭제할 수 없다. 창을 닫은 뒤 폴더만 제거하면 된다.

## 2026-07-11 — 완료 감사 2차 보완

첫 번째 완료 보고 뒤 `HOUSEMOUSE_PLAN.md`의 B 항목과 테스트 전략을 항목별로 다시 대조했다. “코드가 존재한다”는 이유만으로 완료 처리하지 않고 실제 controller, DB schema, OpenAPI, 모바일 상태 처리와 통합 테스트가 요구사항을 직접 증명하는지 확인했다.

### README 질문·검토 흐름

- 일반 채팅과 분리된 README 질문 form을 추가했다. 폴더 목적, 주요 독자, 문체와 필수 section을 구조화해 `intent=README` command로 저장한다.
- 이 요청도 command outbox와 idempotency key를 사용하므로 오프라인 전송과 중복 방지 원칙이 같다.
- 모바일은 Desktop이 proposal summary로 보낸 실제 `readmeDraft`와 `readmeDiff`만 표시한다. 서버나 모바일이 기존 README를 읽은 것처럼 가장하거나 임의 초안을 성공 처리하지 않는다.
- `SCAN`, `ANALYZE`, `CREATE_RULE`, `README` command payload를 intent별 discriminated Zod union으로 제한했다. 임의 shell 필드, unsafe rule destination, 과도한 README 입력은 command 저장 전에 거절한다.

### 캐릭터 최소 성장과 알림

- MVP 범위를 외형 2개와 테마 2개로 제한했다. 갈색·기본 액세서리·포근한 테마는 기본이고, 승인 +1과 실행 성공 +2로 호감도 3이 되면 크림색·목도리·숲 테마를 해금한다.
- 서버가 `unlockedItems`를 계산하고 잠긴 선택의 PATCH를 `FEATURE_LOCKED`로 거절한다. 해금 목록에는 cosmetic namespace만 존재하며 파일 권한이나 규칙 결과에 사용하지 않는다.
- `animationsEnabled=false`를 저장할 수 있고 부분 PATCH가 기존 외형을 지우지 않도록 허용 필드만 병합한다.
- FCM 실기기 설정 전 대체 경로로 앱이 열린 동안 `proposal.created`, terminal `execution.updated`, transfer READY socket event를 전역 SnackBar로 알린다. REST replay는 과거 알림을 연속 표시하지 않고 화면·배지만 복구한다.

### Transfer failure와 pagination 방어

- `file_transfers.failure_code` migration `0014_loud_adam_destine.sql`을 추가했다.
- Desktop 전용 failure endpoint는 `SOURCE_NOT_FOUND`, `SOURCE_CHANGED`, `OUTSIDE_MANAGED_ROOT`, `SIZE_LIMIT_EXCEEDED`, `CHECKSUM_MISMATCH`만 받는다.
- 다른 device의 failure 보고는 거절하고 같은 failure replay는 멱등 처리하며 다른 code replay는 conflict로 처리한다.
- upload가 시작된 뒤 실패하면 partial object deletion job을 transaction에 남긴다.
- 모바일은 FAILED 상태의 구체 code를 읽어 원본 없음·변경·root 이탈·크기·checksum을 서로 다른 문구로 표시한다.
- Dio 오류 body의 `code`를 직접 읽도록 고쳐 HTTP exception 문자열에 code가 포함된다고 가정하지 않는다.
- 다음 page 요청은 기존 READY entries를 지우지 않으며 `DEVICE_OFFLINE`, `TIMED_OUT`, `CURSOR_INVALIDATED` fallback을 unit test로 고정했다.
- 실제 DB 통합 테스트에서 transfer idempotency payload binding, exact device auth, upload/download signed target TTL, 취소/failure tombstone을 검증했다.

### 스마트 캐시 policy 방어

- Policy update와 candidate submit이 같은 room advisory lock을 사용한다.
- 활성화된 policy의 quota를 `AVAILABLE + 유효 RESERVED`보다 작게 줄이는 요청은 `REJECTED_POLICY`로 거절한다.
- Policy disable은 AVAILABLE cache뿐 아니라 진행 중 reservation도 취소하고 각각 deletion tombstone을 만든다.
- `excludedPatterns`를 안전하게 escape한 `*`, `**`, `?` glob matcher로 후보 심사에 실제 적용한다. 정규식 문자를 실행 가능한 pattern으로 해석하지 않는다.
- 모바일 활성화 dialog는 전체 동기화가 아니며 승인된 일부 원본이 private object storage에 보관된다는 사실, room quota, 파일 한도, 제외 pattern과 삭제 정책을 설명한다. 신규 활성화는 명시적 확인 checkbox 없이는 저장할 수 없다.
- quota MB 변환, max-file ≤ room quota, 제외 pattern 개수·길이를 모바일과 서버 양쪽에서 검증한다.

### Event·공개 응답·계약 하드닝

- 채팅 저장, smart-cache policy/후보/완료/취소/삭제와 worker reservation 만료가 PostgreSQL transaction 안에서 각각 `chat.message.created`, `smart-cache.updated` sync event를 남긴다.
- command와 decision replay는 canonical JSON으로 원래 payload와 같은지 확인한다. 같은 key의 다른 intent, path 또는 승인 내용은 `IDEMPOTENCY_CONFLICT`다.
- command, decision, room, device, character, audit 공개 응답에서 DB 내부 `userId`, idempotency key와 device public key를 제거했다.
- 실제 controller decorator와 OpenAPI를 비교하는 `scripts/check-openapi-coverage.mjs`를 추가했다. 현재 58개 method가 모두 일치하며 CI의 `pnpm check:contracts` 단계에서 누락·초과 route와 끊어진 schema reference를 차단한다.
- `docs/FILE_TRANSFER_THREAT_MODEL.md`와 `docs/SMART_CACHE_PRIVACY_ADR.md`에 trust boundary, lifecycle, quota와 privacy 결정을 기록했다.

### B 요구사항별 현재 판정

| 영역 | 저장소에서 증명된 범위 | 판정 |
|---|---|---|
| Auth | 실제 Google/Firebase SDK 흐름, server token guard, 계정별 Drift 격리 | 코드 완료·Firebase 설정 대기 |
| Devices/Pairing/Presence | nonce/HMAC pairing, exact device JWT, revoke, Redis TTL/offline monitor | 구현·로컬 검증 완료 |
| Command/Proposal/Decision/Execution | strict contract, DB queue, payload-bound idempotency, audit, replay | 구현·DB 검증 완료 |
| Mobile product | home/room/rule/proposal/result/chat/README/files/cache, loading·empty·offline·error | 구현·19 tests 완료 |
| P0 FileTransfer | auth, source failure, signed TTL, checksum UX, ACK/취소/만료 deletion job | control plane 완료·실제 S3 E2E 대기 |
| Character/Affinity | domain-event state, ledger, 최소 해금, 외형·테마, animation off | metadata/UI 완료·Rive/overlay 대기 |
| Minimal notification | app-open Socket.IO 알림과 replay-safe badge | 구현 완료·FCM 실기기 대기 |
| P1 Smart cache | opt-in, exclude, quota lock, LRU, freshness, tombstone, offline UI | control plane 완료·실제 암호화 object E2E 대기 |
| Deploy/Recovery | Render Blueprint, readiness, backup/restore, CI, runbook | 구성 완료·운영 권한/secret 대기 |
| Android | `com.housemouse.app`, debug APK, release signing fail-fast | debug 완료·실제 keystore 대기 |

### 2차 감사 이후 외부 차단선

- Firebase Android native config, server service account와 FCM 실기기가 없어서 로그인·background push E2E는 실행할 수 없다.
- Rive asset/state machine 이름과 A의 Desktop overlay native window가 없어서 실제 animation/overlay 시연은 실행할 수 없다.
- Private S3-compatible bucket, encryption/lifecycle policy가 없어서 실제 PUT/HEAD/GET/DELETE와 provider orphan lifecycle은 실행할 수 없다.
- Android release keystore, Render 권한·운영 secret, Sentry 프로젝트/DSN이 없어 signed AAB, production deploy, crash dashboard를 만들 수 없다.
- 위 항목에는 가짜 provider, dummy asset, debug signing fallback을 넣지 않았다.

## 2026-07-11 — 외부 차단 감사 3차

동일한 외부 차단 조건을 세 번째 연속 목표 턴에서 다시 확인했다. 저장소 내부 구현이나 테스트 실패가 아니라 실제 provider 정보·asset·다른 담당 영역 산출물이 존재하지 않는 상태다.

### 확인한 현재 상태

- `apps/mobile/android/app/google-services.json`: 없음
- `apps/mobile/ios/Runner/GoogleService-Info.plist`: 없음
- Firebase server 필수 환경 변수: 3개 모두 없음
- Object storage 필수 환경 변수: 5개 모두 없음
- Android release signing 환경 변수: 4개 모두 없음
- 저장소 Rive `.riv` asset: 0개
- `apps/desktop` 실제 source file: 0개
- release `.jks`/`.keystore`: 0개
- `SENTRY_DSN`: 없음
- 임시 테스트 container/server: 0개
- `pnpm check:contracts`: 실제 controller 58 methods와 OpenAPI 일치
- `git diff --check`: 통과

### 재개에 필요한 입력

1. Firebase Android/iOS native config와 server service account
2. 실제 Android 기기에서 사용할 FCM project 설정
3. Rive asset, artboard와 state machine/input 이름 명세, A의 Desktop overlay shell
4. Private S3-compatible endpoint/bucket/credentials와 encryption/lifecycle 정책
5. Android release keystore path/alias/password 4개
6. Render 운영 권한과 secret, Sentry project/DSN

이 값 없이 성공 흐름을 추가하면 `AI_implement_rule.txt`의 fake provider, dummy asset, secret hardcoding 금지 원칙을 위반한다. 따라서 저장소 내부에서 의미 있게 진행할 B 작업은 모두 완료된 상태로 유지하고, 목표 전체는 위 입력이 들어올 때까지 외부 차단 상태로 전환한다.
