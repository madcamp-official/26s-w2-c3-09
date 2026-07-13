#!/usr/bin/env bash
set -euo pipefail

DOMAIN="${HOUSEMOUSE_DOMAIN:-mousekeeper.madcamp-kaist.org}"
REPOSITORY_URL="${HOUSEMOUSE_REPOSITORY_URL:-https://github.com/madcamp-official/26s-w2-c3-09.git}"
BRANCH="${HOUSEMOUSE_BRANCH:-B}"
APP_DIR=/opt/housemouse
CONFIG_DIR=/etc/housemouse
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
NODE_VERSION=v24.18.0

if [[ "${EUID}" -ne 0 ]]; then
  echo 'Run this script with sudo.' >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y ca-certificates curl xz-utils git nginx certbot python3-certbot-nginx docker.io docker-compose-v2 openssl

if ! swapon --show=NAME --noheadings | grep -q '^/swapfile$'; then
  if [[ ! -f /swapfile ]]; then
    fallocate -l 2G /swapfile
    chmod 600 /swapfile
    mkswap /swapfile >/dev/null
  fi
  swapon /swapfile
fi
if ! grep -q '^/swapfile ' /etc/fstab; then
  printf '%s\n' '/swapfile none swap sw 0 0' >>/etc/fstab
fi

if ! command -v node >/dev/null 2>&1 || [[ "$(node --version)" != "${NODE_VERSION}" ]]; then
  temp_dir="$(mktemp -d)"
  trap 'rm -rf "${temp_dir}"' EXIT
  curl -fsSLo "${temp_dir}/node.tar.xz" "https://nodejs.org/dist/${NODE_VERSION}/node-${NODE_VERSION}-linux-x64.tar.xz"
  curl -fsSLo "${temp_dir}/SHASUMS256.txt" "https://nodejs.org/dist/${NODE_VERSION}/SHASUMS256.txt"
  expected="$(grep " node-${NODE_VERSION}-linux-x64.tar.xz$" "${temp_dir}/SHASUMS256.txt" | cut -d ' ' -f 1)"
  actual="$(sha256sum "${temp_dir}/node.tar.xz" | cut -d ' ' -f 1)"
  if [[ -z "${expected}" || "${expected}" != "${actual}" ]]; then
    echo 'Node.js checksum verification failed.' >&2
    exit 1
  fi
  tar -xJf "${temp_dir}/node.tar.xz" -C /usr/local --strip-components=1
fi
corepack enable
corepack prepare pnpm@11.11.0 --activate

systemctl enable --now docker nginx
if ! id housemouse >/dev/null 2>&1; then
  useradd --system --home-dir "${APP_DIR}" --shell /usr/sbin/nologin housemouse
fi

if [[ ! -d "${APP_DIR}/.git" ]]; then
  rm -rf "${APP_DIR}"
  install -d -o housemouse -g housemouse -m 0750 "${APP_DIR}"
  runuser -u housemouse -- git clone --branch "${BRANCH}" --single-branch "${REPOSITORY_URL}" "${APP_DIR}"
else
  runuser -u housemouse -- git -C "${APP_DIR}" fetch origin "${BRANCH}"
  runuser -u housemouse -- git -C "${APP_DIR}" checkout "${BRANCH}"
  runuser -u housemouse -- git -C "${APP_DIR}" merge --ff-only "origin/${BRANCH}"
fi
chown -R housemouse:housemouse "${APP_DIR}"

install -d -o root -g housemouse -m 0750 "${CONFIG_DIR}"
if [[ ! -f "${CONFIG_DIR}/firebase-service-account.json" ]]; then
  if [[ ! -f /tmp/housemouse-firebase-service-account.json ]]; then
    echo 'UNCONFIGURED: /tmp/housemouse-firebase-service-account.json' >&2
    exit 1
  fi
  install -o root -g housemouse -m 0640 /tmp/housemouse-firebase-service-account.json "${CONFIG_DIR}/firebase-service-account.json"
  rm -f /tmp/housemouse-firebase-service-account.json
fi

if [[ ! -f "${CONFIG_DIR}/infra.env" || ! -f "${CONFIG_DIR}/server.env" ]]; then
  postgres_password="$(openssl rand -hex 24)"
  redis_password="$(openssl rand -hex 24)"
  jwt_secret="$(openssl rand -hex 32)"
  umask 0027
  cat >"${CONFIG_DIR}/infra.env" <<EOF
POSTGRES_DB=housemouse
POSTGRES_USER=housemouse
POSTGRES_PASSWORD=${postgres_password}
REDIS_PASSWORD=${redis_password}
EOF
  cat >"${CONFIG_DIR}/server.env" <<EOF
NODE_ENV=production
PORT=3000
SERVER_HOST=127.0.0.1
DATABASE_URL=postgresql://housemouse:${postgres_password}@127.0.0.1:5432/housemouse
REDIS_URL=redis://:${redis_password}@127.0.0.1:6379
WEB_ORIGIN=https://${DOMAIN}
JWT_OR_DEVICE_TOKEN_SECRET=${jwt_secret}
FIREBASE_SERVICE_ACCOUNT_PATH=${CONFIG_DIR}/firebase-service-account.json
FCM_ENABLED=false
SENTRY_DSN=
FILE_TRANSFER_MAX_BYTES=104857600
FILE_TRANSFER_TTL_SECONDS=600
SMART_CACHE_ENABLED=false
SMART_CACHE_DEFAULT_ROOM_QUOTA_BYTES=524288000
SMART_CACHE_DEFAULT_MAX_FILE_BYTES=52428800
OBJECT_STORAGE_ENDPOINT=
OBJECT_STORAGE_REGION=
OBJECT_STORAGE_BUCKET=
OBJECT_STORAGE_ACCESS_KEY_ID=
OBJECT_STORAGE_SECRET_ACCESS_KEY=
EOF
  chown root:housemouse "${CONFIG_DIR}/infra.env" "${CONFIG_DIR}/server.env"
  chmod 0640 "${CONFIG_DIR}/infra.env" "${CONFIG_DIR}/server.env"
