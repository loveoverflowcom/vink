# Khám phá · Trạng thái chưa theo dõi ai · R1

Chrome 158.0.8127.88 trên Linux, DPR 1, viewport cao 900 CSS px cho ma trận ảnh.
Hai lượt chụp dùng cùng fixture native password với App, Identity và PostgreSQL scratch
17.11. Mức 200% là font gốc đặt qua CDP. Bản trước dùng `db8a95d1` với phần
chụp ma trận bổ sung vào smoke. Bản Web/SSR sau dùng `e1329347`, chạy fixture
và bài đo cập nhật tại `d7f8faa1` với artifact đã kiểm hash. Phần runtime giữa
hai SHA sau giống nhau. Lượt trước là `1795616765878424688-31986`, lượt sau là
`1795618773643336498-97518`. [LayoutSpec R1](../../../../design/social-289.md#following-empty-cta-r1--355)

- Trước và sau trên mỗi chiều rộng · [320 px](truoc-sau-320.jpg) ·
  [390 px](truoc-sau-390.jpg) · [834 px](truoc-sau-834.jpg).
- [Focus của CTA ở 320 px/200%](focus-cta-320.png) ·
  [H2 nhận focus sau Reader → Back](focus-sau-back.png) ·
  [Đã theo dõi, chưa có bài](da-theo-doi-chua-co-bai.png).

Ảnh 200% cuộn dọc khi cần để CTA hiện trọn phía trên thanh dưới. Các khung R1
đi kèm issue là concept `DESIGN_PROPOSED / NO_APPROVED_REFERENCE`.
[Issue #355](https://github.com/loveoverflowcom/vink/issues/355)

| Chiều | Kết quả |
| --- | --- |
| Functional | Thử trên API thật với thành viên mới, trạng thái đã theo dõi nhưng chưa có bài và bài thơ có trước lần Follow. Link mở đúng mode Tác giả. |
| Geometry | 12/12 cấu hình của trạng thái rỗng có 0 px tràn ngang. CTA cao tối thiểu 48 CSS px, bán kính 14 px và chạm được phía trên thanh điều hướng. Menu Đang theo dõi của develop từng gây tràn ngang 34 px tại 320/200%. |
| Visual | Ảnh đã mở trên 320/390/834. Biểu tượng, H2, lời nhắn và nút tonal căn giữa trên nền trang. Dùng icon người và tìm kiếm trong catalog hiện có. |
| Accessibility/Responsive | Cây AX có region “Đang theo dõi”, heading hiện tại và link có tên. Tương phản chữ thấp nhất 7,87:1 và viền focus CTA thấp nhất 4,55:1. Đã kiểm Tab/Enter/Space. Chưa chạy trình đọc màn hình trên thiết bị thật. |
| Interaction | Enter mở Tác giả. Back/Forward trả đúng mode. Back từ Reader sau Unfollow tác giả cuối đưa focus về H2 mới, Tab tiếp đến CTA. |

## Kiểm chứng theo AC

Lượt `verify web --scenario password` tập trung vào fixture Feed, sau khi
thiết lập hai tài khoản qua Identity thật. Runner local dùng lại bundle và hai
binary đã đối chiếu SHA-256 và chạy toàn bộ fixture Feed. Các bài kiểm hồ sơ,
handle và mật khẩu độc lập ở ngoài fixture đó không nằm trong kết quả Feed.

| AC | Bằng chứng |
| --- | --- |
| AC-01 | Hàng Feed theo WorkId và ngày công khai đầu do PostgreSQL cấp. Nhan đề và bút danh có đích riêng. Chữ và dòng thơ được giữ. |
| AC-02 | Qua các trạng thái khách, hai loại rỗng, lỗi đọc, lỗi append và đổi phiên. SSR của Feed riêng trả placeholder và `no-store`. |
| AC-03 | Feed theo phiên đọc lại khi Back. Khi anchor còn quyền, focus về bài hoặc bút danh. Khi bài rút, focus về H2 Đang theo dõi. Khi không còn theo dõi ai, focus về H2 trạng thái rỗng. |
| AC-04 | Ma trận bố cục Feed có bài và các ca Back/lỗi append chạy trong password. Kịch bản đối chứng công khai `return-position` đạt focus và hiển thị ở 390/200% nhưng lệch vị trí cuộn 32,14 CSS px trong ca `resize-text200-reduced-motion`, vượt ngưỡng phép đo. |
| AC-05/06 | Tương tác trực tiếp với CTA và xác minh tên vùng trong cây AX. Test đủ điều hướng Enter/Back/Forward và focus khi mất anchor. |
| AC-07 | 320/390/834 × sáng/tối × cỡ chữ gốc 100/200%. 12 ảnh trước/sau theo cặp và phép đo ở bảng trên. |
| AC-08 | LayoutSpec R1, owner Web và feature map ghi composition cục bộ và đích focus. |
