# Panel Giao diện và Tôi — nhịp spacing và selected R1 (#209)

**Trạng thái:**
- **Product / UX:** `DESIGN_PROPOSED` (PR 2/2 của [#209](https://github.com/loveoverflowcom/vink/issues/209), sau
  [PR1 Dialog #208](dialog-anatomy-r1-208.md)). Không có quyết định sản phẩm mới.
- **Reference class:** `NO_APPROVED_REFERENCE`. Concept R1 là ứng viên bố cục, không phải style anchor đã duyệt;
  quy tắc gutter [#174](ui-ux-quality-174.md) được duyệt riêng và không suy rộng sang concept.
- **Implementation level:** `web` (`apps/vink-web`). Không đổi token, primitive `RadioGroup`, storage key,
  bootstrap hay CMP.

## Phạm vi và lựa chọn

**CHOICE-01 — A (giữ lựa chọn content-sized `flex-wrap`, gom recipe CSS).** Lý do: nhãn Việt dài và chữ 200% xuống
dòng tự nhiên mà không cần cột đặc biệt; DOM/state không đổi nên không có nguy cơ remount radio khi reflow. B (grid
thích ứng) chưa có bằng chứng render cho thấy nhóm rõ hơn ở cả hai consumer, nên không triển khai.

Một recipe `.vk-profile-menu, .vk-appearance-card` trong `apps/vink-web/style/vink.css` nay là owner duy nhất của:

| Thuộc tính | Giá trị | Ghi chú |
| --- | --- | --- |
| Gutter | `--vk-appearance-gutter: clamp(space-3, 7vw, space-6)` | 24px tại ≥ 390px/100%; một owner cho panel và thẻ Tôi (bỏ `clamp(.875rem,3vw,…)` và `clamp(14px,3vw,…)` riêng) |
| Surface | `surface-container-high`, `radius-xl` | Một surface chính, không thêm khung lồng |
| Khoảng giữa nhóm | `space-6` | Thay divider `border-block-start` của thẻ Tôi |
| Nhãn → lựa chọn | `space-2` | Giữ `h3` + `fieldset` hiện có |
| Giữa các control | `8px`, control `flex: 1 1 auto`, `min-block-size: 48px` | Nhãn đầy đủ, không cắt, không thu chữ |
| Idle | `surface-container-low` | Hover `surface-container` |
| Selected | `primary-container` / `on-primary-container` + check | Không còn `primary` đặc ở ba nhóm; check giữ chỗ 16px nên đổi giá trị không xê dịch nhãn |
| Focus | outline 2px flush, `--vk-focus`; trên selected `on-primary-container`; forced-colors giữ `Highlight`/`HighlightText` | Focus tách khỏi selected |

## LayoutSpec đã triển khai

```text
Overlay #profile-menu (modal, neo rail, scrim thật)        Screen #me > .vk-appearance-card
  Header (Giao diện / Đóng Giao diện)                         Title (Giao diện)
  Region .vk-appearance-region  (gap space-6)                 Region .vk-appearance-region
    Row  h3 (icon + nhãn) → fieldset radio (gap space-2)       Row  như panel; ≥ 42rem: h3 30% | choices
    Row  …                                                      Row  …
  Destination #open-me (ngoài /toi)
```

Giữ nguyên: ba nhóm, native radio, `AppearanceChoices`, storage key và bootstrap, inert/scrim/Back/Escape/outside,
`close_profile` trả focus và scroll, rail anchor, mobile đi tới `/toi` (không có route panel mới).

## Kiểm chứng

Nguồn: `develop@ef7da9d` + thay đổi PR (cây làm việc dirty trước commit), Chrome headless qua CDP,
fixture tổng hợp của repository, PostgreSQL 17.11 tạm.

| Lệnh / boundary | Kết quả |
| --- | --- |
| `cargo xtask verify web --scenario main` (`web`, local PG + Chrome `--no-sandbox` do sandbox công cụ) | PASS lần cuối, exit 0 (3906 dòng `ok`, gồm 744 dòng `appearance recipe`). Hai lần FAIL trước đó do chính assertion mới (fill đang transition; xem dưới) đã sửa trước kết quả cuối |
| `appearanceRecipe` (mới, `browser-smoke.mjs`) | Đo computed trên panel (nhiều ma trận rail/theme/chữ/chiều cao 360) và `/toi`: gutter `clamp(space-3, 7vw, space-6)` ±1px ở cả bốn phía, surface `surface-container-high`, khoảng giữa nhóm `1.5rem`, nhãn→lựa chọn `.5rem`, giữa control `8px`, không rule/`hr`, selected = `primary-container`/`on-primary-container`, idle = `surface-container-low` (hover `surface-container`) |

Các FAIL đã sửa: (1) `idle` đo khi `:hover` → so với `surface-container`; (2) `idle` đo giữa transition → chờ
`document.getAnimations()` xong như `appearanceFocusContrast`. Cả hai là lỗi của assertion mới, không phải của giao diện.

Số đo tương phản tính từ token (không suy từ ảnh): chữ selected sáng `10,84:1`, tối `8,23:1`; chữ idle sáng
`15,56:1`, tối `13,95:1`; focus `--vk-focus` trên idle sáng `5,40:1`, tối `9,85:1`. Selected so với surface panel chỉ
`1,00:1` (sáng) / `1,28:1` (tối): màu đơn lẻ không đủ phân biệt, nên dấu ✓ (không dựa vào màu) và trạng thái native
`checked` mang nghĩa chính; forced-colors giữ `Highlight`/`HighlightText`.

### Đánh giá 5 chiều

| Chiều | Kết quả | Bằng chứng |
| --- | --- | --- |
| Functional | Đạt trong smoke | Chọn bằng UI ở 100/200%, reload, system theme, storage bị từ chối, Arrow/Space/Tab, close/Back/route, editor dirty (phần sẵn có của `main`) |
| Geometry | Đạt | `appearanceRecipe` + kiểm panel trong viewport, một scroller, nhãn nguyên từ, control ≥48px ở 320/390/834/1440 và 1440×360 |
| Visual | Đạt, có lưu ý | Đã mở ảnh thật sáng/tối; selected nhẹ hơn, không còn ba mảng primary đặc. Light: selected và idle gần cùng độ sáng, phân biệt bằng hue + ✓ |
| Accessibility / Responsive | Đạt trong phạm vi | Focus tách selected, outline ≥3:1, forced-colors; 200% nhãn đầy đủ, panel cuộn một vùng |
| Interaction | Đạt trong smoke | Focus trả về trigger/editor, rail đổi vị trí panel không nhảy focus |

Ảnh sau sửa (không có ảnh “trước” cùng build nên không khẳng định mức cải thiện):
[panel 1440 sáng 100%](../development/evidence/appearance-r1-209/after-panel-1440-light-text100.png),
[panel 1440 tối 200%](../development/evidence/appearance-r1-209/after-panel-1440-dark-text200.png),
[Tôi 390 sáng 200%](../development/evidence/appearance-r1-209/after-390-me-light-text200.png),
[Tôi 320 tối 200%](../development/evidence/appearance-r1-209/after-320-me-dark-text200.png).

**NOT_RUN / gap:** OS IME, browser zoom gốc (chỉ root text 200%), thiết bị/bàn phím mềm thật, screen reader,
Safari/Firefox, CMP/native, `cargo xtask ui-preview` (không đổi primitive), ma trận chụp đủ `/toi` 1440 sáng/tối chưa mở
từng ảnh. Không kiểm/chờ GitHub CI. Concept R1 chưa là style anchor đã duyệt; không đánh dấu `Accepted`.
