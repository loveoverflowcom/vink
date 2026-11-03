# UI-REPAIR-01 — Reader hẹp và chữ 200%

Ngày kiểm: 03/11/2026 (UTC+07). Baseline UI: `develop` tại
`585ab7edc2c09a79246061e075f0baf640755282`; bản sửa: `codex/ui-text200-narrow`.
Runtime: Leptos SSR + hydrate, Axum, PostgreSQL 17.11 tạm trên loopback,
Chrome headless 156 trên macOS, DPR 1, viewport cao 900 CSS px.
100/200% là tăng root font 16/32px, không phải zoom trình duyệt.

## Lỗi và nguyên nhân đã đo

Reader dùng `ReaderHeader` trong `vink-patterns`, H1 mang vai trò `Display` ở mọi
bề rộng. Với title mẫu dài tiếng Việt NFC/NFD, cỡ 64px ở root 200% làm title chiếm
nhiều màn hình. Bottom bar dùng caption 26px nhưng thừa kế leading body 1,6,
padding tăng theo rem và minimum 5rem, nên cao 179px.

Nguồn tham chiếu: [Reader baseline trên Drive](https://drive.google.com/file/d/1ha-8Pnsu_tq822WE8EI141sHTQmPwseS/view),
[Explore baseline trên Drive](https://drive.google.com/file/d/1fE8eetSVnr-bvaUX7BIiKEl6I1_D4ntQ/view),
[current design 02/11](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view).
Đã mở hai ảnh baseline tương ứng trong bundle local của run `2026-11-03T123450+0700_develop_585ab7ed`;
manifest pin đúng SHA trên. Bảng dưới dùng phép đo baseline mới với cùng fixture,
chỉ thêm instrumentation vào script chụp, trước khi build CSS sửa.

## Bố cục trước → sau

Spec đã được trình bày trong chat trước khi sửa CSS/script:

```text
#reader
  #return-link
  #reader-header
    publication-label
    responsive-title — H1 đầy đủ, chọn/copy được
    author-byline
  #reader-body
    full-public-text — nguyên đoạn, dòng, khổ thơ
#mobile-shell
  #top-bar
  #main
  #bottom-navigation
    explore “Khám phá”
    desk “Bàn viết”
    me “Tôi”
```

Ở bề rộng dưới 760px, title Reader dùng size/leading/tracking của token `type.title`;
font serif, weight và semantic H1 giữ nguyên. `overflow-wrap: anywhere` chỉ áp
heading trực tiếp trong Reader header để chuỗi không có khoảng trắng reflow được.
Bottom bar dùng leading caption 1,4, padding `space.1` và minimum
`size.bottom-bar` 80px; nội dung và safe-area tiếp tục làm bar lớn lên. ResizeObserver
vẫn đo chiều cao thực để chừa cuối trang và scroll margin của control.

## So cùng fixture, light, chữ 200%

| Geometry (CSS px) | 320 trước | 320 sau | 390 trước | 390 sau |
| --- | ---: | ---: | ---: | ---: |
| Title: cỡ chữ | 64 | 44 | 64 | 44 |
| Title: chiều cao | 1689,53 | 857,81 | 1151,95 | 629,06 |
| Header: chiều cao | 2065,09 | 1233,38 | 1396,33 | 873,44 |
| Chữ nội dung đọc | 34 | 34 | 34 | 34 |
| Nhãn navigation: cỡ chữ | 26 | 26 | 26 | 26 |
| Bottom bar: chiều cao | 179,19 | 136,78 | 179,19 | 136,78 |
| Target đầu: rộng × cao | 85,66 × 147,19 | 85,66 × 120,78 | 109 × 147,19 | 109 × 120,78 |

Số đo gốc và danh sách ảnh đã mở: [geometry.json](evidence/ui-repair-01/geometry.json).
Ảnh nguyên viewport:

| Viewport / chữ | Trước | Sau |
| --- | --- | --- |
| 320 / 200% / light | [Reader](evidence/ui-repair-01/before-reader-320-200-light.png) | [Reader](evidence/ui-repair-01/after-reader-320-200-light.png) |
| 390 / 200% / light | [Reader](evidence/ui-repair-01/before-reader-390-200-light.png) | [Reader](evidence/ui-repair-01/after-reader-390-200-light.png) |

[Publication seed thật, 390 / 200% / dark](evidence/ui-repair-01/after-reader-real-390-200-dark.png).
Title mẫu rất dài vẫn cần cuộn; toàn bộ title và nội dung tiếp tục có trong luồng
đọc. Không đặt mục tiêu đưa mọi title dài vào một viewport bằng cắt chữ hoặc khóa scale.

## Kiểm chứng

- **PASS** `cargo xtask verify quick`: token drift, fmt, clippy SSR/hydrate/preview,
  unit/contract/doctest, links và boundary/asset guards.
- **PASS** `cargo xtask verify web`: PostgreSQL thật, SSR/public revision/privacy,
  hydration, return query/Back/Forward, 404/503, editor dirty guard, save/conflict/restart.
  Kiểm mới: title đầy đủ/chọn được, tỷ lệ chữ title/body ở màn hẹp, ba nhãn nằm trong
  target ≥44px, tối đa hai dòng, bar ≤18% viewport 900px, Tab/focus/cuối trang clear bar.
- **PASS** `cargo xtask ui-preview leptos verify`: build, component/state/navigation
  scenarios, reflow 100/200%, không có console error và 642 capture của catalog.
- Ma trận **SCREENSHOT_INSPECTED**: Reader 320/390/834/1440 × light/dark × 100/200%,
  với publication seed thật, title dài + nội dung thơ/văn và document rỗng; shell
  320/390/1024 × light/dark × 100/200%; thêm title không có khoảng trắng tại
  320/390 × light/dark × 200%, reduced motion. Đã mở 64 ảnh qua 16 sheet có nhãn,
  mở riêng ảnh hẹp và vùng navigation/body; sheet chỉ ghép để review, PNG gốc không đổi.
  Ảnh/contexts đầy đủ ở `target/verify-web/screenshots`; CI lưu trong artifact
  `browser-evidence` theo retention hiện có (14 ngày).
- **FAIL** có chủ ý khi chạy assertion mới trên CSS baseline: bar tại
  390px/200% vượt 18% viewport. Chạy sau sửa đạt; baseline smoke cũ đạt nhưng không
  có assertion về mức chiếm viewport.
- **FAIL** lần đầu chuẩn bị `verify web`: PostgreSQL container không chạy. Đã dùng
  cluster PostgreSQL local tạm để chạy lại lane đạt; không đổi cấu hình dùng chung.
- **NOT_RUN** thiết bị mobile, safe-area inset khác 0 trên thiết bị thật,
  Safari/Firefox, OS IME và screen reader. Công thức safe-area không bị loại bỏ;
  ảnh/CDP không được coi là kiểm chứng các nền tảng này.

## Self-review theo selector

`.vk-reader__header > .vk-ui-heading` chỉ chạm H1 của Reader pattern, bao gồm app và
gallery; không khớp title Explore, Desk resume, editor hay dialog preview.
`.vk-ui-bottom-nav` và `.vk-ui-bottom-nav .vk-ui-nav-item` chỉ đổi bar Web, bao gồm
gallery; rail, tooltip, dialog overlay không khớp. Minimum target, inset, focus ring,
ResizeObserver, route selection và `NavigationGuard` giữ nguyên owner. Runtime smoke
đã kiểm dirty editor, overlay, rail toggle/tooltip và resize. Không sửa token sinh,
Explore feed, Desk resume composition, breakpoint hay logo.
