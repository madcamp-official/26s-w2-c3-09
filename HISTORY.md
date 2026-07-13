# MOUSEKEEPER 구현 이력 및 MVP 감사

> 감사 기준일: 2026-07-13
> 감사 기준 브랜치: `B` (`origin/main` 기준 v1.4 구현 포함)
> 기준 문서: `MOUSEKEEPER_PLAN.md`, `IMPLEMENTATION_PLAN.md`, `AI_implement_rule.txt`
> 판정 원칙: 파일이나 클래스가 있다는 이유만으로 완료 처리하지 않고, 호출 경로·영속화·안전 경계·테스트를 함께 확인한다.

## 1. 감사 범위와 판정 기호

추적 중인 소스, 테스트, 계약, migration, 모바일/데스크톱 플랫폼 설정, CI와 배포 파일을 전수 조사했다. `node_modules`, Rust `target`, Flutter/Gradle `build`, `.dart_tool`, Tauri 생성 schema처럼 다시 생성되는 산출물은 구현 판정에서 제외했다.

- `[x]`: 구현 코드와 직접 검증 근거가 모두 있음
- `[~]`: 핵심 경로는 있으나 기획 완료 조건 일부 또는 실제 환경 E2E가 남음
- `[ ]`: 기획에는 있으나 구현이 없거나 `UNCONFIGURED` 상태임

### 1.1 v1.4 연결·청결도·모바일 파일 접근

- [x] `mousekeeper-cleanliness-v1` 공식으로 Rust가 한 번 계산한 점수·감점·시각을 Desktop 표시, outbox, 서버 `RoomSnapshot`, 모바일 상세가 그대로 사용한다.
- [x] 모바일과 Desktop 어느 쪽에서든 device를 해제할 수 있고, 동일 `Idempotency-Key` 재시도는 저장된 결과를 재사용한다. 성공 transaction은 연결 room, 진행 중 browse·transfer·cache reservation까지 함께 종료한다.
- [x] 로그인 직후 활성 device/room을 서버에서 먼저 복구하는 fail-closed Pairing Gate를 적용했다. 활성 Desktop이 없으면 과거 cache나 main navigation을 렌더링하지 않는다.
- [x] room 연결은 양쪽에서 해제할 수 있다. Desktop은 watcher와 disposable index만 정리하고 managed root, 원본 파일, operation journal, undo 기록을 보존한다.
- [x] 모바일에 active folder 목록, breadcrumb 탐색, metadata, pagination, 만료형 transfer, `.part` 다운로드, SHA-256 확인 후 ACK 흐름을 연결했다.
- [x] 파일·폴더 이름 검색은 300ms debounce와 2자 제한, 현재 폴더/managed root scope, generation cursor, 오래된 응답 폐기를 적용했다. 서버는 전체 index를 보관하지 않고 만료 시 검색어와 결과 page를 지운다.
- [x] 모바일 appearance/accessory/theme/unlock 선택과 mutation 호출을 제거하고 고정 기본 외형·테마만 렌더링한다. 기존 DB 값과 deprecated PATCH 계약은 하위 호환을 위해 유지한다.
- [x] `device.revoked`·`room.removed`는 durable sync event를 먼저 기록한 뒤 publish하며, 모바일 Drift cache/outbox와 Desktop binding에 동일 reducer를 적용해 socket 유실 시 replay로 수렴한다.
- [x] `CharacterState` 9종과 lifecycle payload의 wire value를 TypeScript·JSON Schema·OpenAPI·Rust·Flutter에서 통일하고, 모바일 parser가 잘못된 status·ID·sequence를 fail-closed로 거부한다.
- [x] realtime replay·cursor·cache와 mutation queue를 UID·generation에 묶어 A 로그아웃→B 로그인 중 늦은 A 응답이 B 상태를 오염시키지 않도록 계정별로 격리한다.
- [~] PostgreSQL/Redis가 필요한 실제 transaction·object lifecycle 통합 suite는 로컬 Docker daemon 미가동으로 이번 회귀 실행에서 skip됐다. 코드·계약·클라이언트 테스트는 통과했지만 운영 배포 전 실제 DB에서 migration과 revoke 경합 E2E를 다시 실행해야 한다.

