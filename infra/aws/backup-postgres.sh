#!/usr/bin/env bash
set -euo pipefail

BACKUP_DIR=/var/backups/mousekeeper
RETENTION_DAYS="${MOUSEKEEPER_BACKUP_RETENTION_DAYS:-7}"
COMPOSE_FILE=/opt/mousekeeper/infra/aws/compose.yaml
ENV_FILE=/etc/mousekeeper/infra.env

if [[ "${EUID}" -ne 0 ]]; then
  echo 'Run this script as root.' >&2
  exit 1
fi
if ! [[ "${RETENTION_DAYS}" =~ ^[1-9][0-9]*$ ]]; then
  echo 'UNCONFIGURED: MOUSEKEEPER_BACKUP_RETENTION_DAYS' >&2
  exit 1
fi
if [[ ! -r "${ENV_FILE}" ]]; then
  echo 'UNCONFIGURED: /etc/mousekeeper/infra.env' >&2
  exit 1
fi

set -a
# shellcheck source=/dev/null
source "${ENV_FILE}"
set +a
for required_variable in POSTGRES_USER POSTGRES_DB; do
  if [[ -z "${!required_variable:-}" ]]; then
    echo "UNCONFIGURED: ${required_variable}" >&2
    exit 1
  fi
done
COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-mousekeeper-production}"
if ! [[ "${COMPOSE_PROJECT_NAME}" =~ ^[a-z0-9][a-z0-9_-]*$ ]]; then
  echo 'UNCONFIGURED: COMPOSE_PROJECT_NAME' >&2
  exit 1
fi

install -d -o root -g root -m 0700 "${BACKUP_DIR}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
final_path="${BACKUP_DIR}/mousekeeper-${timestamp}.dump"
temporary_path="$(mktemp "${BACKUP_DIR}/.mousekeeper-${timestamp}.XXXXXX.dump")"
trap 'rm -f "${temporary_path}"' EXIT

compose=(docker compose --project-name "${COMPOSE_PROJECT_NAME}" --env-file "${ENV_FILE}" -f "${COMPOSE_FILE}")
"${compose[@]}" exec -T postgres pg_dump \
  --username="${POSTGRES_USER}" \
  --dbname="${POSTGRES_DB}" \
  --format=custom \
  --no-owner \
  --no-privileges >"${temporary_path}"

if [[ ! -s "${temporary_path}" ]]; then
  echo 'Backup validation failed: empty output.' >&2
  exit 1
fi
"${compose[@]}" exec -T postgres pg_restore --list \
  <"${temporary_path}" >/dev/null
chmod 0600 "${temporary_path}"
mv "${temporary_path}" "${final_path}"
trap - EXIT

# Deletion is deliberately constrained to this fixed root-only backup directory.
find "${BACKUP_DIR}" -maxdepth 1 -type f -name 'mousekeeper-*.dump' \
  -mtime "+${RETENTION_DAYS}" -delete

checksum="$(sha256sum "${final_path}" | cut -d ' ' -f 1)"
size="$(stat -c '%s' "${final_path}")"
echo "BACKUP_OK file=$(basename "${final_path}") bytes=${size} sha256=${checksum}"
