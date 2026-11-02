# UI system — shared declarations

Không runtime, không backend. Các nguồn dưới đây đều do người sửa; mọi file renderer sinh từ đây
bằng `cargo xtask ui-tokens`.

| File | Sở hữu |
| --- | --- |
| [tokens.json](tokens.json) | `version` (semver), `fonts` (face tự host, file trong `apps/vink-web/assets/fonts`), `primitives` (màu hex theo họ/tông L\*), `tokens` (semantic + recipe), `contrast` (cặp và ngưỡng) |
| [tokens.schema.json](tokens.schema.json) | Hợp đồng cho editor. Bộ kiểm thật là `xtask/src/ui_tokens.rs` (alias, vòng, type, theme đủ, tương phản) |
| [preview-catalog.json](preview-catalog.json) | ID entry/kịch bản dùng chung cho hai preview, trạng thái từng renderer |
| [fixtures/foundation.json](fixtures/foundation.json) | Đồng hồ cố định, seed, chữ mẫu tiếng Việt `[Mẫu]` (có chuỗi NFD lưu bằng escape) |
| [fixtures/identity.json](fixtures/identity.json) | `avatar-identity-v1`: golden vectors initials/tone (NFC/NFD, Đ/đ, Ỗ/Ữ, emoji, một từ, blank), người `[Mẫu]`, ảnh PNG mẫu và byte AVIF hỏng để kiểm fallback |
| [icons.json](icons.json) | Registry icon: path nguyên văn từ Material Icons (Apache-2.0, commit pin), SHA-256 file upstream được generator dựng lại và so; [license](icons/LICENSE-material-design-icons.txt) |
| [components.md](components.md) | Contract component UIS-02: LayoutSpec/anatomy, props, sự kiện, owner, trạng thái không hỗ trợ, quy tắc identity |
| [states.md](states.md) | Contract UIS-03: trục trạng thái và tổ hợp hợp lệ, `Problem`/intent phục hồi, chính sách vị trí/announce, motion/`LoadingGate`, LayoutSpec, recipe playground |
| [fixtures/states.json](fixtures/states.json) | Vectors chung: 112 tổ hợp vùng, intent theo `ProblemKind`, 48 placement, 20 notice, 7 ca `LoadingGate`; sinh vào `state_vectors.rs`/`VinkStateVectors.kt` |

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
