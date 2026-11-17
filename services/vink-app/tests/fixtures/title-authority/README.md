# Corpus title authority — #263

[Quy tắc, trạng thái và điểm nối](../../../../../docs/architecture/editor-title-authority.md).
Đây là dữ liệu kiểm thử, **không phải document envelope, API payload hay parser mới**.

- `legacy-v1.json`: 11 ca tên riêng trên document v1, gồm ATX/Setext/nested body,
  tên rỗng, NFC/NFD/emoji, tên có markup literal, 200/201 ký tự precomposed, 200 ký tự
  nhập NFD (400 scalar trước NFC, hợp lệ vì giới hạn tính sau NFC) và newline bị từ chối.
  `application::tests::title_authority::legacy_v1_title_authority_corpus` thực thi
  các ca qua `Vink::save_draft`, kiểm một validated change và `RevisionContent::new`.
  Test dựng document bằng hằng `LEGACY_V1_SCHEMA_VERSION = 1` riêng, không theo
  `SCHEMA_VERSION` production; ca có `nfd` trong `id` phải giữ input chưa NFC.
  Recording store chỉ chứng minh application wiring, **không chứng minh PostgreSQL CAS**.
  `expected_v1_hash` là SHA-256 byte vector literal được dựng độc lập bằng Python
  `hashlib`/`struct.pack('>Q', …)`, với title `Tên riêng`, một block hai dòng
  `# Tên trong thân` / `Đoạn sau.`; không lấy expected từ Rust implementation.
- `markdown-proposed.json`: 30 ca **DESIGN_PROPOSED** cho parser tương lai, gồm
  no/empty/multiple/deleted H1, ATX/Setext, nested, fence/escape/HTML, inline/entity,
  Unicode/emoji/CRLF và 200/201 ký tự. `source` là input phải giữ nguyên.
  Thứ tự đề xuất: extraction/flatten từ source → chuẩn hóa như `Title::parse`
  (trim, NFC) → validation (ký tự cấm, >200 scalar). `expected_projection` là chuỗi
  sau chuẩn hóa, **trước kết luận validation**: `nfd-emoji` có source NFD còn projection
  NFC; `title-201` giữ đủ 201 ký tự kèm `expected_validation_error`. Chuỗi rỗng không phải
  placeholder. `expected_validation_error` nêu lỗi dự kiến, không cho truncate.
  Corpus chưa có Rust/TS parser consumer: extraction, differential và source roundtrip
  **NOT_RUN**. JSON parse thành công chỉ xác nhận artifact đọc được.

Các file cố ý tách legacy executable khỏi Markdown proposal, tránh gắn PASS validation
của v1 cho parser chưa có. Khi #255 sẵn sàng, các owner dùng cùng fixture thay vì tự tạo
oracle TS/Rust riêng. Grammar/flatten chi tiết còn phải reconcile ở tài liệu owner.
