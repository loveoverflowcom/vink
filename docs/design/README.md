# Baseline thiết kế Vink — Lavender M3 Expressive

**Ngày:** 01/11/2026 (UTC+07) · **Trạng thái:** baseline đã triển khai cho các màn hiện có
([#5](https://github.com/loveoverflowcom/vink/issues/5), PR 01/05 của
[kế hoạch #4](https://github.com/loveoverflowcom/vink/issues/4)). Đây không phải phê duyệt thương
hiệu production; yêu cầu và D-02–D-12 giữ trạng thái trong [decision log](../architecture/decisions.md).

Từ UI System 01/06 ([#19](https://github.com/loveoverflowcom/vink/issues/19)), owner duy nhất của
giá trị roles màu, chữ, shape, spacing, mật độ, focus và motion là
[`packages/ui-system/shared/tokens.json`](../../packages/ui-system/shared/tokens.json);
`cargo xtask ui-tokens` sinh CSS custom properties (`--vk-*`), Rust và Kotlin từ đó và
`--check` bắt drift. [`vink.css`](../../apps/vink-web/style/vink.css) chỉ còn component web.
Trang này ghi lựa chọn, nguồn và bằng chứng; khi giá trị trong `tokens.json` và bảng dưới khác
nhau, `tokens.json` là hiện trạng và trang này cần cập nhật. Owner map, renderer và preview:
[UI system](../../packages/ui-system/README.md).

Quy tắc trả focus khi quay lại từ bước xác nhận bỏ chữ trong editor và ảnh đối chiếu
nằm ở [UI-REVIEW-18](ui-review-18.md).
Báo cáo [UI-REVIEW-22 — Writing Surface](ui-review-22-writing-surface.md) ghi snapshot,
geometry/typography, kiểm editor, evidence và giới hạn; đây là review-only, không có patch sản phẩm.
Phần đối chiếu cập nhật của báo cáo ghi lại trạng thái trước khi ba PR R1/R2
(#74, #76, #77) được triển khai và merge vào `develop`.

Bổ sung [#174 — Chất lượng UI/UX: tài liệu, skills và pilot](ui-ux-quality-174.md): inventory
owner, rubric, phân loại mốc thị giác, số đo anchor, vòng sửa có điểm dừng và pilot dialog/sheet
trên Web và CMP Desktop. [Sổ mốc thị giác](visual-anchors.md) ghi mốc nào đã được duyệt để
review; hiện chưa có mốc `approved`.

Bổ sung [brief hành vi gửi bình luận #120](discussion-submission-brief.md): anatomy, trạng thái
logic và quy tắc thử lại để chủ sản phẩm design sau; chưa có visual hay code UI.

Bổ sung [#66 — Hình học hàng bản thảo dài](ui-repair-66-draft-rows.md) ghi replay trên develop sau #88, CHOICE-04 và số đo trước–sau.

Bổ sung [#67 — Hình học Tôi và panel](ui-repair-67-me-panel-geometry.md) ghi recapture sau #66,
content bounds và bằng chứng R2 hiện đạt; giữ source presentation và bổ sung regression.

Bổ sung [Surface R5.1 — Khám phá/Bàn viết](search-r5.md) thay anatomy search R3 bằng
input một dòng, hidden native label, nhóm tonal không divider và shared tooltip/details.
Placement, GET/IME/private scope và phần chưa được thay thế giữ contract cũ.

Bổ sung [#217 — hợp nhất trạng thái–hành động lưu Bàn viết (PR 1/2)](ui-editor-unified-save-217.md):
unified `#save-slot` trong `EditorToolbar`, giữ sự thật hai phần chữ/thẻ, `CHOICE-01 B` (focus
handoff trong `#save-slot`) và seam bàn giao cho PR 2/2 (#218).

Bổ sung [#226 — Reader/Nhận xét action surfaces](action-surfaces-reader-discussion-226.md):
LayoutSpec, owner map Web/CMP và seam tiếp tục sau #224; presentation mới chưa triển khai.

Bổ sung [#218 — Editor Paper First](ui-editor-paper-first-218.md): công cụ không container,
và [#254 — Giao diện Editor yên tĩnh](editor-quiet-254.md): bỏ footer `#build-info`, hướng dẫn dài và toàn bộ nút tải `.txt`, làm lại nút về Bàn viết Tonal + icon.
Thêm native trong flow, paper/canvas scoped và giữ save/recovery/focus của #217.

[LayoutSpec R4.3 — Bàn viết, Khám phá và rail](redesign-r4/layout-spec.md) là đầu mối thiết kế
hiện hành (`DESIGN_PROPOSED`); phần hàng tác phẩm Bàn viết và Lượt xem được cập nhật bởi
[LayoutSpec Lượt xem r3](work-views.md) (`#323`/`#514`, `D-01`/`D-02`, `DQ-05`/`DQ-10` còn `OPEN`). [Bàn giao #414](redesign-r4/review-414.md) ghi HOW/WHY, evidence
và owner cho các phần còn mở. [Audit R4.1/R4.2](redesign-r4-ban-viet-kham-pha.md) giữ ba phương
án và findings có provenance lịch sử `develop@7ca0dd0d`.

Bổ sung [#262 — selector định dạng](editor-format-picker-262.md): geometry native Select
opt-in đã tách khỏi prerequisite source/title/provider/Reader; mẫu gallery không bật Markdown
trong Bàn viết.

## Hồ sơ tác giả và Theo dõi (#289)

[LayoutSpec Social r2](social-289.md) đặc tả chín bề mặt Web từ bút danh tới hồ sơ,
Theo dõi và Khám phá. Hướng điều hướng v4 ghi lịch sử của thanh dưới, đích Tôi → Giao diện
và vị trí logo. Phạm vi Web đã được R4 cập nhật dẫn tới [LayoutSpec R4.3](redesign-r4/layout-spec.md).
[HTTP Social r1](../../services/vink-app/contracts/http/v1/social-graph.md)
sở hữu ma trận quyền, projection và lỗi. [Bàn giao #288](../development/social-graph.md)
ghi gate theo từng màn.

## Lưu bài để đọc lại (#385)

[LayoutSpec Reader và tab Đã lưu](saved-works.md) nối r2, D1–D12 và chi tiết bàn giao R3 với [delta Thấm · Lưu](pen-flight-r2.md#cặp-thấm--lưu-trên-reader).

## Thông báo và danh sách Thấm công khai (#485)

[LayoutSpec Thông báo R4](notifications.md) mô tả chuông, hộp thư, quyết định và danh sách người Thấm. Specimen Leptos dùng dữ liệu mẫu. Thiết kế giữ `DESIGN_PROPOSED`.
Hành vi `Seen`, `Read`, “Đọc tất cả” và mở quyết định riêng (`P-12A`) nằm ở [đặc tả chuông và inbox](../architecture/notifications.md#5-đặc-tả-ux-hành-vi-bằng-chữ--chuông-inbox-seenread-và-chi-tiết-quyết-định-ac-03).
Phân trang, tải lại, thao tác phím/chạm, trả focus và chữ 100%–200% nằm ở [đặc tả danh sách Thấm công khai](../architecture/notifications.md#6-đặc-tả-ux-hành-vi-bằng-chữ--danh-sách-người-thấm-công-khai-ac-04-p-08-r3).
Hợp đồng dữ liệu/lỗi/polling nằm tại [HTTP v1](../../services/vink-app/contracts/http/v1/notifications.md)
và danh mục bàn giao cho `Epic AC-16`/`AC-17` nằm tại [bảng bàn giao](../development/notifications.md#5-bàn-giao-riêng-cho-giai-đoạn-design--web-epic-ac-16--ac-17).

## Nguồn và thứ tự ưu tiên

| Nguồn | Dùng cho |
| --- | --- |
| [Quiet Lavender R1 / #66](https://github.com/loveoverflowcom/vink/issues/66), [Expressive Lavender R2 / #67](https://github.com/loveoverflowcom/vink/issues/67), [LayoutSpec R2](https://drive.google.com/file/d/1ChFchoWsrc9XFtkW5BvP5XUfr6GAAbKy/view) | Nguồn lịch sử cho shared fields/actions/search/Explore/Desk/editor (R1, #74/#76) và appearance/account (R2, #77). Dùng [R4.3](redesign-r4/layout-spec.md) cho phần Bàn viết, Khám phá và điều hướng đã được R4 cập nhật. Giữ phân biệt giữa thiết kế và dữ liệu/quyền runtime |
| [Current design 02/11](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view), [amendment v4](https://drive.google.com/file/d/1PQeLmEbwkNiFquI-Qgb6gaEV3i0OZWSn/view) | Tham chiếu thị giác lịch sử của Khám phá. Phạm vi dữ liệu thật và đối chiếu vùng tại thời điểm đó ở [UI-REPAIR-02](ui-repair-02.md) |
| [LayoutSpec Khám phá v3](https://drive.google.com/file/d/1F2tieZPb2-PsR7Tn3vD0xvIlEwBQf2Ne/view), cập nhật [#9](https://github.com/loveoverflowcom/vink/issues/9#issuecomment-6556317033) / [#14](https://github.com/loveoverflowcom/vink/issues/14) | Khám phá và reader thay bố cục spotlight v2.3; capability delta ở [spec lớp đọc](public-reading.md) |
| [Design index v2.3](https://drive.google.com/file/d/16OqseJ0uCdbEtF2H3XZyNzfG33QLLFZC/view), [tokens/typography](https://drive.google.com/file/d/1JiIH3GCsLxCJf766hN7RBU_doQ8xui9K/view), [handoff](https://drive.google.com/file/d/1anrbq2AsNhNIXFCfO72B1yKddgQrVAep/view) | Palette Lavender sáng/tối, Be Vietnam Pro + Literata, shape/motion, owner map |
| [Quyết định 31/10/2026 về điều hướng và logo](https://drive.google.com/file/d/1xfV44hh0oe5b5ZopTFnEzcZD7xYXyI48/view) | Khám phá trước Bàn viết; logo chưa chốt |
| [V-pen R1 08/11/2026](brand/vpen-r1/README.md), [#117](https://github.com/loveoverflowcom/vink/issues/117) | Logo Web hiện hành: V/ngòi bút, Light/Dark transparent, lockup có giọt i; ZIP và manifest SHA trong repo |
| [Selected pack 03/11/2026](brand/final-selected/README.md), [#34](https://github.com/loveoverflowcom/vink/issues/34) | Lịch sử vòng mực trước migration Web #117; giữ nguyên chứng cứ |
| [Logo Enso 01/11/2026](https://github.com/loveoverflowcom/vink/issues/10), PR #17 | Lịch sử đã được selected pack #34 thay thế; không còn là source artwork hiện hành |

Thứ tự ưu tiên: chỉ dẫn hiện tại của người dùng > quyết định mới hơn đúng phạm vi > spec còn hiệu lực.
Với phần thiết kế Bàn viết, Khám phá và rail được cập nhật, dùng [R4.3](redesign-r4/layout-spec.md).
Hợp đồng đọc, tìm và quay lại từ v3/r2 còn hiệu lực ở phần R4 không sửa. Palette và các phần
còn lại của Bàn viết tiếp tục theo nguồn sở hữu tương ứng.
Ảnh tham chiếu không ghi đè quyền của server hay ngữ nghĩa lưu/transport. `source/tokens.css` của
prototype là nơi lấy giá trị ban đầu; từ nay sửa role trong `tokens.json`, không đồng bộ ngược từ
prototype. Lavender là palette riêng của Vink do người dùng chọn, không phải thương hiệu Cantos.

## Điều hướng và logo

- **Khám phá đứng trước Bàn viết.** Rail desktop/tablet: Khám phá → Bàn viết, Tôi ở chân rail.
  Bottom bar mobile: Khám phá → Bàn viết → Tôi. Quyết định 31/10 ghi đè thứ tự Bàn viết-trước
  trong ảnh chụp và ZIP v2.3. Thứ tự tab không tự đổi màn khởi động; `/` vẫn là trang công khai.
- [#7](https://github.com/loveoverflowcom/vink/issues/7) triển khai rail/bar, nhãn Khám phá,
  Tôi và preference Sáng/Tối/Hệ thống; spec và owner ở dưới.
- **Logo V/ngòi bút R1 #117** dùng SVG/PNG từ pack 08/11/2026.
  [Nguồn và cách dùng](../../apps/vink-web/assets/brand/README.md) giữ provenance, lockup
  Light/Dark và mark cho favicon/PWA. [ZIP, reference và giới hạn](brand/vpen-r1/README.md)
  phân biệt trace/concept với runtime. Logo chỉ ở góc phải Khám phá theo #102;
  full lockup thay cả mark + chữ riêng, mono mask cho forced colors. #34/Enso là lịch sử.

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
| `primary` / `on-primary` | `#6846a5` / `#ffffff` | `#d4bbff` / `#381c63` | Hành động chính, link |
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
| `bg-page` = `bg-nav` (R4.2) | `#efe8f6` (`surface-container`) | `#17121d` (`surface`) | Nền trang, Navigation Rail, thanh dưới — một màu |
| `bg-content` / `bg-inset` (R4.2) | `#f7f2fa` / `#e8e0f0` | `#211b29` / `#372e42` | Thẻ, hàng, sheet, menu (sáng hơn nền một bậc) / avatar chữ, nút icon trong thẻ |

Nhóm nội dung bằng fill và khoảng cách: thẻ, empty và notice bỏ viền; viền chỉ còn ở input
(nhận biết control), focus và đường phân cách footer. Từ R4.2 (#402, `DESIGN_PROPOSED`, token
0.8.0) nền trang, rail và thanh dưới dùng chung `bg-page`; thẻ rời bo `radius.card` 20, cách
`space.card-gap` 8; tiêu đề trang công cụ dùng `type.page-title`; control bán vuông bo
`radius.control-56/48/40` theo chiều cao ([R4](redesign-r4-ban-viet-kham-pha.md)).

## Chữ, hình, chuyển động

- UI: **Be Vietnam Pro** 400/500/600 (500 từ R3 #336), fallback Noto Sans, Segoe UI, Roboto, Helvetica Neue, Arial.
  Văn đọc, editor và tiêu đề: **Literata** variable (`wght` 200–900, `opsz` 7–72), fallback
  Noto Serif, Source Serif 4, Georgia, Times New Roman. Văn đọc
  `clamp(1.0625rem, 1rem + 0.25vw, 1.25rem)`, leading 1.85, measure 42rem.
- Font tự host, nguồn/hash/license và cách tải: [assets/fonts](../../apps/vink-web/assets/fonts/README.md).
- Font CMP Desktop và đối chiếu decoder Linux/macOS: [UI-REPAIR-08](ui-repair-08.md).
- R3 (#336): tiêu đề Literata 500 tracking 0, nhãn 14/500, vai trò `section`/`excerpt`, ô nhập
  `surface-container-high`; danh sách và vùng đọc phẳng. Xem [Lavender R3](lavender-r3-336/README.md).
- Radius 8/12/20/28 px, `hero` 32 px, pill. Spacing 0.25–3rem như trước.
- Motion 160/320 ms; `ease-emphasized` cho màu, `ease-spatial` (có vượt nhẹ) cho đổi shape.
  `prefers-reduced-motion` rút mọi transition/animation về gần 0 và giới hạn animation chạy tối
  đa một lần, nên vòng lặp không thành chớp vô hạn. Token UIS-01 tách chuyển tiếp hữu hạn
  (`duration.short/medium` → 0ms) khỏi vòng lặp tải (`duration.loop` giữ chu kỳ,
  `loop.iterations` → 0: dừng).
- UIS-01 thêm role chưa có consumer Web: `on-secondary`, `on-tertiary`, `warning`/`on-warning`,
  `on-success`, `info`/`info-container` (+ `on-`), `disabled-content`/`-container`,
  `state-focus`/`state-pressed`; vai trò chữ display/headline/title/body/label/caption/reading/code
  kiểm kê từ selector hiện có; kích thước chạm 44, icon 20/24, avatar 32/40, mật độ
  thoải mái/gọn. Mọi cặp chữ/nền khai báo trong `contrast` của `tokens.json` được generator tính
  lại cả hai theme và từ chối nguồn dưới ngưỡng; đó là số học token, không phải quét màn hình.
- UIS-02 (token 0.2.0) thêm 6 tông avatar `avatar-N`/`on-avatar-N` (lavender, mauve, rose, amber,
  green, blue; tách khỏi role trạng thái) và tông trung tính, mỗi cặp ≥4.5:1 hai theme; kích thước
  avatar theo R1.2 #100 ngày 09/11/2026 (feed 32/32, spotlight 40/38, reader 44/44, hẹp <760px);
  `density.control-height` 48px (gọn 44px = sàn `size.hit-target`); `measure-wide` 70rem và
  `size.grid-min-sm/md` 10/15rem. Component và contract: [UI System](../../packages/ui-system/shared/components.md).
- UIS-03 (token 0.3.0) thêm `duration.pulse` 1400ms (chu kỳ skeleton), `duration.loading-delay`
  300ms, `duration.loading-min-visible` 500ms, `duration.slow-notice` 8000ms, `duration.snackbar`
  6000ms (không phải chuyển động: không đổi khi giảm), `size.indicator` 20px và
  `size.edge-progress` 4px; thêm icon `warning`, `help`, `lock`, `block`, `refresh`. Khi nào dùng
  skeleton/arc/thanh, empty/lỗi/banner/snackbar/dialog: [Trạng thái và phản hồi](state-and-feedback.md).
- UIS-04 (token 0.4.0) đưa kích thước shell #7 vào token: `size.rail-expanded` 220, `size.rail-compact`
  96 (px Web, dp CMP), `size.nav-item` 52, `size.bottom-bar` 80; thêm icon Material `explore`, `menu`,
  `menu-open` (thay path SVG nét vẽ tay của shell). Contract shell/điều hướng/lớp phủ:
  [navigation.md](../../packages/ui-system/shared/navigation.md).

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

**Phạm vi hiện hành.** Dùng [LayoutSpec R4.3 §3](redesign-r4/layout-spec.md#3-shell-rail-và-thanh-dưới)
và [#413](https://github.com/loveoverflowcom/vink/issues/413) cho rail, Giao diện và cỡ chữ Web.
#413 đặt Giao diện tại Tôi cho khách và người đã đăng nhập và dùng Mặc định 100% / Lớn 150%.
Các tùy chọn màu và thanh điều hướng vẫn ở cùng vùng Giao diện.

**Lịch sử R1/v4 và audit ngày 22/11/2026.** Phần dưới lưu thiết kế và bằng chứng cũ, gồm chế độ
Giao diện trên rail. Mục **Tôi** trên rail và thanh dưới dẫn tới `/toi`.
Ở 760–1099px, rail thu gọn dành 96px cho điều hướng. Khi mở rộng, hộp thoại phủ lên nội dung
để giữ nguyên bề rộng của Bàn viết. Escape hoặc bấm nền đóng danh sách và trả focus về nút mở.
Ở rail xếp dọc, nền chọn bao quanh biểu tượng, nhãn nằm bên dưới và cả mục đều bấm được.
Các mô tả về nút Giao diện trong LayoutSpec r1 và bằng chứng bên dưới ghi lại hành vi ở thời điểm #7.

LayoutSpec r1, đã trình bày trước sửa UI ngày 01/11/2026; nền `develop@6648200`.
Nguồn: [v2.3](https://drive.google.com/file/d/1IlLVGiPHIg9DBohATdwd-38Gdy7z4U5g/view),
[Navigation Research](https://drive.google.com/file/d/1TDA4HoX5O-IHtQfu3RSnOks2WaYXkBaz/view),
quyết định Khám phá-trước 31/10 ở bảng nguồn. Hai không gian là tác vụ đọc/viết; trong bối cảnh
lịch sử `#7`, Tôi là tiện ích trên web để đổi giao diện. Đây là lý do thiết kế được ghi nhận, chưa phải
kết quả nghiên cứu người dùng hay quyết định auth D-05/S-03.

```text
Shell #shell
  Region #navigation — rail desktop/tablet (220px / 96px; modal #navigation-modal ở 760–1099px)
    RailToggle #rail-toggle — mở/thu rail
    Region #nav-primary — Khám phá → Bàn viết
    Region #account-bottom — Tôi (/toi), neo chân rail (từ #413 không còn nút Giao diện trên rail)
  Region #main — route hiện tại, editor giữ một instance
  Region #bottom-navigation — mobile: Khám phá → Bàn viết → Tôi
  Screen #me — /toi: Tài khoản → Giao diện: màu → cỡ chữ (Mặc định 100% | Lớn 150%) → rail
```

Owner: [lib.rs](../../apps/vink-web/src/lib.rs) giữ router và route `/` mặc định;
[shell.rs](../../apps/vink-web/src/shell.rs) ánh xạ `/ban-viet/:id/kiem-duyet` về Bàn viết và
điều phối rail mở rộng/modal trên cửa sổ vừa. Từ UIS-04 (#22) shell render bằng `AdaptiveScaffold`, `NavigationRail`,
`BottomNavigation` của UI system (`NavUtility` và `OverlayLayer` vẫn giữ generic trong UI system nhưng Web bỏ nút Giao diện và `#profile-menu` từ #413), nên ID con của rail suy từ `navigation`
(`#navigation-toggle`, `#navigation-primary`, `#navigation-account`, `#navigation-tooltip`) thay cho
`#nav-primary`/`#account-bottom` trong cây trên;
[appearance.rs](../../apps/vink-web/src/appearance.rs) giữ preference và initials;
[me.rs](../../apps/vink-web/src/pages/me.rs) giữ `/toi`. CSS dùng palette/role hiện có, không
thêm palette hay logo. `initials` ủy quyền cho `vink_ui::identity::initials` (UIS-02,
`avatar-identity-v1`): từ đầu/cuối, NFC và uppercase tiếng Việt, bỏ từ không có chữ/số (emoji,
dấu câu đứng riêng); không có tên thật thì trả dấu trung tính. An Nhiên → AN chỉ là fixture unit test.

- Từ 1100px mặc định 220px, riêng editor mặc định 96px. Preference `vink.ui.rail` ghi đè ngữ cảnh
  ở lớp cửa sổ này. Ở 760–1099px, xem [hợp đồng rail Medium](../../packages/ui-system/shared/navigation.md#chọn-khung-navlayoutresolve).
  Rail compact đặt icon trên nhãn và cuộn khi thiếu chiều cao.
- Dưới 760px rail ẩn và bottom bar có ba đích. Editor ẩn bottom bar,
  giữ đường Về Bàn viết hiện có. `ResizeObserver` đo chiều cao bar (gồm safe area) để dành đáy,
  cả khi tăng chữ 200%; không dùng chiều cao cứng làm khoảng bù.
- **Lịch sử.** [Giao diện R1 (#209)](me-preferences-r1-209.md) dùng cùng ba nhóm radio ở `/toi` và panel
  **Giao diện**. Tên nhóm đứng trên lựa chọn. Các lựa chọn tự xuống dòng theo độ rộng nhãn,
  với nền tonal và dấu check cho mục đã chọn. Trang có thẻ **Tài khoản** và minh họa sách–bút.
  Desktop và mobile đều dùng đích Tôi để tới `/toi`.
- Rail, thanh dưới và thẻ **Tài khoản** dùng chữ viết tắt của bút danh từ phiên hiện tại. Sau
  hydration, Web gọi `GET /api/v1/auth/session/account` (`no-store`) và kiểm lại khi cửa sổ nhận
  focus. SSR hiển thị biểu tượng tài khoản cùng nhãn khách. Bút danh của LocalFixture thuộc
  dữ liệu phát triển. Web tải thông tin tài khoản riêng qua API.
- Rail modal `#navigation-modal` (#415) giữ một modal hoạt động qua `OverlayLayer`. Escape, Đóng,
  bấm ngoài hoặc resize về mobile đóng modal và trả focus (hoặc vào main khi đổi breakpoint);
  Back/Forward đóng modal và giữ URL hiện tại, Back tiếp theo đi qua guard editor. Sự kiện `close`
  đến muộn của lần đóng trước không làm mất trạng thái của modal đã mở lại. (Overlay `#profile-menu`
  từ `#7`/`#209` đã được bỏ hoàn toàn ở #413.)
- Chọn màu theo hệ thống, cỡ chữ Mặc định (100%) hoặc rail tự động sẽ xóa giá trị ghi đè tương ứng trong
  `localStorage`; cỡ chữ Lớn ghi `vink.ui.text="150"` (giá trị cũ `"200"` được bootstrap quy đổi
  sang `"150"` trước CSS và first paint). Ba khóa là `vink.ui.theme`, `vink.ui.text` và `vink.ui.rail`.
  Bootstrap áp dụng preference trước CSS và đồng bộ lựa chọn SSR. Liên kết **Tôi** và back/forward
  đi qua `NavigationGuard`. Lưu lỗi/xung đột/treo giữ nguyên xác nhận rời không lưu.
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

## Dashboard Bàn viết #8

Phần dưới giữ lịch sử lát #8. Cơ chế đọc/tìm/sort hiện hành được thay bởi
[phân trang server #201](desk-server-pagination.md), dùng coordinator #202;
[owner và bằng chứng](../development/desk-pagination.md) ghi giới hạn và trạng thái mới.
Empty slots hiện dùng [composition WebP chung #259](empty-artwork-desk-259.md), thay riêng
panel/heading trong first-use và search miss; privacy và create/reconciliation không đổi.

LayoutSpec r1, trình bày trước sửa UI ngày 01/11/2026; nền `develop@cfad6a2`.
Kế thừa hướng A Lavender trong [v2.3](https://drive.google.com/file/d/1IlLVGiPHIg9DBohATdwd-38Gdy7z4U5g/view)
và [bàn giao](https://drive.google.com/file/d/1anrbq2AsNhNIXFCfO72B1yKddgQrVAep/view);
quyết định Khám phá-trước ở bảng nguồn vẫn có hiệu lực. Không chọn lại logo/navigation/auth.

```text
Screen #desk — tìm lại nháp riêng để viết
  Region #heading — Bàn viết + Tạo bản thảo (tonal, phụ)
  Region #resume — nháp mới cập nhật nhất + Viết tiếp (filled, chính)
  Region #drafts
    Region #draft-tools — tìm tiêu đề, sort, phạm vi nháp đã tải
    Region #draft-results — title đầy đủ, version, updated_at, Mở bản thảo
```

**Cơ chế:** `DeskIndex` dùng `LocalResource` chỉ tải sau hydration → `GET /api/v1/drafts`
→ principal phía server → `Application::list_drafts` → `PgStore` lọc owner, sắp
`updated_at DESC, work_id`, giới hạn 100. `DraftListView` lấy card đầu cho resume, không GET
manuscript. Search/sort chỉ đổi danh sách trên client. Lỗi GET không thành empty; retry giữ query
và sort. POST không idempotent; lỗi network, 503 hoặc phản hồi bất thường được giữ là chưa rõ,
không tự gửi lại. GET thành công sau đó chỉ mở lại khả năng tạo có chủ ý, không tuyên bố đã
chứng minh POST thành công/thất bại. 401/403 là từ chối rõ, cần tải lại để kiểm quyền trước
khi tạo tiếp. Khi rời route, bỏ tác dụng điều hướng của phản hồi tạo đến muộn.

**Lý do có nguồn:** LayoutSpec v2.3 chọn Viết tiếp trước danh sách, giữ nội dung tác phẩm thay
KPI mẫu. [Decision log](../architecture/decisions.md#3-không-dùng-một-state-enum-cho-toàn-bộ-vòng-đời)
tách working draft, revision và publication; có draft không suy ra Chưa đăng. Contract hiện có
chỉ cung cấp title/version/updated_at, không cung cấp profile/góp ý/thể loại/trạng thái xuất bản.
Giới hạn 100 là sự thật của query, không phải toàn bộ thư viện. Chọn bỏ excerpt để tránh thêm
request riêng cho resume là lựa chọn triển khai của lát cắt này, không phải quyết định dài hạn.

**Bố cục:** một cột dùng toàn chiều rộng khả dụng, giới hạn 64rem; không chừa cột support rỗng.
Resume dùng `primary-container`/`on-primary-container`, bo `hero`, title Literata co theo viewport,
CTA tối thiểu 56px. List cùng nền `surface-container-low`, row title wrap, không ellipsis;
metadata ghi giờ UTC rõ, `<time datetime>` giữ timestamp chính xác. Padding resume co theo
viewport với sàn 0.5rem để ở 320px/chữ 200% không ép từ ngắn như “Đường”, “Những” vỡ dòng;
đây là hiệu chỉnh sau khi mở ảnh, không đổi cấu trúc/CTA của r1. Tools flex-wrap, input/select
co theo màn. Mobile giữ resume → list, cuộn trang; không overlay/cuộn riêng hay sticky mới.
Sáng/tối giữ cùng cấu trúc; focus dùng `--vk-focus` hiện có. Controls có label, list có tên,
status/busy có semantics; thông báo không chỉ dựa vào màu.

**Trạng thái:** loading riêng; empty có duy nhất CTA “Viết bản thảo đầu tiên”; không kết quả giữ
resume/query và cho xóa query. Lỗi/401 có thử mở lại và đường đọc công khai, không nút đăng nhập
giả hoặc chi tiết fixture/D-05 trong flow. Create pending khóa nút và không tải lại chồng POST;
unknown yêu cầu đối chiếu, GET lỗi vẫn giữ unknown, GET thành công cho “Tạo thêm bản thảo”.

**Delta với mockup:** không excerpt, genre, word count, feedback, bộ lọc Đã đăng/Chưa đăng,
profile hay tổng thư viện. Không có support data nên bỏ cột 30%; không thêm endpoint/schema.
Chỉ phủ discoverability của UX-03 và một phần UX-01; giữ semantics WR-01/03, không hoàn tất
publication/moderation, auth, autosave hay S-01.

Owner: [desk.rs](../../apps/vink-web/src/pages/desk.rs), [draft_list.rs](../../apps/vink-web/src/draft_list.rs),
[vink.css](../../apps/vink-web/style/vink.css). [Feature map](../development/feature-map.md) dẫn
assertion và giới hạn; PNG/viewport/theme nằm trong `target/verify-web/screenshots/contexts.json`.

### Bằng chứng dashboard ngày 01/11/2026

Nền `develop@cfad6a2`, diff `codex/private-drafts-dashboard` cho #8; Linux, Rust 1.99.1,
Node 24.22.2, Chrome 156.0.8078.73 headless, DPR 1, PostgreSQL local với database thử riêng.

- `PASS` — `CARGO_BUILD_JOBS=2 cargo xtask verify quick`: fmt, clippy server/WASM,
  54 unit/contract tests, repository checks (links, metadata, boundaries, fonts, editor packages).
- `PASS` — `CARGO_BUILD_JOBS=2 cargo xtask verify web`: main + after-restart, không console
  error/exception. Dashboard có fixtures 100/rỗng/401/503, không N+1; POST thật commit rồi bỏ
  ack 201, GET lỗi vẫn khóa, GET thành công tìm lại đúng một nháp mà không replay POST;
  POST trả về muộn sau khi rời route không điều hướng. Guard/save/conflict/preview/export và
  dữ liệu qua restart vẫn qua. Hai job chỉ giới hạn build, không giảm assertions.
- `SCREENSHOT_INSPECTED` — mở dashboard 1440/1024/390/320px, list 320px, title 320px/chữ 200%,
  empty/lỗi 1440/320px, sáng/tối. Tiêu đề dài NFD giữ nguyên, các từ “Đường”, “Những” không
  bị ép tách dòng sau hiệu chỉnh padding; có assertion hình học riêng. Không tràn ngang trong
  các cảnh đã kiểm. Metadata viewport/theme/font/scroll ở `screenshots/contexts.json` của lane.
- Chín cặp chữ dashboard lấy từ `getComputedStyle`: thấp nhất 6.28:1 sáng và 7.87:1 tối;
  JSON ở `target/verify-web/desk-contrast-{light,dark}.json`. Chưa đo mọi hover/disabled.
- Rà source/diff theo owner UI, API, application và store: không thay authority, query hay
  contract; không còn finding cần sửa trong phạm vi đã kiểm. Đây là review cùng tác vụ,
  không phải một GitHub approval độc lập.

`NOT_RUN`: mobile/IME hệ điều hành thật, Safari/Firefox, zoom trình duyệt thật và screen reader
có tên. Browser dùng principal fixture local; chưa chứng minh production auth, publication,
full-library search hay hoàn tất toàn bộ UX-01/UX-03/WR-01/WR-03.

## Khám phá và reader công khai (#9)

[LayoutSpec và delta capability lớp đọc v3](public-reading.md) ghi nguồn #9/#14, cơ chế
SSR/API, bố cục hữu hạn và phạm vi kiểm chứng. Lớp đọc không hoàn tất tương tác cộng đồng.

## Logo Enso #10

**Lịch sử:** phần dưới ghi implementation 01/11; asset và hướng nguồn được thay bằng
[selected pack #34](#logo-final-selected-34) ngày 03/11/2026.

LayoutSpec vùng r1 đã trình bày trước patch ngày 01/11/2026, nền `develop@738e6c6`.

```text
Region #brand — cùng link trang chủ, accessible name và wordmark Vink
  Region #brand-mark — thay V tạm bằng Enso AVIF, vùng vuông 2.25rem
  Region #brand-name — Vink, giữ cách ẩn trên rail compact
Region #head-icons — favicon và manifest theo theme, ngoài hydrated body
```

Rail 220/96px và header mobile dưới 760px dùng cùng mark, giữ thứ tự Khám phá → Bàn viết → Tôi.
Không đổi CTA, route hay owner editor/dirty guard. CSS đổi asset ngay theo override hoặc OS;
preload cặp AVIF 96px và kích thước cố định tránh nhảy layout do tải ảnh. Bootstrap khôi phục
theme trước stylesheet, đồng bộ head links, theo dõi override và OS; storage lỗi vẫn cho đổi
trong phiên. Bản fallback favicon SVG tự theo OS khi JavaScript không chạy.

Không thêm palette vào app. Logo trong suốt được kiểm trên `surface`/`surface-container` của
cả hai theme; lông/ngòi ở 48px và silhouette ở 24px cần mở contact sheet từ derivative thật.
Logo trang trí có `aria-hidden`, link giữ nhãn “Vink — trang chủ”, focus do shell hiện có giữ.
Không tuyên bố mọi pixel gradient của artwork đạt ngưỡng tương phản chữ; wordmark/focus vẫn
dùng token đã đo ở trên. Asset app icon chưa được nghiệm thu trên launcher/OS thật.

`brandAssets` kiểm HTTP/MIME, decode AVIF ở đủ sáu cỡ, manifest/icon URL và theme trên HTML thật
khi chặn riêng hydration bundle; `assertBrand` kiểm theme/geometry, kể cả reload, OS và denied
storage. `shellWithDirtyEditor` giữ kiểm instance, selection, text, save và brand dirty guard.
PNG/manifest môi trường do lane web tạo là bằng chứng render; kết quả lần chạy được ghi ở PR,
không suy ra PASS chỉ từ các assertions tồn tại.

## Logo Final Selected #34

**Lịch sử:** phần artwork dưới đây được Web V-pen R1 #117 thay thế; [baseline mới](brand/vpen-r1/README.md).
Vị trí Explore theo #102 vẫn giữ hiệu lực. Nội dung/evidence cũ giữ đúng revision gốc.

LayoutSpec vị trí hiện hành theo Explore Header R2 của [PR #105](https://github.com/loveoverflowcom/vink/pull/105).
Nguồn artwork: [selected pack, board và quy tắc trung thực](brand/final-selected/README.md).

```text
Khám phá /
  Region #discovery-tools — ô tìm và link Vink
    #brand-mark — SVG chuẩn Light/Dark, khung vuông 2,25rem
    #brand-name — Literata
  Region #explore-heading — H1 Khám phá và lời dẫn
Region #head-icons — compact favicon + PNG/SVG trong manifest
Region #source-record — ZIP nguyên bản + SHA, không phục vụ trong site
```

Logo căn phải ở đầu trang Khám phá. Desktop đặt ô tìm cùng hàng bên trái ở cả hai độ rộng rail.
Mobile đặt ô tìm ở hàng dưới logo, trước H1 và lời dẫn. CSS chọn mark chuẩn theo
`data-theme`/OS và giữ khung vuông trước khi decode. Favicon compact theo cùng preference qua
bootstrap trước stylesheet, với fallback theo OS. Link Khám phá của rail và thanh dưới vẫn
là đường điều hướng chính.

Board có hiệu ứng và hình khối khác SVG/PNG; source production là asset đã chọn nguyên byte,
board chỉ tham khảo trình bày. Không khẳng định visual parity với board hay nghiệm thu app
của người dùng. Compact dùng cho 16/24/32px; PNG pack có đủ 24–512px.

Owner: `apps/vink-web/assets/brand` giữ source/provenance, `vink.css` chọn mark theo theme,
`lib.rs` preload/head, `appearance.rs` đồng bộ favicon/manifest; `xtask` copy PNG/SVG/manifest
và kiểm SHA. `brandAssets`/`assertBrand` theo tên mới, giữ các kịch bản hydration, reload,
OS theme, denied storage và editor guard. Kết quả chạy và ảnh được mở ghi ở PR cho đúng HEAD;
không suy ra PASS từ sự tồn tại của assertions.

[#102 — LayoutSpec R1 và ảnh trước/sau](evidence/navigation-rail-102/README.md).

[#135 — Select Web R1.5: geometry và native states](select-r15.md).

- [Hai hàng chờ kiểm duyệt hữu hạn #205](moderation-queue-pagination.md): state/continuation/recovery riêng, giữ form lý do và session fence.
