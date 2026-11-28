# Markdown trong Nhận xét — #618

Reader của App local, tác phẩm mẫu `/tac-pham/33333333-3333-4333-8333-333333333333`. Số đo và ảnh lấy trên Mac mini, macOS 15.6 (24G84), Chrome 158.0.8127.89, PostgreSQL 17 scratch, một Cargo job và một Rust test thread. Bản gốc `3bbeefb11e10cc4d49d09781c1b5ea901d888cb5`. Bản đo baseline `438dc409002a2db5c1016f8a446d484f041ee701` chỉ thêm phép đo vào harness. Bản Product cuối đã chạy `5b720322d65f8428735c74edda2d98ff269e5c1d`. [Patch tái tạo baseline](baseline/harness.patch.gz).

## Qua giao diện người dùng

| AC | Kết quả trên Mac mini |
|---|---|
| 01 | PASS. Bấm Định dạng ở Nhận xét hoặc Trả lời gắn Comment Markdown của cùng runtime. Đã bấm cả năm nút, dùng phím B/I, undo/redo, clipboard, Enter rồi Gửi. Preview đổi theo dòng có caret. |
| 02 | Textarea SSR nhận chữ, focus và selection trước WASM. Đã giữ rồi cho tải hoặc làm hỏng request module thật. Khi người dùng đã nhập, giữ nguyên control và lịch sử native. Guest đọc và đọc lại nhận xét đã gửi đều tải 0 module. Bàn phím mềm vật lý chưa quan sát. |
| 03 | PASS. PostgreSQL nâng cấp hàng và receipt plain tồn tại trước migration, retry tương đương Unicode trả lại kết quả cũ, thay format xung đột. Client cũ thiếu format đọc/ghi plain, Markdown lưu source NFC cùng format. Đã kiểm giới hạn scalar/dòng, khổ thơ, emoji và ZWJ. |
| 04 | PASS. Cặp root/reply Markdown và plain có dấu Markdown xuất hiện trong HTML SSR lẫn bản hydrate khi đóng nhận xét. CSR standalone kiểm cùng projection tại biên contract. Fixture nguồn công khai được sắp trước trên DB scratch. Shared Reader render typed nodes, heading compact, giữ chữ/nhãn ảnh, giới hạn độ sâu, chỉ URL cho phép. Corpus phòng thủ dùng ví dụ trơ. |
| 05 | PASS. Validation có source và chữ hiển thị, từ chối nội dung chỉ còn ký hiệu định dạng. API moderation trả source, format và chữ hiển thị theo quyền. Kiểm PG thật cho idempotency, thu hồi quyền, scope, quyết định, cache/public và dữ liệu riêng của tác giả. Lượt gửi qua Product vào hàng chờ thật. |
| 06 | PASS. Auth local trả về đúng nháp. Lỗi sau commit 201 và retry dùng đúng request đã chụp, bao gồm key và format, ACK giữ nguyên nháp mới. Case controller dùng request plain đang chờ rồi sửa nháp mới Markdown. Đổi session, readback, đóng/mở, mất đích và copy nằm trong các suite Discussion/Reader. |
| 07 | Đã chạy Product release với App, Identity, API, PG. Đã mở đủ 16 ảnh Light/Dark ở 320, 390, 834, 1440 CSS px × chữ 100%/200%, xác nhận editor trong khung và trang không tràn ngang. Mức chữ dùng DOM override cho hình học. AX ghi tên ô và đích trả lời. Bàn phím mềm thật thuộc phần còn thiếu. |
| 08 | PASS theo ngân sách trên runner này. Bảng dưới dùng artifact sau build, dữ liệu từng sample và cache profile giống nhau. |
| 09 | **NOT_RUN trên OS/điện thoại thật**. CDP composition đã chạy cả rich và native khi giữ request. Hướng chạy thiết bị ở cuối. |
| 10 | SSOT, LayoutSpec, feature map, HTTP fixtures, hướng dẫn editor và phân luồng kiểm thử đã cập nhật. Authority và chính sách quyết định hiện hành vẫn áp dụng. Một PR cho lát này. |

