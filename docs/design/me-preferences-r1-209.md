# Giao diện R1 — trang Tôi (#209, cập nhật bởi #413)

Phạm vi UX-01 và thao tác đổi giao diện (WR-01/WR-02).
[#209](https://github.com/loveoverflowcom/vink/issues/209) tiếp nối
[dialog editor R1](dialog-anatomy-r1-208.md) trên nền `develop@ef7da9d1981ef9c0864af8d9c26a8c5b6d919aa3`.
[Concept](https://drive.google.com/file/d/12I8UnUh5dAVloglTfpavfIF5pOFn-Pof/view) và
[LayoutSpec nguồn](https://drive.google.com/file/d/1T25IrfdL3pjWoQekdkXKB_OuIs7tslYO/view)
là tham chiếu ứng viên lịch sử của `#209`. Fidelity `NO_APPROVED_REFERENCE`.

> **Cập nhật theo [#413](https://github.com/loveoverflowcom/vink/issues/413) (23/11/2026):**
> Thay thế có phạm vi đối với phần rail/overlay của `#209`: bỏ nút **Giao diện** ở nav rail và bỏ
> overlay `#profile-menu` (`#open-me`, `#overlay-*`); không ghép lại vào thẻ tài khoản hay tạo
> trigger ẩn. `/toi` (`#page-appearance`) là điểm chỉnh Giao diện duy nhất trên Web cho cả khách
> và người đã đăng nhập. Nhóm **Cỡ chữ** (`#page-text`) chỉ còn hai lựa chọn **Mặc định**
> (`100%`) và **Lớn** (`150%`, thay cho `200%`), và bootstrap tự chuẩn hóa `vink.ui.text=200` cũ
> sang `150` trước CSS/first paint. Bằng chứng lịch sử của `#209` bên dưới giữ nguyên bối cảnh cũ.

```text
Screen /toi — đích Tôi trên rail và thanh điều hướng mobile (điểm chỉnh Giao diện duy nhất từ #413)
  Region .vk-appearance-card — Giao diện (#page-appearance)
    Region AppearanceChoices — #page-theme → #page-text (Mặc định 100% | Lớn 150%) → #page-rail

Lịch sử trước #413 (#209): từng có Overlay #profile-menu neo theo rail hiệu lực (#profile-title,
#profile-close, #overlay-theme → #overlay-text → #overlay-rail, #open-me ngoài route /toi).
```

Trong vùng Auth của `/toi`, liên kết **Giao diện** đưa người dùng tới cùng
`#page-appearance` và chuyển focus đến vùng chỉnh. Vị trí, trạng thái tương tác và kiểm chứng
của liên kết theo [LayoutSpec Auth R2](auth-password-web.md).

## Kiểm chứng Web hiện hành từ #413

Dùng lựa chọn native trên `/toi` để kiểm Mặc định 100% và Lớn 150% ở 320/390/834/1440 px
với giao diện sáng và tối. Kiểm tên nhóm, phím mũi tên, Tab, dấu chọn, focus và điều kiện
ghi nhớ qua reload. Kiểm profile cũ `vink.ui.text=200` trước CSS và qua hydration ở mức 150%.
Ca stress 200% dùng ghi đè DOM riêng với metadata `dom-override-geometry-only`. Xem
[coverage Web và bootstrap](../development/feature-map.md#assertion-hiện-có-và-phần-còn-thiếu) và
[Shell R4.3](redesign-r4/layout-spec.md#3-shell-rail-và-thanh-dưới).

## Lịch sử panel và trang Tôi trước #413

Các layout, selector và lượt kiểm dưới đây dùng cho hai bề mặt ở thời #209/#219.

- **CHOICE-01 A.** Dùng flex-wrap theo kích thước nội dung. Cả hai nơi đặt tên nhóm
  trên lựa chọn, giữ thứ tự DOM, nhãn tiếng Việt và các glyph hiện có. Ở 320 px và
  chữ 200%, các lựa chọn có thể xuống hàng riêng mà vẫn dùng chiều rộng còn lại.
- Hai bề mặt dùng cùng CSS trong [vink.css](../../apps/vink-web/style/vink.css)
  và cùng [AppearanceChoices](../../apps/vink-web/src/appearance.rs). Nền chính
  `surface-container-high`, radius 28 px, một gutter 24 px ở 100% và tối đa 32 px ở
  200% để dành vùng nhìn cho tiêu đề và radio màu đang chọn. Gap giữa nhóm 24 px,
  từ nhãn tới lựa chọn và giữa lựa chọn 8 px ở 100%.
  Khoảng giữa nhóm do gap sở hữu. Vùng chạm tối thiểu 48 × 48 CSS px, cao theo chữ.
- Lựa chọn idle dùng `surface-container-low/on-surface`. Checked dùng
  `primary-container/on-primary-container` và dấu check 16 px có chỗ dành sẵn.
  Vòng focus 2 px hiển thị độc lập; trên mục đã chọn dùng `on-primary-container`
  để tăng tương phản với nền tonal. Forced colors tiếp tục dùng màu hệ thống
  cho trạng thái chọn/focus và giữ dấu check.
- [Shell](../../apps/vink-web/src/shell.rs) giữ native modal, scrim, inert,
  anchor, focus và history. Khi mở, focus vào radio màu đang chọn nếu radio
  nằm trọn trong vùng nhìn. Nếu màn thấp hơn, focus bắt đầu ở nút Đóng cạnh
  tiêu đề rồi Tab cuộn tới radio đang chọn. Tab/Shift+Tab cũng có thể cuộn tới
  hành động cuối và về nút đóng.
  Fieldset/legend và radio native vẫn sở hữu Arrow/Tab/checked. Bootstrap, phản hồi
  đổi màu hệ thống và ba khóa hiện tại giữ cùng authority với trang Tôi.

Đo vùng trong sau padding với dung sai 1 CSS px. Kiểm panel 834/1440 px, rail
thu gọn/mở rộng, chữ 100%/200%, sáng/tối/hệ thống và viewport cao 360 px.
Kiểm focus lúc mở ở 834 × 320/280 px, tối 200%, rail thu gọn.
Kiểm `/toi` ở 320/390/1440 px, chữ 100%/200%, sáng/tối. Đối chiếu qua
[ma trận và bằng chứng Web](../development/ui-system-qualification-evidence.md#giao-diện-r1-cho-panel-và-trang-tôi).

Delta sau #219 và kiểm chứng local ở [review #220](../development/appearance-220-review.md).
