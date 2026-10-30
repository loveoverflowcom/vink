# Phát triển Vink: setup, lệnh và giới hạn skeleton

**Ngày:** 30/10/2026 · **Trạng thái:** Skeleton đã triển khai và chạy được cục bộ. Đây là nền
phát triển, không phải pilot hay production. Các lựa chọn dưới đây là giả định bootstrap; quyết
định D-02–D-12 trong [decision log](../architecture/decisions.md) vẫn giữ nguyên trạng thái.

Trang này là **nguồn duy nhất cho lệnh**. Skills, README và CI dẫn tới đây; định nghĩa thật của
lệnh nằm trong [`xtask/src/main.rs`](../../xtask/src/main.rs).

## Skeleton làm được gì

- Workspace Rust (toolchain pin 1.99.1) gồm ứng dụng Leptos SSR + Axum, bundle hydration WASM,
  worker và `xtask`.
- PostgreSQL 17 với migration có phiên bản: `principals`, `works`, `working_drafts` (bản nháp có
  owner và version), `revisions` (snapshot bất biến, có `content_hash`, trigger chặn UPDATE) và
  `publications` (con trỏ công khai, chỉ trỏ tới revision của chính tác phẩm).
- Trang chủ hữu hạn và trang đọc `/tac-pham/{id}`: HTML có nội dung, title, description, canonical
  từ server; chỉ đọc snapshot qua `publications`, không bao giờ đọc `working_drafts`.
- Bàn viết `/ban-viet` và `/ban-viet/{id}`: textarea, tạo nháp, lưu có điều kiện theo version,
  trạng thái chưa lưu/đang lưu/đã lưu/lỗi/xung đột; chỉ báo đã lưu sau xác nhận của server;
  phản hồi cũ không đánh dấu nội dung mới hơn là đã lưu; xung đột giữ nguyên nội dung đang viết.
  Server chỉ render khung trang, bản nháp được tải bằng API sau hydration.
- API JSON v1 có contract và fixtures do server sở hữu:
  [services/vink-app/contracts/http/v1](../../services/vink-app/contracts/http/v1/README.md).
- `/health/live` (tiến trình còn phục vụ) tách khỏi `/health/ready` (database tới được và
  migrations đã chạy đủ). Discourse và AI moderation được báo `not_implemented`.

## Chuẩn bị môi trường

| Công cụ | Dùng cho | Ghi chú |
| --- | --- | --- |
| rustup | mọi lệnh | `rustup toolchain install` trong repo đọc [rust-toolchain.toml](../../rust-toolchain.toml) (1.99.1, rustfmt, clippy, wasm32) |
| `wasm-bindgen` CLI | build bundle | Phải bằng phiên bản `wasm-bindgen` trong `Cargo.lock` (hiện 0.2.129): `cargo install wasm-bindgen-cli --version 0.2.129 --locked` |
| PostgreSQL 17 | dev, `verify postgres`, `verify web` | [infra/dev/compose.yaml](../../infra/dev/compose.yaml) chạy bằng Podman hoặc Docker; role cần quyền `CREATEDB` cho test |
| Node.js 22+ và Chrome/Chromium | `verify web` | Script không có npm dependency; đặt `VINK_CHROME` nếu Chrome không nằm trong `PATH` |

Thiết lập lần đầu:

```bash
cp .env.example .env
```

```bash
podman compose -f infra/dev/compose.yaml up -d
```

```bash
cargo xtask doctor
```

```bash
cargo xtask db seed
```

```bash
cargo xtask dev
```

Có thể thay `podman compose` bằng `docker compose`. `cargo xtask dev` tự build và chạy migrations;
dữ liệu mẫu chỉ được nạp khi gọi `db seed`. Mở đúng `VINK_PUBLIC_ORIGIN`
(mặc định `http://127.0.0.1:3000`): lệnh ghi từ một origin khác bị trả `403`.

## Lệnh

