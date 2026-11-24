# Baseline verification local · #497 và #502

Số đo Web, PostgreSQL, SSR, planner và các khoảng trống của lượt tích hợp cuối nằm trong [bàn giao #512](work-chat-4-followup.md).

Ngày 24/11/2026, đo trên một Mac mini arm64, macOS 15.6 (Darwin 24.6), RAM 24 GiB,
còn khoảng 24 GiB đĩa. Worktree Vink riêng, tuần tự, không flush page cache. Hai cụm
PostgreSQL có sẵn nằm ngoài session. Trước phép đo, danh sách tiến trình không có
Cargo/rustc/xtask/Gradle đang chạy và slot không có session record.

- **A** là `45eea5052a60771f2b7a9a3610b70b22499bf051` (develop sạch), **B** là
  `0a0e3a492da86299624fa21cf808956b262a4c2c` (launcher mới). Cùng source Rust/JS,
  Cargo/rustc 1.99.1, Python 3.13.5 và Node 22.15.0.
- Chạy từ gốc checkout, không có `.env`. SHA-256 của `Cargo.lock` là
  `0a2c2dbbd7243fa6c819c172cbb648b6fac83bf875ab1489319a14c3ff775877`.
  Các lượt Vink dùng toolchain pin của repo, không đặt rustflags, build profile, wrapper,
  features hay override incremental. Launcher chọn 2 Cargo jobs, 2 libtest threads.
  Các bước đã chạy không cần registry download. Cache OS có thể đã ấm ngay từ lần đầu.
- [Log quan sát theo từng lượt](samples.txt) giữ kết quả, số mẫu, đơn vị, exit và pressure/swap.
  Đường dẫn riêng của máy đã rút gọn. [Chính sách admission](../../local-resource-budget.md).

## Bước nhẹ trên macOS

| Bước và input | Wall ngoài (s) | Wall session (s) | RSS | Cache và phạm vi |
| --- | ---: | ---: | --- | --- |
| A, `cargo fmt --all -- --check`, lần đầu qua launcher | 1,40 | 1,36 | 96.640 KiB | 2 mẫu session. Checkout chưa có target. |
| A, lặp nguyên lệnh | 1,34 | 1,31 | 94.208 KiB | 2 mẫu. Cả hai lượt fmt đều không tạo target. |
| A, gọi fmt trực tiếp sau cặp trên | 1,27 | — | 95.076.352 byte | `time -l` báo max RSS, không có tổng theo session. |
| B, fmt qua launcher mặc định | 1,34 | 1,31 | 91.968 KiB | 2 mẫu session. |
| B, fmt với `--samples` | 1,36 | 1,33 | 89.520 KiB | 0,001 s: 1 tiến trình / 2.000 KiB; 1,016 s: 2 / 89.520 KiB. |
| A, Node analyzer, cache V8 trống | 0,05 | — | 48.414.720 byte | 5/5 test, TAP 34,052 ms; 0 → 4 tệp (20 KiB đĩa). |
| A, Node analyzer, cùng cache đã ghi | 0,05 | — | 48.381.952 byte | 5/5 test, TAP 32,402 ms; 4 → 4 tệp (20 KiB đĩa). |

Dùng `/usr/bin/time -lp` bọc từng lệnh. Với fmt qua launcher, argv là
`python3.13 -B tools/local-budget run [--samples] --timeout 90 -- cargo fmt --all -- --check`.
Với Node, chạy `NODE_COMPILE_CACHE="$PWD/target/bench-497-node" node --test apps/vink-web/e2e/web-perf-baseline.test.mjs`.
Đây là lệnh test trong `verify quick`; chỉ bật cache module Node cho cặp thí nghiệm.
RSS của Node là max RSS do Darwin time báo cho lệnh, chưa đo tổng đồng thời của Node cha/con.
Pressure normal, swap used 351,62 MiB ở hai đầu mỗi lượt có probe. Lệnh fmt trực tiếp chỉ có
số từ `time`. Mỗi điều kiện có một quan sát, RSS lấy mẫu có thể bỏ lỡ đỉnh ngắn hoặc cộng
trùng trang nhớ. Chênh 0,02 s giữa hai chế độ ghi mẫu chưa đủ ước lượng overhead điển hình.

### Hai worktree fixture, đọc lại bằng Cargo thật

Từ B, sao chép launcher vào crate độc lập một file, không dependency. Hai Git worktree A/B
của **fixture** cùng commit `5ae9722003ae62b9739b621b881d2fcc1bf68054`. Cả hai dùng
một Cargo home fixture và ba biến thư mục Cargo ban đầu cùng trỏ tới một thư mục shared chưa tồn tại.
Chạy `run --samples --timeout 30 -- cargo fmt --all -- --check` lần lượt A, A rồi B.
Một probe qua `RUSTFMT` gọi Cargo thật `metadata --no-deps --offline --format-version=1`
trong môi trường child, kiểm cả hai field rồi chuyển argv sang rustfmt thật.

| Lượt | `target_directory` và `build_directory` đọc lại | Wall ngoài / session | RSS session (1 mẫu) |
| --- | --- | --- | ---: |
| A đầu | `a/target` | 0,52 / 0,49 s | 2.096 KiB |
| A lặp | `a/target` | 0,17 / 0,13 s | 2.016 KiB |
| B đầu | `b/target` | 0,17 / 0,14 s | 1.808 KiB |

Cả ba lượt exit 0, thư mục shared vẫn vắng, pressure normal và swap used 351,62 MiB.
Phép đo bao gồm probe và fmt trên crate nhỏ; mẫu RSS duy nhất rơi sát lúc spawn.
Ba lượt fmt/metadata không chạy compiler. Số rebuild trên source Vink thật nằm ở phép đo guest dưới đây.

## Rust trong Vink VM, giới hạn 2 GiB

Đo trên cùng Mac mini, trong VM riêng `vink-419-verification` (AppleHV, 2 vCPU,
6 GiB, đĩa 16 GiB). Guest Fedora CoreOS Linux arm64 7.2.5 có 5,4 GiB đĩa trống
trước khi đưa dữ liệu vào và không có swap. VM Cantos dùng đĩa khác và vẫn tắt.
Số liệu guest có hệ điều hành, kiến trúc phần mềm và phạm vi đo riêng.

**Nguồn và cache.** Hai worktree Git guest A/B cùng commit Vink
`5008ffd9d4009fd5967d561c82dbc56c95e5681b`, cùng `Cargo.lock` ở trên.
Mỗi worktree ban đầu sạch, có `target` riêng và được mount vào cùng đường dẫn
`/src` ở các container chạy tuần tự. Cargo home thử nghiệm có sparse index
và các archive crates.io đã khóa, được giữ giữa tất cả lượt. Ở hàng *cold*, target
trống. Page cache guest và registry đã giải nén có thể ấm, nhất là khi đến B.
Image đã có sẵn `rust:1.99.1-bookworm`, ID
`10513cd0b64b1e7470d965645f4ac257a45c2b8b66b394cf775a15102b799e74`.
Rustc `58cf8f9c3`, Cargo `5f94df478`, target `aarch64-unknown-linux-gnu`.

**Giới hạn.** Giữ khóa admission trên Mac trong suốt lượt. Guest không có container
cạnh tranh. Mỗi điều kiện có một container rootless mới, network tắt, image và
source chỉ đọc trong container, target và Cargo home ghi được. Trước khi gọi
Cargo, đọc lại `memory.max=2147483648`, `memory.swap.max=0`, `cpu.max=100000 100000`
và `pids.max=128` của chính cgroup. Các dịch vụ guest cũng nằm trong RAM 6 GiB của VM. Bài đo cgroup độc lập này chưa triển khai adapter Linux strict của launcher.

**Workload.** Một compiler job, một libtest thread, Cargo mặc định test profile
và features của package. Chỉ hai hàng `debug0` đặt `CARGO_PROFILE_TEST_DEBUG=0`.
`CARGO_TARGET_DIR` và `CARGO_BUILD_BUILD_DIR` cùng là `/src/target`.
Đo wall của lệnh dưới đây rồi lấy đúng `executable` của test từ JSON của Cargo
và chạy trực tiếp với `--test-threads=1`.

```sh
cargo test --locked --offline -p vink-expressions --lib --no-run --message-format=json
```

| Lượt theo thứ tự chạy | Cargo compile+link (s) | Libtest (s) | Wall hai bước (s) | Cargo fresh | Test đạt | RSS KiB (số mẫu) | Peak cgroup MiB | Target sau lượt KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| a-cold | 1.661 | 0.269 | 1.937 | 0/9 | 8 | 337000 (19) | 598.9 | 45548 |
| a-warm | 0.039 | 0.272 | 0.312 | 9/9 | 8 | 7296 (4) | 42.3 | 45548 |
| b-cold | 1.483 | 0.254 | 1.739 | 0/9 | 8 | 354032 (18) | 232.3 | 45548 |
| b-warm | 0.036 | 0.254 | 0.292 | 9/9 | 8 | 7288 (3) | 42.4 | 45548 |
| b-debug0 | 1.272 | 0.199 | 1.473 | 0/9 | 8 | 318344 (15) | 215.1 | 67648 |
| b-debug0-warm | 0.036 | 0.208 | 0.246 | 9/9 | 8 | 7228 (3) | 42.3 | 67648 |
| b-dirty | 0.174 | 0.253 | 0.428 | 8/9 | 9 | 216812 (5) | 132.6 | 69740 |

Wall hai bước gồm cả khoảng đọc tên executable. Cargo bao gồm resolve/fingerprint,
build scripts khi cần, compile và link; chưa đo riêng linker. Cột *fresh* đếm
`compiler-artifact.fresh` do Cargo trả. Trên B, đổi debug info làm cả 9 artifact
được build lại. Lượt cuối bỏ override, thêm đúng một test rỗng có tên riêng **chỉ
ở B trong guest**. Cargo dùng lại 8 artifact, build lại 1 và binary chạy đủ 9 test.
Thư mục target B chứa cả hai profile variant nên số cuối là dung lượng cộng dồn.

RSS là tổng trang resident trong PID namespace container (gồm sampler/shell), lấy
mẫu khoảng 100 ms, có thể cộng trùng hoặc hụt đỉnh. Kernel `memory.peak` đo toàn
cgroup từ khởi động container và có tính phần file cache bị charge cho cgroup.
PSI memory guest và container đều báo 0 trong các mẫu, swap current và OOM kill
đều bằng 0. Pressure và swap của host macOS chưa được lấy mẫu trong bảy lượt guest.
Release flags, assertions và cấu hình VM giữ nguyên.

Với owner này, compiler đã làm lại 9/9 artifact ở B lạnh dù A có target ấm.
Trong cùng worktree, Cargo dùng lại 9/9 và libtest chiếm phần lớn thời gian còn lại.
Mỗi hàng là một quan sát. Số đo chưa đủ để đổi debug mặc định hay kết luận cho
workspace, browser, PostgreSQL hoặc sccache. [Log thô từng lượt](rust-guest.txt).

## Kiểm kê bước và khoảng trống

| Entry point / owner | Bước, dependency và cache | Bằng chứng lượt này |
| --- | --- | --- |
| [quick](../../../../xtask/src/verify.rs) | UI tokens, artwork, expressions, Node visual oracle và perf analyzer, fmt, clippy workspace SSR và ba cấu hình WASM (hydrate, CSR Web/Preview), doctest, SSR fixture, workspace lib/bins, repository checks. Cargo cache theo graph/target/profile. | `PASS` fmt và 5 test analyzer. Phần khác `NOT_RUN`. |
| [unit store](../../../../xtask/src/workflow.rs) | Bootstrap xtask → doctor → Cargo test `vink-store-postgres --lib` với `--list` (có thể compile/link) → fail nếu zero-match → chạy test đã chọn. Filter tác động số test thực thi. | Resource preflight `PASS`; compile, link và libtest `NOT_RUN`. |
| [bundle](../../../../xtask/src/build.rs) | Kiểm boundary/tool → compile và link wasm hydrate/csr → wasm-bindgen → npm theo lock, typecheck/bundle editor → tokens/Tailwind → chép assets và kiểm output. SSR binary có graph Cargo host riêng. | SSR/hydrate/bundle `NOT_RUN`. Invalidation và reuse asset thuộc #500. |
| [Web discovery](../../../../xtask/src/web.rs) | [Plan](../../../../xtask/src/plan.rs) chọn bundle + App, Chrome, App scratch/migration; chạy fixture, SSR/hydration, tương tác và teardown. Node spawn Chrome, các renderer/process tách session cần kiểm riêng. | Preflight của argv kết thúc bằng `--plan` qua budget trả 0; xtask plan thực `NOT_RUN`. Browser runtime `UNSUPPORTED` qua launcher. |
| [postgres](../../../../xtask/src/verify.rs) | Probe DB, khóa scratch, bốn integration test owner (store, App, Identity, Notifications), build binaries, cấu hình lỗi, [provision/role/cleanup](../../../../xtask/src/provision.rs). DB đã chạy là external. | `NOT_RUN`. Chưa có số của riêng fixture, DB hay runtime. |
| [discussion](../../../../xtask/src/discussion.rs), [expressions](../../../../xtask/src/expressions.rs), [Kani](../../../../xtask/src/kani.rs) | Quint/negative controls và Rust replay theo owner; Kani/CBMC dùng pin cùng manifest/harness. `all` gọi quick, discussion, expressions, postgres và Web. Kani có lane riêng. | `NOT_RUN`. Chi phí solver và tuning PG thuộc #112. |

## Lựa chọn và phép đo tiếp theo

Với #497, chọn đo từng lệnh con qua launcher hiện hữu. Ghi mẫu sử dụng chính lần đọc `ps`
đang phục vụ giám sát. Chỉ cần thêm timer ở `xtask::process` hoặc `Report` khi phải phân bổ
thời gian giữa nhiều bước trong cùng một lần chạy. Các bảng trên không phân rã compile và link.

1. Giữ target và incremental riêng cho từng checkout. Cargo báo 9/9 artifact fresh ở hai
   lượt ấm mặc định và 0/9 ở mỗi target trống. Compiler wrapper như sccache còn
   `UNSUPPORTED` trong launcher do server có thể nằm ngoài session. Chưa đo cache hit/miss
   của sccache hay invalidation khi đổi toolchain hoặc feature.
2. Giữ profile và lịch chạy mặc định của #250. Trên B, test profile bỏ debug info build
   trong 1,272 s; target B tăng thêm 22.100 KiB vì đã có artifact profile mặc định. Cấu hình
   này giảm thông tin cho debugger. Một owner nhỏ và một lượt mỗi điều kiện chưa đủ chọn
   cấu hình chung. Có thể bỏ biến `CARGO_PROFILE_TEST_DEBUG` để về profile trước đó.
3. Browser, CMP/native, OCI và Linux strict có phạm vi và điều kiện riêng trong SSOT.
   Baseline guest chỉ bao phủ build test và libtest của `vink-expressions`. `unit store`,
   workspace, PostgreSQL, SSR/WASM/bundle, browser runtime và invalidation #500 còn mở.
   Khi có môi trường đủ chỗ cho graph lớn hơn, giữ cùng SHA, inputs, coverage và trần RAM,
   so compiler/linker, fixture và runtime trên đúng lane đó.
