# MOUSEKEEPER

## 현재 배포 기준 (2026-07-16)

MouseKeeper는 Windows 데스크톱 에이전트와 Flutter Android 모바일 앱이 같은 서버 room을 공유하는 로컬 우선 파일 관리 서비스입니다.

### 최근 반영된 기능

- 자연어 파일 관리: 조회는 자동 실행하고, 이동·삭제·규칙·전송·undo는 승인 후 실행합니다.
- Desktop 파일 엔진: managed root 경계 검증, canonicalization, symlink/junction/reparse 차단, no-overwrite, journal-before-write, crash recovery, undo를 유지합니다.
- PC↔모바일 동기화: Socket.IO 실시간 이벤트와 REST replay/cursor를 함께 사용하며, 서버가 일시적으로 끊겨도 로컬 cache/outbox를 보존합니다.
- 관리 폴더 연결: 로컬 관리 폴더와 서버 room은 별도 상태입니다. `UNBOUND`면 파일 관리는 가능하지만 room 채팅은 사용할 수 없습니다. 모바일에서 PC 페어링을 해제하면 서버 측 모바일 권한이 취소되어 Desktop이 offline으로 표시될 수 있습니다.
- Desktop overlay: idle 말풍선은 생쥐 왼쪽에 배치되고, 드래그 중 15초가 지나면 화난 반응을 표시합니다.
- 모바일 치즈 상호작용: `KakaoTalk_20260716_033139321.gif`를 사용하며 idle 캐릭터와 같은 크기로 표시합니다.
- 모바일 미니게임: 미로 대신 10스테이지 턴제 치즈 퍼즐을 제공합니다. 방향 이동, 턴 제한, 상자 밀기, 키·문, 치즈 도착 승리, 7~10 스테이지 지정 경로가 포함됩니다.

### 배포 파일

- Android APK: `apps/mobile/build/app/outputs/flutter-apk/app-debug.apk`
- Windows MSI/NSIS: `apps/desktop/src-tauri/target/release/bundle/`
- 운영 API: `https://mousekeeper.madcamp-kaist.org`
- Android application id: `com.mousekeeper.app`

### 설치 및 실행

Android는 APK를 휴대폰으로 전송해 설치합니다. USB 디버깅 설치 시 `adb devices`가 `device` 상태인지 먼저 확인해야 합니다. Windows는 MSI를 실행해 설치하거나 NSIS 설치 파일을 사용합니다. Desktop은 서버 URL과 pairing 상태가 준비되어야 모바일 채팅·승인·파일 동기화를 사용할 수 있지만, 휴대폰 없이도 로컬 파일 관리는 계속할 수 있습니다.

### 검증 기록

- `flutter analyze`: 기능 코드 오류 없음(기존 lint info 경고만 남아 있음)
- Flutter 전체 테스트 및 미니게임 테스트를 변경 시마다 실행
- Android debug APK 빌드·실기기 설치·실행 확인
- Desktop Tauri Windows bundle 빌드 및 설치 확인
- 서버 `/health`, `/ready` HTTP 200 확인

### 주요 커밋

- `dcb55f5` 치즈 먹기 GIF 복원 및 idle 크기 일치
- `17f6867` 맵 차별화, 키·문 기믹
- `b548b95` 10스테이지·상자 밀기
- `0fc6961` 강제 경로를 7번째 스테이지부터 적용
- `84004d6` 다음 스테이지 진행 버튼 수정
- `ee5929d` 미로를 치즈 퍼즐로 교체
- `534d033` Desktop 말풍선 위치 조정
- `95f01ad` `origin/fix/minor-bugs` 병합

자세한 계약·안전 원칙과 소유권은 `MOUSEKEEPER_PLAN.md`, `AI_implement_rule.txt`, `docs/`를 참조합니다.

## 2026-07 현재 구현 반영 메모

- 데스크톱은 `MOUSEKEEPER_SERVER_BASE_URL`을 우선 사용하고, 배포 실행 환경에 값이 없으면 운영 API `https://mousekeeper.madcamp-kaist.org`를 기본값으로 사용합니다.
- 페어링은 사용자가 PC 연결 화면에서 시작하며, 휴대폰 없이도 데스크톱 로컬 파일 기능은 유지됩니다.
- 모바일 채팅은 realtime, 앱 복귀 재조회, 5초 fallback 재조회를 사용합니다. 데스크톱은 realtime/replay와 세션·메시지 재조회를 사용합니다.
- 승인·거절 결과는 `DECISION` 채팅 메시지와 `decision.created` sync event로 저장되며 데스크톱은 `PROPOSAL`과 `DECISION` 타입을 검증합니다.
- 파일 변경은 proposal/approval/precheck/journal/execute 흐름을 유지하고 provider 미설정은 `UNCONFIGURED`로 표시합니다.
- 집 이미지 참조는 `house_1.png`~`house_5.png`로 정정했고 통합 배경은 `packages/character-assets/house/house_background.png`입니다.

