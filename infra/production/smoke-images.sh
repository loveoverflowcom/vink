#!/bin/bash
# Called on a disposable Actions runner with the just-built image tags.
set -euo pipefail
cd "$(dirname "$0")/../.."
revision=${1:?source SHA required}
platform=${2:?platform required}
[[ $revision =~ ^[a-f0-9]{40}$ && $platform =~ ^linux/(amd64|arm64)$ ]]
for service in web worker identity; do
    image="ghcr.io/loveoverflowcom/vink-$service:sha-$revision"
    docker run --rm --entrypoint sh "$image" -c 'test "$(id -u)" -ne 0'
    status=0
    docker run --rm "$image" >/dev/null 2>&1 || status=$?
    test "$status" = 2
done
docker run --rm --entrypoint sh "ghcr.io/loveoverflowcom/vink-web:sha-$revision" -c '
    test -s /opt/vink/site/pkg/vink.js && test -s /opt/vink/site/pkg/vink_bg.wasm &&
    test -s /opt/vink/site/pkg/vink-editor.js && test -s /opt/vink/site/pkg/vink.css &&
    test -d /opt/vink/site/pkg/fonts && test -d /opt/vink/site/pkg/brand &&
    test -d /opt/vink/site/pkg/expressions && test ! -e /src && test ! -e /opt/vink/.env'
root=$(mktemp -d)
project=vink-oci-smoke
compose=(docker compose --project-name "$project" --env-file "$root/public.env"
    -f services/postgres/compose.yaml -f infra/production/compose.yaml)
cleanup() {
    if [[ -f $root/public.env ]]; then "${compose[@]}" down >/dev/null 2>&1 || true; fi
    docker network rm "$project-db" "$project-ingress" >/dev/null 2>&1 || true
    docker volume rm "$project-postgres" >/dev/null 2>&1 || true
    sudo rm -rf -- "$root"
}
trap cleanup EXIT
python3 infra/production/create-runtime-files.py "$root/runtime"
sudo install -d -m 0700 -o 10001 -g 10001 "$root/identity"
cat > "$root/public.env" <<SETTINGS
VINK_PROD_POSTGRES_IMAGE=docker.io/library/postgres@sha256:e5d2edd28b1f8d40a0a6c0c9cdc545b19b56a0d5d88f94bc11a97d970d4700d0
VINK_POSTGRES_VOLUME=$project-postgres
VINK_PROD_PLATFORM=$platform
VINK_PROD_UID=10001
VINK_PROD_DB_NETWORK=$project-db
VINK_PROD_EDGE_NETWORK=$project-ingress
VINK_PROD_SECRETS_DIR=$root/runtime
VINK_PROD_SOCKET_DIR=$root/identity
VINK_PROD_APP_IMAGE=ghcr.io/loveoverflowcom/vink-web:sha-$revision
VINK_PROD_WORKER_IMAGE=ghcr.io/loveoverflowcom/vink-worker:sha-$revision
VINK_PROD_IDENTITY_IMAGE=ghcr.io/loveoverflowcom/vink-identity:sha-$revision
SETTINGS
docker network create --internal --subnet 10.89.3.0/24 "$project-db"
docker network create --internal --subnet 10.89.4.0/24 "$project-ingress"
docker volume create "$project-postgres"
"${compose[@]}" up -d --wait postgres
# PostgreSQL initialization can answer on its temporary Unix socket before TCP is ready.
deadline=$((SECONDS + 30))
until "${compose[@]}" exec -T postgres pg_isready -h 127.0.0.1 -U vink -d postgres -q; do
    if (( SECONDS >= deadline )); then
        echo 'PostgreSQL did not become ready over TCP' >&2
        exit 1
    fi
    sleep 1
done
"${compose[@]}" --profile provision run --rm provision
"${compose[@]}" --profile provision run --rm provision --check
for service in migrate-app migrate-identity; do
    "${compose[@]}" --profile migration run --rm "$service"
    "${compose[@]}" --profile migration run --rm "$service"
done
"${compose[@]}" up -d --wait app identity worker
app() { "${compose[@]}" exec -T app curl --max-time 10 --silent --show-error "$@"; }
app --fail http://127.0.0.1:3000/health/ready | jq -e '.status == "ready" and .checks == {database:"ok", role:"restricted", schema:"current"}'
app --fail http://127.0.0.1:3000/ > "$root/index.html"
grep -q 'Khám phá' "$root/index.html"
app --fail http://127.0.0.1:3000/api/v1/works | jq -e 'type == "object"'
app --fail --unix-socket /run/vink-identity/identity.sock http://identity/health/ready |
    jq -e '.status == "ready" and .checks.database == "ok" and .checks.schema == "current" and .checks.role == "restricted"'
worker=$("${compose[@]}" ps -q worker)
test "$(docker inspect --format '{{.State.Running}}' "$worker")" = true
"${compose[@]}" stop -t 20 worker identity app
for service in worker identity app; do
    container=$("${compose[@]}" ps -a -q "$service")
    test "$(docker inspect --format '{{.State.ExitCode}}' "$container")" = 0
done
