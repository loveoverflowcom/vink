# Bằng chứng test-domain/container

## Triển khai thực trên VPS ngày 15/11/2026

Triển khai thử `test.vink.forum` trên VPS đang chạy Cantos. Vink dùng source
`a677b0d4b5c53104ae03961a2d24c73d1d9f2ec5` và Podman rootful 5.4.2 trên `linux/amd64`.
[Workflow publish #43683453768](https://github.com/loveoverflowcom/vink/actions/runs/43683453768)
build ba image, kiểm loader config, non-root và tài nguyên Web rồi push GHCR. Trên VPS,
`podman pull --platform linux/amd64` và `podman image inspect` xác nhận từng digest và nhãn
`org.opencontainers.image.revision`.

| Service Compose | Package GHCR | Digest |
| --- | --- | --- |
| `app` | `ghcr.io/loveoverflowcom/vink-web` | `sha256:373cea23e8e3fc5d9cf744a883adebc7bbfc10a4223456b25a740ab2c47f5b79` |
| `worker` | `ghcr.io/loveoverflowcom/vink-worker` | `sha256:137fd857dfe1cc21302c767b85fa439034493f1f58073ad7696d3e06c42ace4b` |
| `identity` | `ghcr.io/loveoverflowcom/vink-identity` | `sha256:b1af7c6cde9a82ed93944cf6f125854c1882d38f3c53abae75aa2fc73a9b37d4` |

### Compose và ranh giới PostgreSQL

- Project `vink-test` ghép [PostgreSQL base](../../services/postgres/compose.yaml) và
  [test Compose](../../infra/test/compose.yaml). Dùng `PODMAN_COMPOSE_PROVIDER=/usr/bin/podman-compose`
  (1.3.0), đường dẫn tuyệt đối cho hai file và `--env-file /etc/vink-test/compose.public.env`.
  Provider Docker Compose mặc định cần Podman API socket. Bản native chạy qua Podman CLI.
- Cấu hình công khai đặt origin `https://test.vink.forum`, Web bind `0.0.0.0:3000` trong
  container và publish `127.0.0.1:3300` trên host. App và Identity dùng UID `10001`.
  Directory `/run/vink-test-identity` owner `10001` mode `0700`, App mount chỉ đọc,
  Identity tạo socket và kiểm caller UID. Giữ `VINK_AUTH_SIGN_IN=disabled` và
  `VINK_WORKER_EVALUATION=disabled`.
- PostgreSQL `17.11-alpine` chạy trong container Vink, gắn volume riêng `vink-test-postgres`
  và chỉ vào mạng `vink-test_default`. `podman port` trả rỗng cho PostgreSQL. Hai DB
  `vink` và `vink_identity` có cặp migrator/runtime role riêng. Bootstrap, provision, runtime
  và migration nạp từ các file riêng trong `/etc/vink-test`. App và Worker dùng role runtime
  của App, còn Identity dùng role runtime của Identity. Cantos có các tiến trình và kho dữ
  liệu độc lập.
- Thứ tự quan sát được là `up -d postgres` → `pg_isready` qua TCP → `run --rm provision` →
  `run --rm provision --check` → `run --rm migrate-app` → `run --rm migrate-identity` →
  `up -d identity app worker`. App migration cuối `20261119000000`, Identity migration cuối
  `20261114000000`. App và Identity báo schema `current` và role `restricted`. Worker
  quan sát `running`. PostgreSQL, App và Identity có health check `healthy`.

UFW ban đầu chặn DNS và TCP giữa các container trên bridge Compose. Lượt chạy thêm rule
giới hạn theo interface và subnet Vink cho DNS TCP/UDP tới gateway của bridge, và TCP 5432
chỉ giữa các thành viên cùng bridge DB. Bridge ingress có rule DNS riêng và TCP 3000 giữa
Caddy và Web. Các rule này thuộc cấu hình VPS đã chạy.

### Caddy và DNS

Compose overlay đã kiểm `config` trên VPS. Lượt thử gắn App đang chạy vào mạng ingress bằng
`podman network connect --alias vink-test-web vink-test-ingress vink-test_app_1`.
PostgreSQL, Worker và Identity chỉ ở mạng Compose mặc định.

```yaml
services:
  app:
    networks:
      default: {}
      ingress:
        aliases: [vink-test-web]
networks:
  default:
    name: vink-test_default
    external: true
  ingress:
    name: vink-test-ingress
    external: true
```

Caddy được gắn ngay vào bridge internal `vink-test-ingress`. Quadlet drop-in của edge bổ sung
`[Container] Network=vink-test-ingress` cho lần tạo lại container. Kiểm readiness từ chính
container Caddy qua `http://vink-test-web:3000/health/ready`, chạy `caddy validate` rồi reload
cấu hình. Caddyfile chính import thêm vhost Vink dưới đây. API Cantos ở endpoint kiểm phiên
trả HTTP `401` trước và sau reload.

```caddyfile
test.vink.forum {
    header X-Robots-Tag "noindex, nofollow"
    encode /pkg/* zstd gzip
    reverse_proxy vink-test-web:3000
}
```

- Zone `vink.forum` do `launch1.spaceship.net` và `launch2.spaceship.net` quản lý.
  Advanced DNS UI từ 0 record tạo đúng `test A 42.96.18.245`, TTL 300 giây. Đọc lại từ
  cả hai nameserver có thẩm quyền và resolver mặc định đều trả cùng IPv4.
- `curl` với kiểm tra chứng chỉ mặc định trả HTTP `200` cho `/`, `/health/ready`,
  `/api/v1/works` và `/pkg/vink.js`. Readiness trả `database=ok`, `schema=current`,
  `role=restricted`. API works trả JSON hợp lệ và trình duyệt hiện trang Khám phá với thư viện
  trống. Bấm thu gọn và mở lại điều hướng làm `aria-expanded` đổi `true → false → true`.
- `GET /pkg/vink_bg.wasm` trả `200`. Bản gốc có `104636914` byte và lượt GET giới hạn
  12 giây chưa tải hết. Sau khi bật nén dưới `/pkg/*`, một GET qua HTTPS có
  `Content-Encoding: zstd` tải đủ `4154363` byte. [Vink test](https://test.vink.forum/).

**Mức đạt trong lượt 15/11:** build, publish, pull theo digest, Compose chạy với DB riêng,
DNS, HTTPS, SSR, API và tương tác điều hướng công khai đã quan sát. Đăng nhập/Bàn viết có phiên,
thử restart/reboot và phục hồi, thử gia hạn TLS và rollback vẫn `NOT_RUN`. Cloudflare, gói
cấu hình trên Google Cloud và pull từ máy thứ hai là công việc tiếp theo.

## Kiểm tra nền ngày 13/11/2026

Ngày13/11/2026, Mac-mini-cua-Manh.local (darwin/arm64), worktree riêng từ
`origin/develop@34a9e5d` sau #188. Không thay checkout đang dùng hoặc worktree moderation.

## Phạm vi kiểm chứng

- `cargo test --locked -p xtask`: PASS, 80 unit tests + 1 CLI integration; 1 ignored test cũ
  của owner khác không đổi. Infra có 8 pure tests: inventory/digest/path, đổi origin không đổi
  image, DNS idempotency/case normalization, scoped rollback, drift/authority/scope, target invalid,
  A/CNAME replacement/TTL, unknown fields và không render credential/migrator/auth unlock.
- `cargo clippy --locked -p xtask --all-targets -- -D warnings`: PASS.
- `cargo fmt --all --check`, `git diff --check`: PASS.
- YAML Ruby parser cho Compose và manual workflow: PASS syntax, không thay thế Compose runtime.
- `cargo xtask infra --help`: PASS, đúng command surface.
- `cargo xtask infra check infra/test/test.example.toml`: expected FAIL (unknown platform/digests).
  Đây là kiểm fail-closed, không phải inventory ready.
- Independent source review tìm/sửa5 vấn đề: brand provenance thiếu trong context, ARM runner,
  DNS self-CNAME qua case, snapshot FQDN case, provisioning order/context. Không còn source blocker
  qua review, nhưng chưa có Linux runtime evidence.
- `git ls-remote https://github.com/actions/checkout.git refs/tags/v7.0.1`: pin checkout
  `3d3c42e5aac5ba805825da76410c181273ba90b1` xác minh đúng tag chính thức.

Không đọc/poll/chờ GitHub Actions, checks hoặc workflow logs. Manual workflow không dispatch.

## Prerequisites và trạng thái thật

`podman machine list` cho thấy chỉ có VM `cantos-deploy`, chưa chạy; `podman info` trả connection
refused. Không tự start VM Cantos, cài runtime mới, sửa security settings hoặc tạo Linux target.
Mac chỉ còn khoảng1,7GiB trống sau compile xtask, không phù hợp release build3 images. Image
build/loader smoke/SSR và Linux peer UID/lifecycle là **NOT_RUN**, không gọi cấu hình là đã chạy.

Không có target VPS/IP/CPU/proxy/secrets được giao. Không có Spaceship tool đã kết nối trong
catalog; chưa xác minh account, authoritative NS hoặc đầy đủ records. DNS diff thật/apply/readback
và HTTPS/TLS renew là **NOT_RUN**. MCP/source docs chỉ là phương án, không là bằng chứng auth.

| AC | Evidence | Trạng thái |
| --- | --- | --- |
| AC-01 test config riêng | inventory + Compose, unit fail-closed; không có secret/fixture | PASS code; target NOT_RUN |
| AC-02 OCI source build + SSR/assets | Containerfile dùng xtask, context Git archive + brand provenance | NOT_RUN image build/smoke |
| AC-03 lifecycle/socket UID | manifest3 process; guard/UID contract theo source | NOT_RUN Linux topology/restart/SIGTERM |
| AC-04 DB/role/migrate boundary | runtime env tách, one-shot provision/migrate, DB không host port | PASS config; NOT_RUN DB runtime |
| AC-05 origin runtime | pure test cùng digest đổi origin; không business code đổi | PASS config; NOT_RUN same-image runtime |
| AC-06 GHCR/release/rollback | manual-only workflow, source metadata, remote digest recording, runbook | PASS source; NOT_RUN publish/registry digest |
| AC-07 DNS | pure snapshot plan/drift/idempotency/scoped rollback | PASS pure; NOT_RUN provider apply/readback |
| AC-08 test online | cần endpoint/HTTPS/SSR/assets/health/flows thật | NOT_RUN |

Các tests DNS dùng địa chỉ/hostname tổng hợp, không phải đích test được chọn. Mẫu inventory có
empty fields chủ động, không có digest giả. Không cung cấp release digest khi chưa publish.

## Mức bàn giao và bước tiếp

**Code-ready:** foundation + focused checks như trên. **Image-built:** chưa. **Image-published:**
chưa. **DNS-applied:** chưa. **Test-online:** chưa. Không deploy hoặc đổi production/NS/apex/www.

Khi có target/runtime/disk và quyền phù hợp: chạy exact commands của [runbook](../../infra/test/README.md),
release build3 images rồi Linux smoke với PostgreSQL scratch được cấp quyền, peer đúng/sai UID,
Identity restart và SIGTERM. Chỉ sau đó publish explicit, lấy remote digest thật, kiểm migration
compatibility/rollback, xác minh Spaceship auth+NS+endpoint rồi DNS diff/apply/readback và HTTPS.
Không bỏ peer/auth/profile guard, dùng local outbox làm email thật, hoặc coi noindex là access control.
