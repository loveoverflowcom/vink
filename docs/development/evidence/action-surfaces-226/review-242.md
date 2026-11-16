# Review độc lập #242

Review base develop `11c6a9c` (gồm #228 contract và #244 Desk/Discovery), head gốc
`fd0c7808d94c48bbb99c8dcd6cd5c911ed87f34a`. Merge-base gốc `a81333c`.
Clone riêng bảo toàn checkout chính và worktree #204 có 30 mục chưa commit.
Merge lịch sử #242 vào base mới không conflict, gồm cả feature-map của #244.

## Findings đã sửa

- P3 — Assertions count mới dùng `startsWith('1')`, chấp nhận cả count 10/100 khi
  expected là 1. Đổi bốn assertion #226 sang so sánh chính xác span hiển thị count;
  giữ accessible count unit và kiểm tra node/focus hiện có.
- P2 — Handoff/spec hiện tại nói thiếu contract #224, khiến continuation bị chặn sai
  sau #228. Đã đối chiếu API Rust/CMP và cập nhật dependency, ghi AC-10/AC-11 và
  CHOICE-03 mới trong issue R2. Giữ nguyên evidence cũ với nhãn lịch sử rõ ràng.

`ActionSurface`/forwarding đã có; default Standard. `FilterList` có nhưng `Reply`
chưa có trong shared registry. Không fork primitive, không đổi renderer hoặc policy.
Không phát hiện regression sản phẩm từ diff spec/tests này; UI #226 vẫn
**NOT_IMPLEMENTED**, toàn bộ AC-01..AC-11 chưa nghiệm thu.

## Kiểm chứng lượt review

Kết quả sẽ được ghi sau khi chạy trên source cuối. Không dùng baseline cũ làm PASS mới.
UI mới/paired capture, CMP/native, OS IME và screen reader: NOT_RUN vì không triển khai UI.
GitHub CI: NOT_RUN theo yêu cầu; không deploy.

## Tích hợp

PR review mới chứa lịch sử #242 cùng sửa đổi review, là PR thay thế để merge đúng một lần.
#242 được đóng là superseded sau khi PR thay thế merge; không đánh dấu #226 hoàn thành.
Continuation triển khai UI phải bắt đầu từ develop đã tích hợp phần chuẩn bị này.
