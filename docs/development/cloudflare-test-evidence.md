# Bằng chứng Cloudflare và cấu hình test trên Google Cloud

Ngày 15/11/2026 (UTC+07). Lượt triển khai test được người dùng giao làm thật; người dùng xác nhận
riêng việc thêm `origin-test`, chuyển `test` sang Pages và gắn billing hiện có cho project Vink.
Không mở đăng nhập, provider AI hoặc production Vink. Đây là lựa chọn kỹ thuật cho test,
không thay trạng thái các ADR hay quyết định sản phẩm.

## Source, artifact và provider

- Image source: `a677b0d4b5c53104ae03961a2d24c73d1d9f2ec5`, build/publish
  [#43683453768](https://github.com/loveoverflowcom/vink/actions/runs/43683453768).
  Ba image digest/platform và PostgreSQL nằm trong
  [hồ sơ VPS](test-container-evidence.md). Container export không start được tạo từ đúng Web
  digest; 73 file export khớp byte với site của Web đang chạy. Không rebuild Rust/WASM.
- Nhánh thực hiện `codex/cloudflare-test-distribution`; base ban đầu `a677b0d`, sau khi #198
  merge fast-forward tới `10805a74380c118d5553a861e9fe6c7df44f1186`, giữ nguyên các edit của việc này.
  Infra edge là source mới, không nằm trong image SHA; Pages metadata ghi `commit_dirty=true`.
- Account Cloudflare `8339afe44b4e8cd2b633a38a77f1db41`; account-scoped token verify active.
  Project `vink-test-frontend`, ID `9ca2139f-9622-4a06-9eed-f5adcae7a858`, Direct Upload.
  Zone `vink.forum` không nằm trên Cloudflare. Wrangler 4.151.2, compatibility `2026-10-26`.
- Pages deployment **`a3571c43-58bb-40ac-a56a-0f0665a342e1`**, branch `main`, provider environment
  `production` của project **test**; API readback stage success và `uses_functions=true`.
  [URL deployment](https://a3571c43.vink-test-frontend.pages.dev/),
  [URL Pages](https://vink-test-frontend.pages.dev/).
- Digest inventory 73 asset:
  `ca74ce22778ef115c16e6dafb9b5daa171acd78fbd8a03299f2327630620f346`.
  SHA-256 Worker **staged input**:
  `b21e0ea010c9e5da5f7eda69ea20ab0b53db20b8922ea134a7195ce1630c31ba`.
  Wrangler compile lại Worker khi upload; không gọi hash input là hash remote compiled bundle.

Chọn Pages Advanced Mode: 72 asset (JS, CSS, font, brand, expression runtime) upload thật;
hydration WASM 104636914 byte vẫn stream từ VPS do vượt 25 MiB/file. SHA-256 WASM decoded:
`fc6e4dc5dbe4e2692c256419aac5bdd997228ec7199d491d69ee860a1c63a3ac`.
R2 chưa bật; full offload WASM **NOT_RUN**. Cách này giữ nguyên SSR/Axum và cùng origin cho API,
không chép mô hình CSR/fallback HTML của Cantos. So với proxy CDN thuần, có upload asset thật;
so với chuyển SSR lên edge, không chuyển runtime/backend hoặc nghiệp vụ.

## Origin, DNS và TLS

`origin-test.vink.forum A 42.96.18.245` TTL300 được thêm ở Spaceship. Caddy hiện hữu import
vhost origin trong cùng file Vink, upstream `vink-test-web:3000`, `header_up Host test.vink.forum`.
TLS origin Let's Encrypt hợp lệ, hết hạn 13/02/2027; readiness database ok/schema current/role
restricted. App public origin vẫn `https://test.vink.forum`, auth disabled. DB Vink, volume,
network và credentials không đổi. Cantos endpoint `/api/v1/theatre/identity/session` trả 401
trước và sau reload Caddy.

Record `test` đổi từ A `42.96.18.245` sang CNAME `vink-test-frontend.pages.dev`, TTL300.
Hai NS vẫn `launch1.spaceship.net`, `launch2.spaceship.net`; không sửa apex/mail/Cantos.
UI đọc toàn bộ zone chỉ có hai record này; cả hai NS đọc lại CNAME. Pages association
`20cfcbc8-d750-4f23-b067-ff6b0ae40b04` đã readback `active` cho verification và validation.
Kết quả URL cuối được ghi ở bảng dưới, tách khỏi URL deployment và `--resolve`.

Vhost trực tiếp `test.vink.forum` còn nguyên trên Caddy. `curl --resolve
test.vink.forum:443:42.96.18.245 https://test.vink.forum/` trả 200/TLS verify0, chứng minh đường
trở lại origin. Plan rollback đã sinh từ snapshot after: chỉ đổi CNAME `test` về A cũ, giữ
origin-test. **Chưa thực sự đảo DNS hoặc thao tác Pages rollback.** Runbook ở
[infra/cloudflare](../../infra/cloudflare/README.md#domain-và-rollback).

## Google Cloud và máy đọc thứ hai

Project đã xác minh `vink-d0846`, number `720188176108`. Bucket `vink-d0846-test-config`,
`ASIA-SOUTHEAST1`, versioning enabled, uniform bucket IAM, public access prevention enforced.
Bucket IAM cuối: writer `manhpdbroad@gmail.com` có legacyBucketOwner + objectAdmin;
reader `vink-test-config-reader@vink-d0846.iam.gserviceaccount.com` chỉ objectViewer trên bucket.
Không còn grant mặc định cho projectViewer. Grant project admin tạm để sửa bucket policy đã
gỡ và đọc lại; reader không có project role hoặc user-managed key.

| Config pin | Giá trị |
| --- | --- |
| Schema | `vink-test-release/v1` |
| URI | `gs://vink-d0846-test-config/releases/68d42b899bb7a33a8dae43f28d5c4d97e4a751a9bd5dffb11388f3a81796a071/deployment-config.json` |
| Generation | `1794726270024969` |
| SHA-256 | `68d42b899bb7a33a8dae43f28d5c4d97e4a751a9bd5dffb11388f3a81796a071` |

Upload dùng generation-match0, writer tải lại đúng generation và so checksum. Sau đó **VPS Debian
42.96.18.245 là máy thứ hai**, khác workstation writer: script nhận token ngắn hạn của reader SA
qua SSH stdin, trực tiếp gọi GCS, kiểm SHA, render inventory trong thư mục mới
`/var/tmp/vink-config-reader-20261115/config`. Không gửi Google user credential hoặc SA key lên VPS.
Đây là quyền reader độc lập, bootstrap token vẫn do workstation được phép impersonate cấp;
chưa có workload identity tự động cho VPS/CI.

Reader permission probe trả đúng `storage.objects.get`, `storage.objects.list`, không trả
create/delete/setIamPolicy. `fetch-config.py --pull` dùng authfile rỗng và đã pull/inspect **cả ba**
GHCR digest, linux/amd64 và label source SHA khớp. Image cache trên VPS có sẵn; đây không phải thử
download mọi layer trên máy không cache. Source export mới từ đúng `a677b0d` ở
`/var/tmp/vink-config-reader-20261115/source` chạy `cargo xtask infra check` PASS trên inventory
tải từ GCS. Không restart service hoặc cho reader truy cập password/DB.

Ba GHCR package cho anonymous pull; không lưu token GHCR, Cloudflare hay DB trong Google Cloud
cho consumer này. Manifest `secrets=[]`; Secret Manager và WIF không cần cho luồng thủ công hiện
tại, **NOT_RUN** cho tự động hóa. Cách lặp lại nằm trong
[runbook reader](../../infra/cloudflare/README.md#cấu-hình-google-cloud-và-reader).

## Kiểm chứng

| Boundary | Kết quả và phạm vi |
| --- | --- |
| Local packaging | PASS image/site 73 file khớp; asset inventory hash; inventory `cargo xtask infra check` |
| Node fake fetch | PASS 5 tests: known/missing asset, private response/Origin/body, WASM range, loop/failure, double-slash không đổi upstream |
| Workerd local | PASS readiness qua origin sau sửa tổ hợp cache; không phải bằng chứng browser |
| Cloudflare readback | PASS đúng account/project/deployment/environment/source/stage; custom hostname active |
| Preview HTTP | PASS 72/72 asset hash decoded; WASM decoded khớp image; SSR `/`, `/ban-viet`, `/toi` 200; reader thiếu 404; health 200; API works 200/304; session no-store; missing asset 404 text |
| Preview browser | PASS Khám phá, toggle điều hướng đổi expanded → collapsed; navigation Bàn viết và overlay Tôi; anonymous đúng. IAB 1280×720, ảnh đã xem |
| URL cuối `test.vink.forum` | PASS resolver mặc định + hai NS + 1.1.1.1/8.8.8.8 trả CNAME; HTTPS Google Trust Services WE1 hợp lệ; 72/72 asset hash và delivery Cloudflare; WASM hash khớp; SSR/deep URL/health/API/304/missing-asset như preview; noindex và cache đúng |
| Browser domain cuối | PASS IAB 1280×720: toggle expanded → collapsed, đi Bàn viết, reload trực tiếp `/ban-viet`, anonymous đúng, quay Khám phá; console error rỗng, ảnh đã xem |
| GCS reader trên VPS | PASS pinned generation/checksum; quyền read-only; 3 image pull/inspect; inventory trên source export mới |
| Auth/Bàn viết có phiên | NOT_RUN; sign-in disabled, không fake tài khoản |
| Full WASM upload, WIF, credential Secret Manager, TLS renewal, DNS/Pages rollback thật | NOT_RUN; riêng từng boundary, không suy từ deploy thành công |

Các preview trước cutover chỉ là lịch sử: `0293923b` upload thành công nhưng proxy lỗi do
`cache: no-store` + `cf.cacheTtl:0`; sửa source và kiểm Workerd trước deployment `5e2e5a3a`.
`5e2e5a3a` qua HTTP/browser trước cutover nhưng dùng `test` làm origin, nên sau cutover không
còn là URL vận hành. Deployment hiện hành dùng origin riêng là `a3571c43`.

Bằng chứng cục bộ (ignored): `target/cloudflare/20261115/` chứa manifest, provider/GCS receipts,
DNS plans, log reader và ảnh. Không có token hoặc env secret trong bộ này. Các quy tắc/cache,
lệnh tái lập và rollback thuộc [runbook](../../infra/cloudflare/README.md).
