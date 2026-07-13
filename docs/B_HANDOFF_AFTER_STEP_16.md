# B Task List — After A Step 16 (2026-07-13, Step 16-3까지 갱신)

A는 로컬 19단계 계획(`7/12_plan.md`) 기준 **Step 16-3(스마트 캐시 candidate 제출·reservation·upload 자동화)까지 완료**했다. 이 문서는 B/모바일 코드베이스를 실제로 감사한 결과를 바탕으로, **이미 끝난 것과 진짜 남은 것**을 구분한 태스크 리스트다. 이미 구현된 항목을 다시 만들지 않도록 파일 경로까지 명시한다.

> **먼저 읽을 것**: A가 이번 단계에서 B 소유 서버 파일 2개를 직접 수정했다. 아래 "0-1. A가 B 파일을 직접 수정함" 항목을 반드시 확인할 것.

---

## 0. 이미 끝난 것 — 다시 만들지 말 것

아래 7개 영역은 코드 감사 결과 **구현 완료로 확인됨**. 재작업 금지, 필요하면 스팟체크만.

| 영역 | 근거 |
|---|---|
| **Command/Proposal/Decision/Execution railway** | 상태 전이(`apps/server/src/commands/command-state.ts`), submit-proposal/decision/execution 전부 idempotency-key replay-safe (`proposals.service.ts`, `decisions.service.ts`, `executions.service.ts`) |
| **File Browse API** | create/pending/get/complete/fail, cursor pagination, DEVICE_OFFLINE/TIMED_OUT/CURSOR_INVALIDATED 처리 (`file-browse.controller.ts`, `file-browse.service.ts`). 모바일 breadcrumb/list/loading/empty/error/download 버튼 (`files_page.dart`) |
| **File Transfer / Object Lifecycle** | REQUESTED→UPLOADING→READY→FAILED/EXPIRED/CANCELLED 전체 구현, **complete-upload idempotent 확인됨**(같은 transferId+key 재호출 시 같은 READY 반환), sha256/size 검증, TTL cleanup job (`transfers.service.ts`). 모바일 checksum 검증 다운로드 (`verified_download.dart`) |
| **Pairing/Device/Heartbeat/Revoke** | 코드 기반 pairing, JWT device token, heartbeat+Redis presence. **revoke가 실제로 토큰을 막음**: `devices.controller.ts`가 REVOKED 처리+cache/transfer cleanup 연쇄, `auth.service.ts`의 `authenticateDevice`가 `status='ACTIVE'` 필터링으로 revoke된 토큰 즉시 거부 |
| **Offline Smart Cache 서버/모바일 control plane** | opt-in policy, candidate 제출/reservation/idempotent complete-upload/cancel API (`smart-cache.controller.ts`), cache-disable·device-revoke 시 삭제 job 큐잉 (`devices.controller.ts`), 모바일 freshness label UI (`smart_cache_page.dart`) |
| **Server Replay/WebSocket/Recovery** | 모든 서비스가 DB 트랜잭션 안에서 `sync.append` 호출(소켓 끊겨도 유실 없음), REST replay(`GET /v1/sync/events`), 모바일 cursor 기반 복구 (`realtime_controller.dart`) |
| **Mobile Command UX (버튼 기반)** | command 생성 FAB, README 전용 폼(`readme_command_page.dart`), 진행 상태 표시, desktop online/offline/degraded 표시, "root 미등록" 안내(`EmptyRoomsCard`) — 자연어 입력만 빠짐 |
| **Smart Cache candidate 제출→reservation→upload 왕복 (Step 16-2/16-3)** | 로컬 usage scoring → `POST /v1/agent/cache-candidates` 배치 제출 → 승인된 reservation에 실제 파일 업로드 → `complete` idempotent 호출까지 전 구간 연결됨(`smart_cache_processor.rs`). background tick에 자동 편입, durable outbox로 크래시 복구, source-changed 재검증 포함. **필드 단위로 서버 계약과 완전히 일치 확인함**(요청/응답 모두 대조 완료) |

---

## 0-1. A가 B 파일을 직접 수정함 — 반드시 인지할 것

Step 16-2/16-3 작업 중 A가 **B가 소유한 서버 파일 2개에 직접 커밋을 넣었다.** 코드 검토 결과 안전한 수정이지만, git blame에 A 이름이 남아있으니 B가 인지하고 있어야 한다.

**수정한 파일:**

