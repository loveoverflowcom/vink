#!/bin/sh
# Provisions Vink's PostgreSQL owners on one instance: the App database and the Identity
# database, each with its own migrator and runtime role (see README.md).
#
#   services/postgres/provision.sh [--check] [--env-file FILE]
#
# Connects as the bootstrap/operator superuser through the libpq environment (PGHOST, PGPORT,
# PGUSER, PGPASSWORD or ~/.pgpass, PGDATABASE = a maintenance database such as `postgres`).
# --check runs only the read-only checks. --env-file loads shell-syntax settings first.
# Needs psql 15 or newer on PATH (or PSQL=/path/to/psql). Exit 0 ok, 2 usage, else psql failure.
set -eu

usage() { sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; }

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
check_only=0
while [ $# -gt 0 ]; do
    case "$1" in
        --check) check_only=1 ;;
        --env-file)
            if [ $# -lt 2 ]; then
                echo "provision.sh: --env-file needs a path" >&2
                exit 2
            fi
            case "$2" in */*) env_file=$2 ;; *) env_file=./$2 ;; esac
            set -a
            # shellcheck disable=SC1090
            . "$env_file"
            set +a
            shift
            ;;
        -h | --help) usage; exit 0 ;;
        *) echo "provision.sh: unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

psql=${PSQL:-psql}
if ! command -v "$psql" >/dev/null 2>&1; then
    echo "provision.sh: psql not found; install a PostgreSQL 15+ client or set PSQL" >&2
    exit 2
fi
major=$("$psql" --version | sed -n 's/.* \([0-9][0-9]*\)\..*/\1/p')
if [ "${major:-0}" -lt 15 ]; then
    echo "provision.sh: psql 15+ is required (found: $("$psql" --version))" >&2
    exit 2
fi

: "${VINK_APP_DATABASE:=vink}"
: "${VINK_APP_MIGRATOR_ROLE:=vink_app_migrator}"
: "${VINK_APP_RUNTIME_ROLE:=vink_app_runtime}"
: "${VINK_IDENTITY_DATABASE:=vink_identity}"
: "${VINK_IDENTITY_MIGRATOR_ROLE:=vink_identity_migrator}"
: "${VINK_IDENTITY_RUNTIME_ROLE:=vink_identity_runtime}"

# owner LABEL DATABASE MIGRATOR RUNTIME MIGRATOR_PASSWORD RUNTIME_PASSWORD
owner() {
    (
        unset VINK_PROVISION_MIGRATOR_PASSWORD VINK_PROVISION_RUNTIME_PASSWORD
        if [ -n "$5" ]; then export VINK_PROVISION_MIGRATOR_PASSWORD="$5"; fi
        if [ -n "$6" ]; then export VINK_PROVISION_RUNTIME_PASSWORD="$6"; fi
        echo "==> $1: database $2, migrator $3, runtime $4"
        exec "$psql" -X -q -v ON_ERROR_STOP=1 -v db="$2" -v migrator="$3" -v runtime="$4" \
            -v history_table=_sqlx_migrations -v check_only="$check_only" \
            -f "$here/provision/owner.sql"
    )
}

owner app "$VINK_APP_DATABASE" "$VINK_APP_MIGRATOR_ROLE" "$VINK_APP_RUNTIME_ROLE" \
    "${VINK_APP_MIGRATOR_PASSWORD:-}" "${VINK_APP_RUNTIME_PASSWORD:-}"
owner identity "$VINK_IDENTITY_DATABASE" "$VINK_IDENTITY_MIGRATOR_ROLE" "$VINK_IDENTITY_RUNTIME_ROLE" \
    "${VINK_IDENTITY_MIGRATOR_PASSWORD:-}" "${VINK_IDENTITY_RUNTIME_PASSWORD:-}"
if [ "$check_only" = 1 ]; then echo "==> checks ok"; else echo "==> provisioning ok"; fi
