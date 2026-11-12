# Publication persistence and API runtime — PUB-RUNTIME-01 (#158)

**Phạm vi:** Persistence PostgreSQL và HTTP API v1 cho author snapshots, publication intent, audience changes và privacy enforcement.
Nguồn luật: AUD-01/02, PUB-01/02/05, MOD-05, INV-01/02/07/08 trong [PRD](../../prd/requirements.md), [đặc tả](../../docs/architecture/publication-moderation.md), [DecisionDelta](../../docs/architecture/product-decisions-20261110.md).

## 1. Kiến trúc lưu trữ và khóa (Storage & Lock Graph)

### Migration `20261116000000_work_publication_persistence.sql`
- **`revisions.metadata_snapshot_id`**: FK ràng buộc bất biến tới `work_metadata_snapshots(work_id, id)`. Đảm bảo revision đã snapshot được đóng băng cùng metadata snapshot phiên bản tương ứng.
- **`work_publications`**: Bảng canonical lưu trạng thái tác phẩm (`work_id`, `audience`, `lifecycle`, `generation`, `intent_revision_id`, `intent_generation`, `pointer_revision_id`, `pointer_intent_generation`, `policy_version`, `updated_at`). Có các CHECK constraint phản ánh đúng quy tắc khôi phục của `domain::publication::Publication::restore`.
- **`work_revision_moderation`**: Bảng lưu lịch sử quyết định kiểm duyệt cho từng revision (`work_id`, `revision_id`, `decision_version`, `moderation`, `provenance_kind`, `created_at`, `updated_at`).
- **`work_evaluation_jobs`**: Lưu các ticket kiểm duyệt bền vững với partial unique index cho job `open` theo `(work_id, revision_id)`.
- **`work_publication_command_receipts`**: Lưu receipt idempotency theo `(actor_id, work_id, operation, command_key)`.

### Lock Ordering & Concurrency
Discussion và Endorsement thực hiện đọc/ghi với thứ tự khóa: `publications FOR SHARE` sau đó `principals FOR UPDATE` (locked standing).
Để loại trừ deadlock và đảm bảo tuần tự hóa chặt chẽ giữa tác giả thay đổi trạng thái tác phẩm và bạn đọc, `persistence::publication` tuân thủ nghiêm ngặt đồ thị khóa:
1. `publications WHERE work_id = $1 FOR UPDATE`
2. `work_publications WHERE work_id = $1 FOR UPDATE`
3. `works WHERE id = $1 FOR SHARE`
4. `principals WHERE id = $1 FOR UPDATE` (thông qua `locked_standing`)
5. Đọc receipt `(actor, work, operation, command_key)` — sau standing fence, như endorsement: khóa principal tuần tự hóa mọi lệnh của cùng actor, nên retry đồng thời cùng key chờ lệnh đầu rồi replay receipt đã commit (không `409` stale, không `500`). Principal bị đình chỉ nhận `403` kể cả khi gửi lại key cũ.

Thứ tự này cùng chiều với lưu nháp/thẻ (principal → work → draft/metadata) và discussion/endorsement (`publications` → principal), nên không tạo chu trình chờ. Bảng `work_publications` có dòng từ migration hoặc lần chụp snapshot đầu; work mới chưa chụp được đọc như `private`/generation 1.

## 2. API Endpoints

Tất cả các endpoint tác giả trả về header `Cache-Control: no-store`, lỗi cũng vậy. Mọi lệnh ghi đi qua kiểm `Origin` chung của API; không phải chủ tác phẩm nhận `404` như tác phẩm không tồn tại.

