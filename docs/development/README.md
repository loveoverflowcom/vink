# Phát triển Vink: setup, lệnh và giới hạn skeleton

**Ngày:** 30/10/2026 · **Trạng thái:** Skeleton đã triển khai và chạy được cục bộ. Đây là nền
phát triển, không phải pilot hay production. Các lựa chọn dưới đây là giả định bootstrap; quyết
định D-02–D-12 trong [decision log](../architecture/decisions.md) vẫn giữ nguyên trạng thái.

Trang này là **nguồn duy nhất cho lệnh**. Skills, README và CI dẫn tới đây; định nghĩa thật của
lệnh nằm trong [`xtask/src/main.rs`](../../xtask/src/main.rs).

[Bản đồ tính năng và đường kiểm chứng](feature-map.md) dẫn từ route, thao tác và trạng thái
người dùng tới source/assertion hiện có; đây không phải bảng kết quả chạy hay nghiệm thu.

## Skeleton làm được gì

- Workspace Rust (toolchain pin 1.99.1) gồm ứng dụng Leptos SSR + Axum, bundle hydration WASM,
  worker và `xtask`.
- PostgreSQL 17 với migration có phiên bản: `principals`, `works`, `working_drafts` (bản nháp có
  owner và version), `revisions` (snapshot bất biến, có `content_hash`, trigger chặn UPDATE) và
  `publications` (con trỏ công khai, chỉ trỏ tới revision của chính tác phẩm).
