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

## Mốc công khai đầu tiên (#291)

Publication triển khai phần PostgreSQL của `SOC-FEED-01` theo CHOICE-04 trong
[contract r1 của #288](https://github.com/loveoverflowcom/vink/blob/cdbdde896218251cf74b25fe38c585cddbd9a9de/docs/architecture/social-graph.md).
[Migration 20261123000000](../../services/vink-app/migrations/20261123000000_first_publication.sql)
giữ một hàng trong `work_first_publications` theo `work_id`. PostgreSQL lấy
`clock_timestamp()` sau khi chèn được con trỏ vào `publications`, kể cả khi lệnh vừa
chờ một giao dịch khác. Con trỏ và mốc có hiệu lực cùng lúc khi commit. Rút bài xóa
con trỏ. Thay revision, duyệt lại và đăng lại giữ mốc đã ghi. Seed và các writer cũ
cũng đi qua trigger này.

| Provenance | Chứng cứ và nguồn (`source`) | Dữ liệu thời gian |
| --- | --- | --- |
| `never_public` | Work mới (`new_work`) hoặc work tại lúc nâng cấp chưa có revision và chưa có dấu vết công khai (`no_revision`). | Hai cột thời gian đều NULL. Lần đặt con trỏ đầu chuyển hàng thành `exact`. |
| `exact` | Trigger quan sát lần chèn đầu của work biết chắc chưa từng công khai (`pointer_insert`). | `first_published_at` là mốc đầu xác thực. |
| `legacy` | Còn con trỏ tại lúc nâng cấp (`existing_pointer`). | `legacy_published_at` giữ nguyên `publications.published_at` lúc đó. `first_published_at` là NULL. |
| `unknown` | Đã rút, còn dấu vết từng public nhưng mất mốc (`withdrawn_without_time`), hoặc còn revision mà lịch sử đặt con trỏ chưa đủ (`incomplete_history`). | Hai cột thời gian đều NULL. |

Schema cũ cho phép ghi đè và xóa con trỏ, nên backfill trên schema đó có 0 mốc
`exact`. Migration chỉ đọc khóa revision và trạng thái publication. Lần chạy lại
giữ nguyên kết quả phân loại theo WorkId. Muốn đối soát trên từng database, chạy
truy vấn sau trước khi mở consumer và đối chiếu với tổng số work.

```sql
SELECT provenance, source, count(*) AS works
  FROM work_first_publications
 GROUP BY provenance, source
 ORDER BY provenance, source;
```

**Đầu vào của #296.** View `publications_with_exact_first_time` trả `work_id`,
`revision_id`, `metadata_snapshot_id`, `first_published_at` và `provenance` cho
các mốc `exact` còn con trỏ công khai. Revision và metadata thuộc con trỏ hiện
hành. Discovery kiểm membership, tác giả hợp lệ và quyền đọc trong cùng snapshot
trước khi sắp theo `first_published_at DESC` với khóa phụ WorkId và phân trang.
Khám phá hiện hành tiếp tục dùng `publications.published_at`.

**Triển khai và quay lui.** Migration lấy khóa ghi trên `publications` và `works`
bằng `NOWAIT`. Nếu có writer đang giữ khóa xung đột, để writer hoàn tất rồi chạy
lại migration. Khi quay lui consumer, tắt đường đọc Following trước, giữ bảng, trigger,
view và ledger. Writer dùng câu SQL cũ vẫn bảo toàn mốc. Readiness của binary có
ledger trước #291 báo `diverged` trên schema mới. Dùng bản tương thích có nhúng
migration #291 khi cần phục vụ lại. [Fixture PostgreSQL về backfill, race và rollback](../../services/vink-app/tests/postgres/first_publication.rs).

## Duyệt lại khi policy thay đổi (#275)

Theo `ADM-06@r1`, khi tác giả gửi lại đúng revision đã rút và policy đã tăng, approval cũ
của AI hoặc `legacy_bootstrap` chuyển sang `pending`. Decision đầu tiên là v1, approval sớm
nhất là v2, nên re-review sớm nhất là v3. Lượt mới tăng đúng một decision version, gắn policy
hiện tại và giữ digest của revision. Approval do admin hoặc moderator cấp đi vào hàng người
duyệt với `needs_admin/policy_rereview`. Khi automation tạm dừng, người duyệt cũng nhận
lượt chờ của AI. Chỉ quyết định hợp lệ cho revision còn khớp intent mới đặt được con trỏ công khai.

[Migration 20261121120000](../../services/vink-app/migrations/20261121120000_policy_rereview.sql)
kiểm hình dạng dòng, bao gồm grant và các trường provenance. Trigger kiểm cả quyết định trước
đó khi chuyển sang pending, vì hai dòng v3 có thể có cùng hình dạng dù chỉ một dòng kế tiếp
approval hợp lệ. Một re-review đã pending có thể được gửi lại với cùng version và policy của
quyết định đang chờ. Worker kiểm policy của ticket khi ghi kết quả. Các khóa của lệnh, core và
loader hash đầy đủ tiếp tục kiểm standing, intent generation, quyền người duyệt và snapshot.

Upgrade chỉ đổi schema và kiểm dữ liệu hiện có. Một dòng cũ thiếu provenance của verdict,
thiếu human verdict, hoặc pending có thông tin grant sẽ làm upgrade dừng nguyên giao dịch.
Khi nhận lệnh re-review, adapter lưu approval cũ còn thiếu trong history bằng các giá trị đã
ghi, kể cả timestamp và nguồn legacy. History đã có giữ nguyên actor, case, evaluation ID và
timestamp. Việc này cùng giao dịch với decision, intent, ticket/ca và receipt.

[Migration 20261121120100](../../services/vink-app/migrations/20261121120100_rereview_history.sql)
đối chiếu history của cả version trước và sau khi ghi lượt re-review. Tại commit, hai bản ghi
phải khớp work, revision, version, policy, trạng thái và các trường provenance, kể cả giá trị
NULL. History thiếu hoặc mâu thuẫn làm cả lệnh rollback. Gửi lại cùng lượt người duyệt giữ
version và policy. Khi chốt, decision tăng đúng một version, dùng policy hiện hành, và history
phải trỏ tới ca đã kết luận đúng snapshot, người duyệt, căn cứ admin/grant và verdict.
Đường ghi ca kiểm standing, quyền theo phạm vi và recusal. Các kiểm tra này áp dụng cho
giao dịch ghi re-review. Việc đối soát những quyết định lịch sử ngoài giao dịch đó có phạm vi riêng.

**Backout.** Dừng writer. Chạy [down 20261121120100](../../services/vink-app/migrations-rollback/20261121120100_rereview_history.down.sql)
rồi [down 20261121120000](../../services/vink-app/migrations-rollback/20261121120000_policy_rereview.down.sql).
Script đầu khóa bảng và kiểm điều kiện trước khi gỡ guard. PostgreSQL kiểm toàn bộ dòng khi khôi phục CHECK cũ. Có dòng hiện tại
`pending` với version > 1 thì giao dịch thất bại, ledger và dữ liệu được giữ nguyên. Giữ migration
và xử lý tiếp bằng luồng duyệt bình thường. Khi các dòng hiện tại đều tương thích, có thể backout
và nâng lại, kể cả khi history đã có lượt re-review. Bản cũ lại có giới hạn của CHECK v1.

Các ca [upgrade, backout, rollback, replay và SQL negatives](../../services/vink-app/tests/postgres/publication_policy.rs)
chạy trên PostgreSQL thật. [Ca admin và evaluator đang giữ lease](../../services/vink-app/tests/postgres/work_moderation.rs)
kiểm quyền quyết định và hàng chờ đúng snapshot.

## Đọc trạng thái không materialize history body (#230)

Owner-state dùng typed metadata projection và cùng restore invariants, giữ đủ history/HTTP v1;
command, moderation, worker và appeals vẫn dùng paired loader kiểm hash nội dung đầy đủ.
Public body kiểm hash v1 của revision đang được trỏ trước response/ETag. Xem
[caller/integrity map, compatibility và số đo PostgreSQL trước/sau](publication-state-read.md).
Không thêm schema hoặc đổi authority/standing/receipt/visibility.

## 5. Verification Evidence

Lần chạy 11/11/2026 trên nhánh `codex/publication-persistence-api` (PostgreSQL 17 cục bộ):

- `cargo xtask verify quick`, `cargo xtask verify discussion`, `cargo xtask verify postgres`, `cargo xtask verify kani` — kết quả ghi trong PR #161.
- `services/vink-app/tests/postgres/publication.rs` (9 test): vòng đời snapshot, ticket, r1/r2, riêng tư/ETag `404`, khóa `publications` FOR SHARE, legacy seed, nâng cấp migration thật, backout, retry đồng thời cùng key (thất bại với adapter đọc receipt trước khóa).

[Moderation runtime](moderation-runtime.md) nối worker/người duyệt, Web trạng thái/phản đối và reader
ẩn/bị gỡ. [Vùng công khai của Bàn viết](publication-author-web.md) (PUB-AUTHOR-WEB-01) là consumer
Web của bốn route trên: chọn audience, lưu → chụp → gửi, chuyển riêng tư, đối soát khi mất phản
hồi. Còn thiếu: CMP, xóa tác phẩm; replay của request-public không trả lại `ticket_id` (đọc qua
`GET …/publication/state`, Web đã xử lý như một câu trả lời bình thường). `revisions` của state
xếp cũ trước mới sau; consumer không dựa vào thứ tự.
