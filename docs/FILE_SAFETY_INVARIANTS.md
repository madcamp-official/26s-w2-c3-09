# HOUSEMOUSE 파일 안전 불변식

이 불변식은 모바일·서버·데스크톱 사이 계약의 하한선이다. B control plane은 파일 변경을 직접 수행하지 않으며, A 파일 엔진도 이 조건을 만족한 명령만 실행해야 한다.

## 경로와 권한

- 공개 API와 로그에는 절대 경로를 넣지 않고 검증된 상대 경로만 사용한다.
- 빈 segment, `.`·`..`, NUL, drive/UNC/absolute path는 계약 단계에서 거절한다.
- room, device, request, transfer, reservation은 모두 같은 사용자 소유 관계를 다시 검증한다.
- device token은 매 요청에서 DB의 `ACTIVE` 상태를 확인하며 revoke 즉시 사용할 수 없다.
- 데스크톱은 실행·browse·upload 직전에 canonical root와 source identity/version을 다시 확인한다.

## 변경과 승인

- proposal과 decision이 PostgreSQL에 commit되기 전에는 파일 쓰기를 시작하지 않는다.
- decision에 승인된 item만 실행하며, 변경된 precondition은 `STALE`로 종료한다.
- command/proposal/decision/execution mutation은 idempotency key로 중복 실행을 차단한다.
- 파일 변경 상태 머신과 읽기 전용 FileTransfer 상태 머신을 섞지 않는다.
- 기존 목적지 파일을 자동으로 덮어쓰지 않고 영구 삭제를 모바일에 노출하지 않는다.

## 파일 전달과 캐시

- P0 object는 짧은 TTL의 단일 transfer에만 묶고 background 자동 업로드·폴더 zip을 만들지 않는다.
- signed URL은 session/reservation 남은 수명보다 오래 발급하지 않는다.
- 서버는 object HEAD의 크기를 확인하고, 모바일은 SHA-256이 일치한 뒤에만 완료 파일로 원자적으로 전환한다.
- 완료 ACK·취소·실패·만료는 durable deletion job으로 이어지며 worker 재시작 뒤에도 복구된다.
- P1은 명시적 opt-in과 quota reservation 뒤에만 업로드한다. `AVAILABLE`과 freshness를 별도로 저장하고 오프라인에서는 `UNVERIFIED_OFFLINE`을 숨기지 않는다.
- cache disable, device revoke, 오래된 버전 교체는 object 삭제 tombstone을 남긴다.

## 실패 처리

- PC offline, timeout, cursor invalidation, source change, size limit, checksum mismatch를 서로 다른 오류로 보존한다.
- Socket.IO는 최적화 계층이며 PostgreSQL cursor replay가 복구 경로다.
- 외부 storage나 인증 공급자가 없으면 `UNCONFIGURED`로 실패하고 임시 로컬 저장·가짜 성공으로 우회하지 않는다.
