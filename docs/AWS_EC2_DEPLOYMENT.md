# AWS EC2 배포 절차

이 문서는 HOUSEMOUSE API와 object lifecycle worker를 AWS EC2에 배포하는 절차다. 실제 secret, Firebase 서비스 계정 원문과 S3 credential은 Git이나 명령 기록에 넣지 않는다.

## 확인된 현재 상태

2026-07-13 기준 공용 DNS `8.8.8.8`, `1.1.1.1`은 `mousekeeper.madcamp-kaist.org`를 `13.237.248.194`로 해석한다. Nginx는 80을 HTTPS로 redirect하고 443에서 유효한 인증서를 제공한다. 외부 `/health`와 `/ready`는 모두 `200`이며, API 3000·PostgreSQL 5432·Valkey 6379는 외부에서 닫혀 있다. Private S3 bucket은 EC2 IAM role로만 접근하며 LIST·PUT·HEAD·GET·DELETE smoke test를 통과했다. Object lifecycle worker도 systemd에서 실행 중이다.

Windows 로컬 DNS가 이전 NXDOMAIN을 보관하면 다음으로 확인한다.

```powershell
ipconfig /flushdns
Resolve-DnsName mousekeeper.madcamp-kaist.org -Server 8.8.8.8
```

## 목표 경계

- 외부에는 80과 443만 공개한다.
- API는 EC2 loopback `127.0.0.1:3000`에서만 받고 Nginx가 proxy한다.
- PostgreSQL과 Valkey/Redis는 인터넷에 공개하지 않는다.
- S3 bucket은 private 및 Block Public Access 상태를 유지한다.
- EC2는 장기 Access Key 대신 IAM instance role로 S3에 접근한다.
- P0 동안 `SMART_CACHE_ENABLED=false`를 유지한다.

## 1. EC2 준비

Ubuntu에서 서비스 계정과 디렉터리를 준비한다.

```bash
sudo useradd --system --home /opt/housemouse --shell /usr/sbin/nologin housemouse
sudo install -d -o housemouse -g housemouse -m 0750 /opt/housemouse
sudo install -d -o root -g housemouse -m 0750 /etc/housemouse
sudo install -o root -g housemouse -m 0640 /dev/null /etc/housemouse/server.env
```

Node.js 24 LTS, Corepack과 `pnpm@11.11.0`, Git, Nginx, Certbot을 설치한다. 저장소는 `/opt/housemouse`에 checkout하고 `housemouse` 사용자가 읽을 수 있게 한다.

현재처럼 비어 있는 EC2에는 `infra/aws/bootstrap-ec2.sh`를 복사해 실행할 수 있다. 이 스크립트는 1GB 인스턴스를 위한 2GB swap, loopback 전용 PostgreSQL·Valkey Docker, Node/pnpm, migration, server systemd, Nginx와 Certbot을 구성한다. Firebase service account는 실행 전에 `/tmp/housemouse-firebase-service-account.json`에 권한을 제한해 전송해야 한다. Object storage가 없으면 worker를 시작하지 않고 `UNCONFIGURED`로 남긴다.

환경 변수 이름은 루트 `.env.example`을 기준으로 `/etc/housemouse/server.env`에 입력한다. 파일에는 실제 값이 필요하지만 Git에는 추가하지 않는다. AWS S3와 IAM role을 사용할 때는 다음 규칙을 따른다.

- `SERVER_HOST`: EC2에서는 `127.0.0.1`; 외부 reverse proxy가 없는 container platform에서만 `0.0.0.0`

- `OBJECT_STORAGE_REGION`: bucket 리전
- `OBJECT_STORAGE_BUCKET`: private bucket 이름
- `OBJECT_STORAGE_ENDPOINT`: AWS 기본 S3에서는 빈 값
- `OBJECT_STORAGE_ACCESS_KEY_ID`, `OBJECT_STORAGE_SECRET_ACCESS_KEY`: IAM role 사용 시 빈 값

static credential 중 한쪽만 입력하면 server와 worker는 `UNCONFIGURED`로 중단한다.

## 2. IAM instance role