### 최근 검증 및 제한

- 운영 `/health`, `/ready` 200 확인, 데스크톱 Tauri Windows 번들 및 Android 운영 APK 빌드·설치 완료.
- OCR, 역방향 upload, 일부 문서 분석·smart-cache provider는 계획대로 미구현 또는 `UNCONFIGURED` 상태입니다.

MOUSEKEEPER는 데스크톱의 관리 폴더를 안전하게 분석하고, 모바일과 데스크톱에서 같은 AI 채팅을 이어가며, 사용자의 승인을 받은 파일 작업만 실행하는 local-first 크로스플랫폼 파일 관리 서비스입니다.

> 현황 기준일: 2026-07-15
>
> 기준 브랜치: `main` (`679fb22`)
>
> 운영 API: `https://mousekeeper.madcamp-kaist.org`
>
> Android application ID: `com.mousekeeper.app`

## 현재 상태 요약

| 영역 | 상태 | 요약 |
|---|---|---|
| Desktop Agent | 핵심 구현 완료 | Tauri/React UI, Rust 파일 안전 엔진, 관리 폴더, watcher, 제안·실행·undo, 채팅, 파일 전달 |
| Mobile | 핵심 구현 완료 | Google 로그인 경계, 1:1 PC 페어링, 폴더·파일 탐색, 동일 채팅 세션, 캐릭터 홈, 미니게임 |
| Server | 핵심 구현 완료 | NestJS API, PostgreSQL, Redis/Valkey, Socket.IO, AI provider, 명령·제안·실행 상태 머신 |
| Worker | 구현 완료 | FCM 전송, 만료 object 삭제, 재시도 가능한 PostgreSQL 작업 처리 |
| AWS 운영 | 동작 중 | EC2, Nginx/TLS, systemd, private S3 IAM role, PostgreSQL backup |
| Release hardening | 진행 중 | Android release signing, Sentry 운영 수신, 일부 실기기 E2E와 smart-cache key sync가 남음 |

2026-07-15 현재 `/health`와 `/ready`는 모두 HTTP 200을 반환합니다. 이 확인은 API와 필수 서버 의존성이 준비되었다는 뜻이며, Google 로그인·FCM·OpenAI·S3처럼 외부 자격 증명이 필요한 모든 기능의 성공을 대신하지는 않습니다.

MVP의 핵심 코드 흐름은 구축되어 있지만, signed Android release와 전체 실기기 3자 E2E까지 포함한 “출시 완료” 상태는 아닙니다. 외부 provider가 없거나 계약이 아직 없는 기능은 성공으로 가장하지 않고 `UNCONFIGURED` 또는 명시적 오류로 종료합니다.

## 프로젝트 목표

MOUSEKEEPER는 다음 세 가지를 동시에 검증합니다.

- **Cross-Platform:** Windows Desktop Agent와 Flutter Android 앱이 같은 사용자·폴더·채팅 상태를 공유합니다.
- **Realtime Interaction:** Socket.IO 이벤트와 REST replay를 함께 사용해 연결, 폴더, 명령, 제안, 실행, 채팅 상태를 동기화합니다.
- **LLM Wrapper:** OpenAI Responses API의 구조화 출력을 다시 스키마로 검증하고, AI가 파일을 직접 변경하지 못하도록 승인 경계 뒤에 둡니다.

## 제품 원칙

1. 사용자가 직접 선택한 managed root 밖의 파일은 읽거나 변경하지 않습니다.
2. AI는 대화·규칙·명령 초안만 만들며 파일 작업을 직접 실행하지 않습니다.
3. 쓰기 작업은 `제안 → 사용자 승인 → 실행 직전 재검증 → journal → no-overwrite 실행` 순서를 지킵니다.
4. 서버에는 전체 로컬 파일 인덱스와 절대 경로를 영구 저장하지 않습니다.
5. 실시간 이벤트를 놓쳐도 durable event와 REST cursor replay로 최종 상태가 수렴해야 합니다.
6. secret, API key, 서비스 계정 원문은 Git에 저장하지 않습니다.
7. provider나 환경 변수가 없으면 fake success 대신 `UNCONFIGURED`를 표시합니다.

## 핵심 사용자 흐름

### 1. 로그인과 페어링

```text
Google 로그인
  → 서버에서 Firebase ID token 검증
  → Desktop이 6자리 pairing code 생성
  → Mobile이 code claim
  → 사용자와 Desktop device 연결
  → ACTIVE device/room 확인 후 Mobile Home 진입
```

