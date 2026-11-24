# Ranh giới Web production, Preview và Test

[#479](https://github.com/loveoverflowcom/vink/issues/479) ghi quyết định ngày 24/11/2026:
màn thử nghiệm biểu cảm thuộc Preview. Nguồn trước thay đổi `develop@93034f7`.
[Proposal kiến trúc](../../decisions/proposals/VDR-WEB-PREVIEW-BOUNDARY.r1.toml) còn `PENDING`.

| Phân loại | Surface và caller | Owner, quyết định |
| --- | --- | --- |
| PRODUCT | `/`, `/the/:slug`, `/tac-pham/:id` | Khám phá/Reader gọi public reads có phân trang, SSR và cache. Giữ. |
| PRODUCT | `/ban-viet`, `/ban-viet/:id`, `/ban-viet/:id/kiem-duyet` | Desk tác giả, DraftSession/CodeMirror, trạng thái kiểm duyệt và redirect tương thích. Giữ Markdown Live Preview, chuyển định dạng và guard sản phẩm. |
| PRODUCT | `/kiem-duyet`, `/kiem-duyet/ca/:id`, `/kiem-duyet/phan-doi/:id` | App moderation và quyền theo phiên. Giữ. |
| PRODUCT | `/toi`, `/toi/chinh-ho-so`, `/toi/dia-chi` | Account/Profile/Handle và compatibility thuộc owner Profile. Giữ cho #488. |
| PRODUCT | `/:author` và hai danh sách `nguoi-theo-doi`, `dang-theo-doi` | Public author/follow resolver kiểm tên, quyền danh sách, alias và tombstone. Giữ. |
| PRODUCT | `vink-app → vink-web/ssr`, `vink-web/hydrate`, shared UI/patterns/Markdown, npm editor, profile navigation, cache và `vink.css` | App và UX thật. Giữ edge và font, empty/brand, snippet có caller trong Web build, OCI và Pages. |
| PREVIEW_ONLY | EMO-01A, CSS pilot, npm dotLottie, JS/WASM, poster và NOTICE | Chuyển sang `vink-ui-preview`. `/expressions.html` có HTML tĩnh và hydrate, link từ gallery. URL Web cũ qua NotFound. |
| PREVIEW_ONLY | `state.playground`, catalog/hash, inspector và mẫu màn hình | Preview Leptos/CMP diagnostics đã sở hữu. Giữ CSS/Tailwind source riêng. |
| TEST_ONLY | Media/Notice hydration, sample WebP, captures | Fixture owner Preview, output `target/ui-preview/leptos`. Giữ. |
| TEST_ONLY | `perf-probe`, CDP fault fixtures và `LocalFixture` | Probe opt-in của browser lane. LocalFixture ràng `VINK_PROFILE=local`, loopback và fixture principal ở App server. Giữ gate hiện có. |
| TEST_ONLY | EMO manifest/Quint/Kani, Rust replay và generated Unicode CMP | `packages/expressions` vẫn phục vụ Preview và lanes kiểm. Web/App không có normal edge. |
| UNKNOWN | Các candidate tương lai chưa xác nhận authority | Ghi caller, nguồn và quyết định còn thiếu trước khi dọn. |

Nguồn: [router](../../apps/vink-web/src/lib.rs), [App Cargo](../../services/vink-app/Cargo.toml),
[Web build](../../xtask/src/build.rs), [Preview build](../../xtask/src/ui_preview.rs),
[Web CSS](../../packages/ui-system/leptos/tailwind/web.css), [Preview CSS](../../packages/ui-system/leptos/tailwind/preview.css)
và [server identity](../../services/vink-app/src/identity/mod.rs).

## Đặt source và kiểm

- Chốt nơi chạy trước khi viết. Web và Preview/Test dùng được thư viện chung. Production nhận edge
  có caller nghiệp vụ, contract dữ liệu, lỗi, trợ năng và quyền rõ ràng.
- Mẫu motion dùng Preview với poster, license và hành động mô phỏng. Markdown Live Preview
  trong editor là hành vi làm việc của tác giả. Màn kiểm duyệt dùng auth thật.
- Để promote prototype, ghi caller thật, authority, fallback, route, dependencies, assets, kiểm
  tra tại runtime và quyết định sản phẩm còn mở. Giữ fixture để tiếp tục thử độc lập.
- CHOICE-01 B dùng đường dựng fixture tĩnh/hydrate hiện có để giữ no-JS và vòng đời. Gallery
  liên kết tới fixture; catalog chung giữa renderer giữ format hiện tại.
- CHOICE-02 kết hợp [guard](../../xtask/src/web_boundary.rs) của repo/build với
  [manifest Pages](../../infra/cloudflare/production/artifacts.py). Duyệt route literal cùng
  inventory, đọc normal graph cho SSR, hydrate và App, dọn cây snippet/pilot cũ rồi kiểm output.
  Pages chặn asset Preview trong package mới và đối chiếu archive đã lưu khi rollback.

## Bàn giao kiểm chứng

Build Web release, Preview, Chrome SSR/hydrate, Quint/Kani, Docker OCI và byte raw/gzip/brotli
trước–sau `NOT_RUN` trong lượt draft. Cần Rust/Cargo với target wasm và wasm-bindgen đúng bản repo,
Node/npm pin, Chrome, PostgreSQL scratch cho Web và Docker cho OCI.
Dùng `cargo xtask build web --release`, `cargo xtask build preview`, `cargo xtask verify preview`,
`cargo xtask verify quick`, `cargo xtask verify expressions`, `cargo xtask verify web --scenario main`.
Ảnh và số đo #57 giữ ở mốc lịch sử.
