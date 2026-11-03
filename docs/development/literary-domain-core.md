# Ranh giới nội dung văn học và lưu nháp — RUST-CORE-01

**Phạm vi:** refactor trên nền `develop` tại `3a024091df6eb1da962ad3fa08750954c0b83a4f`,
ngày 02/11/2026. Giữ hành vi lưu nháp/đọc công khai; không triển khai gửi duyệt, xuất bản,
moderation, auth hay schema mới. WR-01–WR-03, PUB-02 và INV-02/05/08 là các yêu cầu liên quan;
phần đã có chỉ bảo đảm lưu có version, owner và tách bản đọc, không hoàn tất các yêu cầu đó.

## HOW — dữ liệu đi qua đâu

```text
PUT /api/v1/drafts/{work_id}
  http::contract::SaveDraftRequest (i64, String, DocumentData chưa tin cậy)
  → http::api::save_draft → application::SaveDraft (giữ nguyên input Rust)
  → Vink::save_draft
      actor trước; DraftVersion → Title → Document (validate/NFC một lần)
      → domain::draft::DraftChange (private fields, không Deserialize)
  → DraftStore::save_draft(owner, work_id, &change)
  → PgStore::save_draft → UPDATE working_drafts … FROM works
      WHERE work_id khớp AND owner_id khớp AND version = expected_version
      RETURNING version, updated_at
  → SaveOutcome → SavedDraft / DraftError → HTTP v1 hiện có
```

[HTTP adapter](../../services/vink-app/src/http/api.rs) kiểm Origin cho POST/PUT, lấy actor từ
resolver phía server và trả `401` trước khi phân loại body lỗi. JSON chỉ tạo
[`DocumentData`](../../services/vink-app/src/domain/document.rs); việc deserialize thành công
không đồng nghĩa nội dung hợp lệ. [`Vink::save_draft`](../../services/vink-app/src/application/mod.rs)
giữ thứ tự lỗi cũ. [`DraftChange`](../../services/vink-app/src/domain/draft.rs) chỉ nhận
`DraftVersion`, `Title`, `Document` đã hợp lệ; không có raw constructor, setter hoặc serde bypass.
Nó **không** xác nhận ownership, không gắn nội dung với work và không phải quyền ghi.

[`PgStore`](../../services/vink-app/src/persistence/postgres.rs) vẫn kiểm owner/version trong
cùng câu UPDATE. Không pre-read rồi ghi vô điều kiện. Nếu UPDATE không khớp, SELECT sau đó chỉ
phân loại version hiện tại **cho owner**; không đọc hay ghi hộ actor khác. Version được báo khi
xung đột có thể đã tiến thêm do bên ghi khác; mỗi lần thử lại vẫn phải CAS.

Đường đọc độc lập: `PublicationStore::{list_publications, publication}` → `publications` →
`revisions` cùng work → `PublicWork`/summary. Không JOIN `working_drafts`. Revision có trigger
chặn UPDATE và con trỏ có foreign key `(work_id, revision_id)`. Lưu nháp không ghi revision,
không đổi pointer; chỉ seed local hiện tạo publication. `WorkingDraft` ≠ `RevisionContent` ≠
`PublicWork`; save ≠ submit, Approved ≠ Published.

## WHY — tái dùng thay vì dựng framework

