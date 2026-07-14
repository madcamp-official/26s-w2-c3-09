# 집쥐인(MOUSEKEEPER) MVP 전체 개발 계획서

- 문서 버전: 1.4
- 기준일: 2026-07-13
- 개발 인원: 2명
- 1차 출시 대상: Windows + Android
- 확장 대상: macOS + iOS
- 문서 성격: 제품 범위, 시스템 구조, 개발 순서, 역할 분담, 테스트와 출시 기준을 하나로 묶은 실행 문서
- v1.1 추가 범위: P0 온라인 파일 요청·전달, P1 사용 빈도 기반 스마트 캐시
- v1.2 보완 범위: 스마트 캐시 서버 사전 승인·quota 예약, 오프라인 명령 경고, 캐시 신선도 분리, 파일 탐색 중 연결 끊김 처리
- v1.3 명칭 변경: 서비스 가제를 **집쥐인(MOUSEKEEPER)**으로 변경하고 저장소·휴지통·내부 지표 식별자를 `mousekeeper`로 통일
- v1.4 연결 경험 보완: 청결도 규칙 단일화, 기기·폴더 연결의 쌍방 즉시 해제, 미연결 화면 게이트, 모바일 파일 탐색·다운로드·이름 검색, 호감도 기반 외형·테마 변경 제거

> 핵심 문장: **모바일에서 정리를 부탁하면, PC 안의 집쥐인 에이전트가 안전한 변경안을 만들고 사용자의 승인 뒤에만 실행한다.**

---

# 0. 문서 목적과 최종 반영 결정

이 문서는 집쥐인(MOUSEKEEPER) MVP를 두 명이 병렬로 개발할 수 있도록 다음을 확정한다.

1. 제품의 MVP 범위와 제외 범위
2. A와 B의 최종 책임 및 코드 소유권
3. 데스크톱·모바일·서버 사이의 계약
4. 안전한 파일 작업 원칙
5. 데이터 모델, API, WebSocket 이벤트, 상태 머신
6. Phase별 개발 순서와 완료 조건
7. MadCamp형 10영업일 압축 일정
8. 테스트, 배포, 출시 차단 조건

## 0.1 최종 역할 분배

### A — Desktop Agent 담당

> 실제 컴퓨터에서 폴더 관리 기능이 안전하게 동작하는 것을 끝까지 책임진다.

A는 다음을 직접 구현하고 최종 결정한다.

- Tauri 프로젝트 구조와 네이티브 창
- Rust 파일 엔진
- Windows/macOS 파일 시스템 추상화
- 관리 폴더 등록과 권한 경계
- 폴더 변화 감지와 전체 재스캔
- 규칙 엔진
- 파일 조회·이동·격리
- 관리자 휴지통
- 복구와 되돌리기
- README 생성·수정의 로컬 적용과 diff
- 작업 저널과 로그
- 자동 시작, 시스템 트레이, 데스크톱 빌드
- 데스크톱 파일 관리 화면
- 모바일의 온라인 파일 탐색 요청에 대한 로컬 조회·경로 검증
- 요청 파일의 읽기 전용 chunk 전송, checksum 검증, 취소 처리
- P1 스마트 캐시 후보 산정을 위한 로컬 사용 이벤트 집계
- B가 만든 캐릭터 UI가 동작할 오버레이 네이티브 창과 읽기 전용 bridge

### B — Product & Cloud 담당

> 사용자가 데스크톱과 모바일에서 같은 관리자를 만나는 경험을 끝까지 책임진다.

B는 다음을 직접 구현하고 최종 결정한다.

- 데스크톱 오버레이의 캐릭터 UI
- 집·방 공간 표현
- 캐릭터 이동·모션·반응
- 청결도 시각화
- 채팅
- 커스터마이징
- 호감도와 성장
- 모바일 홈과 방 화면
- 모바일 원격 명령
- 제안 승인·거절
- 온라인·오프라인 집 표현
- 로그인, 기기 등록, 페어링
- REST API, WebSocket, Heartbeat 상태 저장
- 원격 명령 전달과 오프라인 명령 큐
- 작업 결과 동기화
- 모바일 온라인 파일 탐색·요청·미리보기·다운로드
- 일회성 파일 전송 세션과 만료형 relay/object lifecycle
- P1 사용 빈도 기반 스마트 캐시 정책·저장·삭제·오프라인 조회
- 최소 알림
- 서버·Android 배포

## 0.2 기존 계획에서 수정한 사항

### 서비스 가제와 기술 식별자

- 서비스 가제는 **집쥐인(MOUSEKEEPER)**으로 통일한다.
- 사용자에게 노출되는 한국어 이름은 **집쥐인**, 영문 브랜드명은 **MOUSEKEEPER**를 사용한다.
- 저장소·패키지·내부 지표 등 코드 식별자는 소문자 `mousekeeper`를 사용한다.
- 관리자 전용 임시 휴지통 경로는 `.mousekeeper_trash`로 통일한다.
- 기존 가제에 종속된 브랜드 식별자는 신규 문서와 코드에서 사용하지 않는다.

### 관리 폴더 개수

애플리케이션에는 관리 폴더 개수 하드 제한을 넣지 않는다.

```text
User 1 ── 1 ACTIVE Device
Device 1 ── N Room
Room 1 ── 1 Managed Root
```

다만 내부 알파의 회귀 테스트 기준은 한 PC에서 관리 폴더 3개로 둔다. 이 숫자는 제품 제한이 아니라 테스트 기준이다.

### 반드시 유지할 관리 범위 제한

개수 제한과 달리, 다음 제한은 영구적인 안전 규칙이다.

- 사용자가 명시적으로 등록한 관리 루트만 읽고 변경한다.
- 등록되지 않은 경로는 AI 응답이나 원격 명령에 포함돼도 거절한다.
- 상위 폴더와 그 하위 폴더를 별도 방으로 중복 등록하지 않는다.
- 관리 루트를 벗어나는 `..`, 절대 경로, junction, symlink, reparse point 우회를 차단한다.

### 서버 DB와 ORM

- DB는 **관리형 PostgreSQL**을 사용한다.
- 버전은 특정 숫자를 제품 요구사항으로 고정하지 않고, 선택한 관리형 공급자가 안정 지원하는 버전을 사용한다.
- 기본 데이터 접근 계층은 **Drizzle ORM**으로 정한다.
- 상태 전이, 잠금, 멱등 처리처럼 SQL 표현이 더 명확한 부분은 SQL을 직접 작성한다.
- Prisma는 팀이 이미 충분한 경험을 갖고 있을 때 교체 가능한 선택지이지 필수 기술이 아니다.

### 온라인 파일 접근과 스마트 캐시

파일 접근은 기존 파일 정리 상태 머신을 바꾸지 않고 별도의 읽기 전용 흐름으로 추가한다.

- **P0 — 온라인 파일 요청·전달:** 데스크톱이 연결된 동안 모바일이 관리 루트 내부 파일을 탐색하고 요청하면, 데스크톱이 실행 직전 경로와 파일 identity를 재검증한 뒤 서버의 짧은 수명 전송 세션을 통해 모바일로 전달한다.
- P0 전송 파일은 영구 보관하지 않는다. 모바일 수신 완료 ACK 또는 TTL 만료 시 임시 object를 삭제한다.
- **P1 — 사용 빈도 기반 스마트 캐시:** 사용자가 방별로 기능을 명시적으로 켠 경우에만, 자주 요청하거나 집쥐인을 통해 자주 열어 본 파일을 우선 선정해 암호화된 원본 일부를 서버 object storage에 저장한다.
- 스마트 캐시는 전체 폴더 동기화가 아니며, 용량 한도·파일별 크기 한도·제외 규칙·최신 버전 여부를 항상 표시한다.
- 파일 가져오기는 읽기 전용이므로 Proposal/Decision/Operation Journal을 사용하지 않는다. 대신 `FileTransfer` 상태 머신과 감사 로그를 사용한다.
- 파일 이동·격리·README 쓰기처럼 로컬 상태를 바꾸는 명령은 기존 제안→승인→재검증→저널 흐름을 그대로 사용한다.

## 0.3 절대 바꾸지 않는 제품 원칙

```text
관찰
→ 설명 가능한 제안
→ 사용자 승인
→ 실행 직전 재검증
→ 복구 가능한 실행
→ 결과 동기화
→ 공간과 캐릭터 반응
```

- 승인 없는 파일 쓰기 금지
- 영구 삭제를 모바일에서 수행하지 않음
- 기존 파일 자동 덮어쓰기 금지
- AI가 파일 API를 직접 호출하지 않음
- 캐릭터 성격과 호감도가 파일 권한에 영향을 주지 않음
- 서버는 사용자의 전체 파일 인덱스를 보관하지 않으며, 파일 원문은 P0 만료형 전송 object 또는 사용자가 켠 P1 스마트 캐시 범위에서만 제한적으로 보관
- 중요한 명령과 결과는 WebSocket이 아니라 PostgreSQL에 먼저 저장
- 온라인 파일 전달과 스마트 캐시는 등록된 관리 루트, 인증된 동일 사용자·기기, 명시된 크기·TTL·용량 정책을 벗어나지 않음


## 0.4 v1.2 흐름 보완 결정

기존의 정리 vertical slice와 P0/P1 범위는 변경하지 않고 다음 방어선을 추가한다.

```text
P1 cache candidate metadata
→ server quota/policy decision
→ short-lived quota reservation
→ approved upload targets only
→ encrypt/upload
→ checksum/version verification
→ AVAILABLE + freshness metadata
```

- `AVAILABLE`은 다운로드 가능 상태이고 최신 보장 상태가 아니다.
- PC 오프라인에서는 `last_verified_at`을 기준으로 신선도 불확실성을 표시한다.
- 오프라인 `QUEUED` command와 캐시 목록은 서로의 상태를 숨기지 않는다.
- P0 pagination 실패는 명시적 상태로 종료하고 이미 받은 page만 fallback으로 유지한다.

## 0.5 v1.4 연결·청결도·파일 탐색 결정

- 청결도 점수와 차감 사유는 A의 Rust 파일 엔진이 동일한 공식 버전으로 한 번만 계산한다. 데스크톱은 그 결과를 직접 표시하고 모바일은 서버에 동기화된 같은 `RoomSnapshot`을 표시한다.
- `DevicePairing` 해제와 `Room/ManagedRoot` 연결 해제를 별도 동작으로 취급한다. 둘 다 모바일과 데스크톱에서 요청할 수 있고 성공 event를 받은 양쪽 UI에서 즉시 제거한다.
- 연결 해제의 정상 반영은 heartbeat 만료를 기다리지 않고 DB transaction과 WebSocket event로 처리한다. heartbeat는 5초, presence TTL은 15초로 줄여 비정상 종료 감지의 보조선으로 사용한다.
- 활성 desktop device가 하나도 없는 모바일은 메인 IA를 숨기고 pairing code 입력 화면만 표시한다. device는 연결되어 있지만 room이 0개이면 메인 화면의 “연결된 폴더 없음” 상태를 표시한다. 과거 room cache는 연결 해제 event 또는 다음 replay에서 제거한다.
- 모바일 계정은 동시에 하나의 활성 desktop device만 가진다. 이미 페어링된 상태에서는 새 페어링 진입을 숨기고, 현재 device를 해제한 뒤에만 다른 PC의 코드를 claim할 수 있다. 서버도 동시 claim 경쟁을 포함해 두 번째 활성 device 생성을 `DEVICE_ALREADY_PAIRED`로 거절한다.
- 호감도 ledger와 완료 대사는 유지할 수 있지만, 호감도 상승으로 외형·accessory·방 테마가 바뀌거나 해금되는 기능은 MVP에서 제거한다.
- 모바일 파일 화면은 연결된 room 목록 → 폴더 탐색 → 이름 검색 → 다운로드 흐름을 제공한다. 검색은 파일 내용이 아니라 managed root 안의 파일·폴더 이름만 대상으로 하며 A의 경로 경계 검증을 반드시 통과한다.

---

# 1. 서비스 개요와 MVP 목표

## 1.1 서비스 정의

**집쥐인(MOUSEKEEPER)**은 폴더를 방으로 표현하고, 사용자가 모바일에서 정리 요청을 보내면 PC의 데스크톱 에이전트가 로컬 파일을 분석해 안전한 변경안을 제안하는 크로스플랫폼 파일 관리 서비스다.

## 1.2 MVP에서 검증할 가설

1. 사용자는 PC 앞에 있지 않아도 정리 요청을 만들어 둘 가치가 있다고 느낀다.
2. 자동 실행보다 제안·승인 방식에서 더 높은 신뢰를 느낀다.
3. 폴더의 상태를 방과 청결도로 표현하면 정리 필요성을 쉽게 이해한다.
4. 캐릭터 반응과 성장 요소가 반복 사용 동기를 만든다.
5. 모바일 명령과 PC 실행 사이의 오프라인 큐가 실제 사용 맥락에 유용하다.
6. PC가 켜져 있을 때 모바일에서 필요한 파일을 즉시 가져올 수 있으면 서비스의 일상적 효용이 커진다.
7. 사용자가 자주 쓰는 파일만 선택적으로 캐시하면 전체 폴더 동기화 없이도 PC 오프라인 상황을 일부 보완할 수 있다.

## 1.3 MVP 핵심 성공 시나리오

```text
1. 사용자가 Android 앱에서 다운로드 방을 선택한다.
2. “30일 넘은 PDF를 정리해줘”라고 요청한다.
3. 요청은 서버에 영속 저장된다.
4. Windows 에이전트가 로컬 폴더를 분석한다.
5. 모바일에 파일별 제안과 이유가 표시된다.
6. 사용자가 승인한다.
7. 데스크톱이 승인 이후 파일 상태를 다시 검사한다.
8. 파일을 .mousekeeper_trash 또는 지정 폴더로 안전하게 이동한다.
9. 작업 결과와 청결도가 모바일에 반영된다.
10. 캐릭터가 완료 모션과 대사를 보여준다.
11. 사용자는 데스크톱에서 작업을 되돌릴 수 있다.
```

## 1.4 파일 접근 성공 시나리오

### P0 — 데스크톱 온라인

```text
1. 사용자가 모바일 방의 파일 탭을 연다.
2. 서버가 온라인 데스크톱에 해당 디렉터리의 페이지 조회를 요청한다.
3. 데스크톱이 관리 루트 내부인지 확인하고 상대 경로 기반 목록을 반환한다.
4. 사용자가 파일을 선택하고 가져오기를 누른다.
5. 서버가 짧은 수명의 transfer session을 생성한다.
6. 데스크톱이 파일 identity·크기·수정 시각을 재검증한다.
7. 데스크톱이 파일을 chunk 단위로 전송하고 전체 SHA-256을 확정한다.
8. 모바일이 파일을 내려받아 checksum을 확인한다.
9. 수신 완료 ACK 또는 TTL 만료 뒤 서버의 임시 object가 삭제된다.
```

### P1 — 데스크톱 오프라인

```text
1. 사용자가 방 설정에서 스마트 캐시를 명시적으로 켠다.
2. 데스크톱이 집쥐인을 통해 관찰한 최근 사용 이벤트를 기반으로 후보 점수를 계산한다.
3. 자주 요청·열람된 파일과 사용자가 직접 고정한 파일을 용량 한도 안에서 암호화 업로드한다.
4. PC가 연결되지 않은 상태에서도 모바일은 캐시된 파일 목록과 캐시 시각을 확인한다.
5. 사용자가 파일을 열면 서버의 암호화 object에서 다운로드한다.
6. 원본 변경이 감지된 캐시는 STALE로 표시하고 PC 재접속 전까지 최신본으로 오인시키지 않는다.
```

---

# 2. MVP 범위

## 2.1 P0 — 반드시 구현

| 영역 | 기능 | MVP 완료 기준 |
|---|---|---|
| 계정 | 모바일 로그인 | 사용자 계정으로 모바일 앱에 로그인 가능 |
| 페어링 | 모바일–데스크톱 연결 | QR 또는 짧은 코드로 한 계정에 PC 등록 |
| Presence | PC 온라인·오프라인 | Heartbeat 만료 시 모바일 집의 불이 꺼짐 |
| 멀티 폴더 | 여러 관리 폴더 | 데이터 모델과 UI가 N개의 방을 지원하며 개수 하드 제한 없음 |
| 폴더 등록 | 명시적 관리 루트 | 사용자가 네이티브 폴더 선택창으로 등록한 경로만 관리 |
| 스캔 | 로컬 파일 인덱스 | 파일명, 상대 경로, 크기, 수정 시각, 파일 ID를 SQLite에 저장 |
| 변화 감지 | watcher + 재조정 스캔 | 생성·수정·이동을 감지하고 누락은 주기 스캔으로 보완 |
| 규칙 | 확장자·기간·이름 조건 | 결정론적 Rule DSL로 대상 파일 계산 |
| 제안 | 파일별 변경안 | 이유, 예상 목적지, 충돌 여부를 모바일에서 확인 |
| 승인 | 전체 승인·거절 | 승인 결과가 서버에 영속 저장되고 중복 처리되지 않음 |
| 파일 실행 | 이동·격리·폴더 생성 | 승인된 항목만 실행하고 기존 파일을 덮어쓰지 않음 |
| 휴지통 | `.mousekeeper_trash` | 원래 위치와 복구 정보를 남기고 되돌리기 가능 |
| README | 생성·수정 제안 | diff를 확인한 뒤 승인된 내용만 로컬 파일에 반영 |
| 청결도 | 0~100 점수 | A가 계산한 점수와 감점 이유를 B가 공간으로 표현 |
| 작업 기록 | 로컬·서버 결과 | 작업 항목별 성공·실패·건너뜀 이유를 조회 가능 |
| 온라인 파일 가져오기 | 모바일 파일 탐색·요청·다운로드 | PC가 온라인일 때 관리 루트 내부 파일을 요청하고, 만료형 전송 세션을 통해 checksum 검증 후 모바일로 가져올 수 있음 |
| 오프라인 큐 | PC가 꺼진 상태의 명령 | 명령이 유실되지 않고 재접속 후 순서대로 처리 |
| 캐릭터 | 기본 상태 애니메이션 | IDLE, ANALYZING, WAITING, WORKING, SUCCESS, ERROR |
| 모바일 집 | 방·캐릭터·상태 | 온라인 상태, 청결도, 대기 제안, 최근 결과 표시 |
| 채팅 | 제한된 자연어 명령 | AI 결과를 구조화된 명령 초안으로 변환 후 확인 |
| 호감도 | 단순 성장 | 성공 작업에 대한 원장형 이벤트와 최소 해금 요소 제공 |
| 배포 | Windows installer, Android build, server | 동일 환경에서 E2E 시연 가능 |

