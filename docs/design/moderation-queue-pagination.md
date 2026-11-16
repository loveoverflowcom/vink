# Hai hàng chờ kiểm duyệt hữu hạn (#205)

Intent: người duyệt đi tiếp trong đúng phạm vi quyền hiện tại, hiểu lỗi và có thể đối soát từ đầu.
References: source `develop@6de515d`, regression reference; `NO_APPROVED_REFERENCE` cho lát này.

```text
moderation-home
  case-queue: H2 → Tải lại ca tác phẩm → status/error → danh sách link → Tải thêm/Bắt đầu lại
  appeal-queue: H2 → Tải lại phản đối → status/error → danh sách link → Tải thêm/Bắt đầu lại
  automation: trạng thái + lý do chưa gửi + lệnh có version (owner riêng)
```

Status rỗng không chiếm một hàng/khoảng trống sau khi đọc xong. Mỗi vùng có loading/initial/refresh/append/restart/error riêng. Empty chỉ sau response thành công;
403/401 xóa dữ liệu của vùng. Lỗi tạm giữ dữ liệu/cursor; lỗi cursor cho nút Bắt đầu lại, không giả
làm hết trang. Refresh thay cả chuỗi và nói rõ bỏ trang sau; không suy mục đã được xử lý vì vắng
trong một trang. Thứ tự cũ nhất trước, không đổi ưu tiên.

Interaction: nút native/shared có tên riêng từng queue, Tab/Enter/Space, DOM theo thứ tự đọc.
Append giữ các link cũ theo ID. Nút continuation ở vị trí ổn định khi có trang sau; hết trang
phải đưa focus về nút tải lại cùng vùng nếu focus đang ở continuation bị gỡ. Tải lại giữ focus
nút/anchor hiện có. Paging không remount automation hoặc xóa lý do; chỉ session change xóa form.
Online/focus/pageshow là hint đọc có backoff, không chạy lại mutation. Không có animation mới.

Invariants: không cuộn ngang ở 390px và root text 100%/200%; mọi CTA và lỗi còn tới được; mỗi
queue chỉ phản ánh response ticket/session hiện tại; form lý do không mất chữ/focus khi paging.
Giữ Quiet Lavender và shared controls. Tối đa hai vòng sửa có render; mở ảnh thật và báo riêng
Functional / Geometry / Visual / Accessibility-Responsive / Interaction. Programmatic composition
không chứng minh OS IME.

Mất quyền admin được xác nhận từ queue phản đối sẽ gỡ controls automation và xóa lý do riêng; paging thành công thông thường không tải lại automation.
