# LayoutSpec · Xóa tài khoản · r1

Nguồn nghiệp vụ [#297 · DEL-01–08](https://github.com/loveoverflowcom/vink/issues/297). Hướng Quiet Lavender của Tài khoản, `NO_APPROVED_REFERENCE`.

Trong `/toi`, đặt nút **Xóa tài khoản** ở cuối nhóm xác thực. Cùng điểm vào xuất hiện khi đã đăng xuất hoặc bị đình chỉ. Mở biểu mẫu ngay tại vùng đó và đưa focus đến tiêu đề. Dùng TextField, nút và vùng trạng thái của Tài khoản. Giữ lề/rộng tối đa hiện có, xếp một cột và để nút xuống hàng khi hẹp hoặc phóng chữ.

Biểu mẫu báo rõ dữ liệu tác phẩm bị xóa vĩnh viễn. Người dùng chọn mật khẩu hoặc mã email đã xác minh. Mật khẩu dùng tên đăng nhập và mật khẩu hiện tại. Mã dùng địa chỉ email, nút gửi mã và trường mã một lần ở bước sau. **Xóa vĩnh viễn** là nút xác nhận cuối cùng. **Hủy** đóng biểu mẫu và trả focus về nút mở. Pending khóa submit và cho công nghệ hỗ trợ biết tiến độ. Lỗi để người dùng sửa thông tin hoặc xin mã mới. Sau khi tiếp nhận, làm mới trạng thái phiên và báo kết quả tại `/toi`.

Reader dùng vùng thay thế hiện có với nhãn bài đã xóa, không in tiêu đề cũ, tên hay liên kết tác giả. Bình luận hợp lệ giữ quy tắc của [Social r2](social-289.md).
