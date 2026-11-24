# Frontend trên Cloudflare (#504): route, mức triển khai và bằng chứng

Owner thiết kế: [ADR edge frontend](../architecture/edge-frontend.md) (`DESIGN_PROPOSED`),
[VDR-EDGE-FRONTEND@r1](../../decisions/proposals/VDR-EDGE-FRONTEND.r1.toml) (`PENDING`).
Đọc tại `develop@276d142` ngày 24/11/2026.

## Mức triển khai

| Lát | Nội dung | Mức | Bằng chứng |
| --- | --- | --- | --- |
| 1 | ADR, route matrix, giới hạn, giao thức spike | tài liệu | review diff |
| 1b | Spike Home + Reader trên workerd | `NOT_IMPLEMENTED` | `NOT_RUN` |
| 2 | Renderer edge đủ route + `HttpPublicReads` | `NOT_IMPLEMENTED` | `NOT_RUN` |
| 3 | Controller/packager, A+B, rollback | `NOT_IMPLEMENTED` | `NOT_RUN` |
| 4 | Backend API-only không Leptos, dọn SSR/static | `NOT_IMPLEMENTED` | `NOT_RUN` |
| — | Cutover production | ngoài PR | cần quyền phát hành riêng |

## Route và owner

“Hiện tại” đọc từ [`lib.rs`](../../apps/vink-web/src/lib.rs),
[`http/mod.rs`](../../services/vink-app/src/http/mod.rs) và
[`worker.mjs`](../../infra/cloudflare/production/worker.mjs). “Đích” là đề xuất.

| Route | SSR | Đọc khi render | Hiện tại | Đích | Status cần giữ |
| --- | --- | --- | --- | --- | --- |
| `/` (`?q`, `?tag`, cursor) | Async | `works`, `catalog`, `authors` | VPS SSR | edge SSR + API | 200; 400 input sai; 503 |
| `/the/:slug` | Async | `catalog`, `works` | VPS SSR | edge SSR + API | 200; 404; 503 |
| `/tac-pham/:id` | Async | `reader`, `discussion`, `endorsement_summaries` ×2 | VPS SSR | edge SSR + API | 200 (kể cả trang thay thế); 404; 503 |
| `/:author` (và `/@name`) | Async | `author` | VPS SSR | edge SSR + API | 200; 301 alias; 404; 410; 503 |
| `/:author/nguoi-theo-doi`, `/:author/dang-theo-doi` | Async | `author_relationships`, `author` | VPS SSR | edge SSR + API | 200; 301 alias; 403; 404; 410; 503 |
| `/ban-viet`, `/ban-viet/:id` | thường | không (`LocalResource`) | VPS SSR | edge SSR, không gọi origin | 200 |
| `/ban-viet/:id/kiem-duyet` | thường | không | VPS SSR | edge SSR | 308 |
| `/kiem-duyet`, `/kiem-duyet/ca/:id`, `/kiem-duyet/phan-doi/:id` | thường | không | VPS SSR | edge SSR | 200 |
| `/toi`, `/toi/chinh-ho-so` | thường | không | VPS SSR | edge SSR | 200 |
| `/toi/dia-chi` | thường | không | VPS SSR | edge SSR | 308 + `no-store` |
| path không khớp | `NotFound` | không | VPS fallback | edge SSR | 404 |
| `/pkg/*`, `/_v/<sha>/pkg/*` | — | — | Pages (inventory) | giữ | 200/304/404 |
| `/api/v1/*`, `/health/*` | — | — | Worker → origin | giữ | theo hợp đồng v1 |
| `/robots.txt` | — | — | origin 404 → Content Signals của zone | edge quyết định rõ (`EF-GAP-05`) | 200 |
| `www`, `http:` | — | — | Worker redirect | giữ | 301/308 |
| host preview/provider | — | — | 404 HTML/API | giữ | 404 |

## Khoảng trống cần đóng

| ID | Khoảng trống | Lát |
| --- | --- | --- |
| EF-GAP-01 | Gói Workers của account (`UNKNOWN`); Free giới hạn 10 ms CPU. Chi phí Paid cần chủ dự án duyệt | 1b |
| EF-GAP-02 | `vink-web/ssr` kéo `leptos_axum` default (`tokio/fs`); cần tách để build wasm32 | 1b–2 |
| EF-GAP-03 | `SignInOpen`, `FollowingFeedEnabled` hiện lấy từ `AppState`; chọn nguồn ở edge | 2 |
| EF-GAP-04 | Kiểm tương thích frontend `<sha>` với API đang chạy trước khi promote | 3 |
| EF-GAP-05 | `robots.txt` và tài nguyên gốc khác do edge sở hữu | 2 |
| EF-GAP-06 | Reader có thể 3 lượt origin tuần tự; cần đo và giảm trong ranh giới API | 1b–2 |
| EF-GAP-07 | Dev native SSR sau cutover: giữ, đổi sang workerd local, hay bỏ | 4 |

## Giao thức spike

Phạm vi: `/` và `/tac-pham/:id` với backend scratch (App + PostgreSQL, catalog có ít nhất một tác
phẩm công khai thơ và văn xuôi tiếng Việt). Không dùng production DB.

1. Build `vink-edge` target `wasm32-unknown-unknown` với Wrangler/workerd đã pin trong
   `infra/cloudflare/package-lock.json`. Ghi SHA, lệnh, kích thước WASM chưa nén/nén, thời gian
   instantiate.
2. Render Home và Reader qua workerd tới App scratch: so HTML (body text, title, canonical,
   status) với `serve` native cùng dữ liệu; tắt JS phải đọc được thân bài.
3. Hydration: trang edge tải `/_v/<sha>/pkg` cùng build, `data-hydrated` xuất hiện, không lỗi
   hydration trong console.
4. Lỗi: App dừng → 503; tác phẩm rút về riêng tư → 404 ngay lần đọc sau; work id sai → 404.
5. Riêng tư: hai phiên cookie khác nhau nhận HTML Reader byte-giống; origin log không thấy
   cookie từ edge.
6. Đo theo phương pháp [#473](evidence/web-reload-473/README.md): TTFB/FCP/hydrated cold/warm,
   CPU 1×/4×, CPU time mỗi request trong workerd, số subrequest và chuỗi tuần tự.

Dừng spike và báo blocker (không lùi mục tiêu) nếu: graph `ssr` không build được cho wasm32 mà
không sửa lớn ngoài phạm vi; startup vượt 1 s; CPU render Reader vượt ngân sách gói đã chọn; hoặc
Reader chậm hơn baseline mà không có cách giảm trong ranh giới API.

## Kiểm chứng của PR tài liệu này

`NOT_RUN`: mọi build, `cargo xtask decision validate`, workerd, browser và production. Chỉ kiểm
diff, link tương đối và định dạng TOML bằng mắt so với proposal hiện có.
