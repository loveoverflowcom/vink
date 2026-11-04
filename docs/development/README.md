# Phát triển Vink: setup, lệnh và giới hạn skeleton

**Ngày:** 30/10/2026 · **Trạng thái:** Skeleton đã triển khai và chạy được cục bộ. Đây là nền
phát triển, không phải pilot hay production. Các lựa chọn dưới đây là giả định bootstrap; quyết
định D-02–D-12 trong [decision log](../architecture/decisions.md) vẫn giữ nguyên trạng thái.

Trang này là **nguồn duy nhất cho lệnh**. Skills, README và CI dẫn tới đây; định nghĩa thật của
lệnh nằm trong [`xtask/src/main.rs`](../../xtask/src/main.rs).

[Bản đồ tính năng và đường kiểm chứng](feature-map.md) dẫn từ route, thao tác và trạng thái
người dùng tới source/assertion hiện có; đây không phải bảng kết quả chạy hay nghiệm thu.

[Ranh giới literary core](literary-domain-core.md) lần theo raw save → `DraftChange` đã validate
→ owner/version CAS, phân biệt working draft với immutable revision và public pointer.

[Bình luận hai tầng](discussion-core.md) ghi core, use case application, migration/adapter
PostgreSQL và registry D2T-INV. Vink sở hữu bình luận theo
[ADR ownership](../architecture/discussion-ownership.md) (chốt 03/11/2026). HTTP API v1 có route
đọc/gửi/gỡ/bật-tắt và authority kiểm duyệt bằng grant trong database. [UI thảo luận](../design/discussion-ui.md) hiển thị anonymous SSR và tải projection của viewer sau hydration; controller giữ draft/request theo scope. Chỉ LocalFixture hiện cho kiểm ghi cục bộ, có nhãn rõ; production trả `401`, chưa có UI kiểm duyệt hoặc auth authority.

