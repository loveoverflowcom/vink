# LayoutSpec · Xóa tài khoản · R5

Nguồn nghiệp vụ [#297 · DEL-01–08](https://github.com/loveoverflowcom/vink/issues/297). Hướng Quiet Lavender của Tài khoản, `NO_APPROVED_REFERENCE`.

[#605](https://github.com/loveoverflowcom/vink/issues/605) thay vị trí điểm vào của r1.
Thành viên Active chọn **Xóa tài khoản** trong Cài đặt. Khi đã đăng xuất hoặc bị đình chỉ,
chọn **Hỗ trợ quản lý tài khoản** trong tiện ích trang Tài khoản. Cả hai dẫn tới
`/toi?quan-ly=xoa`, có Back về `/toi` và nút mở xác nhận riêng. Trang Login, Register,
OTP, Chọn bút danh và Auth trong Reader không dựng biểu mẫu xóa. Đường dẫn trực tiếp không cấp phiên.
Vùng xác nhận dùng TextField và Button chung, một cột với nút xuống hàng khi hẹp hoặc phóng chữ.

Biểu mẫu báo rõ dữ liệu tác phẩm bị xóa vĩnh viễn. Người dùng chọn mật khẩu hoặc mã email đã xác minh. Mật khẩu dùng tên đăng nhập và mật khẩu hiện tại. Mã dùng địa chỉ email, nút gửi mã và trường mã một lần ở bước sau. **Xóa vĩnh viễn** là nút xác nhận cuối cùng. **Hủy** đóng biểu mẫu và trả focus về nút mở. Pending khóa submit và cho công nghệ hỗ trợ biết tiến độ. Lỗi để người dùng sửa thông tin hoặc xin mã mới. Sau khi tiếp nhận, làm mới trạng thái phiên và báo kết quả ngay trong vùng quản lý xóa, kể cả sau khi phiên đã thành khách.

Reader dùng vùng thay thế hiện có với nhãn bài đã xóa, không in tiêu đề cũ, tên hay liên kết tác giả. Bình luận hợp lệ giữ quy tắc của [Social r2](social-289.md).