fi

docker compose --env-file "${CONFIG_DIR}/infra.env" -f "${SCRIPT_DIR}/compose.yaml" up -d
for _ in {1..30}; do
  if docker compose --env-file "${CONFIG_DIR}/infra.env" -f "${SCRIPT_DIR}/compose.yaml" exec -T postgres pg_isready -U housemouse -d housemouse >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

runuser -u housemouse -- /bin/bash -c "cd '${APP_DIR}'; PATH=/usr/local/bin:/usr/bin:/bin pnpm install --frozen-lockfile"
runuser -u housemouse -- /bin/bash -c "cd '${APP_DIR}'; PATH=/usr/local/bin:/usr/bin:/bin pnpm --filter @housemouse/contracts build"
runuser -u housemouse -- /bin/bash -c "cd '${APP_DIR}'; PATH=/usr/local/bin:/usr/bin:/bin pnpm --filter @housemouse/database build"
runuser -u housemouse -- /bin/bash -c "cd '${APP_DIR}'; PATH=/usr/local/bin:/usr/bin:/bin pnpm --filter @housemouse/server build"
runuser -u housemouse -- /bin/bash -c "set -a; source '${CONFIG_DIR}/server.env'; set +a; cd '${APP_DIR}'; PATH=/usr/local/bin:/usr/bin:/bin pnpm --filter @housemouse/database db:migrate"

install -o root -g root -m 0644 "${SCRIPT_DIR}/systemd/housemouse-server.service" /etc/systemd/system/housemouse-server.service
install -o root -g root -m 0644 "${SCRIPT_DIR}/systemd/housemouse-worker.service" /etc/systemd/system/housemouse-worker.service
install -o root -g root -m 0755 "${SCRIPT_DIR}/backup-postgres.sh" /usr/local/sbin/housemouse-backup-postgres
install -o root -g root -m 0755 "${SCRIPT_DIR}/restore-postgres-drill.sh" /usr/local/sbin/housemouse-restore-postgres-drill
install -d -o root -g root -m 0700 /var/backups/housemouse
install -o root -g root -m 0644 "${SCRIPT_DIR}/systemd/housemouse-postgres-backup.service" /etc/systemd/system/housemouse-postgres-backup.service
install -o root -g root -m 0644 "${SCRIPT_DIR}/systemd/housemouse-postgres-backup.timer" /etc/systemd/system/housemouse-postgres-backup.timer
systemctl daemon-reload
systemctl enable --now housemouse-server
systemctl enable --now housemouse-postgres-backup.timer

for _ in {1..30}; do
  if curl -fsS http://127.0.0.1:3000/ready >/dev/null; then
    break
  fi
  sleep 2
done
curl -fsS http://127.0.0.1:3000/health >/dev/null
curl -fsS http://127.0.0.1:3000/ready >/dev/null

install -o root -g root -m 0644 "${SCRIPT_DIR}/nginx/housemouse.conf" /etc/nginx/sites-available/housemouse
ln -sfn /etc/nginx/sites-available/housemouse /etc/nginx/sites-enabled/housemouse
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl reload nginx

certbot --nginx --non-interactive --agree-tos --register-unsafely-without-email --redirect -d "${DOMAIN}"

bucket_configured=false
region_configured=false
static_credentials_configured=false
instance_role_configured=false

if grep -Eq '^OBJECT_STORAGE_BUCKET=.+$' "${CONFIG_DIR}/server.env"; then
  bucket_configured=true
fi
if grep -Eq '^OBJECT_STORAGE_REGION=.+$' "${CONFIG_DIR}/server.env"; then
  region_configured=true
fi
if grep -Eq '^OBJECT_STORAGE_ACCESS_KEY_ID=.+$' "${CONFIG_DIR}/server.env" && \
   grep -Eq '^OBJECT_STORAGE_SECRET_ACCESS_KEY=.+$' "${CONFIG_DIR}/server.env"; then
  static_credentials_configured=true
fi

# IMDSv2 is queried only to determine whether an EC2 instance role is attached.
metadata_token="$(curl -fsS --max-time 2 -X PUT \
  -H 'X-aws-ec2-metadata-token-ttl-seconds: 60' \
  http://169.254.169.254/latest/api/token 2>/dev/null || true)"
if [[ -n "${metadata_token}" ]] && curl -fsS --max-time 2 \
  -H "X-aws-ec2-metadata-token: ${metadata_token}" \
  http://169.254.169.254/latest/meta-data/iam/security-credentials/ >/dev/null 2>&1; then
  instance_role_configured=true
fi

if [[ "${bucket_configured}" == true ]] && [[ "${region_configured}" == true ]] && \
   { [[ "${static_credentials_configured}" == true ]] || [[ "${instance_role_configured}" == true ]]; }; then
  systemctl enable --now housemouse-worker
else
  systemctl disable --now housemouse-worker >/dev/null 2>&1 || true
  echo 'UNCONFIGURED: object-storage region, bucket, and credentials/EC2 IAM role; worker remains stopped.'
fi

systemctl --no-pager --full status housemouse-server | sed -n '1,12p'
echo "DEPLOYED: https://${DOMAIN}"
