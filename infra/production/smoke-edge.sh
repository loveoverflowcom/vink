#!/bin/sh
# Public, read-only checks against Caddy and the production Cloudflare proxy.
set -eu

usage() {
  echo "Usage: $0 direct [VPS_IP] | edge [APEX_EDGE_IP WWW_EDGE_IP | --release SHA]" >&2
  exit 2
}

release=
if [ "${1-}" = edge ] && [ "${2-}" = --release ] && [ "$#" = 3 ]; then
  release=$3
  printf '%s\n' "$release" | grep -Eq '^[0-9a-f]{40}$' || exit 2
  set -- edge
fi

case "${1-}:$#" in
  direct:1|direct:2|edge:1|edge:3) ;;
  *) usage ;;
esac

mode=$1
apex=vink.forum
www=www.vink.forum
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM

fail() {
  printf 'FAIL %s %s\n' "${url:-$mode}" "$*" >&2
  exit 1
}

header() {
  awk -v name="$1" '
    index(tolower($0), tolower(name) ":") == 1 {
      sub(/^[^:]+:[[:space:]]*/, "")
      sub(/\r$/, "")
      print
    }
  ' "$tmp/headers"
}

fetch() {
  host=$1 scheme=$2 path=$3 expected_status=$4 expected_type=$5
  shift 5
  url="$scheme://$host$path"
  ip=
  if [ "$mode" = direct ]; then
    ip=$origin_ip
  elif [ "$host" = "$apex" ]; then
    ip=$apex_ip
  else
    ip=$www_ip
  fi

  if [ -n "$ip" ]; then
    port=443
    if [ "$scheme" = http ]; then port=80; fi
    set -- --resolve "$host:$port:$ip" "$@"
  fi
  : > "$tmp/body"
  actual_status=$(curl --noproxy '*' --no-progress-meter --show-error --compressed \
    --connect-timeout 10 --max-time 120 --max-filesize 180000000 \
    --dump-header "$tmp/headers" --output "$tmp/body" --write-out '%{http_code}' "$@" "$url")
  [ "$actual_status" = "$expected_status" ] || fail "HTTP $actual_status, expected $expected_status"

  if [ -n "$expected_type" ]; then
    content_type=$(header Content-Type)
    [ "${content_type%%;*}" = "$expected_type" ] || fail "Content-Type ${content_type:-missing}"
  fi
  ray=$(header CF-Ray)
  if [ "$mode" = edge ]; then
    [ -n "$ray" ] || fail 'missing CF-Ray'
    case "$(header CF-Cache-Status)" in
      ''|BYPASS|DYNAMIC) ;;
      HIT|MISS|EXPIRED|REVALIDATED)
        case "$path" in /_v/*/pkg/*) ;; *) fail 'shared cache on dynamic route';; esac ;;
      *) fail "unexpected CF-Cache-Status: $(header CF-Cache-Status)" ;;
    esac
  else
    [ -z "$ray" ] || fail 'expected direct Caddy'
  fi
  printf '%s %s %s\n' "$mode" "$actual_status" "$url"
}

origin_ip=
apex_ip=
www_ip=
if [ "$mode" = direct ]; then
  origin_ip=${2-}
else
  apex_ip=${2-}
  www_ip=${3-}
fi

fetch "$apex" http '/toi?edge_smoke=1' 308 ''
[ "$(header Location)" = 'https://vink.forum/toi?edge_smoke=1' ] || fail 'HTTP → HTTPS changed path/query'
fetch "$www" https '/toi?edge_smoke=1' 301 ''
[ "$(header Location)" = 'https://vink.forum/toi?edge_smoke=1' ] || fail 'www → apex changed path/query'

fetch "$apex" https / 200 text/html
if [ -n "$release" ]; then
  grep -Fq "/_v/$release/pkg/vink.js" "$tmp/body" || fail 'SSR release differs'
fi
grep -Fq '<link href="https://vink.forum/" rel="canonical">' "$tmp/body" || fail 'canonical missing'
if grep -iq 'name="robots"[^>]*noindex' "$tmp/body" ||
  printf '%s\n' "$(header X-Robots-Tag)" | grep -iqE 'noindex|(^|[[:space:],:])none([[:space:],]|$)'; then
  fail 'public home is noindex'
fi
for path in /toi /ban-viet; do
  fetch "$apex" https "$path" 200 text/html
  grep -iq 'name="robots"[^>]*noindex' "$tmp/body" || fail 'private route is missing noindex'
done

fetch "$apex" https /health/live 200 application/json
fetch "$apex" https /health/ready 200 application/json
python3 - "$tmp/body" <<'PY'
import json
import sys

expected = {"database": "ok", "role": "restricted", "schema": "current"}
with open(sys.argv[1]) as body:
    observed = json.load(body)
checks = observed.get("checks", {})
if observed.get("status") != "ready" or any(checks.get(key) != value for key, value in expected.items()):
    sys.exit(f"FAIL readiness: {observed}")
PY

fetch "$apex" https /api/v1/works 200 application/json
[ -z "$(header Content-Encoding)" ] || fail 'API encoding differs from direct Caddy'
etag=$(header ETag)
cache_control=$(header Cache-Control)
case "$etag" in
  \"*\" ) ;;
  *) fail 'API response is missing a strong ETag' ;;
esac
[ "$cache_control" = 'private, no-cache' ] || fail "unexpected API Cache-Control: $cache_control"
fetch "$apex" https /api/v1/works 304 '' --header "If-None-Match: $etag"
[ ! -s "$tmp/body" ] || fail '304 has a body'
[ "$(header ETag)" = "$etag" ] || fail '304 changed ETag'
[ "$(header Cache-Control)" = "$cache_control" ] || fail '304 changed Cache-Control'

fetch "$apex" https /api/v1/__vink_edge_probe__ 404 application/json
fetch "$apex" https /__vink_edge_probe__ 404 text/html
fetch "$apex" https /tac-pham/00000000-0000-0000-0000-000000000000 404 text/html
fetch "$apex" https /pkg/__vink_edge_probe__.js 404 ''
fetch "$apex" https /robots.txt 404 ''
fetch "$apex" https /sitemap.xml 404 ''

for asset in 'vink.js text/javascript' 'vink.css text/css' 'vink_bg.wasm application/wasm' \
  'fonts/be-vietnam-pro-400.woff2 font/woff2' 'fonts/literata-variable.woff2 font/woff2'; do
  path=${asset% *}
  type=${asset##* }
  fetch "$apex" https "${release:+/_v/$release}/pkg/$path" 200 "$type"
  [ -s "$tmp/body" ] || fail 'empty asset body'
  digest=$(openssl dgst -sha256 -r "$tmp/body")
  printf 'asset %s %s\n' "$path" "${digest%% *}"
  printf 'encoding %s %s\n' "$path" "$(header Content-Encoding)"
done
