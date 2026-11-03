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
| `WorkCard`, `WorkByline`, `ReaderHeader` | Tiêu đề, tác giả được cấp, metadata UTC, đích đọc do host cấp |
| `DraftResume`, `DraftRow` (Web), `DraftMetadata` | Tiêu đề/version/thời điểm từ card; không tải manuscript |
| `EditorToolbar` | Slot trạng thái và hành động; không sở hữu body hay save state |
| `FeedbackComposer` | Giá trị controlled và intent; Chỉ chia sẻ không có form phê bình |
| `FeedbackItem`/`FeedbackThread`, `ParticipantSummary` | Chỉ người được trả về, khử trùng theo key theo thứ tự đầu; không suy tổng số |
| `RecentFeedback` | Mẫu ghi rõ, không gọi là nhận xét mới nhất khi chưa có timestamp có thẩm quyền |

`Author.key = None` khi API chưa cấp author ID. Không dùng display name hoặc work ID thay identity;
initials vẫn từ tên thật được cấp, tone trung tính. Không suy profile URL, ảnh, intent, excerpt,
feedback/count hay publication từ draft. `href` được renderer kiểm allow-list.

Leptos nhận `Signal`/callback; CMP nhận `TextFieldValue`/callback để giữ selection/composition.
Consumer giữ request identity, actor/work/revision/session và cách bỏ phản hồi muộn. Hiện chưa có
feedback transport: mẫu chỉ ghi intent, không có acknowledgment để giả thành công. Khi #14 B/C có
owner/contract thật, adapter mới được nối ở app; không thêm provider vào package này.

Các recipe `screen.explore`, `screen.reader`, `screen.desk`, `screen.editor`, `screen.me`,
`composition.feedback` dùng cùng ID/kịch bản trong catalog và `fixtures/foundation.json` cho
văn bản. Mọi người/nội dung mẫu được ghi nhãn. Native editor chỉ là chrome + controlled text
specimen, không phải CodeMirror hoặc sản phẩm native có backend.

CSS composition ở [leptos/patterns.css](leptos/patterns.css), được build phục vụ cả app và preview;
CSS app chỉ giữ bố cục màn, host CodeMirror và hành vi sticky. Lệnh do
[development guide](../../docs/development/README.md) sở hữu; kết quả và giới hạn ở
[bằng chứng UIS-05](../../docs/development/ui-system-adoption-evidence.md).
