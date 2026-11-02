# UI system — shared declarations

Không runtime, không backend. Ba nguồn, đều do người sửa; mọi file renderer sinh từ đây bằng
`cargo xtask ui-tokens`.

| File | Sở hữu |
| --- | --- |
| [tokens.json](tokens.json) | `version` (semver), `fonts` (face tự host, file trong `apps/vink-web/assets/fonts`), `primitives` (màu hex theo họ/tông L\*), `tokens` (semantic + recipe), `contrast` (cặp và ngưỡng) |
| [tokens.schema.json](tokens.schema.json) | Hợp đồng cho editor. Bộ kiểm thật là `xtask/src/ui_tokens.rs` (alias, vòng, type, theme đủ, tương phản) |
| [preview-catalog.json](preview-catalog.json) | ID entry/kịch bản dùng chung cho hai preview, trạng thái từng renderer |
| [fixtures/foundation.json](fixtures/foundation.json) | Đồng hồ cố định, seed, chữ mẫu tiếng Việt `[Mẫu]` (có chuỗi NFD lưu bằng escape) |

## Lớp token

- **Primitive** (`primitives`): chỉ màu `#rrggbb` thường, đặt tên `<họ>.<L*>` (`lavender.38`).
- **Semantic** (`tokens`, có `light` + `dark`): mỗi theme trỏ một primitive, có thể kèm `alpha`.
  Thiếu theme, primitive lạ hay hex trực tiếp đều bị từ chối.
- **Recipe** (`tokens`, `value: "{color.x}"`): alias của role khác; giải theo từng scope. Alias treo,
  sai type và vòng bị từ chối.
- Khác: `dimension` (`value` theo CSS px/dp, `web`: `rem|px`, tùy chọn `max`+`fluid`, `compact`),
  `number`, `typography` (family/size/weight/lineHeight/tracking, cho phép alias), `fontFamily`
  (`native`: sansSerif|serif|monospace), `duration` (ms nguyên dương, `reduced: 0`), `iterations`
  (`infinite`, giảm → `0`), `cubicBezier`, `shadow`.

Tên CSS: `--vk-` + id, bỏ tiền tố `color.` (`color.surface` → `--vk-surface`); `css` ghi đè khi cần
giữ tên cũ (`shadow.raised` → `--vk-shadow`).

## Đăng ký preview

Mỗi entry khai `status.leptos` và `status.cmp` từ `statuses`. `implemented-unverified` nghĩa là
đã đăng ký và render trong preview đó, **không** có nghĩa đã nghiệm thu runtime, accessibility hay
thiết kế. Component mới phải có mặt ở cả hai preview trước khi app dùng; khác biệt riêng nền tảng
dùng `unsupported` kèm lý do, không bịa bản đối ứng. Test ở hai preview so danh sách đăng ký với
catalog.