| Lệnh | Việc thực hiện | Thoát khác 0 khi |
| --- | --- | --- |
| `cargo xtask doctor` | Kiểm toolchain, wasm target, wasm-bindgen, `DATABASE_URL` (ẩn credential), kết nối PostgreSQL; Node/Chrome là cảnh báo | Thiếu điều kiện cho `dev` |
| `cargo xtask build [--release]` | Build `vink-app`, `vink-worker` và `target/site/pkg/{vink.js, vink_bg.wasm, vink.css}` | Build hoặc wasm-bindgen lỗi |
| `cargo xtask db migrate` | Chạy migrations lên `DATABASE_URL` | Không kết nối được hoặc migration lỗi |
| `cargo xtask db seed` | Nạp dữ liệu tổng hợp có nhãn `[Mẫu]` (chỉ `VINK_PROFILE=local`, idempotent) | Profile khác `local` hoặc chưa migrate |
| `cargo xtask dev` | Build, migrate rồi chạy server ở `VINK_BIND` | Bất kỳ bước nào lỗi |
| `cargo xtask verify quick` | `fmt`, clippy server (workspace, `ssr`), clippy browser (`hydrate`, wasm32), unit/contract tests không cần database, kiểm skills/link/ranh giới module | Một bước lỗi |
| `cargo xtask verify postgres` | Test trên PostgreSQL thật, config sai thoát 2, worker start/SIGTERM | Thiếu PostgreSQL hoặc một bước lỗi |
| `cargo xtask verify web` | Build, database tạm `vink_verify_web`, server thật, Chrome headless, SIGTERM, restart | Thiếu PostgreSQL/Node/Chrome/wasm-bindgen hoặc một bước lỗi |
| `cargo xtask verify all` | `quick`, `postgres`, `web` theo thứ tự | Lane đầu tiên lỗi |

Mỗi lane in bảng `PASS`/`FAIL`; bước sau bước lỗi là `NOT_RUN`. Thiếu dependency là `FAIL`,
không bao giờ là `PASS`. `xtask` đọc `.env` cho các tiến trình nó chạy (biến môi trường thật được
ưu tiên); bản thân binary chỉ đọc biến môi trường. `xtask` không cài toolchain, không bật
container, không push hay deploy. CI ([.github/workflows/ci.yml](../../.github/workflows/ci.yml))
chỉ chuẩn bị Rust, wasm-bindgen, Node và PostgreSQL rồi gọi đúng ba lane này.

Binary `vink-app` nhận `serve` (mặc định), `migrate`, `dev-seed`. Mã thoát: `0` thành công,
`1` lỗi runtime, `2` cấu hình hoặc cách gọi sai. `vink-worker` dùng cùng cấu hình.

## Cấu hình

| Biến | Mặc định | Ý nghĩa |
| --- | --- | --- |
| `DATABASE_URL` | bắt buộc | URL PostgreSQL; không bao giờ được in ra |
| `VINK_PROFILE` | `production` | `local` mở seed và identity fixture; mặc định đóng |
| `VINK_BIND` | `127.0.0.1:3000` | Địa chỉ lắng nghe |
| `VINK_PUBLIC_ORIGIN` | `http://{VINK_BIND}` khi `local`; bắt buộc ở `production` | Origin cho canonical URL và kiểm `Origin` của PUT/POST |
| `VINK_SITE_ROOT` | `target/site` | Thư mục chứa `pkg/` |
| `VINK_DEV_PRINCIPAL` | không có | UUID principal fixture; chỉ hợp lệ khi `local` và `VINK_BIND` là loopback |
| `VINK_LOG_FORMAT` | `text` | `json` cho log có cấu trúc một dòng một object; `RUST_LOG` chỉnh filter |

**Identity fixture không phải đăng nhập.** Khi bật, mọi request đến từ loopback hành động như
`VINK_DEV_PRINCIPAL`. Ngoài fixture, server không có nguồn xác thực nào nên đường riêng tư trả
`401`. Fixture không chứng minh ACC-01, ACC-02 hay S-03; không tự xây hệ mật khẩu để lấp chỗ này.

## Cấu trúc và chiều phụ thuộc