[Trích kết quả test thực chạy](test-results.txt) · [AX và lifecycle tài khoản](final/comment-lifecycle.json) · [Dữ liệu hình học của 16 ảnh](final/contexts.json).

Cặp guest có dữ liệu, Light 390 × 844, 100%, cuộn đến Nhận xét. [Trước khi tương tác, baseline](baseline/618-guest-before-interaction.png) · [sau thay đổi](final/618-guest-before-interaction.png). Khi nhập 4.000 scalar, xem [textarea trước](baseline/618-root-editing.png) và [rich editor sau](final/618-root-editing.png). Với nháp phản hồi đã qua 40 lần thu/mở, xem [320 px, chữ 200%, Dark](final/618-reply-320-200-dark.png) và [desktop, chữ 100%, Light](final/618-reply-1440-100-light.png). Mỗi ảnh có SHA nguồn, viewport và rect trong context cạnh ảnh.

## Chi phí trên Product release

Reader chạy corpus 4.000 scalar / 200 dòng, SHA-256 `464698cf884a875d91d64bafe7faf7d9ee18b09ffa1688d44837d3866eaa3478`. Cùng checkout build, thiết bị, trình duyệt, profile cache và helper. Không throttle CPU/network. Mỗi cặp mở document mới, xóa HTTP cache cho cold, giữ cache cho warm, bỏ hai cặp đầu, đo năm cặp. Sau khi nạp corpus và request ghi lượt xem foreground kết thúc, đo 40 lần gõ/caret sau năm lần làm nóng. Các mốc rAF/task là proxy cho lịch paint. Phân vị dùng nearest rank.

| Phép đo (ms, p50 / p95) | Baseline | #618 |
|---|---:|---:|
| Điều hướng → textarea, cold | 11.9 / 13.9 | 14.3 / 14.9 |
| Điều hướng → rich-ready, cold | — | 203.2 / 212.3 |
| Bấm Định dạng → rich-ready, cold | — | 20.7 / 22.6 |
| Điều hướng → textarea, warm | 9.6 / 10.5 | 12.6 / 13.9 |
| Điều hướng → rich-ready, warm | — | 201.4 / 202.4 |
| Bấm Định dạng → rich-ready, warm | — | 24.5 / 25 |
| beforeinput → sau input | 2.8 / 3.3 | 3.2 / 4.1 |
| beforeinput → rAF kế | 14.8 / 17.2 | 15.3 / 16.9 |
| beforeinput → task sau rAF | 16.4 / 18 | 16.9 / 18.5 |
| Phím caret → rAF | 14.9 / 16.6 | 15.6 / 16.5 |
| Bàn viết, cold-ready | 49.4 / 61 | 49.1 / 60.2 |
| Bàn viết, warm-ready | 37.7 / 39.6 | 38 / 40.3 |
| Bàn viết, xử lý edit | 0.8 / 1.6 | 0.6 / 1.5 |
| Bàn viết, input → rAF | 14.8 / 15.6 | 13.9 / 16.1 |

Trace ghi **0 request CodeMirror trước tương tác**, 0 request khi gõ ở cả hai bản. Sau kích hoạt, tối đa một lần tải module mỗi document ở bản mới. Idle rAF median 16.6 → 16.7 ms, ước lượng missed frames 0 / 546 → 0 / 567 khoảng rAF. Tổng layout shift không gắn input gần đây 0.01364 → 0, có input gần đây 0 → 0.21432. Long tasks ≥50 ms từ navigation qua khi chèn corpus và đo xong ở warm document cuối (ms) [90] → [145]. Trace này không tách từng long task theo thao tác. [Raw baseline](baseline/comment-budget.json) · [raw final](final/comment-budget.json) · [số liệu tổng hợp](paired-summary.json).

