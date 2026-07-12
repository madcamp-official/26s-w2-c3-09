# HOUSEMOUSE (26s-w2-c3-09)

> 폴더 룸메이트 MVP의 구체적인 아키텍처와 개발 순서는 [구현 계획](IMPLEMENTATION_PLAN.md)을 기준으로 합니다.
>
> 현재 통합 기준: `dev` / `3d43800` (`2026-07-12`), `origin/dev`와 동기화 완료

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
- [ ] LLM Wrapper
- [x] Cross-Platform

---

## 프로젝트 개요

HOUSEMOUSE는 사용자가 등록한 로컬 폴더를 데스크톱 에이전트가 안전하게 분석하고, 모바일에서 제안을 검토·승인한 뒤 파일 정리와 복구를 수행하는 local-first 크로스플랫폼 서비스다.

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
  ├─ BullMQ Worker: 알림, 재시도, object 만료·삭제
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

## `dev` 브랜치 구현 현황

### A — Desktop Agent

구현된 코드:

- managed root canonicalization, overlap·traversal·symlink/junction/reparse point 방어
- SQLite WAL 기반 managed root, file index, operation journal과 auto-approval 설정 저장
- scan, 검색, paginated browse, watcher debounce·reconcile·startup 복원
- Rule DSL, proposal, 사용자 decision, 실행 직전 precheck
- journal-before-write, no-overwrite move, 복구형 trash, create/rename, history, undo, journal recovery
- React 파일 관리 UI와 Tauri invoke bridge
- 실제 REST agent transport: pairing 코드 생성·polling, 15초 heartbeat, pending command 조회·상태 전이
- device token의 OS keychain 저장과 UI·로그 비노출, 잘못된 서버 응답 schema 검증
- device별 SQLite sync cursor와 `/v1/sync/events` REST replay
- Desktop Agent 연결·pairing·replay 상태 UI
- system tray, 사용자가 직접 설정하는 autostart, MSI/NSIS Windows bundle 구성
- overlay window/event bridge skeleton
- 파일 엔진용 OpenAPI 외부 schema 6개와 fixture

아직 연결되지 않은 범위:

- Socket.IO 알림 client와 durable sync outbox
- 서버 command → 로컬 proposal → 모바일 decision → execution 결과의 첫 P0 E2E
- README 생성/diff/write 로컬 적용
- FileTransfer source version 검증, chunk, SHA-256, 취소와 source-change 처리
- updater와 Windows release signing
- 실제 Rive overlay와 P1 usage event/cache candidate

서버 URL 또는 pairing이 없으면 agent transport는 fake online을 만들지 않고 명시적으로 `UNCONFIGURED`를 반환한다. pairing 뒤에도 heartbeat나 인증 요청이 성공하기 전에는 `offline`이며, device token은 React 계층으로 전달되지 않는다.

### B — Product & Cloud

- Firebase Android 및 Google 로그인, Firebase Admin token 검증 경로
- PostgreSQL/Drizzle 15개 migration과 Redis/Valkey local compose
- pairing, device, room, heartbeat/presence, command/proposal/decision/execution API
- Socket.IO `/realtime`, idempotency, audit, cursor replay
- Flutter home/room/rule/proposal/result/chat/files/smart-cache UI와 Drift cache/outbox
- P0 browse/transfer control plane과 P1 smart-cache quota/reservation/lifecycle control plane
- worker, Render blueprint, backup/restore 및 안전 문서

실제 S3-compatible storage, Rive asset, Android release keystore, Sentry와 운영 배포 secret은 아직 설정되지 않았다. 따라서 관련 기능은 `UNCONFIGURED` 또는 검증 대기 상태다.

### Phase별 판정