EC2에 `AmazonSSMManagedInstanceCore`와 bucket 최소 권한 정책을 가진 instance profile을 연결한다. bucket ARN은 실제 private bucket으로 제한한다.

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "s3:ListBucket",
      "Resource": "arn:aws:s3:::<bucket-name>"
    },
    {
      "Effect": "Allow",
      "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"],
      "Resource": "arn:aws:s3:::<bucket-name>/*"
    }
  ]
}
```

## 3. 설치, migration, build

```bash
cd /opt/housemouse
sudo -u housemouse pnpm install --frozen-lockfile
sudo -u housemouse pnpm --filter @housemouse/contracts build
sudo -u housemouse pnpm --filter @housemouse/database build
sudo -u housemouse env NODE_OPTIONS=--max-old-space-size=1024 \
  pnpm --filter @housemouse/server build

sudo -u housemouse bash -c 'set -a; source /etc/housemouse/server.env; set +a; pnpm --filter @housemouse/database db:migrate'
```

Migration 성공 전에는 server와 worker를 시작하지 않는다.

## 4. systemd 등록

```bash
sudo cp infra/aws/systemd/housemouse-server.service /etc/systemd/system/
sudo cp infra/aws/systemd/housemouse-worker.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now housemouse-server
sudo systemctl status housemouse-server --no-pager
curl --fail http://127.0.0.1:3000/health
curl --fail http://127.0.0.1:3000/ready
```

Worker는 `OBJECT_STORAGE_REGION`, `OBJECT_STORAGE_BUCKET`과 EC2 IAM role 또는 완전한 static credential pair를 모두 확인한 뒤에만 시작한다. 하나라도 없으면 시작하지 않고 `UNCONFIGURED`로 유지한다.

```bash
sudo systemctl enable --now housemouse-worker
sudo systemctl status housemouse-worker --no-pager
```

Bootstrap은 PostgreSQL 일일 backup timer도 설치한다. 배포 후 첫 backup과 격리된 임시 DB restore drill을 실행해 dump가 실제로 복원되는지 확인한다.

```bash
sudo systemctl enable --now housemouse-postgres-backup.timer
sudo /usr/local/sbin/housemouse-backup-postgres
sudo /usr/local/sbin/housemouse-restore-postgres-drill \
  /var/backups/housemouse/housemouse-<UTC timestamp>.dump
```

실패하면 secret을 출력하지 말고 다음 로그의 오류 코드만 확인한다.

```bash
sudo journalctl -u housemouse-server -u housemouse-worker -n 100 --no-pager
```

## 5. Nginx와 HTTPS

```bash
sudo cp infra/aws/nginx/housemouse.conf /etc/nginx/sites-available/housemouse
sudo ln -sfn /etc/nginx/sites-available/housemouse /etc/nginx/sites-enabled/housemouse
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl reload nginx

sudo certbot --nginx -d mousekeeper.madcamp-kaist.org --redirect
```

AWS Security Group은 80, 443 inbound만 허용하고 3000, 5432, 6379는 외부에 열지 않는다. 인증서 발급 후 다음 검증이 모두 통과해야 배포 완료다.

```powershell
.\scripts\check-production-endpoint.ps1 `
  -BaseUrl https://mousekeeper.madcamp-kaist.org
```

## 6. 클라이언트 연결

운영 URL은 source에 하드코딩하지 않고 빌드·실행 환경에서 주입한다.

```powershell
$env:HOUSEMOUSE_SERVER_BASE_URL = 'https://mousekeeper.madcamp-kaist.org'
pnpm --filter @housemouse/desktop tauri:dev

Set-Location apps/mobile
flutter run `
  --dart-define=FIREBASE_ENABLED=true `
  --dart-define=HOUSEMOUSE_API_URL=https://mousekeeper.madcamp-kaist.org `
  --dart-define=GOOGLE_SERVER_CLIENT_ID=<Google-Web-OAuth-Client-ID>
```

운영 URL을 사용하면 `adb reverse`는 필요하지 않다.

## 7. 배포 갱신

새 버전은 install/build와 migration을 먼저 성공시킨 뒤 server를 재시작한다. Object storage 설정이 검증된 환경에서만 worker도 함께 재시작한다.

```bash
sudo systemctl restart housemouse-server
sudo systemctl is-active housemouse-server

# Object storage가 실제 설정된 경우에만 실행
sudo systemctl restart housemouse-worker
sudo systemctl is-active housemouse-worker
```

DB migration 이후 애플리케이션만 이전 버전으로 내리지 않는다. rollback과 restore는 `docs/RECOVERY_RUNBOOK.md`를 따른다.
