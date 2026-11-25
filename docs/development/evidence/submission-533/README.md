# Bàn viết khi gửi bài còn thiếu tiêu đề và nội dung

Hai ảnh chụp từ Chrome trên macOS ở commit `3e2e69b7a7879c46ff7c063e5041b6f5a090914b`. Tác giả mở bản nháp Mặc định, chọn Công khai rồi nhấn Enter trên nút gửi. [Máy tính, 1024 px, tối, 100%](1024-dark-100.png) · [Màn hẹp, 320 px, tối, 200%](320-dark-200.png).

| Chiều tự review | Kết quả trên Chrome |
| --- | --- |
| Chức năng | PASS cho Mặc định và Markdown. Trong lần bấm thiếu nội dung, số POST chụp revision và gửi duyệt đều bằng 0. Thêm một chữ Việt dạng NFD rồi chủ động gửi tạo một revision, kể cả khi mất phản hồi chụp. |
| Bố cục | PASS ở 320, 390, 1024 CSS px, hai theme, chữ 100% và 200%. Runner đo tràn ngang, chữ bị cắt và vị trí các nút trong panel. |
| Thị giác | Đã mở ảnh thật ở 1024 px tối, 390 px sáng và 320 px tối. Dấu focus ở lề trái cạnh nhãn. Lỗi nằm tại trường và nút gửi vẫn hiện trong phần tùy chọn. Chưa có ảnh chuẩn được duyệt cho trạng thái này. |
| Tiếp cận và màn hẹp | PASS cho thứ tự focus, liên kết mô tả lỗi ở CodeMirror, một vùng thông báo polite và reflow với root font-size 200%. Chưa chạy VoiceOver hoặc đo tương phản cho riêng hai ảnh này. |
| Tương tác | PASS cho Enter, sửa từng trường, giữ nguyên chữ đã gõ và gửi lại sau khi mất phản hồi. Chưa chạy bộ gõ tiếng Việt cấp hệ điều hành. |

Ảnh được chụp sau khi runner cuộn tới vùng công khai để đo hình học. [LayoutSpec](../../../design/publication-author.md) · [Kịch bản trình duyệt](../../../../apps/vink-web/e2e/publication-author-smoke.mjs).