## 2. 구현 완료 기능

### 2.1 공통 계약과 품질 경계

- [x] NestJS controller 62개와 `packages/contracts/openapi.yaml` route coverage 일치
- [x] Command, Proposal, Decision, ExecutionResult, FileBrowse, FileTransfer, 자동 승인 정책 JSON/Zod schema
- [x] event envelope, 사용자별 단조 증가 sequence, correlation/aggregate 식별자
- [x] 중요 mutation의 idempotency key 및 replay 충돌 검증
- [x] 환경 변수 Zod 검증과 provider 미설정 시 `UNCONFIGURED` 처리
- [x] API request, Sentry event, 모바일/worker 오류에서 token·request body·절대 경로 제거
- [x] 단일 루트 `README.md` 유지

### 2.2 서버 API와 PostgreSQL

- [x] Firebase 사용자 token과 별도 device token 인증, ACTIVE device 재검증
- [x] 짧은 코드 pairing session 생성·claim·상태 polling, device 등록·해제
- [x] Redis heartbeat TTL, ONLINE/OFFLINE 전환, presence 만료 event
- [x] 사용자 소유권을 확인하는 device, room, rule, snapshot API
- [x] PostgreSQL durable command queue와 pending command 조회·상태 전이
- [x] proposal item 저장, 전체 승인/거절 decision 영속화, execution 결과와 rollback 상태
- [x] REST sync replay와 Socket.IO 사용자/device room 인증
- [x] file browse 요청, cursor page, timeout/offline/cursor 오류 상태
- [x] FileTransfer 요청·signed PUT/GET·HEAD 크기 확인·SHA-256 metadata·ACK/cancel/expiry 상태
- [x] character profile, affinity 원장과 중복 보상 방지; 기존 외형/테마 선택 API는 하위 호환용 deprecated 경로로만 유지
- [x] chat session/message 영속화와 모바일 세션 선택·생성·삭제·cursor pagination UI. AI 미설정은 assistant 성공을 만들지 않고 명시적으로 `UNCONFIGURED`
- [x] FCM token 등록/해제, notification outbox와 영구 무효 token 정리
- [x] audit activity summary와 privacy-safe request/Sentry 경계

### 2.3 데이터베이스와 worker

- [x] Drizzle schema 28개 PostgreSQL table과 17개 migration
- [x] user/device/room/command/proposal/decision/execution/sync/audit 관계와 인덱스
- [x] browse/transfer/object deletion job 영속화
- [x] notification job claim·lease·retry와 FCM batch 전송
- [x] 스마트 캐시 policy, candidate batch, reservation, cached file, deletion tombstone schema
- [x] `FOR UPDATE SKIP LOCKED` 기반 object/notification job 중복 처리 방지
- [x] transfer ACK·취소·실패·만료 후 object 삭제 재시도
- [x] cache disable, room 삭제, device revoke 후 cache object 삭제 재시도
- [x] orphan object sweep와 PostgreSQL backup/격리 restore drill script

### 2.4 Desktop Agent와 로컬 파일 안전

- [x] Tauri shell, system tray, 사용자 선택 autostart, MSI/NSIS bundle 설정
- [x] managed root canonicalization, 중복·부모/자식 overlap 차단
- [x] `..`, 절대 경로, symlink, junction, Windows reparse point 경계 우회 차단
- [x] SQLite WAL 기반 managed root, file index, operation journal, sync cursor/outbox
- [x] Tauri Tokio worker와 동기 SQLite API 사이의 nested-runtime-safe bridge
- [x] 전체 scan, 이름 검색, 상대 경로 browse, watcher incremental upsert/remove
- [x] 확장자·기간·이름 조건 Rule DSL과 결정론적 proposal 생성; 서버 계약은 modified/created age, size, relative path, file kind, TRASH, CREATE_DIR까지 additive 확장
- [x] source size/mtime precondition, destination 충돌 검출, no-overwrite move
- [x] journal-before-write, 복구형 `.mousekeeper_trash`, operation history, undo, journal recovery
- [x] README draft/hash/write/backup/undo 로컬 경로
- [x] cleanliness 0~100 계산과 감점 근거
- [x] OS keychain device token 저장과 React 계층·로그 비노출
- [x] pairing, heartbeat, REST replay, command polling, room/snapshot 동기화 background runtime
- [x] command → local proposal → server proposal, decision → safe execution → execution result processor
- [x] file browse page processor와 managed root 재검증
- [x] FileTransfer 상대 경로·source version·크기 검증, streaming PUT, 전후 source-change 확인, SHA-256
- [x] local usage score, manual pin/exclude preference, candidate reservation 연동
- [x] overlay window/event bridge가 직접 파일 API를 호출하지 않는 권한 경계
- [x] watcher 오류 로그에서 사용자 절대 경로 제거

