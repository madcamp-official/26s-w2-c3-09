# B 실행 계획 — Product & Cloud 전체 잔여 작업

## 1. 목적과 완료 기준

현재 B 영역의 서버·모바일·worker 제어 로직은 대부분 구현되어 있다. 이 문서는 기존 기능을 다시 만드는 계획이 아니라, 현재 코드를 실제 공급자 및 A의 Desktop Agent와 연결하여 P0와 P1을 검증·배포 가능한 상태로 만드는 실행 문서다.

최종 완료 기준은 다음과 같다.

- Android ↔ Server ↔ Desktop P0 핵심 흐름을 실제 환경에서 시연한다.
- Firebase 인증·FCM, private object storage, AWS EC2 배포를 실제 설정으로 검증한다.
- P0 파일 탐색·다운로드·checksum·TTL 삭제를 완료한다.
- P1 스마트 캐시의 opt-in·quota·암호화·freshness·삭제 보장을 완료한다.
- signed Android AAB와 반복 가능한 E2E·발표 문서를 완성한다.
- fake provider, dummy data, 성공을 가장하는 fallback을 추가하지 않는다.

## 2. 실행 원칙과 우선순위

- B는 모바일·서버·worker·캐릭터 presentation·배포를 소유한다.
- A의 Desktop Agent와 로컬 파일 안전 구현은 B가 대신 수정하지 않는다.
- 계약 변경은 additive 방식으로 진행하고, 삭제나 rename으로 기존 호환성을 깨지 않는다.
- 외부 provider가 없으면 성공을 가장하지 않고 `UNCONFIGURED` 또는 동등한 명시적 오류를 노출한다.
- 파일 쓰기와 스마트 캐시 활성화는 사용자 승인·동의를 우회하지 않는다.
- P1 작업이 P0 제출을 지연시키지 않도록 P0 release candidate를 먼저 고정한다.

실행 순서는 다음과 같이 고정한다.

```text
Firebase
→ 실시간 복구
→ A command 통합
→ P0 파일 전송
→ 배포·Android release
→ 캐릭터·FCM
→ P1 스마트 캐시
→ 최종 E2E·제출 문서
```

각 단계는 다음 크기의 독립 작업으로 나눈다.

1. 계약 또는 환경 사전조건
2. 서버·worker 기능
3. 모바일 UI와 상태 처리
4. 단위·통합 테스트
5. 실제 provider 또는 A 연동 E2E
6. 문서와 증거 자료

## 3. 단계별 작업

### 0단계 — 현재 구현 기준선 고정

#### 작업

- `B_PROCESS.md`의 완료 판정을 서버·모바일·worker 코드 및 테스트와 다시 대조한다.
- 다음 품질 게이트를 실행한다.
  - `pnpm check:contracts`
  - `pnpm typecheck`
  - `pnpm test`
  - `pnpm build`
  - `flutter analyze`
  - `flutter test`
- 실패 항목을 `코드 결함`, `외부 설정 누락`, `A 연동 대기`로 분류한다.
- 현재 OpenAPI route, DB migration, 모바일 상태 처리, worker lifecycle을 기준선으로 기록한다.
- 잔여 작업마다 담당자, 선행 조건, 실행 명령, 검증 방법을 기록한다.

#### 완료 조건

- 로컬에서 실행 가능한 품질 게이트가 모두 통과한다.
- 외부 설정이나 A 산출물이 필요한 실패는 코드 완료로 오인하지 않고 별도 차단 항목으로 기록한다.
- 기존 공개 계약을 깨는 변경이 없다.

### 1단계 — Firebase 인증과 기기 등록

#### 1.1 Android Firebase 연결

- 실제 `google-services.json`의 Android package가 `com.housemouse.app`과 일치하는지 확인한다.
- Gradle Google Services plugin과 Firebase 초기화를 확인한다.
- debug와 release SHA fingerprint를 Firebase 프로젝트에 등록한다.
- 설정 파일이 없거나 잘못된 경우 앱이 명확한 설정 오류를 표시하도록 확인한다.

#### 1.2 서버 Firebase Admin 연결