- 모바일은 활성 상태에서 하나의 데스크톱 연결을 기준으로 동작합니다.
- 페어링된 상태의 설정 화면은 연결된 데스크톱 이름과 연결 해제 동작을 제공합니다.
- 연결이 없으면 모바일은 Pairing Gate 밖으로 나가지 않습니다.
- 데스크톱은 모바일보다 먼저 관리 폴더를 등록할 수 있습니다.
- 저장된 휴대폰 연결이 유효하지 않으면 Desktop onboarding은 다시 pairing 단계로 돌아갑니다.

### 2. 관리 폴더 자동 동기화

```text
Desktop에서 폴더 선택
  → canonical path와 경계 검증
  → 로컬 managed root 저장
  → server room 생성 또는 기존 room 확인
  → room.created/replay
  → Mobile 폴더 목록과 Home에 자동 반영
```

- 별도의 큰 “등록” 버튼 없이 폴더 선택과 등록이 한 흐름으로 처리됩니다.
- 페어링 전에 만든 managed root도 페어링 후 background reconcile에서 서버 room으로 보정됩니다.
- 폴더 해제 시 watcher와 disposable index만 정리하며 원본 파일, journal, undo 기록은 보존합니다.
- 모바일 Home은 폴더 슬롯에 `배경 5 → 배경 1 → 배경 2 → 배경 3 → 배경 4` 순서로 배경을 매핑합니다.

### 3. 파일 정리 승인 흐름

```text
Mobile/Desktop 채팅 또는 수동 명령
  → Server에 command/draft 영속화
  → Desktop이 managed root 안에서 proposal 생성
  → Mobile/Desktop에 승인 카드 표시
  → 사용자 승인 또는 거절
  → Desktop precondition 재검증
  → journal-before-write
  → no-overwrite 실행
  → execution result 동기화
  → 필요 시 undo
```

지원되는 핵심 로컬 작업은 `MOVE`, `RENAME`, `TRASH`/quarantine, `CREATE_DIR`, 빈 파일 `CREATE`, README 생성·수정입니다. 일부 기능은 명령 계약과 실제 실행 가능 범위가 다르므로 아래의 미완료 항목을 확인해야 합니다.

### 4. 온라인 파일 탐색과 다운로드

```text
Mobile browse/search 요청
  → Server가 만료형 request 생성
  → Desktop이 로컬 SQLite index 조회
  → page/cursor 결과 전달
  → Mobile이 파일 선택
  → Desktop source identity·size·mtime 재검증
  → signed object transfer
  → Mobile .part 저장 및 SHA-256 검증
  → 최종 저장과 ACK
  → Worker가 만료 object 삭제
```

검색은 300ms debounce, 최소 2자, scope와 generation cursor를 사용합니다. 원본이 전송 중 변경되면 `SOURCE_CHANGED`, cursor가 무효화되면 `CURSOR_INVALIDATED`로 실패합니다.

### 5. 데스크톱·모바일 공용 AI 채팅

- 채팅 session과 message는 PostgreSQL에 저장됩니다.
- 데스크톱과 모바일은 같은 room의 같은 session API를 사용합니다.
- 어느 쪽에서 보낸 메시지도 `chat.message.created` 이벤트와 cursor 보정으로 반대편에 표시됩니다.
- 최대 5개의 실제 채팅 session을 유지하며, 인위적인 여섯 번째 `승인 대기방`은 제거했습니다.
- 승인할 제안은 별도 가상 채팅방이 아니라 실제 채팅의 proposal card와 proposal 화면에서 처리합니다.
- 사용자 메시지는 즉시 화면에 추가되고 AI 처리 중에는 `답을 만들고 있어요!`를 표시합니다.
- 현재 OpenAI 호출은 구조화된 단일 응답 방식이며 token streaming은 아직 구현하지 않았습니다.

## 시스템 아키텍처

```text
┌──────────────────────────────┐
│ Flutter Mobile              │
│ Riverpod · Drift · Socket.IO│
└──────────────┬───────────────┘
               │ HTTPS + WebSocket
┌──────────────▼───────────────┐
│ NestJS/Fastify Server       │
│ Auth · Pairing · Chat · API │
└───────┬──────────┬───────────┘
        │          │
        │          ├─ Redis/Valkey: presence TTL, Socket.IO adapter
        │          ├─ PostgreSQL: durable product/control-plane data
        │          └─ Private S3: transfer/cache object
        │ HTTPS + WebSocket
┌───────▼──────────────────────┐
│ Tauri Desktop Agent         │
│ React UI + Rust file engine │
│ SQLite WAL · watcher        │
└──────────────────────────────┘

Worker
  ├─ FCM notification delivery
  ├─ expired transfer/cache object cleanup
  └─ durable retry/lease processing
```

### 저장 경계