- Khám phá `/` theo lớp đọc v3 ([#9](https://github.com/loveoverflowcom/vink/issues/9)): luồng
  tối đa 12 tác phẩm mới công khai, tên tác giả/ngày UTC, điểm dừng và gợi ý bắt đầu viết;
  phân biệt loading/rỗng/unavailable. Không spotlight hay tương tác thiếu chủ dữ liệu. Xem
  [spec và delta](../design/public-reading.md). Trang đọc `/tac-pham/{id}`: HTML có nội dung, title, description, canonical
  từ server; chỉ đọc snapshot qua `publications`, không bao giờ đọc `working_drafts`.
- Dashboard `/ban-viet` ([#8](https://github.com/loveoverflowcom/vink/issues/8)): Viết tiếp mở
  nháp mới cập nhật nhất có quyền; danh sách hiện title/version/updated_at, tìm tiêu đề có/không
  dấu (cả Đ/đ, NFC/NFD), sort trong tối đa 100 nháp đã tải và ghi rõ phạm vi. Tách loading/rỗng/
  không kết quả/401/lỗi; không tải toàn bộ manuscript cho thẻ. Tạo khóa một POST đang bay,
  không tự retry kết quả chưa rõ; tải lại danh sách thành công rồi mới cho chủ động tạo thêm.
  Phản hồi tạo muộn sau khi rời dashboard không điều hướng. Xem [spec/delta](../design/README.md#dashboard-bàn-viết-8).
- Bàn viết `/ban-viet/{id}`: nội dung soạn bằng CodeMirror 6
  ([runtime riêng](../../apps/vink-web/editor/README.md)), tạo nháp, lưu có điều kiện theo version,
  trạng thái chưa lưu/đang lưu/đã lưu/lỗi/xung đột; chỉ báo đã lưu sau xác nhận của server;
  phản hồi cũ không đánh dấu nội dung mới hơn là đã lưu; xung đột giữ nguyên nội dung đang viết.
  Xem trước và tải `.txt` từ chữ đang có trên trang. Rời editor khi còn chữ chưa được xác nhận
  (link, brand, back/forward) mở hộp thoại Ở lại / Lưu và rời trang; khi lưu lỗi, xung đột hoặc
  còn treo thì có thêm Tải bản đang viết và Rời mà không lưu (xác nhận hai bước). Server chỉ render
  khung trang, bản nháp được tải bằng API sau hydration.
- Shell thích ứng ([#7](https://github.com/loveoverflowcom/vink/issues/7)): Khám phá (`/`) trước
  Bàn viết, Tôi ở chân rail. Desktop từ 1100px mặc định 220px, tablet 760–1099px và editor
  mặc định 96px; nút thu/mở ghi đè mặc định và nhớ trên thiết bị. Dưới 760px dùng bottom bar
  Khám phá → Bàn viết → Tôi, ẩn khi viết. Web/tablet mở Tôi bằng overlay; mobile tới `/toi`.
  Sáng/Tối/Hệ thống chỉ có trong Tôi. Preference lỗi/không có storage vẫn dùng được trong phiên.
  Không có profile API hay danh tính được suy ra từ fixture; xem [spec shell](../design/README.md#shell-thích-ứng-7).
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
| Node.js 22+ và npm | `build`, `dev`, `verify web` | Cài dependency đã khóa và bundle [runtime editor](../../apps/vink-web/editor/README.md) (cần mạng tới npm registry lần đầu hoặc khi lockfile đổi) |
| Chrome/Chromium | `verify web` | Script smoke không có npm dependency; đặt `VINK_CHROME` nếu Chrome không nằm trong `PATH` |

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
| `cargo xtask doctor` | Kiểm toolchain, wasm target, wasm-bindgen, Node 22+/npm, `DATABASE_URL` (ẩn credential), kết nối PostgreSQL; Chrome là cảnh báo | Thiếu điều kiện cho `dev` |
| `cargo xtask build [--release]` | Build `vink-app`, `vink-worker` và `target/site/pkg/{vink.js, vink_bg.wasm, snippets/, vink-editor.js, vink.css, fonts/}`; `npm ci --ignore-scripts` cho runtime editor khi lockfile đổi | Build, wasm-bindgen, npm hoặc type-check/bundle editor lỗi |
| `cargo xtask db migrate` | Chạy migrations lên `DATABASE_URL` | Không kết nối được hoặc migration lỗi |
| `cargo xtask db seed` | Nạp dữ liệu tổng hợp có nhãn `[Mẫu]` (chỉ `VINK_PROFILE=local`, idempotent) | Profile khác `local` hoặc chưa migrate |
| `cargo xtask dev` | Build, migrate rồi chạy server ở `VINK_BIND` | Bất kỳ bước nào lỗi |
| `cargo xtask verify quick` | `fmt`, clippy server (workspace, `ssr`), clippy browser (`hydrate`, wasm32), unit/contract tests không cần database, kiểm skills/link/ranh giới module, font asset theo `provenance.json` và phiên bản pin của runtime editor | Một bước lỗi |
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
  src/shell.rs            rail/bar/overlay; giữ route/editor, dùng NavigationGuard cho chuyển trang
  src/appearance.rs       theme/rail local, bootstrap trước CSS, initials thuần có fixture tiếng Việt
  src/pages/me.rs         /toi: thông tin tài khoản chưa có, chọn giao diện
  src/contract.rs         biểu diễn API v1 của consumer, kiểm bằng fixtures của server
  src/api.rs              ssr: đọc qua PublicReads; hydrate: fetch /api/v1
  src/server.rs           (ssr) trait PublicReads do composition root cung cấp
  src/draft_list.rs       tìm/sort nháp đã tải và trạng thái tạo/đối chiếu kết quả, thuần
  src/desk_state.rs       máy trạng thái lưu nháp và rời editor (EditorSession, LeaveGuard), thuần
  src/document.rs         chữ nguồn ↔ document v1, DocumentView, export .txt
  src/editor.rs           (hydrate) cầu wasm-bindgen tới CodeMirror, BodySource::snapshot
  src/navigation.rs       NavigationGuard dùng chung cho link, back/forward, điều hướng bằng code
  editor/                 runtime CodeMirror 6 (TypeScript, npm pin, esbuild) → /pkg/vink-editor.js
  style/vink.css          owner token M3 Expressive Lavender: sáng/tối, `data-theme`, primitives
  assets/fonts/           WOFF2 tự host, OFL, provenance.json (phục vụ tại /pkg/fonts/)
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
| `quick` | Tìm tiêu đề NFC/NFD/Đ/đ và guard tạo/đối chiếu kết quả; luật document/title/version/hash; config; máy trạng thái lưu (phản hồi cũ, một request đang bay, xung đột); fixtures provider/consumer; outage PostgreSQL → readiness `503`, API `dependency_unavailable`, reader `503`; private route `401` khi không có authority | Transaction PostgreSQL, trình duyệt |
| `postgres` | Migrate database mới; lưu có điều kiện; 8 lần lưu đồng thời chỉ 1 thắng; actor khác không đọc/ghi/liệt kê được; NFC, dòng, khổ, thụt lề qua database; đọc công khai trả snapshot chứ không trả nháp; revision không UPDATE được; con trỏ không trỏ sang tác phẩm khác; HTTP 401/403/404/409/422; HTML reader không chứa nháp; config sai thoát 2; worker start/stop | Hành vi trình duyệt, IME thật |
| `web` | Dashboard: resume/list không N+1, fixtures 100 row/rỗng/lỗi/401, tìm/sort/retry, POST commit thật rồi mất ack và GET đối chiếu, phản hồi tạo muộn không điều hướng; HTML có nội dung trước JavaScript; hydration không lỗi console; shell 220/96, thứ tự rail/bar, theme + reload + hệ thống + storage denied, overlay bàn phím/Escape/outside/resize/restore-focus, bottom clearance 320px/chữ 200%; trang đọc tải Be Vietnam Pro và Literata tự host, file font trả `font/woff2`, không tải runtime editor; điều hướng client không tải lại trang; CodeMirror mount từ `/pkg/vink-editor.js`, không còn textarea; tạo, gõ, lưu (nút, Ctrl+S), reload; xung đột với tab khác giữ nội dung và cho lưu đè có chủ ý; cùng một EditorView giữ selection/undo/redo qua đổi theme, xem trước và hộp thoại; brand/link/back bị giữ khi chưa lưu, Escape ở lại; lưu rồi rời chỉ sau ack; hủy khi đang lưu thì ack muộn không điều hướng; lỗi lưu → tải bản đang viết → rời mà không lưu, ack muộn không điều hướng hay đánh dấu nháp khác đã lưu; nội dung tệp `.txt`; SIGTERM thoát 0; dữ liệu còn sau restart | IME hệ điều hành thật (chỉ `Input.insertText`/`imeSetComposition`), bàn phím/thiết bị mobile thật, screen reader, nhiều trình duyệt, paste từ Word/Docs |

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
| CodeMirror 6 sửa chữ nguồn của nội dung, build bằng npm/esbuild riêng ([#6](https://github.com/loveoverflowcom/vink/issues/6)) | Giữ undo, selection, IME tốt hơn textarea mà không đổi document v1; runtime chỉ tải ở editor | S-01 chọn editor/schema dài hạn (D-04), cần rich text hoặc bundle quá lớn |
| Guard rời editor: hộp thoại riêng cho điều hướng trong app, `beforeunload` cho reload/đóng tab | Router không hủy được back/forward; guard chặn trước router và đẩy URL trang về | Router có API chặn điều hướng, hoặc shell PR 03 cần lối thoát mới |
| Origin check cho PUT/POST | Seam CSRF trước khi có cookie session | Chọn mô hình session/CSRF cùng D-05 |
| PostgreSQL 17.11, Rust 1.99.1, Leptos 0.8.22, SQLx 0.9.0, Axum 0.8.9 | Bản ổn định đã kiểm tương thích ngày 30/10/2026, khóa trong `Cargo.lock` | Nâng cấp có chủ đích, chạy lại cả ba lane |
| Font tự host: Be Vietnam Pro 400/600 và Literata variable, WOFF2 subset Latin + tiếng Việt ([assets/fonts](../../apps/vink-web/assets/fonts/README.md)) | Bộ thiết kế Lavender chọn hai family OFL này; 332 KB thay vì 1,37 MB TTF; nguồn, hash và license nằm trong repo | Cần weight/italic/script mới, hoặc chọn chính sách cache/CDN production |

## Chưa triển khai hoặc chưa kiểm chứng

- Đăng nhập, session, tài khoản thật (D-05, S-03, ACC-01/ACC-02).
- Gửi duyệt, moderation người/AI, publish intent, publish/withdraw, curation, Tạp Chí
  (PUB-01–PUB-03, MOD-01–MOD-06, ED-02). Chỉ seed local tạo `publications`.
- Discourse, outbox, inbox, job, reconciliation (D-08, S-02, S-04). Worker chưa có job nào.
- Autosave, bản khôi phục cục bộ, paste từ Word/Docs, IME thật trên thiết bị (WR-01/WR-02, S-01).
  Undo/redo và composition mô phỏng đã có trong editor CodeMirror, chưa phải bằng chứng IME hệ
  điều hành.
- Export ngoài `.txt` (WR-05 mới có bản văn bản thuần), nhiều chương (WR-04), tìm kiếm (UX-02),
  backup/restore (OPS-02).
- Tải `.txt` lần thứ hai trên cùng trang có thể khiến Chrome hỏi “tải nhiều tệp”; đó là quyền của
  trình duyệt, không phải lỗi lưu.
- Kiểm tra trên mobile thật, screen reader và đo hiệu năng.

## Việc tiếp theo

1. **S-01 Editor + SSR:** CodeMirror 6 đang sửa chữ nguồn sau ranh giới `DocumentView`/document v1;
   còn IME thật trên macOS/Windows/Android/iOS, paste, autosave dùng lại `EditorSession`, hai tab,
   mobile, và quyết định có cần rich text (D-04).
2. **S-03 Identity:** chọn auth authority, thay `LocalFixture` bằng resolver thật giữ nguyên
   seam `IdentityResolver`, cookie/CSRF, liên kết principal; thử đổi email, ban, logout.
3. **WR-03 + PUB-01/02 trên PostgreSQL (nền cho S-04):** tạo revision bất biến từ nháp khi gửi,
   ý định xuất bản, publish transaction ghi con trỏ + audit + outbox, test r1/r2.
4. **S-02 Discourse:** pin release và plugin, thử cổng tiền kiểm trước khi viết adapter.
5. **S-05 AI + vận hành:** bộ mẫu tiếng Việt, schema output và job lease trong worker; giữ
   AI ở chế độ tư vấn cho tới khi MOD-06 có bằng chứng.