- `FIREBASE_PROJECT_ID`, `FIREBASE_CLIENT_EMAIL`, `FIREBASE_PRIVATE_KEY`를 환경변수로 주입한다.
- private key 개행 변환을 검증하고 key가 로그에 출력되지 않도록 한다.
- 필수 변수가 누락되면 서버가 시작 단계에서 fail-fast하는 기존 동작을 유지한다.

#### 1.3 로그인과 세션

- 실제 Google 로그인으로 Firebase ID token을 발급받는다.
- 서버가 ID token을 검증하여 내부 principal로 변환하는 흐름을 확인한다.
- 계정별 Drift 데이터 격리를 검증한다.
- 로그아웃 시 token, 사용자별 로컬 cache, socket 연결을 정리한다.
- 인증 만료, 네트워크 단절, 잘못된 Firebase project를 서로 다른 오류로 표시한다.

#### 1.4 페어링과 revoke

- 실제 Android와 Desktop 사이 nonce/HMAC pairing을 검증한다.
- pairing 완료 시 정확한 device JWT가 발급되는지 확인한다.
- revoke 이후 REST, Socket.IO, command 수신이 모두 차단되는지 확인한다.
- 다른 사용자 또는 다른 room에 대한 접근 거절 테스트를 추가한다.

#### 완료 조건

- 실제 Android에서 로그인, 페어링, 로그아웃, 재로그인, revoke를 반복할 수 있다.
- revoke된 device는 재인증 전까지 명령과 event를 받지 못한다.

### 2단계 — 실시간 연결과 오프라인 복구

#### 2.1 Socket.IO 연결

- 인증 handshake를 검증한다.
- 사용자·device·room 단위로 올바른 socket room에 join한다.
- 재연결 backoff와 중복 listener 방지를 검증한다.
- 앱 foreground 복귀 시 socket 및 최신 상태를 재동기화한다.

#### 2.2 Presence

- Desktop heartbeat가 Redis TTL을 갱신하는지 확인한다.
- TTL 만료에 따른 online/offline 전환을 검증한다.
- stale connection과 비정상 종료된 device 상태를 정리한다.

#### 2.3 Event replay

- 모바일이 마지막 처리 sequence cursor를 저장한다.
- socket event 유실 뒤 REST replay로 상태를 복구한다.
- 같은 event가 socket과 replay로 중복 도착해도 한 번만 반영한다.
- 여러 room의 event가 서로 섞이지 않도록 검증한다.
- 서버 재시작 뒤에도 queue와 replay가 복구되는지 확인한다.

#### 2.4 모바일 mutation outbox

- offline command를 로컬 outbox에 영속 저장한다.
- 재접속 뒤 생성 순서대로 전송한다.
- 서버 terminal 응답 전에는 성공 UI를 표시하지 않는다.
- 재시도 가능한 실패, idempotency conflict, 영구 실패를 구분한다.
- duplicate approval과 decision이 멱등 처리되는지 확인한다.

#### 완료 조건

- 네트워크와 서버 연결을 강제로 끊고 복원해도 command와 실행 상태가 유실되거나 중복되지 않는다.

### 3단계 — A Desktop Agent와 P0 command vertical slice

#### 3.1 계약 정합성

- Command, Proposal, Decision, ExecutionResult payload를 A 구현과 비교한다.
- 필요한 필드는 additive contract 변경으로 분리한다.
- schema, fixture, 서버 validation, 모바일 parsing, OpenAPI를 같은 작업 단위로 갱신한다.
- DB 내부 필드, secret, 절대 경로가 공개 응답에 노출되지 않도록 확인한다.

#### 3.2 Command 전달

- 모바일에서 command를 생성한다.
- 서버가 PostgreSQL에 `QUEUED` 상태로 영속 저장한다.
- Desktop이 실시간 event 또는 REST poll/replay로 command를 수신한다.
- PC offline 상태에서도 command가 유실되지 않도록 확인한다.

#### 3.3 Proposal과 승인

- Desktop의 실제 scan 결과로 생성된 proposal을 서버에 저장한다.
- 모바일에 proposal summary와 diff를 표시한다.
- MVP에서는 전체 승인 또는 전체 거절을 지원한다.
- 동일 승인 중복 요청을 멱등 처리한다.
- 한 room의 proposal이 다른 room에 노출되지 않도록 통합 테스트한다.