## 2.2 P1 — MVP 뒤에 구현

- macOS 정식 빌드와 notarization
- iOS 정식 빌드
- 여러 사용자 공동 워크스페이스
- 미니게임
- 캐릭터 성격 팩과 강한 자아
- 복잡한 상점·재화 경제
- PC가 꺼져 있어도 자주 쓰는 파일을 가져오는 사용 빈도 기반 스마트 캐시
- 여러 PC 간 파일 복사
- 파일 내용 검색, OCR, 문서 임베딩
- 관리자 권한이 필요한 시스템 폴더 관리
- 네트워크 드라이브와 NAS 쓰기
- 모바일에서 원격 영구 삭제

## 2.3 MVP 운영 가드레일

개수 제한 대신 부하와 충돌을 제어한다.

```text
동시 전체 스캔: PC당 1개
동시 파일 쓰기 실행: PC당 1개
폴더별 watcher: 1개
제안 한 배치: 최대 200개 항목
200개 초과: 여러 proposal batch로 분리
내부 알파 회귀 기준: 3개 관리 루트, 총 100,000개 엔트리
온라인 파일 목록 한 페이지: 최대 200개 항목
동시 원본 전송: PC당 1개, 사용자당 2개
P0 기본 파일 크기 한도: 100MB, 전송 object TTL: 10분
P1 기본 스마트 캐시: 방당 500MB, 파일당 50MB, LRU/낮은 사용 점수부터 퇴출
```

이 값은 코드에 박힌 제품 한도가 아니라 초기 품질 검증 기준이다. 운영 환경에서는 서버 비용과 네트워크 상태에 따라 설정으로 조정한다.

스마트 캐시의 기본 제외 대상은 `.mousekeeper_trash`, 숨김·임시 파일, credential/key 파일, 앱 DB, lock 파일이다. 확장자만으로 민감도를 단정하지 않으며 사용자는 방별 캐시 기능을 끄거나 개별 파일을 제외·고정할 수 있어야 한다.

---

# 3. 최종 소유권과 의사결정권

## 3.1 코드 소유권

```text
mousekeeper/
├─ apps/
│  ├─ desktop/
│  │  ├─ src-tauri/                    # A 최종 소유
│  │  ├─ src/features/files/           # A 최종 소유
│  │  ├─ src/features/admin/           # A 최종 소유
│  │  ├─ src/features/overlay/         # B UI 소유, A native shell 소유
│  │  └─ src/features/character/       # B 최종 소유
│  ├─ mobile/                           # B 최종 소유
│  ├─ server/                           # B 최종 소유
│  └─ worker/                           # B 최종 소유
├─ packages/
│  ├─ contracts/                        # 공동 승인 필요
│  ├─ character-assets/                 # B 최종 소유
│  ├─ design-tokens/                    # B 최종 소유
│  └─ rule-fixtures/                    # A 최종 소유
├─ test-fixtures/
│  └─ file-trees/                       # A 최종 소유
├─ tools/
│  ├─ file-engine-cli/                  # A
│  └─ desktop-agent-simulator/          # B, 개발/테스트 전용
├─ infra/                               # B 최종 소유
└─ docs/                                # 공동
```

## 3.2 영역별 분배 매트릭스

| 영역 | A의 산출물 | B의 산출물 | 연결 계약 | 최종 결정권 |
|---|---|---|---|---|
| Tauri shell | 창, 트레이, 자동 시작, updater | 없음 | window ID와 invoke API | A |
| 데스크톱 캐릭터 | 오버레이 창과 이벤트 bridge | 캐릭터 UI, 모션, 대사 | `CharacterEvent` | B: 표현 / A: native 안전 |
| 폴더 등록 | 네이티브 선택기, canonical path, 로컬 저장 | 모바일 방 표시, 서버 room metadata | `RoomRegistered` | A: 경로 / B: UX |
| 파일 스캔 | watcher, index, reconcile | 진행 상태 표시 | `ScanProgress` | A |
| 규칙 | DSL evaluator, 로컬 검증 | 모바일 규칙 작성 UI, 서버 저장 | `RuleDefinition` | 공동 |
| 청결도 | raw metric과 점수 계산 | 방 그래픽과 등급 표현 | `RoomSnapshot` | A: 계산 / B: 표현 |
| 제안 | 대상 파일 분석과 proposal item 생성 | proposal 저장·모바일 상세 | `ProposalDraft` | 공동 |
| 승인 | 승인 결과 조회와 precondition 검사 | 승인·거절 API와 UI | `Decision` | B: UX / A: 실행 가능성 |
| 파일 실행 | journal, move, quarantine, undo | execution 상태 저장·표시 | `ExecutionResult` | A |
| README | read/hash/diff/write/undo | 질문 흐름, 초안 생성, 모바일 검토 | `ReadmeDraft` | A: 파일 적용 / B: 콘텐츠 UX |
| 온라인 파일 탐색·전달 | root 검증, 목록 조회, stream, checksum | 모바일 파일 UI, transfer session, 임시 object 삭제 | `FileBrowsePage`, `FileTransfer` | A: 로컬 접근 / B: 전달 UX·서버 |
| 스마트 캐시 P1 | 사용 이벤트 집계, 후보 점수, 최신본 검증 | cache policy, object lifecycle, 오프라인 UI | `SmartCachePolicy`, `CachedFileMetadata` | 공동 |
| Heartbeat | 송신·재접속 | TTL 저장·모바일 표현 | `Presence` | 공동 |
| 오프라인 명령 | 로컬 inbox/outbox | PostgreSQL queue와 replay API | cursor + idempotency | 공동 |
| 채팅 | 구조화 명령만 수신 | 채팅 UI, AI adapter, 기록 | `CommandDraft` | B |
| 호감도 | 작업 성공 event 발행 | affinity ledger, 해금, 표현 | `AffinityEvent` | B |
| 로그 | 상세 로컬 파일 저널 | 사용자용 요약, 서버 감사 로그 | `AuditSummary` | 공동 |
| 보안 | 파일 경로·권한·저널 안전 | 인증·API·기기 토큰 보안 | threat model | 각 영역 소유자 |
| 빌드 | Windows/macOS desktop | Android/iOS, server | release checklist | 각 영역 소유자 |

## 3.3 의사결정권

| 결정 | 최종 결정권 | 필수 리뷰 |
|---|---|---|
| 파일 조작 알고리즘 | A | B |
| 경로 검증과 권한 경계 | A | B |
| SQLite 스키마 | A | B |
| 모바일 IA와 사용자 문구 | B | A |
| 캐릭터 디자인과 모션 | B | A |
| 서버 DB·API·배포 | B | A |
| 명령·제안·승인 상태 머신 | 공동 | 양쪽 승인 |
| API와 WebSocket 계약 | 공동 | 양쪽 승인 |
| 파일 전송 경로 검증·checksum 방식 | A | B |
| 전송 TTL·크기·비용 제한 | B | A |
| 스마트 캐시 후보 점수·제외 정책 | 공동 | 양쪽 승인 |
| MVP 범위 변경 | 공동 | 양쪽 승인 |
| 프로덕션 출시 | 공동 | 출시 차단 조건 전부 통과 |

A는 파일 안전성에 대한 거부권을 가진다. B는 제품 경험과 사용자 표현에 대한 최종 결정권을 가진다.

---

# 4. 기술 스택

## 4.1 확정 스택

| 영역 | 기술 | 담당 | 선정 이유 |
|---|---|---|---|
| Desktop shell | Tauri 2 | A | Rust 파일 엔진과 Web UI를 결합하면서 네이티브 권한을 capability로 제한 가능 |
| Desktop core | Rust stable | A | 경로 처리, 파일 I/O, 저널, 동시성에서 명확한 타입과 오류 처리 제공 |
| Desktop UI | React + TypeScript + Vite | A/B | A의 관리 화면과 B의 오버레이 UI를 같은 앱 안에서 기능별 분리 가능 |
| Local DB | SQLite WAL + `sqlx` | A | 설치 불필요, 트랜잭션, FTS, 로컬 우선 복구 지원 |
| Watcher | Rust `notify` | A | Windows와 macOS 백엔드를 동일한 추상화로 사용 가능 |
| Native secret | OS keychain + Rust keyring crate | A | 장치 token과 private key를 평문 설정 파일에 저장하지 않음 |
| Character runtime | Rive | B | 데스크톱과 Flutter에서 동일 상태 머신 에셋 재사용 |
| Mobile | Flutter stable | B | Android 우선 개발과 iOS 확장, 애니메이션 UI에 적합 |
| Mobile state | Riverpod + Freezed + GoRouter | B | 비동기 상태, 불변 DTO, 화면 라우팅을 기능별로 분리 |
| Mobile local DB | Drift + SQLite | B | 방·제안·채팅 캐시와 mutation outbox 구현 |
| API server | Node.js LTS + NestJS + Fastify adapter | B | REST, Guard, DTO, WebSocket Gateway를 모듈러 모놀리스로 관리 |
| Realtime | Socket.IO | B | 재연결, room, ACK 구현이 빠름. 영속 보장은 별도 DB로 처리 |
| Main DB | Managed PostgreSQL | B | 사용자·장치·명령·제안·결과의 관계와 트랜잭션 보장 |
| ORM | Drizzle ORM + SQL migration | B | TypeScript 타입 안전성과 SQL 가시성을 동시에 확보 |
| Presence/cache | Valkey 또는 Redis | B | Heartbeat TTL, Socket.IO adapter, 단기 cache |
| File object storage | S3-compatible object storage | B | P0 만료형 전송 object와 P1 암호화 스마트 캐시를 DB 밖에서 수명주기로 관리 |
| Async jobs | BullMQ worker | B | FCM, AI, 전송 만료 삭제, 캐시 갱신·퇴출, 재시도 분리 |
| Auth | Firebase Authentication | B | Flutter 로그인 구현과 서버 token 검증 비용 절감 |
| Push | Firebase Cloud Messaging | B | Android 백그라운드 제안·결과 알림 |
| AI | Provider adapter + JSON Schema/Zod | B | Gemini/OpenAI 교체 가능, AI는 명령 초안만 생성 |
| Hosting | Render Web + Worker + PostgreSQL + Key Value | B | 2인 팀이 VM, DB, Redis를 직접 운영하지 않고 배포 가능 |
| Observability | Sentry + structured logs | A/B | 데스크톱·모바일·서버 오류를 correlation ID로 추적 |
| CI/CD | GitHub Actions | A/B | Rust, React, Flutter, Node 테스트를 병렬 실행 |

## 4.2 버전 정책

- Rust, Flutter, Node는 구현 시작 시점의 안정 채널 또는 LTS를 사용한다.
- PostgreSQL은 호스팅 공급자가 안정 지원하는 버전을 선택한다.
- 최초 릴리스 버전을 그대로 고정하지 않고 보안·버그 수정 patch를 정기 반영한다.
- major upgrade는 기능 개발 PR과 분리한다.
- `package-lock`, `pnpm-lock`, `Cargo.lock`, `pubspec.lock`을 커밋한다.

## 4.3 사용하지 않는 방식

- 파일 작업을 WebView TypeScript에서 직접 수행하지 않는다.
- WebSocket 이벤트만 믿고 명령을 처리하지 않는다.
- Redis를 명령의 영속 저장소로 사용하지 않는다.
- AI가 반환한 경로와 명령을 그대로 실행하지 않는다.
- 서버가 사용자의 전체 파일 인덱스를 수집하지 않는다.
- P0 파일 전송 object를 영구 저장소처럼 사용하지 않는다.
- P1 스마트 캐시를 사용자 동의 없는 전체 폴더 동기화로 확장하지 않는다.
- 마이크로서비스와 Kubernetes로 시작하지 않는다.

---

# 5. 전체 시스템 아키텍처

```text
┌──────────────────────── Android Mobile ────────────────────────┐
│ Flutter                                                        │
│ - Home / House / Room / Chat                                   │
│ - Online file browser / Request / Download                     │
│ - Proposal review / Approve / Reject                           │
│ - Character / Affinity / Customization                         │
│ - Drift cache + mutation outbox                                │
└──────────── HTTPS + Socket.IO + FCM ────────────────────────────┘
                              │
                              ▼
┌──────────────────── Cloud Control Plane ───────────────────────┐
│ NestJS modular monolith                                        │
│                                                               │
│ Auth │ Device │ Pairing │ Room │ Rule │ Command │ Proposal    │
│ Decision │ Execution │ FileAccess │ Transfer │ SmartCache      │
│ Presence │ Chat │ Character │ Audit                            │
│                                                               │
│ PostgreSQL                                                     │
│ - durable command/proposal/decision/execution ledger           │
│                                                               │
│ Valkey/Redis                                                   │
│ - heartbeat TTL                                                │
│ - socket adapter                                               │
│                                                               │
│ S3-compatible Object Storage                                   │
│ - P0 expiring transfer objects                                 │
│ - P1 encrypted smart-cache objects                             │
│                                                               │
│ BullMQ Worker                                                  │
│ - FCM / AI / expiration / retry                                │
└────────────── HTTPS + Socket.IO ────────────────────────────────┘
                              │
                              ▼
┌──────────────────────── Windows Desktop ───────────────────────┐
│ Tauri                                                          │
│                                                               │
│ React UI                         Rust Agent                     │
│ - A: file admin screens         - managed root boundary        │
│ - B: character overlay          - scan / watcher / rule engine │
│ - B: status visualization       - proposal / file operation    │
│                                  - read-only file transfer      │
│                                  - cache scoring / versioning   │
│                                  - journal / undo / outbox      │
│                                                               │
│ SQLite WAL                                                    │
│ - file index / access events / rules / journal / trash         │
│ - transfer state / cache candidates / sync cursor               │
└────────────────────────────────────────────────────────────────┘
```

## 5.1 책임 경계

### 서버에 저장하는 데이터

- 사용자와 기기
- 방 이름과 표시 설정
- 구조화된 규칙
- 원격 명령
- 제안과 승인·거절
- 실행 결과 요약
- 청결도 snapshot
- 채팅 메시지
- 캐릭터와 호감도
- 감사 로그
- P0 파일 전송 session metadata와 만료 시각
- P1 스마트 캐시 policy, 암호화 object key, 버전·만료 metadata

### 데스크톱에만 저장하는 데이터

- 절대 경로
- 전체 파일 인덱스
- 파일 ID와 상세 메타데이터
- 실제 파일 내용
- 로컬 operation journal
- `.mousekeeper_trash`의 실제 위치
- 파일별 복구 상태
- 파일별 로컬 사용 이벤트와 스마트 캐시 후보 점수
- 전송 직전의 절대 source path

### 제한적으로 서버에 올라가는 파일 정보

모바일 제안 검토와 파일 접근을 위해 필요한 최소 정보만 서버에 저장할 수 있다. 다음 제한을 둔다.

- 절대 경로는 전송하지 않는다.
- 전체 인덱스는 전송하지 않는다. 온라인 파일 탐색 응답은 페이지 단위이며 짧은 TTL 뒤 폐기한다.
- proposal과 실행 기록의 보존 기간을 설정한다.
- 로그와 메트릭 label에 파일명을 넣지 않는다.
- P0 파일 원문은 만료형 transfer object로만 업로드하며 다운로드 완료 ACK 또는 TTL 만료 뒤 삭제한다.
- P1 파일 원문은 사용자가 방별 스마트 캐시를 켠 경우, 선택된 파일과 용량 한도 안에서만 암호화 저장한다.
- 캐시된 파일에는 원본 수정 시각·크기·hash·캐시 시각과 `last_verified_at`을 함께 저장한다. `AVAILABLE`은 다운로드 가능 여부만 뜻하며, 최신 여부는 별도의 freshness 상태로 표시한다.

## 5.2 통신 원칙

```text
REST       = 생성, 조회, 상태 복구, cursor replay, 파일 탐색·전송 session 제어
Socket.IO  = 새로운 데이터가 생겼다는 실시간 알림
PostgreSQL = 중요한 상태와 transfer/cache metadata의 단일 진실 공급원
Object Storage = 만료형 transfer blob과 opt-in 스마트 캐시 blob
SQLite     = 로컬 파일 상태, 사용 이벤트, 실행 복구의 단일 진실 공급원
```

Socket.IO event를 놓쳐도 각 클라이언트는 마지막 cursor 이후의 이벤트와 미처리 상태를 REST로 재조회할 수 있어야 한다.

## 5.3 파일 변경 흐름과 파일 접근 흐름의 분리

```text
파일 변경: Command → Proposal → Decision → Journal → Execute → Undo
파일 접근: BrowseRequest → TransferSession → Validate → Stream → Verify → Expire
스마트 캐시: LocalUsageScore → CandidateMetadata → ServerQuotaReservation → UploadTargets → Encrypt/Upload → VersionCheck → Evict
```

- 파일 접근은 로컬 파일을 수정하지 않으므로 operation journal에 섞지 않는다.
- 파일 접근 실패가 정리 command, proposal, execution 상태를 변경하지 않는다.
- 서버와 모바일은 P0 전송본과 P1 캐시본을 명확히 구분해 표시한다.
- 파일이 격리·이동·수정되면 관련 transfer session은 취소하고 P1 cache entry는 `STALE` 또는 `INVALIDATED`로 전환한다.

---

# 6. 데스크톱 설계 — A 중심, B 오버레이 연동

## 6.1 Rust 모듈 구조

