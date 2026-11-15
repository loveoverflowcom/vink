# Giao diện R1 — panel và trang Tôi (#209)

Phạm vi UX-01 và thao tác đổi giao diện khi đang viết (WR-01/WR-02).
[#209](https://github.com/loveoverflowcom/vink/issues/209) tiếp nối
[dialog editor R1](dialog-anatomy-r1-208.md) trên nền `develop@ef7da9d1981ef9c0864af8d9c26a8c5b6d919aa3`.
[Concept](https://drive.google.com/file/d/12I8UnUh5dAVloglTfpavfIF5pOFn-Pof/view) và
[LayoutSpec nguồn](https://drive.google.com/file/d/1T25IrfdL3pjWoQekdkXKB_OuIs7tslYO/view)
là tham chiếu ứng viên. Fidelity `NO_APPROVED_REFERENCE`.

```text
Overlay #profile-menu — modal neo theo rail hiệu lực, một vùng cuộn
  Row #profile-title / #profile-close — Giao diện / Đóng Giao diện
  Region AppearanceChoices — #overlay-theme → #overlay-text → #overlay-rail
  Action #open-me — Mở trang Tôi + SVG, ngoài route /toi
Screen /toi — đích Tôi của thanh điều hướng mobile
  Region .vk-appearance-card — Giao diện
    Region AppearanceChoices — #page-theme → #page-text → #page-rail
```

- **CHOICE-01 A.** Dùng flex-wrap theo kích thước nội dung. Cả hai nơi đặt tên nhóm
  trên lựa chọn, giữ thứ tự DOM, nhãn tiếng Việt và các glyph hiện có. Ở 320 px và
  chữ 200%, các lựa chọn có thể xuống hàng riêng mà vẫn dùng chiều rộng còn lại.
- Hai bề mặt dùng cùng CSS trong [vink.css](../../apps/vink-web/style/vink.css)
  và cùng [AppearanceChoices](../../apps/vink-web/src/appearance.rs). Nền chính
  `surface-container-high`, radius 28 px, một gutter 24 px ở 100% và tối đa 32 px ở
  200% để dành vùng nhìn cho tiêu đề và radio đầu tiên. Gap giữa nhóm 24 px, từ nhãn tới lựa chọn và giữa lựa chọn 8 px ở 100%.
  Khoảng giữa nhóm do gap sở hữu. Vùng chạm tối thiểu 48 × 48 CSS px, cao theo chữ.
- Lựa chọn idle dùng `surface-container-low/on-surface`. Checked dùng
  `primary-container/on-primary-container` và dấu check có chỗ dành sẵn.
  Vòng focus 2 px hiển thị độc lập. Forced colors dùng màu hệ thống cho
  trạng thái chọn/focus và giữ dấu check.
- [Shell](../../apps/vink-web/src/shell.rs) giữ native modal, scrim, inert,
  anchor, focus và history. Khi mở ở viewport thấp, tiêu đề và nút đóng
  nằm trong vùng nhìn. Tab/Shift+Tab có thể cuộn tới hành động cuối và về nút đóng.
  Fieldset/legend và radio native vẫn sở hữu Arrow/Tab/checked. Bootstrap, phản hồi
  đổi màu hệ thống và ba khóa hiện tại giữ cùng authority với trang Tôi.

Đo vùng trong sau padding với dung sai 1 CSS px. Kiểm panel 834/1440 px, rail
thu gọn/mở rộng, chữ 100%/200%, sáng/tối/hệ thống và viewport cao 360 px.
Kiểm `/toi` ở 320/390/1440 px, chữ 100%/200%, sáng/tối. Đối chiếu qua
[ma trận và bằng chứng Web](../development/ui-system-qualification-evidence.md#giao-diện-r1-cho-panel-và-trang-tôi).
