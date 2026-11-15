# Phân phối frontend test qua Cloudflare

Phạm vi là **test Vink**. [Bằng chứng triển khai](../../docs/development/cloudflare-test-evidence.md)
ghi trạng thái provider, DNS, HTTP, browser và reader Google Cloud riêng biệt. `production` trong
Pages chỉ là nhánh `main` của project **test**, không mở production/auth của Vink.

## Kiến trúc và giới hạn

| Đường | Owner phục vụ | Cache |
| --- | --- | --- |
| 72 tài nguyên đã kê trong `/pkg/*` | Pages Static Assets từ đúng image Web | `public, max-age=0, must-revalidate`; tên chưa hoàn toàn content-addressed |
| `/pkg/vink_bg.wasm` | Caddy/VPS qua Pages Worker, stream nguyên nội dung | `no-store` |
| SSR, URL sâu, `/api/*`, `/health/*` | Caddy/VPS qua Pages Worker | fetch `cache: no-store`; giữ private/no-cache hoặc no-store của API; thêm no-store nếu origin không có |
| Asset không nằm trong inventory | Worker trả 404 text | `no-store`; không fallback HTML |

WASM hydration gốc có 104636914 byte, vượt giới hạn 25 MiB/file của
[Pages](https://developers.cloudflare.com/pages/platform/limits/) và
[Workers Static Assets](https://developers.cloudflare.com/workers/platform/limits/).
Kích thước gzip/zstd khi truyền HTTP không phải kích thước upload. Lượt này upload FE thật nhưng
giữ **một** WASM lớn tại origin. R2 chưa bật; chưa tối ưu/rebuild WASM hoặc chuyển nó lên storage
khác. Không tách API sang cross-origin, không dùng SPA fallback của Cantos.

`worker.mjs` là [Pages Advanced Mode](https://developers.cloudflare.com/pages/functions/advanced-mode/).
Worker chỉ chọn upstream từ release đã đóng gói, giữ `Origin`, cookie, body, query, status,
ETag và redirect; Caddy đặt Host upstream là `test.vink.forum`. Header hop và so host chặn vòng
lặp. Với hostname preview, CSRF vẫn dùng origin cấu hình của App, không thêm wildcard CORS.
`X-Vink-Delivery` phân biệt asset Cloudflare, asset origin và dynamic origin;
`X-Vink-Release` là digest inventory asset. Mọi phản hồi có `noindex, nofollow`.

SSR/API, Worker nghiệp vụ, Identity và PostgreSQL vẫn ở VPS. Không đưa Rust server, SQL,
credential, file env hay nội dung nháp vào bundle. Đăng nhập và evaluator giữ disabled.

## Đóng gói và upload

Node >=22, Python >=3.11 và Wrangler **4.151.2** trong lockfile. Cài bằng
`npm ci --ignore-scripts` trong thư mục này. Không dùng `npx` hoặc bản global ngẫu nhiên.
Compatibility date đã kiểm với runtime đi kèm là `2026-10-26`.

1. Pull image theo digest, inspect platform và `org.opencontainers.image.revision`. Tạo container
   export không start từ đúng digest, `podman cp` `/opt/vink/site/.` rồi xóa container export.
   Không lấy site của container có thể đã sửa mà không đối chiếu image.
2. Inventory public theo [owner infra/test](../test/README.md); chạy `cargo xtask infra check`.
3. Đóng gói ra directory **mới**, ngoài Git. `package.py` không đọc credential, gọi provider hay build:

```sh
python3 infra/cloudflare/package.py \
  --site /absolute/image-site --inventory /absolute/test.toml --output /absolute/release \
  --source-sha a677b0d4b5c53104ae03961a2d24c73d1d9f2ec5 \
  --origin https://origin-test.vink.forum \
  --account 8339afe44b4e8cd2b633a38a77f1db41 --project vink-test-frontend \
  --branch main --environment production
node --test infra/cloudflare/worker.test.mjs
infra/cloudflare/node_modules/.bin/wrangler pages functions build \
  --build-output-directory /absolute/release/public \
  --outdir /absolute/compiled-worker --compatibility-date 2026-10-26
```

Đối chiếu account/project/branch với API trước upload. Project chỉ Direct Upload, production
branch `main`, `fail_open=false`, không có env secret. Preview dùng branch khác main và
`--environment preview`. Đừng đổi byte trong directory đã stage; sửa source thì tạo release mới.
Manifest nằm **ngoài** `public/`. CLI nhận source SHA là lời khai của caller; provenance phải có
bằng chứng inspect/export riêng. SHA của Worker là input staged, không phải lời khẳng định về
byte bundle Cloudflare đã compile.

```sh
WRANGLER_SEND_METRICS=false infra/cloudflare/node_modules/.bin/wrangler pages deploy \
  /absolute/release/public --project-name vink-test-frontend --branch main \
  --commit-hash a677b0d4b5c53104ae03961a2d24c73d1d9f2ec5 --commit-dirty=true
```

Token chỉ đến từ môi trường publisher. Đọc lại **deployment ID** trả về và kiểm project,
environment, branch, source, stage success. Mất ACK phải đối chiếu provider trước retry. Kiểm
HTTPS URL deployment và mọi asset bằng byte/hash, rồi browser hydration; upload thành công
chưa phải site hoạt động. Test Node dùng fake fetch; chạy Workerd và deployed HTTP mới kiểm được
Fetch runtime. Không ghép `cache: no-store` với `cf.cacheTtl`: Workerd từ chối tổ hợp này.

## Domain và rollback

[Pages hỗ trợ subdomain ở DNS ngoài Cloudflare](https://developers.cloudflare.com/pages/configuration/custom-domains/).
Không cần chuyển nameserver toàn zone. Mapping của lượt này:

| Host Spaceship | Record | TTL |
| --- | --- | --- |
| `origin-test` | A `42.96.18.245` | 300 |
| `test` | CNAME `vink-test-frontend.pages.dev` | 300 |

Giữ `launch1.spaceship.net`, `launch2.spaceship.net`. Thêm origin đã được chủ sản phẩm cho phép
riêng trong chat ngày 15/11/2026. [Caddy vhost](origin-test.caddy.example) được ghép vào file
Vink đã import bởi edge hiện hữu. Không thêm proxy tranh cổng 80/443. Chờ DNS công khai trước
certificate issuance, validate/reload Caddy rồi kiểm TLS origin và mẫu API Cantos read-only.

Trước cutover: deployment với upstream origin riêng phải qua HTTP/browser; tạo Pages custom
domain association; snapshot **đầy đủ** records; `infra dns-plan` và fresh `infra dns-check`.
Chỉ đổi `test` theo diff. Readback UI/API và authoritative/public DNS; chờ Pages domain active
và cert hợp lệ; kiểm URL thật. Không coi `--resolve` hoặc URL Pages là bằng chứng resolver mặc
định đã chuyển.

Rollback: fresh scope phải vẫn là CNAME trên; `infra dns-rollback` tạo plan đảo về
`test A 42.96.18.245` TTL300, giữ record origin. Vhost trực tiếp `test.vink.forum` và chứng chỉ
Caddy vẫn tồn tại; đã kiểm read-only bằng `curl --resolve`. Chờ DNS rồi xác minh HTTPS/SSR/API.
Không xóa DB, volume, image hay migration. Để rollback chỉ edge code khi origin còn tốt, dùng
deployment đã biết **có origin riêng** hoặc stage lại Worker trước đó; hai preview ban đầu trỏ
`test` làm origin sẽ bị loop guard sau cutover và không phải rollback candidate.
Việc thực sự đảo DNS về VPS và thao tác Pages rollback vẫn cần bằng chứng riêng.

## Cấu hình Google Cloud và reader

Project `vink-d0846`, bucket `vink-d0846-test-config` ở `ASIA-SOUTHEAST1`, uniform bucket IAM,
public access prevention, object versioning. Billing account hiện có được liên kết theo xác
nhận của người dùng. Bucket chỉ chứa cấu hình **không bí mật**. Writer được quản lý object/bucket;
`vink-test-config-reader@vink-d0846.iam.gserviceaccount.com` có `roles/storage.objectViewer`
trên **bucket này**. Không có project role, DB grant hay service-account key cho reader.

Schema `vink-test-release/v1` ghi environment, image source SHA, inventory public với ba GHCR
digest, hostname/origin, danh sách asset `{path, bytes, sha256, delivery}`, digest inventory,
SHA Worker staged, account/project/branch/deployment Cloudflare và `auth_sign_in=disabled`.
`secrets=[]`. Dùng canonical JSON UTF-8, SHA-256 và object path
`releases/<config-sha256>/deployment-config.json`; upload với `--if-generation-match=0`.
Object generation là metadata ngoài nội dung, được lưu trong receipt cùng URI/SHA. Không đọc
`latest` hoặc chỉ tin filename. [Generation/versioning](https://docs.cloud.google.com/storage/docs/object-versioning).

`fetch-config.py` đọc token ngắn hạn từ stdin, tải đúng numeric generation, kiểm checksum trước
khi render `test.toml`. `--pull` dùng authfile GHCR rỗng và inspect digest/platform/revision;
ba package hiện cho anonymous pull, nên không tạo GHCR secret hay tái dùng publisher token.

```sh
# Trên máy cấp quyền: account vận hành chỉ được impersonate reader SA này.
# Copy script từ source đã review sang máy nhận trước; receipt cung cấp OBJECT, GENERATION, SHA256.
gcloud auth print-access-token \
  --impersonate-service-account=vink-test-config-reader@vink-d0846.iam.gserviceaccount.com \
  | ssh READER_HOST 'python3 /absolute/fetch-config.py \
      --object releases/CONFIG_SHA256/deployment-config.json \
      --generation NUMERIC_GENERATION --sha256 CONFIG_SHA256 \
      --output /absolute/new-config-dir --pull'
# Trên máy nhận, từ source checkout/export đúng SHA:
cargo xtask infra check /absolute/new-config-dir/test.toml
```

Đây là bootstrap bằng token impersonation ngắn hạn qua SSH; **không** phải danh tính tự động
bền vững của VPS. Người vận hành mint lại token cho lần tải sau; không chép user credential hoặc
key lâu dài sang máy nhận. Chỉ đọc trên workstation writer không đủ bằng chứng reader: phải tải
trên host khác bằng SA, kiểm quyền không create/delete/setIamPolicy, rồi pull/inspect.
Nếu cần CI tự động, thiết kế WIF giới hạn repo/ref theo
[hướng dẫn Google](https://docs.cloud.google.com/iam/docs/workload-identity-federation-with-deployment-pipelines)
và xác minh pipeline trước; lượt này không tạo WIF hay sửa workflow publish.

Không cần Secret Manager cho consumer này. Khi có consumer secret được giao sau này, chỉ cấp
access đúng secret, pin [numeric version](https://docs.cloud.google.com/secret-manager/docs/access-secret-version),
không đưa payload vào manifest, browser bundle hoặc log.