#### 3.4 실행 결과와 Undo

- A의 precondition 재검증·journal·작업 결과를 수신한다.
- 모바일은 실제 terminal event 이후에만 성공을 표시한다.
- `STALE`, `CONFLICT`, 권한 오류, offline 상태를 구분한다.
- undo 요청과 실제 결과를 표시한다.
- README draft와 diff는 Desktop이 생성한 실제 값만 표시한다.

#### 완료 조건

- 모바일 command → Desktop proposal → 모바일 승인 → 안전 실행 → 결과 표시 → undo 흐름이 실제 file fixture에서 성공한다.

### 4단계 — P0 온라인 파일 탐색과 다운로드

#### 4.1 Private object storage

- endpoint, region, bucket, access key를 환경변수로 주입한다.
- bucket private access, encryption, CORS, lifecycle 정책을 적용한다.
- 저장소가 미설정이면 `UNCONFIGURED`를 유지한다.
- provider 오류를 성공으로 변환하는 fallback을 만들지 않는다.

#### 4.2 파일 탐색

- 동일 사용자·room·device 권한을 검증한다.
- Desktop이 요청 시점마다 managed root와 경로를 재검증한 결과만 반환한다.
- cursor pagination을 검증한다.
- `DEVICE_OFFLINE`, `TIMED_OUT`, `CURSOR_INVALIDATED`를 구분한다.
- pagination 실패 시 기존 page를 유지하되 최신 전체 목록처럼 표시하지 않는다.

#### 4.3 전송 session

- Desktop이 source identity, version, 크기를 다시 검증한다.
- 서버가 짧은 TTL의 signed upload target을 발급한다.
- `SOURCE_CHANGED`, `OUTSIDE_MANAGED_ROOT`, `SIZE_LIMIT_EXCEEDED`를 구분한다.
- 취소와 partial upload에 deletion tombstone을 생성한다.

#### 4.4 모바일 다운로드

- signed download target으로 `.part` 파일에 저장한다.
- SHA-256 검증 성공 후에만 충돌 없는 최종 이름으로 이동한다.
- checksum 실패를 다운로드 성공으로 표시하지 않는다.
- 최종 파일 저장 뒤 서버에 ACK한다.

#### 4.5 Object lifecycle

- ACK, 취소, 실패, TTL 만료 object를 worker가 삭제한다.
- worker 재시작 뒤 미완료 deletion job을 재처리한다.
- provider-side orphan lifecycle 정책을 실제 bucket에서 확인한다.

#### 추가 범위

- 발표 범위는 20MB 이하 단일 파일 전송으로 고정한다.
- 대용량 resume와 다양한 파일 미리보기는 P0 release candidate 이후로 미룬다.

#### 완료 조건

- 실제 PC managed root의 파일 하나를 Android로 내려받고 checksum을 검증한다.
- ACK 또는 TTL 뒤 임시 object가 실제 storage에서 삭제된다.

### 5단계 — 캐릭터와 알림 제품 경험

#### 5.1 캐릭터 asset

- 실제 `.riv` 파일, artboard, state machine, input 이름을 확정한다.
- asset이 없으면 가짜 animation을 만들지 않고 미설정 상태를 표시한다.
- Desktop overlay native shell은 A가 제공하고 B는 presentation layer만 연결한다.

#### 5.2 Domain event 연결

- proposal 생성, 승인, 실행 성공·실패, offline event에서 캐릭터 상태를 파생한다.
- replay된 과거 event가 animation과 알림을 반복하지 않도록 한다.
- 캐릭터 상태가 파일 권한이나 실행 결과에 영향을 주지 않도록 유지한다.

#### 5.3 성장과 테마

- 기존 affinity ledger와 실제 UI를 연결한다.
- 잠긴 외형 선택을 `FEATURE_LOCKED`로 거절한다.
- animation off 설정을 유지한다.
- 일정이 부족하면 캐릭터 1종과 테마 1종만 유지한다.

#### 5.4 알림