```text
apps/desktop/src-tauri/src/
├─ main.rs
├─ app_state.rs
├─ commands/
│  ├─ managed_roots.rs
│  ├─ file_query.rs
│  ├─ rules.rs
│  ├─ proposals.rs
│  ├─ operations.rs
│  ├─ trash.rs
│  ├─ readme.rs
│  ├─ file_browse.rs
│  ├─ file_transfer.rs
│  └─ settings.rs
├─ domain/
│  ├─ root.rs
│  ├─ file_entry.rs
│  ├─ rule.rs
│  ├─ proposal.rs
│  ├─ operation.rs
│  ├─ file_transfer.rs
│  ├─ smart_cache.rs
│  └─ errors.rs
├─ fs/
│  ├─ mod.rs
│  ├─ platform.rs
│  ├─ windows.rs
│  ├─ macos.rs
│  ├─ path_guard.rs
│  ├─ scanner.rs
│  ├─ watcher.rs
│  ├─ stream_reader.rs
│  └─ executor.rs
├─ storage/
│  ├─ sqlite.rs
│  ├─ migrations.rs
│  ├─ file_index_repo.rs
│  ├─ journal_repo.rs
│  ├─ file_access_repo.rs
│  ├─ cache_candidate_repo.rs
│  └─ outbox_repo.rs
├─ sync/
│  ├─ api_client.rs
│  ├─ socket_client.rs
│  ├─ heartbeat.rs
│  ├─ transfer_client.rs
│  ├─ cache_uploader.rs
│  ├─ inbox.rs
│  └─ outbox.rs
├─ overlay/
│  ├─ window.rs
│  └─ event_bridge.rs
└─ telemetry/
```

`platform.rs`에는 다음 trait를 둔다.

```rust
trait PlatformFileSystem {
    fn canonicalize_root(&self, path: &Path) -> Result<CanonicalRoot>;
    fn file_identity(&self, path: &Path) -> Result<FileIdentity>;
    fn scan(&self, root: &CanonicalRoot) -> Result<Vec<FileEntry>>;
    fn watch(&self, root: &CanonicalRoot) -> Result<WatcherHandle>;
    fn move_no_overwrite(&self, source: &Path, target: &Path) -> Result<()>;
}
```

Windows 구현을 먼저 완성하고 macOS는 같은 trait의 adapter로 추가한다.

## 6.2 관리 루트 등록 규칙

```text
1. 사용자가 네이티브 폴더 선택창에서 경로 선택
2. canonical path 계산
3. 존재 여부와 접근 권한 확인
4. 기존 root와 동일한지 확인
5. 기존 root의 부모 또는 자식인지 확인
6. 위험한 시스템 경로인지 확인
7. 통과하면 managed_roots에 저장
8. 서버에는 room id와 표시 이름만 동기화
```

등록 거절 예시:

```text
이미 등록: C:\Projects
추가 요청: C:\Projects\MOUSEKEEPER
결과: MANAGED_ROOT_OVERLAP
```

## 6.3 watcher와 스캔

- 앱 시작 시 빠른 DB 로드 후 변경분 reconcile scan 수행
- 폴더별 watcher를 등록
- 이벤트 폭주 시 500ms debounce 후 batch 반영
- watcher overflow 또는 오류 발생 시 전체 reconcile 예약
- 전체 scan은 PC당 하나만 실행
- `.mousekeeper_trash`와 앱 내부 metadata 경로는 index에서 제외
- symlink, junction, reparse point는 기본적으로 따라가지 않음

## 6.4 파일 작업 안전 알고리즘

```text
1. 서버에서 승인된 decision 조회
2. decision.idempotency_key 확인
3. proposal item과 현재 파일을 비교
4. canonical source와 destination 검증
5. 파일 ID, 크기, 수정 시각 재검증
6. 목적지 충돌 검사
7. operation_journal = PLANNED
8. operation_items에 이전 상태 저장
9. operation_journal = JOURNALED
10. no-overwrite 방식으로 파일 이동
11. 실제 결과 검증
12. operation_journal = VERIFIED
13. 서버로 result 전송 또는 sync_outbox 저장
```

### 허용 작업

- 조회
- 인증된 모바일로의 읽기 전용 파일 전달
- 동일 관리 루트 내부 이동
- 이름 변경
- 디렉터리 생성
- `.mousekeeper_trash`로 격리
- README 생성·수정
- 위 작업의 되돌리기

### MVP에서 금지

- 영구 삭제
- 기존 파일 덮어쓰기
- 등록되지 않은 루트로 이동
- 두 관리 루트 사이 직접 이동
- 관리자 권한 상승
- 임의 셸, PowerShell, AppleScript 실행
- symlink/junction을 통한 범위 이탈

## 6.5 `.mousekeeper_trash` 정책

```text
<managed-root>/.mousekeeper_trash/<operation-id>/<original-relative-path>
```

`trash_items`에 다음을 저장한다.

- 원래 상대 경로
- 현재 격리 경로
- 파일 identity
- 크기와 수정 시각
- 격리한 operation ID
- 격리 시각
- 복구 시각
- 충돌 상태

복구 목적지에 같은 이름의 파일이 존재하면 자동 덮어쓰지 않고 `RESTORE_CONFLICT`로 중단한다.

## 6.6 README 처리

역할 분리:

```text
B: 폴더 목적 질문, 사용자 답변, AI/템플릿 초안, 모바일 검토
A: 기존 README 읽기, hash, diff, 실행 전 재검증, 로컬 쓰기, undo
```

기존 README가 있으면 다음 precondition을 저장한다.

```json
{
  "relativePath": "README.md",
  "exists": true,
  "sha256": "...",
  "modifiedAt": "..."
}
```

승인 이후 hash가 바뀌었으면 쓰지 않고 `STALE`로 처리한다.

## 6.7 오버레이 bridge

A는 네이티브 오버레이 창을 만들고 B는 그 창 안의 UI를 구현한다.

A가 B에게 제공하는 이벤트:

```ts
type CharacterEvent = {
  eventId: string;
  roomId: string | null;
  kind:
    | "IDLE"
    | "ANALYZING"
    | "WAITING_APPROVAL"
    | "WORKING"
    | "SUCCESS"
    | "ERROR"
    | "USER_WORKING"
    | "OFFLINE";
  progress?: number;
  occurredAt: string;
};
```

B 오버레이에서 호출 가능한 command는 다음으로 제한한다.

- 메인 관리 창 열기
- 오버레이 숨기기
- 표시 모드 변경
- 캐릭터 위치 저장
- 애니메이션 강도 변경

오버레이 WebView에는 파일 이동·삭제·README 쓰기 command를 노출하지 않는다.

## 6.8 온라인 파일 탐색·전달 P0

### 목록 조회

```text
1. 서버에서 인증된 BrowseRequest 수신
2. room id를 local managed root와 매핑
3. 요청 relative directory를 normalize
4. canonical root 내부인지 검증
5. SQLite index에서 최대 200개를 cursor pagination으로 조회
6. 응답 직전 request 만료 여부와 desktop connection generation을 재확인
7. 절대 경로 없이 relative path, name, size, mtime, mime hint만 반환
8. server가 page를 READY로 저장하고 mobile에 알림
9. 조회 중 PC 절전·네트워크 단절·timeout 발생 시 `DEVICE_OFFLINE` 또는 `TIMED_OUT`으로 종료
10. server browse response는 짧은 TTL 뒤 폐기
```

페이지 조회는 각 cursor마다 독립된 요청으로 취급한다. 다음 페이지 요청 중 연결이 끊겨도 이미 받은 페이지는 읽을 수 있지만, 모바일은 목록 상단에 `일부 목록만 표시 중`과 `PC 연결 후 다시 시도`를 표시한다. 재시도는 마지막으로 성공한 cursor에서 시작하며 실패 요청을 무기한 대기 상태로 남기지 않는다.

### 원본 전달

```text
1. 모바일이 파일 상세에서 가져오기를 명시적으로 누름
2. 서버가 FileTransfer를 REQUESTED로 저장하고 일회성 upload target 발급
3. 데스크톱이 room 소유자, device 상태, relative path를 검증
4. file identity, size, mtime을 SQLite index와 실제 파일에서 재검증
5. 크기 정책과 제외 정책 확인
6. random transfer key로 chunk를 암호화하거나 provider-side envelope encryption 사용
7. chunk upload와 SHA-256 계산
8. 서버가 READY로 전환하고 모바일에 download target 제공
9. 모바일 다운로드 후 checksum 확인 및 ACK
10. 서버가 object 삭제; ACK가 없어도 TTL 만료 worker가 삭제
```

원격 전달은 읽기 전용 사용자 행동이므로 정리 proposal 승인을 다시 요구하지 않는다. 대신 모바일에서 파일명·크기·출처 방을 확인하고 직접 `가져오기`를 눌러야 하며, background 자동 다운로드는 하지 않는다.

전송 중 파일의 identity, 크기 또는 수정 시각이 바뀌면 `SOURCE_CHANGED`로 중단한다. range/chunk 단위 재시도는 허용하지만 서로 다른 파일 버전의 chunk를 합치지 않는다.

## 6.9 사용 빈도 기반 스마트 캐시 후보 산정 P1

Windows의 일반적인 last-access timestamp는 비활성화되거나 신뢰하기 어려울 수 있으므로, 캐시 우선순위는 집쥐인이 직접 관찰한 사용 이벤트를 기준으로 계산한다.

관찰 이벤트:

- 최근 30일 모바일 원본 요청 횟수
- 최근 30일 데스크톱 관리 화면에서 미리보기·열기 횟수
- 최근 수정 시각의 근접도
- 사용자의 `오프라인 보관` 고정 여부

기본 점수 예시:

```text
usage_score =
  0.50 × normalized_mobile_request_count_30d
+ 0.30 × normalized_mousekeeper_open_count_30d
+ 0.20 × modification_recency

manual_pin = 용량·보안 정책을 통과하면 점수와 무관하게 우선 후보
```

- 점수 계산과 원본 경로 매핑은 데스크톱에서 수행한다.
- 데스크톱은 파일을 먼저 암호화하거나 업로드하지 않고, 상대 경로·버전·크기·점수·manual pin만 포함한 후보 metadata batch를 서버에 제출한다.
- 서버는 현재 사용량, 진행 중인 quota reservation, 제외 정책, 파일별 크기 한도, 기존 버전을 기준으로 최종 `UploadTarget` 목록을 반환한다.
- quota는 upload target 발급 시 짧은 TTL로 예약하며, 만료·취소·실패 시 즉시 해제한다.
- 데스크톱은 승인된 target만 암호화해 업로드하고, 승인되지 않은 후보는 로컬 `REJECTED_QUOTA` 또는 `REJECTED_POLICY`로 기록한다.
- 서버의 최종 선택은 방별 quota 안에서 manual pin과 높은 점수순으로 수행한다.
- 파일이 수정되면 기존 cache entry의 freshness를 `STALE`로 표시하고 새 버전 업로드가 끝날 때까지 기존 object를 최신본처럼 표시하지 않는다.
- 일정 기간 사용되지 않은 파일은 낮은 점수/LRU 순서로 퇴출한다.
- 사용자는 자동 후보를 제외하거나 파일을 직접 고정할 수 있다.

---

# 7. Product & Cloud 설계 — B 중심

## 7.1 서버 모듈 구조

```text
apps/server/src/
├─ auth/
├─ users/
├─ devices/
├─ pairing/
├─ rooms/
├─ rules/
├─ commands/
├─ proposals/
├─ decisions/
├─ executions/
├─ file-access/
├─ transfers/
├─ smart-cache/
├─ presence/
├─ sync/
├─ chat/
├─ character/
├─ affinity/
├─ notifications/
├─ ai-gateway/
├─ audit/
└─ common/
```

## 7.2 명령 전달 방식

```text
모바일 POST command
→ PostgreSQL commands에 QUEUED 저장
→ transaction commit
→ command.available socket event
→ 데스크톱이 REST로 command 원문 재조회
→ ACK와 ANALYZING 상태 저장
→ proposal 생성
```

WebSocket은 명령 원문을 신뢰성 있게 보관하는 큐가 아니다. 모든 중요 상태는 DB에 먼저 저장한다.

## 7.3 Heartbeat

```text
desktop heartbeat interval: 5초
presence TTL: 15초
```

Presence 상태:

- `ONLINE_IDLE`
- `ONLINE_SCANNING`
- `ONLINE_EXECUTING`
- `DEGRADED`
- `OFFLINE`

모바일은 `OFFLINE`을 “PC가 반드시 꺼짐”으로 표현하지 않고 “연결 끊김” 상태로 해석한다. 집의 불이 꺼지는 표현은 가능하지만 상세 문구는 “PC 에이전트와 연결되지 않음”으로 한다.

정상적인 연결 해제는 TTL을 기다리지 않는다. 서버가 device 또는 room 상태를 transaction 안에서 비활성화하고 `device.revoked` 또는 `room.removed` sync event를 기록한 뒤 WebSocket으로 즉시 알린다. 양쪽 클라이언트는 event 수신 즉시 화면과 로컬 cache를 갱신하며, socket 유실 시 REST replay가 같은 결과를 복구한다. 5초 heartbeat와 15초 TTL은 앱 강제 종료·네트워크 단절처럼 명시적 해제 요청이 없는 경우에만 사용한다.

## 7.4 채팅과 AI 경계

AI가 할 수 있는 일:

- 자연어를 구조화된 command draft로 변환
- 규칙 초안 생성
- README 문구 초안 생성
- 캐릭터 대사 생성

AI가 할 수 없는 일:

- 로컬 경로를 임의로 선택
- 파일을 직접 실행
- 승인 생략
- 영구 삭제
- 기존 파일 덮어쓰기
- 정책 검증 우회

AI 결과 예시:

```json
{
  "intent": "CREATE_RULE",
  "roomId": "room_uuid",
  "rule": {
    "conditions": [
      { "field": "extension", "operator": "IN", "value": [".pdf"] },
      { "field": "ageDays", "operator": "GTE", "value": 30 }
    ],
    "action": {
      "type": "MOVE",
      "destinationTemplate": "Archive/PDF"
    }
  },
  "requiresConfirmation": true
}
```

Zod/JSON Schema 검증에 실패하면 구조화된 명령으로 저장하지 않고 사용자에게 다시 확인을 요청한다.

## 7.5 캐릭터와 호감도

MVP 캐릭터 범위:

- 캐릭터 1종
- 상태 6종 이상
- 고정 기본 외형 1개
- 고정 기본 방 테마 1개
- 호감도 숫자 1개
- 작업 완료·실패 대사 세트
- 상점, 외형·테마 해금, 호감도 기반 시각 변경 없음

호감도는 append-only ledger로 저장한다.

```text
Execution succeeded  → +2
User approved proposal → +1
Undo performed       →  0
Execution failed     →  0
Daily login           → 증가 없음
```

호감도는 파일 권한, 규칙 결과, 작업 우선순위에 영향을 주지 않는다.
호감도 상승은 숫자와 대사에만 반영하며 캐릭터 외형, accessory, 방 테마를 자동 또는 수동으로 바꾸지 않는다. 기존 appearance/theme 설정 UI와 `FEATURE_LOCKED` 해금 UX는 모바일에서 숨기고 신규 사용 흐름에서 호출하지 않는다.

## 7.6 파일 전송 서비스 P0

서버는 파일 원본의 영구 저장소가 아니라 인증·상태·수명주기를 관리하는 relay control plane으로 동작한다.

- `FileTransfer`를 PostgreSQL에 먼저 저장한 뒤 데스크톱에 알린다.
- object storage upload/download URL은 한 transfer와 짧은 TTL에만 유효하다.
- 서버는 desktop owner와 mobile user가 같은 계정에 속하는지 확인한다.
- download 횟수, object size, checksum, 만료와 삭제 결과를 감사 로그에 남긴다.
- 실패하거나 취소된 transfer object도 worker가 최종 삭제한다.
- P0에서는 백그라운드 자동 업로드, 폴더 단위 zip, 전체 동기화를 제공하지 않는다.

## 7.7 스마트 캐시 서비스 P1

- 방별 `SmartCachePolicy`가 `DISABLED`인 상태가 기본값이다.
- 활성화 시 사용자에게 quota, 제외 대상, 서버 보관 사실을 명확히 설명한다.
- 데스크톱은 후보 metadata만 먼저 제출하고, 서버가 quota와 정책을 검증한 뒤 `UploadTarget`과 예약된 byte 수를 반환한다.
- 서버는 `cached_files`, 진행 중인 upload reservation, 이미 발급한 target을 모두 포함해 quota를 계산한다.
- reservation에는 짧은 만료 시각을 두며, upload 완료·취소·실패·만료 시 원자적으로 확정하거나 해제한다.
- 데스크톱은 승인된 target만 암호화·업로드한다. target 없이 임의 object key로 올린 파일은 등록하지 않고 orphan sweep 대상으로 처리한다.
- object는 user/room/version별 namespace에 저장하고 공개 URL을 만들지 않는다.
- 캐시의 availability 상태와 freshness 상태를 분리한다. `AVAILABLE`은 다운로드 가능함을 뜻하고, `VERIFIED_CURRENT`, `UNVERIFIED_OFFLINE`, `STALE`이 최신 확인 여부를 나타낸다.
- PC 오프라인에서는 availability가 `AVAILABLE`인 버전만 다운로드할 수 있으나, `last_verified_at` 이후 원본 최신성은 보장하지 않는다고 표시한다.
- 캐시된 시각과 원본 최신 확인 시각을 모바일에 별도로 표시하며, PC가 오프라인이면 `마지막 확인: <시각>` 경고를 항상 노출한다.
- room에 미처리 `QUEUED` command가 있으면 캐시 목록 상단에 `명령 처리 후 파일 위치나 목록이 변경될 수 있음` 경고를 표시한다.
- 사용자가 기능을 끄거나 기기를 revoke하면 관련 cache object 삭제 작업을 즉시 예약한다.

---

# 8. 정보 구조도

## 8.1 모바일 IA

