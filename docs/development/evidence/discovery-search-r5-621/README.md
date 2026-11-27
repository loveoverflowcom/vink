# Khám phá Search R5 (#621)

Product SSR và hydrate trên Mac mini, Chrome headless/CDP, App và native Identity qua PostgreSQL
scratch trên loopback. Base `4bb11f232f91a14a5551191203ce021ea55ef632`, Product sau sửa
`aa51827ae0dd72f0bf3a17c43c245baf1757117f`. Runtime sau sửa ghi `dirty=false`.

## Cặp ảnh đã mở

| Cảnh khách | Trước, manifest tại `91191186c1ac5ec6f4032dc0292af025eb2c0458` | Sau, Product thật |
| --- | --- | --- |
| Khám phá, 834×900, Dark 100% | [`e-5ad49b5abb7a`](https://drive.google.com/file/d/1BD7PGbRMWIp3wTNC14o5irLVM_WtMU_g/view) | [Search pill và query miss](after-search-dark-834-100.png) |
| Khám phá, 834×900, Light 200% | [`e-217896742d4a`](https://drive.google.com/file/d/1FEb-zGf-5RzF5lkQT0rZbRkY5Ud-kdA6/view) | [Replay cùng cách tăng cỡ chữ root](after-search-light-834-200.png) |
| `/?mode=following`, 834×1112, Light 100% | [`c019`](https://drive.google.com/file/d/1NOpuRLL56E2uB_IrJNnVOUOPpDkG6nkh/view) | [Guard trực tiếp, hai đích công khai](after-following-guard-light-834-100.png) |

[Manifest lịch sử](https://drive.google.com/file/d/1aSRfwKgtDpwg8leGGswwpEBOT0_O6H7J/view) ghi route,
viewport và context; không có giá trị query gõ trong hai dòng bổ sung. Chuỗi fixture “không có tác
phẩm này #100” lấy từ ảnh và HTML preview. Cảnh query miss của scratch Identity có catalog rỗng;
lane Discovery với catalog thật kiểm riêng picker và chip. [Preview HTML](https://drive.google.com/file/d/1IDM3FWKPYF_LhwnIQZ4mNRa8z2LNZKPt/view)
là tham chiếu anatomy, không phải ảnh golden. Local baseline `4bb11f23` cũng qua Chrome Discovery
và Author Search. Hai lỗi static tại baseline là scanner nhầm literal trong assertion picker.

## Đo và kiểm trên Product

| Chiều | Kết quả quan sát |
| --- | --- |
| Functional | Guest SSR và hydrate có Tất cả/Tác giả. Direct URL chờ quyền ở SSR, hiện guard khi Session xác nhận khách và không gọi private feed. Native Identity xác nhận tài khoản đủ quyền, mở feed và quay lại sau đăng nhập. Đăng xuất từ tab khác gỡ link và trạng thái riêng. |
| Geometry | [24 tổ hợp](measurements.json) tại 320/390/834/1440 CSS px, Light/Dark, 100/150/200%. Ở 834 px, chiều cao shell 56 / 63,59 / 84,80 px, border 0 và bán kính 999 px. Hai target 48×48, glyph 24 px, lệch tâm tối đa 0,01 px. CTA 50 px ở 100%. |
| Visual | Ba cặp ở trên đã mở. Đầu trang giữ nhịp và thanh mode intrinsic. Query nằm trong input; artwork, tiêu đề ngắn và CTA tonal tạo một cụm phục hồi. Light 200% vẫn đủ thứ tự đọc và đích hành động. |
| Accessibility và responsive | DOM, AX và Tab riêng cho guest, focus riêng cho input hoặc X trong forced colors. Tương phản nhỏ nhất lúc theme đã ổn định tại 834 px là 5,40:1 ở Light và 7,68:1 ở Dark. Ở 320 px/200%, cuộn tới toàn đích CTA và hit test qua. |
| Interaction | Enter native kích hoạt X khi JavaScript tắt. X còn khi xóa chữ đang gõ mà q áp dụng vẫn tồn tại. Ba mươi vòng mở/gõ/xóa/đóng mất 4.001 ms, giữ node input và tạo 0 lượt đọc works. IME mô phỏng CDP và lịch sử trình duyệt qua lane Author Search. |

Các phép đo và SHA-256 PNG nằm trong [measurements.json](measurements.json). Bộ ảnh đầy đủ, context
và log nằm ngoài `target` ở `/Users/manhblue/vink-evidence/621`. Ma trận dùng app text scale;
ảnh Light tablet replay dùng root font 200% như manifest lịch sử.
