# Phát triển Vink: setup, lệnh và cấu trúc

Trang này là **nguồn duy nhất cho setup và lệnh tổng hợp**. Định nghĩa thật của lệnh nằm trong
[`xtask/src/main.rs`](../../xtask/src/main.rs); lệnh theo phạm vi và theo diff ở
[scoped workflows](scoped-workflows.md); giới hạn tài nguyên trên máy chung ở
[local resource budget](local-resource-budget.md). [Bản đồ tính năng](feature-map.md) dẫn từ hành
trình người dùng tới source và test. Quyết định D-02–D-19 giữ trạng thái trong
[decision log](../architecture/decisions.md); một lựa chọn bootstrap ở đây không phải ADR Accepted.

## Chuẩn bị môi trường

| Công cụ | Dùng cho | Ghi chú |
| --- | --- | --- |
| rustup | mọi lệnh | [rust-toolchain.toml](../../rust-toolchain.toml) (1.99.1, rustfmt, clippy, wasm32) |
| `wasm-bindgen` CLI | build bundle | Bằng phiên bản trong `Cargo.lock`: `cargo install wasm-bindgen-cli --version 0.2.129 --locked` |
| PostgreSQL 17 | dev, `verify postgres`, `verify web` | [services/postgres](../../services/postgres/README.md) qua Podman/Docker; `VINK_TEST_DATABASE_URL` là superuser của cluster local/CI |
| `psql` 15+ | `provision.sh`, `verify postgres` | `brew install libpq` hoặc `postgresql-client` |
| Node.js 22+ và npm | `build`, `dev`, `verify quick/web/expressions/discussion` | Dependency khóa theo lockfile; [editor runtime](../../apps/vink-web/editor/README.md) |
| Chrome/Chromium | `verify web`, `ui-preview leptos verify` | Đặt `VINK_CHROME` nếu không nằm trong `PATH` |
| JDK 17+ | `ui-preview cmp …` | Gradle wrapper 8.14.3 |
| Android SDK, Xcode | chỉ `VINK_UI_NATIVE_TARGETS=1` và host chẩn đoán | `ANDROID_HOME` (platform 36); Xcode + Kotlin/Native 2.2.20 |

Thiết lập lần đầu (chi tiết ở [services/postgres](../../services/postgres/README.md)):

```bash
podman volume create vink-dev_vink-postgres
```

```bash
podman compose -f services/postgres/compose.yaml -f services/postgres/compose.local.yaml up -d
```

```bash
services/postgres/provision.sh --env-file services/postgres/provision.env.example
```

```bash
cp .env.example .env
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

`podman compose` có thể thay bằng `docker compose`. `cargo xtask dev` tự build và migrate; dữ liệu
mẫu chỉ nạp khi gọi `db seed`. Mở đúng `VINK_PUBLIC_ORIGIN` (mặc định `http://127.0.0.1:3000`):
lệnh ghi từ origin khác bị trả `403`.

## Lệnh

Lệnh theo phạm vi (`build api`, `dev web`, `verify unit app --filter …`,
`verify web --scenario password`, `--plan`) ở [scoped workflows](scoped-workflows.md).