- foreground Socket.IO SnackBar를 실제 event로 검증한다.
- FCM token 등록·갱신·삭제를 구현·검증한다.
- background와 terminated 상태에서 proposal 및 terminal execution 알림을 확인한다.
- 로그아웃과 revoke 뒤 알림 전송을 차단한다.

#### 완료 조건

- 실제 domain event로 캐릭터와 알림이 반응한다.
- replay event는 과거 사용자 알림을 반복하지 않는다.

### 6단계 — P1 스마트 캐시 실제 object 흐름

#### 6.1 Opt-in policy

- 사용자가 명시적 확인 checkbox를 선택해야 policy를 활성화한다.
- 전체 폴더 동기화가 아니라 승인된 일부 원본이 저장됨을 설명한다.
- room quota, 최대 파일 크기, 제외 pattern, 삭제 정책을 표시한다.
- 기본값 `SMART_CACHE_ENABLED=false`를 유지한다.

#### 6.2 후보와 quota

- Desktop usage candidate를 수신한다.
- 제외 pattern과 파일 크기 정책을 심사한다.
- `AVAILABLE + 유효 RESERVED` 합계에 room advisory lock을 적용한다.
- 짧은 reservation을 발급하고 만료 시 quota를 해제한다.
- 동시 reservation이 room quota를 초과하지 않도록 통합 테스트한다.

#### 6.3 암호화 업로드

- 서버가 승인한 UploadTarget 이후에만 업로드를 시작한다.
- 실제 파일 내용을 client-side에서 암호화한다.
- key material을 source, 로그, 서버 metadata에 평문 저장하지 않는다.
- object HEAD, 크기, SHA-256, version 검증 후에만 `AVAILABLE`로 전환한다.
- provider 장애는 retry 가능한 실패로 유지하고 성공 처리하지 않는다.

#### 6.4 Offline UX와 freshness

- `AVAILABLE`과 freshness를 별도 상태로 표시한다.
- `last_verified_at`을 표시한다.
- PC offline이면 `UNVERIFIED_OFFLINE`으로 표시한다.
- 원본 변경 감지 시 `STALE`로 표시한다.
- 같은 room에 `QUEUED` command가 있으면 목록 변경 가능성을 경고한다.
- quota 초과 시 LRU eviction을 수행하고 삭제 완료 여부를 추적한다.

#### 6.5 삭제 보장

- policy disable, room 삭제, device revoke 시 deletion tombstone을 생성한다.
- 진행 중 reservation을 취소하고 AVAILABLE object를 삭제한다.
- worker retry 후 실제 object가 삭제되었는지 운영 환경에서 확인한다.

#### 완료 조건

- opt-in 이후 서버가 승인한 파일만 암호화 업로드된다.
- PC offline에서 cache 다운로드가 가능하며 freshness가 정확히 표시된다.
- disable 또는 revoke 뒤 관련 object가 실제 storage에서 삭제된다.

### 7단계 — 배포·Release·관측

#### 7.1 AWS EC2 배포

- PostgreSQL, Redis/Valkey, server, worker를 EC2 또는 같은 VPC의 관리형 서비스에 배포한다.
- Nginx가 loopback의 API를 proxy하고 TLS 인증서로 443을 제공하도록 구성한다.
- migration은 배포 전 단일 실행으로 적용한다.
- readiness가 DB와 Redis 의존성을 실제 확인하도록 한다.
- production CORS와 WebSocket origin을 허용된 origin으로 제한한다.

#### 7.2 Secret 관리

- Firebase, object storage, JWT/device token secret, Sentry 값을 운영 secret으로 등록한다.
- 저장소, 빌드 로그, crash report에 secret이 남지 않는지 점검한다.
- `.env.example`에는 key만 유지하고 값이나 placeholder secret을 넣지 않는다.

#### 7.3 Android release

- 실제 keystore path, alias, password를 환경변수로 주입한다.
- signed APK와 AAB를 생성한다.
- release SHA fingerprint를 Firebase에 등록한다.
- 실제 Android 기기에서 설치, 업데이트, cold start를 검증한다.

#### 7.4 관측과 복구