```text
Mobile App
├─ Login
├─ Connection Gate
│  ├─ Unpaired: Pairing code input only
│  ├─ Pairing/Disconnecting progress animation
│  └─ Paired: Main App 진입
├─ Home / House
│  ├─ PC connection state
│  ├─ Overall cleanliness
│  ├─ Pending proposals
│  ├─ Recent executions
│  └─ Character summary
├─ Rooms
│  └─ Room detail
│     ├─ Cleanliness and reasons
│     ├─ Character and room
│     ├─ Chat
│     ├─ Files
│     │  ├─ Paired folder list
│     │  ├─ Online browser
│     │  ├─ Name search within managed root
│     │  ├─ Verified download
│     │  ├─ Cached offline files P1
│     │  │  ├─ Cached at / Last verified at
│     │  │  └─ Pending-command change warning
│     │  └─ Transfer status
│     ├─ Rules
│     ├─ Proposals
│     └─ Activity history
├─ Proposal detail
│  ├─ File list
│  ├─ Reason
│  ├─ Expected destination
│  ├─ Conflicts
│  ├─ Approve
│  └─ Reject
├─ Character
│  ├─ Affinity number
│  └─ Fixed appearance and room theme
└─ Settings
   ├─ Devices / Disconnect device
   ├─ Paired folders / Disconnect folder
   ├─ Notifications
   ├─ File transfer limits
   ├─ Offline smart cache P1
   ├─ Privacy
   └─ Account
```

## 8.2 데스크톱 IA

```text
Desktop App
├─ Character Overlay                     # B UI / A shell
├─ System Tray                           # A
│  ├─ Open manager
│  ├─ Run scan
│  ├─ Pause agent
│  ├─ Overlay mode
│  └─ Quit
├─ Manager Window                        # A
│  ├─ Dashboard
│  ├─ Managed folders / Disconnect folder
│  ├─ Rules
│  ├─ File browser
│  ├─ Mobile transfer activity
│  ├─ Smart cache policy and candidates P1
│  ├─ Proposals
│  ├─ MOUSEKEEPER trash
│  ├─ Operation history
│  ├─ README diff
│  ├─ Permissions
│  └─ Connection status / Disconnect device
└─ First Run
   ├─ Pair device
   ├─ Choose managed folder
   ├─ Explain permissions
   └─ Initial scan
```

---

# 9. 핵심 사용자 플로우

## 9.1 페어링

```text
1. 데스크톱이 device key pair 생성
2. A가 pairing session 생성 API 호출
3. 서버가 짧은 코드와 QR payload 반환
4. 데스크톱에 QR 표시
5. 사용자가 모바일에서 로그인 후 QR 스캔
6. B 서버가 현재 user와 device를 연결
7. 데스크톱이 pairing 완료 상태를 polling/socket으로 수신
8. scoped device token을 OS keychain에 저장
9. Heartbeat 시작
10. 모바일 집에 온라인 PC 표시
```

모바일의 최상위 화면은 서버의 활성 device 목록을 기준으로 gate한다. 활성 desktop device가 없으면 이전 room, 청결도, 파일 목록, 메인 navigation을 렌더링하지 않고 pairing code 입력과 진행 상태만 표시한다. claim 성공 event 또는 REST 확인이 끝나면 메인 화면으로 전환한다.

활성 device가 이미 있으면 설정에는 새 PC 페어링 진입을 노출하지 않고 현재 PC의 `페어링 끊기`만 표시한다. claim API도 동일 사용자에게 두 번째 활성 device를 만들지 않는다.

### 9.1.1 기기 페어링 해제

```text
모바일 시작: DELETE /v1/devices/:id
데스크톱 시작: DELETE /v1/agent/devices/self
→ 서버가 device REVOKED + 연결 room REMOVED를 한 transaction으로 저장
→ 진행 중 transfer/cache reservation 취소와 삭제 job 생성
→ device.revoked / room.removed sync event 기록·즉시 publish
→ 서버가 해당 device socket을 강제 disconnect
→ 모바일은 메인 navigation과 과거 room cache 제거 후 Pairing Gate 표시
→ 데스크톱은 token·room binding을 지우고 새 pairing code 화면 표시
```

- 요청 중 양쪽은 `DISCONNECTING` 캐릭터 애니메이션과 “연결을 정리하는 중” 문구를 표시한다.
- 정상 요청 목표 응답 시간은 2초 이내다. 3초가 지나면 계속 대기 중임을 표시하되 성공으로 간주하지 않는다.
- socket event가 유실되면 2초 간격의 짧은 상태 확인을 최대 10초 수행하고, 이후 일반 sync replay로 복구한다.
- revoke 완료 뒤 수동 새로고침을 요구하지 않는다.

### 9.1.2 폴더 연결 해제

```text
모바일 시작: DELETE /v1/rooms/:id
데스크톱 시작: DELETE /v1/agent/rooms/:id
→ 서버 room REMOVED + 관련 요청 취소 + 삭제 job 생성
→ room.removed event 즉시 publish
→ A가 해당 managed root watcher/index binding 해제
→ 모바일 room 목록·Drift cache에서 즉시 제거
→ 데스크톱 managed folder 목록에서 즉시 제거
```

폴더 연결 해제는 원본 폴더나 사용자 파일을 삭제하지 않는다. 로컬 index·watcher·서버 metadata 연결만 해제하며 `.mousekeeper_trash`에 남은 복구 가능 작업의 처리 여부는 별도 안내한다.

## 9.2 모바일 명령 → 제안 → 승인 → 실행

```text
1. 모바일에서 room과 명령 입력
2. B가 command draft를 사용자에게 확인
3. commands에 QUEUED 저장
4. command.available event
5. A 데스크톱이 command 조회
6. A가 로컬 index와 규칙으로 대상 분석
7. A가 proposal과 precondition 생성
8. B 서버가 proposal 저장
9. 모바일에 proposal 표시
10. 사용자가 승인 또는 거절
11. decision을 idempotency key와 함께 저장
12. A가 승인 내역 재조회
13. 파일 precondition 재검증
14. operation journal 기록
15. 파일 실행 및 검증
16. execution result 동기화
17. room snapshot 재계산
18. B가 모바일과 캐릭터에 완료 상태 표현
```

## 9.3 PC 오프라인 명령

```text
1. Heartbeat TTL 만료
2. 모바일 집의 불이 꺼짐
3. 사용자가 명령 입력
4. command는 QUEUED로 PostgreSQL에 저장
5. 모바일에 “PC가 연결되면 확인할게요” 표시
6. 같은 room의 P1 캐시 화면에 pending-command warning 표시
7. 데스크톱 재접속
8. last cursor 이후 미처리 command 조회
9. 순서대로 분석
10. proposal 생성 후 FCM 알림
11. proposal 또는 command 종료 뒤 warning 갱신
```

PC가 오프라인인 동안에는 어떤 파일이 proposal 대상이 될지 확정할 수 없다. 따라서 캐시 파일을 숨기거나 임의로 `삭제 예정`이라고 표시하지 않고, room 단위 경고만 노출한다. 캐시 다운로드는 허용하되 `이 대기 명령이 처리되면 파일 이름·위치·존재 여부가 달라질 수 있습니다`라는 안내를 함께 보여준다.

## 9.4 README 생성·수정

```text
1. 모바일 채팅에서 방 목적 질문
2. 사용자가 목적 설명
3. B가 README draft 생성
4. 모바일 또는 데스크톱에서 diff 미리보기
5. 사용자가 승인
6. A가 기존 파일 hash와 수정 시각 재검증
7. operation journal 기록
8. README 생성 또는 수정
9. 결과 동기화
10. undo 시 이전 내용을 복구
```

## 9.5 되돌리기

```text
1. 사용자가 데스크톱 작업 기록에서 Undo 선택
2. A가 operation과 item 상태 조회
3. 복구 목적지 충돌 확인
4. 역순으로 원래 위치 복구
5. operation = ROLLED_BACK
6. 서버 execution 상태 동기화
7. 모바일 활동 기록 갱신
```

## 9.6 온라인 파일 요청·가져오기 P0

```text
1. 모바일이 room의 Files 탭을 연다.
2. 서버가 desktop에 browse.available event를 보낸다.
3. desktop이 REST로 BrowseRequest를 조회한다.
4. 관리 루트와 relative path를 검증하고 한 페이지 목록을 반환한다.
5. 다음 페이지 요청 중 PC 연결이 끊기면 request = DEVICE_OFFLINE 또는 TIMED_OUT으로 종료한다.
6. 모바일은 이미 받은 페이지를 유지하되 `일부 목록만 표시 중` fallback을 보여준다.
7. 모바일이 파일을 선택하고 가져오기를 누른다.
8. 서버가 FileTransfer = REQUESTED 저장 후 desktop에 알린다.
9. desktop이 실제 파일 버전과 크기를 재검증한다.
10. desktop이 만료형 object로 chunk upload한다.
11. server가 checksum과 크기를 확인하고 READY로 전환한다.
12. mobile이 다운로드하고 checksum을 검증한다.
13. mobile ACK 또는 TTL 만료 후 object를 삭제한다.
```

PC가 오프라인이면 새 원본 전송을 큐에 장기 보관하지 않고 `DEVICE_OFFLINE`을 반환한다. 사용자는 P1 캐시본이 있을 때만 오프라인 파일을 받을 수 있다. Pagination 재시도는 첫 페이지부터 강제 초기화하지 않고 마지막 성공 cursor를 사용할 수 있지만, desktop index generation이 바뀌었으면 `CURSOR_INVALIDATED`로 새 탐색을 시작한다.

## 9.7 사용 빈도 기반 오프라인 스마트 캐시 P1

```text
1. 사용자가 room 설정에서 스마트 캐시를 켜고 quota를 확인한다.
2. desktop이 file_access_events를 집계해 usage_score를 계산한다.
3. 높은 점수 파일과 manual pin 파일을 로컬 후보로 만든다.
4. desktop이 크기·버전·점수 metadata만 `POST /v1/agent/cache-candidates`로 제출한다.
5. server가 현재 cache 사용량과 reservation을 기준으로 최종 UploadTarget 목록을 반환한다.
6. desktop은 승인된 target만 암호화 upload한다.
7. server가 checksum·크기·source version을 검증하고 reservation을 확정한 뒤 availability = AVAILABLE로 저장한다.
8. PC 연결이 끊기면 freshness = UNVERIFIED_OFFLINE으로 표현하고 mobile의 Cached files에서 다운로드한다.
9. room에 QUEUED command가 있으면 캐시 목록에 `처리 후 목록 변경 가능` warning을 표시한다.
10. desktop watcher가 원본 변경을 감지하면 로컬에서 freshness = STALE을 기록하고 sync_outbox에 적재한다.
11. 인터넷 단절로 STALE 동기화가 지연되면 mobile은 기존 AVAILABLE cache와 함께 `마지막 원본 확인 시각`을 표시해 최신본으로 단정하지 않는다.
12. 재연결 후 STALE event를 먼저 동기화하고, 새 버전 target 승인·upload가 완료되면 이전 object를 삭제한다.
13. quota 초과 시 server가 낮은 usage_score와 오래 미사용한 항목부터 퇴출한다.
```

후보 제출과 upload target 발급 사이에는 파일 원문 전송이 없다. Server quota reservation이 만료된 target은 업로드를 시작하지 않으며, 업로드 도중 만료되면 완료 API에서 재검증 후 거절하고 임시 object를 삭제한다.

---

# 10. 공통 계약과 상태 머신

## 10.1 첫날 동결할 모델

- `Device`
- `Room`
- `ManagedRootSummary`
- `RuleDefinition`
- `Command`
- `Proposal`
- `ProposalItem`
- `Decision`
- `ExecutionResult`
- `Presence`
- `RoomSnapshot`
- `CleanlinessFormulaVersion`
- `ConnectionState`
- `CharacterEvent`
- `FileBrowseRequest`
- `FileBrowsePage`
- `FileTransfer`
- `SmartCachePolicy`
- `CachedFileMetadata`
- `CacheCandidateBatch`
- `CacheUploadTarget`

## 10.2 명령 상태

```text
QUEUED
→ DELIVERED
→ ANALYZING
→ PROPOSAL_READY
→ WAITING_APPROVAL
├─ APPROVED
├─ REJECTED
└─ EXPIRED

APPROVED
→ EXECUTING
├─ SUCCEEDED
├─ PARTIALLY_SUCCEEDED
├─ FAILED
└─ STALE
```

## 10.3 로컬 operation 상태

```text
PLANNED
→ PRECONDITION_CHECKED
→ JOURNALED
→ APPLIED
→ VERIFIED
→ RESULT_QUEUED
→ SYNCED

실패 분기:
FAILED_BEFORE_APPLY
FAILED_AFTER_PARTIAL_APPLY
ROLLBACK_REQUIRED
ROLLED_BACK
```

## 10.4 온라인 파일 탐색 상태

```text
REQUESTED
→ DELIVERED
→ QUERYING
→ READY

실패·종료:
DEVICE_OFFLINE
TIMED_OUT
EXPIRED
CURSOR_INVALIDATED
OUTSIDE_MANAGED_ROOT
```

- 각 pagination cursor 요청은 독립 상태를 가진다.
- `DEVICE_OFFLINE` 또는 `TIMED_OUT`이면 이전 READY page는 유지하지만 완전한 최신 목록으로 표시하지 않는다.
- desktop index generation이 달라져 cursor 일관성을 보장할 수 없으면 `CURSOR_INVALIDATED`로 새 탐색을 요구한다.

## 10.5 파일 전송 상태

```text
REQUESTED
→ SOURCE_VALIDATING
→ UPLOADING
→ READY
→ DOWNLOADING
├─ COMPLETED
├─ EXPIRED
├─ CANCELLED
└─ FAILED

검증 실패:
DEVICE_OFFLINE
SOURCE_NOT_FOUND
SOURCE_CHANGED
OUTSIDE_MANAGED_ROOT
SIZE_LIMIT_EXCEEDED
CHECKSUM_MISMATCH
```

## 10.6 스마트 캐시 상태 P1

```text
CANDIDATE_LOCAL
→ CANDIDATES_SUBMITTED
→ TARGETS_APPROVED
→ UPLOADING
→ AVAILABLE
├─ EVICTING
├─ INVALIDATED
└─ DELETED

후보·예약 실패:
REJECTED_QUOTA
REJECTED_POLICY
RESERVATION_EXPIRED
UPLOAD_FAILED

별도 freshness 상태:
VERIFIED_CURRENT
UNVERIFIED_OFFLINE
STALE
```

`AVAILABLE + UNVERIFIED_OFFLINE`은 다운로드는 가능하지만 원본 최신 여부는 마지막 확인 시각 이후 보장되지 않는다는 뜻이다. `STALE`은 알려진 구버전이며 최신본처럼 기본 선택하지 않는다.

## 10.7 이벤트 envelope

```json
{
  "eventId": "uuid",
  "eventType": "proposal.created",
  "aggregateType": "proposal",
  "aggregateId": "uuid",
  "deviceId": "uuid",
  "roomId": "uuid",
  "sequence": 183,
  "occurredAt": "2026-07-10T06:00:00.000Z",
  "payload": {}
}
```

- `eventId`는 중복 수신 제거에 사용한다.
- `sequence`는 device 단위 replay 순서를 나타낸다.
- 클라이언트는 처리한 마지막 sequence를 저장한다.
- socket 수신 후에도 필요한 원문은 REST로 조회한다.

## 10.8 연결 해제와 화면 gate 상태

```text
UNPAIRED
→ PAIRING
→ PAIRED
→ DISCONNECTING
→ UNPAIRED

오류 분기:
PAIRING_FAILED
DISCONNECT_FAILED
```

- `PAIRED`는 서버에 현재 사용자 소유의 `ACTIVE` desktop device가 존재하고 인증된 연결 정보를 복구할 수 있는 상태다.
- `DISCONNECTING`에서는 중복 해제 요청을 막고 캐릭터 진행 애니메이션을 표시한다.
- `device.revoked`를 받으면 room cache를 먼저 숨기고 Pairing Gate로 전환한다.
- `room.removed`는 해당 room만 제거하며 다른 활성 device/room이 있으면 메인 화면을 유지한다.
- 로컬 cache는 서버 event sequence보다 앞선 데이터를 다시 활성 상태로 복원할 수 없다.

## 10.9 청결도 snapshot 동일성

- A의 Rust 엔진을 청결도 계산의 단일 source of truth로 사용한다.
- snapshot에 `formulaVersion`을 추가하고 초기값은 `mousekeeper-cleanliness-v1`로 고정한다.
- 점수, 파일 수, 차감 사유 code·count·points, 계산 시각을 하나의 snapshot으로 원자적으로 저장한다.
- 데스크톱과 모바일은 자체 재계산하지 않고 같은 snapshot 필드를 표시한다.
- 모바일은 알 수 없는 차감 code를 숨기지 않고 일반 문구와 원래 code를 표시한다.

## 10.10 파일 이름 검색

- `FileBrowseRequest`에 선택적 `query`와 `searchScope`를 additive 필드로 추가한다.
- `query`는 trim 후 1~100자, `searchScope`는 `CURRENT_DIRECTORY | MANAGED_ROOT`다.
- 검색 대상은 파일·폴더 이름이며 파일 내용 검색은 하지 않는다.
- A는 canonical managed root 안의 index에서 대소문자 비구분 substring 검색을 수행한다.
- 결과는 기존 `FileBrowsePage` entry와 cursor를 재사용하고 최대 200개씩 반환한다.
- 검색 결과 다운로드도 기존 FileTransfer 흐름을 그대로 사용하며 요청 시 source identity와 경계를 다시 검증한다.

---

# 11. 데이터베이스 설계

## 11.1 PostgreSQL 핵심 테이블

### `users`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 사용자 ID |
| `auth_provider_uid` | varchar | UNIQUE | Firebase 사용자 ID |
| `display_name` | varchar | NOT NULL | 표시 이름 |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |
| `deleted_at` | timestamptz | NULL | 계정 비활성화 |

### `devices`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 장치 ID |
| `user_id` | uuid | FK users | 소유자 |
| `platform` | varchar | CHECK | WINDOWS, MACOS, ANDROID, IOS |
| `device_name` | varchar | NOT NULL | 사용자 표시 이름 |
| `public_key` | text | NULL | 데스크톱 공개키 |
| `status` | varchar | NOT NULL | ACTIVE, REVOKED |
| `last_seen_at` | timestamptz | NULL | 마지막 heartbeat |
| `created_at` | timestamptz | NOT NULL | 등록 시각 |

권장 인덱스: `(user_id, status)`, `(last_seen_at)`

### `pairing_sessions`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 세션 ID |
| `desktop_nonce` | varchar | UNIQUE | 데스크톱 요청 식별자 |
| `pairing_code_hash` | varchar | UNIQUE | 코드 hash |
| `claimed_by_user_id` | uuid | FK users, NULL | claim 사용자 |
| `expires_at` | timestamptz | NOT NULL | 만료 |
| `claimed_at` | timestamptz | NULL | 완료 시각 |