| Phase | 현재 판정 | 남은 완료 조건 |
|---|---|---|
| 0 계약·파일 안전 POC | 완료 | 146개 Rust test와 fixture E2E 통과 상태 유지 |
| 1 로그인·페어링·Presence | REST 경로 구현 | 실제 mobile claim E2E와 Socket.IO 알림 client |
| 2 관리 폴더·스캔·청결도 | 양쪽 코드 구현, 통합 전 | room 등록과 snapshot을 실제 서버로 연결 |
| 3 규칙·명령·제안 | 양쪽 코드 구현, 통합 전 | 서버 command를 Rust proposal로 변환해 왕복 |
| 4 실행·Undo·README·파일 전달 | 부분 구현 | README와 A FileTransfer, 실제 storage E2E |
| 5 캐릭터·채팅 | skeleton/metadata | Rive asset, 실제 overlay window, AI provider 선택 |
| 6 오프라인·재접속 | Desktop cursor/replay 구현 | durable outbox, Socket.IO 재접속과 reconnect E2E |
| 7 하드닝·배포 | Rust CI·tray·autostart·installer 구성 | updater, release signing, 운영 배포 |
| 8 P1 스마트 캐시 | B control plane만 선행 | P0 안정화 뒤 A usage scoring/upload/stale 처리 |

## 검증 기록 (`2026-07-12`)

| 검사 | 결과 |
|---|---|
| `pnpm check:contracts` | 서버 controller 58개와 OpenAPI 일치 |
| `pnpm typecheck` | Node/React workspace 전체 통과 |
| Desktop `tsc + vite build` | production web bundle 성공 |
| contracts test | 17개 통과 |
| server test | 실제 PostgreSQL 기준 23개 통과, 실제 object storage가 필요한 2개 suite 제외 |
| desktop-agent-simulator test | 1개 통과 |
| `flutter analyze` | 오류 0개 |
| Flutter test | 19개 통과 |
| Rust file-engine CLI | unit 92개 + integration 3개, 총 95개 통과 |
| Rust Desktop/Tauri core | 51개 통과 |
| Rust fixture E2E | proposal → precheck → execute → undo 통과 |
| Tauri feature check | `cargo check --features tauri-commands` 통과 |
| Windows release bundle | 실제 앱 feature를 포함한 MSI 6.82MB, NSIS 4.81MB 생성 성공 |

CI는 `dev` push를 검사하며 Windows에서 두 Rust crate의 format/test와 Tauri feature check를 실행한다.

## 다음 구현 순서

1. 실제 server와 mobile을 함께 켜서 Desktop pairing code claim → keychain 저장 → heartbeat E2E를 확인한다.
2. Socket.IO는 새 데이터 알림으로만 연결하고, 실패 시 현재 REST cursor replay로 복구한다. 전송 실패 event는 SQLite durable outbox에 적재한다.
3. 첫 P0 vertical slice(command → proposal → mobile approval → journaled execute → result → undo)를 한 managed root로 연결한다.
4. A의 FileTransfer validation/chunk/SHA-256/cancel/source-change를 B의 transfer session에 연결한다.
5. 실제 private S3-compatible bucket에서 PUT/HEAD/GET/delete/TTL lifecycle E2E를 수행한다.
6. updater와 Windows release signing을 마무리한다.
7. Rive·FCM·Sentry를 마무리한다. P1 스마트 캐시는 두 P0 slice가 안정화된 뒤 진행한다.

## 실행 방법

필수 환경 변수는 [.env.example](.env.example)을 기준으로 현재 shell 또는 secret manager에 주입한다. 실제 secret과 서비스 계정 원문은 Git에 넣지 않는다.

```powershell
# Node workspace
pnpm install --frozen-lockfile

# PostgreSQL + Redis/Valkey
docker compose up -d
pnpm --filter @housemouse/database db:migrate

# Server
pnpm --filter @housemouse/server start:dev

# Desktop: Rust stable/MSVC toolchain 필요
$env:HOUSEMOUSE_SERVER_BASE_URL = "http://127.0.0.1:3000"
pnpm --filter @housemouse/desktop tauri:dev

# Windows MSI/NSIS 생성
pnpm --filter @housemouse/desktop tauri:build
```

Android 실기기에서 USB로 로컬 서버를 사용할 때:

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe" reverse tcp:3000 tcp:3000

Set-Location apps/mobile
flutter run `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=HOUSEMOUSE_API_URL=http://127.0.0.1:3000 `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<Google-Web-OAuth-Client-ID>
```

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

- CLI usage and JSON contracts: [`tools/file-engine-cli/README.md`](tools/file-engine-cli/README.md)
- Desktop/Tauri integration plan: [`apps/desktop/src-tauri/INTEGRATION.md`](apps/desktop/src-tauri/INTEGRATION.md)

Current safe flow:

```text
propose -> decision.jsonl -> precheck -> execute -> undo
```
