# Chạy và kiểm đúng phạm vi (#184)

Đây là hợp đồng tooling local, không thay luật sản phẩm, auth authority hay trạng thái ADR.
`build` tạo artifact; `dev` giữ tiến trình để thử tay; `verify` chạy kiểm hữu hạn. PostgreSQL luôn
được attach sau khi [provision](../../services/postgres/README.md), không lệnh nào tự bật container.

## Chọn theo việc muốn làm

Tất cả ví dụ chạy ở repo root. Dùng `cargo xtask --help` để xem cú pháp thực thi.

| Việc | Lệnh | Prerequisite ngoài Rust | Tiến trình / dữ liệu |
| --- | --- | --- | --- |
| Unit/contract một owner | `cargo xtask verify unit app --filter publication` | Dependency biên dịch thật của package | Test binary, không server/DB/browser; zero match là FAIL |
| Build API | `cargo xtask build api` | Không tool frontend | Chỉ binary `vink-app` |
| API công khai | `cargo xtask dev api` | App DB đã migrate, role runtime | App với health + API, ẩn danh |
| API với đăng nhập thật | `cargo xtask dev api --auth identity` | App/Identity DB đã migrate, runtime roles | Identity native + App; local mail outbox, không SMTP |
| Worker | `cargo xtask dev worker` | App DB đã migrate, runtime role | Chỉ `vink-worker`; outbox/retention, evaluation không tự bật |
| Identity riêng | `cargo xtask dev identity` | Identity DB đã migrate, runtime role | Chỉ Identity native/socket/local outbox |
| Web thử tay | `cargo xtask dev web --auth identity` | App/Identity DB, wasm target/bindgen, Node/npm | App SSR + assets và Identity; không Chrome E2E |
| Web bằng dữ liệu mẫu | `cargo xtask dev web --auth fixture --seed` | App DB đã migrate; frontend tools | Principal từ `VINK_DEV_PRINCIPAL`; seed rõ ràng, không phải auth thật |
| Preview Leptos | `cargo xtask dev preview` | wasm target/bindgen | Static host của preview, không App/Identity/DB |
| Kiểm preview | `cargo xtask verify preview` | wasm target/bindgen, Node, Chrome | Giữ toàn bộ assertion preview hiện hữu |
| Một Web scenario | `cargo xtask verify web --scenario password` | Web tools + Chrome, test PG credential | App + Identity và scratch của scenario; không worker |
| Web regression đầy đủ | `cargo xtask verify web` | Web tools + Chrome, test PG credential | Toàn bộ 8 scenario bên dưới |
| Regression hiện hữu | `cargo xtask verify all` | Theo từng lane | Vẫn quick → discussion → expressions → postgres → web; Kani riêng |

`build` nhận `api|web|worker|identity|preview`; `--release` dùng cho bốn target server, không
preview. `build web` tạo browser bundle và App, không build Identity/worker. `verify unit` nhận
`app|identity|store|xtask|web`; `--filter` dùng substring của libtest. Unit App vẫn biên dịch Rust
SSR do dependency package hiện tại; không yêu cầu Node/npm/bundle hoặc database đang chạy.

`dev api|web` mặc định `--auth none`, kể cả `.env`/shell có socket, principal mẫu hoặc sign-in
được bật. `--auth fixture` đòi principal đã cấu hình; không tự tạo tài khoản, grant hay admin.
`--auth identity` xóa fixture, dùng native Identity và giữ kiểm UID/socket, Origin, cookie/Bearer
của dịch vụ. Cấu hình SMTP hoặc authority khác `native` bị từ chối; sign-in bị disable rõ ràng
vẫn được giữ. Workflow local không mở khóa production. Khi cần thử riêng authority email-code
hay SMTP, dùng [runbook auth](auth-operations.md) và binary trực tiếp.

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
graph vẫn có Leptos SSR; chưa có claim loại dependency Rust SSR. `serve` và Web mặc định giữ
SSR/hydration hiện hữu. Không thêm binary/service/domain handler song song.

Mọi build/dev/verify đều build theo source hiện tại (Cargo có cache chuẩn); không có `--no-build`
hay lấy lại bundle từ checkout khác. Cargo child dùng `target` của checkout, ghi đè
`CARGO_TARGET_DIR` để khớp đường artifact. Không `cargo clean` tự động. Artifact lock trong
`target/.xtask-artifacts.lock` giữ suốt lượt build/run/test: hai workflow cùng checkout fail rõ;
worktree khác có target riêng. File khóa được giữ lại, khóa OS tự nhả khi process chết; không xóa
file khóa để vượt một lượt đang chạy.

## Scenario Web và setup

| Selector | Setup / dependency | Assertion được dùng lại |
| --- | --- | --- |
| `main` | App scratch + seed + catalog; fixture tác giả | Toàn bộ phase `main`: SSR/hydration/editor và các kiểm con hiện hữu |
| `discovery` | Seed/catalog + corpus SQL riêng | `discovery-corpus` |
| `restart` | **Dependency `main`** lưu nháp/state; stop → retire tag → start | `after-restart`, version/text/tag/export và graceful stop |
| `endorsement` | Seed/catalog; App tác giả + reader có moderator grant trong scratch | `endorsement`, không cần Identity |
| `review` | Seed/catalog; tác giả + admin + reviewer trong scratch | `review-withdrawn`, không cần Identity |
| `auth` | App scratch migrate/seed + Identity scratch/local outbox | `auth` với session thật |
| `password` | App password scratch migrate + Identity native scratch/local outbox | `auth-password`, không cần phase auth/main |
| `publication` | App publication scratch migrate/seed/catalog + Identity scratch/local outbox | `publication-author` |

Lặp `--scenario` để hợp thành nhiều kiểm; thứ tự canonical giữ như full lane. Typo, chuỗi rỗng,
selector trùng và option trái operation bị chặn trước side effects. Selected PASS chỉ có nghĩa
với các kiểm liệt kê; full lane dùng cùng catalog và giữ tất cả assertions cũ. `main` vẫn là một
nhóm rộng theo runner hiện hữu, chưa tách các kiểm con như IME/appearance thành selector xtask.

Web dùng `VINK_TEST_DATABASE_URL`, không reset `DATABASE_URL` dev. Credential harness chỉ dùng
cho setup; App/Identity runtime dùng role scratch hạn chế. Các tên `vink_verify_web*` và
`vink_verify_prov*` là namespace dành riêng cho test. Một advisory lock trên database `postgres`
của test server chặn các lượt Web/PostgreSQL đồng thời, kể cả khác worktree. Khóa này không tuần
tự hóa concurrency/race bên trong một test. Không chạy binary xtask từ revision cũ song song:
revision cũ chưa tham gia khóa.

Evidence nằm ở `target/verify-web/<run-id>/` (log, state, screenshots), không xóa lượt trước.
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
- CHOICE-02 A: cùng App binary, router composition API/Web. Dependency Rust SSR vẫn còn;
  optional SSR feature là phương án sau nếu có số đo đủ để đánh đổi thêm build combinations.
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