### `rooms`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 방 ID |
| `user_id` | uuid | FK users | 소유자 |
| `desktop_device_id` | uuid | FK devices | 실제 root를 가진 PC |
| `name` | varchar | NOT NULL | 방 이름 |
| `root_alias` | varchar | NOT NULL | Downloads 같은 별칭, 절대 경로 아님 |
| `status` | varchar | NOT NULL | ACTIVE, PAUSED, REMOVED |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |

권장 인덱스: `(desktop_device_id, status)`, `(user_id, created_at)`

### `rules`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 규칙 ID |
| `room_id` | uuid | FK rooms | 적용 방 |
| `name` | varchar | NOT NULL | 규칙 이름 |
| `definition` | jsonb | NOT NULL | Rule DSL |
| `priority` | int | NOT NULL | 평가 순서 |
| `enabled` | boolean | NOT NULL | 활성 여부 |
| `version` | int | NOT NULL | optimistic concurrency |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |
| `updated_at` | timestamptz | NOT NULL | 수정 시각 |

권장 인덱스: `(room_id, enabled, priority)`

### `commands`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 명령 ID |
| `room_id` | uuid | FK rooms | 대상 방 |
| `target_device_id` | uuid | FK devices | 대상 데스크톱 |
| `created_by_user_id` | uuid | FK users | 생성자 |
| `intent` | varchar | NOT NULL | SCAN, CREATE_RULE, ANALYZE, README 등 |
| `payload` | jsonb | NOT NULL | 구조화 명령 |
| `status` | varchar | NOT NULL | 명령 상태 |
| `idempotency_key` | varchar | UNIQUE | 중복 생성 방지 |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |
| `delivered_at` | timestamptz | NULL | 전달 시각 |
| `finished_at` | timestamptz | NULL | 종료 시각 |

권장 인덱스: `(target_device_id, status, created_at)`

### `proposals`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 제안 ID |
| `command_id` | uuid | FK commands | 원 명령 |
| `room_id` | uuid | FK rooms | 대상 방 |
| `status` | varchar | NOT NULL | OPEN, APPROVED, REJECTED, EXPIRED |
| `summary` | jsonb | NOT NULL | 파일 수, 충돌 수, 예상 영향 |
| `expires_at` | timestamptz | NULL | 승인 만료 |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |

### `proposal_items`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 항목 ID |
| `proposal_id` | uuid | FK proposals | 소속 제안 |
| `item_order` | int | NOT NULL | 표시 순서 |
| `action_type` | varchar | NOT NULL | MOVE, QUARANTINE, CREATE_DIR, README_WRITE |
| `source_relative_path` | text | NULL | 절대 경로 제외 |
| `destination_relative_path` | text | NULL | 절대 경로 제외 |
| `reason_code` | varchar | NOT NULL | 규칙/감점 이유 |
| `precondition` | jsonb | NOT NULL | file ID, size, mtime, hash |
| `conflict_state` | varchar | NOT NULL | NONE, NAME_CONFLICT, UNSUPPORTED |

권장 인덱스: `(proposal_id, item_order)`

### `decisions`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 결정 ID |
| `proposal_id` | uuid | FK proposals | 대상 제안 |
| `user_id` | uuid | FK users | 결정 사용자 |
| `decision_type` | varchar | NOT NULL | APPROVE, REJECT |
| `approved_item_ids` | jsonb | NOT NULL | 부분 승인 확장용 |
| `idempotency_key` | varchar | UNIQUE | 중복 승인 방지 |
| `created_at` | timestamptz | NOT NULL | 결정 시각 |

### `executions`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 실행 ID |
| `proposal_id` | uuid | FK proposals | 제안 |
| `decision_id` | uuid | FK decisions | 승인 |
| `desktop_device_id` | uuid | FK devices | 실행 PC |
| `status` | varchar | NOT NULL | EXECUTING, SUCCEEDED, PARTIAL, FAILED, STALE, ROLLED_BACK |
| `result_summary` | jsonb | NULL | 항목별 결과 집계 |
| `idempotency_key` | varchar | UNIQUE | 실행 중복 방지 |
| `started_at` | timestamptz | NULL | 시작 |
| `finished_at` | timestamptz | NULL | 종료 |

### `file_browse_requests`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 탐색 요청 ID |
| `room_id` | uuid | FK rooms | 대상 방 |
| `desktop_device_id` | uuid | FK devices | 조회 PC |
| `relative_directory` | text | NOT NULL | 절대 경로 제외 |
| `cursor` | varchar | NULL | pagination cursor |
| `status` | varchar | NOT NULL | REQUESTED, DELIVERED, QUERYING, READY, EXPIRED, FAILED |
| `failure_code` | varchar | NULL | DEVICE_OFFLINE, TIMED_OUT, CURSOR_INVALIDATED 등 |
| `desktop_generation` | varchar | NULL | pagination 일관성 확인용 index generation |
| `expires_at` | timestamptz | NOT NULL | 목록 응답 만료 |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |

### `file_transfers`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | 전송 ID |
| `room_id` | uuid | FK rooms | 출처 방 |
| `desktop_device_id` | uuid | FK devices | 원본 보유 PC |
| `requested_by_user_id` | uuid | FK users | 요청 사용자 |
| `source_relative_path` | text | NOT NULL | 절대 경로 제외 |
| `source_version` | jsonb | NOT NULL | file ID, size, mtime, hash |
| `status` | varchar | NOT NULL | transfer 상태 |
| `object_key` | text | NULL | 공개 URL이 아닌 내부 key |
| `size_bytes` | bigint | NULL | 검증된 크기 |
| `sha256` | varchar | NULL | 전체 checksum |
| `expires_at` | timestamptz | NOT NULL | object 만료 |
| `completed_at` | timestamptz | NULL | 모바일 검증 완료 |
| `idempotency_key` | varchar | UNIQUE | 중복 요청 방지 |
| `created_at` | timestamptz | NOT NULL | 생성 시각 |

권장 인덱스: `(desktop_device_id, status, created_at)`, `(expires_at, status)`

### `smart_cache_policies`, `cache_upload_reservations`, `cached_files` P1

| 테이블 | 핵심 필드 | 설명 |
|---|---|---|
| `smart_cache_policies` | `room_id UNIQUE`, `enabled`, `quota_bytes`, `max_file_bytes`, `excluded_patterns`, `updated_at` | 방별 opt-in 정책 |
| `cache_upload_reservations` | `id`, `room_id`, `desktop_device_id`, `source_relative_path`, `source_version`, `reserved_bytes`, `status`, `expires_at`, `object_key` | 후보 승인 뒤 실제 upload 전에 quota를 임시 확보하는 원장 |
| `cached_files` | `room_id`, `source_relative_path`, `source_version`, `usage_score`, `manual_pin`, `object_key`, `size_bytes`, `availability_status`, `freshness_status`, `cached_at`, `last_verified_at`, `last_accessed_at` | 오프라인 사용 가능한 캐시 원장과 최신 확인 상태 |

중요 제약:

- `(room_id, source_relative_path, source_version_hash)` unique
- quota 계산은 `AVAILABLE` object와 유효한 `RESERVED` byte를 모두 포함
- reservation은 `(room_id, source_relative_path, source_version_hash)` 기준으로 중복 발급하지 않음
- `object_key`는 API 응답에 직접 노출하지 않음
- policy disabled 또는 device revoked 시 reservation 취소와 삭제 job 생성
- 같은 상대 경로의 `AVAILABLE` 버전은 하나만 유지
- `AVAILABLE`과 `VERIFIED_CURRENT`를 동일한 의미로 취급하지 않음

### `room_snapshots`

| 필드 | 타입 | 제약 | 설명 |
|---|---|---|---|
| `id` | uuid | PK | snapshot ID |
| `room_id` | uuid | FK rooms | 방 |
| `score` | int | CHECK 0..100 | 청결도 |
| `metrics` | jsonb | NOT NULL | 감점 지표와 파일 수 |
| `calculated_at` | timestamptz | NOT NULL | 계산 시각 |

권장 인덱스: `(room_id, calculated_at DESC)`

### `chat_messages`, `character_profiles`, `affinity_events`

| 테이블 | 핵심 필드 | 설명 |
|---|---|---|
| `chat_messages` | `room_id`, `sender_type`, `content`, `command_id`, `created_at` | 방별 채팅과 명령 연결 |
| `character_profiles` | `user_id UNIQUE`, `appearance`, `room_theme`, `affinity_total` | 현재 캐릭터 상태 |
| `affinity_events` | `character_profile_id`, `event_type`, `delta`, `source_execution_id`, `created_at` | append-only 호감도 원장 |

### `sync_events`와 `audit_events`

- `sync_events`: device별 sequence를 가진 replay 가능한 경량 event log
- `audit_events`: 로그인, pairing, 승인, 실행, undo, 권한 변경 기록
- 파일 원문과 절대 경로는 audit payload에 저장하지 않음

## 11.2 Desktop SQLite 핵심 테이블

| 테이블 | 핵심 필드 | 설명 |
|---|---|---|
| `managed_roots` | `id`, `room_id`, `canonical_path`, `display_name`, `enabled` | 허용 경로 원장 |
| `file_index` | `root_id`, `relative_path`, `file_id`, `size`, `mtime`, `extension`, `is_dir` | 로컬 파일 metadata |
| `file_index_fts` | `name`, `relative_path` | 로컬 검색 |
| `local_rules` | `rule_id`, `room_id`, `definition_json`, `version` | 서버 규칙 cache |
| `scan_jobs` | `id`, `root_id`, `status`, `progress`, `error_code` | 스캔 상태 |
| `operation_journal` | `id`, `proposal_id`, `decision_id`, `status`, `started_at`, `finished_at` | 작업 transaction header |
| `operation_items` | `journal_id`, `source`, `destination`, `precondition`, `result` | 파일별 전후 상태 |
| `trash_items` | `operation_item_id`, `original_path`, `trash_path`, `restored_at` | 복구 정보 |
| `file_access_events` | `root_id`, `relative_path`, `event_type`, `occurred_at` | 모바일 요청·앱 내 열람 같은 관찰 가능한 사용 이벤트 |
| `file_transfer_jobs` | `id`, `room_id`, `relative_path`, `source_version`, `status`, `sha256`, `expires_at` | P0 로컬 전송 상태 |
| `cache_candidates` | `root_id`, `relative_path`, `usage_score`, `manual_pin`, `source_version`, `size_bytes`, `server_decision`, `reservation_id`, `target_expires_at`, `status` | P1 로컬 후보, 서버 승인 결과와 target 만료 상태 |
| `sync_outbox` | `id`, `event_type`, `payload`, `attempt_count`, `next_retry_at` | 서버 전송 실패 재시도 |
| `sync_cursors` | `stream`, `last_sequence` | 누락 event replay |
| `settings` | `key`, `value_json` | 오버레이와 앱 설정 |

중요 제약:

- `managed_roots.canonical_path`는 unique
- parent/child overlap은 애플리케이션과 DB transaction 모두에서 확인
- `file_index(root_id, relative_path)` unique
- `operation_journal(proposal_id)` unique
- `sync_outbox`는 성공 ACK 전까지 삭제하지 않음

## 11.3 Mobile Drift 최소 테이블

- `cached_devices`
- `cached_rooms`
- `cached_commands`
- `cached_room_snapshots`
- `cached_proposals`
- `cached_chat_messages`
- `cached_file_browse_pages`
- `cached_file_transfers`
- `cached_offline_files` P1
- `mutation_outbox`
- `sync_cursor`

모바일은 원본 파일 원장 역할을 하지 않는다. 앱 내부 metadata cache를 지워도 서버에서 복구 가능해야 하며, 실제로 내려받은 파일은 사용자가 선택한 모바일 저장 위치의 일반 파일로 취급한다.

---

# 12. REST API와 WebSocket 이벤트

## 12.1 핵심 REST API

| Method | Endpoint | 호출 주체 | 설명 |
|---|---|---|---|
| POST | `/v1/pairing-sessions` | Desktop | pairing 코드 생성 |
| POST | `/v1/pairing-sessions/claim` | Mobile | 로그인 사용자와 PC 연결 |
| GET | `/v1/devices` | Mobile | 내 장치 목록 |
| DELETE | `/v1/devices/:id` | Mobile | device와 연결 room 페어링 해제 |
| DELETE | `/v1/agent/devices/self` | Desktop | 현재 device의 페어링 해제 |
| POST | `/v1/devices/:id/heartbeat` | Desktop | heartbeat와 작업 상태 |
| POST | `/v1/rooms` | Desktop | 로컬 root 등록 후 방 metadata 생성 |
| GET | `/v1/rooms` | Mobile/Desktop | 방 목록 |
| DELETE | `/v1/rooms/:id` | Mobile | 폴더 연결 해제; 원본 파일은 유지 |
| DELETE | `/v1/agent/rooms/:id` | Desktop | managed root 연결 해제 완료 보고 |
| POST | `/v1/rooms/:id/rules` | Mobile/Desktop | 구조화된 규칙 생성 |
| PATCH | `/v1/rules/:id` | Mobile/Desktop | 규칙 수정 |
| POST | `/v1/rooms/:id/commands` | Mobile | 원격 명령 생성 |
| GET | `/v1/devices/:id/commands/pending` | Desktop | 미처리 명령 조회 |
| POST | `/v1/agent/proposals` | Desktop | 제안 저장 |
| GET | `/v1/proposals/:id` | Mobile/Desktop | 제안 상세 |
| POST | `/v1/proposals/:id/decisions` | Mobile | 승인·거절 |
| GET | `/v1/devices/:id/decisions/pending` | Desktop | 실행 대상 승인 조회 |
| POST | `/v1/agent/executions` | Desktop | 실행 시작 |
| PATCH | `/v1/agent/executions/:id` | Desktop | 결과·오류·rollback 반영 |
| POST | `/v1/rooms/:id/snapshots` | Desktop | 청결도 snapshot 저장 |
| POST | `/v1/rooms/:id/file-browse-requests` | Mobile | 온라인 파일 목록 요청 |
| GET | `/v1/devices/:id/file-browse-requests/pending` | Desktop | 미처리 목록 요청 조회 |
| POST | `/v1/agent/file-browse-requests/:id/result` | Desktop | 페이지 단위 파일 목록 반환 |
| POST | `/v1/rooms/:id/file-transfers` | Mobile | 파일 가져오기 session 생성 |
| GET | `/v1/devices/:id/file-transfers/pending` | Desktop | 전송 요청 조회 |
| POST | `/v1/agent/file-transfers/:id/upload-target` | Desktop | 검증 후 일회성 upload target 요청 |
| POST | `/v1/agent/file-transfers/:id/complete-upload` | Desktop | 크기·checksum과 업로드 완료 반영 |
| GET | `/v1/file-transfers/:id/download` | Mobile | 일회성 download target 발급 |
| POST | `/v1/file-transfers/:id/ack` | Mobile | checksum 검증 완료·삭제 예약 |
| DELETE | `/v1/file-transfers/:id` | Mobile/Desktop | 전송 취소 |
| GET/PATCH | `/v1/rooms/:id/smart-cache-policy` | Mobile | P1 방별 스마트 캐시 설정 |
| POST | `/v1/agent/cache-candidates` | Desktop | P1 후보 metadata 제출; 서버가 quota reservation과 승인된 UploadTarget 목록 반환 |
| POST | `/v1/agent/cache-uploads/:reservationId/complete` | Desktop | P1 checksum·크기·버전 검증 후 reservation 확정 및 AVAILABLE 전환 |
| DELETE | `/v1/agent/cache-uploads/:reservationId` | Desktop | P1 취소·실패 target 해제와 임시 object 삭제 예약 |
| GET | `/v1/rooms/:id/cached-files` | Mobile | P1 오프라인 파일 목록, freshness와 pending-command warning metadata 포함 |
| DELETE | `/v1/cached-files/:id` | Mobile | P1 개별 캐시 제거 |
| GET | `/v1/sync/events` | Mobile/Desktop | cursor 이후 event replay |
| GET/POST | `/v1/rooms/:id/chat` | Mobile | 채팅 조회·생성 |
| GET/PATCH | `/v1/character` | Mobile | 캐릭터 상태·꾸미기 |

## 12.2 WebSocket 이벤트

| Event | 발행 | 수신 | 의미 |
|---|---|---|---|
| `presence.updated` | Server | Mobile | PC 상태 변경 |
| `device.paired` | Server | Mobile/Desktop | pairing 완료와 화면 gate 갱신 |
| `device.revoked` | Server | Mobile/Desktop | 기기 연결 즉시 해제와 cache 제거 |
| `room.removed` | Server | Mobile/Desktop | 폴더 연결 즉시 해제와 목록 제거 |
| `command.available` | Server | Desktop | 새 command 조회 필요 |
| `command.updated` | Server | Mobile | 분석 상태 변경 |
| `proposal.created` | Server | Mobile | 새 제안 조회 필요 |
| `decision.created` | Server | Desktop | 승인/거절 조회 필요 |
| `execution.updated` | Server | Mobile | 실행 결과 조회 필요 |
| `file.browse.requested` | Server | Desktop | 온라인 파일 목록 조회 필요 |
| `file.browse.ready` | Server | Mobile | 목록 페이지 조회 가능 |
| `file.browse.failed` | Server | Mobile | DEVICE_OFFLINE, TIMED_OUT, CURSOR_INVALIDATED 등 탐색 실패 |
| `file.transfer.requested` | Server | Desktop | 원본 전송 요청 조회 필요 |
| `file.transfer.updated` | Server | Mobile/Desktop | 업로드·다운로드·만료 상태 변경 |
| `smart-cache.updated` | Server | Mobile/Desktop | P1 캐시 상태·quota 변경 |
| `room.snapshot.updated` | Server | Mobile | 청결도 갱신 |
| `chat.message.created` | Server | Mobile | 새 채팅 메시지 |
| `character.event` | Server/Desktop | Mobile/Desktop overlay | 캐릭터 상태 표현 |

## 12.3 멱등성

다음 요청은 반드시 `Idempotency-Key`를 사용한다.