1. `apps/server/src/smart-cache/smart-cache.service.ts` (커밋 `2cc671f`, 딱 3줄) — candidate batch 승인 결과(`approved` 배열)에 `sourceRelativePath`, `sourceVersionHash`, `sizeBytes` 3개 필드 추가.
2. `packages/contracts/openapi.yaml` (커밋 `41d2fea`) — `POST /v1/agent/cache-candidates`의 `201` 응답에 실제 스키마 참조 추가(원래는 description만 있고 스키마가 비어 있었음), 신규 스키마 컴포넌트 `CacheUploadReservation`/`CacheCandidateBatchResult` 추가.

**왜 건드렸는가 (원인은 B 코드의 기존 결함):**

원래 서버는 candidate batch(최대 200개 파일 배치 제출) 승인 결과를 `{ reservationId, status, uploadUrl?, expiresAt }`만 반환했다. **어떤 reservationId가 어떤 파일(경로/버전)에 대응하는지 알려주는 필드가 없어서**, 배치로 여러 파일을 제출했을 때 데스크톱이 승인된 reservation과 로컬 파일을 매칭할 방법이 없었다 — 구조적으로 다음 단계 진행이 불가능한 상태였다. DB row(`cacheUploadReservations` 테이블)에는 해당 컬럼이 이미 있었고, 서비스의 응답 매핑(`.map()`)에서만 누락되어 있었다. 즉 새 기능이 아니라 **응답 직렬화 누락 버그를 메운 것**이다.

**안전성 확인:**

- 이 응답 타입에 대한 Zod 스키마가 애초에 없었다(`control-plane.ts`엔 요청 스키마만 존재) — 검증 로직을 깨뜨릴 여지 없음
- 기존 필드는 그대로 두고 필드 3개만 **추가**(non-breaking), DB에 이미 있던 값을 노출한 것뿐이라 새 로직 없음
- `cargo test`(smart_cache 관련 6개) 통과, 서버 쪽 기존 `smart-cache.integration.spec.ts`와도 충돌 없음(확인함)

**B가 할 일:** 재작업 불필요. 다만 이 파일을 다음에 손댈 때 diff에 A 커밋이 섞여 있다는 점만 알아두면 됨. 앞으로 이런 상황이 또 생기면 A가 먼저 알리는 쪽으로 하기로 함.

---

## 1. Step 16 남은 것 — 사소한 갭 2개 (급하지 않음)

- **`policy.excludedPatterns`를 A가 로컬에서 안 씀**: 서버 정책의 제외 패턴을 로컬 후보 필터링에 반영하지 않고 일단 제출함. 서버가 `matchesExcludedPattern`으로 다시 걸러내므로(`smart-cache.service.ts`) **안전 문제는 아니고**, 걸러질 파일을 준비/제출하는 낭비만 있음. B가 할 일 없음(A 쪽 최적화 과제).
- **pin/exclude는 A→서버 방향만 연결됨**: 로컬에서 pin/exclude한 파일은 candidate 제출 시 `manualPin`으로 서버에 반영되지만(완료), **서버/모바일에서 pin/exclude를 설정해도 A가 되읽어오는 경로가 아직 없음**. 아래 3.3에서 계속 다룸.

---

## 2. P0 — 우선 처리 (데모 vertical slice에 필요)

### 2.1 Decision item별 부분 승인
현재 서버는 `approvedItemIds`가 전체 집합과 다르면 **명시적으로 거부**한다 (`decisions.service.ts` 88-96행). all-or-nothing만 지원.

- **필요 여부 먼저 확인**: 데모에 item별 승인이 꼭 필요한지 A와 상의. 필요 없으면 스킵 가능(현재도 동작은 함).
- 필요하다면: `decisions` 서비스에서 부분 집합 허용 + `proposal_items`에 개별 상태(`APPROVED`/`REJECTED`) 저장 + 미승인 item은 실행에서 skip 처리.

### 2.2 Reject reason 입력
모바일 proposal 화면(`proposal_page.dart`)에 전체 approve/reject 버튼만 있고 거절 사유 입력이 없음.

- 서버: `decisions` 생성 payload에 `rejectReason?: string` optional 필드 추가, 저장.
- 모바일: reject 버튼 클릭 시 사유 입력 dialog 추가, 서버로 전송.

### 2.3 ~~Smart cache candidate 제출 스키마 확정~~ — 완료됨
Step 16-2/16-3에서 A가 실제로 연결하면서 스키마 불일치(reservation 응답 필드 누락)를 발견해 A가 직접 고쳤다(0-1 참고). 더 할 일 없음.

---

## 3. P1 — 중요하지만 데모 필수는 아닐 수 있음

