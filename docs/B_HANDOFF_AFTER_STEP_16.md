# B Task List — After A Step 16 (2026-07-13)

A는 로컬 19단계 계획(`7/12_plan.md`) 기준 **Step 16(로컬 스마트 캐시 usage scoring)까지 완료**했다. 이 문서는 B/모바일 코드베이스를 실제로 감사한 결과를 바탕으로, **이미 끝난 것과 진짜 남은 것**을 구분한 태스크 리스트다. 이미 구현된 항목을 다시 만들지 않도록 파일 경로까지 명시한다.

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

---

## 1. Step 16 반영 — 새로 생긴 서버 쪽 작업

A가 이번에 로컬 SQLite에 **usage scoring + pin/exclude를 로컬로만** 구현했다 (`apps/desktop/src-tauri/src/storage/smart_cache.rs`, `commands/smart_cache.rs`). 아직 서버로 전송하는 코드는 없다(로컬 tauri command만 등록됨, `lib.rs`에서 호출하는 background runtime 연결 없음).

이게 뜻하는 것:

- A가 다음 단계(16-2)에서 `list_smart_cache_candidates` 결과를 **배치로 서버에 제출**할 예정. 서버의 `POST /v1/agent/cache-candidates`(이미 존재, `smart-cache.controller.ts`)가 그 수신처가 될 것 — **이 엔드포인트는 이미 있으니 스키마만 A와 맞추면 됨**. 필드: `rootId`, `relativePath`, `score`, `eventCount`, `lastUsedUnixMs`, `pinned`.
- **로컬 pin/exclude를 모바일에서 조작할 방법이 없다.** DB에는 `cachedFiles.manualPin`(`packages/database/src/schema.ts:602`) 컬럼이 있지만, 이걸 다루는 API도 모바일 UI도 없음 — **아래 태스크 5 참고**.

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

### 2.3 Smart cache candidate 제출 스키마 확정
1번 항목 참고 — A가 곧 붙일 `POST /v1/agent/cache-candidates`의 요청 스키마를 A와 맞춰서 문서화(OpenAPI에 이미 있는지 확인, 없으면 추가).

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

### 3.3 로컬 pin/exclude → 서버 동기화
- API: `POST /v1/cached-files/:cachedFileId/pin`, `POST /v1/cached-files/:cachedFileId/exclude` (또는 기존 patch 엔드포인트에 필드 추가)
- `cachedFiles.manualPin` 업데이트 + A의 다음 candidate 계산에 반영되도록 A pull 경로와 맞춤(A와 필드명 조율 필요)
- 모바일: cached files 목록에 pin/exclude 토글 추가

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
2. 확정되면: 2.2(reject reason) → 2.3(cache candidate 스키마 확정, A 16-2와 병행) → 3.3(pin/exclude 동기화) 순으로 처리.
3. 3.1(AI)은 별도 트랙으로 분리해서 진행(범위가 크므로 나머지 P0/P1과 병렬 진행 권장).