| 위치 | 저장 내용 |
|---|---|
| PostgreSQL | 사용자, device, pairing, room, rule, command, proposal, decision, execution, chat, transfer/cache metadata, sync/audit event |
| Redis/Valkey | presence TTL, Socket.IO pub/sub adapter, 짧은 수명의 연결 상태 |
| Desktop SQLite | managed root, 로컬 file index, operation journal, sync cursor/outbox, local preference |
| Mobile Drift | 사용자별 표시 cache, mutation outbox, sync cursor, 검증된 다운로드 metadata |
| Private S3 | 만료형 FileTransfer object와 암호화된 smart-cache object |

## 기술 스택

| 영역 | 기술 |
|---|---|
| Desktop UI | Tauri 2, React 18, Vite, TypeScript |
| Desktop engine | Rust, Tokio, SQLite WAL, OS keychain, filesystem watcher |
| Mobile | Flutter, Dart, Riverpod, GoRouter, Drift, Firebase Auth/Messaging |
| Server | NestJS 11, Fastify 5, Socket.IO, Zod |
| Data | PostgreSQL 17, Drizzle ORM, Redis/Valkey 8 |
| Worker | TypeScript, PostgreSQL durable jobs, Firebase Admin, AWS SDK |
| AI | OpenAI Responses API, strict JSON Schema output, Zod 재검증 |
| Storage/Infra | AWS EC2, Nginx, systemd, private S3, TLS |
| Contracts | OpenAPI, JSON Schema, TypeScript schema/fixture |

계획 초기의 BullMQ 대신 현재 worker는 PostgreSQL durable job과 polling/lease를 사용합니다. 기능적으로 동작하지만 이 차이는 향후 ADR로 명시할 필요가 있습니다.

## 저장소 구조

```text
apps/
  desktop/                  Tauri/React Desktop Agent
    src/features/           agent, files, character, onboarding, overlay
    src-tauri/              Rust file engine와 native bridge
  mobile/                   Flutter Android app
  server/                   NestJS control plane/API/WebSocket
  worker/                   FCM 및 object lifecycle worker

packages/
  contracts/                OpenAPI, Zod/TypeScript 계약, event schema
  database/                 Drizzle schema와 SQL migration
  character-assets/         공용 캐릭터·배경 asset

tools/
  file-engine-cli/          로컬 파일 안전 엔진 검증 CLI
  desktop-agent-simulator/  server/agent 계약 시뮬레이터

infra/aws/                  EC2 bootstrap, Nginx, systemd, backup/restore
scripts/                    계약 검사, 배포/운영 점검, release preflight
test-fixtures/              파일 트리, 전송, smart-cache 회귀 fixture
docs/                       안전·계약·배포·E2E 문서
```

## 구현 현황

### Desktop Agent

- [x] managed root 등록, canonicalization, 부모·자식 root overlap 차단
- [x] `..`, 절대 경로, symlink, junction, Windows reparse point escape 차단
- [x] SQLite WAL 기반 root/index/journal/cursor/outbox 저장
- [x] 전체 scan, paginated browse, filename search, watcher debounce와 reconcile
- [x] watcher 누락을 복구하는 주기적 full index reconcile과 청결도 snapshot 계산
- [x] Rule DSL 평가와 결정론적 proposal 생성
- [x] source identity·size·mtime와 destination conflict precheck
- [x] journal-before-write, no-overwrite, `.mousekeeper_trash`, undo, crash recovery
- [x] README draft/write/backup/undo 경로
- [x] 6자리 pairing, 2초 status polling, 5초 heartbeat
- [x] 15초 fast reconcile, 30초 transfer/cache reconcile, WebSocket 즉시 wake-up
- [x] 페어링 전 managed root 등록과 페어링 후 자동 room 동기화
- [x] system tray, autostart, overlay window, Windows MSI/NSIS 설정
- [x] 모바일과 동일한 chat session/message를 사용하는 Desktop chat overlay
- [x] house overlay 내부 캐릭터 이동 제한과 더 타이트한 interaction hitbox
- [x] FileTransfer streaming upload, SHA-256, 전송 전후 source 변경 감지
- [x] AES-256-GCM smart-cache 암호화 업로드

### Mobile