### 2.5 Flutter 모바일

- [x] Firebase/Google 로그인 설정 경계와 명시적 configuration error 화면
- [x] pairing code 입력, device/room/presence 조회
- [x] home의 온라인·오프라인·빈 상태·재시도·pending badge
- [x] room 상세, cleanliness, command/execution 상태
- [x] rule 생성·수정, 구조화 command/README draft 요청
- [x] proposal item 상대 경로·이유·충돌 표시와 전체 승인/거절
- [x] online file browse pagination과 기존 page 보존 fallback
- [x] 다운로드 진행·SHA-256 검증 후에만 완료 처리
- [x] Drift 사용자별 display cache, mutation outbox, sync cursor
- [x] Socket.IO foreground event와 REST replay 복구
- [x] FCM foreground/background handler와 token 등록 경로
- [x] smart-cache opt-in policy, quota/file limit, freshness·last verified·pending command 경고
- [x] affinity 숫자·ledger 표시와 animation off 설정, 고정 기본 외형·테마
- [x] 8종 PNG 상태 이미지가 모바일과 Desktop build에 포함됨

### 2.6 인프라와 운영 코드

- [x] PostgreSQL·Redis local Docker Compose
- [x] AWS EC2 bootstrap, Nginx TLS reverse proxy, loopback API binding
- [x] API/worker/backup systemd unit과 private config directory
- [x] EC2 IAM role credential mode의 private S3 client
- [x] PostgreSQL daily backup timer와 restore drill
- [x] Render blueprint가 별도 배포 선택지로 유지됨
- [x] CI가 Node contract/type/test/build, Flutter analyze/test, Rust format/test/Tauri feature를 검사함

## 3. 구현 중이거나 누락된 기능

### 3.1 P0 핵심 누락

- [~] **모바일 로그인:** `com.mousekeeper.app`용 Firebase JSON을 로컬 설치하고 Firebase 활성 debug build까지 통과했다. 실제 Google login과 release SHA 검증은 남았다.
- [x] **파일 identity 연결:** SQLite `file_index`는 OS-backed nullable `file_id`를 저장한다. proposal/precondition은 같은 identity를 `source_file_id`/`sourceFileId`로 전달해 실행 직전 변경을 `SourceChanged`로 막고, file transfer sourceVersion도 기존 `hm:` hash 대신 같은 OS-backed identity를 사용해 업로드 전후 원본 교체를 `SOURCE_CHANGED`로 차단한다.
- [x] **watcher 재조정:** watcher event와 overflow full reindex가 있고, 30초 full reconcile pass가 active watched root의 SQLite browse/search index와 청결도 snapshot을 주기적으로 보정한다.
- [~] **파일 실행 action:** MOVE, QUARANTINE, README_WRITE, CREATE_DIR, CREATE_FILE 경로는 구현됐다. CREATE 계열의 실제 3자 release E2E와 journal/undo 회귀 고정은 남았다.
- [~] **온라인 파일 전달:** server 실제 S3 lifecycle E2E와 Desktop processor 단위 테스트는 있으나 Android↔Desktop↔server 한 기기 E2E 회귀가 현재 자동화되어 있지 않다.
- [~] **오프라인 큐:** DB queue, mobile/desktop outbox와 cursor replay는 있으나 프로세스 강제 종료·서버 재시작을 포함한 전체 E2E script가 없다.
- [~] **캐릭터 기본 상태:** 8종 PNG 상태 전환은 있으나 기획서의 실제 애니메이션 상태 머신은 없다.
- [~] **제한된 자연어 명령:** chat은 메시지를 영속하고 AI provider 인터페이스를 호출한다. provider가 `COMMAND_DRAFT`를 반환하면 서버가 `createCommandDraftSchema`로 재검증한 뒤 기존 사용자 승인 초안 경로로만 저장하며 바로 실행하지 않는다. metadata 주입이나 invalid expiresAt은 `AI_OUTPUT_INVALID`로 차단한다. 실제 provider 미설정 상태에서는 assistant 성공을 만들지 않고 `assistant: null`, `aiStatus: UNCONFIGURED`, `AI_PROVIDER_UNCONFIGURED`를 반환한다. 자연어→command draft를 만드는 실제 외부 AI adapter는 아직 없다.
- [ ] **동일 환경 배포 완료:** 이름 변경 뒤 Windows installer, signed Android AAB, 실제 Firebase login/FCM, 운영 systemd/path migration을 다시 검증하지 않았다.

