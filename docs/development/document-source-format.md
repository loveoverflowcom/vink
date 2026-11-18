# Registry triển khai — Document canonical (#255) và title theo định dạng (#263)

Kiến trúc và kế hoạch lịch sử bất biến: [document-source-format.md](../architecture/document-source-format.md).
Title authority: [editor-title-authority.md](../architecture/editor-title-authority.md).

## 1. Mức triển khai

| Thành phần | Vị trí | Mức | Ghi chú |
| --- | --- | --- | --- |
| Document `{format, source}`, `Title`, `TitledDocument` | `services/vink-app/src/domain/document.rs` | `core` | Một shape, không version; `asciidoc` = `DeferredFormat`. |
| Trích H1 Markdown (CommonMark lõi, `pulldown-cmark` không extension) | `services/vink-app/src/domain/document/markdown.rs` | `core` | Corpus `tests/fixtures/title-authority/markdown.json` 30 ca chạy thật. |
| Một content hash | `services/vink-app/src/domain/revision.rs` | `core` | `vink-document-content`; golden vector Python. |
| Save/CAS, capture snapshot, đọc public, cổng `FormatNotPublishable` | `application/`, `persistence/{postgres,publication,discovery}.rs` | `persistence-api` | Title vật chất hóa trong cùng UPDATE với `document`. |
| Migration cutover + backout chính xác | `migrations/20261122000000_canonical_document.sql`, `migrations-rollback/…down.sql` | `persistence-api` | Fail-closed khi còn revision cũ (lịch sử bất biến). |
| HTTP API v1 | `http/contract.rs`, `contracts/http/v1/` | `persistence-api` | `title` bắt buộc cho `default`; Markdown bỏ trường hoặc gửi `null`, từ chối chuỗi client; 409 `format_not_publishable`. |
| Web Bàn viết/Reader/kiểm duyệt/IDB | `apps/vink-web/src/{contract,document}.rs`, `pages/{desk,mod,moderation}.rs`, `public_persistence.rs`, `public-cache/indexeddb.js` | `web` (Mặc định) | Gửi buffer nguyên văn; nháp Markdown → `desk-format-unavailable`. |
| Markdown editor/outline/picker/Reader | #256/#257/#262/#264 | `NOT_IMPLEMENTED` | Xem hệ quả ở tài liệu kiến trúc mục 7. CMP: `NOT_IMPLEMENTED`. |

Đã xóa: `domain/source.rs` (`DocumentSource`, `StoredDocument`, `EffectiveFormat::LegacyDefault`,
`LineEnding`, `LegacyLossSummary`, `FormatCapability`), `ContentHash::of_source_v2`,
`RevisionContentV2`, `SCHEMA_VERSION`, `BlockData`, Web `DocumentSourceDto`/`StoredDocumentDto`/
`read_document_envelope`/`format_capabilities`/`text_to_document`/`document_to_text`,
`tests/postgres/document_source_compat.rs`, fixture `legacy-v1.json` (thay bằng `default.json`).

## 2. Bằng chứng

Lệnh, số test và kết quả của lần thay đổi này nằm trong mô tả PR và
[evidence/editor-canonical-255-263](evidence/editor-canonical-255-263/README.md). Không suy PASS từ
merge, fixture hay ảnh cũ.

Sau #287, [chứng cứ kết hợp](evidence/editor-canonical-255-263/integration-287/README.md) ghim fixture legacy, ba mốc migration và lifecycle qua runtime canonical.