- Sentry crash 수집을 실제 project에서 확인한다.
- correlation ID 기반 구조화 로그를 확인한다.
- 절대 경로, 파일 내용, token, secret을 redaction한다.
- PostgreSQL backup과 restore runbook을 실제 환경에서 실행한다.

#### 완료 조건

- production URL과 signed Android 산출물로 P0 핵심 E2E를 실행할 수 있다.
- rollback과 데이터 복구 절차가 반복 가능하게 기록되어 있다.

### 8단계 — 최종 E2E와 제출 문서

#### 필수 E2E

1. 로그인·페어링·revoke
2. 온라인 command 전체 흐름
3. offline command와 재접속
4. 중복 승인
5. 승인 후 source 변경 → `STALE`
6. 서버 재시작 후 replay
7. 사용자·room 간 권한 격리
8. P0 파일 browse·download·checksum·ACK/TTL 삭제
9. source change·size limit·checksum mismatch
10. P1 opt-in·quota·reservation·암호화 upload
11. P1 offline cache·`STALE`·`UNVERIFIED_OFFLINE`
12. P1 disable·room 삭제·device revoke object 삭제

#### 문서와 발표 자료

- 루트 README에 주제, 역할, 일정, 구조도, API, 실행법, 기술 스택을 작성한다.
- Firebase, object storage, AWS EC2, Android release 설정 절차를 작성한다.
- 정상 흐름과 주요 실패 흐름의 데모 영상 또는 캡처를 준비한다.
- B 담당 구현과 A 연동 경계를 명시한다.
- 외부 설정이 없어 검증하지 못한 항목은 `UNCONFIGURED` 또는 차단 상태로 정확히 기록한다.

#### 완료 조건

- 새 개발자가 README와 이 문서만 보고 환경 구성과 데모를 재현할 수 있다.
- 모든 출시 차단 조건을 확인하고 P0 release candidate를 고정한다.

## 4. 공개 계약과 인터페이스 원칙

- 기존 OpenAPI route와 event schema의 하위 호환성을 유지한다.
- A 연동에 필요한 필드는 삭제·rename하지 않고 additive 방식으로 추가한다.
- 계약 변경 시 schema, fixture, 서버 validation, 모바일 parsing, OpenAPI를 함께 갱신한다.
- 공개 응답에서 DB 내부 필드, idempotency key, secret, device public key를 제거한다.
- 외부 provider가 없으면 명확한 `UNCONFIGURED` 오류를 반환한다.
- 파일 쓰기와 캐시 활성화는 사용자 승인·동의를 우회하지 않는다.

## 5. 외부 선행조건과 차단 항목

다음 항목은 저장소 내부 코드만으로 완료할 수 없다.

1. Firebase Android/iOS native config와 server service account
2. 실제 Android 기기에서 사용할 FCM project 설정
3. A의 Desktop Agent, overlay native shell, command/file-transfer adapter
4. 실제 Rive asset과 artboard/state machine/input 명세
5. Private S3-compatible endpoint, bucket, credentials, encryption/lifecycle 정책
6. Android release keystore path, alias, password
7. AWS EC2 운영 권한, IAM role과 production secret
8. Sentry project와 DSN

이 값이 없을 때는 fake provider나 debug signing fallback을 추가하지 않는다. 해당 기능은 차단 상태로 유지하고 필요한 입력과 검증하지 못한 범위를 문서에 기록한다.

2026-07-13 기준 5번의 private AWS S3 bucket·EC2 IAM role object 접근과 7번의 AWS EC2 API/worker 배포는 제공·검증됐다. S3 bucket lifecycle 정책과 실제 FileTransfer TTL 삭제 E2E는 아직 별도 검증 대상이다.

## 6. 기본 시연 범위

- 플랫폼: Android + Windows Desktop
- 사용자/공간: 한 사용자, 한 room을 발표 기본값으로 사용하되 데이터 모델은 여러 room을 유지한다.
- 파일: managed root 안의 20MB 이하 단일 파일
- P0 필수: command 승인·실행·undo, offline queue, 온라인 파일 가져오기, checksum, TTL 삭제
- P1 추가: 명시적 opt-in, quota reservation, 암호화 cache, freshness, revoke/disable 삭제
- P0 release candidate를 먼저 완료한 뒤 P1을 통합한다.