- [x] Firebase/Google 로그인 configuration gate와 명시적 오류 화면
- [x] pairing code 입력, 연결 상태 gate, 연결된 PC 이름과 연결 해제 설정
- [x] managed folder 자동 수신, 폴더 선택, Home 배경 매핑
- [x] 폴더가 없을 때 `폴더를 연결해 주세요!` 안내
- [x] 고정 Y축에서 idle/walk GIF 전환, 방향별 좌우 반전, 캐릭터·배경 동방향 패닝
- [x] 캐릭터 탭 시 현재 folder의 공용 AI chat으로 이동
- [x] folder·console·cheese·hanger 이미지 기반 하단 navigation
- [x] 파일 목록 modal/page, breadcrumb, 검색, pagination, 검증 다운로드
- [x] 미로 찾기, 30초 두더지형 게임, random cage escape 게임
- [x] draggable cheese feeding과 1초 feeding feedback
- [x] wardrobe placeholder route
- [x] Desktop과 동일한 chat session, message, proposal/draft UI와 optimistic send
- [x] synthetic `승인 대기방` 제거; 실제 session만 표시
- [x] proposal 승인·거절, 실행 결과, history 흐름
- [x] 사용자별 Drift cache/outbox/cursor와 로그아웃 세대 격리
- [x] Socket.IO item patch와 REST replay, 5초 lightweight connection safety check
- [x] cheese loading overlay, pixel font/theme, 반응형 말줄임과 크기 조정
- [x] smart cache 자동 정책; 일반 사용자에게 수동 cache 설정 UI를 노출하지 않음

### Server API

- [x] Firebase user token과 별도 Desktop device token 인증
- [x] pairing session create/claim/status와 device revoke
- [x] room 생성·삭제·자동 managed-folder sync event
- [x] Redis presence TTL과 ONLINE/OFFLINE event
- [x] `/v1/home/summary`, `/v1/connections/summary` 분리
- [x] command, proposal, decision, execution durable 상태 머신
- [x] rule/rule-draft 생성·검증·확정 API
- [x] chat session 생성·수정·삭제, cursor message API
- [x] desktop/mobile 공용 chat realtime event
- [x] file browse/search request와 page/cursor lifecycle
- [x] FileTransfer signed URL, ACK, cancel, expiry lifecycle
- [x] smart-cache policy, quota, reservation, usage score, stale/delete lifecycle
- [x] Socket.IO `/realtime`과 Redis multi-process adapter
- [x] durable sync event, 사용자별 sequence와 REST replay
- [x] FCM token/outbox와 privacy-safe audit
- [x] OpenAI Responses provider와 strict structured output 검증
- [x] AI에 room name/root alias/existing rules context 전달
- [x] 최근 browse, snapshot, proposal 등 server cache 기반 file context 전달

### Worker와 인프라

- [x] FCM batch 전송, lease/retry, 영구 무효 token 정리
- [x] transfer/cache object 만료·취소·실패 후 삭제 재시도
- [x] orphan object sweep
- [x] local PostgreSQL·Valkey Docker Compose
- [x] AWS EC2 bootstrap과 loopback server binding
- [x] Nginx HTTP→HTTPS, TLS reverse proxy
- [x] API/worker/backup systemd unit
- [x] private S3 bucket과 EC2 IAM role credential 방식
- [x] PostgreSQL backup timer와 격리 restore drill script
- [x] fail-closed release E2E preflight

## 최근 주요 변경

| 날짜 | 변경 |
|---|---|
| 2026-07-15 | Desktop의 기존 managed root를 페어링 직후 자동으로 room과 동기화 |
| 2026-07-15 | Desktop과 Mobile 채팅을 동일 server session으로 통합하고 양방향 realtime 보정 |
| 2026-07-15 | Mobile 채팅 목록의 synthetic `승인 대기방` 제거 |
| 2026-07-15 | AI에 room context와 최근 server-side file cache context 추가 |
| 2026-07-15 | OpenAI 불완전 structured output 1회 재시도와 fail-closed 검증 강화 |
| 2026-07-15 | smart cache를 자동 정책으로 전환하고 수동 설정 중심 UI 제거 |
| 2026-07-15 | 픽셀 홈 navigation, 미니게임 3종, cheese feeding, wardrobe route 추가 |
| 2026-07-15 | managed-folder 배경 순서 회전, maze mouse와 random cage map 적용 |

세부 커밋 단위 이력은 `git log`, 구현 감사 기록은 [HISTORY.md](HISTORY.md)에서 확인할 수 있습니다.

## AI 채팅

### 동작 방식

1. 사용자 메시지를 먼저 PostgreSQL에 저장합니다.
2. 서버가 room과 최근 파일 context를 구성합니다.
3. OpenAI Responses API에 strict JSON Schema 형식으로 요청합니다.
4. 반환 JSON을 다시 Zod/DTO로 검증합니다.
5. 일반 답변 또는 command/rule draft를 채팅 message로 저장합니다.
6. draft는 사용자가 확정하더라도 기존 proposal·approval 경계를 우회하지 않습니다.

### 필요한 서버 환경 변수

- `AI_PROVIDER=openai`
- `AI_API_KEY`
- `AI_MODEL` — 현재 목표 운영 모델은 `gpt-5.4`
- `AI_TIMEOUT_MS`
- `AI_MAX_OUTPUT_TOKENS`

