# Một document canonical: định dạng và nguồn nguyên văn — #255

Owner: [issue #255](https://github.com/loveoverflowcom/vink/issues/255) (hợp đồng document, format,
hash, migration). Title authority thuộc [#263](editor-title-authority.md). Base thay đổi:
`develop@607ef83`, 17/11/2026.

> [!IMPORTANT]
> **Quyết định sản phẩm 17/11/2026 (chủ sản phẩm, trong phiên giao việc):** “Replace hoàn toàn.
> Không cần giữ v1, không cần đặt v2, không cần duy trì nhiều phiên bản document.” Quyết định này
> thay thế hướng dual-read/dual-write và rollout v1→v2 hai pha của
> `VDR-DOCUMENT-SOURCE-FORMAT@r1`. Kiến trúc tương ứng là
> [`VDR-DOCUMENT-SOURCE-FORMAT@r2`](../../decisions/proposals/VDR-DOCUMENT-SOURCE-FORMAT.r2.toml),
> **`ACCEPTED`** theo [signoff r2](../../decisions/signoffs/VDR-DOCUMENT-SOURCE-FORMAT.r2.toml). Chủ sản phẩm duyệt cơ chế dừng an toàn khi còn revision cũ.
> Mọi thay đổi lịch sử hoặc migration trên dữ liệu thật cần ủy quyền riêng (mục 6).

## 1. Bốn chiều trạng thái

| Chiều | Trạng thái | Ghi chú |
| --- | --- | --- |
| Quyết định sản phẩm | `PRODUCT_CONFIRMED` (17/11/2026: thay hoàn toàn, một document; 16/11/2026: Mặc định có tên riêng, Markdown lấy H1 đầu tiên) · `DEFERRED` (AsciiDoc) | Lời chốt trích ở đầu tài liệu và ở [#263](editor-title-authority.md). |
| Quyết định kiến trúc | `ACCEPTED` | `VDR-DOCUMENT-SOURCE-FORMAT@r2` thay r1; `VDR-CONTENT-STORAGE@r1` (JSONB nguyên khối) giữ `PENDING`, không đổi. |
| Mức triển khai | `persistence-api` + `web` (Mặc định) | Domain, application, PostgreSQL, HTTP API v1, Web Bàn viết/Reader dùng một shape. Markdown: `persistence-api` (nháp + snapshot riêng tư), **không** công khai và **không** mở trên Bàn viết. |
| Kiểm chứng | Xem [registry](../development/document-source-format.md) | `unit`, `postgres` (scratch), `web`; mục `NOT_RUN` ghi riêng. |

## 2. Hợp đồng canonical

```json
{ "format": "default" | "markdown", "source": "<chuỗi UTF-8 đúng như tác giả viết>" }
```

- **Một shape, không version.** Không có `schema_version`, `blocks`, `line_ending`, envelope phân
  nhánh, reader legacy hay dual-write. Wire (HTTP API v1, Web) và lưu trữ (`working_drafts.document`,
  `revisions.document`) dùng đúng hai khóa này; khóa lạ hoặc thiếu bị từ chối ở serde
  (`deny_unknown_fields`) và ở `CHECK vink_document_is_canonical(document)`.
  Route `/api/v1` và số thứ tự migration không liên quan và không đổi.
- **`source` nguyên văn:** giữ NFD/NFC đúng như nhập, emoji ZWJ/FE0F/skin tone, tab, thụt lề,
  dòng trống liên tiếp, dòng chỉ khoảng trắng và line ending (`\n`, `\r\n`, `\r`). Không NFC-fold,
  không gộp dòng trống, không đổi CRLF khi lưu. Bị từ chối tường minh (không âm thầm sửa): ký tự
  điều khiển ngoài tab/`\n`/`\r`, U+2028/U+2029, quá `MAX_DOCUMENT_CHARS = 200 000` ký tự (đếm sau
  NFC, không tính `\n`/`\r`), quá `MAX_SOURCE_BYTES = 2 097 152` byte.
- **Format tường minh:** `default` | `markdown`. `asciidoc` nhận lỗi riêng `DeferredFormat`; tên
  khác nhận `UnsupportedFormat`. Không bao giờ suy Markdown từ dấu `#` hay nội dung.
- **Projection dẫn xuất** (không bao giờ ghi đè `source`): title Markdown (H1 → `Title::parse`,
  NFC), search/sort key từ cột `title`, đoạn hiển thị của Reader (`document::paragraphs`: dải dòng
  trống phân đoạn, nhận cả LF, CRLF và CR khi hiển thị).
- **Owner:** `services/vink-app/src/domain/document.rs` (`DocumentFormat`, `DocumentData`,
  `Document`, `Title`, `TitledDocument`), `domain/document/markdown.rs` (trích H1),
  `domain/revision.rs` (`ContentHash::of`, `RevisionContent`). Web: `apps/vink-web/src/contract.rs`
  (`DocumentDto`, `DocumentFormatDto`) và `document.rs` (`default_document`, `paragraphs`,
  `DocumentView`).

## 3. Title authority (tóm tắt, chi tiết ở [#263](editor-title-authority.md))

`TitledDocument` là constructor duy nhất ghép title với document:

- `author(document, title)` (biên tác giả): Mặc định **bắt buộc** trường `title` (được rỗng ở nháp);
  Markdown **từ chối** mọi chuỗi `title` client (`TitleNotWritable`) và lấy H1 đầu tiên trong AST.
  HTTP bỏ trường `title` hoặc gửi `null` đều ánh xạ thành `None`, không phải một tên writable.
- `from_stored(document, title)` (biên lưu trữ): Mặc định dùng cột `title`; Markdown tính lại H1 và
  bắt buộc bằng cột đã lưu, lệch là lỗi integrity — không đổi tên thầm.
- Cột `title` là projection vật chất hóa **trong cùng UPDATE/CAS** với `document` (#263
  `CHOICE-02 A`); `desk_title_key` và `title_search_key` do PostgreSQL sinh từ cột đó.

## 4. Một content hash

`ContentHash::of(&TitledDocument)` = SHA-256 của `b"vink-document-content"` rồi từng trường
length-prefixed (`u64` big-endian): `format`, title hiệu lực (sau trim/NFC), byte nguyên văn của
`source`. Golden vector độc lập (Python) nằm trong `domain/revision.rs`; SQL fixture tính cùng
encoding. Hash là digest nội dung của snapshot, **không** phải HTTP `ETag`: `ETag` (READ-01) vẫn là
hash thân response, tự đổi khi shape JSON đổi. Không còn `of_source_v2`/hash v1 song song.

## 5. Kiểm kê consumer sau thay thế

| Tầng | Thành phần | Sau #255 |
| --- | --- | --- |
| Domain | `document.rs`, `document/markdown.rs`, `revision.rs`, `draft.rs` (`DraftChange`/`WorkingDraft` mang `TitledDocument`) | Một model; `source.rs`, `RevisionContentV2`, `StoredDocument`, `EffectiveFormat::LegacyDefault`, `LegacyLossSummary`, `FormatCapability` đã xóa. |
| Application | `save_draft` (document trước, rồi title authority), `PublicWork.content`, `request_public` | Cổng `FormatNotPublishable` (HTTP 409 `format_not_publishable`) cho revision không phải `default` đến khi Reader #264 an toàn. |
| PostgreSQL | `postgres.rs::content`, `publication.rs` (capture, revision content), `discovery.rs` (đọc + kiểm hash), `moderation.rs`/`appeals.rs` (trả `DocumentData` của snapshot), `author_works.rs` (`d.document IS DISTINCT FROM pr.document`) | Đọc/ghi một shape; capture lấy title + source từ cùng phiên bản nháp đã khóa `FOR SHARE`. |
| HTTP API v1 | `Draft`, `SaveDraftRequest` (`title` tùy chọn), `Publication`, `CaseDetailDto`, appeals | Fixture `contracts/http/v1/fixtures/*` dùng `{format, source}` với dòng trống và newline cuối để chứng minh nguyên văn. |
| Web | `contract.rs`, `document.rs`, `pages/desk.rs` (gửi buffer CodeMirror nguyên văn; bản Markdown dừng ở thông báo `desk-format-unavailable`), `pages/mod.rs`, `pages/moderation.rs`, `public_persistence.rs`, `public-cache/indexeddb.js` | Không còn `text_to_document`/`document_to_text`; IDB chỉ nhận `format: "default"`. |
| Fixture/seed | `dev_seed.rs`, `xtask/src/provision.rs`, `apps/vink-web/e2e/fixtures/*.sql`, script e2e (`e2e/document.mjs`) | Canonical shape và hash mới. |

## 6. Cutover cơ sở dữ liệu và lịch sử bất biến

Migration forward-only [`20261122000000_canonical_document.sql`](../../services/vink-app/migrations/20261122000000_canonical_document.sql)
(không sửa migration cũ), chạy **offline**: dừng mọi App/worker trước; binary cũ không đọc được shape
mới và binary này không đọc shape cũ.

1. **Chặn khi có revision ở shape cũ.** Có ≥1 revision chưa có khóa `format` thì RAISE
   (`object_not_in_prerequisite_state`, “cutover blocked”) trước mọi thay đổi; transaction rollback,
   `_sqlx_migrations` không ghi gì.
2. Kiểm shape nháp cũ đúng `{schema_version: 1, blocks: [{lines: [text…]}…]}`; lạ thì RAISE. Dừng trước khi ghi nếu block rỗng, line không phải đầu ra hợp lệ của writer cũ, tổng scalar vượt trần hoặc title cũ không đọc lại được chính xác.
3. Đổi nháp tất định: dòng nối `\n`, block nối một dòng trống, `format = default`, giữ `title`,
   `version`, `updated_at`. Chỉ khôi phục được chữ còn lưu: dòng trống/CRLF/NFD mà writer cũ đã bỏ
   thì không phục hồi.
4. Thay `CHECK` của `working_drafts.document` và `revisions.document` bằng
   `vink_document_is_canonical` (bọc `coalesce(…, false)` để `NULL` không lọt), kiểm số dòng nháp
   trước/sau và nháp mồ côi. Chạy lại là no-op.

Backout [`migrations-rollback/20261122000000_canonical_document.down.sql`](../../services/vink-app/migrations-rollback/20261122000000_canonical_document.down.sql)
chỉ chạy khi chính xác: từ chối nếu có revision, nháp Markdown, hoặc nháp có dòng trống thừa,
dòng chỉ khoảng trắng, CR, NFD hay dòng trống đầu/cuối. Nếu một hàng được ghi ngoài writer vượt giới hạn scalar, chứa control bị cấm hoặc có title không hợp lệ, backout dừng nguyên giao dịch.

### Lịch sử bất biến

Đổi một revision cũ sang shape mới là **viết lại lịch sử**: `revisions` có trigger
`revisions_are_immutable`; `content_hash` cũ băm `{title, blocks}` theo `vink-revision-content-v1`
và **khác** hash canonical của cùng chữ (test `cutover_refuses_while_immutable_revisions_exist`
chứng minh cả hai); digest này còn được sao vào `work_evaluation_jobs.content_digest`,
`moderation_cases.content_digest` và `work_appeals.content_digest` (bảng appeals ghi rõ “History is
never rewritten”). Vì vậy PR này **không** tắt trigger, không xóa, không đổi pointer và không giả
hash bằng nhau. Môi trường có revision (mọi môi trường đã từng có publication, kể cả dev seed cũ)
không thể nhận migration cho đến khi một phương án dưới đây được **ủy quyền riêng**:

| Phương án | Nội dung | Đánh đổi |
| --- | --- | --- |
| P1 — Rehash có kiểm toán (đề xuất) | Maintenance window; một migration riêng có tham số ủy quyền ghi bảng `revision_content_rehash_audit` (revision, document cũ, hash cũ, hash mới, thời điểm, người ủy quyền); thay tạm trigger bằng trigger chỉ cho đổi đúng `document`/`content_hash` khi khớp mã ủy quyền phiên; đổi revision theo bước 3; cập nhật ba cột digest phụ thuộc trong cùng transaction; khôi phục trigger. | Identity (id, pointer) giữ nguyên; digest đổi có dấu vết. Cần chủ sản phẩm duyệt việc đổi “History is never rewritten” của appeals. |
| P2 — Reset môi trường không có dữ liệu thật | Xóa và seed lại DB scratch/dev. | Chỉ hợp lệ khi xác nhận không có nội dung người dùng; không áp dụng production. |
| P3 — Lưu trữ revision cũ ngoài runtime | Chuyển revision cũ sang archive chỉ đọc và tạo revision mới. | Đổi revision identity/pointer/appeal; không khuyến nghị. |

Signoff r2 xác nhận cơ chế dừng khi có revision cũ. Việc merge code vẫn phụ thuộc kết quả tích hợp và kiểm chứng. Các môi trường có lịch sử tiếp tục bị chặn cutover. Kiểm chứng sử dụng dữ liệu tổng hợp trong PostgreSQL scratch.

## 7. Hệ quả cho các issue phụ thuộc

- **#256 (Markdown editor):** dùng dialect CommonMark 0.31.2 lõi không extension như server; chạy
  `tests/fixtures/title-authority/markdown.json` cho parser TS (differential) trước khi hiện title.
  Gửi `{format: "markdown", source}` không kèm `title`.
- **#257 (outline):** H1 đầu tiên là title đúng như server; outline chỉ đọc, không ghi title.
- **#262 (picker/chuyển đổi):** tạo work mới hiện vẫn là `default`; chuyển format là đổi `format`
  trong cùng save/CAS, có preview/Hủy, không suy từ nội dung. Bàn viết hiện dừng tường minh với
  nháp Markdown.
- **#264 (Reader):** gỡ cổng `FormatNotPublishable` chỉ khi renderer đóng (allowlist, không thực thi
  HTML) dùng cùng dialect/corpus; cập nhật `public_persistence`/IDB nhận `markdown`.

## Lịch sử

`VDR-DOCUMENT-SOURCE-FORMAT@r1` và tài liệu dual-version của
[PR #279](https://github.com/loveoverflowcom/vink/pull/279) tại
[`92b2eb3`](https://github.com/loveoverflowcom/vink/blob/92b2eb33dc4ad55bfb1bed15cfb7f614e08df56c/docs/architecture/document-source-format.md)
là bằng chứng lịch sử; không còn định nghĩa kiến trúc đích.
