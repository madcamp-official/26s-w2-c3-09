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

install -d -o root -g root -m 0700 "${BACKUP_DIR}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
final_path="${BACKUP_DIR}/mousekeeper-${timestamp}.dump"
temporary_path="$(mktemp "${BACKUP_DIR}/.mousekeeper-${timestamp}.XXXXXX.dump")"
trap 'rm -f "${temporary_path}"' EXIT

compose=(docker compose --env-file "${ENV_FILE}" -f "${COMPOSE_FILE}")
"${compose[@]}" exec -T postgres pg_dump \
  --username=mousekeeper \
  --dbname=mousekeeper \
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
