# Baseline thiết kế Vink — Lavender M3 Expressive

**Ngày:** 01/11/2026 (UTC+07) · **Trạng thái:** baseline đã triển khai cho các màn hiện có
([#5](https://github.com/loveoverflowcom/vink/issues/5), PR 01/05 của
[kế hoạch #4](https://github.com/loveoverflowcom/vink/issues/4)). Đây không phải phê duyệt thương
hiệu production; yêu cầu và D-02–D-12 giữ trạng thái trong [decision log](../architecture/decisions.md).

Owner duy nhất của roles màu, chữ, shape, focus và motion là
[`apps/vink-web/style/vink.css`](../../apps/vink-web/style/vink.css). Trang này ghi lựa chọn,
nguồn và bằng chứng; khi giá trị trong CSS và bảng dưới khác nhau, CSS là hiện trạng và trang này
cần cập nhật.

## Nguồn và thứ tự ưu tiên

| Nguồn | Dùng cho |
| --- | --- |
| [Design index v2.3](https://drive.google.com/file/d/16OqseJ0uCdbEtF2H3XZyNzfG33QLLFZC/view), [tokens/typography](https://drive.google.com/file/d/1JiIH3GCsLxCJf766hN7RBU_doQ8xui9K/view), [handoff](https://drive.google.com/file/d/1anrbq2AsNhNIXFCfO72B1yKddgQrVAep/view) | Palette Lavender sáng/tối, Be Vietnam Pro + Literata, shape/motion, owner map |
| [Quyết định 31/10/2026 về điều hướng và logo](https://drive.google.com/file/d/1xfV44hh0oe5b5ZopTFnEzcZD7xYXyI48/view) | Khám phá trước Bàn viết; logo chưa chốt |
| [Ghi chú logo v2](https://drive.google.com/file/d/1pu1i_YqguPfbMCe2XDZk9rfU6bXZFtoO/view) và quyết định 01/11/2026 ([#10](https://github.com/loveoverflowcom/vink/issues/10)) | Trạng thái logo |

Thứ tự ưu tiên: chỉ dẫn hiện tại của người dùng > quyết định Drive mới hơn có phạm vi > spec v2.3.
Ảnh tham chiếu không ghi đè quyền của server hay ngữ nghĩa lưu/transport. `source/tokens.css` của
prototype là nơi lấy giá trị ban đầu; từ nay sửa role trong `vink.css`, không đồng bộ ngược từ
prototype. Lavender là palette riêng của Vink do người dùng chọn, không phải thương hiệu Cantos.

## Điều hướng và logo

- **Khám phá đứng trước Bàn viết.** Rail desktop/tablet: Khám phá → Bàn viết, Tôi ở chân rail.
  Bottom bar mobile: Khám phá → Bàn viết → Tôi. Quyết định 31/10 ghi đè thứ tự Bàn viết-trước
  trong ảnh chụp và ZIP v2.3. Thứ tự tab không tự đổi màn khởi động; `/` vẫn là trang công khai.
- [#7](https://github.com/loveoverflowcom/vink/issues/7) triển khai rail/bar, nhãn Khám phá,
  Tôi và preference Sáng/Tối/Hệ thống; spec và owner ở dưới.
- **Logo chưa có bản production.** Mark chữ “V” trong shell là tạm thời, chỉ đổi sang
  `primary`/`on-primary`. Logo vết mực v2 (PNG 1254 px) là preview và đã bị thay hướng bởi logo
  Enso lông vũ + ngòi bút ngày 01/11; [#10](https://github.com/loveoverflowcom/vink/issues/10) sở
  hữu vector hóa, các kích thước và tích hợp. Không dùng PNG preview làm logo.

## Màu và theme

Light là mặc định. Dark theo `prefers-color-scheme` khi `<html>` không có `data-theme`;
`data-theme="dark"` hoặc `"light"` ghi đè hệ thống ở cả hai chiều và đặt `color-scheme` tương ứng.
Bộ chọn trong Tôi lưu `vink.ui.theme` trên thiết bị; Hệ thống xóa override để CSS theo OS.
Bootstrap trước stylesheet khôi phục override trước hydration; SSR không cần đọc storage và vẫn
có CSS theo hệ thống khi JavaScript/storage bị chặn. Lựa chọn mới vẫn áp dụng trong phiên nếu ghi
storage lỗi, nhưng không hứa giữ sau reload.

| Role | Light | Dark | Dùng cho |
| --- | --- | --- | --- |
| `surface` / `on-surface` | `#fffaff` / `#241d2d` | `#17121d` / `#f0e7f8` | Nền trang, chữ chính |
| `surface-container-low` | `#f7f2fa` | `#211b29` | Input, vùng soạn nội dung |
| `surface-container` | `#efe8f6` | `#2b2434` | Hero, thẻ, empty, notice |
| `surface-container-high` / `-highest` | `#e8e0f0` / `#e3d8ec` | `#372e42` / `#44394f` | Thanh lưu, hover thẻ |
| `on-surface-variant` | `#5f566b` | `#d1c3db` | Chữ phụ, metadata, nav chưa chọn |
| `outline` / `outline-variant` | `#81768e` / `#d6cadd` | `#a095ab` / `#50455a` | Viền control / đường phân cách |
| `primary` / `on-primary` | `#6846a5` / `#ffffff` | `#d4bbff` / `#381c63` | Hành động chính, link, mark tạm |
| `primary-container` / `on-` | `#ebddff` / `#35205a` | `#4e3477` / `#f0e3ff` | Nav đang chọn |
| `primary-fixed` / `on-` | `#dcc5ff` / `#2b1050` | `#5e4388` / `#f0e3ff` | Dự phòng cho điểm nhấn trang trí |
| `secondary` / `secondary-container` / `on-` | `#685b78` / `#eadcf3` / `#31233d` | `#cec0df` / `#493c56` / `#f2daff` | Nút tonal |
| `tertiary` / `tertiary-container` / `on-` | `#85506a` / `#ffd9e8` / `#3a1227` | `#f4b2ce` / `#65364d` / `#ffd9e8` | Chấm “chưa lưu”; đề bài sau này |
| `error` / `error-container` / `on-` | `#ba1a1a` / `#ffdad6` / `#410002` | `#ffb4ab` / `#6d2520` / `#ffdad6` | Lưu lỗi, xung đột |
| `on-error` | `#ffffff` | `#690005` | Chữ trên nút nguy hiểm “Bỏ thay đổi và rời” |
| `warning-container` / `on-` | `#f7edcb` / `#514218` | `#4e4223` / `#f4e5af` | Thông báo xung đột |
| `success` / `success-container` / `on-` | `#326846` / `#d7efde` / `#214b30` | `#a3d5b2` / `#244631` / `#c7ebd1` | Chấm “đã lưu” |
| `focus` | `#7250b4` | `#d4bbff` | Vòng focus 3 px |
| `inverse-surface` / `inverse-on-surface` | `#241d2d` / `#fffaff` | `#f0e7f8` / `#17121d` | Tooltip, snackbar sau này |
| `state-hover`, `scrim`, `shadow` | primary 8 %, tối 44 %, bóng 0 12 40 | primary 10 %, đen 58 %, bóng 0 16 48 | Trạng thái, lớp phủ, độ nổi |

Nhóm nội dung bằng fill và khoảng cách: thẻ, empty và notice bỏ viền; viền chỉ còn ở input
(nhận biết control), focus và đường phân cách footer.

## Chữ, hình, chuyển động

- UI: **Be Vietnam Pro** 400/600, fallback Noto Sans, Segoe UI, Roboto, Helvetica Neue, Arial.
  Văn đọc, editor và tiêu đề: **Literata** variable (`wght` 200–900, `opsz` 7–72), fallback
  Noto Serif, Source Serif 4, Georgia, Times New Roman. Văn đọc
  `clamp(1.0625rem, 1rem + 0.25vw, 1.25rem)`, leading 1.85, measure 42rem.
- Font tự host, nguồn/hash/license và cách tải: [assets/fonts](../../apps/vink-web/assets/fonts/README.md).
- Radius 8/12/20/28 px, `hero` 32 px, pill. Spacing 0.25–3rem như trước.
- Motion 160/320 ms; `ease-emphasized` cho màu, `ease-spatial` (có vượt nhẹ) cho đổi shape.
  `prefers-reduced-motion` rút mọi transition/animation về gần 0.

## Tương phản đo trên app

Đo ngày 01/11/2026 trên nhánh `codex/lavender-ui-foundation` (nền `develop@9fd8b0b`), Chrome
156 headless, Linux, DPR 1. Màu lấy từ `getComputedStyle` sau khi theme áp dụng, ghép alpha lên nền
đặc gần nhất; công thức WCAG 2.x, ngưỡng 4.5 cho chữ, 3 cho chữ lớn và thành phần không phải chữ.
Quét mọi phần tử có chữ trên `/`, `/tac-pham/{id}`, `/ban-viet`, `/ban-viet/{id}` và trang 404 ở
1440 px, 320 px và chữ 200 %; editor có thêm trạng thái chưa lưu và xung đột. Không có cặp nào dưới
ngưỡng. Đây là đo cho app tại thời điểm trên, không phải 42 PASS của prototype hay chứng nhận WCAG.

| Cặp | Light | Dark |
| --- | --- | --- |
| Văn đọc / tiêu đề trên `surface` | 15.81 | 15.33 |
| Chữ phụ (`on-surface-variant`) trên `surface` | 6.72 | 10.99 |
| Chữ phụ trên `surface-container` (hero, thẻ) | 5.79 | 8.93 |
| Link (`primary`) trên `surface` | 6.77 | 10.83 |
| Eyebrow (`primary`) trên hero | 5.83 | 8.79 |
| Nút filled, skip link, mark | 6.98 | 8.16 |
| Nav đang chọn | 10.84 | 8.23 |
| Nút tonal | 11.14 | 7.87 |
| Chữ trong input và vùng soạn CodeMirror | 14.77 | 13.95 |
| Nút text (`primary`) trên thanh công cụ/hộp thoại editor | 5.44 | 7.57 |
| Nút “Bỏ thay đổi và rời” (`on-error` trên `error`) | 6.46 | 7.72 |
| “Rời mà không lưu…” (`error`) trên hộp thoại | 5.03 | 7.58 |
| Thông báo lỗi lưu (`on-error-container`) | 13.26 | 8.40 |
| Chữ được chọn trong editor (`on-primary-container`) | 10.84 | 8.23 |
| Trạng thái lưu trên thanh lưu | 5.40 | 7.68 |
| Xung đột (`error`) trên thanh lưu | 5.03 | 7.58 |
| Tiêu đề thông báo warning | 8.37 | 7.84 |
| Viền input trên `surface` (không phải chữ) | 4.14 | 6.47 |
| Chấm đã lưu / chưa lưu / xung đột | 5.10 / 4.88 / 5.03 | 7.79 / 7.42 / 7.58 |
| Vòng focus, thấp nhất trên `surface-container-high` | 4.64 | 7.57 |

Năm hàng từ “Nút text” tới “Chữ được chọn” thêm cho editor của
[#6](https://github.com/loveoverflowcom/vink/issues/6), tính từ giá trị token (nền đặc, không alpha)
chứ chưa qua lần quét `getComputedStyle` ở trên. Vòng focus đo bằng phím Tab thật: 51 lần dừng mỗi theme, tất cả khớp `:focus-visible`. Chưa đo
trạng thái hover (`filter: brightness`), nút disabled (độ mờ 50 %, được loại khỏi ngưỡng) và nền ảnh.

## Kiểm thị giác đã chạy và giới hạn

Ảnh chụp đã mở ở 1440/1024/390/320 px, sáng/tối, chữ 200 % (cỡ chữ gốc 200 %, tương đương đặt cỡ
chữ mặc định 32 px) ở 320/390/1440 px, tiêu đề tiếng Việt dài, `data-theme` đè hệ thống cả hai
chiều, và chặn `/pkg/fonts/*` để xem fallback. Không tràn ngang sau khi sửa: nav trong topbar
xuống dòng và lưới thẻ co về bề rộng khung khi chữ lớn; skip link ẩn theo chiều cao của chính nó.
Không có lỗi console hay cảnh báo preload. Khi giả lập `prefers-reduced-motion: reduce`, thời
lượng transition tính toán của thẻ, nút và nav về `1e-05s` (bình thường 160/320 ms).

Chưa kiểm: zoom trình duyệt thật, Safari/Firefox, thiết bị di động thật, IME hệ điều hành, screen
reader có tên và cài đặt reduced-motion của hệ điều hành. Ảnh tĩnh không chứng minh tương tác hay
độ mượt.

## Shell thích ứng #7

LayoutSpec r1, đã trình bày trước sửa UI ngày 01/11/2026; nền `develop@6648200`.
Nguồn: [v2.3](https://drive.google.com/file/d/1IlLVGiPHIg9DBohATdwd-38Gdy7z4U5g/view),
[Navigation Research](https://drive.google.com/file/d/1TDA4HoX5O-IHtQfu3RSnOks2WaYXkBaz/view),
quyết định Khám phá-trước 31/10 ở bảng nguồn. Hai không gian là tác vụ đọc/viết; Tôi là tiện ích
ngắn trên web để đổi giao diện trong lúc viết. Đây là lý do thiết kế được ghi nhận, chưa phải
kết quả nghiên cứu người dùng hay quyết định auth D-05/S-03.

```text
Shell #shell
  Region #navigation — rail desktop/tablet
    Region #brand — mark V tạm
    Region #nav-primary — Khám phá → Bàn viết
    Region #account-bottom — Tôi, neo chân rail
  Region #main — route hiện tại, editor giữ một instance
  Region #bottom-navigation — mobile: Khám phá → Bàn viết → Tôi
  Overlay #profile-menu — trạng thái tài khoản, appearance, Mở trang Tôi
  Screen #me — /toi: trạng thái tài khoản và appearance
```

Owner: [lib.rs](../../apps/vink-web/src/lib.rs) giữ router và route `/` mặc định;
[shell.rs](../../apps/vink-web/src/shell.rs) giữ rail/bar/dialog;
[appearance.rs](../../apps/vink-web/src/appearance.rs) giữ preference và initials;
[me.rs](../../apps/vink-web/src/pages/me.rs) giữ `/toi`. CSS dùng palette/role hiện có, không
thêm palette hay logo. `initials` dùng từ đầu/cuối, NFC và uppercase tiếng Việt; không có tên
thật thì trả dấu trung tính. An Nhiên → AN chỉ là fixture unit test.

- Từ 1100px mặc định 220px; 760–1099px và editor mặc định 96px. Preference `vink.ui.rail`
  ghi đè ngữ cảnh. Rail compact đặt icon trên/nhãn dưới; rail cuộn khi thiếu chiều cao.
- Dưới 760px rail ẩn, header brand trong luồng trang, bottom bar ba đích. Editor ẩn bottom bar,
  giữ đường Về Bàn viết hiện có. `ResizeObserver` đo chiều cao bar (gồm safe area) để dành đáy,
  cả khi tăng chữ 200%; không dùng chiều cao cứng làm khoảng bù.
- Nội dung/CTA chính thuộc route hiện tại. Trong Tôi, tác vụ chính là chọn giao diện; thông tin
  tài khoản chưa có được nói rõ. Không có nút đăng nhập/đăng xuất giả hay bộ đếm thành viên.
- Overlay native `dialog` neo cạnh chân rail, nền trong suốt và ngoài dialog inert; focus vào
  radio đang chọn, Tab giữ bên trong, Escape/Đóng/bấm ngoài trả trigger. Resize về mobile đóng
  overlay và focus main đang hiển thị; route, scroll và editor được giữ.
- Đổi theme/rail, mở/đóng overlay chỉ đổi preference/DOM shell, không lưu hay gọi guard. Nút
  Mở trang Tôi đóng overlay trước khi gọi `NavigationGuard.navigate`; link và back/forward dùng
  guard PR 02. Lưu lỗi/xung đột/treo giữ nguyên đường xuất và xác nhận rời không lưu.
- Loading/empty/lỗi/saved/dirty/conflict tiếp tục do route/editor hiện có trình bày. Shell không
  suy ra trạng thái tài khoản hay bản thảo. Tooltip thu/mở rail có hover/focus/Escape, nằm ngoài
  vùng rail cuộn để không bị cắt; thông tin thiết yếu vẫn có nhãn và trạng thái trực tiếp.

[Feature map](../development/feature-map.md) dẫn tới assertions. Ảnh PNG từ browser smoke nằm
trong `target/verify-web/screenshots/` và phải được mở trước khi kết luận thị giác. Ảnh/chữ đầu
không chứng minh profile thật, OS IME, screen reader hay thiết bị mobile.

### Bằng chứng shell ngày 01/11/2026

Nền `develop@6648200`, diff `codex/adaptive-navigation` cho #7, Linux, Rust 1.99.1,
Node 24.22.2, Chrome 156.0.8078.73 headless, DPR 1, PostgreSQL local với database thử riêng.

- `PASS` — `cargo xtask verify quick`: fmt, clippy server/WASM, 51 unit/contract tests và
  repository checks. Fixture initials gồm tên nhiều từ, một từ, dấu tổ hợp và blank.
- `PASS` — `CARGO_BUILD_JOBS=1 cargo xtask verify web`: đầy đủ main + after-restart, không có
  console error/exception; một job chỉ giới hạn build, không giảm assertions.
- `SCREENSHOT_INSPECTED` — mở 16 PNG cùng manifest `contexts.json`: 1440/1024/390/320px sáng/tối,
  320px chữ 200% (Tôi, cuối trang và Bàn viết), tooltip, overlay, editor dirty và preview tiêu đề
  dài. Nhãn điều hướng không tách giữa từ sau khi bỏ padding ngang trong ô compact/mobile;
  khoảng đáy theo chiều cao bar thật. Không còn finding thị giác cần sửa trong các cảnh đã mở.
- Năm cặp chữ shell lấy từ `getComputedStyle` sau khi transition kết thúc: thấp nhất 5.79:1
  sáng, 8.23:1 tối; JSON nằm ở `target/verify-web/shell-contrast-{light,dark}.json`.
- Native focus của [runtime](../../apps/vink-web/editor/src/editor.ts) gọi `EditorView.focus()`
  để giữ selection khi trở lại từ rail/overlay. Browser kiểm Tab, native blur/focus, cùng DOM,
  chữ/tiêu đề/dirty, không có PUT/POST do utility; guard cả ba đích, save chậm/hủy, lỗi/xung đột,
  undo/redo, restart và export. Fixture tiêu đề dài giữ giới hạn tên tệp 80 ký tự vốn có.

`NOT_RUN`: IME hệ điều hành, điện thoại thật, zoom trình duyệt thật, Safari/Firefox và screen
reader có tên. Resize CSS, root font 200% và CDP input không thay những bằng chứng đó.
Navigation chỉ phủ một phần UX-01/UX-03; utility Tôi chưa hoàn tất UX-04 hay D-05/S-03.
