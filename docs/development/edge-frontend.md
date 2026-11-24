# Frontend standalone cho Vink

[ADR](../architecture/edge-frontend.md) · [Issue #504](https://github.com/loveoverflowcom/vink/issues/504) · [Proposal r2 đang chờ signoff](../../decisions/proposals/VDR-EDGE-FRONTEND.r2.toml)

## Build và chạy local

Từ gốc checkout sạch, với toolchain của Web trong [scoped workflows](scoped-workflows.md), chạy:

```sh
VINK_SPA_SIGN_IN=enabled VINK_SPA_FOLLOWING_FEED=enabled ./tools/local-budget run -- cargo xtask build spa --release
node apps/vink-web/dev-server.mjs target/spa 8080 3000
```

Mở `http://127.0.0.1:8080`, kể cả khi backend dừng. Frontend phát hành gồm `target/spa/index.html` và `_v/<git-sha>/pkg/`. Source archive thiếu thư mục Git cần truyền `VINK_SPA_REVISION` đủ 40 chữ số hex thường. Hai cờ `VINK_SPA_*` bật điều khiển tương ứng khi deployment đã bật backend, mặc định tắt. Backend quyết định quyền cho mỗi request.

Trong terminal khác, với App/Identity scratch đã chuẩn bị và credential runtime phù hợp:

```sh
# Toolchain Rust native theo pin của checkout; không cài target WASM cho backend.
export RUSTUP_TOOLCHAIN="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml)"
./tools/local-budget run -- cargo xtask build api
VINK_BIND=127.0.0.1:3000 VINK_PUBLIC_ORIGIN=http://127.0.0.1:8080 target/debug/vink-app serve-api
```

Node phục vụ asset ở loopback và chuyển API/health tới cổng thứ hai. Cần đặt public origin của App trùng URL frontend để các thao tác đăng nhập/lưu vượt kiểm tra Origin. Cấu hình socket/UID, mail outbox và DB của Identity theo [runbook auth](auth-operations.md). Lane `cargo xtask verify web --scenario standalone` tự dựng scratch App/Identity và chạy Chrome. [Cấu hình credential test](scoped-workflows.md)

Container target `api` của [Containerfile](../../infra/Containerfile) build `vink-app` với `--no-default-features`; runtime dùng `serve-api`. Target `worker` cũng bỏ dependency Leptos. `cargo xtask infra build-images docker linux/amd64 --api-only` đóng gói bộ `api/worker/identity`. Workflow publish-images có input `api_only=true` để smoke/publish ba image backend; giữ `website=false` khi chọn bộ này. Build frontend và API thực thi độc lập.

## Đóng gói, phát hành và rollback FE

Giữ hai thư mục artifact bất biến từ hai checkout sạch A và B. Tên phiên bản là Git SHA. Giữ cả index và toàn bộ cây `_v` của mỗi bản. Dùng lại đúng artifact đã kiểm khi rollback.

```sh
python3 infra/cloudflare/production/artifacts.py --spa "$BUILD_B" --retain "$BUILD_A" --out "$PAGES_B"
python3 infra/cloudflare/production/artifacts.py --spa "$BUILD_A" --retain "$BUILD_B" --out "$PAGES_ROLLBACK"
```

Mỗi lệnh tạo một thư mục mới, xác thực inventory, giới hạn file và trùng namespace. Khi rollback, document chọn A còn tab đã mở B vẫn tải được các import của B. Có thể lặp `--retain` cho các bản trong cửa sổ tương thích. Lần chuyển từ SSR phải giữ các asset versioned của document SSR đang mở trong cửa sổ đó. Truyền `--retain "$ARCHIVES/$OLD_SHA"` cho thư mục cũ chứa `pkg` và `manifest.json` của controller. Packager đối chiếu SHA và inventory cũ trước khi nhận.

Dùng `npm ci --prefix infra/cloudflare` rồi chạy `node infra/cloudflare/production/local-edge.mjs "$PAGES_B" http://127.0.0.1:3000 8080` để diễn tập local bằng workerd. Kiểm Reader hiện văn bản của `GET /api/v1/works/{id}/reader`, guest không đọc được nháp, cookie/Origin, `/health/ready`, asset A/B và hành vi API down. Xác nhận backend đang phục vụ DTO v1 mà cả hai artifact sử dụng trước mỗi promote/rollback. `serve-api` không phục vụ document hoặc asset.

Sau signoff r2 và quyền cutover, dùng Wrangler đã pin trong `infra/cloudflare/node_modules/.bin/` để upload thư mục đã stage vào project `vink-production`. Stage alias `r-<26 ký tự đầu của SHA>` để kiểm byte theo revision, đối chiếu predecessor thực tế, sau đó dùng `wrangler pages deploy "$PAGES_B" --project-name vink-production --branch main`. Khi cần quay lại, upload `"$PAGES_ROLLBACK"` qua cùng quy trình. Các lần đổi UI sau cutover không cần restart VPS. [Runbook production hiện tại](../../infra/production/cloudflare-edge.md)

### Kiểm chứng trên trình duyệt

Lane `cargo xtask verify web --scenario standalone --release` dựng hai database dùng một lần, chạy App API-only và Identity thật, rồi mở Chrome trên frontend. Sau `npm ci --prefix infra/cloudflare`, thêm `VINK_STANDALONE_WORKERD=1` vào cùng lệnh để chạy qua Worker và static binding của workerd. Cả hai cách dùng cùng hành trình. Chrome kiểm tra Reader, tìm kiếm/lịch sử, thẻ tiếng Việt, hồ sơ công khai và hồ sơ không tồn tại, đăng ký, lưu/mở nháp, giữ chữ chưa lưu, gửi bản duyệt, riêng tư của nhận xét chờ, quyền kiểm duyệt, đăng xuất và đổi tài khoản.

### Thời điểm đọc được tác phẩm

Ngày 24/11/2026, Chrome trên macOS đọc cùng tác phẩm mẫu qua PostgreSQL scratch. So sánh hai build release từ mã runtime `3b8b07a5`. SSR dùng Axum, CSR dùng static host Node cùng origin với API. Mỗi ô là một lần chạy cục bộ, đơn vị ms. “Văn bản” là callback frame đầu tiên sau khi DOM chứa dòng đầu của tác phẩm. FCP có thể chỉ là shell. LCP là quan sát mới nhất khi lấy mẫu. “Sẵn sàng” là hydration hoàn tất ở SSR và shell đã mount ở CSR; mốc văn bản đo trải nghiệm đọc chung. Thứ tự các cặp số là **SSR / CSR**.

| CPU | Lần mở | TTFB | FCP | LCP | Sẵn sàng | Văn bản | GET Reader của CSR, bắt đầu–kết thúc |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1× | Cache trống | 13 / 4 | 68 / 52 | 68 / 116 | 114 / 85 | 60 / 109 | 86–92 |
| 1× | Reload | 6 / 1 | 48 / 36 | 48 / 52 | 34 / 18 | 28 / 32 | 19–21 |
| 1× | Tab mới, cache ấm | 3 / 1 | 40 / 24 | 40 / 84 | 84 / 55 | 24 / 75 | 56–58 |
| 4× chậm | Cache trống | 4 / 1 | 64 / 32 | 64 / 32 | 143 / 88 | 53 / 114 | 89–91 |
| 4× chậm | Reload | 4 / 1 | 52 / 24 | 52 / 24 | 152 / 91 | 45 / 121 | 92–93 |
| 4× chậm | Tab mới, cache ấm | 8 / 2 | 128 / 68 | 128 / 296 | 403 / 187 | 109 / 279 | 192–194 |

Tổng `transferSize` khi cache trống là 8.934.039 / 6.625.792 byte. Khi reload hoặc mở tab mới là 24.539 / 4.970 byte. SSR trả văn bản trong HTML. CSR bắt đầu GET Reader sau khi WASM mount ở 6/6 mẫu. Khi CPU bị làm chậm, tab mới có khoảng chờ lớn nhất. Trace CSR ghi năm lượt API gồm session, Reader, discussion và hai lượt endorsement; cần đo phân phối nhiều lần cùng chi phí thực tại edge trước cutover. Không dùng số loopback để dự toán quota hay tốc độ production.

Để chạy lại, dùng `browser-smoke.mjs --phase reader-startup` với `--base` trỏ từng host, `--chrome` trỏ Chrome và `--state` trỏ một tệp trong thư mục scratch. Mỗi dòng `READER_STARTUP` trả về timing, byte và thời điểm từng API.

## Ranh giới chuyển đổi

- `build web`, `dev web`, các kịch bản hồi quy SSR cũ và controller `release.py` ghép image với asset vẫn phục vụ hệ thống đã phát hành. Build standalone dùng chung router và components, chỉ khác điểm khởi động. Ngừng đường phát hành ghép ngay khi chuyển production sang artifact độc lập. Gỡ feature `legacy-ssr`, hydration entry và adapter in-process sau khi hoàn tất di trú các kịch bản còn dùng SSR và hết cửa sổ rollback cũ.
- Merge không xác nhận signoff hay cutover. Còn cần đo quota/thời gian thực trên Cloudflare, kiểm cửa sổ tương thích trên release thật và xác minh topology sau khi được phép triển khai.
