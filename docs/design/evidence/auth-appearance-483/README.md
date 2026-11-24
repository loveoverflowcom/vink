# Giao diện trong Auth Web · bằng chứng #483

Đối chiếu màn Đăng nhập của `/toi` bằng Chrome 158 headless trên macOS, DPR 1, Be Vietnam
Pro, App và Identity native với PostgreSQL scratch. Ca đóng đăng nhập chạy trên App có
cổng đăng nhập tắt. Ảnh trước lấy từ `c12013d8` trên nền `478dcbf0`, có bản sửa build App
và giao diện gốc. Ảnh sau lấy từ `06a855b1` trên nền `276d142f`. Cả hai lượt chụp từ cây sạch.

| Kịch bản | Trước | Sau |
| --- | --- | --- |
| 320 × 900, sáng, Mặc định 100% | [PNG](before-320-light-100.png) | [PNG](after-320-light-100.png) |
| 1280 × 900, tối, stress 200% | [PNG](before-1280-dark-200.png) | [PNG](after-1280-dark-200.png) |

[Ảnh 320 × 900, tối, stress 200% sau cuộn](after-320-dark-200-end.png) cho thấy liên kết
vẫn nằm phía trên thanh điều hướng. Stress 200% ghi đè cỡ chữ gốc qua DOM. Mặc định 100%
và Lớn 150% ở bản sau được chọn bằng radio Giao diện.

## Tự đối chiếu theo LayoutSpec

| Chiều | Kết quả trên Chrome |
| --- | --- |
| Functional | PASS. Liên kết native đi tới vùng Giao diện duy nhất. Màu và cỡ chữ được giữ khi tải lại, đổi mode và hoàn tất đăng nhập. Giá trị `200` cũ được đổi thành `150` trước hydration. |
| Geometry | PASS. 120 tổ hợp trên năm trạng thái Auth ở 320/390/834/1280 CSS px. Vùng bấm nhỏ nhất đo được 133,94 × 48 CSS px. Ca 834 px dùng chiều cao 600 px để kiểm chỗ trống phía trên form. |
| Visual | PASS. Hai cặp ảnh cho thấy hành động chính của form giữ ưu tiên thị giác. Biểu tượng bảng màu đứng trước nhãn và nền của liên kết hòa với nền trang khi nghỉ. |
| Accessibility/Responsive | PASS. Tương phản ở trạng thái nghỉ đạt ít nhất 5,83:1. Kiểm thứ tự Tab, vòng focus, tên liên kết qua markup native, reflow 200% và khả năng bấm phía trên thanh điều hướng. |
| Interaction | PASS. Enter, click và chạm qua CDP cuộn và focus đúng đích, giữ nguyên input đang nhập. Tab tiếp đến radio. Lớp trạng thái được đo khi hover, nhấn và focus bàn phím. |

## Lượt chạy

- Trước: `cargo xtask verify web --scenario password` tại `target/verify-web/1795510392805080000-98813/`.
- Sau: `cargo xtask verify web --scenario password --scenario auth` tại `target/verify-web/1795510965102255000-6199/`.
- `auth` và các kiểm #483 trong `password` đạt. Toàn scenario `password` dừng ở kiểm tràn ngang
  của Khám phá #296 tại 320 px, sáng, 200%, tái hiện trên cả bản trước và bản sau.
