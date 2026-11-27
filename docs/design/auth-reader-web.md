# Reader · Đăng nhập để nhận xét hoặc lưu bài (Auth Web R3.1, #559)

**Intent.** Người đọc đang viết nhận xét gốc hoặc phản hồi và muốn đăng nhập ngay tại bài.
Quay lại đúng ô soạn sau khi phiên đã được đối soát, không mất chữ hoặc bị gửi thay.

**Reference.** [Auth R3.1](https://github.com/loveoverflowcom/vink/issues/559) và
[HTML preview đã duyệt](https://drive.google.com/file/d/1PC5meJCs_J_5ig3jwaZT-OEP6RzSAUY3/view)
định nghĩa phân cấp hành động hiện hành. [03-dang-nhap-reader.png, 390×844 tối](https://drive.google.com/file/d/1Uef6IlEtkxQ_trMB8rw0JLz2nh9RlqZb/view)
thuộc bộ R2, được duyệt ngày 19/11/2026 và ghi trong [issue #328](https://github.com/loveoverflowcom/vink/issues/328).
Đây là mốc lịch sử. [Form Auth chung](auth-password-web.md) sở hữu trường FloatingTonal,
thứ tự hành động, lỗi và đối soát phiên.

| Vùng | Bố cục và thứ tự đọc |
| --- | --- |
| Reader + composer | Giữ bài và ô soạn phía sau scrim. Khách và thành viên cùng dùng nút Gửi cho nhận xét gốc và phản hồi. Khi khách có nháp đủ điều kiện và bấm Gửi, Reader mở Auth theo ngữ cảnh. Phiên đã xác thực nhưng thiếu bút danh vào thẳng bước Chọn bút danh. |
| Sheet/dialog | Logo Vink căn giữa, Đóng ở góc trên bên phải. Tiêu đề và lời dẫn theo mục đích, căn giữa trước form. Tên đăng nhập → Mật khẩu → Đăng nhập (Filled) → Dùng mã email (Tonal, thư) → Chưa có tài khoản? Tạo tài khoản (Text). Khi đăng ký, Đã có tài khoản? Đăng nhập dùng Text. Ở bước nhập email, Gửi mã đăng nhập (Filled) đứng trước Dùng mật khẩu (Tonal, khóa). Các bước chuyển phương thức, OTP và bút danh dùng form chung. |
| Responsive | Sheet chạm đáy viewport dưới 760 CSS px. Từ 760 CSS px trở lên, dialog căn giữa, rộng tối đa 30rem. Vùng form cuộn dọc bên trong dialog, chừa lề 20–24 px và giữ tiêu đề nhìn thấy khi mở ở 320 CSS px/chữ gốc 200%. Tab tới được nút cuối. |

**Interaction.** Native dialog làm nền inert. Kích hoạt Gửi bằng chuột, chạm hoặc bàn phím mới mở dialog, focus đầu tại Đóng. Focus ô soạn, nhập chữ, Enter xuống dòng và mở ô trả lời không mở Auth.
Tab và Shift+Tab không rời modal. Escape, Đóng và Back qua điều hướng trong ứng dụng trả focus
về nút Gửi đã gọi hoặc về ô soạn nếu nút đã biến mất. Sau khi phiên và phần riêng được đối soát, trả focus về đúng textarea đã mở modal. Người viết bấm Gửi lần nữa để tạo nhận xét. Phần riêng
có thể xuất hiện trước một nhánh; giữ vị trí của ô soạn so với viewport thay vì đơn thuần giữ
`scrollY` khi bố cục phía trên ô thay đổi. Việc trả focus không dựng lại input; nếu ngữ cảnh
cũ không còn hợp lệ, không phục hồi quyền gửi cũ.
Không dùng animation làm điều kiện đóng, không thêm vuốt để đóng hoặc nút recovery.

**Hai điểm vào Reader.** #388 dùng lại dialog từ nút Lưu và truyền mục đích, điểm trả focus
và điều kiện xác nhận phiên từ nơi gọi. [LayoutSpec Lưu bài](saved-works.md) sở hữu copy và hành vi
đó. #388 từng dùng viền 2 px cho hai nút phụ. R3 chuyển sang Tonal và R3.1 dành Tonal cho lựa chọn
phương thức. Khi đăng nhập từ composer, focus về đúng ô soạn. Intent từ nút Gửi thuộc #328.

**Evidence.** So sánh cùng cảnh Reader trước/sau ở desktop sáng và mobile tối. Smoke Auth
kiểm computed style trong cả hai theme, dialog/sheet ở 320/390/834/1440 CSS px và cỡ chữ
100/150/200%, forced colors, trap focus, Escape/Back và giữ nháp. Xem
[cách chạy Auth Web](../development/auth-web.md#kiểm-chứng).

[Ảnh Chrome và kết quả Reader R2 theo AC](evidence/auth-r2-328/README.md).