### 3.1 AI Command Server/UX — **완전 미착수, 가장 큰 덩어리**
`apps/server/src/ai-gateway/`는 `.gitkeep`만 있음. chat 엔드포인트는 항상 `aiStatus: 'UNCONFIGURED'`, `assistant: null` 반환 (`chat.controller.ts:81`).

필요 작업:
- LLM provider 설정(키 관리, `UNCONFIGURED` fallback 유지 — 미설정 시 성공 위장 금지 원칙 준수)
- 자연어 → command JSON 변환 엔드포인트
- schema validation 실패 시 사용자에게 되묻는 UX
- `packages/contracts/src/control-plane.ts:184-242`에 이미 README intent/payload 스키마가 있음 — 이걸 타겟으로 우선 연결
- **규칙: AI가 직접 파일을 쓰면 안 됨.** 최종 변경은 반드시 proposal approval → precheck → journal 경로만 통과.

이 항목은 범위가 크므로, 데모 우선순위에서 뒤로 미룰지 A와 먼저 상의 권장.

### 3.2 README 인앱 편집 UX
현재 승인/거절 뷰만 있고 draft를 모바일에서 직접 편집하는 기능이 없음(`proposal_page.dart`의 `ProposalSummaryCard`는 view-only).

- 모바일에 README draft 텍스트 편집 필드 추가 + 편집된 내용을 approval payload에 포함하는 서버 처리.

### 3.3 pin/exclude 양방향 동기화 — 절반 완료(A→서버는 됨, 서버→A는 안 됨)
A→서버 방향은 Step 16-2에서 이미 연결됨: 로컬 pin은 candidate 제출/complete-upload에 `manualPin`으로 실려서 `cachedFiles.manualPin`에 반영됨(추가 작업 불필요).

남은 것은 **서버/모바일 → A** 방향:
- API: `POST /v1/cached-files/:cachedFileId/pin`, `POST /v1/cached-files/:cachedFileId/exclude` (또는 기존 patch 엔드포인트에 필드 추가)
- 모바일: cached files 목록에 pin/exclude 토글 추가
- 이 API가 생기면 A도 로컬 `smart_cache_file_preferences`로 pull해오는 코드가 필요함(A 쪽 작업, B는 API/UI만 제공하면 됨) — API 완성되면 A에게 알릴 것.

### 3.4 File Browse 에러 코드 세분화
현재 `PERMISSION_DENIED`/`ROOT_DISABLED`/`INVALID_PATH`가 뭉뚱그려 `FILE_OPERATION_FAILED`로 나감. 급하지 않으면 스킵 가능(A도 현재 `OUTSIDE_MANAGED_ROOT`로 대충 매핑 중이라 서로 맞춰야 함).

---

## 4. P2 — 있으면 좋음, 데모엔 없어도 무방

- **QR 스캔 UX**: 현재 6자리 코드 수동 입력만 있음(`pairing_page.dart`). 카메라 스캔은 nice-to-have.
- **자연어 command 입력**: 현재 버튼/폼 기반만. AI Command Server(3.1)와 묶어서 처리하는 게 효율적.
- **Overlay/Character 실제 모션**: Rive 자산 미연결(`character_settings_page.dart`가 `riveAssetStatus: 'UNCONFIGURED'` 명시). 시간 남으면.

---

## 5. B에게 요청하는 순서 제안

1. 2.1(item별 승인 필요 여부)과 3.1(AI 우선순위)을 **먼저 A와 15분 정도 상의** — 둘 다 범위/설계 결정이 필요해서 바로 코드 시작하면 手戻り 위험 있음.
2. 확정되면: 2.2(reject reason) → 3.3(pin/exclude API/UI) 순으로 처리.
3. 3.1(AI)은 별도 트랙으로 분리해서 진행(범위가 크므로 나머지 P0/P1과 병렬 진행 권장).

---

## 참고 (B 작업 아님, 공유 컨텍스트용)

Step 16 검증 중 발견: `cargo test --features tauri-commands`가 `overlay.rs`의 테스트 코드 시그니처 불일치로 **컴파일 자체가 안 됨**(Step 11부터 있던 문제, Step 16과 무관). CI(`​.github/workflows`)가 `cargo test`는 기본 feature로만 돌리고 `tauri-commands` feature는 `cargo check`만 실행해서 지금까지 안 걸렸음. B 작업은 아니지만, "Rust 테스트 전부 통과"라는 README 문구를 그대로 믿으면 안 되는 상태라는 점만 공유.
