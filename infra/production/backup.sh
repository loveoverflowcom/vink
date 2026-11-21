#!/bin/bash
# Produces one private, complete pair. Keeps the seven most recent Vink-only backups.
set -euo pipefail
umask 077
root=/var/backups/vink-prod
cd /opt/vink-prod/current
install -d -m 0700 "$root"
exec 9> "$root/.lock"
flock -n 9
# Leave at least 5 GiB available before using the shared host filesystem for a dump.
(( $(df -Pk "$root" | awk 'NR == 2 {print $4}') >= 5 * 1024 * 1024 ))
work=$(mktemp -d "$root/.incomplete.XXXXXXXX")
trap 'rm -f -- "$work/vink.dump" "$work/vink_identity.dump" "$work/vink_notifications.dump" "$work/SHA256SUMS"; rmdir -- "$work" 2>/dev/null || true' EXIT
export PODMAN_COMPOSE_PROVIDER=/usr/bin/podman-compose
compose=(podman compose --project-name vink-prod --env-file /etc/vink-prod/release.env
    -f /opt/vink-prod/current/services/postgres/compose.yaml -f /opt/vink-prod/current/infra/production/compose.yaml)
# Each owner database is dumped on its own (K1); the Notification dump is taken last, so a restore
# of this set leaves it at or after the App's point and the App relay's replay covers the gap.
# A database that is not provisioned yet (vink_notifications before #347 is deployed) is skipped.
dumps=()
for database in vink vink_identity vink_notifications; do
    if [ "$database" = vink_notifications ] && ! "${compose[@]}" exec -T postgres \
        psql -U vink -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = 'vink_notifications'" | grep -q 1; then
        continue
    fi
    "${compose[@]}" exec -T postgres pg_dump -U vink -Fc "$database" > "$work/$database.dump"
    "${compose[@]}" exec -T postgres pg_restore --list < "$work/$database.dump" > /dev/null
    dumps+=("$database.dump")
done
(cd "$work" && sha256sum "${dumps[@]}" > SHA256SUMS)
stamp=$(date -u +%Y%m%dT%H%M%SZ)
test ! -e "$root/$stamp"
mv -T -- "$work" "$root/$stamp"
trap - EXIT
mapfile -t old < <(find "$root" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' |
    awk '/^[0-9]{8}T[0-9]{6}Z$/' | sort -r | tail -n +8)
for stamp in "${old[@]}"; do
    rm -f -- "$root/$stamp/vink.dump" "$root/$stamp/vink_identity.dump" "$root/$stamp/vink_notifications.dump" "$root/$stamp/SHA256SUMS"
    rmdir -- "$root/$stamp"
done