```text
apps/vink-web/            crate vink-web: view Leptos, route, DTO riêng của web, CSS tokens
  src/contract.rs         biểu diễn API v1 của consumer, kiểm bằng fixtures của server
  src/api.rs              ssr: đọc qua PublicReads; hydrate: fetch /api/v1
  src/server.rs           (ssr) trait PublicReads do composition root cung cấp
  src/desk_state.rs       máy trạng thái lưu nháp, thuần, có unit test
  src/document.rs         textarea ↔ document v1, DocumentView
  style/vink.css          token M3 Expressive của Vink, sáng/tối
  e2e/browser-smoke.mjs   kiểm trình duyệt qua Chrome DevTools Protocol
services/vink-app/        crate vink-app: composition root, bin vink-app và vink-worker
  src/domain/             luật thuần: document, title, version, revision hash
  src/application/        use case và port (DraftStore, PublicationStore)
  src/persistence/        adapter PostgreSQL (SQLx), migrator nhúng
  src/http/               Axum: API v1, health, SSR host, Origin check
  src/identity.rs         seam xác định principal phía server
  src/config.rs           cấu hình chung, kiểm hợp lệ, ẩn secret
  src/dev_seed.rs         dữ liệu tổng hợp cho local
  migrations/             migration có phiên bản
  contracts/http/v1/      contract API và fixtures (owner: server)
  tests/postgres.rs       test PostgreSQL thật và HTTP qua router
xtask/                    command owner cho local và CI
infra/dev/compose.yaml    PostgreSQL local
```

Chiều phụ thuộc: `http`, `persistence` → `application` → `domain`. `vink-app` phụ thuộc
`vink-web` (feature `ssr`) và cung cấp `PublicReads`; SSR gọi application trong tiến trình, không
vòng HTTP về chính server. `vink-web` không phụ thuộc `vink-app`, SQLx hay secrets; build
`hydrate` chỉ chứa view, DTO và fetch client. `verify quick` kiểm các import bị cấm trong
`domain`/`application` và dependency của `vink-web`. Hai feature `ssr` và `hydrate` là hai build
riêng; không dùng `--all-features`.

## Bằng chứng theo ranh giới

| Lane | Chứng minh | Không chứng minh |
| --- | --- | --- |
| `quick` | Luật document/title/version/hash; config; máy trạng thái lưu (phản hồi cũ, một request đang bay, xung đột); fixtures provider/consumer; outage PostgreSQL → readiness `503`, API `dependency_unavailable`, reader `503`; private route `401` khi không có authority | Transaction PostgreSQL, trình duyệt |
| `postgres` | Migrate database mới; lưu có điều kiện; 8 lần lưu đồng thời chỉ 1 thắng; actor khác không đọc/ghi/liệt kê được; NFC, dòng, khổ, thụt lề qua database; đọc công khai trả snapshot chứ không trả nháp; revision không UPDATE được; con trỏ không trỏ sang tác phẩm khác; HTTP 401/403/404/409/422; HTML reader không chứa nháp; config sai thoát 2; worker start/stop | Hành vi trình duyệt, IME thật |
| `web` | HTML có nội dung trước JavaScript; hydration không lỗi console; điều hướng client không tải lại trang; tạo, gõ, lưu, reload; xung đột với tab khác giữ nội dung và cho lưu đè có chủ ý; SIGTERM thoát 0; dữ liệu còn sau restart | IME hệ điều hành thật (chỉ `Input.insertText`/`imeSetComposition`), mobile thật, screen reader, nhiều trình duyệt |

Không lane nào chứng minh Discourse, AI moderation, xác thực, publish/withdraw, hiệu năng hay
triển khai. Chưa có model Quint; skeleton chưa có race đủ phức tạp để mô hình hóa ngoài CAS
đã được test trên PostgreSQL.

## Lựa chọn bootstrap và điều kiện xem xét lại

