# App Preview (`apps/vink-preview`)

App Preview là nơi thực thi duy nhất của specimen, mock, demo và fixture hydrate cho UI System.
Nó dùng thư viện chung qua public API, không gọi service, không cần database, phiên đăng nhập
hay credential, và không đi cùng bản build Product. Quyết định nằm ở
[#552](https://github.com/loveoverflowcom/vink/issues/552) (Epic
[#554](https://github.com/loveoverflowcom/vink/issues/554), PR 1/4). Inventory và guard ở
[ranh giới Web/Preview](../../docs/development/web-preview-boundary.md).

| Host | Nguồn | Build và kiểm | Output |
| --- | --- | --- | --- |
| Leptos | [`leptos/`](leptos): crate `vink-ui-preview` (workspace member), `index.html`, `style/preview.css`, `examples/*_hydration.rs`, `e2e/` (Chrome smoke, capture), `expressions/` (dotLottie, npm lock riêng) | `cargo xtask build preview`, `cargo xtask dev preview`, `cargo xtask verify preview` (tương đương `ui-preview leptos build\|run\|verify`) | `target/ui-preview/leptos` |
| CMP Desktop | [`cmp/`](cmp): Gradle project `:preview` (Compose Desktop window, font TTF, capture test) | `cargo xtask ui-preview cmp build\|run\|verify` từ build Gradle `packages/ui-system/cmp`, nơi `settings.gradle.kts` map `projectDir` | `apps/vink-preview/cmp/build/{captures,test-results}` |

Lệnh và yêu cầu công cụ: [docs/development](../../docs/development/README.md).

## Quy tắc

- Chỉ phụ thuộc thư viện: `vink-ui`, `vink-patterns`, `vink-expressions` (Leptos); `:ui`,
  `:patterns`, `:diagnostics` (CMP). Không có chiều ngược lại: Web/App, thư viện UI và patterns
  không phụ thuộc app này.
- Stylesheet `vink-preview.css` biên dịch từ entry
  [`packages/ui-system/leptos/tailwind/preview.css`](../../packages/ui-system/leptos/tailwind/preview.css),
  đặt cạnh compiler Tailwind đã pin để `tailwindcss/*` lấy từ `node_modules` của package đó. Entry
  chỉ quét `leptos/src` và `leptos/examples` của app này cùng source thư viện; `web.css` không
  quét app này.
- Fixture có nhiều consumer nằm ở owner của contract hoặc thư viện, không ở đây:
  [ví dụ thông báo](../../services/vink-app/contracts/http/v1/fixtures/notifications-examples.json)
  (contract Gateway) và [probe `Select`](../../packages/ui-system/leptos/e2e/select-geometry.mjs)
  (thư viện UI). Test và runner Product không import file trong app này.
- Màn quản trị, authority hoặc dữ liệu thật không đặt ở đây. Promotion sang Product theo điều
  kiện của [ranh giới](../../docs/development/web-preview-boundary.md#đặt-source-và-kiểm).
- Guard: `cargo xtask` từ chối crate nằm dưới `apps/vink-preview` trong normal graph của Web/App
  và output `vink-preview*`/`vink_ui_preview*` trong package Web; manifest Pages chặn cùng nhóm
  asset.
