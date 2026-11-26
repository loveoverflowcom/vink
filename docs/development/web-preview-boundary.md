# Ranh giới Web production, Preview và Test

[#479](https://github.com/loveoverflowcom/vink/issues/479) ghi quyết định ngày 24/11/2026:
màn thử nghiệm biểu cảm thuộc Preview. Nguồn trước thay đổi `develop@93034f7`.
Ngày 26/11/2026 chủ sản phẩm yêu cầu tách mock/preview thành một app riêng trong `apps/` và bỏ
màn quản trị kiểm duyệt khỏi Web (`USER_CONFIRMED`, Epic
[#554](https://github.com/loveoverflowcom/vink/issues/554)). PR 1/4
([#552](https://github.com/loveoverflowcom/vink/issues/552)) chuyển host thực thi sang
[app Preview](../../apps/vink-preview/README.md); [#555](https://github.com/loveoverflowcom/vink/issues/555)
gỡ route staff. [Proposal kiến trúc](../../decisions/proposals/VDR-WEB-PREVIEW-BOUNDARY.r1.toml)
còn `PENDING`; việc di chuyển không đổi trạng thái đó.

| Phân loại | Surface và caller | Owner, quyết định |
| --- | --- | --- |
| PRODUCT | `/`, `/the/:slug`, `/tac-pham/:id` | Khám phá/Reader gọi public reads có phân trang, SSR và cache. Giữ. |
| PRODUCT | `/ban-viet`, `/ban-viet/:id`, `/ban-viet/:id/kiem-duyet` | Desk tác giả, DraftSession/CodeMirror, trạng thái kiểm duyệt và redirect tương thích. Giữ Markdown Live Preview, chuyển định dạng và guard sản phẩm. |
| RETIRED | `/kiem-duyet`, `/kiem-duyet/ca/:id`, `/kiem-duyet/phan-doi/:id` | [#555](https://github.com/loveoverflowcom/vink/issues/555) gỡ các màn, link, probe quyền trên trang Tôi và CSS chỉ dùng cho staff. URL cũ về 404. Nhân sự dùng [API được phân quyền](moderation-runtime.md#7-web-kiểm-duyệt). Studio Admin được hoãn. |
| PRODUCT | `/toi`, `/toi/chinh-ho-so`, `/toi/dia-chi` | Account/Profile/Handle và compatibility thuộc owner Profile. Giữ cho #488. |
| PRODUCT | `/:author` và hai danh sách `nguoi-theo-doi`, `dang-theo-doi` | Public author/follow resolver kiểm tên, quyền danh sách, alias và tombstone. Giữ. |
| PRODUCT | `/:author/da-luu` | [Đã lưu](saved-works.md#hồ-sơ-web-389) có caller Reader và chủ hồ sơ sau khi xác minh phiên. SSR dùng trang 404 chung. |
| PRODUCT | `vink-app → vink-web/ssr`, `vink-web/hydrate`, shared UI/patterns/Markdown, npm editor, profile navigation, cache và `vink.css` | App và UX thật. Giữ edge và font, empty/brand, snippet có caller trong Web build, OCI và Pages. |
| PREVIEW_ONLY | EMO-01A, CSS pilot, npm dotLottie, JS/WASM, poster và NOTICE | `vink-ui-preview` trong `apps/vink-preview/leptos`. `/expressions.html` có HTML tĩnh và hydrate, link từ gallery. URL Web cũ qua NotFound. |
| PREVIEW_ONLY | `state.playground`, catalog/hash, inspector và mẫu màn hình | App Preview (Leptos, CMP Desktop) và specimen `:diagnostics`. Giữ CSS/Tailwind source riêng. |
| TEST_ONLY | Media/Notice hydration, sample WebP, captures | Fixture owner Preview (`apps/vink-preview/leptos/examples`), output `target/ui-preview/leptos`. Giữ. |
| TEST_ONLY | `perf-probe`, CDP fault fixtures và `LocalFixture` | Probe opt-in của browser lane. LocalFixture ràng `VINK_PROFILE=local`, loopback và fixture principal ở App server. Giữ gate hiện có. |
| TEST_ONLY | EMO manifest/Quint/Kani, Rust replay và generated Unicode CMP | `packages/expressions` vẫn phục vụ Preview và lanes kiểm. Web/App không có normal edge. |
| UNKNOWN | Các candidate tương lai chưa xác nhận authority | Ghi caller, nguồn và quyết định còn thiếu trước khi dọn. |

Nguồn: [router](../../apps/vink-web/src/lib.rs), [App Cargo](../../services/vink-app/Cargo.toml),
[Web build](../../xtask/src/build.rs), [Preview build](../../xtask/src/ui_preview.rs),
[app Preview](../../apps/vink-preview/README.md),
[Web CSS](../../packages/ui-system/leptos/tailwind/web.css), [Preview CSS](../../packages/ui-system/leptos/tailwind/preview.css)
và [server identity](../../services/vink-app/src/identity/mod.rs).

## App Preview vật lý (#552)

Trước PR này host Leptos nằm lồng trong thư mục thư viện `packages/ui-system/leptos/preview` và
host CMP Desktop trong `packages/ui-system/cmp/preview`. Guard đã cô lập chúng khỏi normal graph
và package Web; vấn đề là owner và vị trí, chưa có số đo cho thấy rò rỉ. Inventory tại
`develop@261ce09` và sau di chuyển:

| Hạng mục | Vị trí, caller | Phân loại và cô lập |
| --- | --- | --- |
| Host Leptos `vink-ui-preview` | `apps/vink-preview/leptos` (workspace member); `cargo xtask build\|dev\|verify preview`, `ui-preview leptos …` | PREVIEW_ONLY. Guard normal graph từ chối theo tên crate, theo root `apps/vink-preview` và thành phần đường dẫn `preview/examples/fixtures` cho Web SSR, hydrate, CSR, API-only và App default. Manifest thư viện bị `verify quick` cấm phụ thuộc `vink-ui-preview`. |
| Host CMP Desktop `:preview` | `apps/vink-preview/cmp`, Gradle root `packages/ui-system/cmp` map `projectDir` (như `:patterns`) | PREVIEW_ONLY. `:ui`/`:patterns` bị cấm `project(":preview")`; output `apps/vink-preview/cmp/build`. |
| Fixture hydrate, sample WebP, dotLottie npm | `apps/vink-preview/leptos/{examples,expressions}`; WebP sinh từ `packages/ui-system/shared` lúc build | TEST_ONLY/PREVIEW_ONLY, chỉ ghi vào `target/ui-preview/leptos`. |
| Ví dụ thông báo (counts/inbox/decisions) | `services/vink-app/contracts/http/v1/fixtures/notifications-examples.json`; test `vink-web` `notifications::state`, `notifications-smoke.mjs`, specimen Preview | TEST_ONLY, đặt cạnh contract Gateway thay vì trong host Preview, để test Product không đọc file của app Preview. Provider chưa kiểm file này. |
| Probe `Select` | `packages/ui-system/leptos/e2e/select-geometry.mjs`; `browser-smoke.mjs`, `editor-desk-regressions.mjs`, `preview-smoke.mjs` | TEST_ONLY của thư viện UI; không nằm trong crate hay artifact. |
| Tailwind | `web.css` quét `apps/vink-web/src`, `packages/ui-system/leptos/src`, `packages/vink-patterns/leptos/src`; `preview.css` quét thêm `apps/vink-preview/leptos/{src,examples}` và import `style/preview.css` của app | Hai entry ở package compiler pin; `xtask` kiểm allowlist `@source` từng entry. Product không quét source Preview. |
| OCI | `infra/Containerfile` sao cả context (cần đủ workspace member) nhưng chỉ build `web`, `api`, `worker`, `identity`, `vink-notifications`; image runtime chỉ nhận binary và `target/site{,-release}` | Source Preview có trong stage build, không được build hay ship. |
| Pages | `infra/cloudflare/production/artifacts.py` | Chặn `expressions/`, `preview/`, `fixtures/`, `vink-ui-preview-*`, `vink_ui_preview*`, `vink-preview*`, `expression_hydration*` trong package mới. |
| Capture | Product: `apps/vink-web/e2e/visual-evidence.mjs` (identity gồm thư viện, không gồm app Preview), output `target/verify-web`. Preview: `focus-capture.mjs`/`InteractionCaptureTest` (identity thêm `apps/vink-preview/{leptos,cmp}`), output `target/ui-preview/leptos/captures` và `apps/vink-preview/cmp/build/captures` | Host và output tách riêng. Corpus review phân loại Product scratch, specimen Preview và ảnh staff lịch sử. [Catalog và provenance #556](evidence/literary-corpus-556/README.md). |
| Host chẩn đoán native và `:diagnostics` | `packages/ui-system/cmp/{diagnostics,androidDiagnostic,iosDiagnostic}`; opt-in `ui-preview android\|ios` | Qualification host của UI System, chưa chuyển trong PR 1/4 (cần Android SDK/Xcode để kiểm). `:diagnostics` là thư viện specimen dùng chung cho `:preview` và host native. Chuyển hay giữ là việc còn mở. |

## Đặt source và kiểm

- Chốt nơi chạy trước khi viết. Web và Preview/Test dùng được thư viện chung. Production nhận edge
  có caller nghiệp vụ, contract dữ liệu, lỗi, trợ năng và quyền rõ ràng. Specimen, mock và demo
  thực thi nằm trong `apps/vink-preview`; fixture có consumer Product đặt ở owner contract hoặc
  thư viện.
- Mẫu motion dùng Preview với poster, license và hành động mô phỏng. Markdown Live Preview
  trong editor là hành vi làm việc của tác giả. Chi tiết kiểm duyệt tác giả dùng phiên đã xác nhận.
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
