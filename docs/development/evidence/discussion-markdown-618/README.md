# Markdown trong Nhận xét — #618

Baseline Product trên Mac mini tại `3bbeefb11e10cc4d49d09781c1b5ea901d888cb5`. Chrome 158.0.8127.89, PostgreSQL 17 scratch, một Cargo job và một Rust test thread. Reader dùng tác phẩm mẫu `/tac-pham/33333333-3333-4333-8333-333333333333` qua App local. Cặp ảnh guest cuộn tới Nhận xét ở 390 × 844, Light, chữ 100%.

- [Ảnh guest baseline](baseline/618-guest-before-interaction.png), [ảnh gõ baseline](baseline/618-root-editing.png), [context](baseline/contexts.json).
- [Dữ liệu thô Reader](baseline/comment-budget.json), [Bàn viết](baseline/desk-core-budget.json), [fixture transaction](baseline/editor-markdown-p95.json). Chuỗi 4.000 scalar / 200 dòng có SHA-256 `464698cf884a875d91d64bafe7faf7d9ee18b09ffa1688d44837d3866eaa3478`.
- Baseline textarea ready p50/p95 cold 12,4/13,7 ms và warm 10,5/10,7 ms. Cửa sổ gõ bắt đầu sau khi lượt xem foreground đã ghi xong. Trace ghi 0 request trong khi gõ và 0 request CodeMirror trước tương tác. Đo 5 cặp cold/warm sau 2 cặp làm nóng, xử lý input và caret 40 mẫu sau 5 lần làm nóng, trên cùng trình duyệt không throttle.
- [Asset baseline](baseline/bundle.json), [asset thay đổi](final/bundle.json). Raw 370.168 → 370.762 byte, gzip CLI `-n -9` 125.847 → 126.042 byte, Brotli 11 107.560 → 107.677 byte.
- [Patch phép đo cho baseline](baseline/harness.patch.gz). Áp dụng lên checkout tại SHA baseline bằng `gzip -dc /path/to/harness.patch.gz | git apply -` trước khi chạy cùng scenario release.

## Kiểm thiết bị

Telex/VNI qua hệ điều hành còn **NOT_RUN**. Mac có bộ gõ Vietnamese Simple Telex nhưng phiên này thiếu quyền Accessibility và Post Event. Chưa xác nhận có điện thoại kết nối. [Khả năng của runner](final/ime-capabilities.json).

Để chạy lại, mở Reader của App local bằng Chrome hoặc Safari có bàn phím hệ điều hành, chọn Simple Telex hoặc VNI. Trên điện thoại dùng thiết bị thật với bàn phím Việt, mở cùng bản build qua mạng thử nghiệm. Tại Nhận xét và Trả lời, giữ đang ghép dấu khi module được tải chậm, di chuyển caret, đổi vùng chọn, kết thúc composition kể cả trường hợp nội dung giữ nguyên rồi xuống dòng. Kiểm bàn phím còn mở, chữ và selection còn nguyên, Gửi chỉ có hiệu lực sau khi kết thúc composition và bấm nút. Ghi thiết bị, OS, trình duyệt, bộ gõ và kết quả từng bước.
