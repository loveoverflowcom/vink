#!/usr/bin/env bash
# Render-only check with Docker Compose and stand-ins. No daemon, network, DB or credential needed.
set -euo pipefail
cd "$(dirname "$0")/../.."
fixture=$(mktemp -d)
trap 'rmdir "$fixture/socket" "$fixture/resend"; unlink "$fixture/public.env"; for name in app-runtime worker-runtime identity-runtime app-migrator identity-migrator notifications-runtime notifications-migrator provision; do unlink "$fixture/$name.env"; done; rmdir "$fixture"' EXIT
mkdir "$fixture/socket" "$fixture/resend"
for name in app-runtime worker-runtime identity-runtime app-migrator identity-migrator notifications-runtime notifications-migrator provision; do
    touch "$fixture/$name.env"
done
unset VINK_PROD_AUTH_SIGN_IN VINK_TEST_AUTH_SIGN_IN VINK_AUTH_SIGN_IN
unset VINK_PROD_RESEND_FROM VINK_TEST_RESEND_FROM VINK_PUBLIC_ORIGIN
cat > "$fixture/public.env" <<CONF
VINK_POSTGRES_VOLUME=vink-render-check-postgres
VINK_PUBLIC_ORIGIN=https://qualification.example.test
VINK_TEST_HOST_PORT=3300
VINK_PROD_INBOX_DIR=$fixture/inbox
VINK_PROD_INGEST_DIR=$fixture/ingest
VINK_PROD_NOTIFICATIONS_IMAGE=ghcr.io/loveoverflowcom/vink-notifications:render-fixture
VINK_PROD_DB_NETWORK=vink-render-check-db
VINK_PROD_EDGE_NETWORK=vink-render-check-edge
VINK_PROD_POSTGRES_IMAGE=postgres:17
CONF
for realm in PROD TEST; do
    cat >> "$fixture/public.env" <<CONF
VINK_${realm}_APP_IMAGE=ghcr.io/loveoverflowcom/vink-web:render-fixture
VINK_${realm}_IDENTITY_IMAGE=ghcr.io/loveoverflowcom/vink-identity:render-fixture
VINK_${realm}_WORKER_IMAGE=ghcr.io/loveoverflowcom/vink-worker:render-fixture
VINK_${realm}_PLATFORM=linux/amd64
VINK_${realm}_UID=10001
VINK_${realm}_SECRETS_DIR=$fixture
VINK_${realm}_SOCKET_DIR=$fixture/socket
VINK_${realm}_RESEND_SECRETS_DIR=$fixture/resend
VINK_${realm}_AUTH_MAIL_IDENTITY_IP=192.0.2.10
CONF
done

for realm in production test; do
    if [[ $realm == production ]]; then
        flag=VINK_PROD_AUTH_SIGN_IN
        other=VINK_TEST_AUTH_SIGN_IN
        sender=VINK_PROD_RESEND_FROM
        mail=vink-prod-auth-mail
        db=default
    else
        flag=VINK_TEST_AUTH_SIGN_IN
        other=VINK_PROD_AUTH_SIGN_IN
        sender=VINK_TEST_RESEND_FROM
        mail=vink-test-auth-mail
        db=auth-test-db
    fi
    base=(docker compose --project-name vink-auth-render --env-file "$fixture/public.env"
        -f services/postgres/compose.yaml -f "infra/$realm/compose.yaml")
    render=("${base[@]}" -f "infra/$realm/auth-resend.compose.yaml" config --format json)
    # Omitting the overlay must not allow a same-named shell flag to activate App.
    env "$flag=enabled" "${base[@]}" config --format json | jq -e '
        .services.app.environment.VINK_AUTH_SIGN_IN == "disabled"
        and .services.identity.environment.VINK_IDENTITY_RESEND_API_KEY_FILE == null' >/dev/null
    # Omitting a required sender must fail before anything could start.
    if "${render[@]}" >/dev/null 2>&1; then
        echo "$realm accepted an auth overlay without its required sender" >&2
        exit 1
    fi
    env "$sender=no-reply@$realm.example" "${render[@]}" | jq -e --arg mail "$mail" --arg db "$db" '
        .services.app.environment.VINK_AUTH_SIGN_IN == "disabled"
        and .services.worker.environment.VINK_AUTH_SIGN_IN == "disabled"
        and .services.app.depends_on.identity.condition == "service_healthy"
        and .networks["auth-mail"].name == $mail and .networks["auth-mail"].external
        and (.services.identity.networks | keys == ["auth-mail", $db])
        and .services.identity.environment.VINK_IDENTITY_AUTHORITY == "native"
        and .services.identity.environment.VINK_IDENTITY_MAIL_TRANSPORT == "resend-api"
        and .services.identity.environment.VINK_IDENTITY_RESEND_API_KEY_FILE == "/run/vink-resend/api-key"
        and .services.identity.environment.VINK_IDENTITY_RESEND_PROXY == "http://vink-resend-egress:3128"
        and (.services.identity.volumes | any(.target == "/run/vink-resend" and .read_only))
        and (.services.postgres.ports // [] | length == 0)
        and (.services.identity.ports // [] | length == 0)' >/dev/null
    if [[ $realm == test ]]; then
        env "$sender=no-reply@fixture.example" "${render[@]}" | jq -e '
            .services.app.environment.VINK_PUBLIC_ORIGIN == "https://qualification.example.test"
            and (.networks.default.internal // false | not)
            and .networks["auth-test-db"].internal
            and (.services.app.networks | keys == ["default"])
            and (.services.worker.networks | keys == ["default"])
            and (.services.postgres.networks | keys == ["auth-test-db","default"])
            and (.services.postgres.networks["auth-test-db"].aliases | index("postgres") != null)
            and (.services.app.ports | length == 1 and .[0].host_ip == "127.0.0.1")' >/dev/null
    fi
    # A flag in the wrong realm cannot enable this App.
    env "$sender=no-reply@$realm.example" "$other=enabled" "${render[@]}" | jq -e '
        .services.app.environment.VINK_AUTH_SIGN_IN == "disabled"' >/dev/null
    env "$sender=no-reply@$realm.example" "$flag=enabled" "${render[@]}" | jq -e '
        .services.app.environment.VINK_AUTH_SIGN_IN == "enabled"
        and .services.worker.environment.VINK_AUTH_SIGN_IN == "disabled"' >/dev/null
done
echo "Auth overlay Docker Compose renders: safe base, prerequisites, defaults and separate activation OK"
