# Compatibility và nguồn authoring

Token source hiện hành 0.5.0, catalog 0.7.0, API admission map 0.3.0. Package nội bộ chưa phát
hành registry.

TAG-PR-05 (#99) thêm entry `composition.tag-selector` (catalog 0.7.0, 9 kịch bản giống nhau ở hai
gallery) và composition `TagSelector`/`TagChip` trong `vink-patterns` (Leptos `src/tags.rs`, CMP
`Tags.kt`; admission map 0.3.0). Chỉ ghép primitive hiện có (`FilterChip`, `SearchField`, `Button`,
`IconButton`, `SupportingDetails`; CMP thay disclosure bằng nút Text vì chưa có support API); không
thêm token, primitive hay đổi API cũ. Facts DTO-free, consumer giữ request, version và lưu.

LVE #73 (UI System) thêm role recipe `action.toggle.content`, `action.toggle.selected-container`,
`action.toggle.selected-content` (alias của `on-surface-variant`, `secondary-container`,
`on-secondary-container`; ba cặp contrast mới), primitive `ToggleAction`/`VinkToggleAction` với
vectors `toggle-contract-v1` (`fixtures/toggle.json` → `toggle_vectors.rs`/`VinkToggleVectors.kt`)
và composition `EndorsementAction` ở patterns. Minor: không đổi giá trị hay API hiện có; mọi callsite
cũ giữ nguyên. Catalog thêm `action.toggle-action` và `composition.endorsement`.

PR-01 Quiet Lavender thêm `FieldVariant::{ExternalLabel,FloatingTonal}` ở cả hai renderer;
`ExternalLabel` mặc định giữ callsite. CMP short fields mặc định đổi từ Material outlined label
sang nhãn ngoài; muốn nhãn nổi dùng `FloatingTonal`. Textarea luôn nhãn ngoài. Giữ toàn bộ
TextFieldValue/native input và API form/editor. Field disabled thiếu helper có lý do trung tính.
Chip idle tonal, focus actions/fields một perimeter; Text button chừa status slot cả khi enabled.
Catalog 0.5.3 đăng ký `external-label` trong entry hiện có, token source vẫn 0.4.0.

D2T-03 bổ sung `DiscussionThread` ở Leptos/CMP và bốn scenario trong entry
`composition.feedback`. `FeedbackComposer.comments_closed`/`commentsClosed` optional giữ
callsite cũ; sửa lỗi Sharing ẩn input. Intent không quyết định quyền bình luận. Pattern chỉ
nhận view facts, consumer giữ transport/authority; CMP chưa có service consumer/native app.

UI-REVIEW-17 F3 đổi `FeedbackIntent::label()` thành `Option<&str>` ở Leptos và
`FeedbackIntent.label` thành `String?` ở CMP: `Unspecified` không thêm nhãn intent
hoặc phần tử rỗng; nhãn textarea vẫn có. Ba intent có tên giữ nguyên nhãn. Mọi
consumer trong repository dùng renderer đã cập nhật; token, catalog và DTO không đổi.

UI-REPAIR-06/07 bổ sung slot thao tác optional cho `StatePanel`: Leptos `children`,
CMP `actions`. Typed recovery giữ thứ tự trước slot, caller vẫn sở hữu link/disabled/
effect; mọi callsite cũ giữ hành vi khi bỏ slot. Không đổi token, catalog ID hay DTO.
Web dùng slot để link SSR thật và create/reconcile không bị tách khỏi panel.

| Bề mặt giữ tương thích | Owner duy nhất | Consumer và điều kiện gỡ |
| --- | --- | --- |
| `--vk-font-ui`, `--vk-font-reading` | `font.ui`, `font.reading` trong tokens.json | Web brand/document/CodeMirror; chỉ gỡ khi mọi consumer đã đổi sang `--vk-type-*-font` và browser regression giữ typography/IME |
| `--vk-text-reading`, `--vk-leading-reading` | `text.reading`, `leading.reading` | Document/CodeMirror; typed reading role tham chiếu cùng nguồn, không có giá trị thứ hai; gỡ sau migration editor được kiểm |
| `--vk-shadow` | `shadow.raised.css` | App Tôi overlay; gỡ sau đổi consumer và mở ảnh/focus test |
| `--vk-ui-bar-height`, tooltip tọa độ | Renderer đo DOM, runtime geometry | Không là semantic token; không sinh từ JSON, không gỡ khi vẫn có measured inset |
| 15px save status và near-zero finite motion fallback | Feature CSS, exact exceptions ở api-coverage.json | App giữ EditorSession/chrome; điều kiện gỡ nằm trong từng record |

UIS-06 thay literal 13/14/16/22px và weight 600 bằng token đang có cùng giá trị; giữ page layout,
CodeMirror host, Enso assets và provenance. Không đổi expected ảnh. `vk-feed-end` không còn caller
trong source/template/dynamic selector: ListEnd do UI renderer giữ, rule cũ được gỡ.

Các snapshot thiết kế/bằng chứng UIS-01–05 vẫn là lịch sử theo SHA. UIS-06 cũng nới vùng nhãn compact rail trong padding để giữ từ tiếng Việt ở chữ 200%;
không đổi rail 96px hay font scale. Chỉ mục current và
[readiness UIS-06](../../../docs/development/ui-system-qualification-evidence.md) dẫn tới phần hiện hành;
không sửa lịch sử để gắn PASS mới. Xem [extension recipe](../EXTENDING.md) trước thay contract.

Pen Flight R2 (#73, 10/11/2026) thêm `IconName::PenFlight`/`IconName.PenFlight` từ
`icons.json`, provenance Vink-owned tách khỏi Material Apache-2.0; generator kiểm hash SVG R2
nguyên bản. EndorsementAction dùng slot icon hiện có ở hai renderer; không đổi props,
token values, toggle-contract-v1, state/counter/authority.