### 3.2 P1 및 운영 누락

- [~] **스마트 캐시 암호화:** 서버 E2E는 AES-256-GCM 암호문 lifecycle을 검증하지만 실제 Desktop `smart_cache_processor`는 원본 파일을 그대로 signed PUT한다. 기능 flag를 켜기 전에 client-side encryption과 key 보관/폐기가 반드시 필요하다.
- [~] **스마트 캐시 최신성:** source version 전후 검증은 있으나 watcher 변경을 STALE event로 우선 outbox에 넣는 완전한 경로가 확인되지 않았다.
- [ ] **Rive:** `.riv`, artboard/state machine/input 명세와 실제 interaction이 없다.
- [ ] **Android release signing:** keystore 4개 환경 변수가 없으며 release build는 fail-fast한다.
- [ ] **Sentry 운영 수신:** SDK와 redaction test는 있으나 DSN/dashboard 수신 검증은 없다.
- [~] **FCM 실기기:** server/worker/mobile 코드와 새 package debug build는 있으나 release SHA 기준 background/terminated 수신을 다시 검증해야 한다.
- [~] **Socket.IO 다중 인스턴스:** Redis presence는 구현됐지만 Socket.IO Redis adapter는 없다. 단일 API process에는 동작하나 horizontal scale 전 선행해야 한다.
- [~] **worker 기술 차이:** 계획의 BullMQ 대신 PostgreSQL durable job + polling worker를 사용한다. 현재 단일 배포에는 기능적으로 유효하지만 계획과 다른 선택을 ADR로 남겨야 한다.
- [ ] **updater signing, macOS adapter/notarization, iOS release:** MVP 이후 또는 release hardening 범위로 남아 있다.

### 3.3 테스트·문서 공백

- [~] 일반 server test에서는 외부 DB/object storage가 필요한 7개 suite가 환경 변수 없이 skip된다. 운영 의존 E2E는 별도 opt-in CI job으로 분리해야 한다.
- [ ] `docs/e2e-scenarios.md`, `docs/threat-model.md`, `docs/adr/0001-local-first.md`가 계획 목록과 달리 없다.
- [ ] 독립 `presence`, `smart-cache`, `rule-dsl` JSON schema 일부가 없다. 현재 OpenAPI와 TypeScript Zod schema로 검증되지만 계약 산출물 목록과 일치시키는 결정이 필요하다.
- [ ] 100,000 entry, 3 managed root 성능 회귀와 watcher overflow 장시간 soak 결과가 없다.

## 4. 현재까지 개발 히스토리 요약

