# Nguồn văn bản có định dạng, phiên bản và tương thích v1 — #255

Phạm vi kiến trúc tại base `develop@d352dba77c2a86d18d7afe5690fcb9b10153c055` (17/11/2026) cho
[issue #255](https://github.com/loveoverflowcom/vink/issues/255) (`[Editor 2/7] Nguồn văn bản có định dạng và version, tương thích document v1`).

> [!IMPORTANT]
> Theo `AC-05`, chưa có exact-revision signoff của chủ sản phẩm cho schema nguồn v2 và hash v2. Do đó
> đề xuất [VDR-DOCUMENT-SOURCE-FORMAT@r1](../../decisions/proposals/VDR-DOCUMENT-SOURCE-FORMAT.r1.toml)
> giữ trạng thái `PENDING`, `SCHEMA_VERSION` sản xuất vẫn là `1`, và **không** có migration DB hay
> writer v2 nào được mở trong đợt thay đổi này.

## 1. Bốn chiều trạng thái

| Chiều | Trạng thái hiện tại | Chi tiết |
| --- | --- | --- |
| Quyết định sản phẩm | `PRODUCT_CONFIRMED` (kế thừa từ #263) + `DEFERRED` (`asciidoc`) | `Default` có tên riêng và body chữ thường (không sniff Markdown); v1 giữ nguyên ngữ nghĩa legacy; `Markdown` dùng tên từ H1 đầu tiên (#263); `AsciiDoc` hoãn (`DEFERRED`). Quy tắc bảo vệ quyền riêng tư, không autosave/offline private, giữ nguyên `#254`/`PR #277` (đã bỏ `.txt` export). |
| Quyết định kiến trúc | `PENDING` | `VDR-DOCUMENT-SOURCE-FORMAT@r1` (`CHOICE-01 A`, `CHOICE-02 A`), yêu cầu `D-02-CONTENT@r1` (`DESIGN_PROPOSED`). Không làm đổi `PENDING` của [VDR-CONTENT-STORAGE@r1](../../decisions/proposals/VDR-CONTENT-STORAGE.r1.toml). |
| Mức triển khai | `core` + adapter cô lập (`web` reader stub, PG scratch test); **chưa** bật `persistence-api` v2 | Có pure domain `services/vink-app/src/domain/source.rs` và `ContentHash::of_source_v2` / `RevisionContentV2` trong `revision.rs`; compatibility reader tại `apps/vink-web/src/document.rs`; test PostgreSQL bảng nháp `document_source_compat.rs`. Bảng sản xuất `working_drafts`/`revisions`, API HTTP và writer Bàn viết giữ v1. |
| Bằng chứng kiểm chứng | Tách riêng `unit` + `PG scratch` | Xem bảng kiểm chứng và các phần `NOT_RUN` tại [registry phát triển](../development/document-source-format.md). |

## 2. Kiểm kê producer và consumer hiện hữu (`AC-01`)

Mọi điểm chạm hiện hành với `DocumentData`, `DocumentDto`, `SCHEMA_VERSION` và `ContentHash` tại `develop@d352dba`:

| Tầng | Thành phần | Vai trò hiện tại | Tác động khi cutover v2 tương lai |
| --- | --- | --- | --- |
| Domain | `domain::document` (`SCHEMA_VERSION = 1`, `MAX_DOCUMENT_CHARS = 200_000`) | Chuẩn hóa NFC từng dòng, bỏ dòng trống, cấm `\r`/`\u{2028}`/`\u{2029}` | Giữ nguyên cho reader v1; song hành `domain::source` (`SOURCE_SCHEMA_VERSION = 2`) |
| Domain | `domain::revision` (`ContentHash::of`, `RevisionContent`) | Hash `vink-revision-content-v1` + title/blocks NFC | Giữ bất biến cho revision v1; bổ sung `of_source_v2` + `RevisionContentV2` |
| Domain / App | `DraftChange`, `Vink::save_draft`, `capture_snapshot` | Ghi `DocumentData` v1, CAS version, standing fence | Cần dual-read; chỉ ghi v2 khi bật cờ sau signoff và #262/#263 |
| Persistence (DDL) | `migrations/20261030000000_literary_core.sql` | `working_drafts.document` & `revisions.document` `CHECK` buộc `blocks` là `array` + `schema_version`; trigger `revisions_are_immutable` | Cần migration additive nới `CHECK` cho phép v1 **hoặc** v2; **không** sửa migration cũ hay viết lại hàng v1 |
| Persistence (SQL) | `drafts.rs`, `publication.rs`, `author_works.rs`, `evaluation.rs`, `moderation.rs`, `appeals.rs`, `discovery.rs`, `dev_seed.rs` | Đọc/ghi JSONB; `author_works` dùng `d.document IS DISTINCT FROM pr.document` tính `has_draft_changes`; `load_author_publication_state` (#230) và `discovery` (#95–#100) **không** đọc `revisions.document` | Mọi hàm decode `document` (`load_draft`, `load_paired_state`, `publication`, `reader_work`, `evaluation`, `moderation`, `appeals`) cần đọc qua `StoredDocument` v1+v2; `IS DISTINCT FROM` vẫn đúng với `DocumentSourceData` |
| HTTP API v1 | `http::contract`, `http::appeals`, `http::conditional::respond` | Trao đổi `DocumentDto` v1 trong `Draft`, `SaveDraftRequest`, `Publication`, `ReaderResponse`, `CaseDetailDto`, `DetailDto`; `respond` hash toàn bộ JSON response làm strong `ETag` | Giữ wire v1 đến trước Pha 2; `ETag` tự động phản ánh thay đổi biểu diễn mà không đụng `ContentHash` |
| Web Bàn viết & Reader | `apps/vink-web/src/contract.rs`, `document.rs`, `pages/desk.rs`, `pages/mod.rs`, `pages/moderation.rs`, `editor/src/editor.ts` | `text_to_document` gộp nhiều dòng trống và đổi CRLF→LF; giữ `EditorSession`, save ticket, CAS, không `.txt` export (#254/#277) | Mở reader dual-version, kết nối `#262` (format picker), `#256` (Live Preview/Source mode), `#263` (title H1), `#264` (Markdown Reader) |
| Client caches | `public_reads.rs` (`READ-02` RAM cache), `public_persistence.rs` (IDB `#203`, `ENABLED = false`) | `READ-02` giữ response JSON theo route; `public_persistence` kiểm `schema_version == 1` | Khi bật v2 trên API public, cập nhật validator của `public_persistence` nếu mở lại gate; không lưu nháp riêng tư vào IDB |

## 3. Hợp đồng nguồn văn bản v2, Capability Schema & Tương thích v1 (`AC-02`, `AC-03`)

### 3.1. Envelope v2 và Capability Schema

```json
{
  "schema_version": 2,
  "format": "default | markdown",
  "source": "<UTF-8 giữ nguyên NFD/emoji/tab/dòng trống; line break dùng \\n>",
  "line_ending": "lf | crlf"
}
```

- `FormatCapability::catalog()` định nghĩa capability schema máy đọc được:
  - `default` (`"Mặc định"`, `selectable = true`, `deferred = false`, `title_authority = separate_title`): văn bản thường/thơ có tên riêng; không bao giờ tự suy diễn ký tự `#`, `*`, `---` thành Markdown.
  - `markdown` (`"Markdown"`, `selectable = true`, `deferred = false`, `title_authority = first_h1_projection`): nguồn CommonMark tường minh nối với [#263](editor-title-authority.md), [picker #262](../design/editor-format-picker-262.md) và Reader `#264`.
  - `asciidoc` (`"AsciiDoc"`, `selectable = false`, `deferred = true`, `title_authority = deferred_document_header`): chỉ là điểm mở rộng đặt chỗ (`DEFERRED`); parser/writer và `CHECK` SQL từ chối `asciidoc` (`DeferredFormat`). Mọi format ngoài danh mục bị từ chối (`UnsupportedFormat`).

### 3.2. Bảo toàn nguyên văn vs Derived Projection

- **Không NFC-fold `source`:** `DocumentSource::parse` và `DocumentSource::from_authored` giữ nguyên byte UTF-8 của văn bản gốc (NFD vẫn là NFD, các chuỗi emoji ZWJ/`FE0F`/skin-tone, khoảng trắng đầu dòng, tab `\t`, dòng trống đầu/cuối, nhiều dòng trống liên tiếp `\n\n\n`, và dòng chỉ có khoảng trắng đều giữ nguyên).
- **Quy ước line ending:** `source` chuẩn hóa line break thành `\n` (`LF`). Nếu văn bản tác giả gửi lên dùng đồng nhất `\r\n` (`CRLF`), `line_ending` ghi `"crlf"` để `to_authored_text()` khôi phục nguyên văn byte-exact. Văn bản hỗn hợp `\r\n` + `\n` hoặc `\r` đơn lẻ chuẩn hóa về `\n` (`"lf"`), nhất quán với CommonMark 0.31.2 §2.1 và CodeMirror 6. Parser vẫn từ chối `\r` sót trong payload JSON v2, `\u{2028}`, `\u{2029}` và ký tự điều khiển ngoài `\t`/`\n`.
- **Ngân sách độ dài:** giữ ngưỡng `MAX_DOCUMENT_CHARS = 200_000` tính trên tổng ký tự non-newline sau khi áp dụng `.nfc()` từng dòng (tương thích ngân sách tác phẩm v1), kèm chặn an toàn `MAX_RAW_SOURCE_BYTES = 2_097_152` (2 MiB UTF-8) để bảo vệ JSONB/HTTP khi chứa chuỗi NFD dài hoặc nhiều dòng trống.
- **Derived projections tách riêng:** `to_nfc_search_projection()` phục vụ tìm kiếm/đối soát Unicode; `to_derived_v1_lossy()` xuất ra `Document` v1 kèm `LegacyLossSummary` đếm chính xác thông tin bị mất, không bao giờ ghi ngược vào `source`.

### 3.3. Tương thích v1 & thất thoát lịch sử đã biết

- Document v1 (`schema_version = 1`, không có khóa `format`) luôn đọc qua `StoredDocument` thành `EffectiveFormat::LegacyDefault`: **tuyệt đối không sniff Markdown**, kể cả khi dòng đầu là `# Tiêu đề`.
- Chuyển đổi một tài liệu v1 sang `markdown` hoặc format mới chỉ xảy ra khi tác giả chủ động thao tác; đọc lại bản cũ không tự nâng phiên bản và không sửa `revisions.content_hash` hay con trỏ `publications`.
- **Thất thoát đã biết của dữ liệu v1 cũ:** Schema v1 trước đây đã (1) NFC hóa từng dòng, (2) gộp nhiều dòng trống liên tiếp thành một ranh giới block, (3) loại bỏ dòng chỉ chứa khoảng trắng cùng dòng trống đầu/cuối, (4) mất phân biệt `crlf` vs `lf`, và (5) không lưu metadata định dạng. V1 cũ không thể phục hồi lại các chi tiết đã mất này, và hợp đồng ghi nhận rõ điều đó.

## 4. Phân tách biên ngữ nghĩa, Hash v2 & HTTP ETag (`AC-06`)

1. **Ngữ nghĩa nguồn vs hiển thị vs lưu trữ:**
   - *Nguồn nội dung (`#255`):* `DocumentSource` (`format`, `source`, `line_ending`, `schema_version = 2`).
   - *Chế độ hiển thị soạn thảo (`#256`):* chuyển đổi giao diện `Soạn thảo` / `Mã nguồn` chỉ là presentation state phía client, **không** thay đổi `format` hay `schema_version`.
   - *Granularity lưu trữ (`VDR-CONTENT-STORAGE@r1`, `PENDING`):* giữ nguyên quyết định về JSONB nguyên khối/TOAST, không đính kèm chunk manifest hay tối ưu nén vào hợp đồng định dạng.
2. **Canonical ContentHash v2:**
   - `ContentHash::of` (`vink-revision-content-v1`) giữ nguyên 100% cho revision v1.
   - `ContentHash::of_source_v2(title, source)` hash với domain separator `vink-revision-content-v2` nối theo thứ tự length-prefixed (`u64` big-endian) của `format`, `line_ending`, `title.as_str()` (NFC) và raw UTF-8 bytes của `source`. Nhờ đó mọi thay đổi ở format, CRLF/LF, NFD/NFC, khoảng trắng hay dòng trống đều tạo khác biệt trên hash v2 mà không chạm tới v1.
3. **Strong HTTP ETag (`http::conditional::respond`):**
   - HTTP `ETag` được tính trên đúng chuỗi JSON phản hồi cuối cùng (hoặc thay đổi theo projection/renderer khi triển khai `#264`), độc lập hoàn toàn với `revisions.content_hash` bất biến.

## 5. Phương án kiến trúc & kế hoạch rollout / rollback (`CHOICE-01`, `CHOICE-02`)

### 5.1. `CHOICE-01` — Phương án A (chọn) vs Phương án B

| Phương án | Cơ chế | Lý do chọn / loại |
| --- | --- | --- |
| **A (Đề xuất chọn)** | Một cột `document jsonb` mang discriminated envelope (`v1` hoặc `v2`) | Một nguồn chân lý duy nhất; không tạo song song hai bản ghi có thể lệch nhau; giữ nguyên `d.document IS DISTINCT FROM pr.document` trong `author_works.rs`; không tăng gấp đôi dung lượng TOAST của tài liệu 200k; phù hợp [VDR-CONTENT-STORAGE@r1](../../decisions/proposals/VDR-CONTENT-STORAGE.r1.toml); không chạm các row/trigger bất biến trong `revisions`. |
| B (Không chọn) | Thêm cột mới hoặc nhúng thêm `blocks` projection cạnh `source` | Gấp đôi dữ liệu lưu/chuyển tải trên JSONB 200k và phát sinh nguy cơ hai phiên bản nội dung lệch pha nếu có đường ghi bỏ sót đồng bộ projection. |

### 5.2. `CHOICE-02` — Phương án A: Rollout 2 pha & Rollback

- **Pha 1 — Nới CHECK constraint & mở dual-read (`v1 + v2`), writer v2 giữ TẮT:**
  - Chạy migration additive thay `CHECK` cũ bằng điều kiện chấp nhận `(schema_version = 1 AND blocks là array)` **hoặc** `(schema_version = 2 AND format IN ('default', 'markdown') AND source là string AND line_ending IN ('lf', 'crlf'))`.
  - Cập nhật `vink-app` (`serve` + `vink-worker` + `evaluation` + `moderation` + `appeals`) và `vink-web` (SSR + hydration) qua `StoredDocument` / `read_document_envelope`.
  - Không thay đổi bất kỳ row v1 hay `content_hash` nào hiện có.
- **Pha 2 — Mở writer v2 có kiểm soát (chỉ sau signoff & hội đủ consumer):**
  - Điều kiện tiên quyết: `VDR-DOCUMENT-SOURCE-FORMAT` có `SIGNOFF`; hoàn tất tích hợp [#262](../design/editor-format-picker-262.md), [#263](editor-title-authority.md), `#256`, `#264`, và kiểm tra moderation/evaluation trên `StoredDocument::V2`.
  - Khi đó `save_draft` và `capture_snapshot` mới ghi envelope `schema_version = 2` (với `ContentHash::of_source_v2` cho revision v2 mới), còn dữ liệu cũ vẫn giữ nguyên v1 nếu tác giả không chỉnh sửa/chuyển đổi.
- **Rollback an toàn:**
  - Nếu gặp sự cố sau khi mở Pha 2: tắt writer v2 để bản lưu/snapshot mới quay về v1, **giữ nguyên dual-read `v1 + v2`** ở mọi reader để các bản v2 đã lưu trong thời gian bật cờ không bị lỗi đọc. Không hạ `CHECK` constraint khi bảng đang chứa bản ghi v2.

### 5.3. Migration SQL đề xuất (chỉ chạy sau exact-revision signoff)

Lưu ý kỹ thuật phát hiện qua PostgreSQL 17: biểu thức `jsonb_typeof(document -> 'source') = 'string'`
trả về `NULL` (`UNKNOWN`) nếu key `'source'` vắng mặt, và `CHECK` của PostgreSQL cho qua `UNKNOWN`
(tương tự lỗ hổng `blocks` ở migration bootstrap `20261030000000` mà ứng dụng hiện chặn ở
`Document::parse`). Vì vậy constraint Pha 1 bọc từng nhánh bằng `IS TRUE`:

```sql
-- CHƯA ÁP DỤNG TRONG PR NÀY (AC-05). Chỉ tham chiếu cho Pha 1 sau khi có signoff VDR-DOCUMENT-SOURCE-FORMAT:
ALTER TABLE working_drafts
    DROP CONSTRAINT IF EXISTS working_drafts_document_check,
    ADD CONSTRAINT working_drafts_document_check CHECK (
        jsonb_typeof(document) = 'object'
        AND (
            (
                (document ->> 'schema_version') = '1'
                AND jsonb_typeof(document -> 'blocks') = 'array'
            ) IS TRUE
            OR
            (
                (document ->> 'schema_version') = '2'
                AND (document ->> 'format') IN ('default', 'markdown')
                AND jsonb_typeof(document -> 'source') = 'string'
                AND (document ->> 'line_ending') IN ('lf', 'crlf')
            ) IS TRUE
        )
    );

ALTER TABLE revisions
    DROP CONSTRAINT IF EXISTS revisions_document_check,
    ADD CONSTRAINT revisions_document_check CHECK (
        jsonb_typeof(document) = 'object'
        AND (
            (
                (document ->> 'schema_version') = '1'
                AND jsonb_typeof(document -> 'blocks') = 'array'
            ) IS TRUE
            OR
            (
                (document ->> 'schema_version') = '2'
                AND (document ->> 'format') IN ('default', 'markdown')
                AND jsonb_typeof(document -> 'source') = 'string'
                AND (document ->> 'line_ending') IN ('lf', 'crlf')
            ) IS TRUE
        )
    );
```

## 6. Ranh giới giữ nguyên (`AC-04`)

- Giữ nguyên `EditorSession`, save ticket, CAS (`expected_version`) và standing fence `principal::require_active`.
- Không thêm autosave ngầm (`WR-01`), không gửi nháp riêng tư tới AI provider, không mở lưu nháp riêng tư ở IndexedDB, không mở lại `.txt` export (`PR #277`).