키가 없거나 모델 접근 권한이 없거나 provider 응답이 계약 검증을 통과하지 못하면 각각 `AI_PROVIDER_UNCONFIGURED`, `AI_OUTPUT_INVALID` 등 명시적 상태로 처리합니다. 모델 출력 원문과 사용자 파일명·경로는 로그에 남기지 않습니다.

## 자동 스마트 캐시

스마트 캐시는 일반 사용자가 매번 켜고 끄는 기능이 아니라 server/Desktop이 자동 정책으로 관리합니다.

- Desktop이 usage event와 pin/exclude policy를 기반으로 후보를 선정합니다.
- 서버가 quota와 max file size를 검증하고 upload reservation을 발급합니다.
- Desktop은 AES-256-GCM으로 암호화한 뒤 ciphertext만 S3에 올립니다.
- 서버는 ciphertext checksum/size와 encryption metadata만 저장합니다.
- 모바일은 실제 key가 제공될 때 ciphertext와 GCM tag, plaintext checksum을 검증한 뒤 저장합니다.

현재 Desktop→Mobile 복호화 key sync provider가 없으므로 encrypted mobile cache 다운로드 기본 경로는 `UNCONFIGURED: SMART_CACHE_DECRYPTION_KEY_SYNC`로 닫혀 있습니다. 이 항목을 해결하기 전에는 스마트 캐시의 물리 Android 전체 E2E를 완료로 판정하지 않습니다.

## 실행 방법

### 필수 도구

- Node.js와 `pnpm@11.11.0`
- Docker Desktop 또는 Docker Engine
- Flutter SDK와 Android SDK
- Rust stable MSVC toolchain
- Windows Tauri build prerequisites
- Google 로그인 사용 시 프로젝트와 package가 일치하는 `google-services.json`

### 1. 의존성 설치

```powershell
pnpm install --frozen-lockfile

Set-Location apps/mobile
flutter pub get
Set-Location ../..
```

### 2. PostgreSQL·Valkey 실행

루트 `.env.docker.example`의 key를 참고해 Git에 포함되지 않는 로컬 `.env`에 실제 개발 값을 넣습니다. 빈 값으로 실행하면 Compose가 fail-fast합니다.

```powershell
docker compose up -d
docker compose ps
```

그다음 현재 shell에 서버가 사용할 `DATABASE_URL`, `REDIS_URL`과 필수 환경 변수를 주입하고 migration을 적용합니다.

```powershell
pnpm --filter @mousekeeper/database db:migrate
```

### 3. Server와 Worker 실행

필수 key 목록은 [.env.example](.env.example)을 기준으로 합니다. Firebase Admin credential은 파일 경로 또는 개별 환경 변수 중 한 방식으로 실제 값을 주입해야 합니다.

```powershell
pnpm --filter @mousekeeper/server start:dev
pnpm --filter @mousekeeper/worker start
```

로컬 확인 주소:

- `http://127.0.0.1:3000/health`
- `http://127.0.0.1:3000/ready`

### 4. Desktop 실행

```powershell
$env:MOUSEKEEPER_SERVER_BASE_URL = "http://127.0.0.1:3000"
pnpm --filter @mousekeeper/desktop tauri:dev
```

운영 서버를 사용할 때는 URL을 `https://mousekeeper.madcamp-kaist.org`로 바꿉니다.

Windows bundle:

```powershell
pnpm --filter @mousekeeper/desktop tauri:build
```

### 5. Android 실기기 실행

로컬 서버를 USB Android 기기에서 사용할 때:

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe" reverse tcp:3000 tcp:3000

Set-Location apps/mobile
flutter run `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=MOUSEKEEPER_API_URL=http://127.0.0.1:3000 `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<실제-Web-OAuth-Client-ID>
```

운영 서버를 사용할 때는 `MOUSEKEEPER_API_URL=https://mousekeeper.madcamp-kaist.org`를 사용하며 `adb reverse`는 필요하지 않습니다.

Debug APK 생성:

```powershell
Set-Location apps/mobile
flutter build apk --debug `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=MOUSEKEEPER_API_URL=https://mousekeeper.madcamp-kaist.org `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<실제-Web-OAuth-Client-ID>
```

기본 산출물 경로는 `apps/mobile/build/app/outputs/flutter-apk/app-debug.apk`입니다.

## 운영 배포

운영 API는 `https://mousekeeper.madcamp-kaist.org`입니다.

- Nginx가 80을 443으로 redirect합니다.
- API는 loopback에서만 listen하며 3000, PostgreSQL 5432, Valkey 6379는 외부에 노출하지 않습니다.
- server와 worker는 systemd로 실행합니다.
- S3 static access key 대신 최소 권한 EC2 IAM role credential을 사용합니다.
- DB migration, service restart, health/ready, worker, backup을 배포마다 함께 확인해야 합니다.

