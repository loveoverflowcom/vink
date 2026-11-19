# Reader · Đăng nhập để nhận xét (Auth Web R2, #328)

**Intent.** Người đọc đang viết nhận xét gốc hoặc phản hồi và muốn đăng nhập ngay tại bài.
Quay lại đúng ô soạn sau khi phiên đã được đối soát, không mất chữ hoặc bị gửi thay.

**Reference.** [03-dang-nhap-reader.png, 390×844 tối](https://drive.google.com/file/d/1Uef6IlEtkxQ_trMB8rw0JLz2nh9RlqZb/view)
thuộc bộ R2, được duyệt ngày 19/11/2026 và ghi trong [issue #328](https://github.com/loveoverflowcom/vink/issues/328).
Dùng làm mốc phân cấp, không dùng làm golden từng pixel. [Form Auth chung](auth-password-web.md)
sở hữu trường FloatingTonal, action, lỗi và đối soát phiên.

| Vùng | Bố cục và thứ tự đọc |
| --- | --- |
| Reader + composer | Giữ bài và ô soạn phía sau scrim. Khi khách đủ điều kiện, chỗ nút Gửi là “Đăng nhập để nhận xét”; nếu phiên cần bút danh, hiện “Chọn bút danh”. |
| Sheet/dialog | Thanh logo Vink bên trái, nút Đóng bên phải; tiêu đề “Đăng nhập để nhận xét” trước form. Một nút Filled chính, Tạo tài khoản và Dùng mã email là hai hành động text. Các bước chuyển phương thức và bút danh dùng form chung. |
| Responsive | Sheet chạm đáy viewport dưới 760 CSS px; từ 760 CSS px trở lên là dialog căn giữa. Vùng form cuộn dọc, không tràn ngang ở 320 CSS px/chữ gốc 200%; khi mở vẫn thấy tiêu đề, Tab tới được nút cuối. |

**Interaction.** Native dialog làm nền inert; click hoặc Enter vào CTA mở, focus đầu tại Đóng.
Tab và Shift+Tab không rời modal. Escape, Đóng và Back qua điều hướng trong ứng dụng trả focus
về điểm gọi. Khi chủ tài khoản đã xác nhận, trả focus về đúng textarea đã mở modal. Phần riêng
có thể xuất hiện trước một nhánh; giữ vị trí của ô soạn so với viewport thay vì đơn thuần giữ
`scrollY` khi bố cục phía trên ô thay đổi. Việc trả focus không dựng lại input; nếu ngữ cảnh
cũ không còn hợp lệ, không phục hồi quyền gửi cũ.
Không dùng animation làm điều kiện đóng, không thêm vuốt để đóng hoặc nút recovery.

**Evidence.** Đối chiếu mobile tối 390×844/100% với mốc được duyệt; Chrome cũng đo desktop sáng
và 320×800/tối/200% với reduced motion. Xem [cách chạy Auth Web](../development/auth-web.md#kiểm-chứng).

[Ảnh Chrome và kết quả Reader R2 theo AC](evidence/auth-r2-328/README.md).
