# Cấu hình và khôi phục Vink production

Mốc 22/11/2026, release `a9d6a4f9fb0a35b1e0e56f843132d4e3fe5dbb20`. Các lệnh VPS chạy
trên host quản trị đã xác minh SSH host key, với quyền đọc tương ứng. Lưu bản sao cấu hình
có bí mật trong kho vận hành riêng. Các lệnh kiểm kê dưới đây chỉ in metadata hoặc giá trị
cấu hình được liệt kê rõ.

## Cloudflare, Spaceship và VPS

| Thành phần | Lấy cấu hình đang dùng | Thiết lập lại |
| --- | --- | --- |
| Đăng ký tên miền | Spaceship → `vink.forum` → Nameservers, DNSSEC. Kiểm DS ở parent bằng `dig +trace DS vink.forum` và NS qua `dig +trace NS vink.forum`. | So khớp zone trước khi đổi NS. Gỡ DS của nhà cung cấp cũ và chờ TTL cũ trước khi đổi sang NS Cloudflare. Khi zone Active và DNS trả đúng, bật DNSSEC trong Cloudflare, đặt DS mới ở Spaceship và kiểm DS tại parent. [Checkpoint và thứ tự chuyển DNS](cloudflare-dns-20261119.md). |
| DNS và TLS | Cloudflare → zone `vink.forum` → DNS Records/Export và SSL/TLS. Đọc từng record và trạng thái proxy trước khi khôi phục. | Production dùng `carlane.ns.cloudflare.com`, `drew.ns.cloudflare.com` và DS `2371/13/2` tại mốc này. Gắn apex và www qua Pages Custom domains, giữ `origin-prod.vink.forum` DNS-only và chứng chỉ đúng hostname. |
| Pages và asset | Workers & Pages → `vink-production` → Deployments, Custom domains, Settings. Với Wrangler đã đăng nhập đúng account và có quyền Pages, chạy `npx --no-install wrangler pages deployment list --project-name vink-production --environment production` từ `infra/cloudflare` của release đã cài dependencies từ lock. | Giữ Direct Upload, production branch `main`, advanced-mode Worker và binding `ORIGIN_KEY` riêng của production. Lấy asset từ image Web đúng digest và theo trình tự trong [runbook phát hành](cloudflare-edge.md#từ-build-đến-receipt). |
| Origin Caddy | Đọc vhost trên host ở `/root/cantos-services/edge/vink-origin-prod.caddy` và `vink-prod.caddy`. Container `cantos-edge` mount thư mục này thành `/etc/caddy`. Xem tên và path file riêng qua metadata của container/unit. | Đồng bộ Pages `ORIGIN_KEY` và Caddy `VINK_ORIGIN_KEY`. Pages chỉ cho xem tên binding, nên lấy giá trị từ bản lưu riêng của origin hoặc cấp lại đồng thời hai đầu nếu mất bản lưu. Validate rồi reload Caddy. Giữ origin trả `403` khi thiếu khóa. [Vhost mẫu](origin-prod.caddy.example). |
| Runtime và dấu vết phát hành | `readlink -f /opt/vink-prod/current`, `systemctl cat vink-prod.service`, `podman inspect --format '{{.ImageName}}' vink-prod_app_1 vink-prod_identity_1 vink-prod_worker_1`. | Release env ở `/etc/vink-prod/release.env`, env từng service ở `/etc/vink-prod/runtime/`. Lấy cấu hình origin riêng từ `/etc/vink-prod/pages-origin.env` trong phiên quản trị. Receipt thủ công và bản trước cutover ở `/opt/vink-prod/pages-bootstrap/<sha>/`. Dùng manifest theo thứ tự dưới đây và các digest trong receipt. [Khôi phục origin và edge](cloudflare-edge.md#cutover-và-khôi-phục). |

Metadata bổ sung để đối soát từ máy quản trị:

```sh
dig @carlane.ns.cloudflare.com vink.forum A +short
dig @drew.ns.cloudflare.com vink.forum A +short
curl -fsS https://vink.forum/health/ready
curl -sS -o /dev/null -w '%{http_code}\n' https://origin-prod.vink.forum/health/ready
```

Lệnh cuối kỳ vọng `403`. So khớp deployment ID trong [mốc Cloudflare](cloudflare-edge.md#mốc-triển-khai-ngày-22112026).
Kiểm lại Preview và qualification trước khi dùng controller cho bản kế tiếp. [Lệnh Wrangler Pages](https://developers.cloudflare.com/workers/wrangler/commands/pages/).

## Khóa phát hành và khóa gửi thư

| Secret | Nơi quản lý và cách lấy |
| --- | --- |
| `VINK_RELEASE_DISPATCH_TOKEN` | GitHub repo → Settings → Secrets and variables → Actions. GitHub chỉ cho xem metadata. Lấy bản sao từ project GCP `vink-d0846`; token cần quyền Actions write đúng repo để dispatch. |
| `VINK_PROD_RESEND_API_KEY` | Google Secret Manager cùng project, phiên bản đã kiểm lúc triển khai là `1`. Chọn phiên bản enabled khớp với khóa **Vink production OTP** ở Resend → API keys, quyền Sending access, domain `mail.vink.forum`. |
| `ORIGIN_KEY` / `VINK_ORIGIN_KEY` | Pages production binding và cấu hình origin riêng trên VPS. Hai tên dùng cùng giá trị theo hợp đồng Worker–Caddy. |

```sh
gh secret list --repo loveoverflowcom/vink
for name in VINK_RELEASE_DISPATCH_TOKEN VINK_PROD_RESEND_API_KEY; do
  gcloud secrets versions list "$name" --project=vink-d0846 --format='table(name,state)'
done
```

Khi cần dùng khóa để chạy qualification, chọn phiên bản dispatch đã xác minh (biến
`DISPATCH_VERSION`) và dùng tài khoản có quyền đọc đúng secret. Ví dụ trong Bash với shell
tracing tắt. `publish=false` và `website=false` chỉ chạy qualification trên GitHub.

```bash
set +x
dispatch_token="$(gcloud secrets versions access "${DISPATCH_VERSION:?chọn phiên bản dispatch}" \
  --secret=VINK_RELEASE_DISPATCH_TOKEN --project=vink-d0846)" &&
  GH_TOKEN="$dispatch_token" gh workflow run publish-images.yml --repo loveoverflowcom/vink \
    --ref develop -f platform=linux/amd64 -f publish=false -f website=false
unset dispatch_token
```

## Đăng nhập bằng mã email trên VPS

1. **Resend và DNS.** Chọn domain `mail.vink.forum`, region Tokyo, bật gửi thư, giữ nhận
   thư tắt. Lấy DKIM TXT `resend._domainkey.mail.vink.forum` và hai CNAME từ Resend → Domains.
   Tại mốc này `rsend.mail.vink.forum → rsend-apne1.forge.rmta.net` và
   `send.mail.vink.forum → send.forge.rmta.net` đều DNS-only. Nhập chính xác TXT của domain
   đã chọn, đọc lại từ hai NS Cloudflare rồi chờ Resend báo Verified. Tạo khóa Sending access
   riêng của domain này, lưu vào Secret Manager và cài vào VPS như dưới đây. [Quyền API key Resend](https://resend.com/docs/dashboard/api-keys/introduction).
2. **Proxy.** Dùng [Squid policy](../auth-mail/squid.conf.example) với listener
   `10.89.5.2:3128`, peer `10.89.5.3/32` và resolver `10.89.6.1`. Cấu hình ở
   `/etc/vink-prod/resend-egress/squid.conf`, Quadlet ở
   `/etc/containers/systemd/vink-prod-resend-egress.container`. Đọc lại bằng
   `systemctl cat vink-prod-resend-egress.service`, `podman network inspect` cho đúng hai mạng,
   và `ufw status numbered` trên VPS. Proxy dùng image `docker.io/ubuntu/squid` pin digest
   `sha256:6a097f68bae708cedbabd6188d68c7e2e7a38cedd05a176e1cc0ba29e3bbe029`.
   Khi dựng lại, pull digest, kiểm `squid -k parse` trên image đó, thêm Quadlet với hai
   attachment theo bảng. Quadlet dùng tên container `vink-resend-egress`, mount file
   policy chỉ đọc vào `/etc/squid/squid.conf`, chạy `/usr/sbin/squid -N -f
   /etc/squid/squid.conf` và có `WantedBy=multi-user.target`. Sau đó chạy `systemctl daemon-reload` và start service. Proxy chỉ CONNECT
   tới `api.resend.com:443`, không publish cổng, không mount key. Kiểm TLS qua proxy
   và các phép chặn theo [runbook mail](../auth-mail/README.md#egress-proxy-riêng-không-cấp-internet-trực-tiếp-cho-identity).
3. **Identity và App.** Tạo bản sao root-only của release env, drop-in và scope mạng trước
   khi đổi. Thêm đúng một lần các biến dưới đây vào `/etc/vink-prod/release.env`. Bản
   `/etc/vink-prod/runtime/identity-runtime.env` có `VINK_IDENTITY_AUTHORITY=password` làm
   cấu hình nền. Overlay đặt `native` với cả password và email, và ưu tiên hơn `env_file`.

| Mạng Podman | Bridge và IP | Firewall đã áp dụng |
| --- | --- | --- |
| `vink-prod-auth-mail` internal, DNS bật | `vinkpmail0`, `10.89.5.0/24`, gateway .1, proxy .2, Identity .3 | .3 → .1 port 53 TCP/UDP và .3 → .2 TCP 3128 |
| `vink-prod-auth-mail-uplink` riêng của proxy, DNS bật | `vinkpmailwan0`, `10.89.6.0/24`, gateway .1, proxy .2 | .2 → .1 port 53 TCP/UDP và .2 → TCP 443 qua interface WAN (`ens18` trên host hiện tại) |

```dotenv
VINK_PROD_AUTH_SIGN_IN=enabled
VINK_PROD_RESEND_FROM=no-reply@mail.vink.forum
VINK_PROD_RESEND_SECRETS_DIR=/etc/vink-prod/resend
VINK_PROD_AUTH_MAIL_IDENTITY_IP=10.89.5.3
```

Khi cấp khóa mới, đưa tệp khóa riêng có mode `0600` vào Secret Manager bằng
`gcloud secrets versions add VINK_PROD_RESEND_API_KEY --project=vink-d0846 --data-file=PATH`.
Nếu dựng project mới, tạo secret bằng `gcloud secrets create` với cùng tên, replication
`automatic` và `--data-file=PATH`. Đọc metadata sau khi lưu và chọn phiên bản vừa tạo.
[Xem cách quản lý phiên bản secret](https://docs.cloud.google.com/secret-manager/docs/add-secret-version).

Để lấy lại khóa từ GCP, chạy trên máy quản trị trong Bash với tracing tắt. Đặt
`RESEND_VERSION` theo phiên bản đã đối chiếu và `VINK_PROD_SSH_HOST` theo SSH alias của
host Vink đã pin host key. Lệnh thay file theo cách nguyên tử, sau khi kiểm payload.
Dùng cùng cách khi xoay khóa. Mount thư mục giúp Identity đọc inode mới ở lần gửi kế tiếp.

```bash
(
  set +x
  set -euo pipefail
  umask 077
  key_file="$(mktemp)"
  trap 'rm -f -- "$key_file"' EXIT
  gcloud secrets versions access "${RESEND_VERSION:?chọn phiên bản Resend}" \
    --secret=VINK_PROD_RESEND_API_KEY --project=vink-d0846 --out-file="$key_file"
  grep -qEx 're_[A-Za-z0-9_-]{24,}' "$key_file"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=yes "${VINK_PROD_SSH_HOST:?chọn host Vink}" '
    set -eu
    umask 077
    install -d -o 10001 -g 10001 -m 0700 /etc/vink-prod/resend
    staged="$(mktemp /etc/vink-prod/resend/.api-key.XXXXXX)"
    trap "rm -f -- \"\$staged\"" EXIT
    cat > "$staged"
    grep -qEx "re_[A-Za-z0-9_-]{24,}" "$staged"
    chown 10001:10001 "$staged"
    chmod 0600 "$staged"
    mv -T -- "$staged" /etc/vink-prod/resend/api-key
  ' < "$key_file"
)
```

Trên VPS, drop-in `/etc/systemd/system/vink-prod.service.d/60-auth-resend.conf` giữ
`Wants=`/`After=vink-prod-resend-egress.service` và thay cả `ExecStart` lẫn `ExecStop`.
Dùng thứ tự manifest dưới đây trong cả hai lệnh, giữ phần `chown` socket và danh sách
service từ [unit gốc](vink-prod.service). Đọc lại bằng `systemctl cat vink-prod.service`.
Sau `systemctl daemon-reload`, áp dụng riêng cho Identity bằng Bash trên VPS.

```bash
export PODMAN_COMPOSE_PROVIDER=/usr/bin/podman-compose
vink_compose=(podman compose --project-name vink-prod --env-file /etc/vink-prod/release.env
  -f /opt/vink-prod/current/services/postgres/compose.yaml
  -f /opt/vink-prod/current/infra/production/compose.yaml
  -f /opt/vink-prod/current/infra/production/auth-resend.compose.yaml)
"${vink_compose[@]}" config > /dev/null &&
  "${vink_compose[@]}" up -d --no-deps --force-recreate identity
podman exec vink-prod_identity_1 curl -fsS --unix-socket /run/vink-identity/identity.sock \
  http://identity/health/ready
```

Kỳ vọng `authority=available`, `mail_transport=resend_api`, schema current và queue rỗng
sau khi gửi xong. Kiểm đúng App có cổng mở, phiên khách và health qua HTTPS. Dùng chính
`/toi` để xin mã vào mailbox được phép, xác nhận thư đã vào hộp thư, nhập mã trên trình
duyệt đã xin mã, rồi đăng xuất và tải lại. Kiểm riêng đường mật khẩu nếu đổi App hoặc
Identity image. [Kiểm thư, sự cố và rollback](../../docs/development/auth-operations.md).

Khi phải quay về chỉ dùng mật khẩu, khôi phục release env và cấu hình unit trước Resend,
reload systemd, rồi recreate riêng Identity với hai manifest nền và xác minh session.
Muốn chặn mọi cấp phiên mới thì đặt `VINK_PROD_AUTH_SIGN_IN=disabled` và recreate App
bằng cùng stack đang chạy. Giữ database, socket và schema. Khi deploy mới, kiểm candidate
giữ overlay Resend, cổng App và quyền đọc file key trước lúc chuyển image.
