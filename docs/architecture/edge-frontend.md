# Frontend Web trên Cloudflare, VPS chỉ chạy backend/API (#504)

**Trạng thái kiến trúc:** `DESIGN_PROPOSED` (lát 1 của Epic
[#504](https://github.com/loveoverflowcom/vink/issues/504)). Proposal
[VDR-EDGE-FRONTEND@r1](../../decisions/proposals/VDR-EDGE-FRONTEND.r1.toml) ở `PENDING` đến khi
chủ sản phẩm signoff đúng revision/SHA-256 theo `VDR-DECISION-SIGNOFF@r1`.
**Mức triển khai:** `NOT_IMPLEMENTED`. Spike, build edge, đo và production đều `NOT_RUN`
([registry và route matrix](../development/edge-frontend.md)).

Tài liệu này đọc tại `develop@276d142` ngày 24/11/2026. Hướng đi do người dùng yêu cầu
(“Mang hết sang cloudflare phần FE giúp tôi”, 24/11/2026). Cách triển khai (`CHOICE-01`,
`CHOICE-02`) còn `OPEN` tới khi spike có số đo.

## 1. Bối cảnh

Hiện nay ([runbook edge](../../infra/production/cloudflare-edge.md),
[Worker](../../infra/cloudflare/production/worker.mjs)):

```text
trình duyệt ──► Cloudflare (Pages Advanced Mode, _worker.js)
                 ├─ /pkg, /_v/<sha>/pkg  → env.ASSETS (inventory từ image đã qualify)
                 └─ mọi path khác         → https://origin-prod.vink.forum (Caddy → vink-app)
                                             ├─ /health/*, /api/v1/*   (api::routes)
                                             └─ HTML: Leptos SSR (leptos_axum) + PublicReadsAdapter
                                                       đọc in-process Vink<PgStore>
```

- `vink-web` là **một crate, hai build**: `ssr` (kéo `leptos_axum` chỉ để lấy `ResponseOptions`
  đặt status/redirect) và `hydrate` (WASM trình duyệt). Mọi đọc khi render đi qua port
  [`PublicReads`](../../apps/vink-web/src/server.rs): không có phương thức đọc nháp, không nhận
  actor. Trang riêng tư (`/ban-viet`, `/toi`, `/kiem-duyet`…) dùng `LocalResource`, nên SSR không
  đọc cookie và không gọi dữ liệu nào.
- `vink-app` là composition root duy nhất cài `PublicReads`
  ([`http/ssr.rs`](../../services/vink-app/src/http/ssr.rs)), chuyển kiểu provider sang kiểu Web
  qua JSON nên SSR và API cùng một biểu diễn v1. `serve-api` đã mount riêng health + API (URL Web
  trả 404), nhưng Cargo graph vẫn kéo Leptos SSR
  ([scoped workflows](../development/scoped-workflows.md)).
- [#473](../development/evidence/web-reload-473/README.md): TTFB median reload 260 ms. Tách chặng:
  chỉ edge 82 ms, Worker → origin → App (không DB) 173 ms, SSR Async trang chủ 243 ms. Phần lớn là
  mạng Việt Nam → SIN → origin; render/SQL chỉ 20–70 ms.

## 2. Mục tiêu và bất biến

Cloudflare sở hữu toàn bộ frontend: document HTML, routing/deep link, SSR, hydration, asset, trang
lỗi frontend. VPS chỉ giữ App/API, Identity/session, worker, PostgreSQL, ingress API/health.
Không chuyển database, authority Identity hay luật nghiệp vụ.

| ID | Bất biến | Nguồn |
| --- | --- | --- |
| EF-INV-01 | Reader công khai có văn bản cốt lõi, title, canonical và status đúng **trước** JavaScript; không chuyển sang CSR-only | PRD §9 Reader/SSR |
| EF-INV-02 | Render ở edge **không có actor**: không chuyển `Cookie`/`Authorization` của trình duyệt vào lời đọc public, không đọc nháp, không đọc overlay riêng | `PublicReads`, INV-01 |
| EF-INV-03 | Quyền truy cập do backend quyết định trên mỗi lần đọc. Edge không cache chia sẻ HTML hay JSON có thể bị rút/gỡ; READ-01 chỉ revalidate | PUB-03, COM-03, READ-01 |
| EF-INV-04 | HTML SSR và WASM hydrate thuộc **cùng một build** (cùng `VINK_ASSET_PREFIX=/_v/<sha>`); A+B asset giữ cho tab cũ | Runbook edge |
| EF-INV-05 | Một bộ view/router Leptos. Không có HTML hay logic frontend song song | Yêu cầu #504 |
| EF-INV-06 | Secret (`ORIGIN_KEY`, token) chỉ ở binding Worker; không vào asset, HTML, log | AGENTS.md |
| EF-INV-07 | Origin vẫn chỉ nhận request có `X-Vink-Origin-Key` và IP client do edge cấp; rate limit theo IP client thật | Runbook edge |
| EF-INV-08 | Lỗi backend (timeout, 5xx) cho trạng thái lỗi trung thực với status phù hợp; không hiển thị dữ liệu giả hay bản cũ đã mất quyền | AC-03 |

## 3. Kiến trúc đề xuất

```text
trình duyệt ──► Cloudflare: một đơn vị phát hành frontend theo <sha>
                 ├─ router JS (worker.mjs, giữ nguyên hợp đồng asset/proxy)
                 │    ├─ /pkg, /_v/<sha>/pkg      → asset inventory A+B
                 │    ├─ /api/v1/*, /health/*      → origin (như hiện nay)
                 │    └─ document route            → renderer edge (Rust → wasm32)
                 └─ renderer edge: vink_web::{shell, App} + HttpPublicReads
                          └─ fetch actor-free  → origin /api/v1/… (ORIGIN_KEY, IP client, không cookie)
VPS: vink-app serve-api (build không có Leptos/vink-web) · Identity · worker · PostgreSQL
```

### 3.1 Seam tái sử dụng

- **Port giữ nguyên.** Thêm composition root mới cho edge (đề xuất crate
  `apps/vink-edge`, `crate-type = ["cdylib"]`, target `wasm32-unknown-unknown`) cài
  `PublicReads` bằng HTTP tới `/api/v1`. Đây là cặp đối xứng với `PublicReadsAdapter` của App:
  cùng port, cùng kiểu `vink_web::contract`, không thêm domain handler.
- **Đường dẫn API dùng chung.** Nhánh trình duyệt trong
  [`api.rs`](../../apps/vink-web/src/api.rs) đã dựng path (`public_discussion_path`,
  `endorsements_path`, `/api/v1/authors?q=…`). Lát 2 công khai các hàm dựng path đó để edge
  không chép lại query encoding.
- **Status và redirect.** `ResponseStatus` dùng `leptos_axum::ResponseOptions`. `leptos_axum`
  0.8.10 có feature `wasm` (khởi tạo `Executor::init_wasm_bindgen`, bỏ `tokio/fs`). Lát 2 tách
  dependency: `vink-web/ssr` lấy `leptos_axum` với `default-features = false`, host native bật lại
  default, edge bật `wasm`. Giả thuyết này chỉ được nhận khi spike build và render đạt.
- **`Send`.** `PortFuture` yêu cầu `Send`; fetch trong Workers không `Send`. Bọc bằng
  `send_wrapper` như nhánh `hydrate` đang làm (isolate đơn luồng).
- **Ngữ cảnh render.** `SiteOrigin` lấy từ binding Worker. `SignInOpen` và
  `FollowingFeedEnabled` hiện đọc từ `AppState`; ở edge chúng là gợi ý hiển thị, route vẫn tự quyết.
  Lát 2 chọn giữa biến môi trường do controller ghi cùng release (kiểm khớp `release.env`) và một
  endpoint cấu hình công khai đã có version (`EF-GAP-03`). `LocalDiscussionFixture` chỉ có ở dev
  native, không có trên edge production.

### 3.2 Đường đọc public ở edge

`HttpPublicReads` gửi `GET` tới origin với `X-Vink-Origin-Key`, `X-Vink-Edge-Hop`,
`X-Vink-Client-IP` = `CF-Connecting-IP` của request trình duyệt, **không** cookie hay
`Authorization`. Phản hồi có `Set-Cookie` hoặc status ngoài hợp đồng bị coi là `Unavailable`.
Phân loại lỗi tái sử dụng `LoadError` của client trình duyệt (404 → `NotFound`, 400 có mã →
`Invalid`, timeout/5xx/mạng → `Unavailable`). Timeout mỗi lời gọi có ngân sách cố định (đề xuất
khởi điểm 2 s, đo ở spike). HTML trả `Cache-Control: no-store` như hiện tại.

Origin không còn render HTML. Health, API, Identity, cookie/CSRF/Origin, `Set-Cookie` và READ-01
(`ETag`/`304`, `private, no-cache, no-transform`) giữ nguyên đường proxy trong `worker.mjs`.

### 3.3 Hiệu năng: không mặc định nhanh hơn

Theo #473, trang không có resource SSR (Bàn viết, Tôi, kiểm duyệt, editor, 404) sẽ **bỏ được**
chặng edge → origin (≈ 90 ms median). Trang công khai có dữ liệu vẫn phải tới origin, giờ qua HTTP
thay vì in-process:

| Trang | Port gọi khi SSR (đọc từ source) | Chuỗi phụ thuộc |
| --- | --- | --- |
| `/` (Khám phá, `?q`, `?tag`) | `works`, `catalog`; `authors` khi tìm tác giả | song song, 1 lượt |
| `/the/:slug` | `catalog`, `works(tag)` | song song, 1 lượt |
| `/tac-pham/:id` | `reader` → `endorsement_summaries(work)` ∥ `discussion` → `endorsement_summaries(comments)` | có thể tới 3 lượt tuần tự |
| `/:author` | `author` | 1 lượt |
| `/:author/nguoi-theo-doi`, `/dang-theo-doi` | `author_relationships`, `author` | cần đo |

Reader có thể **chậm hơn** hiện nay nếu ba lượt tuần tự mỗi lượt ≈ 90 ms. Spike phải đo trước khi
chọn. Hướng giảm trong ranh giới API sẵn có: khởi động các resource độc lập sớm, hoặc một endpoint
đọc public tổng hợp chỉ ghép các read đã có (không logic mới), kèm hợp đồng v1. Không đổi privacy
hay cache để lấy tốc độ.

### 3.4 Giới hạn và chi phí Cloudflare

[Workers limits](https://developers.cloudflare.com/workers/platform/limits/) đọc ngày 24/11/2026:
CPU mỗi request Free 10 ms, Paid 5 phút (mặc định 30 s); bộ nhớ 128 MB/isolate; kích thước Worker
64 MiB chưa nén; startup 1 s; subrequest Free 50, Paid 10.000; 6 kết nối mở đồng thời; asset tĩnh
25 MiB/file. Hệ quả:

- 10 ms CPU của gói Free nhiều khả năng không đủ cho render Leptos (chưa đo). Gói account hiện tại
  `UNKNOWN`. Nếu cần Paid, đó là chi phí chủ dự án phải duyệt trước khi cutover (`EF-GAP-01`).
- WASM SSR ước lượng cùng cỡ WASM hydrate (≈ 7 MB sau `--remove-name-section`), dưới 64 MiB,
  nhưng thời gian instantiate phải nằm trong startup 1 s và cold start phải đo.
- Reader dùng tối đa 4 subrequest và 3 kết nối đồng thời trong giới hạn.

## 4. Lựa chọn còn mở

### CHOICE-01 — Chiến lược render ở edge (`OPEN`, chọn bằng spike)

| | A — SSR Leptos trên Rust Worker cho mọi document route | B — Shell tĩnh pre-render cho route không có dữ liệu + SSR edge cho route công khai |
| --- | --- | --- |
| Một mô hình render | Có. Mọi route đi qua cùng `shell`/`App` | Hai đường: shell build sẵn và SSR |
| Hydration | Fingerprint `data-rendered-route` tính theo path/query thật | Shell tĩnh không có fingerprint của URL động (`/ban-viet/:id`); phải đổi cơ chế `canHydrateCurrentEntry` |
| CPU/chi phí | Render mọi document | Ít render hơn ở trang riêng tư |
| Rủi ro | Tương thích wasm32 của graph `ssr`, startup, CPU | Phân loại route, parity hydrate, phát hành hai loại artifact |

**Khuyến nghị đưa vào spike: A**, vì giữ một đường render và cơ chế fingerprint hiện có; trang
riêng tư không gọi dữ liệu nên chi phí render thấp. B chỉ xét lại nếu số đo CPU/chi phí của A
vượt ngân sách. Không xây đủ cả hai.

### CHOICE-02 — Hình dạng phát hành frontend (`OPEN`)

| | A — Giữ Pages `vink-production`, `_worker.js` thành thư mục module gồm router JS + renderer WASM | B — Gộp router, renderer và asset vào một Worker có Static Assets |
| --- | --- | --- |
| Giữ cơ chế đang chạy | Giữ project, custom domain, alias `r-<26>`, controller | Cutover project, domain, alias fallback và controller |
| Nguyên tử theo `<sha>` | Một deployment Pages chứa cả renderer và asset A+B | Một version Worker chứa cả hai |
| Điều cần chứng minh | Pages Advanced Mode import được module WASM, startup trong giới hạn | Rollback/preview, giới hạn file, chi phí |

**Khuyến nghị đưa vào spike: A**, giữ cơ chế phát hành đã qualify. Không dùng service binding tới
một Worker render riêng vì tách renderer khỏi asset sẽ phá EF-INV-04 trừ khi version theo SHA.

## 5. Lỗi, cache và bảo mật

- Origin down hoặc timeout: trang công khai trả trạng thái “không tải được” với `503`, `no-store`;
  không fallback HTML từ origin hay cache. Trang riêng tư vẫn render shell; dữ liệu do trình duyệt
  tải qua `/api/v1` và tự báo lỗi như hiện tại.
- 404/410/redirect (`/@handle`, `/toi/dia-chi`, `/ban-viet/:id/kiem-duyet`) giữ đúng status do
  view đặt qua `ResponseStatus`.
- Không Cache API, Cache Everything hay Always Online cho HTML. Edge không cache JSON public; nếu
  sau này cần, chỉ revalidate bằng `ETag` với origin (không giảm RTT, chỉ giảm byte).
- Hai phiên khác nhau nhận HTML public giống hệt nhau; trang riêng tư không chứa dữ liệu tài khoản
  trong HTML/hydration (đã đúng nhờ `LocalResource`, kiểm lại trên edge).
- Request gửi từ edge tới origin bị loop guard hiện có (`X-Vink-Edge-Hop`) chặn nếu vòng ngược.

## 6. Phát hành, rollback và dọn dẹp

- Frontend phát hành theo `<sha>` độc lập khi hợp đồng `/api/v1` tương thích. Controller kiểm
  tương thích bằng contract fixtures trước khi promote (`EF-GAP-04`).
- Rollback frontend: publish lại deployment chứa renderer A + asset A+B. Rollback origin không
  cần đụng frontend nếu API tương thích.
- Trong cửa sổ chuyển đổi, `serve` (SSR native) giữ làm đường rollback có **hạn chót** ghi trong
  receipt. Sau khi live smoke xác nhận không còn request HTML tới origin: image production chạy
  `serve-api`, `vink-app` có feature tắt `vink-web`/Leptos, xóa `ssr.rs` khỏi build production,
  Containerfile không copy `site/` vào image backend. Dev native (`cargo xtask dev web`) chỉ giữ
  nếu có owner và không được dùng làm bằng chứng cho edge.

## 7. Chia lát

1. **Lát 1 (PR này):** ADR, route/ownership matrix, giới hạn, giao thức spike, proposal `PENDING`.
2. **Lát 1b — spike có giới hạn:** build `vink-edge` cho Home + Reader trong workerd/Wrangler đã
   pin, đo theo §3.3 và giao thức trong [registry](../development/edge-frontend.md#giao-thức-spike),
   chốt CHOICE-01/02 bằng số đo (revision r2 của proposal).
3. **Lát 2 — render edge đủ route:** tách feature `leptos_axum`, `HttpPublicReads`, router →
   renderer, route matrix đủ, test no-JS/hai tài khoản/rút bài/outage với backend scratch.
4. **Lát 3 — phát hành:** controller/packager đóng gói renderer, A+B, rollback, staged smoke.
5. **Lát 4 — API-only VPS:** build backend không Leptos, gỡ đường SSR/static, cập nhật runbook;
   cutover production là bước riêng cần quyền.

## 8. Phương án đã loại

- **CSR-only cho Reader:** vi phạm EF-INV-01.
- **Giữ proxy SSR về VPS làm kết quả cuối:** không đạt mục tiêu #504.
- **Viết lại frontend (React/TS) hoặc HTML riêng cho edge:** vi phạm EF-INV-05.
- **Đưa PostgreSQL/Identity hoặc luật truy cập lên edge:** ngoài phạm vi, vi phạm D-01.