JS phát hành `target/site/pkg/vink-editor.js`, byte raw 370168 → 370762 (+0,16%), gzip `-n -9` 125847 → 126042, Brotli 11 107560 → 107677. SHA-256 và kích thước lấy trên file output. [Baseline](baseline/bundle.json) · [final](final/bundle.json).

### Lifecycle

Guest trên cùng route, lấy counters sau GC trước soạn, sau 40 lần thu/mở cùng nháp và ngay sau SPA unmount. Owner trong bảng là host của ô phản hồi trong DOM. Document, nodes, listeners và heap là số toàn trang, bao gồm module đã import và trang đích sau điều hướng.

| Phiên bản / thời điểm | Document | DOM node | Listener | JS heap sau GC (byte) | Ô phản hồi / view CM |
|---|---:|---:|---:|---:|---:|
| baseline / before | 3 | 593 | 81 | 2352944 | 0 / 0 |
| baseline / reopened | 4 | 705 | 86 | 2438504 | 1 / 0 |
| baseline / after | 4 | 511 | 82 | 2640984 | 0 / 0 |
| final / before | 3 | 658 | 88 | 2462716 | 0 / 0 |
| final / reopened | 4 | 850 | 147 | 4112356 | 1 / 1 |
| final / after | 4 | 440 | 78 | 4200188 | 0 / 0 |

Bàn viết dùng route riêng, corpus 10.000 scalar, đo readiness, xử lý, input frame và teardown. Sau unmount còn 0 host/view/callback cũ. Fixture Markdown 10k/50k/100k/200k đo riêng synchronous transaction, năm warmup rồi 40 samples mỗi kích cỡ. [Bàn viết trước](baseline/desk-core-budget.json) · [sau](final/desk-core-budget.json) · [fixture trước](baseline/editor-markdown-p95.json) · [sau](final/editor-markdown-p95.json).

## Tái chạy

Cần toolchain của repo, Chrome, PG17 loopback và database scratch mà runner được quyền tạo DB test. Đặt `DATABASE_URL` cho SQLx và `VINK_TEST_DATABASE_URL` cho Product runner cùng trỏ vào scratch cục bộ. Chạy tuần tự qua `python3.13 tools/local-budget run --low-disk --`, từng lệnh bên dưới. Baseline Reader dùng checkout tại SHA gốc và `gzip -dc /path/to/harness.patch.gz | git apply -`.

```sh
cargo test --release -p vink-app --test postgres discussion::
cargo test --release -p vink-app --test postgres fresh_database_migrates_to_current_schema
cargo test --release -p vink-app --test postgres decision_details_are_private_to_the_owner_and_the_writer
cargo test --release -p vink-app --lib http::contract::
cargo test --release -p vink-web --lib
cargo xtask verify web --release --scenario reader
cargo xtask verify web --release --scenario discussion
cargo xtask verify web --release --scenario auth
cargo xtask verify web --release --scenario standalone
cargo xtask verify web --release --scenario editor-format
cargo xtask ui-preview leptos verify
```

## Kiểm thiết bị

Telex/VNI qua hệ điều hành còn **NOT_RUN**. Mac có bộ gõ Vietnamese Simple Telex nhưng phiên này thiếu quyền Accessibility và Post Event. Chưa xác nhận có điện thoại kết nối. [Khả năng của runner](final/ime-capabilities.json).

Để chạy lại, mở Reader của App local bằng Chrome hoặc Safari có bàn phím hệ điều hành, chọn Simple Telex hoặc VNI. Trên điện thoại dùng thiết bị thật với bàn phím Việt, mở cùng bản build qua mạng thử nghiệm. Tại Nhận xét và Trả lời, giữ đang ghép dấu khi module được tải chậm, di chuyển caret, đổi vùng chọn, kết thúc composition kể cả trường hợp nội dung giữ nguyên rồi xuống dòng. Kiểm bàn phím còn mở, chữ và selection còn nguyên, Gửi chỉ có hiệu lực sau khi kết thúc composition và bấm nút. Ghi thiết bị, OS, trình duyệt, bộ gõ và kết quả từng bước.
