# Shared CodeMirror 6 — kiểm chứng #617

Baseline tại `7e5d784a262cdf6fc3aa16266126f6b45914ce87` (sau #635). Runtime cuối tại `3a8107aeea1f79005b95dde70f5f380661280d78`, kiểm trên `e8bff712fa63648a2eefdaca330b282467f13426` sau khi sửa oracle chờ import. Bốn tổ hợp của preset Document/Comment và body Plain/Markdown đi qua cùng asset. Bàn viết là caller Product. Comment là fixture của API sẽ được #618 tích hợp. [Hợp đồng JS/Rust và quyền sở hữu](../../../../apps/vink-web/editor/README.md).

## Số đo release

Mac mini arm64, macOS 15.6, Chrome 158.0.8127.89 qua CDP, Node 22.15.0, rustc 1.99.1, PostgreSQL 17 scratch loopback. Cả hai lượt chạy trên cùng checkout, một workload, một Cargo job, một Rust test thread. Số thời gian tính bằng ms, theo **p50 / p95**. Bộ số liệu thô gồm [Product trước](baseline/desk-core-budget.json), [Product sau](final/desk-core-budget.json), [fixture trước](baseline/editor-markdown-p95.json), [fixture sau](final/editor-markdown-p95.json), [artifact trước](baseline/bundle.json) và [artifact sau](final/bundle.json).

| Chỉ số | Baseline | Final |
| --- | ---: | ---: |
| Raw (byte) | 369425 | 370168 |
| gzip CLI -n -9 (byte) | 125573 | 125847 |
| Brotli 11 (byte) | 107324 | 107560 |
| Cold, navigation → view ready | 49.50 / 54.70 | 48.30 / 50.30 |
| Warm, navigation → view ready | 37.50 / 40.50 | 38.00 / 40.20 |
| Cold, resource duration | 1.50 / 1.80 | 1.50 / 1.70 |
| Warm, resource duration | 0.00 / 0.00 | 0.00 / 0.00 |
| Product, transaction gõ | 0.60 / 1.90 | 0.70 / 1.40 |
| Product, transaction caret | 0.30 / 0.70 | 0.30 / 0.50 |
| Product, beforeinput → rAF kế | 14.70 / 15.50 | 13.80 / 15.40 |
| Product, beforeinput → task sau rAF | 16.10 / 17.20 | 15.70 / 17.60 |
| Khoảng cách frame | 16.70 / 16.70 | 16.70 / 16.70 |
| Số long task (≥50 ms) | 0 | 0 |
| Số frame trễ ước tính | 0 | 0 |
| DOM view / host đánh dấu / callback cũ sau 40 vòng | 0 / 0 / 0 | 0 / 0 / 0 |
| Fixture 10k, gõ | 0.30 / 0.50 | 0.30 / 0.50 |
| Fixture 10k, caret | 0.50 / 1.00 | 0.50 / 1.00 |
| Fixture 50k, gõ | 0.30 / 0.40 | 0.30 / 0.40 |
| Fixture 50k, caret | 0.40 / 0.50 | 0.30 / 0.40 |
| Fixture 100k, gõ | 0.30 / 0.50 | 0.30 / 0.50 |
| Fixture 100k, caret | 0.40 / 0.60 | 0.40 / 0.60 |
| Fixture 200k, gõ | 0.50 / 0.70 | 0.50 / 0.70 |
| Fixture 200k, caret | 0.40 / 0.50 | 0.30 / 0.40 |


- Asset phát ra là `vink-editor.js`. Raw tăng 743 B (0,20%), gzip CLI tăng 274 B (0,22%), Brotli quality 11 tăng 236 B (0,22%). Các tệp bundle JSON ghi SHA-256, gzip CLI và các tuỳ chọn nén Node để tái đối chiếu. Receipt đầu vào của esbuild chỉ dùng rà dependency.
- Mỗi cặp cold/warm mở một document trình duyệt mới vào bản nháp Markdown đã lưu tại `/ban-viet/{id}`, 1440 × 900, 10.000 Unicode scalar tiếng Việt có emoji và dấu tổ hợp. Bỏ 2 cặp đầu, ghi 40 cặp. Cold xóa HTTP cache. Warm mở ngay sau cold. Mốc ready dùng MutationObserver bắt view mount. Mỗi lượt có đúng một resource editor. 40/40 cold có transferSize dương và 40/40 warm bằng 0 ở cả hai lần.
- Product đo dispatch gõ/caret đồng bộ trên view thật của Leptos (5 warmup + 40 mẫu), hoàn tác thay đổi bằng transaction đảo. Một phép đo riêng gửi CDP `Input.insertText` vào contenteditable (5 + 40), từ timestamp beforeinput đến rAF và task đầu sau rAF. Hai mốc này xấp xỉ cơ hội trình duyệt vẽ. Không có timestamp pixel lên màn hình. p95 task sau rAF tăng 0,40 ms. Mục tiêu p95 xử lý edit ≤16 ms áp dụng cho transaction. Một máy và profile như trên cho p95 cuối 1,40 ms.
- Long task từ PerformanceObserver trong loạt transaction. Ước lượng frame bỏ lỡ theo khoảng cách rAF so với trung vị 15 frame nghỉ. Fixture 10/50/100/200k đo dispatch/caret đồng bộ, 5 warmup + 40 mẫu mỗi cỡ; không bao gồm thời gian vẽ hay nối Leptos. 40 vòng mount/destroy kiểm DOM, marker của host và callback sau teardown. Heap/GC không được đo.
- Đã mở ảnh trước/sau. Ở 320 px tiêu đề và đoạn văn xuống dòng trong giấy. Tại 834 px có rail gọn; 1440 px giữ rail và thanh công cụ cạnh giấy. Cả bốn cặp PNG cùng SHA-256. [Ảnh và context baseline](baseline/) · [Ảnh và context final](final/). Text scale ở bốn ảnh dùng DOM override để kiểm hình học. Oracle AX đọc cây accessibility của Chrome. Xem [trace assertion trước](baseline/interaction-and-ax.txt) và [trace assertion sau](final/interaction-and-ax.txt) cho tên textbox, phím, paste, composition, undo và focus.

## Chạy lại

Từ gốc repo, dùng đúng toolchain trong repo, Chrome, PostgreSQL scratch qua `VINK_TEST_DATABASE_URL` và `VINK_CHROME`. Dùng một target riêng cho phiên, lưu output bên ngoài target trước khi clean. Lượt baseline dùng checkout sạch ở SHA trên. Sao chép [patch instrumentation](baseline/harness.patch.gz) ra ngoài checkout trước, rồi áp dụng `gzip -dc /path/to/harness.patch.gz | git apply -` vào baseline. Patch chỉ thêm phép đo vào e2e. Đo xong baseline mới chuyển sang source final. Cùng chạy tuần tự các lệnh sau:

```sh
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
node apps/vink-web/editor/scripts/build.mjs --check
python3.13 tools/local-budget run --low-disk -- cargo xtask build web --release
cargo xtask verify web --release --scenario editor-format
```

Sao chép JSON và ảnh từ thư mục `target/verify-web/<run>/editor-format` cho mỗi lượt. Đo file JS thực tế `target/site/pkg/vink-editor.js` trong artifact release bằng `wc -c`, `shasum -a 256`, `gzip -n -9 -c` và Node `zlib.brotliCompressSync` với `BROTLI_PARAM_QUALITY=11`. Rerun UI liên quan và Rust ở final:

```sh
cargo xtask verify web --release --scenario reader --scenario editor-chrome --scenario editor
cargo xtask verify web --release --scenario standalone
cargo test --locked --release -p vink-web --features ssr --lib editor::tests
```

**IME thật và thiết bị.** CDP composition và input commit tiếng Việt chạy trên Chrome desktop. macOS SimpleTelex có trong máy, runner thiếu Accessibility và PostEvent. Telex/VNI qua OS và Android/iOS thật là **NOT_RUN**. Để chạy lại, dùng phiên GUI có quyền gửi phím thực hoặc người gõ trực tiếp, bật Telex/VNI; trên điện thoại thật bật IME tiếng Việt và mở Bàn viết bằng tài khoản thử trong môi trường scratch. Gõ và sửa “Tiếng Việt” giữa dòng có marker, chọn ngược, dán, undo/redo, dùng toolbar/đổi focus, lưu lúc đang composition; so source và caret sau commit rồi mở lại để so văn bản đã lưu. Ghi OS, IME, browser và thiết bị.