Các luật `Document`, `Title`, `DraftVersion`, `RevisionContent`, ID riêng và SQL CAS đã có từ
bootstrap (`85cc0b2`); việc tách pool chung sau đó (`6357922`) không chuyển luật văn học vào
package storage. [Development guide](README.md#lựa-chọn-bootstrap-và-điều-kiện-xem-xét-lại)
ghi lý do giữ một crate domain/application/adapters khi chưa có consumer thứ hai.
[Decision log](../architecture/decisions.md#3-không-dùng-một-state-enum-cho-toàn-bộ-vòng-đời)
ghi đề xuất tách các trục draft/revision/publication. Những nguồn này không phải ADR Accepted.

RUST-CORE-01 chọn một refinement nhỏ: đặt tên cho **một thay đổi đã validate** trên đường
ghi thật. Đây không phải vá lỗ hổng raw JSON đi thẳng vào SQL: trước refactor application đã
validate, port đã nhận từng type hợp lệ. Grouping làm ranh giới rõ và giảm việc truyền các
mảnh của thay đổi riêng rẽ; SQL vẫn là nơi bảo đảm quyền/CAS dưới cạnh tranh.

| Owner | Trước | Sau |
| --- | --- | --- |
| HTTP / `SaveDraft` | Raw DTO và command, API v1 | Không đổi wire shape, fixtures hoặc public input Rust |
| Domain | Ba giá trị riêng đã validate | Thêm `DraftChange` chứa chính ba giá trị, không luật hay validate trùng |
| Application | Actor → version → title → document → ba tham số | Cùng thứ tự → một `DraftChange` dùng ngay |
| `DraftStore` / PostgreSQL | `expected`, `&Title`, `&Document` | `&DraftChange`; cùng SQL, bind values, outcomes và lỗi |
| Revision / public read | Snapshot/hash và publication pointer riêng | Không đổi production behavior |

Không thêm crate/service, generic ID framework, trait suy đoán, `Arc` hay dynamic dispatch.
Ba ID vẫn là nominal newtype; raw text sai UUID bị từ chối, `from_uuid` nhận UUID đã parse từ
SQL/fixture. Không tự thêm policy nil/v7: dữ liệu cũ có UUID v4, work mới dùng v7. Không thêm
parser stored hash vì chưa có consumer đọc `content_hash` vào `ContentHash`.

## Bất biến và failure modes

| Bất biến / lỗi | Owner và kết quả không đổi | Bằng chứng kiểm |
| --- | --- | --- |
| Nháp rỗng/chưa đặt tên hợp lệ; revision cần title | `Document::empty`, `Title::parse`, `RevisionContent::new` | Unit + PostgreSQL empty round-trip |
| NFC/NFD tiếng Việt, Đ/đ, đoạn/khổ/dòng/thụt lề/tab | Domain normalize NFC, không case-fold hoặc flatten; whitespace trong dòng được giữ | Literal expected documents + PG round-trip |
| Invalid version/title/document không vào store | Application boundary; `422 validation_failed`, field giữ nguyên | RecordingStore + HTTP/PG không đổi dữ liệu |
| Malformed shape/unknown fields; unknown schema; quá dài | Deserialize raw shape hoặc domain reject; không thêm hạn chế mới | Unit + HTTP/PG `body` khác `document` |
| Anonymous / foreign actor | `401` / `404` không tiết lộ draft | Application precedence + PG read/list/write |
| Stale/concurrent save | `409`, không ghi; đúng một save của một version thắng | PG stale + 8 writer đồng thời, so title/document/version bền vững |
| Storage unavailable / stored data invalid | `503` / `500`, không giả thành công | Application outcome tests, HTTP outage/contract tests |
| Draft/snapshot chưa được trỏ không công khai | Chỉ pointer chọn snapshot | PG public API/list/SSR privacy + immutable/pointer constraints |

Hash v1 giữ nguyên SHA-256 với domain separator `vink-revision-content-v1`, độ dài UTF-8 của
title và mỗi dòng bằng u64 big-endian, số block rồi số dòng từng block cũng bằng u64 big-endian.
Hash dùng title/document đã NFC, không phụ thuộc JSON formatting. Thay line boundary, stanza,
case hay indentation phải đổi hash. Không đổi schema/format/hash persisted. Golden vector cố
định cho title `Việt Đ/đ`, block `["Đêm xuống,", "  đợi trăng"]`, rồi `["đường xa."]` là
`88bbbf2e34c3174e82d6a49388dddb70882a216e7ec3ba6b76eea165a49c36dd`; expected được tính
độc lập từ literal canonical bytes, không từ helper production trong test.

## Kiểm chứng và giới hạn

[Feature map](feature-map.md) dẫn tới các assertions; [development guide](README.md) sở hữu
lệnh/lane. `verify quick` chạy thêm doctests compile-fail của literary core: không thể thay
raw input cho validated type, deserialize vào type đó, sửa private field hoặc trộn loại ID.
RecordingStore chỉ chứng minh application gọi port đúng; không thay bằng chứng PostgreSQL.

Không đổi state/guard sản phẩm nên không tạo model Quint. Không sửa frontend/editor hoặc
chạy browser để tuyên bố IME/thị giác. Chưa có auth authority, submit/moderation/publish thật;
seed publication và test resolver không chứng minh các quy trình đó. Kết quả lần chạy và
giới hạn môi trường nằm trong [bản ghi kiểm chứng](literary-domain-verification.md).