1. **모노레포와 계약 기반 구성:** Tauri/React, Flutter, NestJS/Fastify, worker, Drizzle/PostgreSQL, 공통 OpenAPI·event schema 구조를 만들었다.
2. **로컬 파일 안전 엔진:** root 경계, link/reparse 차단, no-overwrite, journal, trash, undo, README write와 CLI fixture 흐름을 구축했다.
3. **Cloud control plane:** auth/pairing/presence와 command→proposal→decision→execution durable 상태 흐름을 구축했다.
4. **오프라인 복구:** server sync sequence, mobile mutation outbox, desktop SQLite cursor/outbox와 REST replay를 연결했다.
5. **파일 접근 확장:** read-only browse와 별도 FileTransfer 상태 머신, signed object storage, checksum, ACK/TTL 삭제를 추가했다.
6. **스마트 캐시 control plane:** opt-in policy, usage score, quota reservation, freshness metadata와 object deletion tombstone을 추가했다.
7. **제품 경험 기반:** 모바일 home/room/rule partial-update와 expanded DSL form/proposal/files/chat session/character 설정, pairing gate pixel-fill loading과 Desktop Agent/file UI, 8종 PNG 상태 이미지를 연결했다.
8. **운영 하드닝:** AWS EC2, private S3 IAM role, Nginx/systemd, FCM worker, PostgreSQL backup/restore, Sentry redaction 경계를 추가했다.
9. **A/B 통합:** Desktop background processor가 server command, decision, browse, transfer, cache reservation 계약을 소비하도록 병합했다.
10. **브랜드 표준화:** package scope, Rust crate, Flutter package, Android/iOS/Tauri identifier, 환경 변수, local state dir, 배포 unit/path를 MOUSEKEEPER로 통일했다.

## 5. 네이밍 변경과 호환성 영향

- npm scope: `@mousekeeper/*`
- Flutter app/package: `mousekeeper`, shared asset package `mousekeeper_character_assets`
- Android: `com.mousekeeper.app`
- Tauri: `com.mousekeeper.desktop`, Rust crate `mousekeeper-desktop`
- device token prefix: `mk_device_`
- 환경 변수 prefix: `MOUSEKEEPER_*`
- local state/trash: `.mousekeeper`, `.mousekeeper_trash`
- EC2/systemd/Nginx/path: `mousekeeper-*`, `/opt/mousekeeper`, `/etc/mousekeeper`, `/var/backups/mousekeeper`

이 변경은 기존 device token, local state directory, Android Firebase client, installed app identifier와 운영 systemd path에 호환성 영향을 준다. 출시 전에는 재페어링 또는 명시적 데이터 migration 정책을 선택하고, 운영 서버는 기존 unit을 중지한 뒤 새 unit/path로 원자적으로 전환해야 한다.

## 6. 삭제·정리된 파일과 코드

### 6.1 삭제

- 소스가 들어온 뒤 남아 있던 `.gitkeep` 52개
- package asset과 SHA-256이 같은 루트 캐릭터 PNG 복사본 8개
- 참조되지 않는 구형 `packages/character-assets/mascot.png`와 동일 원본 복사본 1개
- 최신 Step 16 인수인계 문서로 대체된 `docs/B_HANDOFF_AFTER_STEP_13.md`, `docs/B_HANDOFF_AFTER_STEP_14.md`
- 새 Android package와 일치하지 않는 Firebase `google-services.json`
- 무시된 이전 IDE module 파일 2개와 빈 package directory 1개

### 6.2 코드 정리

- 모든 제품명 case, package/import, 환경 변수, 앱 identifier, crate, service/unit 파일명을 일관되게 변경
- Android Firebase Gradle plugin을 새 package용 JSON이 있을 때만 적용하도록 변경
- watcher startup/index 오류 로그가 절대 경로나 provider error 원문을 출력하지 않도록 축약
- root workspace package 이름을 `mousekeeper`로 정리
- 캐릭터 asset 선언에서 삭제된 구형 mascot 항목 제거

테스트 fixture와 현재 사용 중인 source/spec는 “오래됐다”는 이유만으로 삭제하지 않았다. 회귀 안전성을 증명하는 테스트는 유지하는 편이 의미 없는 파일 삭제보다 우선한다.

## 7. MVP 달성도 평가

### 7.1 P0 22개 항목 판정