| Lệnh | Việc thực hiện |
| --- | --- |
| `cargo xtask doctor` | Kiểm toolchain, wasm target, wasm-bindgen, Node/npm, `DATABASE_URL`, `VINK_MIGRATE_DATABASE_URL`; PostgreSQL test, `psql`, Chrome là cảnh báo |
| `cargo xtask build [--release]` | Build `vink-app`, `vink-worker`, `vink-identity` và `target/site/pkg/` (WASM, editor, `vink.css` qua Tailwind v4, fonts, brand, empty artwork sau khi kiểm provenance) |
| `cargo xtask db migrate` / `db seed` | Migrate App bằng `VINK_MIGRATE_DATABASE_URL` rồi Identity nếu có; seed dữ liệu `[Mẫu]` chỉ ở `VINK_PROFILE=local` |
| `cargo xtask dev` | Build, migrate, chạy Identity (nếu có `VINK_IDENTITY_SOCKET`) rồi server ở `VINK_BIND` |
| `cargo xtask decision --help` | Tra quyết định, diff revision, proposal/signoff ([kho TOML](../../decisions/README.md)) |
| `cargo xtask ui-tokens [--check]` | Validate nguồn UI system và sinh CSS/Rust/Kotlin; `--check` không ghi |
| `cargo xtask ui-preview leptos\|cmp build\|run\|verify` | Preview Leptos (loopback `127.0.0.1:3019`) và CMP Desktop; `verify` chụp và kiểm catalog |
| `cargo xtask verify quick` | fmt, clippy (ssr/hydrate/csr/preview), unit/contract/doctests, `ui-tokens --check`, repository guards (decisions, skills, links, boundaries, fonts, brand, editor pins, Tailwind, secrets, bloat) |
| `cargo xtask verify postgres` | Integration tests Store/App/Identity/Notifications trên PostgreSQL thật, provisioning, backup/restore, binary thật |
| `cargo xtask verify discussion` | Quint (lockfile 0.33.1): typecheck, simulation, negative controls, drift, replay Rust production có tên |
| `cargo xtask verify web` | Build, database tạm, server thật, Chrome headless, restart; scenario trong [plan.rs](../../xtask/src/plan.rs) |
| `cargo xtask verify expressions` | Noto manifest/hash/drift, Quint lifecycle và replay |
| `cargo xtask verify kani` | Kani pin 0.67.0 theo manifest owner-local; đòi đúng số `#[kani::proof]` |
| `cargo xtask verify all` | `quick`, `discussion`, `expressions`, `postgres`, `web` (không gồm `kani`) |

Mỗi lane in `PASS`/`FAIL`; bước sau bước lỗi là `NOT_RUN`; thiếu dependency là `FAIL`. `xtask` đọc
`.env` cho tiến trình con (biến môi trường thật thắng), không cài toolchain, không bật container,
không push hay deploy. [CI](../../.github/workflows/ci.yml) gọi cùng lane và lưu output thô thành
GitHub Actions artifact; cấu hình CI không tự là bằng chứng một run đã PASS.

`verify postgres` mặc định chạy tối đa hai test cùng lúc mỗi gói; `RUST_TEST_THREADS` và
`CARGO_BUILD_JOBS` điều chỉnh mức song song.

### Binary và lệnh vận hành

