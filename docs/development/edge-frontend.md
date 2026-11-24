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

## Ranh giới chuyển đổi

- `build web`, `dev web`, các kịch bản hồi quy SSR cũ và controller `release.py` ghép image với asset vẫn phục vụ hệ thống đã phát hành. Build standalone dùng chung router và components, chỉ khác điểm khởi động. Ngừng đường phát hành ghép ngay khi chuyển production sang artifact độc lập. Gỡ feature `legacy-ssr`, hydration entry và adapter in-process sau khi hoàn tất di trú các kịch bản còn dùng SSR và hết cửa sổ rollback cũ.
- Merge không xác nhận signoff hay cutover. Còn cần đo quota/thời gian thực trên Cloudflare, kiểm cửa sổ tương thích trên release thật và xác minh topology sau khi được phép triển khai.
