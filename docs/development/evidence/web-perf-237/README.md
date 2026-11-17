# Evidence Web Perf #237 — Ngân sách byte RAM trong READ-02

Lượt đo và đối chiếu A/B/C (`WEB-PERF-07 / CHOICE-01 = B`), base `origin/develop` tại `afb4020`, ngày 17/11/2026 trên macOS arm64 (Rust 1.99.1, wasm-bindgen 0.2.129, Node v26.12.0, Chrome headless, PostgreSQL 17 loopback).

- **Kho quyết định ([decisions.json](decisions.json)):** `READ-01-CHOICE-01..04`, `READ-02-CHOICE-01..04`, `DISC-WEB-CHOICE-01..03` ở `r1 DESIGN_PROPOSED`; `O-03`, `O-17` (`r2`), `USER-02`, `USER-09` (`r1`) ở `PRODUCT_CONFIRMED`; `VDR-PUBLIC-OFFLINE-AGE@r1`, `VDR-PUBLIC-SNAPSHOT-STORAGE@r1`, `VDR-WEB-READ-LIFECYCLE@r1` ở trạng thái `PENDING`. Không signoff hay đổi quyền/tuổi dữ liệu (`max_display_age = 15 phút`, `24 giờ` offline, `public_persistence::ENABLED = false`).
- **Tách biến theo AC-01:** `snapshot_weight_bytes` trong `apps/vink-web/src/public_reads.rs` chỉ đo **proxy byte UTF-8 + container cấu trúc** của snapshot không quan sát trong `InMemoryCache`. Các biến riêng gồm: active DTO/signal trên view đang mount (`Entry::data`, `Live.data`, `shown`), `in_flight` ticket, JSON UTF-8 dây truyền/decode (`serde_json`), DOM nodes (`Memory.getDOMCounters`), V8 JS heap (`Runtime.getHeapUsage`) và bộ nhớ tuyến tính WASM (`WebAssembly.Memory.buffer.byteLength`). **Giới hạn:** trọng số logic không bảo đảm trần RSS tiến trình OS hoặc bộ nhớ GPU.

| File | Nội dung |
| --- | --- |
| [decisions.json](decisions.json) | Kết quả `xtask decision query` cho 18 ID liên quan tại base `afb4020` |
| [corpus-and-sim-release.csv](corpus-and-sim-release.csv) | Bảng 21 case corpus `#229` + 3 fixture `#234` (UTF-8, JSON, proxy, `clone_us`, `walk_us`, `decode_us`) và mô phỏng 50 vòng A/B/C ở `--release` |
| [probe-abc-chrome.json.gz](probe-abc-chrome.json.gz) | Trace Chrome `--release` (`wasm-release`) đo A (`HEAD afb4020`), B (`READ_CACHE_BUDGET`) và C (`NoOpCache`) trên 16 bài `#234`, 16 bài `#229` 200k và 4 bài `tiny_blocks-200000` |
| [perf-baseline-after.json.gz](perf-baseline-after.json.gz) | Raw `verify web --scenario perf-baseline --release` sau chỉnh `#237` (cùng fixture với baseline `afb4020`) |
| [return-position-after.json.gz](return-position-after.json.gz) | Raw `verify web --scenario return-position --release` (50 vòng back/return) sau chỉnh `#237` |
| [negative-control.log](negative-control.log) | Đối chứng âm chạy fault-injection trên `freshness.rs` (không commit): bỏ removal fence ở `Store::seed` làm test FAIL đúng kỳ vọng |

## Lựa chọn kỹ thuật `WEB-PERF-07 / CHOICE-01`: chọn B (`2 MiB` tổng / `1 MiB` mỗi entry)

| Phương án | `#234` (16 bài) WASM / GET | `#229` 16 bài 200k WASM / GET | `tiny_blocks-200000` × 4 WASM / Reopen | Kết luận Go/No-Go |
| --- | --- | --- | --- | --- |
| **A (trước sửa, soft 16 entry)** | `1,84 → 2,88 → 3,15 MB`; list revisit `0` GET | `1,84 → 21,76 MB`; list revisit `0` GET | Tăng đơn điệu `91,68 → 102,24 → 112,79 → 123,40 → 132,45 MB` (+40,76 MB) | **NO-GO**: 16 bài `tiny_blocks-200k` giữ ~51,2 MB JSON / ~156,8 MB wasm32 proxy; gây `memory.grow` liên tục |
| **B (weighted LRU `2 MiB / 1 MiB`)** | `1,84 → 1,90 → 3,15 MB`; list revisit `0` GET | Phẳng ở đoạn giữa `9,18 MB`, đỉnh `16,84 MB` (−4,92 MB); list revisit `0` GET | Phẳng tuyệt đối `91,68 → 91,68 → 91,68 → 91,68 → 91,68 MB`; render đủ `200.000` `<p>`; list `0` GET | **GO**: giữ 100% cache hit ở `#234`/bài văn thông thường, chặn hoàn toàn cộng dồn `tiny_blocks` |
| **C (`NoOpCache` tất cả)** | `1,84 → 2,82 → 2,88 MB`; **+21 GET** (`listReqs16=16`, `revisitList=6`, `revisitReader=6`) | `1,84 → 12,45 MB`; **+19 GET list**, miss mọi revisit | `91,68 → 98,11 MB`; **+5 GET list** trên back | **NO-GO mặc định** (giữ làm đường **rollback 1 dòng** an toàn) |