- command 생성
- proposal 저장
- decision 생성
- execution 시작
- execution result 저장
- affinity event 생성
- file transfer 생성과 upload 완료
- smart cache candidate batch 등록, upload reservation 확정·취소

같은 key의 요청은 기존 결과를 반환하고 파일 작업이나 동일 object 업로드를 다시 수행하지 않는다.

---

# 13. 보안과 개인정보

## 13.1 인증

- 모바일은 Firebase ID token 사용
- 서버는 Firebase Admin SDK로 검증
- 데스크톱은 pairing 완료 뒤 device-scoped token 사용
- device token과 private key는 OS keychain 저장
- 기기 해제 시 token 즉시 revoke

## 13.2 경로 보안

- canonical path 기준 검증
- 상대 경로 normalize
- `..` 제거 후에도 root 내부인지 재검사
- symlink/junction/reparse point 기본 차단
- UNC와 network drive 쓰기 차단
- 시스템 보호 폴더 등록 차단
- destination은 실행 직전에 다시 canonicalize

## 13.3 데이터 최소화

- 절대 경로를 서버와 로그에 전송하지 않음
- 전체 파일 인덱스를 서버에 전송하지 않음
- 파일 원문은 P0 만료형 transfer 또는 P1 opt-in 스마트 캐시 외에는 서버에 업로드하지 않음
- 활성 proposal, 짧은 수명의 browse page, transfer/cache metadata에 필요한 상대 경로만 저장
- 로그에 대화 원문, 파일명, 경로를 metric label로 사용하지 않음

## 13.4 파일 전송과 스마트 캐시 보안

- transfer/cache API는 해당 room과 desktop device를 소유한 동일 사용자만 호출 가능
- 데스크톱은 서버가 보낸 상대 경로를 신뢰하지 않고 매번 local room mapping과 canonical path를 검증
- P0 upload/download target은 transfer ID, object key, content length, 짧은 만료에 묶음
- object storage bucket은 public access를 차단하고 server-side encryption을 기본 적용
- 가능하면 파일별 random data key를 사용하고 key material과 object를 분리
- 모바일은 다운로드 후 SHA-256이 일치하지 않으면 파일을 완료 처리하지 않음
- P1 캐시 활성화 전 저장 범위, 예상 용량, 서버 보관 사실을 사용자에게 표시
- credential/key/secret로 분류되는 파일과 앱 내부 DB는 기본 자동 캐시 제외
- device revoke, account deletion, room removal, cache disable 시 object 삭제를 보장하는 tombstone job 사용
- 캐시 파일명·상대 경로는 application log와 metric label에 남기지 않음

## 13.5 개발·테스트 원칙

- 실제 파일 테스트는 `/test-fixtures` 아래 전용 root에서만 수행
- 생산 코드에서 fake success를 반환하지 않음
- 외부 provider가 미설정이면 `UNCONFIGURED` 오류를 명확히 표시
- simulator는 `/tools`에 두고 릴리스 build에서 제외
- 테스트 fixture는 허용하지만 사용자 데이터처럼 보이는 seed를 운영 DB에 넣지 않음

---

# 14. 개발 Phase와 분업

## Phase 0 — 계약·범위·파일 안전 POC

### 목표

파일 엔진의 최소 안전성과 두 클라이언트가 공유할 계약을 먼저 증명한다.

### A

- Tauri + Rust skeleton
- 테스트 root 등록
- `move_no_overwrite`
- `.mousekeeper_trash` 이동
- operation journal 초안
- undo POC
- path traversal와 overlap 차단
- `file-engine-cli` 작성

### B

- NestJS + Flutter skeleton
- PostgreSQL·Drizzle migration 기반
- Firebase auth skeleton
- Command/Proposal/Decision/Execution DTO
- FileBrowse/FileTransfer DTO와 독립 상태 머신 초안
- OpenAPI와 event schema 초안
- 모바일 주요 화면 wireframe
- deterministic desktop-agent simulator

### 공동 산출물

- `docs/mvp-scope.md`
- `docs/file-safety-invariants.md`
- `packages/contracts/openapi.yaml`
- `packages/contracts/events/*.schema.json`
- E2E 시나리오 3개
- 온라인 파일 전달 threat scenario 2개

### Exit Criteria

- 테스트 파일을 격리하고 되돌릴 수 있음
- 루트 밖 이동이 자동 차단됨
- 동일 operation 재호출이 두 번 실행되지 않음
- A와 B가 같은 JSON fixture로 개발 가능
- 파일 변경 contract와 읽기 전용 transfer contract가 분리됨

---

## Phase 1 — 로그인·페어링·Presence

### A

- device key pair와 OS keychain
- pairing QR/코드 표시
- device token 저장
- heartbeat sender
- 재접속과 backoff
- 데스크톱 시작 device self-revoke와 token 즉시 폐기
- `DISCONNECTING` bridge와 pairing-only 화면 전환
- 트레이와 앱 생명주기

### B

- 로그인
- pairing session API
- device 등록·해제
- Socket.IO 인증
- Redis/Valkey TTL presence
- 모바일 기기 목록
- 온라인·오프라인 집 표현
- 모바일 시작 device revoke
- `device.paired`·`device.revoked` 즉시 publish와 대상 socket disconnect
- 활성 device가 없을 때 pairing-only navigation gate
- revoke 뒤 room·Drift cache 자동 제거

### 통합 산출물

```text
모바일 로그인
→ QR 스캔
→ 데스크톱 연결
→ 모바일 집 불 켜짐
→ 앱 종료
→ 15초 이내 연결 끊김 표시
→ 한쪽에서 연결 해제
→ 양쪽이 2초 목표로 pairing 화면 전환
```

### Exit Criteria

- 다른 사용자 계정이 device를 claim할 수 없음
- revoke된 device token으로 연결 불가
- 재연결 후 presence가 정상 회복
- 양쪽 어디서 revoke해도 수동 새로고침 없이 반영
- 과거 연결 room이 새 pairing 화면이나 새 device의 room 목록에 노출되지 않음

---

## Phase 2 — 관리 폴더·스캔·청결도

### A

- managed root 등록·해제
- overlap 검사
- SQLite file index
- 전체 scan
- watcher와 reconcile
- 파일 조회 화면
- 모바일용 cursor 기반 파일 목록 조회 adapter
- managed root 내부 파일·폴더 이름 검색 adapter
- 데스크톱 시작 managed root 연결 해제와 watcher/index 정리
- file_access_events 저장 기반
- 청결도 raw metric과 점수

### B

- Room API와 모바일 방 목록
- Room detail 화면
- Files 탭과 online/offline/empty 상태
- scan progress 표시
- room snapshot 저장
- 청결도 등급과 방 그래픽
- 기본 캐릭터 room 배치
- 모바일 시작 room 연결 해제와 목록/cache 즉시 제거
- 동일 snapshot·formulaVersion 기반 청결도 표시

### 통합 산출물

- 데스크톱에서 폴더를 등록하면 모바일에 방이 생성됨
- 폴더 변경 뒤 청결도가 갱신됨
- 여러 방을 전환해도 상태가 섞이지 않음

### Exit Criteria

- 관리 루트 개수 하드 제한 없음
- parent/child 중복 등록 차단
- 앱 재시작 후 index와 방 연결 복구
- 모바일 목록 응답에 절대 경로가 포함되지 않음
- 데스크톱과 모바일의 점수·차감 사유·formulaVersion이 동일함
- 양쪽 어디서 room을 해제해도 원본 파일은 유지되고 목록에서 자동 제거됨
- 이름 검색이 managed root 경계를 벗어나지 않고 결과 파일 다운로드로 이어짐

---

## Phase 3 — 규칙·원격 명령·제안

### A

- Rule DSL parser/evaluator
- extension, ageDays, nameContains 조건
- MOVE, QUARANTINE, CREATE_DIR action plan
- proposal item과 precondition 생성
- 충돌 탐지
- 데스크톱 rule 설정 화면

### B

- 모바일 규칙 생성·수정 UI
- Rule API와 version 관리
- 채팅 command draft
- Command durable queue
- Proposal API와 모바일 proposal 목록·상세
- `command.available`, `proposal.created` event

### 통합 산출물

```text
모바일에서 규칙 저장
→ 데스크톱 동기화
→ 로컬 분석
→ proposal 생성
→ 모바일에서 파일별 이유 확인
```

### Exit Criteria

- AI 없이도 규칙 버튼과 form으로 전 흐름 사용 가능
- AI 출력이 invalid하면 command로 저장되지 않음
- 같은 파일이 한 proposal에서 중복되지 않음

---

## Phase 4 — 승인·실행·휴지통·Undo·README·온라인 파일 전달

### A

- 승인 조회
- precondition 재검증
- operation journal 완성
- move/rename/create-dir/quarantine
- 충돌 처리
- `.mousekeeper_trash` 화면
- undo
- crash recovery
- README read/hash/diff/write/undo
- FileTransfer source validation, chunk upload, SHA-256
- 전송 취소·TTL 만료 시 local cleanup
- 작업 기록 화면

### B

- 승인·거절 API
- 모바일 proposal 승인 화면
- execution API와 상태 표시
- 결과 알림
- README 질문·초안·diff UX
- FileTransfer API, 만료형 object lifecycle
- 모바일 다운로드·진행률·checksum UX
- audit summary

### 통합 산출물

```text
모바일 승인
→ 데스크톱 안전 실행
→ 모바일 완료 표시
→ 데스크톱 Undo
→ 모바일 ROLLED_BACK 표시

모바일 파일 선택
→ 데스크톱 버전 재검증
→ 만료형 object upload
→ 모바일 checksum 검증 다운로드
→ object 삭제
```

### Exit Criteria

- 승인 없이 파일 쓰기 0건
- 기존 파일 덮어쓰기 0건
- 승인 뒤 변경된 파일은 STALE 처리
- crash 후 journal로 상태 판단 가능
- 온라인 파일 전달이 정리 operation 상태를 변경하지 않음
- 전송 완료·취소·만료 뒤 임시 object가 남지 않음
- 파일 목록 pagination 중 PC 연결이 끊기면 `DEVICE_OFFLINE` 또는 `TIMED_OUT`으로 종료되고 이전 page만 fallback 표시

---

## Phase 5 — 캐릭터·집·채팅·연결 대기 표현

### A

- 투명 오버레이 native window
- always-on-top
- 클릭 통과
- 화면 경계 고정과 위치 저장
- `CharacterEvent` bridge
- overlay 표시 모드와 배터리 절약

### B

- Rive 캐릭터 상태 머신
- 데스크톱 overlay UI
- 모바일 집과 방 완성
- 채팅 UI와 대화 템플릿
- 제한된 AI 명령 parser
- affinity 숫자와 완료 대사
- 고정 기본 외형·테마
- pairing·disconnecting 대기 애니메이션

### 통합 산출물

```text
scan → ANALYZING
proposal → WAITING_APPROVAL
execution → WORKING
success → SUCCESS
failure → ERROR
pairing/disconnecting → CONNECTING
idle → IDLE
```

### Exit Criteria

- 캐릭터 UI가 파일 command를 직접 호출할 수 없음
- 애니메이션을 완전히 끌 수 있음
- 호감도가 파일 권한에 영향을 주지 않음
- 호감도 상승으로 외형·accessory·방 테마가 변경되거나 해금되지 않음

---

## Phase 6 — 오프라인 큐·멀티 폴더·재접속

### A

- desktop inbox/outbox
- sync cursor
- 순서 보장
- 중복 event 제거
- 여러 root의 watcher 격리
- PC당 scan/write concurrency queue
- transfer concurrency queue와 취소 복구

### B

- command replay API
- event sequence log
- FCM 최소 알림
- 모바일 mutation outbox
- 여러 방의 pending badge
- P1 cached files 화면의 room 단위 pending-command warning
- 서버 재시작 뒤 queue 복구
- transfer TTL 삭제 worker와 orphan object sweep

### Exit Criteria

- PC 오프라인 명령이 유실되지 않음
- 재접속 후 한 번만 처리
- 여러 방의 제안과 실행 결과가 섞이지 않음
- 오프라인 전송 요청은 자동 장기 대기하지 않고 명확히 실패
- 만료 transfer를 재접속 후 잘못 재개하지 않음
- 오프라인 cached files가 대기 명령 존재 여부를 숨기지 않음

---

## Phase 7 — 하드닝·빌드·폐쇄형 베타

### A

- Windows installer
- 자동 시작
- updater 준비
- 대규모 폴더 성능 측정
- watcher overflow 복구
- junction/symlink 공격 테스트
- 로그 개인정보 제거
- 대용량·네트워크 중단·checksum mismatch 전송 테스트
- macOS adapter compile skeleton

### B

- Android release build
- server production deploy
- DB backup
- rate limit
- 기기 revoke
- Sentry와 dashboard
- onboarding·오류 UX
- FCM 실기기 테스트
- object storage private policy, lifecycle, orphan sweep 검증

### 공동 Exit Criteria

- 전체 release blocker 통과
- 테스트 PC와 Android에서 E2E 시연
- 데이터 삭제·기기 해제 확인
- 장애 시 복구 절차 문서화

---

## Phase 8 — P1 사용 빈도 기반 스마트 캐시

Phase 8은 P0 MVP 출시 뒤 진행하며, 기존 정리·전송 흐름을 변경하지 않는 독립 기능 플래그로 구현한다.

### A

- `file_access_events` 집계와 usage score
- manual pin/exclude
- cache candidate versioning과 metadata batch 제출
- 서버가 승인한 UploadTarget만 암호화 upload
- reservation 만료·취소·실패 처리
- watcher 기반 STALE/INVALIDATED 로컬 기록과 sync_outbox 우선 동기화
- quota 거절 후보 로컬 상태 표시

### B

- room별 opt-in policy와 설명 UX
- cache candidate 심사와 quota reservation transaction
- cache metadata/object lifecycle
- quota와 LRU eviction worker
- cached files 오프라인 화면
- availability/freshness/cached-at/last-verified-at 분리 표시
- pending command가 있는 room의 목록 변경 warning
- revoke·disable·delete tombstone 처리

### Exit Criteria

- 기능 기본값이 꺼져 있음
- 자주 사용한 파일이 낮은 점수 파일보다 우선 캐시됨
- quota 승인 전 파일 암호화·upload가 시작되지 않음
- 동시 후보 제출에서도 AVAILABLE + RESERVED 합계가 quota를 넘지 않음
- 만료된 reservation이 quota를 계속 점유하지 않음
- 캐시본의 다운로드 가능 여부와 원본 최신 확인 여부를 모바일에서 구분 가능
- PC 오프라인 수정으로 STALE 동기화가 지연돼도 마지막 확인 시각이 명확히 표시됨
- pending command가 있는 room에서 파일 목록 변경 가능 warning이 표시됨
- PC 오프라인에서 availability가 AVAILABLE인 파일만 다운로드 가능
- 기능 해제 후 관련 object가 정해진 시간 안에 삭제됨
- 스마트 캐시 장애가 command/proposal/execution을 막지 않음

---

# 15. 10영업일 MadCamp 압축 일정

이 일정은 발표 가능한 데모 MVP다. 전체 제품 모델은 유지하되 실제 구현 범위를 강하게 줄인다.

## 15.1 압축 범위

### 유지

- 사용자 1명
- PC 1대
- 데이터 모델은 여러 방 지원
- 발표 시 관리 폴더 1개 사용
- 규칙 1종: 확장자 또는 오래된 파일
- 작업 1종: `.mousekeeper_trash` 격리
- 전체 승인·거절
- undo
- heartbeat
- 오프라인 command queue
- 캐릭터 상태 4~6개
- 모바일 방·제안 화면
- PC 온라인 상태에서 파일 목록 조회와 20MB 이하 파일 1개 가져오기

### 제거

- 부분 승인
- 복잡한 규칙 조합
- README 수정, 필요하면 생성만
- 자유 대화
- 호감도 해금 체계
- 커스터마이징 상점
- FCM, 필요하면 앱 열린 상태 socket만
- P1 사용 빈도 기반 스마트 캐시
- 폴더 단위 zip과 대용량 resume
- macOS/iOS build

## 15.2 일자별 분업

| Day | A — Desktop Agent | B — Product & Cloud | 통합 결과 |
|---:|---|---|---|
| 1 | Tauri/Rust/SQLite skeleton, 테스트 root | NestJS/Flutter/Postgres skeleton, contract | 세 앱 실행, 공통 DTO 확정 |
| 2 | 폴더 선택, scan, file index, paginated query | 로그인 최소 흐름, Room API/Files UI | 모바일에 방과 온라인 파일 목록 표시 |
| 3 | watcher, cleanliness, heartbeat sender | presence TTL, 집 불 표현 | 파일 변화와 PC 상태 표시 |
| 4 | 규칙 evaluator, proposal fixture | Command/Proposal API, 모바일 명령 | 명령이 proposal로 변환 |
| 5 | precondition, journal, quarantine | 승인 UI, Decision API | 승인 뒤 파일 격리 |
| 6 | undo, history, conflict 처리, 20MB chunk upload | Execution 결과, FileTransfer API/UI | 격리·되돌리기와 온라인 파일 다운로드 |
| 7 | overlay shell, CharacterEvent | Rive/캐릭터 UI, 모바일 방 | 작업에 캐릭터 반응 |
| 8 | local outbox, reconnect, transfer cancel | offline queue, replay, TTL cleanup | PC 오프라인 명령 복구와 임시 object 정리 |
| 9 | 안전성·crash·build 테스트 | 인증·오류 UX·서버 배포 | release candidate |
| 10 | Windows build와 데모 안정화 | Android build와 발표 흐름 | E2E 발표 |

## 15.3 발표 시나리오

```text
1. 모바일에서 PC 온라인 확인
2. PC 앱 종료 후 집 불 꺼짐 확인
3. 모바일에서 PDF 정리 명령 생성
4. PC 재실행
5. proposal 생성과 캐릭터 분석 모션
6. 모바일에서 파일 목록 확인 후 승인
7. 파일이 .mousekeeper_trash로 이동
8. 청결도 상승과 완료 모션
9. 데스크톱 기록에서 Undo
10. 원래 위치로 복구
11. PC가 온라인인 상태에서 모바일 Files 탭 열기
12. 테스트 파일을 요청해 모바일로 다운로드
13. checksum 완료와 임시 object 삭제 확인
```