| P0 항목 | 판정 | 핵심 근거 또는 남은 조건 |
|---|---|---|
| 모바일 로그인 | 부분 | 새 Firebase debug build 통과, 실제 login 필요 |
| 페어링 | 완료 | 짧은 코드 claim/polling/device token |
| Presence | 완료 | Redis TTL, OFFLINE event, UI 표시 |
| 여러 관리 폴더 | 완료 | N개 room/root 모델, overlap만 제한 |
| 명시적 관리 루트 | 완료 | native picker와 canonical root store |
| 로컬 파일 인덱스 | 완료 | SQLite index와 OS-backed nullable file_id 저장, proposal/precondition source identity 검증, transfer sourceVersion identity 검증 |
| watcher + 재조정 | 완료 | watcher/overflow reindex와 30초 active-root reconcile |
| Rule DSL | 부분 완료 | extension/age/name 결정론 평가 완료, 확장 DSL은 서버 계약 추가 후 Desktop evaluator 통합 대기 |
| 파일별 제안 | 완료 | reason/destination/collision UI |
| 전체 승인·거절 | 완료 | DB 영속·idempotency |
| 파일 실행 | 부분 | MOVE/QUARANTINE/README/CREATE_DIR/CREATE_FILE 구현, release E2E 고정 필요 |
| 복구형 trash | 완료 | metadata/journal/undo |
| README 제안·적용 | 완료 | hash/write/backup/undo |
| 청결도 | 완료 | 0~100와 감점 근거 |
| 작업 기록 | 완료 | local history와 server execution/audit |
| 온라인 파일 가져오기 | 부분 | 양쪽 구현, 실제 3자 E2E 자동화 필요 |
| 오프라인 큐 | 부분 | durable queue/outbox/replay, restart E2E 필요 |
| 캐릭터 상태 애니메이션 | 부분 | PNG 상태 이미지, 실제 animation 없음 |
| 모바일 집 | 완료 | room/presence/cleanliness/pending/result |
| 제한된 자연어 채팅 | 미충족 | AI gateway 없음, `UNCONFIGURED` |
| 호감도 | 완료 | 원장형 event와 숫자·완료 대사; 시각 해금 없음 |
| 동일 환경 배포 | 미충족 | rename 후 release artifact/운영 migration 미검증 |

- 엄격 완료: **13/22 (59%)**
- 부분 구현: **7/22 (32%)**
- 미충족: **2/22 (9%)**
- 부분 구현을 절반으로 계산한 참고 진척도: **75%**
- 최종 출시 판정: **FAIL**

`FAIL`은 현재 코드가 전반적으로 불안정하다는 뜻이 아니다. 핵심 control plane과 파일 안전 기반은 넓게 구현됐지만, 기획서의 출시 차단 조건은 하나라도 검증되지 않으면 통과시킬 수 없기 때문이다.

### 7.2 캐릭터 interaction 전에 반드시 닫아야 할 Critical Checklist

- [ ] Desktop smart-cache upload에 AES-256-GCM 등 client-side authenticated encryption을 적용하고 key를 OS 보안 저장소에 보관한다.
- [ ] 캐시 업로드 metadata를 암호문 size/checksum 기준으로 맞추고 mobile 복호화·tag 검증을 연결한다.
- [x] Indexed file identity를 proposal/precondition/transfer sourceVersion에서 size·mtime과 함께 비교하도록 연결했다.
- [ ] scan/write/transfer 동시성 제한을 명시하고 테스트한다.
- [ ] 승인된 CREATE_DIR/CREATE_FILE의 journal-before-write/no-overwrite/undo 정책을 실제 3자 E2E로 고정한다.
- [ ] 자연어 command provider를 선택하고 출력 Zod validation, 사용자 draft 확인, `UNCONFIGURED` 경계를 구현한다.
- [ ] 로컬 설치된 `com.mousekeeper.app` Firebase 설정에 debug/release SHA를 확인하고 Google login/FCM을 실기기 검증한다.
- [ ] Android↔server↔Desktop command/proposal/execute/undo와 browse/transfer를 하나의 반복 가능한 E2E로 고정한다.
- [ ] rename에 따른 device token/local state/EC2 systemd migration 절차를 실행하고 rollback을 검증한다.
- [ ] 운영 S3 encryption/lifecycle/public-access-block/IAM policy를 다시 확인한다.