- **Lý do kỹ thuật của ngưỡng (không chép disk quota `4 MiB / 512 KiB`):**
  - `max_entry_bytes = 1 MiB` (`1.048.576 B`): đứng đúng tại ranh giới giữa tài liệu nguyên khối/thơ 200k scalar (`vi_nfc-200k` 453 KiB, `poem_tabs-200k` 552 KiB, `emoji-200k` 801 KiB — `.clone()` 7–110 µs, tất cả được giữ) và vùng phân mảnh cực đoan `tiny_blocks` (`30k` = 1,47 MiB, `100k` = 4,90 MiB, `200k` = 9,80 MiB proxy — `.clone()` 2,4–7,5 ms, tăng ~10,55 MB WASM thực mỗi bài, bỏ qua lưu để đọc bình thường không tích lũy RAM).
  - `max_total_bytes = 2 MiB` (`2.097.152 B`): giữ đủ 16 bài `#234` (~16 KiB) hoặc ~4 tiểu thuyết `vi_nfc-200k`, giới hạn retention gần ~1× WASM heap baseline ấm (`1,84–3,15 MB`) thay vì phình 7–156 MB.
- **Sửa ownership đi kèm:** `Store::observe` rút bản sao ra khỏi `InMemoryCache` khi đang mount (`total_bytes == 0` với key đang đọc); `Store::seed` bỏ clone thừa khi ghi đè và xóa bản cũ trong cache; `Store::edit`/`edit_all` trả snapshot không quan sát về lại `SnapshotCache` thay vì kẹt ngoài cache trong `Entry::data`; `Coordinator::finish` đẩy fetch đã hủy waiter cuối (`abandoned`) qua `park_unobserved` thay vì rò rỉ payload quá cỡ.
- **Rollback an toàn:** chuyển `Coordinator::new` về `RetentionBudget::UNBOUNDED` (chính sách 16-entry cũ) hoặc `Coordinator::with_cache(Policy::V1, NoOpCache)` — 1 dòng, không đổi schema, API hay `ENABLED=false`.

## Bằng chứng kiểm tra cuối (trên cây đã khôi phục sạch sau negative control)

- **Lệnh Rust / contract / repository (`./tools/local-budget`):**
  - `cargo xtask verify unit web --filter removal_blocks_older_seeds_without_preventing_republication`: đối chứng âm fault-injection ([negative-control.log](negative-control.log)) **`FAIL` (`0 passed; 1 failed`)**; sau khôi phục `freshness.rs` **`PASS` (`1 passed; 0 failed`)**.
  - `cargo xtask verify unit web --filter public_reads`: **`PASS` (`14 passed; 0 failed; 1 ignored` `measure_237_corpus_and_retention_tradeoff`)** — phủ AC-03/AC-04 conformance cho 3 cấu hình cache (`InMemoryCache::default`, `READ_CACHE_BUDGET`, `NoOpCache`), 50 vòng điều hướng, `abandoned` waiter và race eviction/removal.
  - `cargo test --locked -p vink-web --lib --features ssr`: **`PASS` (`240 passed; 0 failed; 1 ignored`)**, gồm `read_lifecycle::replay::*` (`2/2`).
  - `cargo xtask verify quick`: **`PASS`** (fmt, clippy server/browser `wasm32`/UI preview `wasm32`, doctests `25 + 34 compile_fail`, `reader_ssr_boundary` `1`, unit/contract toàn workspace gồm `vink-app` `344 + 4`, `vink-identity` `41 + 2`, `vink-web` `240 / 1 ignored`, `xtask` `83 / 1 ignored`, repository checks decisions/skills/links/boundaries).
  - `xtask decision validate` (`127 revisions; 88 declared references`) & `xtask decision refs` (`problems: []`): **`PASS`**.
- **Lệnh Web / Chrome release (`VINK_CHROME` + PostgreSQL 17 scratch, chạy trực tiếp ngoài launcher vì browser là `UNSUPPORTED` theo [local-resource-budget.md](../../local-resource-budget.md)):**
  - `cargo xtask verify web --scenario main --scenario discovery --scenario review --scenario perf-baseline --scenario return-position --release`: **`PASS`** cả 5 scenario. Ở [perf-baseline-after.json.gz](perf-baseline-after.json.gz), `reader_client_nav_ms` median `47,2 ms`, `discovery_return_ms` median `31,15 ms`, warm transport `{reader:200=1, public-endorsement:200=20, public-discussion:200=10}`, **`0` `public-list`** GET; ở [return-position-after.json.gz](return-position-after.json.gz), **`50/50`** vòng pass (`max error_px = 0,015625`, `nodes = 917`).
- **Đo corpus/probe `--release` riêng cho #237:** `measure_237_corpus_and_retention_tradeoff` ([corpus-and-sim-release.csv](corpus-and-sim-release.csv)) và probe Chrome A/B/C ([probe-abc-chrome.json.gz](probe-abc-chrome.json.gz)) đã chạy ở `--release`/`wasm-release` trước bước inject/khôi phục đối chứng âm trên cùng logic `Option B`.
- **Chưa chạy (`NOT_RUN`):** `storage`, `restart`, `endorsement`, `auth`, `password`, `publication`; `verify discussion` (Quint CLI độc lập), `verify expressions`, `verify kani`, `verify postgres`, và CMP/native (`NOT_IMPLEMENTED`).
