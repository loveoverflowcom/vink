# NavigationBar R1 trên Web

Phạm vi [Web #131](https://github.com/loveoverflowcom/vink/issues/131). Căn hàng kế thừa
#88 (PR #89). Mốc thiết kế `NO_APPROVED_REFERENCE`. Tham chiếu
[Flutter NavigationBar](https://api.flutter.dev/flutter/material/NavigationBar-class.html)
cho cấu trúc icon, indicator và nhãn.

## Cấu trúc và quyết định

```text
Thanh dưới · nền phẳng, có safe area
  Ba liên kết bằng nhau · Khám phá → Bàn viết → Tôi
    Vùng indicator · 64 × 32 CSS px ở các kích thước đã kiểm
      Icon 24 px hoặc AccountAvatar
    Nhãn · hàng riêng, xuống tối đa hai dòng trong ma trận
```

Baseline `f581f87a106d6e39a603a27c0faaed1338598f5f` tô cả liên kết đang chọn. Chọn
wrapper trong [NavItem](../../packages/ui-system/leptos/src/navigation.rs) để dùng cùng
một vùng căn giữa cho SVG và avatar. [CSS dùng chung](../../packages/ui-system/leptos/style/vink-ui.css)
chỉ tạo hộp cho wrapper trong thanh dưới. Rail tiếp tục dùng bố cục inline/stacked theo
container. Khung, lớp phủ và liên kết do host quản lý.

Pill dùng `primary-container`, icon chọn dùng `on-primary-container` và nhãn chọn đọc trên
mặt thanh với `on-surface`. Hover và pressed có lớp màu riêng trong vùng indicator. Vòng
focus bao toàn vùng thao tác. Forced colors vẽ viền hệ thống ở indicator và vòng focus.
Padding dọc của liên kết được giới hạn để giữ nhãn lớn cùng nhịp và thanh dưới 18% chiều
cao viewport trong ma trận cũ. ResizeObserver tiếp tục đo chiều cao thật.

## Kiểm chứng

| Acceptance | Kết quả local |
| --- | --- |
| AC-01 · Anatomy | **PASS** cho ba route và avatar. Oracle kiểm màu, vùng tô và hình học, thất bại trên artifact baseline vì nền phủ cả đích, sau đó qua với R1. Đối chứng âm tiêm lại nền cũ cũng bị phát hiện. |
| AC-02 · Căn hàng, nhãn, đích bấm | **PASS** 60 ca cao 900 px ở 320/360/390/480/759 × 100/200% × sáng/tối × ba route, cộng sáu ca cũ cao 800 px. Hai trục không lệch, nhãn đủ và không tràn ngang. Thanh cao tối đa 140,78 px ở 320/chữ 200%, dưới giới hạn 144 px tại viewport cao 800 px. |
| AC-03 · Điều hướng và accessibility | **PASS** SSR và hydration, thứ tự và `aria-current`, tên AX “Tôi”, Tab, Back/Forward, modifier click và middle click mở tab thật. |
| AC-04 · Khung và nội dung cuối trang | **PASS** đo chiều cao, giữ khoảng cuộn, focus cuối trang với inset CDP 24 px dưới và 20 px hai mép, rail 759/760/834/1440, editor ẩn thanh dưới và gallery dạng khung cố định. |
| AC-05 · Theme và trạng thái | **PASS** tương phản tối thiểu 6,28:1 sáng và 8,23:1 tối cho các nhãn và icon SVG được đo. Hover, pressed và focus không đổi geometry. Forced colors và reduced motion qua kiểm trong Chrome. |
| AC-06 · Bằng chứng | **PASS** ba cặp ảnh đối chiếu đã mở cùng điều kiện, cùng chứng cứ FAIL trên baseline và PASS sau sửa. |

Ảnh rail 834 px sáng ở chữ 100% trùng từng byte giữa hai lượt chụp.

| Tự đánh giá | Kết quả trong phạm vi Chrome |
| --- | --- |
| Functional | **PASS** ánh xạ ba route và khung editor. |
| Geometry | **PASS** tâm icon, dòng đầu, vùng bấm và khoảng chừa cuối trang. |
| Visual | **PASS** ảnh đã mở cho thấy điểm nhấn tập trung vào icon/avatar trên một mặt thanh phẳng. |
| Accessibility/Responsive | **PASS** tên AX, focus, tương phản, reflow 200% và chuyển sang rail. |
| Interaction | **PASS** thao tác chuột, phím, lịch sử trình duyệt và đổi trạng thái. |

Browser zoom, OS text scaling, screen reader, thiết bị thật và CMP là **NOT_RUN**.
[CMP #132](https://github.com/loveoverflowcom/vink/issues/132) có thể tái sử dụng cấu trúc
indicator/nhãn và ma trận chữ lớn, với phép đo riêng trên CMP.

Source và artifact local đã xác minh. CSS được App serve khớp từng byte với bundle local.
Lúc kiểm 20:30 UTC ngày 18/11/2026, HTML công khai vẫn dùng cấu trúc cũ và stylesheet
trả CSS tô toàn đích. Phản hồi không cho biết deployed SHA. Chưa xác minh production chạy R1.
[Stylesheet công khai](https://vink.forum/pkg/vink.css?brand=vpen-r1)
