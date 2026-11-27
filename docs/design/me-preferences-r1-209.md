# Giao diện R1 — trang Tôi (#209, cập nhật bởi #413)

Phạm vi UX-01 và thao tác đổi giao diện (WR-01/WR-02).
[#209](https://github.com/loveoverflowcom/vink/issues/209) tiếp nối
[dialog editor R1](dialog-anatomy-r1-208.md) trên nền `develop@ef7da9d1981ef9c0864af8d9c26a8c5b6d919aa3`.
[Concept](https://drive.google.com/file/d/12I8UnUh5dAVloglTfpavfIF5pOFn-Pof/view) và
[LayoutSpec nguồn](https://drive.google.com/file/d/1T25IrfdL3pjWoQekdkXKB_OuIs7tslYO/view)
là tham chiếu ứng viên lịch sử của `#209`. Fidelity `NO_APPROVED_REFERENCE`.

## Giao diện thành viên (#413, #576)

[Issue #413](https://github.com/loveoverflowcom/vink/issues/413) dời Giao diện khỏi rail
và giữ hai mức chữ 100% và 150%. Từ [#576](https://github.com/loveoverflowcom/vink/issues/576),
chỉ thành viên đã được xác nhận Active thấy vùng này. Các form khách, bút danh, sign-in đóng
và auth theo ngữ cảnh tiếp tục hiển thị các tác vụ của chúng. Rail thu/mở theo ngữ cảnh và thao
tác tạm thời trong tài liệu đang mở. Khách dùng màu hệ thống và chữ 100%.

```text
/toi (Cài đặt, thành viên có phiên Active đã xác nhận)
  Giao diện (#page-appearance)
    Chế độ màu (#page-theme) → Cỡ chữ (#page-text: Mặc định 100% | Lớn 150%)
    → Thanh điều hướng (#page-rail)
```

Khung đầu SSR và CSR dùng mặc định. Sau lần kiểm phiên thành công, ứng dụng đọc tùy chọn
theo principal hiện tại trên thiết bị. Khóa lưu dùng SHA-256 của principal mờ; giá trị gồm
màu, cỡ chữ hoặc rail. Các giá trị dùng chung trước #576 chưa biết chủ nên được giữ nguyên
trong storage và bỏ qua. Một giá trị cỡ chữ `200` có scope hợp lệ được đọc là Lớn 150%.
Sau lỗi đọc phiên hoặc tín hiệu đổi cookie, tùy chọn mặc định được áp dụng cho tới khi
có xác nhận mới. Nếu storage bị chặn, lựa chọn vẫn áp dụng trong tài liệu đang mở.

[#602](https://github.com/loveoverflowcom/vink/issues/602) dùng biểu tượng bảng màu trần và nhãn
nhóm nhỏ hơn trong Cài đặt. Ba `RadioGroup` dùng hàng phân đoạn có thể xuống dòng, vùng chạm
≥48 CSS px, màu trung tính mặc định và Lavender kèm dấu tick khi chọn. Nhãn/Arrow/Tab theo
radio native. Trợ giúp rail nằm trong mô tả truy cập của nhóm, giữ ba lựa chọn hiện có.

## Kiểm chứng Web hiện hành

Kiểm radio native tại `/toi` ở 320/390/1440 CSS px, sáng/tối, dùng Tab, Arrow và kiểm AX.
Kiểm tài liệu trước hydration, sau xác nhận thành viên, khi thoát, khi đọc phiên thất bại và
khi đổi giữa hai principal. Kiểm rail nhấn tạm thời và điều hướng Bàn viết qua guard hiện có.
Ca stress cỡ chữ gốc 200% có metadata `dom-override-geometry-only` và tách khỏi browser zoom.
[Coverage Web](../development/feature-map.md#assertion-hiện-có-và-phần-còn-thiếu) ·
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
