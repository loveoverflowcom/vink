#!/usr/bin/env bash
# Run only on the Vink production host, under release.py's single-writer lock.
set -euo pipefail
umask 077
mode=$1 old=$2 revision=$3 web=$4 worker=$5 identity=$6 platform=$7 schema_action=${8:-same} notifications=${9:-}
current=/opt/vink-prod/current
target=/opt/vink-prod/releases/$revision
envfile=/etc/vink-prod/release.env
archive=/var/lib/vink-production-release
[[ $old =~ ^[0-9a-f]{40}$ && $revision =~ ^[0-9a-f]{40}$ ]]
[[ $(realpath "$current") == "/opt/vink-prod/releases/$old" ]]
[[ $(git -C "$target" rev-parse HEAD) == "$revision" && -z $(git -C "$target" status --porcelain) ]]
[[ $(git -C "$current" rev-parse HEAD) == "$old" && -z $(git -C "$current" status --porcelain) ]]
[[ $(grep -Fx "VINK_PROD_PLATFORM=$platform" "$envfile") ]]
export PODMAN_COMPOSE_PROVIDER=/usr/bin/podman-compose
compose=(podman compose --project-name vink-prod --env-file "$envfile"
    -f "$current/services/postgres/compose.yaml" -f "$current/infra/production/compose.yaml")
# Preserve the existing opt-in overlay or stop if the running project has additional owners.
service_id() {
    local id
    id=$(podman ps -q --filter label=io.podman.compose.project=vink-prod \
         --filter "label=com.docker.compose.service=$1")
    [[ $id =~ ^[a-f0-9]{12,64}$ ]] || return 1
    printf '%s' "$id"
}
app=$(service_id app)
test -n "$app"
configs=$(podman inspect -f '{{index .Config.Labels "com.docker.compose.project.config_files"}}' "$app")
base="$current/services/postgres/compose.yaml,$current/infra/production/compose.yaml"
case "$configs" in
    "$base") ;;
    "$base,$current/infra/production/auth-resend.compose.yaml")
        test -f "$target/infra/production/auth-resend.compose.yaml"
        compose+=(-f "$current/infra/production/auth-resend.compose.yaml");;
    *) echo 'Running Compose project files need reconciliation before release' >&2; exit 1;;
esac
# Three-image predecessors remain valid for observation and backout. Bind the candidate
# receipt to the services declared by its immutable checkout.
old_services=(identity app worker)
new_services=(identity app worker)
if grep -q '^  notifications:$' "$current/infra/production/compose.yaml"; then
    old_services+=(notifications)
fi
if grep -q '^  notifications:$' "$target/infra/production/compose.yaml"; then
    [[ $notifications =~ ^ghcr\.io/loveoverflowcom/vink-notifications@sha256:[0-9a-f]{64}$ ]]
    new_services+=(notifications)
else
    test -z "$notifications"
fi
observe() {
    for name in "${new_services[@]}"; do
        id=$(service_id "$name")
        test "$(podman inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$id")" = "$revision"
        case "$name" in app) expected=$web;; identity) expected=$identity;; worker) expected=$worker;; notifications) expected=$notifications;; esac
        test "$(podman inspect --format '{{.Image}}' "$id")" = "$(podman image inspect --format '{{.Id}}' "$expected")"
        test "$(podman inspect --format '{{.State.Running}}' "$id")" = true
    done
    for attempt in {1..30}; do
        if "${compose[@]}" exec -T app curl -fsS http://127.0.0.1:3000/health/ready >/dev/null &&
           "${compose[@]}" exec -T identity curl -fsS --unix-socket /run/vink-identity/identity.sock http://identity/health/ready >/dev/null; then
            if [[ -z $notifications ]] || {
                "${compose[@]}" exec -T app curl -fsS --unix-socket /run/vink-inbox/inbox.sock http://notifications/health/ready >/dev/null &&
                "${compose[@]}" exec -T worker curl -fsS --unix-socket /run/vink-ingest/ingest.sock http://notifications/health/ready >/dev/null
            }; then
                return 0
            fi
        fi
        sleep 2
    done
    return 1
}
if [[ $mode == observe ]]; then
    observe
    exit
