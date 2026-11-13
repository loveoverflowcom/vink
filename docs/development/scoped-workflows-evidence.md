# Bằng chứng local cho workflow có phạm vi (#184)

Ngày 13/11/2026, macOS Darwin arm64; checkout bắt đầu sạch tại
`develop@7361ab0771240418bfa7eae46191460634c81960`, đã `git pull --ff-only origin develop` trước
khi tạo `codex/scoped-local-workflows`.

- Input của runtime smoke và `quick` trước rebase: implementation
  `577a50d3ffcddcfb85834a98cba2cb1a1bc2173c`.
- Đã fetch/rebase lên `origin/develop@4ff33bf4fdcfa431797c316534a1b2c4e90cf3d1`;
  implementation sau rebase: `338fef0dd8eef814c1b8d8d843abc395291878b0`, không conflict.
  Các file `xtask`, App `http/mod.rs` và `main.rs` giống hệt bản trước rebase.
- Trên source sau rebase, chạy lại `cargo test --locked -p xtask`: 72 unit + 1 integration PASS
  (1 PG test ignore mặc định); `cargo clippy --locked -p vink-app --lib --bin vink-app -- -D warnings`
  PASS. Đây là kiểm App/Web dependencies ở chế độ server sau khi tích hợp #183, không phải browser
  hay runtime smoke mới. Các kiểm runtime đầy đủ sau rebase vẫn NOT_RUN do dung lượng.
- Commit tiếp theo chỉ cập nhật metadata bằng chứng này; không đổi implementation.

Đây không phải nghiệm thu CI hay quyết định kiến trúc Accepted. [Hợp đồng và choices](scoped-workflows.md) sở hữu phạm vi lệnh.

## Môi trường và điều kiện

- Rust/Cargo 1.99.1; PostgreSQL 17.11 Homebrew; Node 22.15.0, npm 10.9.2;
  wasm-bindgen 0.2.129; Chrome có tại `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`.
- Cwd: `/Users/manhblue/projects/loveoverflow/vink`. Không có `.env`; không tạo hoặc sửa `.env`.
- PostgreSQL test riêng do lượt này tạo bằng `initdb -D /tmp/vink184-pg -U vink --auth=trust`,
  chỉ nghe loopback `127.0.0.1:55484`. Dùng provisioning hiện hữu để tạo App/Identity và hai role
  mỗi owner. Không dùng container hoặc database dev có sẵn; dừng và dọn cluster do lượt này tạo sau khi kiểm.
- Các kiểm Cargo cuối dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
  vì máy thiếu dung lượng. Không đổi profiles, test threads, Kani/Quint bounds trong repository.
- Log/harness local được giữ ở `target/issue184-evidence/`; log dịch vụ ở `target/dev-runs/<run-id>/`.
  Chỉ dọn artifact tạm do chính lượt này tạo sau lỗi hết đĩa, không chạy `cargo clean` mặc định.
  Không có so sánh cold/warm, thời gian/RAM hoặc phần trăm tiết kiệm.

## Kết quả theo boundary

| Lệnh / phép kiểm | Kết quả | Boundary và giới hạn |
| --- | --- | --- |
| `cargo test --locked -p xtask` | PASS | 72 unit tests, 1 integration CLI test; 1 test PG ignore mặc định và được chạy riêng bên dưới |
| `cargo clippy --locked -p xtask --all-targets -- -D warnings` | PASS | Static Rust tooling; bản cuối cũng được workspace server clippy kiểm qua quick |
| `cargo xtask verify unit xtask --filter plan::tests` | PASS | 4 test được discovery và execute thật; không DB/browser/server |
| `VINK_TEST_DATABASE_URL=… cargo test --locked -p xtask scratch_lock_serializes_runs_and_releases_on_drop -- --ignored` | PASS | PostgreSQL thật: connection thứ hai bị từ chối, thả owner rồi acquire lại được; không tuần tự hóa race trong test |
| `cargo fmt --all -- --check`, `git diff --check`, kiểm relative links/metadata bị đổi | PASS | Repository/source/doc scope |
| `cargo xtask verify quick` | FAIL — môi trường | PASS ui tokens, expression manifest, fmt và **clippy server toàn workspace**; clippy hydrate WASM lỗi `No space left on device`; các bước sau NOT_RUN |
| `cargo test --locked -p vink-app --lib http::tests::api_composition` | FAIL — môi trường | Linker hết đĩa trước khi test chạy. Hành vi API/router đã được kiểm bằng binary + PG thật bên dưới |
| `cargo xtask verify web --scenario password` | FAIL — môi trường | Preflight Rust/WASM/bindgen/Node/npm/Chrome/PG pass sau khi đặt `VINK_CHROME`; build hydrate dừng ở `tachys` vì hết đĩa. Browser assertions NOT_RUN |
| `cargo xtask verify web` đầy đủ | NOT_RUN | Cùng artifact WASM đang bị chặn; phải chạy lại vì đổi shared setup/orchestration |
| `cargo xtask verify preview`, `dev preview`, Web manual/hydration | NOT_RUN | Thiếu dung lượng để hoàn tất artifact WASM; không dùng kết quả planner thay runtime proof |
| `cargo xtask verify postgres` toàn lane | NOT_RUN | Chưa đủ dung lượng cho các integration test binaries; smoke process/role/lock không thay toàn lane |
| `cargo tree --locked -p vink-app -i leptos` | PASS (inspection) | Xác nhận App vẫn kéo Leptos 0.8.22/SSR; không claim backend build đã loại Rust SSR |

