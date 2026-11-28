# Chạy và kiểm đúng phạm vi (#184)

Trên máy Mac nhiều agent, đặt lệnh nặng được hỗ trợ sau
`./tools/local-budget run --` để giữ slot từ trước Cargo bootstrap. Xem
[ngân sách local](local-resource-budget.md) để kiểm cấu hình, phạm vi và cách dừng.

Đây là hợp đồng tooling local, không thay luật sản phẩm, auth authority hay trạng thái ADR.
`build` tạo artifact; `dev` giữ tiến trình để thử tay; `verify` chạy kiểm hữu hạn. PostgreSQL luôn
được attach sau khi [provision](../../services/postgres/README.md), không lệnh nào tự bật container.

## Chọn theo việc muốn làm

Tất cả ví dụ chạy ở repo root. Dùng `cargo xtask --help` để xem cú pháp thực thi.

| Việc | Lệnh | Prerequisite ngoài Rust | Tiến trình / dữ liệu |
| --- | --- | --- | --- |
| Static tài liệu hoặc token | `cargo xtask verify static docs` / `cargo xtask verify static tokens` | — | Chỉ checker nội bộ tương ứng |
| Compile một package/target | `cargo xtask verify check app --target api` | WASM target chỉ cho `hydrate` hoặc `csr` | Cargo check, không link binary package hay chạy test |
| PostgreSQL một owner | `cargo xtask verify postgres store --filter fresh_database` | Cluster test qua `VINK_TEST_DATABASE_URL` | Một test target, lock scratch, fixture thật |
| PostgreSQL lifecycle | `cargo xtask verify postgres lifecycle` | Cluster test, `psql` 15+, `pg_dump`/`pg_restore` | Binary App/Identity/Notifications, config rejection, provision, role fences, backup/restore/recovery |
| Unit/contract một owner | `cargo xtask verify unit app --filter publication` | Dependency biên dịch thật của package | Test binary, không server/DB/browser; zero match là FAIL |
| Một integration target | `cargo xtask verify unit app --test ssr_boundary` | Như unit | Đúng target đó; `postgres` vẫn qua `verify postgres OWNER` |
| Chọn kiểm theo diff | `cargo xtask verify changed --plan` rồi `cargo xtask verify changed --expect PLAN_ID` | Theo từng check đã chọn | Plan chỉ đọc; khi chạy, xtask kiểm drift trước mỗi check |
| Build FE standalone | `cargo xtask build spa --release` | wasm-bindgen, wasm target, Node/npm | [Index CSR và asset theo revision](edge-frontend.md), độc lập App/DB |
| Build API | `cargo xtask build api` | Không tool frontend | Chỉ binary `vink-app` |
| API công khai | `cargo xtask dev api` | App DB đã migrate, role runtime | App với health + API, ẩn danh |
| API với đăng nhập thật | `cargo xtask dev api --auth identity` | App/Identity DB đã migrate, runtime roles | Identity native + App; local mail outbox, không gửi ra ngoài |
| Worker | `cargo xtask dev worker` | App DB đã migrate, runtime role | Chỉ `vink-worker`; outbox/retention, evaluation chạy mock `approve` mặc định (#536), `VINK_EVALUATOR=manual` để chuyển người duyệt |
| Identity riêng | `cargo xtask dev identity` | Identity DB đã migrate, runtime role | Chỉ Identity native/socket/local outbox |
| Web thử tay | `cargo xtask dev web --auth identity` | App/Identity DB, wasm target/bindgen, Node/npm | App SSR + assets và Identity; không Chrome E2E |
| Web bằng dữ liệu mẫu | `cargo xtask dev web --auth fixture --seed` | App DB đã migrate; frontend tools | Principal từ `VINK_DEV_PRINCIPAL`; seed rõ ràng, không phải auth thật |
| Preview Leptos | `cargo xtask dev preview` | wasm target/bindgen, Node/npm (Tailwind v4 CLI) | Static host của preview (`vink-preview.css`), không App/Identity/DB |
| Kiểm preview | `cargo xtask verify preview` | wasm target/bindgen, Node/npm, Chrome | Giữ toàn bộ assertion preview hiện hữu trên `vink-preview.css` |
| Một Web scenario | `cargo xtask verify web --scenario password` | Web tools + Chrome, test PG credential | App + Identity và scratch của scenario; không worker |
| Web regression đầy đủ | `cargo xtask verify web` | Web tools + Chrome, test PG credential | 18 suite mặc định, gồm storage đúng một lần và capture đầy đủ |
| Regression hiện hữu | `cargo xtask verify all` | Theo từng lane | Vẫn quick → discussion → expressions → postgres → web; Kani riêng |

`build` nhận `api|spa|web|worker|identity|preview`; `--release` áp dụng cho mọi target trừ
preview. `build web` tạo browser bundle và App.

## Static, compile, unit và PostgreSQL

- `verify static [docs|tokens|assets|visual|perf|fmt|repo|all]` mặc định là `all`. `docs`
  kiểm skills, kho quyết định và link tương đối. `tokens` so CSS/Rust/Kotlin đã sinh,
  `assets` kiểm derivative artwork và manifest Expressions. `visual` và `perf` chạy
  oracle Node tương ứng. `fmt` cần rustfmt. `repo` gồm cả `docs` và các guard repository.
  `all` chạy các bước static của `quick`, tính `docs` trong `repo` một lần.
- `verify check OWNER [--target lib|bins|all-targets|api|ssr|hydrate|csr]` gọi
  `cargo check` với dependency graph của đúng package/target. Mặc định chọn lib/bins
  hiện hữu của owner. `api` dành cho App không feature `legacy-ssr`, `ssr` dành cho
  App/Web, `hydrate` cho Web, `csr` cho Web/Preview. Hai target WASM cần Rust target,
  không cần wasm-bindgen CLI, npm, asset bundle, database hoặc browser.
- `verify unit OWNER [--test TARGET] [--filter NAME]` nhận `app|identity|store|xtask|web|notifications|ui|patterns|expressions|markdown|preview|work-views`,
  cùng danh sách owner của `verify check`. `work-view-policy` là tên thay thế cho `work-views`. Chọn lib/bins theo manifest. Unit App có
  graph SSR theo feature mặc định. Unit Web bật rõ `--features ssr` để các test render
  HTML chạy được cả khi chỉ chọn riêng `vink-web`.
- `verify postgres store|app|identity|notifications [--filter NAME]` chọn đúng
  `--test postgres`. App giữ feature SSR mặc định vì test target có các ca SSR.
  Một số integration test còn yêu cầu Cargo build binary cùng package. Lượt `lifecycle`
  cần binary ba service hiện hữu để kiểm config, provision, migration/role và backup/recovery
  mà không chạy bốn test target. `verify postgres` không selector chạy cả bốn owner rồi lifecycle.

`--filter` chọn substring libtest. Xtask liệt kê test khớp và test ignored trước khi
chạy để chặn trường hợp zero match hoặc chỉ khớp test opt-in. PostgreSQL được probe
sau discovery, với khóa scratch giữ suốt phần chạy DB thật. Số test runnable khác chi phí
compile/link. Hai lượt list có thể build hoặc xác nhận lại artifact. Kết quả libtest in số thực thi
theo binary. Cargo bootstrap xtask là chi phí riêng cho cả lệnh `--plan`. Full `quick` giữ fmt,
clippy workspace và ba cấu hình WASM, doctests, SSR fixture, workspace lib/bins cùng các bước
static. Full `postgres` giữ nguyên các assertions, fixtures, role fences và race trong từng test.

`dev api|web` mặc định `--auth none`, kể cả `.env`/shell có socket, principal mẫu hoặc sign-in
được bật. `--auth fixture` đòi principal đã cấu hình; không tự tạo tài khoản, grant hay admin.
`--auth identity` xóa fixture, dùng native Identity và giữ kiểm UID/socket, Origin, cookie/Bearer
của dịch vụ. Cấu hình gửi ra ngoài hoặc authority khác `native` bị từ chối; sign-in bị disable rõ ràng
vẫn được giữ. Workflow local không mở khóa production. Khi cần thử riêng authority email-code
hay Resend HTTPS API, dùng [runbook auth](auth-operations.md) và binary trực tiếp.

## Lập kế hoạch theo diff

```sh
cargo xtask verify changed --plan                      # mặc định: origin/develop, worktree
cargo xtask verify changed --expect PLAN_ID            # chạy đúng plan đã xem
cargo xtask verify changed --base BASE --head HEAD_SHA --plan
cargo xtask verify changed --only "verify unit app"    # chạy một phần, kết quả PARTIAL
```

**Phạm vi.** `--base` mặc định là `origin/develop` của checkout. Xtask không fetch ref này,
nên fetch trước nếu cần base mới. Planner lấy merge base của base và head. Commit chỉ có ở
base không vào diff; plan in số commit bị loại. Không có `--head`, scope là worktree: commit
`merge-base..HEAD`, index, thay đổi chưa stage và tệp mới chưa bị ignore. `--head REV` chỉ xét
commit. Khi checkout khác head đã chọn hoặc có thay đổi cục bộ, plan committed chỉ đưa ra các lane bảo thủ và ghi rõ chưa xác định các integration target của head. Để nhận và chạy plan chính xác, dùng checkout sạch đúng head. Rename xét cả hai tên, deletion
giữ owner cũ. Ref sai, range (`A..B`), lịch sử không có merge base, tệp
`skip-worktree`/`assume-unchanged` và biến chuyển hướng `GIT_DIR`, `GIT_WORK_TREE`,
`GIT_COMMON_DIR`, `GIT_INDEX_FILE` đều là lỗi.

**Dependency.** Owner/path registry nằm ở [rules.rs](../../xtask/src/changed/rules.rs).
Registry kết hợp các dữ kiện đọc từ checkout ([graph.rs](../../xtask/src/changed/graph.rs)), không giữ bản sao:

- `cargo metadata --no-deps --offline` cho package, target, feature và path dependency.
  Thay đổi trong một crate chọn check mọi target, các biến thể feature có thật
  (`api`, `ssr`, `hydrate`, `csr`), unit, integration target và closure các crate phụ thuộc.
- `include_str!`/`include_bytes!`, `#[wasm_bindgen(module)]`, `sqlx::migrate!` và `#[path]` nối tệp
  dữ liệu về crate nhúng nó. Vector Quint, fixture contract và JS của Web đi theo cách này.
- `@import`/`@source` của hai entry Tailwind, module Kani và replay filter của `verify discussion`.
- Import ES giữa các fixture browser. Module được dispatcher gọi map tới scenario trong
  `BROWSER_MODULES`; helper đi theo importer. Phase chỉ chạy tay được ghi là nghĩa vụ thủ công.

Docs chọn `verify static docs`. CSS chọn token/repo guard, toàn bộ Web và standalone. Editor
chọn các scenario có editor. Domain App chọn unit, `discussion` hoặc Kani nếu bị replay hay compile.
HTTP chọn biến thể `api` và `verify postgres app`. Migration chọn PostgreSQL của owner và `lifecycle`.
Contract chọn test producer và consumer. `Cargo.lock`, toolchain, `.cargo/`, `.gitignore`, runner
xtask dùng chung hoặc tệp không có rule mở rộng sang mọi lane CI cùng các opt-in Web. Consumer
không chọn được ghi kèm điều kiện cần thêm. `verify quick` thay các selector nó đã chạy.
`verify postgres` thay các owner PostgreSQL.

**Output.** Plan in base/merge-base/head, fingerprint input, từng path với layer, status và
owner. Mỗi check có chi phí, prerequisite, lý do và path gây ra nó. Plan cũng in artifact cần
dựng lại, nghĩa vụ thủ công, phần không chọn kèm lý do, input không rõ, fallback và thời
gian lập plan. `--plan` chỉ đọc Git, manifest Cargo và vài tệp entry. Lệnh không build,
migrate, chạm database/browser hay network ngoài Cargo bootstrap của xtask. Diff rỗng in
`NO_WORK`, không coi là PASS.

**Thực thi.** Không có `--plan`, xtask tính cùng plan và chạy từng selector qua executor hiện
hữu, rẻ trước. `--expect PLAN_ID` từ chối nếu plan đã khác bản đã xem. Trước mỗi check và sau
check cuối, xtask tính lại fingerprint. Nếu commit, index, tệp sửa hay tệp mới thay đổi, các
check còn lại `NOT_RUN` và kết quả `FAIL`; khi đó lập plan lại. Một lỗi dừng các check sau.
Trong `tools/local-budget`, lane browser/Gradle được báo `UNSUPPORTED`.
Exit code là 0 khi mọi check đã chọn PASS, 1 khi có FAIL hoặc drift, 2 khi sai cú pháp
hoặc `--only` sai. Exit 3 dành cho `PARTIAL` (`--only`, `UNSUPPORTED`) và `NO_WORK`.
`--only` nhận đúng selector có trong plan. Typo, selector hợp lệ nhưng không được chọn và
selector đã nằm trong lane rộng hơn đều bị từ chối. Nghĩa vụ thủ công luôn `NOT_RUN` trong summary.

`verify unit OWNER --test TARGET` chạy một integration target ngoài `postgres`, ví dụ
`ssr_boundary`, `document_budget` của App, `scoped_cli` của xtask hay `editor_tools` của
patterns. Owner `work-views` là `vink-work-view-policy`. Hậu tố cũ
`--from-diff BASE_SHA HEAD_SHA committed|worktree` vẫn kiểm một selector với diff hiện tại.

Thêm package, module xtask, module browser hay loại input mới thì cập nhật registry. Test xtask
fail khi package thiếu owner, feature WASM thiếu selector, dispatcher import module chưa
đăng ký, module xtask chưa có vai trò, registry trỏ tới tệp không còn hoặc tệp tracked chưa có rule.
Chính sách bằng chứng ở [verification](../../.agents/skills/vink-engineering/references/verification.md).

## Plan, doctor, dữ liệu và artifact

```sh
cargo xtask dev api --auth identity --migrate --plan
cargo xtask doctor dev api --auth identity --migrate
cargo xtask doctor build api
cargo xtask verify web --scenario restart --plan
```

`--plan` phải ở cuối; liệt kê artifact, tools, services, credential **key** (không giá trị),
migration/seed, selected/dependency/not-selected checks và địa chỉ bind. Planner không chạy
probe, build, migrate, seed, child hoặc xóa output. Cargo có thể biên dịch chính `xtask` trước
khi vào planner. `doctor <command>` kiểm đúng tools/DB của plan; không gọi executor. Doctor
không thay validation cấu hình dịch vụ hoặc readiness sau startup.

Process env thắng `.env`, kể cả giá trị rỗng dùng để bỏ giá trị file. Scope là lớp giới hạn
capability cuối cùng: runtime child chỉ nhận các key của owner và auth đã chọn. Runtime không
nhận credential migrator/test-superuser; preflight từ chối role có quyền DDL/quản trị. Không sửa
`.env` của người dùng. Dev chỉ nhận loopback, `VINK_PROFILE=local` (mặc định của command) và
`VINK_PUBLIC_ORIGIN=http://VINK_BIND`. Preview dùng `VINK_UI_PREVIEW_BIND`, mặc định
`127.0.0.1:3019`; App mặc định `127.0.0.1:3000`.

Dev mặc định không migrate/seed. `--migrate` migrate App cho api/web/worker, Identity cho
`--auth identity` hoặc target identity, bằng credential tương ứng. Worker có `--migrate` cần
build thêm App để chạy migration; không khởi động API. `--seed` chỉ có trên api/web và dùng seed
mẫu hiện hữu; người dùng phải chọn rõ. Runtime không dùng URL migrator thay URL runtime.

API isolation chọn mức **không bundle và không mount SSR/static routes**: binary chạy
`serve-api`, dùng cùng router API, health và authorization. Các URL Web/static trả 404. Cargo
graph của `build api` và worker cũng loại Leptos SSR qua Cargo feature (#504). `serve` và Web mặc định giữ
SSR/hydration hiện hữu. Không thêm binary/service/domain handler song song.

Mọi build/dev/verify đều build theo source hiện tại (Cargo có cache chuẩn); không có `--no-build`
hay lấy lại bundle từ checkout khác. Cargo child dùng `target` của checkout, ghi đè
`CARGO_TARGET_DIR` để khớp đường artifact. Không `cargo clean` tự động. Artifact lock trong
`target/.xtask-artifacts.lock` giữ suốt lượt build/run/test: hai workflow cùng checkout fail rõ;
worktree khác có target riêng. File khóa được giữ lại, khóa OS tự nhả khi process chết; không xóa
file khóa để vượt một lượt đang chạy.

### Build package Web

`build web`, `dev web` và các kịch bản SSR dùng `target/site/pkg`. CSR phát hành dùng
`target/spa/_v/<revision>/pkg`, CSR verification dùng `target/verify-spa/_v/<revision>/pkg`.
Các lệnh vẫn gọi Cargo với profile, feature, Cargo.lock, environment và dependency graph hiện hành.
Cargo con build binary bỏ metadata tên, phiên bản và thư mục manifest của xtask để dependency native dùng chung với Cargo cha giữ được freshness.
Cargo quyết định freshness của WASM thô. Package có thể tái dùng đầu ra wasm-bindgen khi
WASM thô, executable bindgen đã kiểm version, cờ, feature, profile, prefix và biến
`WASM_BINDGEN_*` cùng đường loader do Cargo truyền khớp, đồng thời đủ tệp và SHA-256 khớp.
Tool wrapper, phiên bản bindgen mới, loader injection và WASM có thể tham chiếu `package.json`
đều chạy bindgen mỗi lượt. Mỗi đường package có một receipt trong `target/.xtask-web`, ngoài cây
được phục vụ.

Fonts, brand và empty artwork so trực tiếp tập tên tệp và SHA-256 với nguồn. Brand và artwork
vẫn chạy kiểm provenance mỗi lần. `HIT` tái dùng tệp nội bộ trong cùng checkout, `MISS` tạo
lại bước tương ứng. Package mới được dựng trong staging rồi chuyển vào đường publish khi đủ tệp
và qua Web artifact guard. Trên macOS/Linux, hệ điều hành đổi chỗ hai thư mục trong một
thao tác. Filesystem cần hỗ trợ directory exchange. Các host Unix khác giữ cơ chế chuyển dưới khóa. Xtask kiểm tín hiệu hủy trước khi bắt đầu chuyển package. Nếu cập
nhật receipt lỗi sau khi chuyển, xtask thu hồi package mới và khôi phục bản trước nếu có. Khi
package và receipt đã được cài, lỗi dọn bản sao cũ sẽ được báo để lượt sau dọn tiếp. SPA chỉ
khôi phục entry sau khi publish package thành công. Verify vẫn chạy kiểm bí mật, fixture và
assertion của scenario đã chọn.

Manifest theo từng package lưu bên ngoài thư mục public, tại
`target/.xtask-web/<khóa checkout và đường package>/manifest.json`. Mỗi mục ghi fingerprint
đầu vào, các nhóm tạo nên fingerprint và SHA-256 của output. Cargo vẫn chạy ở mỗi lượt. Xtask đọc
thông điệp `compiler-artifact.fresh` và băm WASM theo khối, kể cả bản debug lớn.
Bảng cuối log cho biết bước nào HIT, MISS hoặc VERIFIED, lý do và thời gian.

| Artifact | Đầu vào quyết định việc tạo lại |
| --- | --- |
| Token | Nguồn JSON, fixture, license/icon, font và 16 tệp CSS/Rust/Kotlin đã sinh. Mỗi lượt đối chiếu lại với generator. |
| Rust/WASM | Cargo theo dõi crate, dependency, toolchain, profile, feature và cờ. Digest của WASM thô cấp đầu vào cho bindgen. |
| Bindgen | Bytes của WASM, executable và version, target/flags, feature, profile, prefix, environment. Kiểm đủ JS, WASM và snippets trước khi tái dùng. |
| Tailwind | Recipe gọi compiler, script dò dependency, entry và các CSS import cục bộ, nội dung các thư mục Rust trong allowlist, tập file Oxide chọn, npm lock, cây compiler đã cài, Node, environment và minify. Compiler khai báo dependency thực. Plugin/config động hoặc nguồn ngoài allowlist buộc tạo lại CSS. |
| Editor | Nguồn TypeScript, script build, tsconfig, corpus, package/lock, cây npm đã cài và Node. Typecheck và corpus chạy cả khi HIT. Khi MISS, esbuild kê đầu vào thực trong metafile. Chỉ lưu receipt với một output và toàn bộ input thuộc editor. |
| Fonts, brand, empty | So tên và bytes với nguồn đã chọn. Tệp bị xóa khỏi nguồn cũng biến mất khỏi package mới. |
| Entry CSR | Template, appearance, revision/prefix và hai cờ `VINK_SPA_*`. Ghi entry sau khi package hoàn tất. |

Hai owner npm giữ stamp cài đặt trong checkout. Thay đổi cục bộ trong package phụ thuộc cũng đổi
khóa artifact. Thành phần PATH tương đối, wrapper, trình nạp tùy biến, symlink ngoài phạm vi hoặc thay đổi script bundle
có thể khiến bước tương ứng phải chạy mỗi lượt. Khi đổi cách build Editor, rà các input thật và
cập nhật digest của recipe sau khi kiểm.

**Bàn giao cho #501.** [Hợp đồng DAG](../../xtask/src/web_dag.rs) khai báo node, cạnh,
đường nguồn, tool, environment và output cho SSR/hydration, perf-probe và CSR.
`build web --plan` và `build spa --plan` in hợp đồng theo variant. `verify changed --base REV
--worktree --plan` gọi `affected_artifacts` cho cả hai phía của rename, in node bị chạm trực
tiếp, closure và đường Web chưa ánh xạ. Planner giữ các lệnh kiểm và fallback của owner.
Khi thực thi, digest thực tế quyết định có cần chạy producer. Preview có compiler entry và
thư mục xuất riêng.

Để đối chiếu hai package SSR, chạy trực tiếp `browser-smoke.mjs --phase reader-bundle`
với `--base`, `--chrome` và `--state` cho từng App dùng DB scratch có bài thơ mẫu.
Smoke kiểm hydration, hai khổ thơ, CSS, font và import module editor.

## Scenario Web và setup

| Selector | Setup / dependency | Assertion được dùng lại |
| --- | --- | --- |
| `main` (alias) | Hợp các suite từ `editor-first-draft` đến `storage` dưới đây | Tương thích với nhóm legacy kể cả storage và guest Desk |
| `editor-first-draft` | App đã migrate và hai principal mới, một cho mỗi format | `firstDraftFormat`, CTA/radius/keyboard ở 320px text200, POST và reload |
| `reader` | Sample work đã công khai, nháp riêng và tag catalog, loopback author | `serverHtml`, `readerBeforeHydration`, `conditionalPublicReads`, `tracePublicReadLifecycle`, 404/asset của Preview, `publicReading`, stanza/font/hydration và không tải editor runtime. Pha `reader-views` dùng thêm App khách để kiểm điều hướng, ngưỡng, mất ACK sau commit và số riêng của tác giả |
| `discussion` | Sample work và author, scratch writes cùng response fixture có nhãn | `discussionReadingAndWrites` (lost ACK, CAS, closed/copy, own remove, flat context, paging, theme/geometry) và `endorsementAsAuthor` |
| `appearance` | Sample App, danh mục thẻ từ lệnh operator và loopback author | `adaptiveShell` + `bottomNavigationRegression`, brand, hydration/first paint, focus/overflow, theme/text, storage denied, forced colors/reduced motion |
| `desk` | Sample App/author, response fixture có nhãn, browser context riêng và App khách tắt sign-in | `authorWorks`, `draftDashboard`, `deskEmptyState`, `draftR6Hierarchy`, `timestampPresentation`, `draftPagination`, `selectChevron`, `deskGuest` |
| `editor-chrome` | Sample App/author, nháp mới và tag catalog | `editorChrome` gọi `editorPaperFirst`, `editorWriting`, reflow, AX, CDP IME, race và recovery matrices |
| `editor` | Sample App/author và catalog thật | Cùng client navigation Reader → Desk → nháp, Vietnamese input, save/reload, conflict, leave/back, late ACK, undo/redo và `draftTags` |
| `editor-format` | Sample App/author và nháp fixture tạo qua API/UI | `deskRefusesLossyOpen`, `markdownLivePreviewAndFormatPicker`, `editorDeskRegressions`, `deskBudget` (route Product, cold/warm và input), `markdownBudget` (dispatch CM), `editorCoreContracts` (preset, mount trễ/lỗi) |
| `storage` | App scratch, hai tab Chrome và reopen cùng profile | `publicStorage`/`storageReopened`, IndexedDB envelope/transaction/epoch/quota/migration; chạy một lần khi ghép với `main` |
| `discovery` | Seed/catalog + corpus SQL riêng | `discovery-corpus` |
| `restart` | `restart-prepare` lưu và reload nháp riêng, catalog Đô thị/Mùa; stop → retire tag → start | Giữ `afterRestartPhase` và `retiredTagAfterRestart` cho đúng draft/version/title, tag và unsaved buffer |
| `endorsement` | Sample App; App tác giả + reader có moderator grant trong scratch | `endorsement`, không cần Identity |
| `review` | Sample App, tag catalog, tác giả + admin + reviewer trong scratch | `review-withdrawn`, phản đối tại Bàn viết, quyền API ca/quyết định, snapshot Markdown và Reader SSR/hydrate |
| `auth` | App scratch migrate/seed + Identity scratch/local outbox | `auth` với session thật |
| `password` | App password scratch migrate + Identity native scratch/local outbox | `auth-password`, bao gồm scope Thông báo trước và sau logout/re-login qua App Gateway, không cần phase auth/main |
| `standalone` | Artifact CSR + proxy loopback, API-only, Identity và App DB scratch | Reader, tìm kiếm, lịch sử, mật khẩu, editor, bảo vệ bản nháp và Lượt xem qua hai tài khoản thật (gửi lại sau commit, Bàn viết, đổi phiên). #616 mở ô trả lời của khách với projection có nhãn, kiểm hình học và Auth khi Gửi |
| `author-search` | App password scratch và Identity native, fixture hồ sơ với bút danh trùng | `author-search`, SSR/no-JS, IME mô phỏng, Back, phân trang và Theo dõi theo phiên |
| `publication` | App publication scratch migrate/seed/catalog + Identity scratch/local outbox | `publication-author` |
| `notifications` | Sample App scratch với loopback author; CDP mô phỏng Auth và các API Thông báo | `notifications` (tab đủ quyền, badge, popover, hộp thư, quyết định, reflow và fence ở client) |
| `perf-baseline` | App scratch + catalog + 200 public works/1.000 comments tổng hợp, phiên khách | 10 cold SSR/hydration + 10 warm Reader/Back, ledger CDP, history và secondary; **opt-in** |
| `return-position` | Cùng fixture công khai của `perf-baseline` | Back/Forward, liên kết quay về, scope/filter/cursor, 50 vòng và các ca focus; **opt-in** |

`verify web … --release` build/chạy binary release và bundle `wasm-release` trên cùng setup
scratch. Mặc định vẫn dùng dev. Chọn rõ `perf-baseline` và `return-position`; full regression
không chạy hai phép đo này. Xem [phương pháp #234](web-perf-baseline.md) và
[LayoutSpec #236](../design/discovery-return-position-236.md).

Lặp `--scenario` để ghép suite. Thứ tự canonical giữ sample Reader trước corpus discovery và
mỗi scenario chạy được từ scratch mới. Chọn `restart` chỉ chạy setup browser được ghi trong
bảng. Typo, chuỗi rỗng, selector trùng và cờ sai phạm vi bị từ chối trước khi vào executor.
Catalog này là mapping từ [phase legacy](../../apps/vink-web/e2e/browser-smoke.mjs) tới các
lệnh mới. Các nhánh auth thực vẫn dùng Identity và local outbox. Scenario `notifications` kiểm
hành vi trình duyệt R4 trên shell/hộp thư/quyết định; luồng hai process thật với Notification
Service vẫn nằm ở lane PostgreSQL owner/lifecycle.

Mỗi scenario SSR còn cần App binary, WASM hydrate/bindgen, CSS/Tailwind, editor JS,
Node/npm, bundle guard, Chrome và scratch PostgreSQL. `standalone` dùng CSR và App API.
Selector và capture giảm công việc sau build. Cache artifact áp dụng theo hợp đồng package ở trên.

`--capture full` là mặc định toàn lane và từng scenario. `--capture checks` yêu cầu ít nhất
một `--scenario` tường minh, giữ hành vi, a11y, target và các assertion DOM như kích thước,
theme, text-scale. Chế độ này bỏ PNG và kiểm pixel, ghi `PNG/visual evidence NOT_RUN`
trong plan và không tạo manifest visual. Mở PNG và đối chiếu context khi claim visual.

Web dùng `VINK_TEST_DATABASE_URL`, không reset `DATABASE_URL` dev. Credential harness chỉ dùng
cho setup; App/Identity runtime dùng role scratch hạn chế. Các tên `vink_verify_web*` và
`vink_verify_prov*` là namespace dành riêng cho test. Một advisory lock trên database `postgres`
của test server chặn các lượt Web/PostgreSQL đồng thời, kể cả khác worktree. Khóa này không tuần
tự hóa concurrency/race bên trong một test. Không chạy binary xtask từ revision cũ song song:
revision cũ chưa tham gia khóa.

Evidence nằm ở `target/verify-web/<run-id>/`. Scenario có subtree riêng cho browser state
và `screenshots/contexts.json` ở capture đầy đủ. Manifest giữ execution phase, assertion/scenario ID,
viewport, theme, text scale, required targets và build provenance. Hai format first-draft
và guest Desk có thư mục con. Giữ log ở ngoài target trước khi dọn target.
Dev log ở `target/dev-runs/<run-id>/`; kiểm Identity/backup của lane PostgreSQL giữ
`target/verify-identity/<run-id>/` và `target/verify-backup/<run-id>/`. Socket test có thư mục riêng ngắn dưới `/tmp` cho giới hạn
macOS, xóa sau khi child dừng. Dev giữ socket cấu hình của người dùng; nếu đã tồn tại thì fail,
không tự xóa stale socket. Kiểm log và owner trước khi tự xử lý socket cũ.

## Dừng, lỗi và compatibility

Ctrl-C/SIGTERM hủy workflow, trả exit khác 0, dừng child do nó tạo và giữ log. Cargo/Node được
chạy trong process group riêng để dọn descendants. App/Identity chờ readiness thật; worker đòi
log schema current và queue ready (hoặc outbox được disable rõ ràng). Timeout, child thoát sớm,
port conflict và duplicate run là lỗi; không `pkill`, không dừng tiến trình đang giữ cổng. Khi
setup thất bại, DB dev/volume không bị reset; scratch cleanup nằm trong khóa dành cho test.

| Lệnh cũ | Hành vi / đường chuyển |
| --- | --- |
| `cargo xtask build [--release]` | Giữ full bundle + App/worker/Identity; dùng `build api|web|worker|identity` khi chỉ cần một phần |
| `cargo xtask dev` | Giữ build đầy đủ, migrate owner đã cấu hình, optional Identity và App; dùng `dev <target>` để scope explicit và prepared DB |
| `cargo xtask doctor` | Giữ preflight dev tổng hợp; dùng `doctor <scoped command>` cho prerequisite tối thiểu |
| `cargo xtask db migrate` / `db seed` | Giữ owner/policy cũ; scoped dev có `--migrate`/`--seed` explicit |
| `cargo xtask ui-preview leptos build|run|verify` | Vẫn dùng được; tương ứng `build preview`, `dev preview`, `verify preview` |
| Các lane quick/discussion/expressions/kani/postgres/all, CMP/native previews | Giữ nghĩa vụ kiểm; không đổi bounds hoặc resource controls #112 |

## Owners và lựa chọn kỹ thuật

Baseline khảo sát `develop@7361ab0`: build/dev luôn tạo WASM/editor/assets + cả ba binary;
doctor đòi Web tools; Web verify luôn chạy mọi phase và xóa thư mục evidence cũ. Build binary
không có nghĩa start process: dev cũ không bật PostgreSQL hay worker. UI preview vốn không cần
business stack. Không có baseline RAM/thời gian để công bố phần trăm cải thiện.

- CHOICE-01 A: mở rộng verb + target hiện có; không thêm `stack up`/daemon/session store.
- CHOICE-02 A: cùng App binary, router composition API/Web. SSR trước đây là dependency bắt buộc. #504 tách feature `legacy-ssr`.
- CHOICE-03 A: typed `Request`/`Plan`/`Scenario`; không registry shell/DSL hay task graph tổng quát.
- CHOICE-04 A: attach PG đã provision, supervise foreground; không quản lý volume/container.

[cli.rs](../../xtask/src/cli.rs) parse; [plan.rs](../../xtask/src/plan.rs) resolve thuần;
[workflow.rs](../../xtask/src/workflow.rs) thực thi/config scope;
[build.rs](../../xtask/src/build.rs) artifact; [lifecycle.rs](../../xtask/src/lifecycle.rs) child,
ready, cancellation, locks; [verify.rs](../../xtask/src/verify.rs) compose regression lanes;
[web.rs](../../xtask/src/web.rs) setup/assertions Web; [legacy.rs](../../xtask/src/legacy.rs)
compatibility. Main chỉ dispatch. Không đổi state machine nghiệp vụ, vì vậy không tạo model
Quint mới; lifecycle OS/SQL/browser cần kiểm đúng boundary thay cho model.

Bằng chứng lượt triển khai và phần chưa chạy: [scoped-workflows-evidence](scoped-workflows-evidence.md).

### Fixture thử biểu cảm

Dùng `cargo xtask build preview`, `cargo xtask dev preview` và mở Thử biểu cảm · Mẫu trong gallery.
`cargo xtask verify preview` chạy coverage Chrome cho fixture. Web quick/build kiểm route, dependency
và package. [Ranh giới Web/Preview](web-preview-boundary.md) ghi chi tiết.
