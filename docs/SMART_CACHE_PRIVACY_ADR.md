# ADR: P1 스마트 캐시 quota와 privacy

- 상태: 코드 구현 완료, 실제 object storage 검증 대기
- 결정일: 2026-07-11

## 결정

스마트 캐시는 object storage가 구성된 운영 환경에서 자동 활성화된다. 서버는 room별 기본 quota policy를 자동 생성하며, global feature flag는 장애 대응용 kill switch로만 사용한다. Desktop은 원본보다 먼저 상대 경로, source version hash, 크기와 usage score만 제출하고, 서버가 transaction 안에서 quota를 예약해 반환한 target만 업로드할 수 있다.

## 이유

전체 폴더 동기화는 서버에 불필요한 파일 원문·인덱스를 모으고 비용과 privacy 위험을 키운다. 자동 활성화 후에도 후보 metadata 선제출, 크기 제한, 제외 pattern과 quota reservation을 분리해 작은 집합만 보관하고 동시 요청의 초과 할당을 차단한다.

## 불변식

- quota 사용량은 `AVAILABLE + 만료 전 RESERVED` 합계다.
- room advisory transaction lock으로 동시 후보 batch를 직렬화한다.
- 같은 candidate batch는 canonical request hash와 idempotency key로 재생한다.
- manual pin, 높은 usage score가 우선이고 non-pinned 낮은 score·오래 미사용한 항목부터 퇴출한다.
- availability와 freshness는 별도 상태다. Offline이면 `VERIFIED_CURRENT`를 그대로 주장하지 않고 `UNVERIFIED_OFFLINE`으로 응답한다.
- cache time과 last verified time을 모바일에 각각 표시한다.
- room 제거, device revoke, account 삭제와 버전 교체는 삭제 tombstone을 남긴다.
- pending command가 있는 room은 목록이 바뀔 수 있다는 경고를 숨기지 않는다.
- object key, 절대 경로와 개별 사용 event 원문을 로그에 남기지 않는다.

## 검증

PostgreSQL 통합 테스트는 policy 자동 생성, 100-byte quota에 60-byte 후보 2개를 동시에 제출해 하나만 예약되는지, batch replay와 payload 충돌, 만료 reservation 해제, LRU eviction, 취소·room remove tombstone을 확인한다. 모바일 테스트는 수동 설정 UI가 없고 pending warning과 `AVAILABLE + UNVERIFIED_OFFLINE` 표현을 유지하는지 확인한다.

## 보류된 외부 결정

실제 provider-side encryption mode, bucket retention/lifecycle, 비용 dashboard는 private object storage가 선택된 뒤 확정한다. 임시 로컬 저장이나 성공을 가장하는 adapter는 만들지 않는다.