| Method | Path | Quyền / Auth | Mô tả |
| --- | --- | --- | --- |
| `POST` | `/api/v1/works/{work_id}/snapshots` | Actor (Author) | Chụp snapshot bất biến từ working draft, liên kết `metadata_snapshot_id`. Nhận `expected_draft_version` & `expected_metadata_version`. Idempotency qua `command_key`. |
| `POST` | `/api/v1/works/{work_id}/publication/request-public` | Actor (Author) | Yêu cầu công khai revision. Tạo job `open` trong `work_evaluation_jobs`, tăng generation, ghi intent. |
| `POST` | `/api/v1/works/{work_id}/publication/set-private` | Actor (Author) | Chuyển tác phẩm sang `private`. Xóa con trỏ trong `publications`, tăng generation, giữ nguyên quyết định kiểm duyệt đã có. |
| `GET` | `/api/v1/works/{work_id}/publication/state` | Actor (Author) | Đọc toàn bộ trạng thái publication của tác giả, danh sách revision và ticket đang mở. |

## 3. Privacy Enforcement và Conditional Reads (READ-01)

- Khi tác phẩm chuyển sang `private`, bản ghi con trỏ tương ứng trong bảng `publications` bị xóa (`DELETE FROM publications WHERE work_id = $1`).
- Đọc công khai (`GET /api/v1/works/{work_id}/publication`) trả về `404 Not Found`.
- Đọc có điều kiện với `If-None-Match` chứa ETag cũ của revision trước đó **trả về `404 Not Found`**, tuyệt đối **không trả về `304 Not Modified`**.
- Discovery feed (`GET /api/v1/works`) loại bỏ tác phẩm private khỏi kết quả.

## 4. Tương thích dữ liệu kế thừa (Legacy Bootstrap) và backout

- Con trỏ `publications` có trước migration được nâng lên generation 2 và ghép một dòng `work_publications` công khai cùng quyết định `approved` với provenance `legacy_bootstrap` (`DecisionProvenance::LegacyBootstrap`): ghi nhận trung thực rằng bản này đã công khai trước khi có kiểm duyệt, không phải AI hay quản trị viên duyệt. Chỉ migration và dev seed tạo provenance này; không lệnh runtime nào tạo được.
- Work không có con trỏ thành `private`, generation 1.
- `dev_seed` chỉ ghép trạng thái này khi tạo work lần đầu: chạy lại seed không tạo lại con trỏ mà tác giả đã thu hồi; trên schema trước #158 seed chỉ ghi con trỏ và backfill của migration ghép phần còn lại.
- Backout: [`20261116000000_work_publication_persistence.down.sql`](../../services/vink-app/migrations-rollback/20261116000000_work_publication_persistence.down.sql) chỉ dùng khi schema chưa được dùng. Có receipt, ticket hay quyết định không phải `legacy_bootstrap` thì script từ chối: giữ dữ liệu và roll forward. `publications.generation` giữ ≥ 2, schema cũ chấp nhận.
- Lỗi `409 version_conflict` của snapshot nêu đúng trường đã đổi: `expected_draft_version` (kèm `current_version` của nháp) hoặc `expected_metadata_version` (kèm version metadata hiện tại).

## 5. Verification Evidence

Lần chạy 11/11/2026 trên nhánh `codex/publication-persistence-api` (PostgreSQL 17 cục bộ):

- `cargo xtask verify quick`, `cargo xtask verify discussion`, `cargo xtask verify postgres`, `cargo xtask verify kani` — kết quả ghi trong PR #161.
- `services/vink-app/tests/postgres/publication.rs` (9 test): vòng đời snapshot, ticket, r1/r2, riêng tư/ETag `404`, khóa `publications` FOR SHARE, legacy seed, nâng cấp migration thật, backout, retry đồng thời cùng key (thất bại với adapter đọc receipt trước khóa).

[Moderation runtime](moderation-runtime.md) nối worker/người duyệt, Web trạng thái/phản đối và reader
ẩn/bị gỡ. Còn thiếu: UI chọn audience/gửi duyệt, CMP, xóa tác phẩm; replay của request-public
không trả lại `ticket_id` (đọc qua `GET …/publication/state`).
