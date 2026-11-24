# Baseline verification local · #497 và #502

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

## Phép đo đã chạy

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
Số hit/miss của compiler, số object reuse và thời gian rebuild xuyên worktree là `NOT_RUN`.

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

1. Giữ target và incremental theo checkout. Bootstrap của launcher và Cargo con dùng chung
   nơi chứa sản phẩm cuối và intermediate. Bằng chứng ở trên xác nhận cách giải quyết đường dẫn.
   Compiler wrapper như sccache còn `UNSUPPORTED` do tiến trình server có thể ngoài session;
   không có số hit/miss, dung lượng cache hoặc rebuild cho biến thể dirty/toolchain/features.
2. Giữ profile hiện tại và giới hạn lịch chạy của #250. Giảm debuginfo có thể giảm dữ liệu
   linker nhưng làm mất thông tin debug. Số đo fmt và Node chưa xác định được compile/link
   chiếm bao nhiêu thời gian hoặc RAM; chưa có cơ sở chọn giảm debuginfo hay thay số job.
3. Lượt benchmark này được giao trần cứng **toàn workload ≤14 GB** và một owner tại một thời điểm.
   macOS launcher chỉ có soft admission. Hai Podman VM có sẵn đều đang tắt. Điều kiện để tiếp tục đo
   build trên máy này là xác minh trần của môi trường được cấp và quyền sở hữu workload.
   Browser/CMP/native/OCI và Linux strict giữ trạng thái trong SSOT; chạy các lane này ở
   lượt riêng sau khi có owner, readback giới hạn và bằng chứng cancel/teardown tương ứng.
4. Khi đủ điều kiện, đo `cargo xtask verify unit store` với SHA, lockfile, toolchain,
   feature, jobs và profile cố định. Dùng target trống của checkout thử, lặp ngay trên cùng
   target, rồi đo ở worktree thử thứ hai. Ghi riêng discovery/compile+link và execution,
   số test được chọn, wall/RSS, pressure/swap, kích thước target, compiler hit/miss nếu có.
   Để so sánh debuginfo và scheduling, chỉ đổi một yếu tố mỗi lần. Thử dirty source,
   toolchain, feature, profile và hủy/khởi động lại trước khi kết luận về artifact reuse.
   Giữ cache giữa các lượt cùng điều kiện. Lưu log ngoài target trước khi owner thu hồi dung lượng.