---

# 16. 담당자별 상세 백로그

## 16.1 A 우선순위

### A-P0 안전 기반

1. managed root canonicalization
2. overlap, traversal, symlink/junction 차단
3. SQLite migration
4. operation journal
5. no-overwrite move
6. quarantine와 undo
7. crash recovery

### A-P1 파일 관리

1. scan과 watcher
2. file index와 조회
3. Rule DSL
4. proposal/precondition
5. README diff/write
6. 청결도 계산
7. 작업 기록 UI
8. online file browse adapter
9. FileTransfer validation/chunk/checksum

### A-P2 플랫폼

1. pairing bridge
2. heartbeat와 socket client
3. sync outbox/cursor
4. Tauri overlay shell
5. system tray/autostart
6. Windows build
7. macOS platform adapter

### A-P3 스마트 캐시 P1

1. file access event aggregation
2. usage score
3. manual pin/exclude
4. cache candidate versioning
5. cache upload와 stale invalidation

## 16.2 B 우선순위

### B-P0 cloud control plane

1. auth
2. devices/pairing
3. commands/proposals/decisions/executions
4. durable queue와 replay
5. presence TTL
6. idempotency와 audit
7. FileTransfer session과 object lifecycle
8. server deploy

### B-P1 product experience

1. pairing-only connection gate와 disconnect 진행 상태
2. 모바일 home/house
3. active room 목록과 쌍방 room 연결 해제
4. 동일 cleanliness snapshot 표시
5. proposal review와 approval/reject
6. execution result와 online/offline 표현
7. chat command UI
8. online Files browser, 이름 검색, verified download UX

### B-P2 character

1. Rive state machine
2. desktop overlay UI
3. cleanliness visualization
4. affinity ledger
5. 고정 기본 appearance/theme
6. pairing·disconnecting 진행 animation
7. minimal notification
8. Android release build

### B-P3 스마트 캐시 P1

1. opt-in policy UI
2. encrypted object metadata
3. quota/LRU worker
4. offline cached files UI
5. revoke/disable deletion tombstone

## 16.3 공동 우선순위

1. contract schema
2. state machine
3. error code catalog
4. E2E fixture
5. threat model
6. file transfer threat model과 lifecycle test
7. smart cache scoring/privacy ADR
8. release checklist

---

# 17. 병목 방지 방식

## 17.1 개발 전용 도구

### A: `file-engine-cli`

입력 JSON으로 로컬 fixture를 분석하고 proposal/result JSON을 출력한다.

```bash
file-engine-cli analyze fixtures/command.json
file-engine-cli execute fixtures/decision.json --root test-fixtures/basic
file-engine-cli rollback <operation-id>
file-engine-cli browse fixtures/browse-request.json --root test-fixtures/basic
file-engine-cli transfer fixtures/transfer-request.json --root test-fixtures/basic --output /tmp/chunks
file-engine-cli cache-score --root test-fixtures/basic
```

### B: `desktop-agent-simulator`

서버의 command를 받아 고정된 contract에 맞는 proposal과 execution result를 반환한다.

- production build에 포함하지 않음
- 실제 성공처럼 운영 로그를 만들지 않음
- contract test와 UI 병렬 개발에만 사용
- browse page, transfer progress, checksum mismatch, smart-cache metadata fixture를 제공

## 17.2 상호 의존 차단

- B는 A의 scanner가 완성되기 전에 simulator로 모바일 승인 화면 개발
- A는 B 서버가 완성되기 전에 CLI fixture로 파일 엔진 개발
- 계약 변경은 `packages/contracts` PR 한 곳에서만 수행
- DTO 변경 PR은 A와 B 모두 승인해야 merge 가능
- B는 실제 파일 없이 simulator의 encrypted dummy object로 다운로드 UI를 개발
- A는 실제 object storage 없이 local mock upload target으로 chunk/checksum을 검증

---

# 18. 협업 운영 규칙

## 18.1 일일 리듬

```text
09:30 15분: 오늘의 통합 목표와 blocker
13:00 contract 변경 여부 확인
18:00 오늘의 vertical slice demo
```

각자 작업량 보고가 아니라 “오늘 사용자 흐름이 어디까지 연결됐는가”를 기준으로 공유한다.

## 18.2 PR 규칙

- PR은 하나의 기능 또는 하나의 contract 변경만 포함
- 파일 작업 로직 PR에는 fixture test 필수
- API 변경 PR에는 OpenAPI/event schema 변경 필수
- migration은 되돌릴 수 있는지 설명
- UI PR에는 loading, empty, error, offline 상태 포함
- secret과 절대 경로가 log에 포함되지 않는지 확인
- 파일 전송 PR에는 TTL 삭제, checksum, 취소, 크기 제한 테스트 포함
- 스마트 캐시 PR에는 opt-in, quota, stale 표시, 삭제 보장 테스트 포함
- merge 전 상대 담당자 리뷰 1회 필수

## 18.3 브랜치

```text
main       = 항상 배포 가능한 상태
develop    = 통합 테스트
feat/a-*   = A 기능
feat/b-*   = B 기능
hotfix/*   = 출시 장애 수정
```

장기 브랜치를 피하고 하루 또는 이틀 단위로 작은 vertical slice를 통합한다.

## 18.4 ADR

다음 결정은 `docs/adr`에 남긴다.

- 파일 identity 방식
- root overlap 정책
- operation journal 복구 방식
- command replay와 sequence
- 청결도 공식 변경
- AI provider 변경
- DB major version 또는 ORM 변경
- P0 transfer relay와 object lifecycle 방식
- P1 usage score, quota, eviction, encryption 정책

---

# 19. Definition of Done

## 19.1 공통 DoD

기능은 다음을 모두 만족해야 완료다.

- 정상 경로 동작
- 빈 상태
- 오프라인 상태
- 중복 요청
- 권한 없음
- 재시작 복구
- 구조화 로그
- unit/contract/integration test
- 문서와 schema 업데이트

## 19.2 A 기능 DoD

- 등록 root 밖 파일을 읽거나 변경하지 않음
- 기존 파일을 덮어쓰지 않음
- journal 없이 쓰기 작업을 시작하지 않음
- 작업 실패 후 실제 파일 상태를 확인할 수 있음
- undo 가능 여부가 명확히 표시됨
- 앱 종료 후 상태 복구 가능
- 데스크톱 청결도와 서버에 보낸 snapshot이 동일함
- device/room 해제 event 뒤 token·binding·watcher가 자동 정리됨
- 파일 이름 검색과 다운로드가 managed root 경계를 우회하지 않음

## 19.3 B 기능 DoD

- 모든 중요 상태가 DB에 영속 저장됨
- socket 유실 후 REST replay 가능
- 중복 요청이 멱등 처리됨
- 모바일에 loading/error/offline 표시가 있음
- 로그아웃·기기 revoke가 동작함
- 캐릭터 상태가 실제 domain event에서 파생됨
- unpaired 상태에서는 pairing code 화면 외 메인 데이터를 표시하지 않음
- device/room 해제가 반대편에 자동 반영되고 removed cache가 재등장하지 않음
- 모바일 청결도가 데스크톱 snapshot과 동일함
- 호감도 변화로 외형·방 테마가 변하지 않음

## 19.4 파일 접근 기능 DoD

- 파일 목록과 전송 요청이 인증된 동일 사용자·등록 room에만 허용됨
- 데스크톱이 요청 시점마다 managed root와 source version을 재검증함
- 파일 변경 상태 머신과 FileTransfer 상태 머신이 독립적임
- 모바일 checksum 검증 전에는 다운로드 완료로 표시하지 않음
- 완료·취소·실패·만료 object가 lifecycle worker로 삭제됨
- PC 오프라인, source changed, size limit, network interruption 상태가 구분됨
- P1은 opt-in이며 캐시 시각·최신 확인 시각·availability/freshness 상태를 분리 표시함
- P1 upload는 서버 quota reservation과 UploadTarget 승인 후에만 시작됨
- P1 cached files는 같은 room의 QUEUED command 존재 시 목록 변경 가능 warning을 표시함
- P1 비활성화·room 삭제·device revoke 후 object 삭제를 확인할 수 있음

---

# 20. 테스트 전략

## 20.1 A 테스트

- Rust unit test: path guard, rule evaluator, collision naming
- SQLite integration: migration, journal, outbox
- fixture E2E: scan → proposal → execute → undo
- crash simulation: JOURNALED, APPLIED 시점 강제 종료
- Windows junction/reparse point 공격 테스트
- watcher overflow와 reconcile 테스트
- browse traversal와 out-of-root 요청 테스트
- cleanliness formulaVersion과 desktop/server snapshot 동일성 테스트
- Desktop 시작 device/room 해제와 watcher/token 정리 테스트
- filename search root boundary, generation cursor, symlink/junction 차단 테스트
- 전송 중 source change, chunk retry, checksum mismatch 테스트
- usage score와 cache stale invalidation P1 테스트
- quota 승인 전 암호화·upload 미실행, reservation expiry, 승인 target 외 업로드 차단 테스트
- 로컬 원본 수정 후 네트워크 단절 시 STALE outbox 및 last-verified 표시 테스트

## 20.2 B 테스트

- server unit: 상태 전이, 권한, idempotency
- DB integration: transaction, queue replay, sequence
- WebSocket contract test
- Flutter widget test: offline/empty/error/proposal
- pairing security test
- 모바일 시작·Desktop 시작 revoke와 즉시 event/replay test
- unpaired Pairing Gate와 removed room cache purge widget test
- 동일 cleanliness snapshot 표시 widget test
- 파일 이름 검색 debounce·stale response 폐기·download 연결 test
- affinity 변경 뒤 appearance/theme 불변 test
- FCM device test
- transfer auth, signed target expiry, lifecycle delete test
- smart-cache quota, concurrent reservation, expiry release, eviction, revoke deletion P1 test
- pending command warning과 AVAILABLE/UNVERIFIED_OFFLINE/STALE 표시 widget test
- browse pagination 중 DEVICE_OFFLINE, TIMED_OUT, CURSOR_INVALIDATED fallback test

## 20.3 공동 E2E

반드시 자동화하거나 반복 가능한 script로 만든다.

1. 온라인 command
2. 오프라인 command와 재접속
3. 승인 중복 전송
4. 승인 후 파일 변경 → STALE
5. 목적지 이름 충돌
6. 작업 중 데스크톱 강제 종료
7. undo 충돌
8. root overlap 등록
9. device revoke
10. 서버 재시작 후 queue 복구
11. 온라인 파일 탐색과 1회 다운로드
12. 등록 root 밖 상대 경로 요청 차단
13. 전송 도중 원본 변경 → SOURCE_CHANGED
14. checksum mismatch → 다운로드 실패
15. transfer ACK/TTL 뒤 object 삭제
16. P1 높은 사용 점수 우선 캐시
17. P1 원본 수정 → STALE 표시
18. P1 device revoke → cache object 삭제
19. P1 후보 제출 → server quota 승인 target만 upload
20. P1 동시 reservation → quota 초과 방지
21. P1 원본 수정 + network offline → 마지막 확인 시각과 UNVERIFIED/STALE 방어 표시
22. PC 오프라인 + QUEUED command + cached files → 목록 변경 warning
23. P0 pagination 중 연결 끊김 → 이전 page 유지 + DEVICE_OFFLINE fallback

---

# 21. 출시 차단 조건

다음 중 하나라도 발생하면 MVP를 배포하지 않는다.

1. 승인 없이 파일 쓰기가 발생한다.
2. 관리 루트 밖 파일을 읽거나 변경할 수 있다.
3. symlink, junction, reparse point로 경계를 우회할 수 있다.
4. 기존 파일을 자동 덮어쓴다.
5. 동일 승인으로 작업이 두 번 실행된다.
6. journal 기록 전에 파일 변경을 시작한다.
7. 앱 종료 후 작업 완료 여부를 판단할 수 없다.
8. 휴지통 복구가 충돌을 숨긴다.
9. PC 오프라인 command가 유실된다.
10. socket event 유실 뒤 상태를 복구할 수 없다.
11. 서버 로그에 절대 경로나 파일 원문이 남는다.
12. 캐릭터 또는 AI가 파일 안전 정책을 우회한다.
13. device revoke 후에도 데스크톱이 명령을 받는다.
14. 여러 방의 proposal이 다른 방에 표시된다.
15. 모바일이 실제 실행 전 성공으로 표시한다.
16. 다른 사용자·다른 room의 파일을 탐색하거나 다운로드할 수 있다.
17. 파일 전달이 managed root 또는 symlink/junction 경계를 우회한다.
18. checksum 불일치 파일을 정상 다운로드로 처리한다.
19. 완료·취소·실패·만료된 P0 transfer object가 삭제되지 않는다.
20. P0 임시 object가 캐시처럼 장기간 유지된다.
21. P1 활성화 전 사용자 동의 없이 원본 파일을 업로드한다.
22. P1 캐시본을 최신 원본으로 오인하게 표시한다.
23. device revoke 또는 cache disable 뒤 P1 object 삭제가 실행되지 않는다.
24. 서버 upload target 승인 전에 P1 파일 암호화·업로드를 시작한다.
25. AVAILABLE object와 유효 reservation 합계가 방별 quota를 초과한다.
26. PC 오프라인 상태에서 캐시본을 원본 최신 확인 완료로 표시한다.
27. 같은 room에 QUEUED command가 있는데 cached files 화면이 목록 변경 가능성을 알리지 않는다.
28. 파일 탐색 pagination 중 연결 단절 요청이 무기한 대기하거나 완전한 최신 목록처럼 표시된다.

---

# 22. 일정이 밀릴 때 자르는 순서

| 순서 | 제거 기능 | 유지할 대체 |
|---:|---|---|
| 1 | 커스터마이징 | 캐릭터 1종, 테마 1종 |
| 2 | 호감도 해금 | 완료 대사만 제공 |
| 3 | 자유 채팅 | 규칙 버튼과 명령 template |
| 4 | README 수정 | README 생성만 제공 |
| 5 | 부분 승인 | 전체 승인·거절 |
| 6 | FCM | 앱 열린 상태 socket 알림 |
| 7 | 복합 규칙 | 확장자 또는 기간 규칙 하나 |
| 8 | 여러 방 발표 | 한 방 시연, 데이터 모델은 N개 유지 |
| 9 | 파일 미리보기 종류 확대 | 일반 다운로드만 유지 |
| 10 | 대용량 resume | 20MB 이하 단일 파일 전송 유지 |

P1 스마트 캐시는 MVP 이후 범위이므로 일정이 밀리면 전체를 연기할 수 있다. 다만 P0 온라인 파일 가져오기는 아래 최소 형태를 유지한다.

절대 자르지 않는 기능:

- 명시적 managed root
- 제안과 사용자 승인
- 실행 직전 재검증
- no-overwrite
- operation journal
- `.mousekeeper_trash`
- undo
- idempotency
- 오프라인 명령 영속 저장
- PC 온라인 상태의 읽기 전용 파일 1개 가져오기
- transfer 경로 재검증, checksum, TTL 삭제

---

# 23. 환경 변수와 배포

## 23.1 Server

```env
NODE_ENV=
DATABASE_URL=
REDIS_URL=
WEB_ORIGIN=
JWT_OR_DEVICE_TOKEN_SECRET=
FIREBASE_PROJECT_ID=
FIREBASE_CLIENT_EMAIL=
FIREBASE_PRIVATE_KEY=
FCM_ENABLED=
AI_PROVIDER=
AI_API_KEY=
AI_MODEL=
SENTRY_DSN=
OBJECT_STORAGE_ENDPOINT=
OBJECT_STORAGE_REGION=
OBJECT_STORAGE_BUCKET=
OBJECT_STORAGE_ACCESS_KEY_ID=
OBJECT_STORAGE_SECRET_ACCESS_KEY=
FILE_TRANSFER_MAX_BYTES=104857600
FILE_TRANSFER_TTL_SECONDS=600
SMART_CACHE_ENABLED=false
SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES=524288000
SMART_CACHE_DEFAULT_MAX_FILE_BYTES=52428800
```

- `.env.example`에는 key만 둔다.
- 실제 secret은 Render secret 또는 로컬 `.env.local`로 주입한다.
- provider가 비어 있으면 fake 성공을 만들지 않는다.
- object storage가 미설정이면 P0 파일 가져오기를 `UNCONFIGURED`로 표시하고 서버 로컬 디스크에 영구 저장하는 fallback을 만들지 않는다.
- P1 스마트 캐시는 feature flag 기본값을 false로 둔다.

## 23.2 Desktop

- 서버 base URL은 build config로 주입
- device token은 OS keychain
- 사용자 절대 경로는 로그와 crash report에서 제거
- updater signing key는 CI secret으로 관리
- file transfer 임시 chunk와 key material은 완료·취소·실패 후 로컬에서 정리
- file access event 원문은 로컬 SQLite에만 저장하고 서버에는 집계 점수만 전송

## 23.3 배포 책임

| 산출물 | 담당 |
|---|---|
| Windows installer | A |
| macOS skeleton/build | A |
| Android APK/AAB | B |
| iOS skeleton/build | B |
| API/Worker/DB/Redis | B |
| E2E release checklist | 공동 |

---

# 24. 첫날 생성할 파일

```text
docs/mvp-scope.md
docs/file-safety-invariants.md
docs/threat-model.md
docs/e2e-scenarios.md
docs/adr/0001-local-first.md

packages/contracts/openapi.yaml
packages/contracts/events/event-envelope.schema.json
packages/contracts/events/presence.schema.json
packages/contracts/events/command.schema.json
packages/contracts/events/proposal.schema.json
packages/contracts/events/decision.schema.json
packages/contracts/events/execution-result.schema.json
packages/contracts/events/file-browse.schema.json
packages/contracts/events/file-transfer.schema.json
packages/contracts/events/smart-cache.schema.json
packages/contracts/smart-cache-policy.schema.json
packages/contracts/rule-dsl.schema.json
packages/contracts/fixtures/

test-fixtures/file-trees/basic/
test-fixtures/file-trees/conflicts/
test-fixtures/file-trees/nested-root/
test-fixtures/file-transfers/basic/
test-fixtures/file-transfers/source-changed/
test-fixtures/file-transfers/checksum-mismatch/
test-fixtures/smart-cache/usage-events/

CODEOWNERS
README.md
```

