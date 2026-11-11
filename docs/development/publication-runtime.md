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

## 2. API Endpoints

Tất cả các endpoint tác giả trả về header `Cache-Control: private, no-cache, no-store`.

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

## 4. Tương thích dữ liệu kế thừa (Legacy Bootstrap)

- Tác phẩm/revision khởi tạo trước migration được nâng lên generation 2.
- `work_revision_moderation` được gắn `DecisionProvenance::LegacyBootstrap` cho các bản ghi kế thừa, cho phép khôi phục trạng thái `Moderation::Approved` mà không vi phạm quy tắc provenance.

## 5. Verification Evidence

- `cargo xtask verify quick`: PASS (fmt, clippy server/browser/preview, type boundary doctests, unit & contract tests, repo checks).
- `cargo xtask verify discussion`: PASS (Quint models, target specifications, production Rust trace replays).
- `services/vink-app/tests/postgres/publication.rs`:
  - `test_publication_state_and_snapshot_capture_lifecycle`: PASS
  - `test_request_public_and_ticket_creation`: PASS
  - `test_r1_remains_readable_while_r2_is_submitted`: PASS
  - `test_set_private_clears_pointer_and_enforces_privacy`: PASS
  - `test_lock_ordering_concurrency_set_private_and_publications_share`: PASS
  - `test_schema_upgrade_and_legacy_compatibility`: PASS
