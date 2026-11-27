# Bằng chứng điều hướng Thông báo mobile · #608

Ảnh sau thay đổi được chụp trên Chrome 158.0.8127.89 headless, macOS arm64, DPR 1,
Web SSR/hydration dev tại `60c08f90aa2f49ec03cf4e70cc675b509c206337`.
Cỡ chữ 200% áp dụng qua root font-size. Phiên và dữ liệu Thông báo dùng fixture CDP.
[Thông số từng ảnh và SHA-256](contexts.json) · [LayoutSpec](../../../design/notifications.md#2-shell-rail-chuông-thanh-dưới).

| Cảnh | Trước | Sau |
| --- | --- | --- |
| Tài khoản 320 × 800 CSS px, 200%, Dark | [c142 với danh tính tổng hợp](https://drive.google.com/file/d/1Tn18YgVIaAsWxt6hnCNHHyd77MZznXZN/view) | [Bốn ô, glyph chuông ở thanh dưới](account-320-200-dark.png) |
| Hộp thư 320 × 800 CSS px, 200%, Dark | [c324 với Notifications mô phỏng](https://drive.google.com/file/d/1Qzp95q0kUVlFwZxJdb1CNiiSpt6yst6D/view) | [Direct link chọn Thông báo](inbox-320-200-dark.png) |
| Khám phá 320 × 800 CSS px, 200%, Dark | | [Badge 99+ chừa tâm glyph](badge-320-200-dark.png) |
| Hộp thư khách 390 × 844 CSS px, 150%, Light | | [Ba đích và CTA la bàn Về Khám phá](guest-inbox-390-150-light.png) |
| Khám phá và popover, 100%, Light | | [Thanh dưới 759 px](mobile-759-100-light.png) · [Rail và popover 760 px](rail-760-100-light.png) |

c142 và c324 đến từ source `91191186c1ac5ec6f4032dc0292af025eb2c0458`. c142 chỉ dùng danh tính
tổng hợp. So sánh ảnh theo vùng bố cục và điều hướng. Trên ảnh sau thay đổi, Chrome đo mỗi ô ở
320 px rộng 76,25 CSS px, tâm glyph và dòng đầu lệch 0 CSS px. Badge 99+ kết thúc cao hơn tâm
glyph 4 CSS px. Bài kiểm safe area mô phỏng 24 CSS px đáy, 20 CSS px mỗi cạnh.

[Registry Notifications](../../notifications.md#12-đích-thông-báo-trên-mobile-608) ghi ranh giới kiểm
client, ca Gateway với phiên Identity thật và lịch sử socket/Seen/Read của [PR #581](https://github.com/loveoverflowcom/vink/pull/581).
