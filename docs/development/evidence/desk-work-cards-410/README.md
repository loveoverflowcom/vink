# Kiểm chứng thẻ tác phẩm Bàn viết #410

App SSR và WASM dựng từ `b062860d`, CSS đã dựng lại tại `2aa8fa96`, smoke mở Reader
và Back tại `dd05127f`. Base `develop@4b2fb157`. Chrome 157 headless trên macOS arm64,
App profile `local`, PostgreSQL scratch. 200% là stress reflow bằng root font-size.

Các ca chạy trực tiếp bằng `node apps/vink-web/e2e/browser-smoke.mjs --phase`, với App
fixture, Chrome và thư mục state theo lượt. `endorsement` dùng hai process App cùng DB,
một tác giả và một người đọc. Lệnh `cargo xtask verify web --scenario main --scenario
publication --scenario endorsement` FAIL khi build/link thiếu chỗ đĩa. Ca
`publication-author` với Identity chưa chạy.

| Chiều | Kết quả trên route |
| --- | --- |
| Functional | PASS `author-works` cho nhãn theo dữ kiện, không có link sửa khi thiếu nháp, chỉ số của bản đang công khai, 0 thật và số chưa tải được. PASS `endorsement` với App và PostgreSQL thật. |
| Geometry | PASS bo 20, khe 8, Pen Flight 26, nhận xét 24 và liên kết Đọc cao ít nhất 48 ở mặc định. |
| Visual | Self-review ảnh trước/sau ở desktop sáng và mobile tối; lối Đọc ở cạnh thẻ khi đủ rộng. |
| Accessibility/Responsive | PASS 24 cấu hình 320/390/834/1440, sáng/tối, 100/150/stress 200 và nút Đọc nhận focus ở màn hẹp. Screen reader thật chưa chạy. |
| Interaction | PASS mở đúng bản công khai và Back về Bàn viết. PASS `desk-pagination` ở `b062860d` cho 24 mục/trang, 240 mục/phần, retry và focus khi về đầu. |

Ảnh trước dùng `3356c271` của #411. Thay đổi trạng thái rỗng, tải và lỗi của nhánh đó
không áp dụng trong ảnh danh sách đã tải. Cùng fixture `author-works`; ảnh sau đặt một
nháp thành nhan đề trống để kiểm “Chưa đặt tên”.

| Cảnh | Trước | Sau |
| --- | --- | --- |
| 1440 × 900, sáng, 100% | [Ảnh](truoc-1440-sang-100.png) | [Ảnh](sau-1440-sang-100.png) |
| 390 × 900, tối, stress 200% | [Ảnh](truoc-390-toi-200.png) | [Ảnh](sau-390-toi-200.png) |
