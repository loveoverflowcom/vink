# Tài khoản và Chỉnh hồ sơ R5

Hướng Quiet Lavender và ảnh 26/11 ở [#589](https://github.com/loveoverflowcom/vink/issues/589).
[HTML R5 v1](https://drive.google.com/file/d/1yQmt7mfxwqgLSgeVV1aw8HPbyUvn0u5e/view)
dùng dữ liệu minh họa. Product dùng token Vink và dữ liệu của phiên hiện hành.

## LayoutSpec

| Vùng | Bố cục và điểm cố định |
| --- | --- |
| A1 Nhận diện | Avatar, bút danh và chuông hiện có. Dùng [tóm tắt danh tính hai dòng](redesign-r4/layout-spec.md#tóm-tắt-danh-tính-r5-605) cho bút danh. Giữ các đích xem và chỉnh hồ sơ. |
| A2 Cài đặt | Hàng phẳng, icon trung tính cùng trục. Quyền riêng tư là link toàn hàng với chevron dẫn tới section của form. Tên đăng nhập và mật khẩu có chevron theo trạng thái disclosure. Email hiển thị địa chỉ đã xác minh. [Giao diện](me-preferences-r1-209.md#giao-diện-thành-viên-413-576) là nhóm con, Đăng xuất là nút chữ. [Xóa tài khoản](account-deletion.md) mở vùng quản lý riêng. |
| R1 Đầu trang | Một Back về Tài khoản. Pill có chữ trên desktop, tròn 48 CSS px trên mobile. Link về entry của Tài khoản có sẵn khi mở trang trực tiếp. |
| R2 Nội dung | Avatar chữ từ phiên đã xác minh, Bút danh với trợ giúp ngắn, địa chỉ cạnh action Thiết lập/Đổi địa chỉ, Giới thiệu với ô nhiều dòng. |
| R3 Quyền riêng tư | Sáu giá trị độc lập, gồm hai cặp list/count. Label và giá trị xếp lại khi thiếu chỗ. Quyền đã khóa có lý do. Cảnh báo hiện ngay sau khi chọn thu hẹp quyền. |
| R4 Lưu | Một submit trong footer để giữ đường đến Lưu trên viewport thấp. Lưu, lỗi, Đối chiếu lại và conflict dùng owner hiện có. |

Back là link nội bộ với focus return của Product. Nháp được khôi phục trong cùng phiên.
Dialog Handle giữ focus, quota và cảnh báo tại quyết định. Chrome hỗ trợ picker có icon và dấu
chọn, theo thao tác bàn phím, Escape và light dismiss của trình duyệt. Trình duyệt thiếu
`appearance: base-select` dùng menu native có nhãn chữ. TextField/TextArea giữ native control,
label, lỗi và IME. Giới thiệu tăng chiều cao khoảng 3–8 dòng khi hỗ trợ `field-sizing`.
Desktop/mobile dùng 100% và 150% từ Giao diện. 200% dùng root-font stress riêng.

[#602](https://github.com/loveoverflowcom/vink/issues/602) cập nhật cách trình bày của A2 theo
[concept R1](https://drive.google.com/file/d/1wRQ9W1kNWNrRi6fdqioGqxsl-BKqLrgT/view).
Hàng link có lớp hover, pressed và vòng focus. Dữ liệu, quyền mở vùng, đích và focus khi quay về
tiếp tục theo phiên và điều hướng R5.

## Lựa chọn và bàn giao

- CHOICE-01 dùng `SelectVariant::IconTonal` trên một Select native. `ChoiceOption::with_icon`
  gắn glyph trang trí cho giá trị và option. `inline=true` xếp cùng hàng khi đủ chỗ. Option
  có nhãn ở SSR và thêm icon sau mount để parser cũ hydrate được cùng cấu trúc.
  `Standard` và `Inset16` giữ kích thước cũ. Preview dùng specimen `field.select`.
- CHOICE-02 đặt Quyền riêng tư trong form versioned. `pages::profile::privacy::PrivacySelect`
  nhận `Signal<Visibility>`, `locked`, `disabled` và callback typed. `visibility_choice`
  sở hữu nhãn Công khai / Chỉ mình tôi và icon public / lock. App quyết định trạng thái khóa
  theo giá trị đã lưu. API kiểm tra quyền ở commit.
- CHOICE-03 dùng `vink_ui::BackLink` trên `Link`. Caller truyền `href` nội bộ, `label` của đích,
  và thuộc tính `id` hoặc `data-profile-return` khi cần. Guard/router thuộc host.
- `TextField variant=FloatingTonal leading=… mode=…` và
  `TextArea variant=FloatingTonal grow=true mode=…` dùng cùng recipe và input native.
  Trạng thái disabled/read-only và giá trị theo phiên đến từ form.

## Caller Web

| Vùng | Owner |
| --- | --- |
| Tài khoản, Chỉnh hồ sơ, Handle inline | [AccountHub](../../apps/vink-web/src/account.rs), [EditProfile](../../apps/vink-web/src/pages/profile.rs), [form](../../apps/vink-web/src/pages/profile/browser.rs) và [handle](../../apps/vink-web/src/pages/profile/browser/handle.rs) nhận delta R5. |
| Hồ sơ công khai, người theo dõi, đang theo dõi | [follow.rs](../../apps/vink-web/src/pages/follow.rs) sở hữu đường dẫn, focus và saved position. BackLink dẫn tới hồ sơ chuẩn và dùng nhãn **Hồ sơ** theo [#605](https://github.com/loveoverflowcom/vink/issues/605). |
| Bàn viết, Reader, Thông báo, Khám phá | Back có guard, overlay, history hoặc khôi phục vị trí theo từng hành trình. |
| Auth, editor format, Desk, discovery filters | Các control dùng variant hiện hành. Giao diện giữ RadioGroup, theme/text/rail và storage scope theo thành viên. |
| Microcopy | [PR #586](https://github.com/loveoverflowcom/vink/pull/586) là draft khi bắt đầu. Copy Bút danh, Giới thiệu, Thiết lập và Lưu trong các vùng R5 tương thích với PR đó. Auth và Following theo PR #586. |
