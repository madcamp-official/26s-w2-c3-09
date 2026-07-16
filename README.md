# MouseKeeper

자연어로 파일을 찾고, 정리하고, 승인하고, 되돌릴 수 있는 로컬 우선 파일 관리 에이전트입니다. Windows 데스크톱 에이전트가 실제 파일 시스템을 안전하게 다루고, Android 모바일 앱은 같은 서버 room의 채팅·제안·승인·실행 상태를 확인합니다.

## 1. 프로젝트 개요

### 목적

파일 관리 명령을 별도의 복잡한 UI 대신 자연어로 요청할 수 있게 하고, 파일을 변경하는 작업은 반드시 사용자의 승인과 데스크톱의 최종 안전 검증을 거치도록 합니다.

### 핵심 가치

- 조회는 빠르게 자동 수행하고, 변경은 승인 후 수행합니다.
- AI는 파일 API를 직접 호출하지 않고 검증된 도구·DTO를 통해서만 작업합니다.
- 파일 변경 전 경로 경계, 충돌, 원본 상태, journal을 다시 확인합니다.
- 서버가 끊겨도 데스크톱의 로컬 파일 관리와 모바일의 캐시/outbox를 보존합니다.
- 설정되지 않은 provider는 성공으로 가장하지 않고 `UNCONFIGURED`로 표시합니다.

## 2. 주요 기능

### Desktop Agent

- 관리 폴더(managed root) 등록·해제·상태 관리
- canonical path, `..`, symlink, junction, Windows reparse point 탈출 차단
- 파일 목록·검색·stat·watcher·reconcile·SQLite 인덱스
- 자연어 요청을 command/rule/proposal로 변환
- `MOVE`, `RENAME`, `TRASH`, `CREATE`, `ORGANIZE`, README 작업
- no-overwrite, precondition, journal-before-write, crash recovery, undo
- 서버 room 연결, pairing, heartbeat, WebSocket/replay 동기화
- 파일 browse, transfer, checksum, source-change 검증
- Windows tray, autostart, overlay, 말풍선·캐릭터 상호작용

### Mobile App

- Google/Firebase 인증 및 Desktop pairing
- 관리 폴더·room 목록과 파일 조회
- Desktop과 동일한 채팅 session/message 표시
- proposal 승인·거절, execution 결과, history 조회
- Socket.IO 실시간 이벤트와 REST cursor replay를 이용한 자동 동기화
- 오프라인 cache와 mutation outbox
- 치즈 급여 및 GIF 캐릭터 애니메이션
- 10스테이지 턴제 치즈 퍼즐 미니게임
  - 방향 이동과 턴 제한
  - 상자 밀기
  - 키를 얻어 문 열기
  - 치즈 도착 시 클리어
  - 7~10 스테이지 지정 경로

### Server / Worker

- NestJS API, PostgreSQL, Valkey/Redis, Socket.IO
- pairing·device·room·chat·command·proposal·decision·execution 상태 관리
- durable sync event와 `/v1/sync/events` replay
- OpenAPI route coverage 및 DTO/Zod 검증
- AI structured output 검증과 실패 시 fail-closed 처리
- Worker의 FCM·transfer/cache object lifecycle 처리

## 3. 사용 흐름

```text
Desktop 실행
  -> 서버 URL 확인 및 pairing code 생성
Mobile에서 code claim
  -> Desktop device 연결
Desktop에서 managed root 등록
  -> 서버 room 생성/연결
모바일 또는 데스크톱에서 자연어 요청
  -> 조회 도구 자동 실행 또는 변경 proposal 생성
사용자 승인
  -> Desktop precondition 검증
  -> journal 기록
  -> 파일 변경 및 execution 결과 동기화
```

관리 폴더는 로컬 등록과 서버 room 연결이 별도 상태입니다. `UNBOUND`면 로컬 파일 관리는 가능하지만 room 채팅은 사용할 수 없습니다. 모바일에서 PC pairing을 해제하면 모바일 권한이 취소되어 Desktop이 offline으로 보일 수 있습니다.

## 4. 화면 및 IA

### Desktop