## 8. 향후 업무 로드맵

### 우선순위 1 — 캐릭터 interaction 전 backend/infra 보완

1. 스마트 캐시를 기본 `false`로 유지한 채 실제 Desktop encryption/key lifecycle을 먼저 완성한다.
2. Indexed file ID 기반 proposal/precondition/transfer 안전 회귀와 CREATE_DIR/CREATE_FILE 안전 operation을 release E2E로 고정한다.
3. 외부 DB/S3 integration suite를 CI의 secret-protected opt-in job으로 실행한다.
4. Firebase debug/release SHA, release keystore, Sentry DSN을 secret manager와 provider console에 등록한다.
5. 새 systemd/path/IAM 이름으로 EC2를 migration하고 health/ready/worker/backup/restore를 재검증한다.
6. 물리 Android와 Windows PC를 사용하는 release E2E script와 체크리스트를 만든다.
7. worker polling 채택과 Socket.IO 단일 인스턴스 제약을 ADR로 기록한다.

### 우선순위 2 — Frontend UI/UX 고도화

1. 모바일과 Desktop의 loading/empty/offline/error 문구와 버튼 hierarchy를 같은 design token으로 통일한다.
2. pairing, managed root sync, proposal review, transfer progress를 한 화면에 과밀하게 넣지 말고 단계별 flow로 분리한다.
3. proposal item 필터·검색·충돌 우선 표시와 실행 후 항목별 결과 비교를 강화한다.
4. cache freshness, last verified, pending command warning을 아이콘뿐 아니라 텍스트로 항상 노출한다.
5. 접근성 label, keyboard navigation, reduced motion, 고대비 상태를 자동 UI test에 추가한다.

### 우선순위 3 — 실제 캐릭터 asset과 interaction 아키텍처

1. `CharacterEvent`를 유일한 입력으로 유지하고 UI가 file engine command를 직접 호출하지 못하게 한다.
2. domain state → presentation state 변환기를 공통 계약으로 두고 `IDLE`, `ANALYZING`, `WAITING_APPROVAL`, `WORKING`, `SUCCESS`, `ERROR`, `OFFLINE` 우선순위를 고정한다.
3. Rive artboard/state machine/input 이름을 versioned manifest로 관리하고 asset 미설정은 계속 `UNCONFIGURED`로 표시한다.
4. 캐릭터 탭/드래그/대사 interaction은 command draft까지만 생성하고 실제 쓰기는 proposal→approval→precheck→journal 경로만 통과시킨다.
5. animation controller는 모바일/Desktop adapter로 분리하고 reduced motion에서는 정적 PNG fallback을 사용한다.
6. interaction analytics에는 파일명·절대 경로·대화 원문을 넣지 않고 event kind와 latency 같은 최소 집계만 남긴다.

## 9. 검증 기록

| 검사 | 감사 시 결과 |
|---|---|
| `pnpm check:contracts` | controller 62개와 OpenAPI 일치 |
| Node workspace typecheck/build | 통과 |
| contracts test | 25개 통과 |
| server test | 40개 통과, 외부 DB/인프라 suite 7개 skip |
| worker test | 8개 통과 |
| desktop-agent-simulator test | 1개 통과 |
| Flutter analyze | 오류 0개 |
| Flutter test | 78개 통과 |
| Rust file-engine | unit 106개 + CLI integration 3개 통과 |
| Rust Desktop core | 기본 feature 142개 통과 |
| Rust format | 두 crate 모두 통과 |
| Tauri feature check/test | feature test 114개와 Clippy `-D warnings` 통과 |
| Android debug build | `UNCONFIGURED` build와 `com.mousekeeper.app` Firebase 활성 build 모두 통과 |

Firebase JSON은 API key가 포함된 provider 설정이므로 `.gitignore`에 유지하고 로컬에만 설치했다. CI와 팀원 환경에는 secret/file injection 방식이 필요하다.