| Lựa chọn | Lý do | Xem lại khi |
| --- | --- | --- |
| Một crate `vink-app` chứa domain/application/adapters, `vink-web` riêng | Ranh giới browser/server là ranh giới build thật; domain chưa có consumer thứ hai | Domain cần dùng từ crate khác (ví dụ một tool), hoặc compile time trở thành vấn đề |
| Worker là binary thứ hai trong `vink-app` | Dùng chung config/application; chưa có job | Có job thật cần deploy hoặc scale riêng |
| REST JSON v1 cho browser, SSR đọc trong tiến trình qua `PublicReads` | Server sở hữu contract; consumer có DTO riêng; không có luật nghiệp vụ thứ hai trong server function | Editor/frontend thay thế (D-04) cần transport khác |
| Bàn viết chỉ render khung ở server, tải nháp bằng API | Không có đường nào đưa nháp vào HTML/hydration payload | Có auth thật và cần render nháp cho chủ sở hữu ở server |
| Document v1: block là danh sách dòng, NFC | Giữ khổ thơ, xuống dòng, thụt lề; hash ổn định giữa cách gõ dấu dựng sẵn và tổ hợp | S-01 chọn editor và schema dài hạn; cần migration document |
| Lưu thủ công (nút, Enter ở tiêu đề, Ctrl/Cmd+S), chưa autosave | Đủ kiểm CAS và trạng thái lưu; tránh hẹn giờ trước khi chọn editor | Làm WR-01 đầy đủ trong S-01 |
| Origin check cho PUT/POST | Seam CSRF trước khi có cookie session | Chọn mô hình session/CSRF cùng D-05 |
| PostgreSQL 17.11, Rust 1.99.1, Leptos 0.8.22, SQLx 0.9.0, Axum 0.8.9 | Bản ổn định đã kiểm tương thích ngày 30/10/2026, khóa trong `Cargo.lock` | Nâng cấp có chủ đích, chạy lại cả ba lane |
| Font hệ thống có hỗ trợ tiếng Việt, chưa tải web font | Tránh chính sách asset chưa chọn | Chọn bộ font và quy trình asset |

## Chưa triển khai hoặc chưa kiểm chứng

- Đăng nhập, session, tài khoản thật (D-05, S-03, ACC-01/ACC-02).
- Gửi duyệt, moderation người/AI, publish intent, publish/withdraw, curation, Tạp Chí
  (PUB-01–PUB-03, MOD-01–MOD-06, ED-02). Chỉ seed local tạo `publications`.
- Discourse, outbox, inbox, job, reconciliation (D-08, S-02, S-04). Worker chưa có job nào.
- Editor hoàn chỉnh, autosave, paste từ Word/Docs, undo, IME thật trên thiết bị (WR-01/WR-02, S-01).
- Export (WR-05), nhiều chương (WR-04), tìm kiếm (UX-02), backup/restore (OPS-02).
- Rời Bàn viết bằng điều hướng trong app (link "Bàn viết", "Đọc") chưa cảnh báo khi còn nội dung
  chưa lưu; cảnh báo hiện chỉ có khi đóng hoặc tải lại tab (`beforeunload`).
- Kiểm tra trên mobile thật, screen reader và đo hiệu năng.

## Việc tiếp theo

1. **S-01 Editor + SSR:** thử editor JavaScript (ProseMirror hoặc tương đương) sau ranh giới
   `DocumentView`/document v1; IME thật trên macOS/Windows/Android/iOS, paste, undo, autosave
   dùng lại `EditorSession`, hai tab, mobile.
2. **S-03 Identity:** chọn auth authority, thay `LocalFixture` bằng resolver thật giữ nguyên
   seam `IdentityResolver`, cookie/CSRF, liên kết principal; thử đổi email, ban, logout.
3. **WR-03 + PUB-01/02 trên PostgreSQL (nền cho S-04):** tạo revision bất biến từ nháp khi gửi,
   ý định xuất bản, publish transaction ghi con trỏ + audit + outbox, test r1/r2.
4. **S-02 Discourse:** pin release và plugin, thử cổng tiền kiểm trước khi viết adapter.
5. **S-05 AI + vận hành:** bộ mẫu tiếng Việt, schema output và job lease trong worker; giữ
   AI ở chế độ tư vấn cho tới khi MOD-06 có bằng chứng.
