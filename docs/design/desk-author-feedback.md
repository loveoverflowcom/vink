# Bàn viết — trạng thái và số liệu bản công khai (#214)

LayoutSpec r1, hướng triển khai theo issue #214, kế thừa compact R6/#213. Mục tiêu: tác giả
quét được trạng thái và hai số riêng trên từng tác phẩm; điểm chính vẫn là mở bản nháp để viết.
Nguồn: `develop@b28e4bdd42eb84c1f45a7ebff4b983cfbc9f9567`, `DeskIndex`, pattern
`AuthorWorkRow`; luật và data flow ở [registry](../development/desk-author-feedback.md).
Reference: ảnh thư mục issue là **historical regression input**, đã mở ảnh desk-list-1440
web-013; không golden, không có mốc mới được duyệt (`NO_APPROVED_REFERENCE`).

```text
Screen #desk — Bàn viết
  Region #heading — Bàn viết | Tạo bản thảo
  Region #drafts — Tác phẩm của bạn
    Row #controls — tìm theo tên bản thảo | sắp xếp | tải lại
    Region #draft-results — một surface, một row/WorkId
      Column #work — tên (link editor khi có draft)
        trạng thái · Cập nhật <thời điểm máy>
        Thấm N · Nhận xét M (chỉ bản đang công khai)
        phạm vi / tên bản công khai nếu khác tên nháp
        link sibling: Đọc bản công khai
    Region #continuation — tải thêm / lỗi / phần tiếp / hết
  Region #privacy — nháp riêng; số liệu thuộc bản công khai; lưu không gửi duyệt
```

Tên > trạng thái/thời điểm > số liệu/phạm vi. Không resume, hero, pill hoặc tổng “tương tác”.
Bản công khai và bản sửa cùng tồn tại; pending/rejected mới không thay nhãn bản đang đọc.
Không có draft: hiện đủ tên, ghi rõ không có bản nháp để sửa; không tạo editor link giả.
Nháp/private/unknown không hiện zero giả. Public zero là `0`; unavailable là `—` kèm lý do.

320/390/834/1440 CSS px dùng cùng cấu trúc, metadata/số dài wrap, cuộn chính toàn trang;
không overlay/cuộn riêng. Hai theme theo token Lavender, body/caption phân tầng. 100% ưu tiên
quét gọn; 200% root font-size ưu tiên đọc/reflow và tới đủ hành động, không thay browser zoom.
Invariant: không cuộn ngang (sai số hình học tối đa 1 CSS px); tên Unicode đầy đủ trong DOM,
focus/hover mở toàn bộ tên draft; link ≥48px, không nested link/button.

Interaction: submit/clear/composition tiếp tục do SearchField và ReadOwner hiện có sở hữu.
Refresh/reconnect là đọc, giữ input identity/text/selection và anchor khi còn hợp lệ, không
phát mutation. Snapshot giữ lại khi lỗi đọc cùng account có nhãn stale; 401/403/logout/đổi
scope xóa dữ liệu. Reduced motion không che state. OS IME/screen-reader cần kiểm riêng.

Ngân sách: tối đa hai vòng capture-review-fix; self-review theo năm chiều, kết quả và gap
ở registry. Ảnh chỉ chứng minh Visual; race, focus và privacy cần assertion trên runtime.
