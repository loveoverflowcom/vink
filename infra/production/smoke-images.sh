#!/bin/bash
# Called on a disposable Actions runner with the just-built image tags.
set -euo pipefail
cd "$(dirname "$0")/../.."
revision=${1:?source SHA required}
platform=${2:?platform required}
app_image=${3:-web}
[[ $app_image == web || $app_image == api ]]
# Opt-in auth Compose renders with stand-ins only; image smoke below retains auth disabled.
ruby infra/auth-mail/check-static.rb
bash infra/auth-mail/check-render.sh
[[ $revision =~ ^[a-f0-9]{40}$ && $platform =~ ^linux/(amd64|arm64)$ ]]
for service in "$app_image" worker identity notifications; do
    image="ghcr.io/loveoverflowcom/vink-$service:sha-$revision"
    docker run --rm --entrypoint sh "$image" -c 'test "$(id -u)" -ne 0'
    status=0
    docker run --rm "$image" >/dev/null 2>&1 || status=$?
    test "$status" = 2
done
if [[ $app_image == api ]]; then
    docker run --rm --entrypoint sh "ghcr.io/loveoverflowcom/vink-api:sha-$revision" -c '
        test ! -e /opt/vink/site && test ! -e /opt/vink/site-release &&
        test ! -e /src && test ! -e /opt/vink/.env'
else
    docker run --rm --entrypoint sh "ghcr.io/loveoverflowcom/vink-web:sha-$revision" -c '
        test -s /opt/vink/site/pkg/vink.js && test -s /opt/vink/site/pkg/vink_bg.wasm &&
        test -s /opt/vink/site/pkg/vink-editor.js && test -s /opt/vink/site/pkg/vink.css &&
        test -d /opt/vink/site/pkg/fonts && test -d /opt/vink/site/pkg/brand &&
        test ! -e /opt/vink/site/pkg/expressions &&
        test -z "$(find /opt/vink/site/pkg -type d -name expressions -print -quit)" &&
        test ! -e /src && test ! -e /opt/vink/.env'
fi
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
sudo install -d -m 0700 -o 10001 -g 10001 "$root/identity" "$root/inbox" "$root/ingest"
cat > "$root/public.env" <<SETTINGS
VINK_PROD_POSTGRES_IMAGE=docker.io/library/postgres@sha256:e5d2edd28b1f8d40a0a6c0c9cdc545b19b56a0d5d88f94bc11a97d970d4700d0
VINK_POSTGRES_VOLUME=$project-postgres
VINK_PROD_PLATFORM=$platform
VINK_PROD_UID=10001
VINK_PROD_DB_NETWORK=$project-db
VINK_PROD_EDGE_NETWORK=$project-ingress
VINK_PROD_SECRETS_DIR=$root/runtime
VINK_PROD_SOCKET_DIR=$root/identity
VINK_PROD_INBOX_DIR=$root/inbox
VINK_PROD_INGEST_DIR=$root/ingest
VINK_PROD_NOTIFICATIONS_IMAGE=ghcr.io/loveoverflowcom/vink-notifications:sha-$revision
VINK_PROD_APP_IMAGE=ghcr.io/loveoverflowcom/vink-$app_image:sha-$revision
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
for service in migrate-app migrate-identity migrate-notifications; do
    "${compose[@]}" --profile migration run --rm "$service"
    "${compose[@]}" --profile migration run --rm "$service"
done
"${compose[@]}" up -d --wait notifications
"${compose[@]}" up -d --wait app identity worker
app() { "${compose[@]}" exec -T app curl --max-time 10 --silent --show-error "$@"; }
app --fail http://127.0.0.1:3000/health/ready | jq -e '.status == "ready" and .checks == {database:"ok", role:"restricted", schema:"current"}'
if [[ $app_image == api ]]; then
    for path in / /toi /pkg/vink.js; do
        [[ $(app -o /dev/null -w '%{http_code}' "http://127.0.0.1:3000$path") == 404 ]]
    done
else
    app --fail http://127.0.0.1:3000/ > "$root/index.html"
    grep -q 'Khám phá' "$root/index.html"
fi
app --fail http://127.0.0.1:3000/api/v1/works | jq -e 'type == "object"'
app --fail --unix-socket /run/vink-identity/identity.sock http://identity/health/ready |
    jq -e '.status == "ready" and .checks.database == "ok" and .checks.schema == "current" and .checks.role == "restricted"'
# Check each caller's mount and peer independently.
app --fail --unix-socket /run/vink-inbox/inbox.sock http://notifications/health/ready |
    jq -e '.scope == "inbox" and .checks.role == "least_privileged" and .checks.recovery == "serving"'
"${compose[@]}" exec -T worker curl -fsS --unix-socket /run/vink-ingest/ingest.sock http://notifications/health/ready |
    jq -e '.scope == "ingest" and .status == "ready"'
"${compose[@]}" exec -T app test ! -e /run/vink-ingest/ingest.sock
"${compose[@]}" exec -T worker test ! -e /run/vink-inbox/inbox.sock

# Exercise the production binaries through a real session in this disposable project.
# Password authority needs no mail provider. Keep the production manifests locked by default.
cat > "$root/password-test.yaml" <<'YAML'
services:
  app:
    environment:
      VINK_AUTH_SIGN_IN: enabled
  identity:
    environment:
      VINK_IDENTITY_AUTHORITY: password
YAML
compose+=(-f "$root/password-test.yaml")
"${compose[@]}" up -d --wait identity app
base=http://127.0.0.1:3000/api/v1
auth=(-H 'Origin: https://vink.forum' -H 'Content-Type: application/json')
app --fail -D - -o /dev/null "${auth[@]}" --data '{"username":"smoke_inbox","password":"scratch-inbox-password"}' "$base/auth/register" |
    sed -nE 's/^[Ss]et-[Cc]ookie: (__Host-vink_session=[^;]+);.*/Cookie: \1/p' > "$root/session"
test -s "$root/session"
# Pass the header via stdin, keeping credentials out of argv and stdout.
private() { app -H @- "$@" < "$root/session"; }
private --fail "${auth[@]}" --data '{"pen_name":"Bạn đọc thử"}' "$base/auth/principal" >/dev/null
inbox="$base/me/notifications"
private --fail "$inbox" | jq -e '.items == [] and (.inbox_snapshot | type == "string")'
private --fail "$inbox/count" | jq -e '.freshness.status == "fresh" and .freshness.unseen_group_count == 0'
gate() { "${compose[@]}" exec -T notifications vink-notifications maintenance recovery "$1" smoke >/dev/null; }
gate begin
test "$(private -o /dev/null -w '%{http_code}' "$inbox")" = 503
private --fail "$inbox/count" | jq -e '.freshness.status == "stale"'
gate end
private --fail "$inbox" | jq -e '.items == []'
private --fail "$inbox/count" | jq -e '.freshness.status == "fresh"'
worker=$("${compose[@]}" ps -q worker)
test "$(docker inspect --format '{{.State.Running}}' "$worker")" = true
"${compose[@]}" stop -t 20 worker app identity notifications
for service in worker identity app notifications; do
    container=$("${compose[@]}" ps -a -q "$service")
    test "$(docker inspect --format '{{.State.ExitCode}}' "$container")" = 0
done
