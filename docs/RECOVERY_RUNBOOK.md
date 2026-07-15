# MOUSEKEEPER 운영·복구 Runbook

이 문서는 B 담당 영역인 API, worker, PostgreSQL, Valkey, object storage의 배포와 복구 절차를 정의한다. 실제 secret이나 사용자 파일 경로는 문서와 로그에 기록하지 않는다.

## 배포 전 확인

1. `pnpm typecheck`, `pnpm test`, `pnpm build`를 통과시킨다.
2. `flutter analyze`, `flutter test`와 Android release signing 설정을 확인한다.
3. EC2의 root 소유 환경 파일 또는 AWS secret 주입 절차로 Firebase service account, object storage, `WEB_ORIGIN`을 입력한다.
4. Object storage와 IAM role이 검증된 운영 환경에서는 `SMART_CACHE_ENABLED=true`인지 확인한다. 장애 시에만 kill switch로 내린다.
5. pre-deploy migration이 성공한 뒤에만 새 server/worker를 시작한다.
6. `/health` liveness와 PostgreSQL·Valkey를 직접 확인하는 `/ready` readiness, worker deletion job 처리를 확인한다.

외부 값이 하나라도 없으면 해당 기능은 `UNCONFIGURED`로 유지한다. 로컬 디스크 저장이나 임시 인증 우회는 사용하지 않는다.

## PostgreSQL 백업

AWS EC2에서는 root만 읽을 수 있는 `/var/backups/mousekeeper`에 매일 custom-format backup을 만든다. systemd timer의 최근 실행과 다음 실행 시각은 다음처럼 확인한다.

```bash
sudo systemctl list-timers mousekeeper-postgres-backup.timer
sudo systemctl status mousekeeper-postgres-backup.service --no-pager
```

수동 백업과 운영 DB를 변경하지 않는 임시 DB 복원 훈련은 다음 순서다. 복원 훈련은 필수 schema를 확인한 뒤 임시 DB를 항상 삭제한다.

```bash
sudo /usr/local/sbin/mousekeeper-backup-postgres
sudo /usr/local/sbin/mousekeeper-restore-postgres-drill \
  /var/backups/mousekeeper/mousekeeper-<UTC timestamp>.dump
```

이 백업은 EC2 EBS 장애까지 보호하는 off-instance backup이 아니다. 별도 암호화 백업 저장소와 권한이 확정되기 전에는 외부 업로드를 성공으로 가장하지 않는다.

Windows 또는 별도 PostgreSQL client 환경에서는 기존 스크립트를 사용할 수 있다.

운영자가 접근 통제된 터미널에서 `DATABASE_URL`을 환경 변수로 주입하고 다음을 실행한다.

```powershell
.\scripts\backup-postgres.ps1 -OutputPath <보안 저장소의 절대 경로>
```

스크립트는 custom-format dump를 만들고 크기와 SHA-256을 출력한다. 출력 파일과 checksum은 접근 통제·암호화된 백업 저장소로 옮긴다. 저장소 위치가 확정되지 않은 현재는 자동 업로드를 설정하지 않는다.

## 복원 훈련

운영 DB에 바로 복원하지 않는다. 새 빈 PostgreSQL 인스턴스를 만들고 그 인스턴스의 URL만 `DATABASE_URL`에 주입한다.

```powershell
.\scripts\restore-postgres.ps1 -BackupPath <검증할 dump 경로> -Apply
```

복원 뒤 migration table, users/devices/rooms 수, 최신 sync sequence, deletion job 상태를 확인한다. 사용자 파일의 object key나 인증 token은 출력하지 않는다.

## 장애별 복구

- PostgreSQL 장애: API mutation을 중단하고 복원 인스턴스 검증 후 연결 문자열을 전환한다. Redis 데이터를 진실의 원천으로 사용하지 않는다.
- Valkey 장애: presence와 rate limit은 unavailable 상태로 취급한다. command/proposal/execution은 PostgreSQL에서 보존되며 복구 뒤 cursor replay한다.
- Socket.IO 장애: 클라이언트는 마지막 user sequence 이후 `/v1/sync/events`를 replay한다.
- Worker 장애: deletion job은 DB에 남는다. worker 재시작 후 `PENDING` job을 `SKIP LOCKED`로 다시 처리한다.
- Object storage 장애: 파일 browse와 정리 command는 유지하되 download/upload target은 `UNCONFIGURED` 또는 provider 오류로 종료한다. 성공 응답을 만들지 않는다.
- Firebase 장애: 신규 로그인은 실패한다. 기존 device token도 서버 DB의 ACTIVE 상태를 계속 검증한다.

## 기기 해제와 데이터 삭제 확인

기기 revoke 뒤 다음을 확인한다.

- device 상태가 `REVOKED`이고 기존 device token이 거절됨
- presence key가 삭제되거나 최대 45초 뒤 만료됨
- 진행 중 transfer와 cache reservation이 `CANCELLED`로 전환됨
- object deletion job이 생성되고 worker가 `COMPLETED`로 전환함
- 모바일 active device 목록에서 해제한 기기가 사라짐

## Rollback

DB migration이 적용된 뒤 애플리케이션만 이전 버전으로 되돌리면 안 된다. 하위 호환 여부를 확인하고, 호환되지 않으면 검증된 backup을 새 DB에 복원한 뒤 server와 worker를 함께 전환한다. migration SQL을 수동 역실행하지 않는다.