`CODEOWNERS` 예시:

```text
/apps/desktop/src-tauri/             @A
/apps/desktop/src/features/files/    @A
/apps/desktop/src/features/admin/    @A
/apps/desktop/src/features/overlay/  @A @B
/apps/desktop/src/features/character/ @B
/apps/mobile/                        @B
/apps/server/                        @B
/infra/                              @B
/packages/contracts/                 @A @B
```

---

# 25. 최종 실행 우선순위

가장 먼저 완성해야 하는 vertical slice는 다음 하나다.

```text
모바일 command 생성
→ 서버 영속 저장
→ 데스크톱 command 조회
→ 로컬 분석
→ proposal 생성
→ 모바일 승인
→ 파일 precondition 재검증
→ journal 기록
→ .mousekeeper_trash 이동
→ 결과 동기화
→ Undo
```

이 흐름이 완성되기 전에는 상점, 복잡한 호감도, 미니게임, 강한 자아를 구현하지 않는다.

그다음 완성할 두 번째 P0 vertical slice는 기존 정리 흐름을 건드리지 않는 읽기 전용 파일 전달이다.

```text
모바일 Files 탭
→ 온라인 데스크톱 페이지 조회
→ 파일 가져오기 요청
→ 관리 루트와 source version 재검증
→ 만료형 object upload
→ 모바일 checksum 검증 다운로드
→ ACK/TTL object 삭제
```

P1 스마트 캐시는 위 두 P0 vertical slice가 안정화된 뒤 feature flag로 추가한다. 캐시 후보는 집쥐인이 관찰한 모바일 요청·앱 내 열람 빈도를 중심으로 산정하며, 전체 폴더 자동 동기화로 확대하지 않는다.

A는 이 vertical slice의 **로컬 안전성**을 책임지고, B는 같은 slice의 **사용자 경험과 클라우드 연속성**을 책임진다. 두 책임이 연결되어야만 하나의 기능이 완료된 것으로 간주한다.

---

# 26. v1.4 상세 실행 계획 — 청결도·연결 해제·모바일 파일 접근

이 섹션은 v1.4에서 추가된 7개 요구사항의 구현 순서다. 앞 절과 충돌하면 이 섹션의 결정이 우선한다.

## 26.1 Step 1 — 공통 계약을 먼저 고정

### 공동 계약 변경

- `RoomSnapshot`에 `formulaVersion`을 additive 필드로 추가한다.
- `CharacterState`에 연결 진행 표시 전용 `CONNECTING`을 additive 값으로 추가한다. 파일 작업 상태에는 사용하지 않는다.
- `FileBrowseRequest`에 nullable `query`와 `searchScope`를 추가한다.
- `Device`와 `Room` 공개 응답은 `ACTIVE` 항목만 기본 목록에 포함한다.
- `device.paired`, `device.revoked`, `room.removed` event payload에 aggregate id와 최종 상태를 포함한다.
- 연결 해제 mutation은 `Idempotency-Key`를 받고 동일 key 재요청에는 기존 최종 결과를 반환한다.

### 계약 완료 조건

- Zod/JSON Schema, OpenAPI, TypeScript export, Rust/Flutter DTO가 동일하다.
- 기존 browse 요청과 기존 snapshot을 읽을 수 있도록 신규 필드는 호환 가능한 기본 처리 경로를 가진다.
- 서버 DB 내부 user id, 절대 경로, device token은 event payload에 포함하지 않는다.

## 26.2 Step 2 — 청결도 규칙 단일화

### A

1. `cleanliness.rs`의 현재 공식을 `mousekeeper-cleanliness-v1`로 버전 고정한다.
2. scan/reconcile 완료 시 점수와 metrics를 한 번 계산한다.
3. 데스크톱 dashboard는 이 snapshot을 직접 표시한다.
4. 같은 객체를 서버 snapshot API에 전송한다.
5. 점수 차감 사유는 `UNORGANIZED_FILES`, `UNREADABLE_OR_UNSAFE_ENTRIES`, `PROPOSAL_CONFLICTS` code와 count·points를 유지한다.

### B

1. 서버는 snapshot을 재계산하지 않고 schema 검증 후 최신 room snapshot으로 저장한다.
2. `room.snapshot.updated` event에는 snapshot id 또는 room id만 전달하고 모바일이 REST 원문을 다시 조회하게 한다.
3. 모바일 room detail은 데스크톱과 같은 score, reason code, count, points, calculatedAt, formulaVersion을 표시한다.
4. 오래된 snapshot은 “마지막 계산 시각”을 표시하고 현재값인 것처럼 보이지 않게 한다.

### 테스트

- 동일 fixture에 대해 데스크톱 표시값, 서버 저장값, 모바일 표시값이 완전히 같은지 contract fixture로 확인한다.
- formulaVersion이 다르면 모바일이 조용히 섞지 않고 업데이트 필요 상태를 표시한다.
- watcher reconcile 뒤 snapshot sequence가 역행하지 않는지 확인한다.

## 26.3 Step 3 — 기기 페어링 쌍방 해제와 즉시 반영

### 서버·B

1. 모바일용 device revoke는 기존 `DELETE /v1/devices/:id` transaction을 재사용·보강한다.
2. Desktop agent가 자기 자신만 revoke할 수 있는 `DELETE /v1/agent/devices/self`를 추가한다.
3. transaction 안에서 device를 `REVOKED`, 연결 room을 `REMOVED`로 바꾸고 진행 중 transfer/cache reservation을 취소한다.
4. 삭제 대상 object에는 tombstone을 남긴다.
5. `device.revoked`와 각 `room.removed` sync event를 기록한 뒤 commit 후 즉시 publish한다.
6. Realtime gateway는 해당 `device:{id}` socket을 강제 disconnect한다.
7. 같은 device의 후속 heartbeat, pending command, browse, transfer 요청을 거절한다.

### A Desktop

1. 연결 설정에 “모바일 연결 끊기” 버튼과 확인 dialog를 제공한다.
2. 해제 요청 즉시 UI를 `DISCONNECTING`으로 바꾸고 CONNECTING animation을 재생한다.
3. 성공하면 keychain의 device token, 서버 room binding, socket session을 삭제한다.
4. 로컬 managed root 자체는 삭제하지 않되 서버와 연결된 room id는 제거한다.
5. 새 pairing session을 만들고 pairing code 화면으로 전환한다.

### B Mobile

1. device 목록에 “연결 끊기”를 제공한다.
2. 요청 중 중복 입력을 막고 CONNECTING animation과 진행 문구를 표시한다.
3. `device.revoked` 수신 즉시 해당 device와 room을 state 및 Drift cache에서 제거한다.
4. 활성 desktop device가 0개가 되면 navigation stack을 초기화하고 Pairing Gate로 이동한다.
5. socket event를 놓친 경우 sync replay와 짧은 상태 조회로 같은 결과를 복구한다.

### 지연 목표와 실패 처리

- 정상 API 응답과 event 반영 목표는 2초 이내다.
- pairing status polling은 1초 간격, 해제 fallback 확인은 2초 간격·최대 10초로 제한한다.
- heartbeat는 5초, TTL은 15초로 줄이되 정상 해제 확인에 사용하지 않는다.
- timeout이면 연결이 끊겼다고 가장하지 않고 `DISCONNECT_FAILED`와 재시도 버튼을 표시한다.

## 26.4 Step 4 — Pairing Gate와 과거 데이터 격리

### 모바일 gate 규칙

1. 로그인 완료 뒤 `GET /v1/devices`와 `GET /v1/rooms`를 먼저 복구한다.
2. 활성 desktop device가 없으면 pairing code 입력 화면만 렌더링한다.
3. Home, Rooms, Files, Rules, Proposals route에 진입 guard를 둔다.
4. claim 성공 뒤 `device.paired` 또는 REST 결과를 확인한 후에만 메인 navigation을 생성한다.
5. loading 중에는 이전 메인 화면을 잠깐 보여주지 않고 pairing/loading 전용 화면을 사용한다.

### 과거 room 제거 규칙

- revoke transaction에서 기존 device의 room을 `REMOVED`로 전환한다.
- 모바일은 removed room의 snapshot, browse page, transfer display cache를 삭제한다.
- 새 device가 같은 로컬 경로를 다시 등록해도 새 room id로 취급한다.
- 서버의 기본 room 목록은 `ACTIVE`만 반환하므로 과거 폴더가 재등장하지 않는다.
- offline cache에 과거 room이 남아 있어도 최신 sync sequence가 제거 상태를 덮어쓰지 못하게 한다.

### 테스트

- 최초 설치, 로그아웃 후 재로그인, revoke 직후, 앱 강제 종료 후 재실행 상태를 widget/integration test로 확인한다.
- unpaired 상태에서 deep link로 room/files route에 접근해도 Pairing Gate로 이동하는지 확인한다.

## 26.5 Step 5 — 폴더 연결의 쌍방 해제

### 서버·B

1. 모바일용 `DELETE /v1/rooms/:id`와 Desktop용 `DELETE /v1/agent/rooms/:id`를 구현한다.
2. 모바일 요청은 소유 사용자와 active room을, Desktop 요청은 room에 연결된 정확한 agent device를 검증한다.
3. room을 `REMOVED`로 전환하고 browse request, transfer, cache reservation을 취소한다.
4. P0/P1 object 삭제 job을 생성하고 `room.removed`를 기록·publish한다.
5. 요청은 멱등 처리하여 이미 removed인 room 재요청이 파일 삭제나 중복 job을 만들지 않게 한다.

### A Desktop

1. managed folder 목록에 “연결 해제”를 제공한다.
2. 모바일에서 시작된 `room.removed` event를 받으면 watcher를 정지하고 room binding을 제거한다.
3. 로컬 SQLite index는 안전하게 삭제하거나 orphan 표식을 남긴 뒤 정리한다.
4. 실제 root와 내부 파일은 절대 삭제하지 않는다.
5. 미완료 journal 또는 undo 가능 작업이 있으면 해제 전 경고하고 정책에 따라 해제를 차단하거나 복구 안내를 표시한다.

### B Mobile

1. paired folder 목록과 room detail에 “폴더 연결 해제”를 제공한다.
2. 성공 event 수신 즉시 목록과 관련 cache를 갱신한다.
3. 마지막 room을 제거해도 device pairing은 유지하므로 메인 화면에서 “연결된 폴더 없음” 상태를 표시한다.
4. device까지 revoke된 경우에만 Pairing Gate로 이동한다.

### 완료 조건

- 어느 쪽에서 시작해도 양쪽 목록에서 2초 목표로 사라진다.
- 새로고침 없이 반영되고 앱 재시작 뒤 다시 나타나지 않는다.
- 원본 폴더와 파일은 그대로 유지된다.

## 26.6 Step 6 — 모바일 paired folder 목록·탐색·다운로드

### B Mobile

1. Home 또는 Rooms에 “연결된 폴더” 버튼을 제공한다.
2. `GET /v1/rooms`의 active room을 표시하고 선택하면 Files 화면으로 이동한다.
3. Files 화면은 breadcrumb, directory entry, 파일 크기·수정 시각, pagination을 제공한다.
4. directory 선택은 기존 browse request에 해당 `relativeDirectory`를 전달한다.
5. 파일 선택은 기존 FileTransfer session을 생성한다.
6. download target으로 `.part`에 받고 SHA-256 검증 뒤에만 최종 파일로 이동하고 ACK한다.
7. loading, empty, offline, timeout, cursor invalidation, source changed, checksum mismatch를 별도로 표시한다.

### A Desktop

1. pending browse request를 수신해 요청 시점마다 room과 managed root binding을 재검증한다.
2. relative path, symlink, junction, reparse point 경계 검사를 수행한다.
3. 파일 목록에는 상대 경로와 허용 metadata만 반환하고 절대 경로는 전송하지 않는다.
4. download 요청 시 source identity·size·modifiedAt을 다시 검증한다.
5. chunk upload, checksum, 취소, source-change 실패를 기존 transfer 흐름에 연결한다.

### 통합 완료 조건

- 실제 A agent가 반환한 폴더 내용만 모바일에서 보인다.
- 다른 사용자·room, removed room, root 밖 경로 요청은 거절된다.
- checksum 성공 전에는 모바일이 다운로드 완료를 표시하지 않는다.

## 26.7 Step 7 — 모바일 파일 이름 검색

### UX

- Files 화면 상단에 검색 버튼과 입력창을 둔다.
- 기본 scope는 현재 directory이며 사용자가 “전체 연결 폴더에서 검색”을 선택하면 `MANAGED_ROOT`를 사용한다.
- 300ms debounce 후 2자 이상 입력에서 요청한다. 빈 query는 일반 browse로 돌아간다.
- 검색 결과에 파일/폴더 구분과 상대 breadcrumb를 표시한다.
- 검색 결과 파일은 동일한 FileTransfer 흐름으로 다운로드한다.

### A 검색 adapter

- SQLite file index를 우선 사용하고, index generation을 cursor에 결합한다.
- filename case-insensitive substring으로 검색한다.
- root 경계를 벗어난 index entry와 symlink/junction 대상은 결과에서 제외한다.
- generation 변경 시 `CURSOR_INVALIDATED`를 반환해 모바일이 첫 page부터 다시 검색하게 한다.
- 서버나 모바일에 전체 파일 index를 영구 저장하지 않는다.

### B server/mobile

- 서버는 query 길이·scope·room 소유권·device online 상태를 검증한다.
- request와 결과는 기존 만료형 browse request lifecycle을 재사용한다.
- 모바일은 새 검색이 시작되면 이전 요청을 취소하거나 응답 request id가 다르면 폐기한다.
- 검색어와 파일 이름을 analytics 또는 일반 서버 로그에 남기지 않는다.

### 검색 완료 조건

- 현재 폴더 및 managed root 범위 이름 검색이 pagination과 함께 동작한다.
- 빠른 연속 입력에서 오래된 결과가 최신 결과를 덮어쓰지 않는다.
- 검색 결과 다운로드도 source 변경과 checksum 불일치를 안전하게 처리한다.

## 26.8 Step 8 — 호감도 기반 시각 변경 제거

### B Mobile/Server

- 모바일 Character 설정에서 appearance, accessory, room theme, unlocked item 선택 UI를 제거한다.
- Home/Room은 고정 기본 외형과 기본 테마만 렌더링한다.
- affinity 숫자, ledger, 완료 대사는 유지할 수 있다.
- affinity 증가 event가 appearance/theme mutation을 발생시키지 않는지 확인한다.
- 기존 사용자의 저장된 appearance/theme 값은 데이터 손실을 피하기 위해 DB에서 즉시 삭제하지 않되 MVP UI에서는 적용하지 않는다.
- 기존 `PATCH /v1/character` 계약은 즉시 파괴하지 않고 deprecated로 표시한 뒤 모바일 호출을 제거한다.

### 완료 조건

- 호감도가 변해도 캐릭터 외형과 방 테마가 변하지 않는다.
- 파일 권한·규칙·우선순위와 affinity 사이 의존성이 없다.
- pairing/disconnecting animation은 affinity와 무관한 일시적 connection state로만 동작한다.

## 26.9 구현 순서와 PR 단위

1. `contract/v1.4-connection-cleanliness-browse`: snapshot version, connection event, search request, CONNECTING state
2. `backend/device-room-disconnect`: 쌍방 revoke/remove transaction, socket disconnect, replay
3. `desktop/disconnect-cleanliness`: A UI, keychain/watcher 정리, 동일 snapshot 표시
4. `mobile/pairing-gate`: pairing-only gate, cache purge, disconnect progress
5. `desktop/file-search-adapter`: index 검색, cursor generation, 경계 검증
6. `mobile/files-browser-search`: room 목록, browse, search, verified download UX
7. `mobile/fixed-character`: appearance/theme 해금 UI와 호출 제거
8. `e2e/v1.4-connection-files`: 쌍방 해제, 동일 청결도, 검색·다운로드 회귀 테스트

공유 계약 PR은 A와 B가 함께 승인하고, A/B 소유 코드 변경은 가능한 한 별도 PR로 유지한다.

## 26.10 v1.4 필수 E2E

1. 같은 fixture에서 데스크톱·모바일 청결도 점수와 차감 사유 일치
2. 모바일에서 device revoke → Desktop 즉시 pairing 화면, 모바일 Pairing Gate
3. Desktop에서 self-revoke → 모바일 즉시 Pairing Gate
4. revoke event 유실 → replay로 동일 상태 복구
5. unpaired 앱 재시작 → pairing code 화면만 표시
6. 새 device pairing → 과거 removed room 미노출
7. 모바일에서 room 제거 → Desktop watcher 해제와 양쪽 목록 제거
8. Desktop에서 room 제거 → 모바일 목록 자동 제거
9. room 제거 후 원본 폴더와 파일 보존
10. 연결된 폴더 browse와 directory 이동
11. 현재 directory와 managed root 이름 검색
12. 빠른 검색어 변경 시 오래된 결과 폐기
13. 검색 결과 파일 다운로드와 checksum 검증
14. removed room·root 밖·다른 사용자 검색/다운로드 차단
15. 호감도 증가 뒤 외형·방 테마 불변
16. disconnect timeout에서 성공 가장 없이 재시도 UI 표시

## 26.11 v1.4 출시 차단 조건

- 데스크톱과 모바일이 같은 room에 서로 다른 청결도 공식 또는 결과를 표시한다.
- device/room 해제 뒤 수동 새로고침 없이는 반대편에 계속 표시된다.
- revoke된 token이나 removed room으로 command, browse, transfer가 가능하다.
- unpaired 상태에서 과거 room 또는 메인 화면이 잠깐이라도 노출된다.
- 폴더 연결 해제가 원본 파일을 삭제하거나 journal 복구 가능성을 숨긴다.
- 검색이 managed root 밖, symlink, junction, reparse point 경계를 우회한다.
- 서버가 파일 이름 전체 index나 검색어를 장기 보관·일반 로그에 기록한다.
- checksum 검증 전에 다운로드 성공을 표시한다.
- 호감도 변화가 외형·방 테마 또는 파일 동작에 영향을 준다.
