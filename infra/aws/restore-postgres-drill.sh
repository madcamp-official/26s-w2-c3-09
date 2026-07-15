#!/usr/bin/env bash
set -euo pipefail

BACKUP_DIR=/var/backups/mousekeeper
COMPOSE_FILE=/opt/mousekeeper/infra/aws/compose.yaml
ENV_FILE=/etc/mousekeeper/infra.env

if [[ "${EUID}" -ne 0 ]]; then
  echo 'Run this script as root.' >&2
  exit 1
fi
if [[ "$#" -ne 1 ]]; then
  echo 'Usage: restore-postgres-drill.sh /var/backups/mousekeeper/<backup>.dump' >&2
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

backup_path="$(realpath -e -- "$1")"
case "${backup_path}" in
  "${BACKUP_DIR}"/mousekeeper-*.dump) ;;
  *)
    echo 'Restore drill accepts only a MOUSEKEEPER backup from the protected backup directory.' >&2
    exit 1
    ;;
esac

drill_database="mousekeeper_restore_drill_$(date -u +%Y%m%d%H%M%S)_$$"
compose=(docker compose --project-name "${COMPOSE_PROJECT_NAME}" --env-file "${ENV_FILE}" -f "${COMPOSE_FILE}")
cleanup() {
  "${compose[@]}" exec -T postgres dropdb --username="${POSTGRES_USER}" \
    --maintenance-db="${POSTGRES_DB}" \
    --if-exists --force "${drill_database}" >/dev/null
}
trap cleanup EXIT

"${compose[@]}" exec -T postgres pg_restore --list \
  <"${backup_path}" >/dev/null
"${compose[@]}" exec -T postgres createdb --username="${POSTGRES_USER}" \
  --maintenance-db="${POSTGRES_DB}" \
  "${drill_database}"
"${compose[@]}" exec -T postgres pg_restore \
  --username="${POSTGRES_USER}" \
  --dbname="${drill_database}" \
  --exit-on-error \
  --no-owner \
  --no-privileges <"${backup_path}" >/dev/null

schema_ok="$("${compose[@]}" exec -T postgres psql \
  --username="${POSTGRES_USER}" \
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