상세 절차는 [AWS EC2 배포 문서](docs/AWS_EC2_DEPLOYMENT.md)를 따릅니다.

```powershell
.\scripts\check-production-endpoint.ps1 `
  -BaseUrl https://mousekeeper.madcamp-kaist.org
```

## 검증 방법

### Node/계약

```powershell
pnpm check:contracts
pnpm typecheck
pnpm test
pnpm build
```

### Mobile

```powershell
Set-Location apps/mobile
flutter analyze
flutter test
```

### Desktop Rust

```powershell
Set-Location tools/file-engine-cli
cargo fmt --check
cargo clippy -- -D warnings
cargo test

Set-Location ../../apps/desktop/src-tauri
cargo fmt --check
cargo clippy --all-targets --features tauri-commands -- -D warnings
cargo test --features tauri-commands
```

Windows에서 symlink 테스트가 권한 오류 1314로 실패하면 Windows 개발자 모드를 켜거나 관리자 터미널을 사용해야 합니다. 권한 부족을 제품 성공으로 간주해 테스트를 삭제하거나 무조건 skip하지 않습니다.

### Release preflight

```powershell
pnpm e2e:preflight
```

이 스크립트는 API URL, health/ready, Android tool/device, Rust toolchain, 필수 경로와 dirty worktree를 확인합니다. 필요 조건이 없으면 `UNCONFIGURED` 또는 `FAIL`로 종료합니다.

### 최근 확인된 검증 기록

| 날짜 | 검사 | 결과 |
|---|---|---|
| 2026-07-15 | Server TypeScript | 통과 |
| 2026-07-15 | Desktop TypeScript | 통과 |
| 2026-07-15 | OpenAI Responses provider spec | 11개 통과 |
| 2026-07-15 | Mobile `flutter analyze` | 오류 0개 |
| 2026-07-15 | Mobile chat widget/reducer spec | 17개 통과 |
| 2026-07-13 감사 기준 | Flutter 전체 기록 | 78개 통과 |
| 2026-07-13 감사 기준 | Rust file-engine 기록 | unit 106개 + integration 3개 통과 |
| 2026-07-13 감사 기준 | Tauri feature 기록 | 114개 통과, Clippy 통과 |
| 2026-07-15 | 운영 `/health`, `/ready` | 각각 HTTP 200 |

과거 숫자는 해당 날짜의 감사 기록이며 현재 전체 suite의 자동 통과를 대신하지 않습니다. release 전에는 위 명령을 다시 실행해야 합니다.

## 환경 변수

루트 [.env.example](.env.example)은 key 이름만 관리합니다. 실제 값은 로컬 shell, 안전한 비추적 `.env`, CI secret 또는 EC2의 `/etc/mousekeeper/*.env`에 주입합니다.

### Server

- Core: `NODE_ENV`, `PORT`, `SERVER_HOST`, `DATABASE_URL`, `REDIS_URL`, `WEB_ORIGIN`
- Auth: `JWT_OR_DEVICE_TOKEN_SECRET`
- Firebase: `FIREBASE_SERVICE_ACCOUNT_PATH` 또는 `FIREBASE_PROJECT_ID`, `FIREBASE_CLIENT_EMAIL`, `FIREBASE_PRIVATE_KEY`
- AI: `AI_PROVIDER`, `AI_API_KEY`, `AI_MODEL`, `AI_TIMEOUT_MS`, `AI_MAX_OUTPUT_TOKENS`
- Transfer/cache: `FILE_TRANSFER_MAX_BYTES`, `FILE_TRANSFER_TTL_SECONDS`, `SMART_CACHE_ENABLED`, `SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES`, `SMART_CACHE_DEFAULT_MAX_FILE_BYTES`
- Observability: `SENTRY_DSN`

### Worker/Object storage

- `DATABASE_URL`
- `OBJECT_STORAGE_ENDPOINT`, `OBJECT_STORAGE_REGION`, `OBJECT_STORAGE_BUCKET`
- 로컬 static credential 사용 시 `OBJECT_STORAGE_ACCESS_KEY_ID`, `OBJECT_STORAGE_SECRET_ACCESS_KEY`
- EC2에서는 IAM role을 사용하므로 static credential을 비워 둡니다.
- `FCM_ENABLED`와 Firebase Admin credential

### Clients

- Desktop: `MOUSEKEEPER_SERVER_BASE_URL`
- Mobile dart-define: `FIREBASE_ENABLED`, `MOUSEKEEPER_API_URL`, `GOOGLE_SERVER_CLIENT_ID`, 선택적 `SENTRY_DSN`
- Android release: `MOUSEKEEPER_ANDROID_KEYSTORE_PATH`, `MOUSEKEEPER_ANDROID_KEY_ALIAS`, `MOUSEKEEPER_ANDROID_STORE_PASSWORD`, `MOUSEKEEPER_ANDROID_KEY_PASSWORD`

## 미완료·검증 대기 항목

### 출시 전 필수

- [ ] Android release keystore를 secret으로 주입하고 signed AAB 검증
- [ ] Firebase release SHA 기준 Google 로그인 재검증
- [ ] FCM foreground/background/terminated 실기기 수신 검증
- [ ] Sentry 운영 DSN과 redaction된 event 수신 확인
- [ ] Android ↔ Server ↔ Desktop command/proposal/execute/undo 전체 E2E 자동화
- [ ] browse/search/download를 실제 기기와 managed root에서 반복 검증
- [ ] EC2 migration/rollback, worker, backup/restore release runbook 재실행

### 기능 계약이 아직 닫힌 항목

- [ ] Desktop rule evaluator dry-run transport — 현재 `RULE_DRAFT_PREVIEW_UNCONFIGURED`
- [ ] Mobile→Desktop inverse upload — 현재 `UPLOAD_TRANSFER_UNCONFIGURED`
- [ ] DOWNLOAD의 `expectedIdentity` 전달 — 현재 `EXPECTED_IDENTITY_UNSUPPORTED`
- [ ] Smart-cache Desktop→Mobile 복호화 key sync provider
- [ ] AI token streaming과 응답 취소/부분 표시
- [ ] 실제 Rive `.riv` state machine; 현재 GIF/PNG motion 사용
- [ ] 개별 proposal item 부분 승인과 reject reason UX

### MVP 이후

- [ ] 100,000 entry, 다중 managed root, watcher overflow 장시간 soak test
- [ ] macOS filesystem adapter, signing/notarization
- [ ] iOS release
- [ ] Desktop updater signing
- [ ] wardrobe 실제 item/equipment 시스템

## 역할과 코드 소유권

### A — Desktop Agent와 로컬 파일 안전

- `apps/desktop/src-tauri/`
- `apps/desktop/src/features/files/`
- `apps/desktop/src/features/admin/`
- `tools/file-engine-cli/`
- `packages/rule-fixtures/`, file safety/transfer fixture

### B — Product, Cloud, Mobile, Character

- `apps/mobile/`
- `apps/server/`
- `apps/worker/`
- `infra/`
- `apps/desktop/src/features/character/`
- `packages/character-assets/`

공유 계약 경계는 `packages/contracts/`, `packages/database/`, `.env.example`, `docs/`, 루트 `README.md`입니다. breaking contract 변경은 schema, fixture, server, Desktop, Mobile을 함께 검토합니다.

## 문서

- 전체 계획: [MOUSEKEEPER_PLAN.md](MOUSEKEEPER_PLAN.md)
- 구현 계획과 남은 항목: [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)
- 구현 이력과 MVP 감사: [HISTORY.md](HISTORY.md)
- AI 구현 원칙: [AI_implement_rule.txt](AI_implement_rule.txt)
- API/계약: [docs/CONTRACTS.md](docs/CONTRACTS.md)
- 파일 안전 불변식: [docs/FILE_SAFETY_INVARIANTS.md](docs/FILE_SAFETY_INVARIANTS.md)
- FileTransfer threat model: [docs/FILE_TRANSFER_THREAT_MODEL.md](docs/FILE_TRANSFER_THREAT_MODEL.md)
- 서비스 threat model: [docs/threat-model.md](docs/threat-model.md)
- E2E 시나리오: [docs/e2e-scenarios.md](docs/e2e-scenarios.md)
- 복구 절차: [docs/RECOVERY_RUNBOOK.md](docs/RECOVERY_RUNBOOK.md)
- AWS EC2 배포: [docs/AWS_EC2_DEPLOYMENT.md](docs/AWS_EC2_DEPLOYMENT.md)
- file-engine CLI: [docs/FILE_ENGINE_CLI.md](docs/FILE_ENGINE_CLI.md)
- local-first ADR: [docs/adr/0001-local-first.md](docs/adr/0001-local-first.md)

## 완료 기준

기능을 완료로 표시하려면 다음 조건을 모두 만족해야 합니다.

- 실제 호출 경로가 연결되어 있어야 합니다.
- 공개 입력과 AI 출력은 schema/DTO 검증을 통과해야 합니다.
- 파일 쓰기는 managed-root 경계, approval, precheck, journal, no-overwrite를 지켜야 합니다.
- loading, empty, offline, error, `UNCONFIGURED` 상태가 구분되어야 합니다.
- 관련 typecheck/test/build가 통과해야 합니다.
- 외부 provider가 필요한 기능은 실제 credential과 실기기 검증 전까지 완료로 가장하지 않습니다.

로컬 파일 작업의 안전한 기준 흐름은 다음과 같습니다.

```text
propose → approve/reject → precheck → journal → execute → verify → history/undo
```
