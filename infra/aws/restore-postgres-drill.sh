#!/usr/bin/env bash
set -euo pipefail

BACKUP_DIR=/var/backups/housemouse
COMPOSE_FILE=/opt/housemouse/infra/aws/compose.yaml
ENV_FILE=/etc/housemouse/infra.env

if [[ "${EUID}" -ne 0 ]]; then
  echo 'Run this script as root.' >&2
  exit 1
fi
if [[ "$#" -ne 1 ]]; then
  echo 'Usage: restore-postgres-drill.sh /var/backups/housemouse/<backup>.dump' >&2
  exit 1
fi

backup_path="$(realpath -e -- "$1")"
case "${backup_path}" in
  "${BACKUP_DIR}"/housemouse-*.dump) ;;
  *)
    echo 'Restore drill accepts only a HOUSEMOUSE backup from the protected backup directory.' >&2
    exit 1
    ;;
esac

drill_database="housemouse_restore_drill_$(date -u +%Y%m%d%H%M%S)_$$"
compose=(docker compose --env-file "${ENV_FILE}" -f "${COMPOSE_FILE}")
cleanup() {
  "${compose[@]}" exec -T postgres dropdb --username=housemouse \
    --if-exists --force "${drill_database}" >/dev/null
}
trap cleanup EXIT

"${compose[@]}" exec -T postgres pg_restore --list \
  <"${backup_path}" >/dev/null
"${compose[@]}" exec -T postgres createdb --username=housemouse \
  "${drill_database}"
"${compose[@]}" exec -T postgres pg_restore \
  --username=housemouse \
  --dbname="${drill_database}" \
  --exit-on-error \
  --no-owner \
  --no-privileges <"${backup_path}" >/dev/null

schema_ok="$("${compose[@]}" exec -T postgres psql \
  --username=housemouse \
  --dbname="${drill_database}" \
  --tuples-only \
  --no-align \
  --command="select to_regclass('public.users') is not null and to_regclass('drizzle.__drizzle_migrations') is not null")"
if [[ "${schema_ok}" != 't' ]]; then
  echo 'Restore validation failed: required schema objects are missing.' >&2
  exit 1
fi

checksum="$(sha256sum "${backup_path}" | cut -d ' ' -f 1)"
echo "RESTORE_DRILL_OK file=$(basename "${backup_path}") sha256=${checksum}"
