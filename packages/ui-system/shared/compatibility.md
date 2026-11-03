# Compatibility và nguồn authoring

Token source hiện hành 0.4.0, catalog 0.5.2. Package nội bộ chưa phát hành registry.

D2T-03 bổ sung `DiscussionThread` ở Leptos/CMP và bốn scenario trong entry
`composition.feedback`. `FeedbackComposer.comments_closed`/`commentsClosed` optional giữ
callsite cũ; sửa lỗi Sharing ẩn input. Intent không quyết định quyền bình luận. Pattern chỉ
nhận view facts, consumer giữ transport/authority; CMP chưa có service consumer/native app.

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