[Đọc công khai có điều kiện](public-conditional-reads.md) (READ-01, [#52](https://github.com/loveoverflowcom/vink/issues/52))
ghi ETag mạnh trên đúng byte JSON, `private, no-cache`, thứ tự lỗi trước 304, bằng chứng và số đo.

[Biểu cảm Google Noto](expressions.md) bổ sung manifest + policy Rust + Web pilot độc lập EMO-01A;
chưa tích hợp composer/reaction/sticker persistence.

## Skeleton làm được gì

- Workspace Rust (toolchain pin 1.99.1) gồm ứng dụng Leptos SSR + Axum, bundle hydration WASM,
  worker và `xtask`.
- PostgreSQL 17 với migration có phiên bản: `principals`, `works`, `working_drafts` (bản nháp có
  owner và version), `revisions` (snapshot bất biến, có `content_hash`, trigger chặn UPDATE) và
  `publications` (con trỏ công khai, chỉ trỏ tới revision của chính tác phẩm).
- Khám phá `/` theo mật độ v4 trong [lớp đọc hiện có](../design/ui-repair-02.md) ([#9](https://github.com/loveoverflowcom/vink/issues/9)): luồng
  tối đa 12 tác phẩm mới công khai, tên tác giả/ngày UTC, điểm dừng và gợi ý bắt đầu viết;
  tìm tiêu đề/tên tác giả có/không dấu trong danh sách đã tải, giữ truy vấn/vị trí qua reader,
  reload và lịch sử URL; phân biệt loading/rỗng/không kết quả/unavailable. Không spotlight hay tương tác thiếu chủ dữ liệu. Xem
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
  Từ UIS-04 shell render bằng primitive điều hướng của UI system; Back/Forward khi Tôi mở chỉ đóng
  Tôi, route lạ (`/ban-vietnam`) là 404 và không sáng đích nào, skip link chuyển focus vào main.
- [UI System](../../packages/ui-system/README.md) UIS-01 ([#19](https://github.com/loveoverflowcom/vink/issues/19)):
  một nguồn token `packages/ui-system/shared/tokens.json` sinh CSS/Rust/Kotlin có `--check`;
  crate Leptos `vink-ui` (`UiRoot`/`ThemeScope`), thư viện Compose Multiplatform `:ui`
  (`VinkTheme`/`VinkScope`), hai preview chạy bằng renderer thật với cùng catalog ID. Web dùng
  CSS token sinh ra và kiểu theme của `vink-ui`; route/DOM/editor/session không đổi. Android/iOS
  compile khi bật; diagnostic hosts và readiness hiện hành ở UIS-06 bên dưới. UIS-02 ([#20](https://github.com/loveoverflowcom/vink/issues/20)):
  25 component nền theo họ ở cả `vink-ui` và `:ui` theo một [contract](../../packages/ui-system/shared/components.md)
  (controlled value/callback, `ActionState`, field có nhãn/gợi ý/lỗi liên kết, icon registry có
  license, avatar initials + màu theo `avatar-identity-v1` có golden vectors chung), đăng ký trong hai
  preview; app chỉ ủy quyền `initials`, chưa màn nào dùng component. UIS-03
  ([#21](https://github.com/loveoverflowcom/vink/issues/21)): [contract trạng thái](../../packages/ui-system/shared/states.md)
  theo trục (nội dung × pha có tổ hợp hợp lệ, `Problem` an toàn + intent phục hồi do caller cấp,
  chính sách vị trí/announce, `LoadingGate` thuần với đồng hồ tiêm) và component trình bày ở hai
  renderer (`ContentRegion`, `StatePanel`, `InlineNotice`/`Banner`, `Skeleton`, `LoadingIndicator`
  trong slot `status` của Button, `EdgeProgress`, `StatusBadge`, `ListEnd`, `Snackbar`, `Dialog`);
  vòng lặp chỉ chạy khi host báo đang hoạt động; [hướng dẫn thiết kế](../design/state-and-feedback.md).
  Thư viện không fetch/retry/điều hướng; `EditorSession`, `CreateState` và phân loại lỗi API vẫn ở app.
  UIS-04 ([#22](https://github.com/loveoverflowcom/vink/issues/22)): [contract điều hướng](../../packages/ui-system/shared/navigation.md)
  với `NavLayout::resolve` thuần (preference > ngữ cảnh, thanh dưới khi cửa sổ hẹp), `OverlayLayer`
  (một modal, Back đóng lớp phủ trước, đóng muộn bị bỏ) và primitive ở hai renderer
  (`AdaptiveScaffold`, `NavigationRail` có toggle thường trực, `BottomNavigation`, `TopBar`,
  `NavItem`, `NavUtility`); vectors chung Rust/Kotlin. Shell Web đã dùng; preview CMP có adapter
  chẩn đoán (back stack, deep link, chữ chưa lưu), chưa phải app native.
- UIS-05 ([#23](https://github.com/loveoverflowcom/vink/issues/23)): năm màn Web dùng shared
  controls/fields/avatar/byline/notice/dialog, composition Vink ở
  [packages/vink-patterns](../../packages/vink-patterns/README.md) nhận view props và callbacks;
  adapter DTO vẫn ở app. Hai gallery có 6 recipe cùng ID (Explore/Reader/Desk/Editor/Me/feedback),
  CMP render component native thật; community/native editor chỉ mẫu có nhãn, không fake backend.
  [Spec, mapping và bằng chứng](ui-system-adoption-evidence.md).
- UIS-06 ([#24](https://github.com/loveoverflowcom/vink/issues/24)): API admission map và scan
  style/dependency trong `verify quick`; specimens CMP chung ở `:diagnostics`, Android debug
  host và iOS simulator host độc lập. [Mở rộng UI](../../packages/ui-system/EXTENDING.md),
  [compatibility](../../packages/ui-system/shared/compatibility.md),
  [readiness và bằng chứng từng target](ui-system-qualification-evidence.md).
- API JSON v1 có contract và fixtures do server sở hữu:
  [services/vink-app/contracts/http/v1](../../services/vink-app/contracts/http/v1/README.md).
- `/health/live` (tiến trình còn phục vụ) tách khỏi `/health/ready` (database tới được và lịch
  sử migration trùng migration nhúng; thiếu là `pending_migrations`, checksum lệch/version lạ/dở
  dang là `diverged`). AI moderation được báo `not_implemented`; không còn mục Discourse (thảo luận do Vink sở hữu).
- PostgreSQL có owner tại [services/postgres](../../services/postgres/README.md): một container,
  database `vink` (App) và `vink_identity` (Identity, chưa có bảng), mỗi owner một role
  migrator và một role runtime do `provision.sh` tạo/kiểm. App chạy bằng role runtime (DML, không
  DDL); `vink-app migrate` chỉ dùng credential migrator. Pool, vòng đời migration và phân loại
  lỗi kết nối nằm ở [vink-store-postgres](../../packages/store-postgres/README.md). Ranh giới:
  [identity-postgres-boundaries](../architecture/identity-postgres-boundaries.md).
- [vink-identity](../../services/vink-identity/README.md): binary riêng (`serve`/`migrate`) trên
  database `vink_identity`, chỉ nghe Unix socket; caller là OS user kết nối (peer uid). Contract
  riêng tư [v1 draft](../../services/vink-identity/contracts/rpc/v1/README.md) trả
  `authority_not_configured` vì chưa có auth authority — production authentication
  NOT_IMPLEMENTED. App hỏi Identity qua client riêng
  ([src/identity](../../services/vink-app/src/identity/client.rs)) chỉ khi request trình
  `Authorization: Bearer` (carrier tạm thời): không credential là `401`, Identity không quyết
  được là `503 dependency_unavailable`, mọi câu trả lời khác vẫn là `401` vì chưa có binding
  subject → principal. Đọc công khai/SSR không phụ thuộc Identity. Readiness App báo
  `checks.role` (production từ chối role `privileged`) và `integrations.identity`.

## Chuẩn bị môi trường

| Công cụ | Dùng cho | Ghi chú |
| --- | --- | --- |
| rustup | mọi lệnh | `rustup toolchain install` trong repo đọc [rust-toolchain.toml](../../rust-toolchain.toml) (1.99.1, rustfmt, clippy, wasm32) |
| `wasm-bindgen` CLI | build bundle | Phải bằng phiên bản `wasm-bindgen` trong `Cargo.lock` (hiện 0.2.129): `cargo install wasm-bindgen-cli --version 0.2.129 --locked` |
| PostgreSQL 17 | dev, `verify postgres`, `verify web` | [services/postgres](../../services/postgres/README.md) chạy bằng Podman hoặc Docker; `VINK_TEST_DATABASE_URL` là superuser của cluster local/CI để tạo database/role tạm |
| `psql` 15+ | `services/postgres/provision.sh`, `verify postgres` | Ví dụ `brew install libpq` hoặc gói `postgresql-client` |
| Node.js 22+ và npm | `build`, `dev`, `verify quick`, `verify web`, `verify expressions` | Cài dependency đã khóa và bundle [runtime editor](../../apps/vink-web/editor/README.md) (cần mạng tới npm registry lần đầu hoặc khi lockfile đổi) |
| Quint 0.33.1 qua npm lockfile | `verify discussion`, `verify expressions` | Lane cài local dependency bằng `npm ci --ignore-scripts`, dùng backend TypeScript tường minh; không cần global Quint hay thêm Rust toolchain |
| Chrome/Chromium | `verify web`, `ui-preview leptos verify` | Script smoke không có npm dependency; đặt `VINK_CHROME` nếu Chrome không nằm trong `PATH` (macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`) |
| JDK 17+ | `ui-preview cmp …` | Gradle wrapper 8.14.3 (tải bản phân phối lần đầu, kiểm SHA-256); không tự tải JDK |
| Android SDK, Xcode | chỉ `VINK_UI_NATIVE_TARGETS=1` | `ANDROID_HOME` (platform 36) cho target Android; Xcode + tải Kotlin/Native 2.2.20 lần đầu cho iOS |

Thiết lập lần đầu (chi tiết, chuyển từ `infra/dev` và trường hợp `-p`:
[services/postgres](../../services/postgres/README.md)):

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

Có thể thay `podman compose` bằng `docker compose`. `cargo xtask dev` tự build và chạy migrations;
dữ liệu mẫu chỉ được nạp khi gọi `db seed`. Mở đúng `VINK_PUBLIC_ORIGIN`
(mặc định `http://127.0.0.1:3000`): lệnh ghi từ một origin khác bị trả `403`.

## Lệnh

| Lệnh | Việc thực hiện | Thoát khác 0 khi |
| --- | --- | --- |
| `cargo xtask doctor` | Kiểm toolchain, wasm target, wasm-bindgen, Node 22+/npm, `DATABASE_URL` và `VINK_MIGRATE_DATABASE_URL` (ẩn credential, thử kết nối); `VINK_TEST_DATABASE_URL`, `psql` và Chrome là cảnh báo | Thiếu điều kiện cho `dev` |
| `cargo xtask build [--release]` | Build `vink-app`, `vink-worker`, `vink-identity` và `target/site/pkg/{vink.js, vink_bg.wasm, snippets/, vink-editor.js, vink.css, fonts/, brand/}` (`vink.css` = `tokens.css` sinh ra + `apps/vink-web/style/vink.css`, sau khi `ui-tokens --check` đạt); `npm ci --ignore-scripts` cho runtime editor khi lockfile đổi | Build, wasm-bindgen, npm hoặc type-check/bundle editor lỗi |
| `cargo xtask db migrate` | Migrate App bằng `VINK_MIGRATE_DATABASE_URL` (không đọc `DATABASE_URL`), rồi Identity bằng `VINK_IDENTITY_MIGRATE_DATABASE_URL` nếu có | Thiếu biến, không kết nối được, thiếu quyền, lịch sử diverged hoặc migration lỗi |
| `cargo xtask db seed` | Nạp dữ liệu tổng hợp có nhãn `[Mẫu]` (chỉ `VINK_PROFILE=local`, idempotent) | Profile khác `local` hoặc chưa migrate |
| `cargo xtask dev` | Build, migrate các owner, chạy Identity (nếu có `VINK_IDENTITY_SOCKET`) và chờ ready, rồi chạy server ở `VINK_BIND` | Bất kỳ bước nào lỗi, Identity không ready trong 20 s |
| `cargo xtask ui-tokens [--check]` | Validate `packages/ui-system/shared/{tokens.json, preview-catalog.json, icons.json, fixtures/}` (type, đơn vị, alias treo/vòng/sai type, đủ light/dark, tương phản khai báo; icon: commit pin, license, SHA-256 upstream dựng lại; identity: tone trong khoảng, ảnh base64; states: tổ hợp vùng, intent, placement, notice và ca `LoadingGate` khớp chính sách token) rồi ghi `tokens.css`, `tokens.rs`, `catalog.rs`, `icons.rs`, `identity_vectors.rs`, `state_vectors.rs`, `VinkTokens.kt`, `VinkCatalog.kt`, `VinkIcons.kt`, `VinkIdentityVectors.kt`, `VinkStateVectors.kt`; `--check` không ghi | Nguồn không hợp lệ; với `--check`, file sinh ra thiếu hoặc lệch |
| `cargo xtask ui-preview leptos build\|run\|verify` | Bundle preview Leptos (`csr`) vào `target/ui-preview/leptos`; `run` phục vụ loopback ở `VINK_UI_PREVIEW_BIND` (mặc định `127.0.0.1:3019`); `verify` chạy [preview-smoke](../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs) trong Chrome headless và ghi ảnh + `contexts.json` | Build, wasm-bindgen, Node/Chrome thiếu hoặc một assertion lỗi |
| `VINK_UI_FOCUS_CAPTURE=1 cargo xtask ui-preview leptos verify` | Giữ smoke/matrix raw hiện có; thêm 36 capture specimen chọn lọc (6 entry × light/dark × 390/1440 text100, 390 text200), `focus-manifest.json` và PNG `--focus[--partN]`. Clip theo geometry, padding 12 CSS px; specimen cao chia tile cân bằng, tối đa 4096px và không quá chiều cao viewport (mặc định 900px), không thu ảnh. Cuộn trong viewport thật, không dùng CDP resize ngầm. Kiểm PNG/bounds/ảnh không trắng, phục hồi viewport/scroll/focus và source fingerprint | Cùng prerequisite với preview; identity/theme/clip/PNG/restore hoặc source đổi trong lúc chụp |
| `cargo xtask ui-preview cmp build\|run\|verify` | Gradle trong `packages/ui-system/cmp`: compile `:ui`/`:preview` (desktop); `run` mở cửa sổ; `verify` chạy test common + desktop và ghi ảnh vào `packages/ui-system/cmp/preview/build/captures`. `VINK_UI_NATIVE_TARGETS=1` thêm Android/iOS | JDK/Gradle thiếu, compile hoặc test lỗi |
| `cargo xtask verify quick` | `ui-tokens --check`, `fmt`, clippy server (workspace, `ssr`), clippy browser (`hydrate`, wasm32), clippy preview UI (`vink-ui-preview`, `csr`, wasm32), doctests type-boundary của literary core (`vink-app`), unit/contract tests không cần database (gồm `vink-ui`, `vink-ui-preview`, generator), kiểm skills/link/ranh giới module (UI system không phụ thuộc service/router/storage; Kotlin `commonMain` không import API nền tảng), font/logo asset theo `provenance.json` và phiên bản pin của runtime editor | Một bước lỗi |
| `cargo xtask verify postgres` | Test vòng đời migration của `vink-store-postgres` và test App trên PostgreSQL thật; config sai thoát 2; `provision.sh` thật với tên tạm (DB cũ được nhận quyền, DB mới, Identity, chạy lại, ca âm), migrate/seed bằng role tách, thử DDL/kết nối chéo, worker start/SIGTERM bằng role runtime | Thiếu `VINK_TEST_DATABASE_URL`, PostgreSQL hoặc `psql` 15+, hoặc một bước lỗi |
| `cargo xtask verify discussion` | Cài Quint pinned, typecheck/scenarios, simulation seed cố định, mutation counterexample, drift fixture và replay vào Rust production | Thiếu Node/npm, sai tool version, model/test/drift hoặc negative control không phát hiện lỗi |
| `cargo xtask verify web` | Build, database tạm `vink_verify_web` (tạo bằng `VINK_TEST_DATABASE_URL`), server thật, Chrome headless, SIGTERM, restart | Thiếu PostgreSQL/Node/Chrome/wasm-bindgen hoặc một bước lỗi |
| `cargo xtask verify expressions` | Noto manifest/hash/drift, Quint lifecycle simulation/negative control và Rust replay (xem mục EMO-01A) | Thiếu dependency hoặc bất kỳ kiểm nào lỗi |
| `cargo xtask verify all` | `quick`, `discussion`, `expressions`, `postgres`, `web` theo thứ tự | Lane đầu tiên lỗi |

Mỗi lane in bảng `PASS`/`FAIL`; bước sau bước lỗi là `NOT_RUN`. Thiếu dependency là `FAIL`,
không bao giờ là `PASS`. `xtask` đọc `.env` cho các tiến trình nó chạy (biến môi trường thật được
ưu tiên); bản thân binary chỉ đọc biến môi trường. `xtask` không cài toolchain, không bật
container, không push hay deploy. CI ([.github/workflows/ci.yml](../../.github/workflows/ci.yml))
chuẩn bị Rust, wasm-bindgen, Node và PostgreSQL rồi gọi các lane tương ứng; job `discussion`
chạy cùng runner cấu trúc như local. Job Web còn chạy preview Leptos với
`VINK_UI_FOCUS_CAPTURE=1`; job `ui-cmp` dùng JDK 17 để chạy Desktop tests/captures.
Các job giữ bundle qua artifact 14 ngày theo workflow. `ui-cmp-macos` chạy cùng lane và JDK 17
trên macOS để giữ regression font Desktop; Linux `ui-cmp` vẫn là gate của lỗi font
[PR 08](../design/ui-repair-08.md). Android/iOS diagnostic vẫn cần
môi trường native riêng. Không có lane Kani hoặc generic behavior. Cấu hình CI không tự
là bằng chứng một run đã PASS.

Binary `vink-identity` nhận `serve` (mặc định) và `migrate`, cùng quy ước mã thoát; cấu hình
`VINK_IDENTITY_*` ở [README của service](../../services/vink-identity/README.md). Khi `.env` có
`VINK_IDENTITY_SOCKET`, `cargo xtask db migrate` migrate thêm Identity và `cargo xtask dev` chạy
Identity, chờ readiness trên socket rồi mới chạy App; dừng App thì Identity được SIGTERM. Với lệnh
local, xtask đặt `VINK_IDENTITY_CALLER_UID`/`VINK_IDENTITY_SERVER_UID` bằng uid của bạn nếu chưa đặt.

Binary `vink-app` nhận `serve` (mặc định), `migrate`, `dev-seed` và
`moderator grant <principal-uuid> <ghi chú> | revoke <principal-uuid> | list`: lệnh vận hành duy
nhất cấp/thu hồi quyền kiểm duyệt bình luận (dùng `DATABASE_URL`; không có route HTTP nào làm việc
này, quyền sở hữu tác phẩm không bao giờ kéo theo quyền kiểm duyệt). Mã thoát: `0` thành công,
`1` lỗi runtime (kể cả credential migrate thiếu quyền — không thử lại bằng role khác), `2` cấu hình
hoặc cách gọi sai. `vink-worker` dùng cùng cấu hình với `serve`.

## Cấu hình

| Biến | Mặc định | Ý nghĩa |
| --- | --- | --- |
| `DATABASE_URL` | bắt buộc cho `serve`, `dev-seed`, worker | URL PostgreSQL của role runtime; không bao giờ được in ra |
| `VINK_MIGRATE_DATABASE_URL` | bắt buộc cho `migrate` | URL của role migrator; `migrate` chỉ đọc biến này, không fallback sang `DATABASE_URL` |
| `VINK_TEST_DATABASE_URL` | chỉ `xtask verify` | Superuser của cluster local/CI để tạo database/role tạm; binary không đọc, không dùng cho app |
| `VINK_IDENTITY_SOCKET` | không có | Socket của `vink-identity`; App hỏi Identity cho credential được trình, Identity nghe trên đó |
| `VINK_IDENTITY_SERVER_UID` | bắt buộc khi có socket | UID phải đang phục vụ socket; App không tin socket của user khác |
| `VINK_PG_BIN` | `PATH` | Thư mục `pg_dump`/`pg_restore` cho bước backup/restore của `verify postgres` (phải ≥ major của server) |
| `VINK_PROFILE` | `production` | `local` mở seed và identity fixture; mặc định đóng |
| `VINK_BIND` | `127.0.0.1:3000` | Địa chỉ lắng nghe |
| `VINK_PUBLIC_ORIGIN` | `http://{VINK_BIND}` khi `local`; bắt buộc ở `production` | Origin cho canonical URL và kiểm `Origin` của PUT/POST |
| `VINK_SITE_ROOT` | `target/site` | Thư mục chứa `pkg/` |
| `VINK_DEV_PRINCIPAL` | không có | UUID principal fixture; chỉ hợp lệ khi `local` và `VINK_BIND` là loopback |
| `VINK_LOG_FORMAT` | `text` | `json` cho log có cấu trúc một dòng một object; `RUST_LOG` chỉnh filter |

**Identity fixture không phải đăng nhập.** Khi bật, mọi request đến từ loopback hành động như
`VINK_DEV_PRINCIPAL`. Ngoài fixture, server không có nguồn xác thực nào nên đường riêng tư trả
`401`. Fixture không chứng minh ACC-01, ACC-02 hay S-03; không tự xây hệ mật khẩu để lấp chỗ này.

## Diagnostic UI native (UIS-06)

Chạy từ root; hai host chỉ dùng dữ liệu mẫu, không service/đăng nhập/save backend.
Web/JVM không cần Android SDK/Xcode; native modules chỉ configure khi gọi target tương ứng
hoặc bật `VINK_UI_NATIVE_TARGETS=1`. Không có app native production.

```sh
export JAVA_HOME=$(/usr/libexec/java_home -v 17)
export ANDROID_HOME="$HOME/Library/Android/sdk"
# Khởi động một emulator bằng Android Studio hoặc emulator CLI trước verify/run.
cargo xtask ui-preview android build
cargo xtask ui-preview android verify
VINK_UI_ENTRY=field.text-area VINK_UI_SCENARIO=multiline VINK_UI_THEME=dark cargo xtask ui-preview android run

# macOS arm64, Xcode + iOS simulator đã cài, XcodeGen (project.yml là nguồn).
xcrun simctl list devices
export VINK_UI_IOS_DEVICE=SIMULATOR_UDID
cargo xtask ui-preview ios build
cargo xtask ui-preview ios verify
VINK_UI_ENTRY=screen.reader VINK_UI_SCENARIO=long VINK_UI_THEME=dark cargo xtask ui-preview ios run
```

`ANDROID_SERIAL` chọn một emulator/device cụ thể. `VINK_UI_ENTRY`, `VINK_UI_SCENARIO`,
`VINK_UI_THEME=light|dark|system`, `VINK_UI_MOTION=reduce|system`, `VINK_UI_SCALE=100|200`
chọn replay khi `run`. Native host từ chối entry/scenario/tùy chọn lạ.
Launch arguments trực tiếp iOS/Desktop: `--entry=field.text-field --scenario=default --theme=dark`;
Android dùng intent extras cùng tên. Web dùng `#entry/scenario`, các preference do controls giữ.
Clock/seed cố định do fixtures chung sở hữu, OS text scale/density/insets do máy thực sở hữu.

Android `verify` gọi `:androidDiagnostic:connectedDebugAndroidTest` thật; iOS gọi framework
`:diagnostics:linkDebugFrameworkIosSimulatorArm64`, XcodeGen rồi `xcodebuild test` scheme
`VinkUIDiagnostic` trên UDID đã chọn (không tự clone simulator). Thư viện iOS device compile
vẫn qua `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp build`; iOS diagnostic host hiện
chỉ simulator arm64, chưa có signing/distribution cho device.

Output: Android `androidDiagnostic/build/outputs/androidTest-results/`, iOS
`iosDiagnostic/build/DerivedData/Logs/Test/*.xcresult`, CMP `preview/build/captures/`.
Chụp màn native bằng `adb exec-out screencap -p > PATH.png` hoặc
`xcrun simctl io "$VINK_UI_IOS_DEVICE" screenshot PATH.png`, rồi mở ảnh và ghi context/hash.
Đây không phải screenshot baseline tự được bless. Native dùng font hệ thống; pipeline branded
TTF/OTF, OS IME tiếng Việt, TalkBack/VoiceOver và device thật cần evidence riêng.

## Cấu trúc và chiều phụ thuộc

```text
apps/vink-web/            crate vink-web: view Leptos, route, DTO riêng của web, CSS tokens
  src/shell.rs            đích Vink (Destination), lớp phủ Tôi, chính sách responsive; render bằng
                          primitive UIS-04, dùng NavigationGuard cho chuyển trang
  src/appearance.rs       theme/rail local, bootstrap trước CSS, initials thuần có fixture tiếng Việt
  src/pages/me.rs         /toi: thông tin tài khoản chưa có, chọn giao diện
  src/contract.rs         biểu diễn API v1 của consumer, kiểm bằng fixtures của server
  src/api.rs              ssr: đọc qua PublicReads; hydrate: fetch /api/v1
  src/server.rs           (ssr) trait PublicReads do composition root cung cấp
  src/draft_list.rs       tìm/sort nháp đã tải và trạng thái tạo/đối chiếu kết quả, thuần
  src/public_list.rs      lọc title/author của public cards, dựng link ngữ cảnh nội bộ, thuần
  src/search.rs           comparison key tiếng Việt dùng chung, không sửa chữ gốc
  src/desk_state.rs       máy trạng thái lưu nháp và rời editor (EditorSession, LeaveGuard), thuần
  src/document.rs         chữ nguồn ↔ document v1, DocumentView, export .txt
  src/expressions.rs      static SSR + isolated Google Noto pilot, Rust policy bridge
  expressions/            dotLottie 0.82.1, runtime hashes/licenses, lazy Web effects
  src/editor.rs           (hydrate) cầu wasm-bindgen tới CodeMirror, BodySource::snapshot
  src/navigation.rs       NavigationGuard dùng chung cho link, back/forward, điều hướng bằng code;
                          hỏi lớp phủ shell trước (Back đóng Tôi, điều hướng thật đóng rồi đi tiếp)
  editor/                 runtime CodeMirror 6 (TypeScript, npm pin, esbuild) → /pkg/vink-editor.js
  style/vink.css          component web `vk-*`; build ghép tokens.css (sinh) + vink-ui.css + vink.css
  assets/fonts/           WOFF2 tự host, OFL, provenance.json (phục vụ tại /pkg/fonts/)
  assets/brand/           Logo selected #34 SVG/PNG, manifest, provenance (phục vụ tại /pkg/brand/)
  e2e/browser-smoke.mjs   kiểm trình duyệt qua Chrome DevTools Protocol
packages/ui-system/       UI System (README owner map): shared/ tokens.json + catalog + fixtures;
  leptos/                 crate vink-ui (tokens/catalog sinh ra, UiRoot, ThemeScope, component,
                          state/feedback, navigation) + preview/
                          crate vink-ui-preview (CSR, Chrome smoke)
  cmp/                    Gradle: :ui (Compose Multiplatform commonMain), :preview (Desktop)
packages/store-postgres/  crate vink-store-postgres: pool, vòng đời migration, trạng thái schema,
                          phân loại lỗi kết nối; không biết database/bảng nghiệp vụ
services/postgres/        container PostgreSQL (compose + overlay local), provision.sh, runbook
services/vink-identity/   crate vink-identity: bin riêng, Unix socket, contract rpc/v1, migrations
                          riêng; không phụ thuộc vink-app/vink-web và không đọc bảng App
services/vink-app/        crate vink-app: composition root, bin vink-app và vink-worker
  src/domain/             luật thuần: document, title, version, revision hash
  src/application/        use case và port (DraftStore, PublicationStore)
  src/persistence/        adapter PostgreSQL (SQLx), migrator nhúng, wrapper vink-store-postgres
  src/http/               Axum: API v1, health, SSR host, Origin check
  src/identity.rs         seam xác định principal phía server
  src/config.rs           cấu hình chung, kiểm hợp lệ, ẩn secret
  src/dev_seed.rs         dữ liệu tổng hợp cho local
  migrations/             migration có phiên bản
  contracts/http/v1/      contract API và fixtures (owner: server)
  tests/postgres.rs       test PostgreSQL thật và HTTP qua router
xtask/                    command owner cho local và CI; src/provision.rs kiểm provisioning;
                          src/ui_tokens.rs generator/validator, src/ui_states.rs vectors trạng
                          thái, src/ui_preview.rs hai preview
infra/dev/README.md       chỉ dẫn chuyển sang services/postgres
```

Chiều phụ thuộc: `http`, `persistence` → `application` → `domain`; `persistence` →
`vink-store-postgres`, package không phụ thuộc service/web/framework HTTP. `vink-app` phụ thuộc
`vink-web` (feature `ssr`) và cung cấp `PublicReads`; SSR gọi application trong tiến trình, không
vòng HTTP về chính server. `vink-web` không phụ thuộc `vink-app`, SQLx hay secrets; build
`hydrate` chỉ chứa view, DTO và fetch client. `verify quick` kiểm các import bị cấm trong
`domain`/`application`, dependency của `vink-web` và của `packages/*`, việc `vink-app` và
`vink-identity` không import nhau, SQL của Identity không chạm bảng App, và file được track không
có private key hay URL PostgreSQL mang password không phải giá trị local dùng một lần. Hai feature `ssr` và `hydrate` là hai build
riêng; không dùng `--all-features`.

## Bằng chứng theo ranh giới

| Lane | Chứng minh | Không chứng minh |
| --- | --- | --- |
| `quick` | Tìm tiêu đề NFC/NFD/Đ/đ và guard tạo/đối chiếu kết quả; luật document/title/version/hash; config; máy trạng thái lưu (phản hồi cũ, một request đang bay, xung đột); fixtures provider/consumer; outage PostgreSQL → readiness `503`, API `dependency_unavailable`, Khám phá/reader `503`; private route `401` khi không có authority | Transaction PostgreSQL, trình duyệt |
| `discussion` | Cấu trúc root/flat reply D2T-01 (replay qua `DiscussionThread`); vòng đời bình luận, chế độ bật/tắt và authority kiểm duyệt D2T-02 (replay qua `decide_submission`, `authorize_*`, `change_mode`, `ModerationAction::apply`, `view`); Quint simulation, negative control, fixture drift | Không Kani/exhaustive model checking, auth authority, khóa/transaction/API/UI; model một thread/một tác phẩm không chứng minh work/tenant binding |
| `postgres` | Vòng đời migration dùng chung: pending/current/diverged (checksum, version lạ, dở dang), chạy lại no-op, chỉ owner ghi lịch sử, phiên bị ngắt là unavailable; provisioning thật: DB cũ do một superuser tạo được nhận quyền mà dữ liệu/ID/Unicode/con trỏ/lịch sử không đổi, chạy lại không đổi password, `--check` bắt quyền thừa; runtime không DDL/TEMP/ghi lịch sử, không kết nối DB owner khác khi chỉ đổi dbname; migrate bằng runtime bị từ chối (thoát 1); migrator không tạo DB/role; migrate database mới; lưu có điều kiện; 8 lần lưu đồng thời chỉ 1 thắng; actor khác không đọc/ghi/liệt kê được; NFC, dòng, khổ, thụt lề qua database; đọc công khai trả snapshot chứ không trả nháp; revision không UPDATE được; con trỏ không trỏ sang tác phẩm khác; HTTP 401/403/404/409/422; HTML reader không chứa nháp; config sai thoát 2; worker start/stop bằng role runtime; Identity: migrations/readiness thật, binary thật trên Unix socket với role đã provision (config sai thoát 2, migrate runtime bị từ chối, migrate hai lần, migrator/App database bị từ chối, contract 200/501/400/413/404/503, uid khác và thiếu caller là 401, header giả không đổi kết quả, role migrator là `privileged` 503, mất kết nối tự phục hồi, sai database/schema là not ready, header gửi dở bị đóng và không giữ được SIGTERM, socket cũ/instance thứ hai, log không lộ credential); App + Identity thật cùng chạy: không credential `401`, credential `401` (chưa authority), Identity dừng thì chỉ request có credential `503`, đọc công khai/SSR vẫn `200`, restart độc lập, App bằng role migrator `503 privileged`, socket do user khác phục vụ không được tin; backup/restore từng owner bằng `pg_dump`/`pg_restore` vào database khác khớp dữ liệu và lịch sử | Hành vi trình duyệt, IME thật; Compose/container (chạy thủ công, xem [bằng chứng](identity-postgres-verification.md)); PostgreSQL managed không có superuser |
| `web` | Bundle `/pkg/` không chứa URL database, tên biến credential hay password local; Khám phá/reader: public HTML và metadata trước JS, save riêng không đổi publication, 404/503, link thật, list không N+1, states riêng, reflow 320–1440px/root text 100/200% và contrast hai theme; Dashboard: resume/list không N+1, fixtures 100 row/rỗng/lỗi/401, tìm/sort/retry, POST commit thật rồi mất ack và GET đối chiếu, phản hồi tạo muộn không điều hướng; HTML có nội dung trước JavaScript; hydration không lỗi console; shell 220/96, thứ tự rail/bar, theme + reload + hệ thống + storage denied, overlay bàn phím/Escape/outside/resize/restore-focus, bottom clearance 320px/chữ 200%; trang đọc tải Be Vietnam Pro và Literata tự host, file font trả `font/woff2`, không tải runtime editor; điều hướng client không tải lại trang; CodeMirror mount từ `/pkg/vink-editor.js`, không còn textarea; tạo, gõ, lưu (nút, Ctrl+S), reload; xung đột với tab khác giữ nội dung và cho lưu đè có chủ ý; cùng một EditorView giữ selection/undo/redo qua đổi theme, xem trước và hộp thoại; brand/link/back bị giữ khi chưa lưu, Escape ở lại; lưu rồi rời chỉ sau ack; hủy khi đang lưu thì ack muộn không điều hướng; lỗi lưu → tải bản đang viết → rời mà không lưu, ack muộn không điều hướng hay đánh dấu nháp khác đã lưu; nội dung tệp `.txt`; SIGTERM thoát 0; dữ liệu còn sau restart; điều hướng (UIS-04): toggle có tên/`aria-expanded`, ≥44px không bị che khi thu, link thật không phải tab, skip link focus main, click phím bổ trợ để trình duyệt xử lý, route lạ 404 không sáng đích, bấm dồn dập chỉ còn đích cuối, Back khi Tôi mở chỉ đóng Tôi rồi Back sau mới rời | IME hệ điều hành thật (chỉ `Input.insertText`/`imeSetComposition`), bàn phím/thiết bị mobile thật, screen reader, nhiều trình duyệt, paste từ Word/Docs |
| `ui-preview leptos verify` | Catalog ID đúng thứ tự, link kịch bản chính xác, không ID trùng và mọi `aria-describedby` trỏ tới phần tử có thật, Sáng/Tối/Hệ thống và đổi OS không reload, scope lồng nhau (cả Hệ thống trong Tối, recipe giải theo scope), mật độ gọn giữ 44px, vòng lặp chạy/dừng theo scope/OS/preference; component: click/Enter/Space gọi đúng một lần, disabled/pending không lần nào, pending giữ bề rộng/focus, Button mặc định không gửi form, focus ring 3px, target ≥48px, link tab mới có `rel`, URL script không thành link/ảnh, chip Space/Tab, caret giữa chữ và composition (`imeSetComposition`) giữ qua controlled value, nhãn/gợi ý/lỗi/required/read-only/disabled, giá trị `0`, Enter tìm và nút xóa trả focus, checkbox một target, radio phím mũi tên, ảnh AVIF hỏng/URL lạ → initials, màu avatar theo key khớp golden vectors khi đảo thứ tự, nhóm không bịa số, byline đọc tên một lần, tương phản chữ đã resolve ≥4.5 hai theme (control disabled được miễn); tìm không dấu, không tràn ngang 320–1440px ở chữ 100/200%, font tự host tải được, không lỗi console; trạng thái (UIS-03): vùng/intent/announce, snackbar theo đồng hồ tiêm, 8 recipe playground (không chỉ báo trước 300ms, retry một lần giữ chữ, refresh giữ focus/cuộn, huỷ trung tính, uncertain/conflict không saved hay replay, hết phiên giữ chữ), dialog focus/Tab/Escape/bấm ngoài/trả focus, vòng lặp dừng khi giảm/không hoạt động/tab ẩn thật; điều hướng (UIS-04): thứ tự, link thật, dispatch một lần theo key, reader thuộc Khám phá, host cấp đích, toggle 96→220→96 bằng chuột/Enter/Space giữ trang/chữ/selection, tooltip ngoài rail, skip link bị clip tới khi focus, một modal, Back đóng lớp phủ trước, Escape trả focus; link của mọi kịch bản trong catalog hiện hành | Chrome khác, thiết bị thật, screen reader, IME hệ điều hành thật, chất lượng thị giác (ảnh phải được người mở) |
| `ui-preview cmp verify` | Logic common trên JVM (parse, resolve theme, override, tương phản, motion, line-height sp; identity golden vectors, allow-list link/ảnh, nhóm không bịa số); precedence scope trong composition; preview: đăng ký catalog, mọi kịch bản render, control đổi theme + nhật ký, vòng lặp dừng khi giảm, mật độ gọn ≥44, tìm không dấu, WOFF2 đọc được qua Skia; component: nút gọi một lần, disabled/pending không gọi, pending giữ node enabled + `stateDescription`, text field giữ selection qua `TextFieldValue`, nhãn/lỗi trong semantics, checkbox/chip một lần, avatar có tên/ẩn trong byline, AVIF hỏng → `Failed`; trạng thái (UIS-03): vectors chung (cả iOS simulator khi bật native), reducer bỏ double-click/ack muộn, gate chờ độ trễ trên đồng hồ thủ công, retry một lần giữ input, conflict/uncertain không saved hay gửi lại tự động, giảm/không hoạt động/ẩn không vòng lặp, dialog focus an toàn + trả focus, snackbar theo thời lượng, chỉ hành động hợp lệ; điều hướng (UIS-04): vectors chung (cả iOS simulator khi bật native), deep link round-trip và từ chối link ngoài, Back đóng lớp phủ trước rồi pop, chữ chưa lưu giữ Back/điều hướng, đổi khung không remount, toggle giữ selection, thứ tự thanh dưới, một modal; ảnh off-screen theo catalog và các fixture có chủ đích | Android/iOS runtime và lifecycle, predictive back/swipe-back, IME, TalkBack/VoiceOver, cửa sổ desktop thật (ảnh là render off-screen), hover |

Không lane nào chứng minh AI moderation, xác thực, publish/withdraw, hiệu năng hay triển khai.
Chỉ có model Quint cấu trúc nhỏ của D2T-01; vòng đời bình luận (D2T-02a) có test domain trong
`quick` và test PostgreSQL thật trong `postgres`, chưa có model, HTTP hay browser. Draft CAS vẫn có boundary tests PostgreSQL riêng.

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
| Một instance, database và role riêng cho App/Identity; provisioning bằng `psql` ([services/postgres](../../services/postgres/README.md)) | Runtime không DDL, Identity không đọc được từ App; vận hành nhẹ ở quy mô hiện tại | Cần tách instance/failure domain, PostgreSQL managed không có superuser, hoặc chọn auth authority (D-05) |
| PostgreSQL 17.11, Rust 1.99.1, Leptos 0.8.22, SQLx 0.9.0, Axum 0.8.9 | Bản ổn định đã kiểm tương thích ngày 30/10/2026, khóa trong `Cargo.lock` | Nâng cấp có chủ đích, chạy lại cả ba lane |
| Font tự host: Be Vietnam Pro 400/600 và Literata variable, WOFF2 subset Latin + tiếng Việt ([assets/fonts](../../apps/vink-web/assets/fonts/README.md)) | Bộ thiết kế Lavender chọn hai family OFL này; 332 KB thay vì 1,37 MB TTF; nguồn, hash và license nằm trong repo | Cần weight/italic/script mới, hoặc chọn chính sách cache/CDN production |

## Chưa triển khai hoặc chưa kiểm chứng

- Đăng nhập, session, tài khoản thật (D-05, S-03, ACC-01/ACC-02). `vink-identity` chỉ có ranh giới
  process/database/contract; không authority, không mapping subject → principal, App chưa gọi nó.
- Gửi duyệt, moderation người/AI, publish intent, publish/withdraw, curation, Tạp Chí
  (PUB-01–PUB-03, MOD-01–MOD-06, ED-02). Chỉ seed local tạo `publications`.
- UI bình luận, route và authority kiểm duyệt bình luận, báo cáo/khiếu nại, thông báo, outbox,
  inbox, job, reconciliation (D-08, S-02, S-04). Worker chưa có job nào. Discourse không còn trong
  phạm vi (Vink sở hữu bình luận).
- Autosave, bản khôi phục cục bộ, paste từ Word/Docs, IME thật trên thiết bị (WR-01/WR-02, S-01).
  Undo/redo và composition mô phỏng đã có trong editor CodeMirror, chưa phải bằng chứng IME hệ
  điều hành.
- Export ngoài `.txt` (WR-05 mới có bản văn bản thuần), nhiều chương (WR-04), tìm toàn bộ thư viện
  công khai/tags (UX-02; hiện chỉ tìm title/author trong tối đa 12 public cards đã tải),
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
4. **Bình luận:** authority kiểm duyệt + hàng đợi duyệt, model Quint vòng đời, rồi UI hai tầng
   (#45) trên API v1 đã có; xem [registry](discussion-core.md).
5. **S-05 AI + vận hành:** bộ mẫu tiếng Việt, schema output và job lease trong worker; giữ
   AI ở chế độ tư vấn cho tới khi MOD-06 có bằng chứng.

## Biểu cảm Google Noto (EMO-01A)

Pilot có nhãn Mẫu tại `/thu-nghiem/bieu-cam`, không cần principal hoặc ghi database. Catalog
16 asset, nguồn/hash/license, policy production Rust và traceability ở
[expressions](expressions.md); [manifest owner](../../packages/expressions/README.md).
Build cài npm lockfile riêng ở `apps/vink-web/expressions`, self-host controller/Software JS,
WASM và asset có hash dưới `/pkg/expressions/`. Reader/Explore không import animation engine.

```sh
node packages/expressions/generate.mjs --check
node --test packages/expressions/validate.test.mjs
cargo xtask verify expressions
cargo xtask verify quick
cargo xtask verify web
```

`verify expressions` cài Quint **0.33.1** từ lockfile owner-local, typecheck canonical model,
simulation TypeScript (seed 25, 1000 × 40), scenario/trace drift, negative control callback và
replay input qua Rust production. Không sửa model thảo luận; `verify discussion` vẫn là lane
riêng. Muốn cập nhật output sau review nguồn dùng `node packages/expressions/generate.mjs --write`;
fixture model dùng `node packages/expressions/behavior/verify.mjs --write` sau khi dependency được cài.

Kani là kiểm riêng, **không bị tự cài hoặc tự đổi toolchain bởi xtask**. Pin `cargo-kani 0.67.0`
và compiler bundled `nightly-2025-11-21`; từ repo root:

```sh
cargo kani --version
cargo kani --manifest-path packages/expressions/kani/Cargo.toml
```

Manifest compatibility trỏ đúng source production, không đổi toolchain 1.99.1 của workspace.
Ba harness: 6 event/3 player/cap 0..3 (unwind 12 + covers), callback/facts (8), 4 Unicode input
đầy đủ (40). Giữ unwinding assertions. Toolchain thiếu phải ghi NOT_RUN, không thay bằng unit test.

`web` gọi `expressionPilot` trong `e2e/expressions-smoke.mjs`: no-JS/SSR/hydration, network tĩnh,
keyboard/touch, one-shot, cap 2/idempotency, delayed-load/unmount, motion/visibility, missing
module/WASM/JSON/poster, source Unicode, responsive/theme/text scale và same-origin fallback.
Một test inject CSP trên response (`worker-src 'none'`, connect self, wasm-unsafe-eval); không
phải production CSP của toàn app. Browser tạo PNG qua capture helper chuẩn và context JSON.
Native chỉ generated contract/Unicode vectors trong module `:ui`; kiểm JVM bằng:

```sh
cd packages/ui-system/cmp
./gradlew :ui:desktopTest --tests vink.ui.NotoExpressionTest
```

Không suy Android/iOS runtime, IME OS, screen reader, heap GC hoặc backend benchmark từ các kiểm
này. Những acceptance còn lại của #25/#43/#45 được giữ mở trong [scope](expressions.md).
