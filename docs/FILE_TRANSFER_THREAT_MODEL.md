# P0 FileTransfer 위협 모델

## 보호 대상과 신뢰 경계

보호 대상은 관리 root 내부 원본, 상대 경로 metadata, transfer object, checksum, 사용자·device token이다. 모바일과 Desktop은 서로의 입력을 신뢰하지 않으며 서버도 Desktop의 source validation 결과를 구조화된 계약으로만 받는다. Object storage는 private bucket과 짧은 signed URL을 제공하는 외부 경계다.

## 주요 위협과 방어

| 위협 | 방어선 | 검증 근거 |
|---|---|---|
| 다른 사용자의 room·transfer 조회 | 모든 조회에서 `requestedByUserId`와 room owner 확인 | transfer DB integration |
| 다른 Desktop이 upload/failure 보고 | device JWT의 정확한 `desktopDeviceId` 일치 확인 | transfer DB integration |
| 절대 경로·traversal 주입 | 공개 계약은 빈 segment, absolute/drive, NUL, `.`·`..` 거절 | contracts tests |
| 같은 key로 다른 파일 재사용 | user idempotency key와 room·source path를 함께 비교 | transfer DB integration |
| 승인되지 않은 장기 upload | Desktop source version 보고 뒤 session 남은 TTL 이하 target만 발급 | signed target expiry test |
| 원본 변경·없음·root 이탈 은폐 | `SOURCE_NOT_FOUND`, `SOURCE_CHANGED`, `OUTSIDE_MANAGED_ROOT`, size/checksum 실패를 DB에 저장 | failure endpoint integration |
| 부분 upload 방치 | 실패·취소·ACK·만료 시 durable deletion job, orphan sweep | worker와 lifecycle tests |
| object key·token 로그 유출 | 공개 응답에서 object/internal key 제거, route-template 구조화 로그만 기록 | request logging test |
| checksum 전 완료 표시 | 모바일 `.part` 다운로드 후 SHA-256 확인과 충돌 없는 rename 뒤 ACK | Flutter download 구현 |

## 상태와 실패 원칙

- 정리 execution과 읽기 전용 transfer 상태 머신을 공유하지 않는다.
- Desktop offline 요청은 장기 대기시키지 않고 `DEVICE_OFFLINE`로 종료한다.
- provider 미설정은 DB mutation 전에 `UNCONFIGURED`로 실패한다.
- server는 source path를 해석하거나 로컬 파일 fallback을 만들지 않는다.
- signed download URL은 60초와 transfer 남은 수명 중 더 짧은 값을 사용한다.

## 외부 환경에서 남은 검증

실제 private S3-compatible bucket에서 PUT, HEAD, GET, DELETE와 provider lifecycle을 실행해야 한다. 이 검증 전에는 object storage E2E를 완료로 간주하지 않는다.
