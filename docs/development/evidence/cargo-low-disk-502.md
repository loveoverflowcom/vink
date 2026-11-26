# Dung lượng Cargo dev/test trên một checkout (#502)

Phép đo Cargo dùng launcher ở `c9afc79706994b8534fd29e806cdb99de6e9592d`, base
`8d45a6afb0271bb2d0272c756bdf0db89cbee879`. Package, manifests, lockfile,
toolchain và các owner xtask giống nhau giữa hai revision. Sửa preflight sau phép đo
bổ sung kiểm tra `.env` của xtask. Checkout đo không có `.env` và bốn biến Cargo
được đối chiếu lại ở head bàn giao. Thời điểm đo 26/11/2026 UTC.

## Phép so sánh

Mac mini arm64, một checkout mount chỉ đọc vào VM Vink scratch 4 GiB. Mỗi lượt dùng
một container Linux arm64 mới từ cùng image Rust 1.99.1 (`10513cd0b64b`), offline,
một CPU, một Cargo job, một libtest thread. Giữ slot admission của Mac khi chạy guest.
Mỗi container kiểm `memory.max=2147483648` byte và `memory.swap.max=0` trước Cargo.
Launcher chạy trên macOS. Guest có cgroup riêng và dùng bốn giá trị từ
`tools/local-budget plan --low-disk` cho lượt bật tùy chọn.

Lệnh kiểm chạy từ gốc checkout.

```sh
cargo test --locked --offline -p vink-markdown-text --lib --message-format=json
```

Giữ cùng registry đã giải nén, source, flags và đường dẫn `/src/target`. Dùng hai
target scratch lúc đầu rỗng, gắn lần lượt vào đường dẫn đó. Đo compact lạnh rồi ấm,
mặc định lạnh rồi ấm, sau đó bật và bỏ cờ trên chính target mặc định. Mỗi lượt chạy
đạt một unit test. Cargo báo `opt_level=0`, debug assertions và overflow checks bật
ở cả hai chế độ. Debuginfo của test binary lần lượt là 2 mặc định và 0 ở compact.

| Target và chế độ | Wall (s) | RSS mẫu lớn nhất (KiB) | Peak cgroup (byte) | Toàn target đầu → đỉnh mẫu → cuối (KiB) | Cargo fresh / rebuilt |
| --- | ---: | ---: | ---: | --- | --- |
| Trống, mặc định | 1,237 | 304.780 | 195.829.760 | 0 → 38.756 → 38.756 | 0 / 8 |
| Ấm, mặc định | 0,063 | 1.900 | 51.154.944 | 38.756 → 38.756 → 38.756 | 8 / 0 |
| Trống, low-disk | 0,944 | 288.428 | 171.061.248 | 0 → 16.668 → 16.668 | 0 / 8 |
| Ấm, low-disk | 0,063 | 7.616 | 51.126.272 | 16.668 → 16.668 → 16.668 | 8 / 0 |
| Đang có bản mặc định, bật low-disk | 0,945 | 293.292 | 171.343.872 | 38.756 → 55.412 → 55.412 | 0 / 8 |
| Bỏ cờ trên cùng target | 0,063 | 2.236 | 51.138.560 | 55.412 → 55.412 → 55.412 | 8 / 0 |

Swap cgroup bằng 0 ở cả sáu lượt. Wall gồm build và libtest. Đếm `compiler-artifact.fresh`
từ stdout Cargo. Lấy tổng RSS của session Cargo và `du -sk` toàn target khoảng 60 ms/lần
kèm thời gian đọc mẫu. RSS có thể đếm trùng trang chia sẻ. Lượt ấm chỉ có một mẫu.
`memory.peak` của cgroup gồm cả file cache được tính cho container. Mỗi điều kiện đo
một lần. Page cache của hệ điều hành được giữ. Mức giảm dung lượng 57,0% áp dụng cho
package và target trống này. Cần đo riêng Web/SSR/WASM để định lượng lợi ích cho chúng.

## Kiểm chứng theo phạm vi

- Trên macOS, suite launcher chạy 16/16 ca, gồm contention, cancel, crash fence, target
  riêng, precedence giữa bootstrap và `.env` của xtask, profile opt-in và khôi phục môi trường ở lượt kế. `plan --low-disk` hiển thị
  bốn setting trước bootstrap. `run --low-disk -- cargo test --help` tới Cargo thật.
- Trong guest, `cargo xtask verify unit store` chạy đạt 5/5 test với cấu hình low-disk.
  Đây là kiểm unit của Store. Compile/test đầy đủ qua launcher
  native trên Mac dưới trần cứng 14 GB là `NOT_RUN`.
- Fixture Cargo độc lập có một test. Bật feature thêm một test và build lại một unit.
  Tắt feature dùng artifact cũ và còn một test. Thêm một test vào source rồi phục hồi
  làm Cargo build lại ở mỗi lượt, lần lượt chạy hai và một test. Hai lần thử feature
  HTML/SIMD trên dependency thật của Markdown trả 101 trước compile vì index offline
  thiếu `pulldown-cmark-escape`. Không đổi lockfile.
- Benchmark hai worktree, đổi branch với source khác và đổi Rust toolchain trong lượt
  này là `NOT_RUN`. Compiler wrapper, browser, CMP và Linux strict vẫn là
  `UNSUPPORTED` qua launcher. Chạy các workload external ở lượt riêng có ngân sách phù hợp.
  [Baseline hai worktree ở revision cũ](verification-budget-497/README.md).
