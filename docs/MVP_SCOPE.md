# MOUSEKEEPER MVP 범위

이 문서는 `MOUSEKEEPER_PLAN.md`의 B(Product & Cloud) 구현 경계를 짧게 고정한다. 파일 시스템 변경 엔진은 A 영역이고, B 영역은 사용자의 요청·승인·결과를 안전하게 전달하고 표시하는 control plane이다.

## P0 출시 범위

- Firebase Google 로그인과 사용자별 모바일 데이터 격리
- 짧은 코드/nonce 기반 데스크톱 pairing, 90일 device token, 기기 revoke
- PostgreSQL에 영속되는 room, command, proposal, decision, execution, audit와 cursor replay
- Redis/Valkey TTL 기반 presence와 rate limit
- 모바일 home/room, 규칙 작성, 제안 검토, 승인·거절, 실행 결과와 청결도 표시
- 등록 room 내부 상대 경로만 다루는 온라인 browse와 만료형 file transfer
- 다운로드 SHA-256 검증, 모바일 ACK 또는 TTL 뒤 object 삭제
- Drift 표시 cache와 mutation outbox를 이용한 오프라인 읽기·재전송
- 실제 domain event에서 파생되는 CharacterEvent와 affinity ledger
- PostgreSQL backup/restore, readiness, deletion worker와 배포 구성

## 기능 플래그 뒤의 P1

스마트 캐시는 `SMART_CACHE_ENABLED=false`가 기본이다. 사용자가 room별로 opt-in한 경우에만 후보 metadata를 받고, 서버가 `AVAILABLE + RESERVED` quota를 잠근 뒤 승인한 target만 업로드할 수 있다. 캐시 가능 여부와 원본 최신성은 별도 상태로 표시한다.

## 의도적으로 하지 않는 것

- 모바일 원격 영구 삭제, 승인 없는 파일 변경, 기존 파일 덮어쓰기
- 서버 전체 파일 인덱스·절대 경로 수집
- P0 transfer object의 장기 보관 또는 로컬 디스크 fallback
- opt-in 없는 폴더 전체 동기화
- 외부 provider가 없을 때의 가짜 로그인·AI 답변·signed URL·성공 응답

## 외부 설정 경계

Firebase, object storage, Rive asset, Android release signing, Sentry와 운영 배포 secret은 저장소에 넣지 않는다. 값이 없으면 fail-fast 또는 `UNCONFIGURED`로 노출하며, 실제 설정이 들어오기 전에는 해당 연동의 완료를 주장하지 않는다.
