# Composition Vink — UIS-05

Package theo feature cho hai consumer thật: ứng dụng Web và gallery; CMP có thư viện
`patterns` common và consumer mẫu trong preview desktop. Primitive do
[UI system](../ui-system/README.md) sở hữu. Package này không có API client, DTO nghiệp vụ,
route lookup, session, persistence hoặc fixture production.

```text
provider DTO → apps/vink-web/src/ui/adapters.rs → immutable view facts
  → vink-patterns (Leptos) → vink-ui → semantic tokens
fixture adapter → cùng API vink-patterns (gallery Web)
fixture adapter → :patterns (CMP common) → :ui → semantic tokens
```

| Composition | Trách nhiệm |
| --- | --- |
| `WorkCard`, `WorkByline`, `ReaderHeader` | Tiêu đề, tác giả được cấp, thời gian tuyệt đối (Web: device zone sau hydrate, fallback UTC), đích đọc do host cấp |
| `AuthorWorkRow` (Web) | Status/counters/title/scope và href do host cấp; không DTO/fetch/mutation; recipe `screen.desk` có specimen mẫu (#214) |
| `DraftResume`, `DraftRow` (Web), `DraftMetadata` | Tiêu đề/thời điểm từ card; row Web bỏ version nhìn thấy (#213), DraftMetadata/resume specimen giữ version; không tải manuscript |
| `EditorToolbar` | Slot `#save-slot` hợp nhất trạng thái sống duy nhất, optional `save_action` (`ChildrenFn`) / `status_problem` + chi tiết máy chủ và secondary actions; không sở hữu body hay save state ([#217](../../docs/design/ui-editor-unified-save-217.md)) |
| `FeedbackComposer` | Giá trị controlled; intent độc lập với comments mode; Closed giữ input và chặn gửi, Pending vẫn đúng trạng thái transport |
| `FeedbackItem`/`FeedbackThread`, `ParticipantSummary` | Chỉ người được trả về, khử trùng theo key theo thứ tự đầu; không suy tổng số |
| `DiscussionThread` | Root/neo + một danh sách replies phẳng; status/context được cấp; phát reply/remove intents, không quyết định authority. Slot Web tùy chọn `CommentEndorsements` (facts theo ID, `None` = không có đích) render `EndorsementAction` ở cuối hàng thao tác của từng gốc/phản hồi, không đổi thứ tự; CMP chưa có slot này |
| `EndorsementAction` | Ghi nhận văn học (#73) cho một đích: `EndorsementKind::{Work, Comment}` chọn copy “Thấm”/“Hữu ích”, `EndorsementBlock` thành lý do bằng chữ; nhận `EndorsementFacts` (giá trị đã xác nhận, phase, số đếm của riêng đích) và phát trạng thái mong muốn qua `ToggleAction`. Không giữ `command_key`/order/session, không cộng số lên gốc/tác phẩm, glyph Pen Flight R2 A · Mềm (CHOICE-07) từ registry, trang trí 20px, giữ nguyên mọi phase và slot cuối riêng cho tiến trình/✓ |
| `TagSelector`, `TagChip` | Thẻ tác phẩm (#99): nhóm “Dạng bài” (chip chọn một) và “Chủ đề” (chip có nút bỏ tên riêng, ô tìm giữ raw query qua composition, status polite, gợi ý có alias/mô tả, disclosure các chủ đề có sẵn). Thẻ ngừng dùng/bị từ chối ghi bằng chữ và “Thay bằng …” khi có kế nhiệm. Nhận `TagFacts`/`TagOptions`/`CatalogFacts` và phát intent; không fetch, không version, không lưu, không giới hạn thay server. Consumer: Bàn viết (`apps/vink-web/src/pages/work_info.rs`); #100 dùng lại cho bộ lọc |
| `RecentFeedback` | Mẫu ghi rõ, không gọi là nhận xét mới nhất khi chưa có timestamp có thẩm quyền |

`Author.key = None` khi API chưa cấp author ID. Không dùng display name hoặc work ID thay identity;
initials vẫn từ tên thật được cấp, tone trung tính. Không suy profile URL, ảnh, intent, excerpt,
feedback/count hay publication từ draft. `href` được renderer kiểm allow-list.

Leptos nhận `Signal`/callback; CMP nhận `TextFieldValue`/callback để giữ selection/composition.
Consumer giữ request identity, actor/work/revision/session và cách bỏ phản hồi muộn. Web reader
có adapter thật tại `apps/vink-web/src/pages/discussion.rs` và controller `discussion_state.rs`;
production auth chưa có, chỉ LocalFixture có marker host để kiểm ghi cục bộ. Gallery và CMP vẫn
là mẫu, không có acknowledgment để giả thành công. Không thêm provider vào package này.
`FeedbackIntent::Unspecified` không render nhãn intent riêng; nhãn textarea vẫn gọi tên input.
Các intent có tên giữ nhãn ở cả Leptos và CMP. `label()` trả `Option<&str>` ở Rust,
`label` là `String?` ở Kotlin; consumer bỏ phần tử nhãn khi không có giá trị.
`composition.endorsement` dùng cùng 7 scenario ở hai gallery (tác phẩm, gốc, hai phản hồi phẳng;
mỗi đích một action). Reader Web có adapter thật (`apps/vink-web/src/pages/endorsement.rs` + reducer
`endorsement_state.rs` theo session/đích/order, xử lý `409`/replay); production vẫn `401`, chỉ
LocalFixture ghi cục bộ ([runtime](../../docs/development/literary-endorsement-runtime.md#web-adapter-wp-04)).
`composition.feedback` dùng cùng scenario IDs `sharing-open`, `sharing-closed`, `discussion-flat`,
`discussion-anchor` ở hai gallery; đây là projection mẫu, không phải proof Rust/Quint hoặc native integration.

Các recipe `screen.explore`, `screen.reader`, `screen.desk`, `screen.editor`, `screen.me`,
`composition.feedback` dùng cùng ID/kịch bản trong catalog và `fixtures/foundation.json` cho
văn bản. Mọi người/nội dung mẫu được ghi nhãn. Native editor chỉ là chrome + controlled text
specimen, không phải CodeMirror hoặc sản phẩm native có backend.

CSS composition ở [leptos/patterns.css](leptos/patterns.css), được build phục vụ cả app và preview;
CSS app chỉ giữ bố cục màn, host CodeMirror và hành vi sticky. Lệnh do
[development guide](../../docs/development/README.md) sở hữu; kết quả và giới hạn ở
[bằng chứng UIS-05](../../docs/development/ui-system-adoption-evidence.md).

Delta #213: Desk Web và recipe `screen.desk` không mount `DraftResume`; pattern và
`DraftMetadata` mặc định còn được giữ tương thích, không còn là bố cục Desk hiện hành.
Recipe CMP chứa khối viết tiếp được ghi rõ là mẫu cũ, không đại diện Bàn viết native (chưa có).
Row Web dùng thời gian local sau hydrate, fallback UTC ở SSR và không version nhìn thấy.
