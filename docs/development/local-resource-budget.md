# Ngân sách build/test local trên macOS

`local-24g` là profile **SOFT_MANAGED** cho các agent/worktree Vink cùng tài khoản trên một
máy. Dùng Python 3.13+ có `os.waitid` trên macOS. Launcher nằm trong checkout và gọi Cargo
sau khi có quyền vào slot. [Issue #250](https://github.com/loveoverflowcom/vink/issues/250) ·
[Launcher](../../tools/local-budget) · [Tài liệu Python](https://docs.python.org/3.13/library/os.html#os.waitid).

## Dùng hằng ngày

```sh
./tools/local-budget plan
./tools/local-budget plan -- cargo xtask verify unit store
./tools/local-budget run -- cargo xtask build api --plan
./tools/local-budget run -- cargo xtask verify unit store
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 ./tools/local-budget run --timeout 600 -- cargo test --locked -p xtask
```

`plan` đọc profile mà không lấy lock hay gọi Cargo. `plan -- cargo …` xét thêm chính sách tài
nguyên cho lệnh đã nhập (mã 69 khi lane chưa được hỗ trợ). Muốn xem dependency do xtask chọn,
chạy `./tools/local-budget run -- cargo xtask verify web --scenario main --plan`. Lệnh đó có
Cargo bootstrap. Planner và executor [#184](scoped-workflows.md) giữ quyền chọn việc. Launcher nhận `cargo xtask|test|build|check|clippy|fmt` từ gốc checkout của chính launcher.

- Một slot dùng chung tại `/tmp/vink-local-budget-<uid>/admission.lock`. Checkout đang giữ slot
  có thể gọi Cargo con trực tiếp qua xtask. Gọi lồng launcher trả mã 2. Thứ tự lấy khóa là slot
  tài nguyên → khóa artifact của checkout → khóa scratch PostgreSQL nếu lane cần. Race test
  vẫn chạy các transaction/barrier bên trong từng test.
- Cargo nhận tối đa 2 compiler jobs ngay từ bootstrap. Với `build.jobs` dương, file gần checkout
  thắng file tổ tiên và Cargo home. `CARGO_BUILD_JOBS` từ môi trường có ưu tiên cao hơn.
  Libtest nhận tối đa 2 thread từ `RUST_TEST_THREADS` trong môi trường, rồi `.env` của checkout.
  Giá trị dương thấp hơn được giữ. Giá trị lớn hơn bị hạ xuống 2. Một `--test-threads` tường minh
  được phép nếu không vượt mức hiệu lực. Dùng biến môi trường thay cho `--jobs` và viết
  từng cờ Cargo ngắn riêng, ví dụ `-p xtask`.
- Launcher từ chối setting mơ hồ hoặc có thể ghi đè ngân sách trước bootstrap. Các trường hợp
  gồm số âm/0/sai định dạng, `Cargo --config`, Cargo config có `include`, Cargo `[env]` đặt
  biến ngân sách, jobserver từ một orchestrator đang chạy, alias xtask bị ghi đè và rustc wrapper
  có thể gọi compiler bên ngoài. Các cấu hình này có thể hợp lệ khi chạy Cargo thông thường.
  [Tham chiếu Cargo](https://doc.rust-lang.org/cargo/reference/config.html#buildjobs).
- Từ bootstrap, launcher đặt `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR` và
  `CARGO_BUILD_BUILD_DIR` vào `target` của checkout. Cả artifact cuối và file biên dịch
  trung gian ở lại đó khi chạy lại. Launcher từ chối `--target-dir` trỏ nơi khác,
  mọi symlink ở chính `target` (kể cả link hỏng hay vòng) và Cargo `[env]` đặt các biến này.
  Với `--target-dir` hợp lệ, Cargo nhận đường dẫn tuyệt đối đã kiểm tra. Profile và debug info vẫn do
  Cargo chọn theo lệnh. [Cấu hình target-dir và build-dir](https://doc.rust-lang.org/cargo/reference/config.html#buildtarget-dir).
- Mốc 10–12 GiB là mục tiêu quan sát trên máy 24 GiB, để chừa khoảng 12–14 GiB cho macOS, IDE,
  trình duyệt và dịch vụ có sẵn. Profile thử hai compiler job và hai libtest thread để giảm
  số đỉnh đồng thời mà vẫn giữ một ít song song. Một compiler/solver có thể dùng nhiều RAM hơn
  mục tiêu. Launcher ghi pressure và swap đầu/cuối, chỉ nhận việc khi pressure là `normal`,
  và ghi RSS cộng theo session từ `ps` mỗi khoảng 1 giây. Số mẫu lớn nhất có thể bỏ qua đỉnh ngắn
  và tính lặp shared pages. Nó không phải RAM vật lý duy nhất hay trần RAM.
  [Chỉ báo pressure trên macOS](https://github.com/apple-oss-distributions/xnu/blob/main/doc/vm/memorystatus_notify.md).

## Đo một verification step

Dùng đúng lệnh con trong lane cần đo. Ví dụ sau đo bước định dạng thuộc `verify quick`.

```sh
/usr/bin/time -lp python3.13 -B tools/local-budget run --samples -- cargo fmt --all -- --check
```

`--samples` ghi thời gian từ lúc bắt đầu, số tiến trình sống cùng UID/session và tổng RSS
(KiB) ở từng lần quan sát. Log dùng stderr. `UNKNOWN` cho biết lần quan sát không thấy
thành viên sống. Khi mọi lần quan sát đều trống, peak cũng là `UNKNOWN`. Wall của launcher
bao gồm spawn và drain. Wall từ `time` còn gồm cả Python và admission. Khi chạy cả lane,
số liệu session thuộc toàn lệnh. Chỉ gán chi phí cho compile/link/test khi đo được ranh giới đó.
Giữ log bên ngoài target trước khi bàn giao. [Baseline macOS và Rust trong VM, số thô và phần chưa đo](evidence/verification-budget-497/README.md).

## Phạm vi tiến trình

| Đường chạy | Phạm vi |
| --- | --- |
| Cargo/bootstrap, Rust test, xtask, compiler/bundler và process con cùng Unix session | Tham gia admission. Tín hiệu hủy đi tới process group do launcher tạo. |
| PostgreSQL đã chạy, IDE và daemon có trước | External. Khóa scratch và cấu hình PostgreSQL của [#112](https://github.com/loveoverflowcom/vink/issues/112) giữ nguyên. |
| Kani/CBMC và Quint | Cùng session nếu tool giữ quan hệ tiến trình thông thường. Giữ version, harness, proof bound, solver scheduling và kiểm summary hiện có. Chưa chạy kiểm boundary của các tool này trong #250. |
| Gradle/CMP desktop | `UNSUPPORTED` trước bootstrap. Gradle 8.14.3 tạo JVM single-use ở session riêng dù đã dùng `--no-daemon`. `-Xmx2g` hiện hữu chỉ là heap của JVM. |
| Native Android/iOS, browser verification, `verify all`, OCI build | `UNSUPPORTED` qua launcher. Browser/native chưa đủ bằng chứng ownership. Engine/VM của OCI nằm ngoài session. Lane nguyên bản vẫn giữ suite và assertions. |
| Linux strict | `UNSUPPORTED` trước bootstrap, kể cả khi chỉ định `--mode strict` trên Mac. Thiếu adapter đã kiểm chứng với systemd user và cgroup v2 memory controller. |

Build script hoặc tool tùy biến có thể khởi động process ngoài Unix session. Chỉ dùng profile với
workflow biết được vòng đời tiến trình. Lệnh Cargo/Gradle chạy trực tiếp và checkout chưa dùng
launcher nằm ngoài admission. Các việc nặng dành cho những đường đó cần được bố trí riêng.

## Hủy, phục hồi và rollback

- Slot bận hoặc pressure chưa đạt trả mã 75. Đọc session ID trong log và chạy lại khi lượt đang
  giữ slot kết thúc hoặc pressure về normal. Launcher không xếp hàng hay tự retry.
- Dừng bằng Ctrl-C ở terminal sở hữu hoặc gửi SIGTERM tới **supervisor đang chạy đã xác minh**
  trong log. `--timeout SECONDS` tính cả bootstrap và trả 124 khi hết thời gian. Launcher báo lỗi
  child theo exit status, giữ leader chưa reap khi gửi tín hiệu tới process group và dùng SIGKILL
  sau tối đa 10 giây nếu cần. Nếu còn process cùng session sau hủy, lượt sau vẫn bị chặn.
- Khi supervisor bị SIGKILL, hệ điều hành nhả flock. Record đã ghi trước Cargo giữ session ID
  và boot UUID. Lượt mới chỉ dọn record sau khi xác minh session không còn tiến trình sống hoặc
  máy đã reboot. Đợi session cũ kết thúc, hoặc dùng terminal/Activity Monitor để xác minh và dừng
  tiến trình thuộc chính lượt đó. Không dùng PID trong record cũ làm lệnh kill. Không xóa
  `admission.lock` hoặc `session` để vượt chặn.
- Nếu một lượt đọc `ps` trong lúc giám sát lỗi hoặc quá 3 giây, launcher không coi workload là
  đã kết thúc: nó gửi SIGKILL tới process group, trả mã 1 và giữ record. Lượt sau chỉ nhận việc
  sau khi xác minh session cũ không còn tiến trình sống. Máy đang chịu áp lực bộ nhớ cao có thể
  gặp trường hợp này; chạy lại khi pressure về normal.
- Bật profile khi gọi launcher. Chờ lượt hiện tại kết thúc trước khi trở lại lệnh
  [xtask nguyên bản](scoped-workflows.md) hoặc revert PR. Không cần đổi shell startup,
  Cargo/Gradle home, cache hay dữ liệu DB.

## Kiểm launcher

```sh
python3 -B -m unittest discover -s tools -p test_local_budget.py
```

Nhóm `Profile` (precedence, validator argv, wrapper, `--plan`) chạy trên mọi nền tảng có
Python 3.11+. Nhóm `Lifecycle` (hai checkout, hủy, timeout, crash fence, pressure) cần macOS
và tự `skip` ở nơi khác. Suite này chưa thuộc lane `cargo xtask verify` nào và CI không chạy nó;
sửa launcher cần chạy lại trên Mac và ghi rõ nhóm đã chạy.

## Quyết định triển khai

| Lựa chọn của #250 | Phương án |
| --- | --- |
| CHOICE-01 | Launcher trước Cargo. Guard trong `xtask::main` không thấy compiler của lượt bootstrap lạnh. Giữ planner, executor và lifecycle của xtask. |
| CHOICE-02 | Một flock theo host/tài khoản, từ chối ngay và hiển thị session giữ slot. So với chờ có timeout, phương án này không giữ hàng đợi hoặc worker chờ lâu trong phiên agent. |
| CHOICE-03 | Hướng tiếp theo là một transient user unit cho mỗi lượt đi qua cùng slot, thay vì slice có nhiều child unit. Với một lượt hoạt động, unit có thể giữ một ngân sách tổng cho descendants. Cần kiểm membership và readback bằng một fixture cô lập trên Linux có systemd user/cgroup v2, thử high 10 GiB, max 12 GiB và swap hữu hạn. Nếu controller sai, quyền thiếu hoặc dependency thoát nhóm, strict phải trả lỗi trước workload chính. [systemd resource control](https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html). |

## Bằng chứng macOS 15.6 arm64, 24 GiB

`cargo test --offline --locked -p vink-expressions` dùng cùng source, Cargo 1.99.1, libtest 2 và
registry cache có sẵn. Chạy tuần tự. Lượt tham chiếu dùng 4 compiler jobs, cùng supervisor và bộ
lấy mẫu. Lượt profile dùng 2 jobs qua CLI. Mỗi cấu hình đo một lượt với target trống, sau đó
chạy lại trên target đã build. Mỗi lượt chạy đủ 8 unit test. Package này có 0 doctest.

| Target | Jobs tham chiếu → profile | Wall tham chiếu → profile | RSS cộng lớn nhất trong mẫu, KiB | Số mẫu, cách 1 giây |
| --- | --- | --- | --- | --- |
| Trống | 4 → 2 | 4,04 → 2,39 s | 381.056 → 292.000 | 4 → 3 |
| Đã build | 4 → 2 | 0,46 → 0,46 s | 2.000 → 2.192 | 1 → 1 |

Pressure normal ở đầu/cuối. `sysctl vm.swapusage` báo used 4.620,69 M ở cả bốn lượt.
18 process PostgreSQL có từ trước có tổng RSS tức thời 29.600–33.328 KiB trước các lượt, ngoài
mẫu session. Overhead của phần còn lại của hệ thống chưa đo riêng. Page cache của OS được giữ
và thứ tự đo có thể ảnh hưởng thời gian. Lượt warm ngắn hơn chu kỳ lấy mẫu. Số đo chỉ mô tả
workload nhỏ này, chưa định lượng đánh đổi cho toàn workspace hay cho Kani/PostgreSQL.

Lượt `cargo test --locked -p xtask` qua launcher từ target trống mất 42,93 s, ghi
999.280 KiB với 42 mẫu, đạt 81 unit + 1 CLI test; một test scratch PostgreSQL được ignore theo
cấu hình sẵn có. Lượt thử CMP cũng chạy xong test desktop, nhưng PID của Gradle daemon là session
leader riêng và Gradle test worker kế thừa session đó. Một mẫu test worker có RSS 896.272 KiB
ngoài session launcher. Kết quả đó chứng minh vì sao profile từ chối CMP ở bản bàn giao.
[Giải thích single-use daemon của Gradle](https://docs.gradle.org/current/userguide/gradle_daemon.html#sec:disabling_the_daemon).
