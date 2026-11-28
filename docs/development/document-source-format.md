# Registry triển khai — Document canonical (#255) và title theo định dạng (#263)

Kiến trúc và kế hoạch lịch sử bất biến: [document-source-format.md](../architecture/document-source-format.md).
Title authority: [editor-title-authority.md](../architecture/editor-title-authority.md).

## 1. Mức triển khai

| Thành phần | Vị trí | Mức | Ghi chú |
| --- | --- | --- | --- |
| Document `{format, source}`, `Title`, `TitledDocument` | `services/vink-app/src/domain/document.rs` | `core` | Một shape, không version; `asciidoc` = `DeferredFormat`. |
| Trích H1 Markdown (CommonMark lõi, `pulldown-cmark` không extension) | `packages/markdown-text`, kiểm corpus tại `services/vink-app/src/domain/document/markdown.rs` | `core` | Corpus `tests/fixtures/title-authority/markdown.json` 30 ca chạy thật. |
| Một content hash | `services/vink-app/src/domain/revision.rs` | `core` | `vink-document-content`; golden vector Python. |
| Save/CAS, capture snapshot, đọc public và gửi duyệt | `application/`, `persistence/{postgres,publication,discovery}.rs` | `persistence-api` | Title vật chất hóa trong cùng UPDATE với `document`. Cả hai format đi qua luồng kiểm duyệt revision. |
| Migration cutover + backout chính xác | `migrations/20261122000000_canonical_document.sql`, `migrations-rollback/…down.sql` | `persistence-api` | Fail-closed khi còn revision cũ (lịch sử bất biến). |
| HTTP API v1 | `http/contract.rs`, `contracts/http/v1/` | `persistence-api` | `POST /api/v1/drafts` nhận body JSON tùy chọn `{format}` (`default` \| `markdown`, vắng/rỗng = `default`); `title` bắt buộc cho `default`; Markdown bỏ trường hoặc gửi `null`, từ chối chuỗi client. |
| Web Bàn viết/Reader/kiểm duyệt/IDB | `apps/vink-web/src/{contract,document}.rs`, `pages/{desk,mod,moderation}.rs`, `public_persistence.rs`, `public-cache/indexeddb.js` | `web` | Cùng pipeline đọc Mặc định/Markdown trên Reader và snapshot kiểm duyệt. [#475](../design/editor-r4.md) bỏ hộp thoại xem trước bản nháp, giữ xem trước khi đổi định dạng. Bàn viết gửi buffer nguyên văn và từ chối mở nháp chứa CR vì CodeMirror đổi line ending. Validator envelope nhận cả hai format, gate IndexedDB tắt. |
| Markdown Live Preview / Outline / Picker | #256/#257/#262 (`apps/vink-web/editor/src/{editor,markdown}.ts`, `apps/vink-web/src/{editor,pages/desk}.rs`) | `web` | CommonMark 0.31.2 lõi (`@codemirror/lang-markdown`), chạy đủ 30 ca `markdown.json`. CMP: `NOT_IMPLEMENTED`. |

Đã xóa: `domain/source.rs` (`DocumentSource`, `StoredDocument`, `EffectiveFormat::LegacyDefault`,
`LineEnding`, `LegacyLossSummary`, `FormatCapability`), `ContentHash::of_source_v2`,
`RevisionContentV2`, `SCHEMA_VERSION`, `BlockData`, Web `DocumentSourceDto`/`StoredDocumentDto`/
`read_document_envelope`/`format_capabilities`/`text_to_document`/`document_to_text`,
`tests/postgres/document_source_compat.rs`, fixture `legacy-v1.json` (thay bằng `default.json`).

## 2. Pipeline đọc Web

[`ReadingDocument`](../../apps/vink-web/src/document.rs) phân nhánh bằng `format`. Mặc định
hiện chữ nguyên văn trong đoạn/khổ, giữ thụt lề và xuống dòng trong khổ. Một dải dòng trắng
phân cách hai khổ. [Renderer Markdown](../../apps/vink-web/src/document/markdown.rs) đọc AST
CommonMark 0.31.2 lõi từ cùng [parser Rust](../../packages/markdown-text/src/lib.rs) với title
authority. Heading, nhấn mạnh, danh sách, trích dẫn, mã, đường kẻ và liên kết dùng node Leptos
theo danh sách đóng. Soft break hiển thị xuống dòng, hard break dùng `br`. Cú pháp bảng, task,
gạch ngang và toán hiển thị theo cách CommonMark lõi đọc chúng. Khi cây vượt 24 tầng lồng,
văn bản hiện nguyên dạng và vẫn đặt title ở vị trí H1 đầu.

- H1 nguồn đầu tiên hiện title hiệu lực tại vị trí AST, với anchor `muc-N` theo thứ tự heading
  trong revision. H1 sau vẫn là nội dung. Mặc định và nháp chưa có H1 nhận heading ngoài nội dung.
  Nháp chưa có tên hiện “Chưa đặt tên”. Reader dùng title chụp cùng source cho HTML title và
  Open Graph. Description Markdown lấy dòng chữ đầu của đoạn văn đầu có nội dung, tối đa 160
  Unicode scalar. Tìm kiếm công khai dùng title snapshot và thẻ đi cùng pointer.
- HTML thô trong thân hiện thành chữ. Ảnh hiện alt, hoặc “Ảnh” khi thiếu alt. Href chỉ dùng cho
  đường dẫn Vink bắt đầu với một dấu `/`, fragment và HTTP/HTTPS tuyệt đối có host và không có
  credential. Nhãn của URL bị từ chối vẫn đọc được. Preview và nội dung kiểm duyệt tải sau
  hydration theo quyền phiên.
- READ-01 băm byte JSON đã chọn theo quyền hiện hành, gồm format và source. HTML đi qua SSR
  riêng. Hai validator envelope Rust/JS nhận cùng shape canonical. RAM và IndexedDB lưu source DTO.
  Gate IndexedDB tiếp tục ở `false`.
- [Corpus render](../../services/vink-app/tests/fixtures/title-authority/render.json) dùng chung giữa
  kiểm SSR, parser CodeMirror và trình duyệt. [Lane Review](scoped-workflows.md#scenario-web-và-setup)
  đi qua snapshot, gửi duyệt, duyệt, đổi pointer và chuyển riêng tư trên PostgreSQL scratch.

## 3. Bằng chứng #255 và #263

Lệnh, số test và kết quả của lần thay đổi này nằm trong mô tả PR và
Không suy PASS từ
merge, fixture hay ảnh cũ.

Sau #287, chứng cứ kết hợp ghim fixture legacy, ba mốc migration và lifecycle qua runtime canonical.
