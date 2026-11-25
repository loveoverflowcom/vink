# Bàn viết khi mở và thu gọn ô tìm

Ảnh chụp ngày 25/11/2026 cho [#531](https://github.com/loveoverflowcom/vink/issues/531). Hai bản Web chạy trên Chrome headless 158.0.8127.89, macOS arm64, tại `/ban-viet` sau hydration. Cả hai dùng cùng bộ 24 tác phẩm giả lập của `draftDashboard`, scope thông báo giả lập và count bằng 0. Chuông hiện ở đầu trang mobile. Viewport cao 900 CSS px, DPR 1, theme tối, chữ 100%. Mốc so sánh là bản chạy tại máy.

| Viewport | Trước · `52e8dbd5` | Sau · `1de26346` |
| --- | --- | --- |
| 390 × 900 | [Chỉ còn ô tìm dưới tiêu đề](before-390.png) | [Giữ chuông, định dạng, tạo và sắp xếp](after-390.png) |
| 1440 × 900 | [Ô tìm thay nhóm thao tác](before-1440.png) | [Ô tìm ở hàng riêng dưới nhóm thao tác](after-1440.png) |

Trên màn 390 px, chuông giữ rect `x=311, y=32, w=48, h=48` CSS px khi đóng và mở. Trên màn 1440 px, ô tìm rộng 512 CSS px. Khi thu gọn sau khi gõ tiếp, kết quả và nhãn vẫn thuộc cụm đã gửi. [Ảnh bộ lọc đã áp dụng](applied-1440.png).

[Ảnh stress 320 px ở cỡ chữ 200%](open-320-text200.png) và ảnh bộ lọc dùng fixture danh sách thông thường của scenario Desk. Cỡ chữ là phần trăm trên phần tử gốc. Các ảnh lấy nguyên viewport từ Chrome. [Bố cục và trạng thái](../../../design/redesign-r4/layout-spec.md#51-header-tìm-và-phục-hồi).

| Chiều kiểm | Quan sát |
| --- | --- |
| Chức năng | Gõ và đóng/mở giữ nguyên số GET. Bỏ lọc đã áp dụng phát một GET về đầu danh sách. |
| Hình học | So rect trước/sau ở bảy bề rộng 320–1536 px, sáng/tối, chữ 100/150/200%. Sai số vị trí và kích thước cho phép của các điểm neo là 1 CSS px. |
| Thị giác | Ảnh cùng fixture cho thấy nút tạo và sort vẫn nhận ra được khi tìm. Đổi lại, hàng kết quả đầu thấp hơn khi mở. |
| Trợ năng và reflow | Searchbox có tên trong cây trợ năng. Kiểm vòng focus ở forced colors, vùng bấm và cuộn ngang. |
| Tương tác | Bàn phím, Back, đóng/mở nhanh, reduced motion và composition giả lập trên Chrome. IME hệ điều hành, thiết bị cảm ứng thật và đối chiếu pixel theo thiết kế đã duyệt chưa được kiểm. |
