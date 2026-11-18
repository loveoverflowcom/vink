# Corpus title authority — #263

[Quy tắc và hiện trạng](../../../../../docs/architecture/editor-title-authority.md). Dữ liệu kiểm thử
cho document canonical `{format, source}` (#255); không phải payload API.

- `default.json`: 12 ca định dạng `default`. Tên riêng là authority; `#`, Setext, blockquote/list
  trong thân vẫn là chữ; tên rỗng, NFC/NFD/emoji, markup literal trong tên, 200/201 ký tự
  precomposed, 200 ký tự nhập NFD (giới hạn tính sau NFC), newline bị từ chối, và một ca source có
  CRLF/dòng trống/dòng chỉ khoảng trắng/NFD được lưu nguyên văn. `expected_hash` của
  `explicit-over-atx` được dựng độc lập bằng Python `hashlib`/`struct.pack('>Q', …)` theo encoding
  `vink-document-content`. Ca có `nfd` trong `id` phải giữ input chưa NFC.
  Thực thi: `application::tests::title_authority::default_title_authority_corpus`.
- `markdown.json`: 30 ca định dạng `markdown`: không/rỗng/nhiều/xóa H1, ATX/Setext, nested,
  fence/escape/HTML, inline/entity, Unicode/emoji/CRLF và 200/201 ký tự. `expected_projection` là
  chuỗi sau trim + NFC, trước kết luận validation; `expected_validation_error` nêu lỗi, không cho
  truncate. Chuỗi rỗng không phải placeholder. Thực thi:
  `domain::document::markdown::tests::markdown_title_corpus` (parser) và
  `application::tests::title_authority::markdown_title_authority_corpus` (save → store → revision).

Editor Markdown (#256) và Reader (#264) phải chạy cùng `markdown.json` cho parser của mình
(differential) trước khi hiển thị title hoặc render Markdown.