fi
for name in "${old_services[@]}"; do
    id=$(service_id "$name")
    test "$(podman inspect --format '{{.State.Running}}' "$id")" = true
    test "$(podman inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$id")" = "$old"
    case "$name" in app) var=APP; package=web;; worker) var=WORKER; package=worker;; identity) var=IDENTITY; package=identity;; notifications) var=NOTIFICATIONS; package=notifications;; esac
    declared=$(sed -n "s/^VINK_PROD_${var}_IMAGE=//p" "$envfile")
    [[ $declared =~ ^ghcr\.io/loveoverflowcom/vink-${package}@sha256:[0-9a-f]{64}$ ]]
    test "$(podman inspect --format '{{.Image}}' "$id")" = "$(podman image inspect --format '{{.Id}}' "$declared")"
done
# Both old and new binaries must have been rehearsed against the resulting schema.
case "$schema_action" in
    same)
        owners=(vink-app vink-identity)
        if [[ -n $notifications ]]; then owners+=(vink-notifications); fi
        for owner in "${owners[@]}"; do
            diff -qr "$current/services/$owner/migrations" "$target/services/$owner/migrations" >/dev/null
        done;;
    expand|expand-additive|keep) ;; # release.py checked the direction and exact migration inventory.
    maintenance) [[ $mode == plan ]] ;; # Only the journaled maintenance adapter can switch.
    *) exit 2;;
esac
# Maintenance and older schema reviews retain the verified-backup gate. Only a
# reviewed additive migration, or an unchanged deployment topology, can omit it.
needs_backup=true
if [[ $schema_action == expand-additive ]]; then
    needs_backup=false
elif [[ $schema_action == same ]] && [[ "${old_services[*]}" == "${new_services[*]}" ]] &&
     cmp -s "$current/services/postgres/compose.yaml" "$target/services/postgres/compose.yaml" &&
     cmp -s "$current/infra/production/compose.yaml" "$target/infra/production/compose.yaml" &&
     cmp -s "$current/infra/production/auth-resend.compose.yaml" "$target/infra/production/auth-resend.compose.yaml"; then
    needs_backup=false
fi
if [[ $needs_backup == true ]]; then
    backup=$(realpath "${VINK_VERIFIED_BACKUP:?recent pair of verified Vink dumps required}")
    [[ $backup =~ ^/var/backups/vink-prod/[0-9]{8}T[0-9]{6}Z$ ]]
    test -s "$backup/vink.dump"
    test -s "$backup/vink_identity.dump"
    if [[ -n $notifications || " ${old_services[*]} " == *" notifications "* ]]; then
        test -s "$backup/vink_notifications.dump"
        grep -Eq '^[a-f0-9]{64}  vink_notifications.dump$' "$backup/SHA256SUMS"
    fi
    test -n "$(find "$backup/SHA256SUMS" -mtime -1 -print)"
    (cd "$backup" && sha256sum --check --strict SHA256SUMS >/dev/null)
fi
[[ $mode == apply ]] || exit 0
saved=$(mktemp -d "$archive/origin-$old.XXXXXXXX")
cp -p "$envfile" "$saved/release.env"
next=$(mktemp "${envfile}.XXXXXXXX")
awk -v app="$web" -v worker="$worker" -v identity="$identity" -v notifications="$notifications" '
    /^VINK_PROD_APP_IMAGE=/{print "VINK_PROD_APP_IMAGE=" app; a++; next}
    /^VINK_PROD_WORKER_IMAGE=/{print "VINK_PROD_WORKER_IMAGE=" worker; w++; next}
    /^VINK_PROD_IDENTITY_IMAGE=/{print "VINK_PROD_IDENTITY_IMAGE=" identity; i++; next}
    /^VINK_PROD_NOTIFICATIONS_IMAGE=/{n++; next}
    {print} END {if (a!=1 || w!=1 || i!=1 || n>1) exit 1; if (notifications!="") print "VINK_PROD_NOTIFICATIONS_IMAGE=" notifications}
' "$envfile" > "$next"
# Both paths are on their respective destination filesystem for atomic replacement per file.
link="$current.next-$$"
if [[ -z $notifications && " ${old_services[*]} " == *" notifications "* ]]; then
    "${compose[@]}" stop worker notifications
fi
ln -s "$target" "$link"
mv -T "$next" "$envfile"
mv -T "$link" "$current"
if [[ $schema_action == expand || $schema_action == expand-additive ]]; then
    migrators=(migrate-app migrate-identity)
    if [[ -n $notifications ]]; then migrators+=(migrate-notifications); fi
    for migrator in "${migrators[@]}"; do
        "${compose[@]}" --profile migration run --rm --no-deps "$migrator" </dev/null
    done
fi
"${compose[@]}" up -d --no-build --no-deps "${new_services[@]}"
observe