- `vink-app serve|serve-api|migrate|dev-seed` và lệnh vận hành `admin`, `moderator`, `principal`,
  `tag`, `outbox`, `notifications`, `post-review`, `maintenance prune` (cú pháp: `vink-app` không
  tham số). Mã thoát `0` thành công, `1` lỗi runtime, `2` cấu hình/cách gọi sai. Catalog thẻ chỉ đổi
  qua `vink-app tag …` ([runbook](taxonomy-api.md#6-runbook-vận-hành-catalog)); dead letter qua
  `vink-app outbox …` ([runbook](../architecture/discussion-outbox.md#runbook)).
- `vink-worker`: outbox thảo luận, đánh giá kiểm duyệt, relay thông báo, retention; production từ
  chối role đặc quyền. `VINK_WORKER_OUTBOX=enabled|disabled`, `VINK_WORKER_OUTBOX_CONCURRENCY` (1..6).
- `vink-identity serve|migrate|account` ([README](../../services/vink-identity/README.md)),
  `vink-notifications serve|migrate|maintenance` ([README](../../services/vink-notifications/README.md)).

## Cấu hình

| Biến | Mặc định | Ý nghĩa |
| --- | --- | --- |
| `DATABASE_URL` | bắt buộc cho `serve`, `dev-seed`, worker | Role runtime; không bao giờ được in ra |
| `VINK_MIGRATE_DATABASE_URL` | bắt buộc cho `migrate` | Role migrator; không fallback sang `DATABASE_URL` |
| `VINK_TEST_DATABASE_URL` | chỉ `xtask verify` | Superuser cluster local/CI để tạo database/role tạm |
| `VINK_IDENTITY_SOCKET` / `VINK_IDENTITY_SERVER_UID` | không có | Socket của Identity và UID phải phục vụ nó |
| `VINK_PG_BIN` | `PATH` | `pg_dump`/`pg_restore` cho backup/restore của `verify postgres` |
| `VINK_PROFILE` | `production` | `local` mở seed và identity fixture |
| `VINK_BIND` | `127.0.0.1:3000` | Địa chỉ lắng nghe |
| `VINK_PUBLIC_ORIGIN` | `http://{VINK_BIND}` khi `local`; bắt buộc ở production | Canonical URL và kiểm `Origin` của PUT/POST |
| `VINK_SITE_ROOT` | `target/site` | Thư mục chứa `pkg/` |
| `VINK_DEV_PRINCIPAL` | không có | Principal fixture; chỉ khi `local` và loopback |
| `VINK_AUTH_SIGN_IN` | `disabled` ở production | Mở đăng nhập ([auth operations](auth-operations.md)) |
| `VINK_EVALUATOR` | mock bootstrap | `manual` gửi mọi job cho người ([moderation runtime](moderation-runtime.md)) |
| `VINK_LOG_FORMAT` | `text` | `json` cho log có cấu trúc; `RUST_LOG` chỉnh filter |

**Identity fixture không phải đăng nhập.** Khi bật, mọi request từ loopback hành động như
`VINK_DEV_PRINCIPAL`; fixture không chứng minh ACC-01/02 hay S-03.

## Host chẩn đoán UI native

Hai host chỉ dùng dữ liệu mẫu, không service hay đăng nhập; không có app native sản phẩm. Native
module chỉ configure khi gọi target tương ứng hoặc bật `VINK_UI_NATIVE_TARGETS=1`.

```sh
cargo xtask ui-preview android build|verify|run   # cần emulator/device; ANDROID_SERIAL chọn máy
VINK_UI_IOS_DEVICE=<UDID> cargo xtask ui-preview ios build|verify|run   # macOS, simulator arm64
```

`VINK_UI_ENTRY`, `VINK_UI_SCENARIO`, `VINK_UI_THEME=light|dark|system`, `VINK_UI_MOTION`,
`VINK_UI_SCALE=100|200` chọn replay khi `run`. Android `verify` chạy instrumentation thật; iOS chạy
`xcodebuild test` scheme `VinkUIDiagnostic`. Lossy WebP: `VINK_UI_ENTRY=identity.image-frame`.
Chưa qualified: TalkBack/VoiceOver, IME tiếng Việt, predictive Back/swipe-back, thiết bị thật,
signing, cửa sổ Desktop thật, pipeline font branded TTF/OTF.

## Cấu trúc và chiều phụ thuộc

```text
apps/vink-web/            crate vink-web: route, view Leptos, DTO consumer, CSS; editor/ (CodeMirror 6),
                          assets/{fonts,brand,empty}/ có provenance, e2e/ browser smoke (CDP), behavior/ model Web
apps/vink-preview/        Preview (Leptos + CMP Desktop): specimen/mock/demo, không service, không ship
                          cùng Product ([ranh giới](web-preview-boundary.md))
packages/ui-system/       tokens.json, catalog, fixtures; crate vink-ui; Gradle :ui, :diagnostics
packages/vink-patterns/   composition Vink không DTO cho Web và Preview
packages/expressions/     manifest Noto, policy, Kani manifest
packages/store-postgres/  pool, vòng đời migration, phân loại lỗi kết nối
services/vink-app/        composition root, bin vink-app và vink-worker
  src/domain/             luật thuần; src/application/ use case và port; src/persistence/ SQLx;
  src/http/ Axum; src/identity/ resolver phiên; migrations/; contracts/http/v1/; behavior/ Quint;
  kani/; bench/ (benchmark PostgreSQL tái lập)
services/vink-identity/   bin riêng trên Unix socket, database vink_identity
services/vink-notifications/ bin riêng, database vink_notifications
services/postgres/        container, provision.sh, runbook
xtask/                    command owner cho local và CI
infra/                    production, Cloudflare, test host
```

Chiều phụ thuộc: `http`, `persistence` → `application` → `domain`; package không phụ thuộc
service/web/framework HTTP. `vink-app` phụ thuộc `vink-web` (feature `ssr`) và cung cấp
`PublicReads`; `vink-web` không phụ thuộc `vink-app`, SQLx hay secrets. `vink-app`,
`vink-identity`, `vink-notifications` không import nhau và SQL không chạm bảng owner khác.
`verify quick` kiểm các ranh giới này. Mọi crate kế thừa `unsafe_code = "forbid"`; một doctest
`compile_fail` cần lawful twin biên dịch được. `ssr`, `hydrate`, `csr` là build riêng; không dùng
`--all-features`.

## Lane chứng minh gì

| Lane | Chứng minh | Không chứng minh |
| --- | --- | --- |
| `quick` | Luật thuần, use case với store giả, contract fixtures, máy trạng thái Web, guard HTTP khi DB/Identity lỗi, repository guards | Transaction PostgreSQL, trình duyệt |
| `discussion` | Model Quint (D2T, LVE, admission, outbox, governance, ReadOwner/PageChain) và replay qua code production; counterexample phải bị từ chối | Model checking vét cạn, khóa/transaction/I/O, UI |
| `kani` | Quyết định thuần trên toàn miền input trong bounds đã khai báo | Chuỗi nhiều bước, I/O, code ngoài `src/domain` |
| `postgres` | Migration, provisioning, role tách, CAS, race hai kết nối, quyền riêng tư, HTTP qua router, binary thật, backup/restore | Trình duyệt, Compose/container, PostgreSQL managed |
| `web` | SSR/hydration, route, editor, auth, publication, thông báo trên Chrome headless với PostgreSQL scratch | IME hệ điều hành, thiết bị thật, screen reader, trình duyệt khác |
| `ui-preview leptos/cmp verify` | Catalog, component, state, navigation trên renderer thật | App/API/DB, native runtime, chất lượng thị giác chưa mở ảnh |

Không lane nào chứng minh AI moderation thật, hiệu năng production hay triển khai. Model đích
trong `services/vink-app/behavior/target` chạy trong `discussion` nhưng chưa có code thực thi.

## Kani

xtask không tự cài verifier. Pin `cargo-kani 0.67.0`:

```sh
cargo install --locked kani-verifier --version 0.67.0 && cargo kani setup
cargo xtask verify kani
```

Manifest [`services/vink-app/kani`](../../services/vink-app/kani/README.md) và
[`packages/expressions/kani`](../../packages/expressions/kani/README.md) biên dịch đúng source
production. Một import mới trong `src/domain` cần thêm vào manifest cùng thay đổi; `quick` không
biên dịch manifest này. Chạy một harness:
`cargo kani --manifest-path services/vink-app/kani/Cargo.toml --exact --harness <đường dẫn>`.

## Lựa chọn bootstrap và điều kiện xem xét lại

| Lựa chọn | Xem lại khi |
| --- | --- |
| Một crate `vink-app` chứa domain/application/adapters, `vink-web` riêng | Domain cần consumer thứ hai hoặc compile time thành vấn đề |
| Worker là binary thứ hai trong `vink-app` | Job cần deploy/scale riêng |
| REST JSON v1 cho browser, SSR đọc trong tiến trình qua `PublicReads` | Frontend thay thế (D-04) cần transport khác |
| Bàn viết chỉ render khung ở server, tải nháp bằng API | Cần render nháp cho chủ sở hữu ở server |
| Một document canonical `{format, source}` nguyên văn (#255) | Cần dialect Markdown khác |
| CodeMirror 6 sửa chữ nguồn, build npm/esbuild riêng | S-01 chọn editor dài hạn (D-04) hoặc cần rich text |
| Origin check cho PUT/POST | Chọn mô hình session/CSRF cùng D-05 |
| Một instance, database và role riêng cho App/Identity/Notifications | Cần tách failure domain hoặc PostgreSQL managed |
| Font tự host Be Vietnam Pro + Literata WOFF2 subset | Cần weight/script mới hoặc chính sách CDN |

## Owner theo miền

| Miền | Registry / runbook |
| --- | --- |
| Literary core, lưu nháp | [literary-domain-core](literary-domain-core.md), [document format](document-source-format.md), [draft HTTP budget](draft-http-budget.md) |
| Bàn viết | [pagination](desk-pagination.md), [feedback read-model](desk-author-feedback.md), [compact + Thấm](desk-compact-tham.md) |
| Đọc công khai | [READ-01](public-conditional-reads.md), [READ-02](public-read-freshness.md), [SSR Reader](reader-ssr-boundary.md), [perf baseline](web-perf-baseline.md) |
| Thảo luận | [core](discussion-core.md), [Web reads](discussion-web-reads.md), [post-review](discussion-post-review.md) |
| Ghi nhận văn học | [core](endorsement-core.md), [runtime](literary-endorsement-runtime.md) |
| Phân loại, khám phá | [core](taxonomy-core.md), [PostgreSQL](taxonomy-postgres.md), [API](taxonomy-api.md), [editor](taxonomy-editor.md), [discovery API](discovery-api.md), [discovery UI](discovery-ui.md) |
| Xuất bản, kiểm duyệt | [publication core](publication-core.md), [runtime](publication-runtime.md), [state read](publication-state-read.md), [author Web](publication-author-web.md), [moderation core](moderation-core.md), [moderation runtime](moderation-runtime.md) |
| Quyết định sản phẩm 10/11 | [registry](product-decisions-registry.md), [rollout kho quyết định](decision-registry-rollout.md) |
| Auth | [Auth Web](auth-web.md), [auth operations](auth-operations.md), [action inventory](auth-action-inventory.md) |
| Điểm Vink | [registry](reputation.md), [SR-01](reputation-science.md) |
| Social, lưu bài, lượt xem, thông báo | [social graph](social-graph.md), [saved works](saved-works.md), [work views](work-views.md), [notifications](notifications.md) |
| Web và UI | [edge frontend](edge-frontend.md), [Web/Preview boundary](web-preview-boundary.md), [web semantics](web-semantics.md), [refresh](product-web-refresh.md), [Tailwind](tailwind-web-foundation.md), [expressions](expressions.md), [UI system](../../packages/ui-system/README.md) |
| Triển khai | [production runbook](../../infra/production/README.md), [Cloudflare edge](../../infra/production/cloudflare-edge.md), [test host](../../infra/test/README.md) |

## Chưa triển khai hoặc chưa kiểm chứng

- Production sign-in mặc định tắt; D-05 Provisional. Còn CMP auth, khôi phục tài khoản (O-11) và
  qualification production.
- AI provider thật, UI grant/nhóm kiểm duyệt, điều tra, ca/UI kiểm duyệt bình luận, route
  xóa/tombstone, bộ sưu tập, hashtag, xếp hạng, sửa bình luận.
- Autosave khi offline, bản khôi phục cục bộ, paste từ Word/Docs, IME thật trên thiết bị (WR-01/02,
  S-01). Public IndexedDB nằm sau gate tắt.
- Diễn tập vận hành dài hạn, restore production, hiệu năng trên thiết bị đích, screen reader.