- 관리 콘솔: managed root, 파일 목록, 검색, proposal, history
- PC 연결: 서버 URL, device ID, pairing, heartbeat 상태
- 채팅 overlay: 자연어 요청, 진행 상태, 승인 카드, 결과
- 캐릭터/house overlay: idle, working, angry reaction, 말풍선

### Mobile

- Pairing Gate: Desktop 연결과 오류 상태
- Home: room·관리 폴더·캐릭터·하단 navigation
- Chat: session 목록, message, proposal, decision, execution
- Files: browse, search, 다운로드 상태
- Mini Games: 치즈 퍼즐, 다른 픽셀 미니게임
- Settings: 연결·문서 분석 동의·권한 상태

## 5. 기술 스택

| 영역 | 기술 |
|---|---|
| Desktop UI | Tauri 2, React, TypeScript, Vite |
| Desktop engine | Rust, Tokio, SQLite WAL, filesystem watcher |
| Mobile | Flutter, Dart, Riverpod, GoRouter, Drift |
| Server | NestJS, Fastify, Socket.IO, Zod |
| Database | PostgreSQL, Drizzle ORM |
| Realtime/cache | Valkey/Redis, Socket.IO Redis adapter |
| AI | OpenAI Responses API, strict structured output |
| Infra | AWS EC2, Nginx/TLS, systemd, private object storage |
| Contract | OpenAPI, TypeScript schema, JSON Schema |

## 6. 저장소 구조

```text
apps/desktop/        Tauri/React UI와 Rust 파일 엔진
apps/mobile/         Flutter Android 앱
apps/server/         NestJS API와 WebSocket control plane
apps/worker/         FCM·object lifecycle worker
packages/contracts/  OpenAPI·DTO·event 계약
packages/database/   Drizzle schema와 migration
packages/character-assets/
                     공용 캐릭터·house asset
infra/aws/           EC2·Nginx·systemd·backup 설정
scripts/             계약 검증·배포 preflight·백업 스크립트
docs/                ADR·배포·복구·E2E 문서
```

## 7. 로컬 실행

### 공통

```powershell
corepack enable
pnpm install
pnpm check:contracts
```

### Desktop

```powershell
$env:MOUSEKEEPER_SERVER_BASE_URL = "https://mousekeeper.madcamp-kaist.org"
pnpm --filter @mousekeeper/desktop tauri:dev
```

### Mobile

```powershell
Set-Location apps/mobile
flutter pub get
flutter run `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=MOUSEKEEPER_API_URL=https://mousekeeper.madcamp-kaist.org `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<Google-Web-OAuth-Client-ID>
```

휴대폰 USB 설치 시 `adb devices`가 `device` 상태여야 합니다. `unauthorized`이면 휴대폰에서 USB 디버깅 인증을 허용해야 합니다.

## 8. 테스트와 빌드

```powershell
pnpm check:contracts
pnpm --filter @mousekeeper/server typecheck
pnpm --filter @mousekeeper/server test -- --runInBand
pnpm --filter @mousekeeper/server build

Set-Location apps/mobile
flutter analyze
flutter test
flutter build apk --debug `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=MOUSEKEEPER_API_URL=https://mousekeeper.madcamp-kaist.org `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<Google-Web-OAuth-Client-ID>

Set-Location ../..
pnpm --filter @mousekeeper/desktop tauri:build
```

계약 검증 기준은 `OpenAPI route coverage OK: 88 controller methods.`입니다.

## 9. 운영 배포

운영 서버 주소는 `https://mousekeeper.madcamp-kaist.org`입니다. EC2 SSH host alias는 `mad_camp_week2`입니다.

```powershell
ssh mad_camp_week2
sudo -u mousekeeper bash -lc 'cd /opt/mousekeeper && git fetch origin main && git merge --ff-only origin/main'
sudo -u mousekeeper bash -lc 'cd /opt/mousekeeper && NODE_OPTIONS=--max-old-space-size=1024 pnpm --filter @mousekeeper/server build'
sudo systemctl restart mousekeeper-server
sudo systemctl is-active mousekeeper-server
sudo systemctl is-active mousekeeper-worker
```

검증:

```powershell
Invoke-WebRequest https://mousekeeper.madcamp-kaist.org/health
Invoke-WebRequest https://mousekeeper.madcamp-kaist.org/ready
```

Object storage 설정이 없는 경우 worker는 `UNCONFIGURED`로 유지해야 하며, 성공을 가장하면 안 됩니다. 상세 절차는 [AWS EC2 배포 문서](docs/AWS_EC2_DEPLOYMENT.md)와 [복구 runbook](docs/RECOVERY_RUNBOOK.md)을 참고합니다.

## 10. 설치 파일

현재 배포 파일은 다음 위치에 생성됩니다.

```text
apps/mobile/build/app/outputs/flutter-apk/app-debug.apk
apps/desktop/src-tauri/target/release/bundle/msi/*.msi
apps/desktop/src-tauri/target/release/bundle/nsis/*.exe
```

배포용 파일은 `C:\Users\yoons\Downloads\MouseKeeper-release-20260716`에 모아둘 수 있습니다.

## 11. 트러블슈팅

### `UNCONFIGURED: desktop device pairing is required`

Mobile에서 Desktop pairing이 완료되지 않은 상태입니다. Desktop에서 pairing code를 생성하고 Mobile에서 claim해야 합니다.

### `Downloads에 메시지 입력`이 비활성화됨

관리 폴더는 등록됐지만 서버 room과 연결되지 않은 `UNBOUND` 상태입니다. Desktop에서 해당 폴더의 room 연결을 다시 수행해야 합니다.

### Desktop이 `offline`으로 보임

Desktop agent가 일시정지됐거나 heartbeat가 끊긴 상태일 수 있습니다. Desktop의 `실행`을 눌러 agent를 재개하고, 서버 `/health`와 `/ready`를 확인합니다.

### `502 Bad Gateway`

Nginx는 살아 있지만 upstream API가 실행되지 않는 상태입니다. 다음을 확인합니다.

```bash
sudo systemctl status mousekeeper-server --no-pager -l
sudo journalctl -u mousekeeper-server -n 100 --no-pager
test -f /opt/mousekeeper/apps/server/dist/main.js
```

`dist/main.js`가 없으면 EC2에서 Node heap을 늘려 서버를 빌드한 뒤 재시작합니다.

### 모바일 채팅이 나갔다 들어와야 동기화됨

Socket.IO 이벤트는 지연 최적화이고, 최종 정합성은 REST replay가 담당합니다. 앱 lifecycle 복귀·socket reconnect 시 `/v1/sync/events?after=<cursor>`가 실행되는지 확인합니다.

## 12. 안전 정책과 제한

- AI가 절대 경로, `..`, 미조회 후보, 잘못된 DTO를 출력하면 서버와 Desktop에서 거부합니다.
- MOVE/TRASH/rule/transfer/undo는 사용자 승인 없이 실행하지 않습니다.
- 원본 문서 전체를 서버 DB에 저장하지 않습니다. 문서 분석은 관리 루트별 동의가 필요합니다.
- OCR, 미설정 upload provider, 미설정 object storage는 지원하지 않거나 `UNCONFIGURED`로 표시합니다.
- 서버·모바일·Desktop의 원문 로그에는 비밀키와 개인정보를 남기지 않습니다.

## 13. 현재 기준 커밋

- `8fe966b` 서버 AI provider 테스트 타입 오류 수정
- `ad999a0` room document consent sync event 계약 수정
- `5d801da` 자연어 organize selector 적용
- `dcb55f5` 치즈 애니메이션 복원 및 idle 크기 일치
- `17f6867` 10스테이지 퍼즐 맵·키·문 기믹
- `534d033` Desktop 말풍선 위치 조정
- `95f01ad` `origin/fix/minor-bugs` 병합

## 14. 라이선스 및 보안

이 저장소는 프로젝트 제출 및 운영 목적의 private repository입니다. API key, Firebase service account, OAuth secret, SSH private key는 커밋하지 않습니다. 환경 변수 예시는 `.env.example`에 key 이름만 기록하고 실제 값은 로컬 shell 또는 EC2의 보호된 환경 파일에 주입합니다.
