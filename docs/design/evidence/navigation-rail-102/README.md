# Navigation Rail R1 · #102

[Nguồn issue](https://github.com/loveoverflowcom/vink/issues/102) ·
[LayoutSpec trên Drive](https://drive.google.com/file/d/1ymjidgI_NN0IT3f5ouoWXv4QCmHd8afF/view)

Ảnh dưới đây ghi lại R1. Vị trí logo hiện hành được mô tả ở [Explore Header R2](../../README.md#logo-final-selected-34).

## Bố cục đã triển khai

```text
Web ≥760px
  Rail 96/220px
    Nút mở/thu → Khám phá → Bàn viết
    Tôi ở chân rail
  Main
    Khám phá / → ô tìm hiện hành → heading (logo Vink + H1) → danh sách
    Reader / Bàn viết / Editor / Tôi / 404 → nội dung riêng
Web <760px
  Cùng heading Khám phá, thanh dưới Khám phá → Bàn viết → Tôi
  Editor dùng lối “← Bàn viết”
```

Chọn A với `menu` / `menu-open` và cỡ icon 20px đã có trong UI system. Hai hình
cùng trục, vùng bấm 48px ở mật độ mặc định. Màu nghỉ theo `on-surface-variant`.
Ở 96px icon nằm trên nhãn. Rail 220px giữ hàng ngang và chuyển sang xếp dọc khi
chữ lớn cần cả dòng cho nhãn. Giới hạn inset ngang để giữ khoảng cho từng từ tiếng Việt.

Đặt logo #34 cạnh H1 vì Khám phá sở hữu heading và có thể dùng chung một vị trí trên
desktop/mobile. Hàng logo và H1 tự xuống dòng theo chỗ trống. Màn đọc có Khám phá
đang chọn và giữ heading riêng. Bản mobile của editor tiếp tục dùng link Bàn viết
đi qua cùng guard.

## Đối chiếu trước và sau

Ảnh chụp từ app SSR/hydration thật và dữ liệu mẫu trên PostgreSQL 17.11, Chrome
157.0.8103.38 headless Linux, DPR 1. “200%” là cỡ chữ root, đo độc lập với zoom.
Ảnh trước lấy từ `82e86ae`, trước khi #104 cập nhật trình soạn. Các cặp Khám phá
giữ cùng route, theme, viewport và bề rộng rail. Ca editor dùng ảnh sau trên nền
editor R5 để ghi lại đúng UI đã kiểm.

| Ca | Điều đã kiểm | Ảnh |
| --- | --- | --- |
| V01 · 1440×900 Sáng | Toggle là control đầu, vùng bấm 48px. Icon Khám phá, Bàn viết và Tôi cùng trục với toggle. | 96px [trước](before-v01-96.png) · [sau](after-v01-96.png), 220px [trước](before-v01-220.png) · [sau](after-v01-220.png) |
| V02 · 1440×900 Tối | Cùng phân cấp và hình học. Mục đang chọn giữ nền Lavender. | 96px [trước](before-v02-96.png) · [sau](after-v02-96.png), 220px [trước](before-v02-220.png) · [sau](after-v02-220.png) |
| V03 · 1024×900, 200% | DOM Range kiểm từng từ trong rail. Chọn 220px ở tablet vẫn giữ route và chiều rộng. | 96px [trước](before-v03-96.png) · [sau](after-v03-96.png), 220px [trước](before-v03-220.png) · [sau](after-v03-220.png) |
| V04 · 834×360 | Tab đến toggle và Tôi ở cỡ chữ 100/200%. Tooltip nằm ngoài vùng cuộn, trong viewport. | [trước 200%](before-v04.png) · [sau 100%](after-v04-text100.png) · [sau 200%](after-v04.png) · [tooltip](after-v04-tooltip.png) · [Tôi](after-v04-account.png) |
| V05 · 320/390px | Bottom bar giữ thứ tự, chiều cao theo nhãn ở 100/200%. Logo nằm trong heading và không chồng ô tìm. | 320px/Tối/200% [trước](before-v05-320.png) · [sau](after-v05-320.png), 390px/Sáng/100% [trước](before-v05-390.png) · [sau](after-v05-390.png) |
| V06 · Editor có chữ chưa lưu | Toggle, theme và resize giữ EditorView, nội dung, selection và phiên bản trên server. Link Khám phá ở rail và Bàn viết trên mobile kích hoạt guard. Ctrl/Cmd+Z giữ hành vi undo. | 390px [sau](after-v06.png) |
| V07 · SSR và hydration | Khám phá có một logo, các route còn lại không có brand shell. Kiểm favicon/manifest và mark với Light, Dark, System theo OS sáng và tối. | Trước hydration, System [sáng](after-v07-system-light.png) · [tối](after-v07-system-dark.png) |
| V08 · Keyboard và trạng thái | Tab có focus ring, Enter/Space đổi chiều rộng một lần, Escape ẩn tooltip. Kiểm forced colors và giảm chuyển động trong Chrome. | [Forced colors](after-v08-focus.png) · [hover tooltip](after-v08-hover.png) |

## Acceptance và kết quả cục bộ

| Issue | Bằng chứng |
| --- | --- |
| AC-01 | LayoutSpec và lựa chọn icon/brand ở trên. |
| AC-02, AC-06 | `railReview`, `adaptiveShell` và hồi quy bottom bar #88 trong browser smoke. V01–V05. |
| AC-03 | `serverHtml`, `brandAssets` và `assertBrandPlacement` kiểm HTML trước JS, điều hướng sau hydration và asset theo theme. V05, V07. |
| AC-04 | `navigation` ở preview và `adaptiveShell` ở app kiểm hành vi phím, vòng focus, tooltip và tên hành động. V08. |
| AC-05 | `adaptiveShell` kiểm reload/tùy chọn rail và Tôi. `shellWithDirtyEditor` và `mainPhase` kiểm EditorView, chữ, selection, undo, guard và số lần ghi. V06. |
| AC-07 | `verify quick` kiểm owner, generator và provenance. Diff dưới `apps/vink-web/assets/brand` chỉ đổi README. |
| AC-08 | Ảnh runtime ở bảng trên, cùng kết quả cục bộ dưới đây. |

- **PASS** `cargo xtask verify quick` tại `3234129`, nền `develop@b4134a7`.
- **PASS** `cargo xtask verify web` tại `3234129`. Cả ba phase browser `main`, `after-restart` và `endorsement` hoàn tất với PostgreSQL cục bộ. Lượt trước rebase có một lần lỗi timing ở mốc 119 giây của READ-02. Lượt trên nền R5 đi qua cùng assertion.
- **PASS** `cargo xtask ui-preview leptos verify` tại `2f9916a` trước rebase, 774 ảnh. Trên `3234129`, build lại preview và chạy ba ca `navigation`, `navigationFit` và `compactVietnameseWords` từ cùng smoke runner. Cả ba qua, sinh 12 ảnh.
- **NOT_RUN** thiết bị thật, screen reader và Compose. Bằng chứng ở đây là Web trên Chrome headless với viewport và media mô phỏng.