Không chạy Kani/Quint/CMP/native vì không đổi owner luật/proof đó. Không inspect/poll/rerun
GitHub Actions/checks/logs/artifacts theo scope issue.

## Smoke process thật và kiểm phủ định

Harness local chạy đúng `target/debug/xtask` được build từ source của PR, với các giá trị mẫu
trong `.env.example` chuyển sang port PG `55484`; App bind `127.0.0.1:30184`. PATH đặt các
executable sentinel cho `node`, `npm`, `wasm-bindgen`, `google-chrome`, `chromium`: nếu được gọi
chúng ghi marker rồi thoát 98. Không marker nào xuất hiện trong cả ba workflow sau.

| Workflow | Child quan sát qua `ps` sau readiness | Outcome |
| --- | --- | --- |
| `dev api --migrate --seed` | Chỉ `vink-app serve-api` | `/health/live`, `/health/ready`, `/api/v1/works` = 200; `/api/v1/drafts` = 401; `/` và `/pkg/vink.js` = 404 |
| `dev api --auth identity --migrate` | `vink-identity serve` + `vink-app serve-api` | Native password register = 200/needs_principal + session cookie; principal = 201; tạo nháp = 201; đọc bằng session = 200, ẩn danh = 401. Không fixture fallback |
| `dev worker` | Chỉ `vink-worker` | Schema current, outbox ready; không API/Identity/browser. Đây là lifecycle/readiness, không chứng minh delivery của mọi consumer |

Migrate dùng credential owner; preflight kiểm runtime role bằng `role_is_privileged` hiện hữu.
Sau readiness, Ctrl-C gửi tới parent làm command trả nonzero và dọn child của nó; các lần start
sau trên cùng cổng thành công. Đăng nhập mật khẩu không cần gửi email; outbox chỉ local và không
bật SMTP/provider thật. Chưa kiểm luồng email-code qua command mới bằng browser.

Các smoke PASS trên source `577a50d` được quan sát trước lượt `quick` (Identity/App PID 71160/71163 ở lượt auth; worker PID 71272). Lượt lặp smoke sau `quick` bị chặn
ngay khi relink App vì hết đĩa; lưu riêng ở `runtime-repeat-failed.log`, không phải một PASS mới.
Log `auth-api.log`, `worker.log` và các thư mục `dev-runs` của lượt PASS còn được giữ. Không suy
runtime PASS từ binary build thất bại của lượt lặp.

Các ca lỗi đã chạy:

- Start lần hai bị artifact/session lock từ chối; App đầu tiên vẫn ready, không bị dừng.
- Giữ cổng bằng socket khác: dev từ chối, giữ nguyên socket đó. Preflight dùng `SO_REUSEADDR`
  để không nhầm TIME_WAIT của phiên vừa dừng với listener đang sống.
- Runtime DB không tồn tại/không nghe: fail trước build/start.
- Path socket có file của bên khác: fail và giữ nguyên bytes, không tự xóa stale socket.
- Libtest filter không khớp: fail `zero tests`, không báo PASS rỗng.
- Hủy trong bước unit discovery với Cargo giả đang giữ child `sleep`: parent fail, process group
  và descendant bị dọn. Đây là kiểm wiring/cancellation, không giả runtime DB/auth.
- Test readiness kiểm child thoát 7 và timeout: trả lỗi; guard dọn child đang chờ.
- Binary integration test đặt tool sentinel và credential marker: `--plan`/selector sai không gọi
  tool/executor, không in credential hoặc địa chỉ cấu hình không hợp lệ.

## Acceptance và phần cần tiếp tục

| AC | Bằng chứng / khoảng trống |
| --- | --- |
| 01–03 | Guide inventory + typed plan/parse/doctor; unit/CLI + actual selected unit + sentinel runtime PASS |
| 04 | API không bundle/SSR routes được kiểm thật; Rust SSR graph còn và được công bố |
| 05 | API public/auth/worker PASS; Web manual và preview runtime NOT_RUN |
| 06 | Catalog/dependency/invalid/zero selection PASS; selected/full browser assertions NOT_RUN |
| 07 | Scope env/precedence tests và App/Identity restricted-role runtime smoke PASS; Web scratch grant thay đổi còn cần regression browser/PG |
| 08 | Guard/timeout/child failure/cancel/duplicate/occupied-port/stale-socket PASS trên macOS; không tuyên bố Windows/Linux runtime đã chạy |
| 09 | File lock + PG advisory lock PASS; output unique qua source; Web scratch cleanup/restart đầy đủ còn cần runtime |
| 10 | Main dispatch; CLI/plan/build/workflow/lifecycle/verify/feature setup có owner riêng; server workspace clippy PASS |
| 11 | Sentinel + child tree trên PG thật PASS; không có benchmark hoặc tổng RSS suy đoán |
| 12 | Guide/feature map/verification skills cập nhật; PR dùng **Refs #184**, chưa đóng issue khi thiếu regression |

Tiếp tục trên máy có đủ đĩa: chạy focused App router test, một `verify web --scenario password`
độc lập, rồi `verify web` đầy đủ, `verify preview`, Web manual và `verify postgres`; cuối cùng
`verify quick` đủ lane. Không cần cài toolchain mới trên máy hiện tại; phải chuẩn bị lại
`VINK_TEST_DATABASE_URL` vì cluster tạm đã dọn và đặt `VINK_CHROME` trên macOS. App/Identity
migrations và runtime roles vẫn là hai credential riêng. Full PASS không được suy từ các hàng
smoke hoặc static checks ở trên.
