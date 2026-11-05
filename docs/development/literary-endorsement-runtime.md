# Ghi nhận văn học (LVE) — application, HTTP API và PostgreSQL

**Lát:** PR2 + PR3 của [#73](https://github.com/loveoverflowcom/vink/issues/73) gộp một PR, nền
`develop@69f0a3e` (đã có lõi PR1 #78 và fence #79). **Trạng thái:** đã hiện thực và kiểm cục bộ;
production vẫn trả `401` cho mọi ghi và trạng thái riêng vì chưa có auth authority (D-05). Không
có UI, icon, xếp hạng hay consumer web/native nào dùng các endpoint này.

Ý nghĩa và lựa chọn: [decision record](../architecture/literary-endorsement.md). Lõi thuần, Quint,
Kani và registry LVE-INV: [lõi](literary-endorsement-core.md). Hợp đồng HTTP chuẩn:
[API v1](../../services/vink-app/contracts/http/v1/README.md#literary-endorsement-lve-73). Số đo:
[bằng chứng PostgreSQL](evidence/lve-postgres/README.md).

## Owner

| Phần | File | Vai trò |
| --- | --- | --- |
| Use case | [`application/endorsement.rs`](../../services/vink-app/src/application/endorsement.rs) | Xác thực actor → kiểm input → **một** lời gọi store → ánh xạ quyết định; batch `CommentBatch`; capability tư vấn |
| Port | [`application/ports.rs`](../../services/vink-app/src/application/ports.rs) `EndorsementStore` | Hợp đồng transaction nguyên tử; trả facts + quyết định thuần đã áp |
| HTTP | [`http/api.rs`](../../services/vink-app/src/http/api.rs), [`contract.rs`](../../services/vink-app/src/http/contract.rs), [`error.rs`](../../services/vink-app/src/http/error.rs) | Route, DTO `deny_unknown_fields`, mã lỗi, `no-store` |
| Adapter | [`persistence/endorsement.rs`](../../services/vink-app/src/persistence/endorsement.rs) | Khóa → `decide_*_endorsement` → ghi có điều kiện → commit; đọc batch một snapshot |
| Schema | [`migrations/20261105000000_literary_endorsement.sql`](../../services/vink-app/migrations/20261105000000_literary_endorsement.sql) | Hai bảng, FK đúng loại, version ≥ 2, trigger giữ hàng/version |
| Test | [`application/endorsement_tests.rs`](../../services/vink-app/src/application/endorsement_tests.rs), `http::contract::tests::endorsement`, `http::tests`, [`tests/postgres/endorsement.rs`](../../services/vink-app/tests/postgres/endorsement.rs) | Store ghi lại (wiring), fixture, router, PostgreSQL thật |

## Hợp đồng Rust (bàn giao)

```rust
// application::ports
pub trait EndorsementStore: Send + Sync {
    fn set_work_endorsement(&self, actor: PrincipalId, work_id: WorkId, request: WorkRequest)
        -> impl Future<Output = Result<WorkEndorsementWrite, StoreError>> + Send;
    fn set_comment_endorsement(&self, actor: PrincipalId, work_id: WorkId, comment_id: CommentId,
        request: CommentRequest) -> impl Future<Output = Result<CommentEndorsementWrite, StoreError>> + Send;
    fn endorsement_summaries(&self, work_id: WorkId, comments: &CommentBatch)
        -> impl Future<Output = Result<EndorsementSummaryFacts, StoreError>> + Send;
    fn own_endorsements(&self, actor: PrincipalId, work_id: WorkId, comments: &CommentBatch)
        -> impl Future<Output = Result<OwnEndorsementFacts, StoreError>> + Send;
}
// WorkEndorsementWrite { facts: Option<WorkFacts>, decision: WorkDecision, count: EndorsementCount }
// CommentEndorsementWrite { facts: Option<CommentFacts>, decision: CommentDecision, count }

// application::endorsement (Vink<S: EndorsementStore>)
pub async fn set_work_endorsement(&self, actor: Option<PrincipalId>, command: SetWorkEndorsement)
    -> Result<EndorsementOutcome, EndorsementError>;
pub async fn set_comment_endorsement(&self, actor: Option<PrincipalId>, command: SetCommentEndorsement)
    -> Result<EndorsementOutcome, EndorsementError>;
pub async fn endorsement_summaries(&self, work_id: WorkId, comment_ids: Option<&str>)
    -> Result<EndorsementSummaries, EndorsementError>;
pub async fn own_endorsements(&self, actor: Option<PrincipalId>, work_id: WorkId, comment_ids: Option<&str>)
    -> Result<OwnEndorsements, EndorsementError>;
```

`EndorsementError`: `Unauthenticated` (401), `NotFound` (404), `SelfEndorsement` (403
`self_endorsement`), `RevisionChanged { published }` (409 `revision_changed`), `CommentsClosed` (409),
`Conflict { current }` (409 `version_conflict` + trạng thái lưu), `Invalid(field)` (422),
`Unavailable` (503), `Internal` (500, gồm `VersionExhausted`). `StoreError::UnknownPrincipal` →
`Unauthenticated`. DTO, fixture và bảng lỗi: [API v1](../../services/vink-app/contracts/http/v1/README.md#literary-endorsement-lve-73).

## Một đường ghi: `PUT …/discussion/comments/{id}/endorsement/me`

1. `http::api::set_comment_endorsement`: middleware Origin; `Actor` từ resolver hiện có (`503` khi
   Identity không quyết được); không actor → `401` **trước** khi đọc body; ID sai dạng → `404`;
   body `deny_unknown_fields` → `422`.
2. `Vink::set_comment_endorsement`: `expected_version` → `EndorsementVersion` (≥ 1), `endorsed` →
   `CommentIntent`; gọi `EndorsementStore::set_comment_endorsement` đúng một lần.
3. `PgStore` trong **một** transaction (READ COMMITTED):
   principal tồn tại → `publications` của tác phẩm trên route `FOR SHARE` → `discussion_threads`
   `FOR SHARE` → bình luận **trong thread đó** `FOR SHARE OF c` (work lấy từ thread của chính bình
   luận) → hàng `(comment, actor)` `FOR UPDATE` hoặc `Endorsement::ABSENT` →
   `decide_comment_endorsement` → chỉ khi `Transition::write()` có giá trị: `UPDATE … WHERE version =
   <đã đọc>` hoặc `INSERT … ON CONFLICT DO NOTHING` (thua thì đọc lại dưới khóa và quyết định lại, tối
   đa một lần) → `count(*)` các hàng `selected` → `COMMIT` → trả facts + quyết định + count.
4. Use case ánh xạ: `Changed`/`Reaffirmed` → `accepted: true`; `AlreadyCurrent` → `accepted:
   false`; `Stale` → `Conflict` kèm trạng thái lưu; các từ chối → lỗi riêng. Trạng thái riêng và
   capability do `own_comment_view` + chính `decide_comment_endorsement` (yêu cầu thử ở version hiện
   tại) tính, không viết luật thứ hai.

Đường tác phẩm giống vậy, bỏ thread/bình luận; facts là `works.owner_id` và
`publications.revision_id` đã khóa, hàng lưu `revision_id` server đã kiểm.

## Một đường đọc batch: `GET …/endorsements?comment_ids=`

`http::api::get_endorsement_summaries` (không resolve actor) → `CommentBatch::parse` (≤ 100 mục,
không phần tử rỗng, khử trùng theo thứ tự) → `PgStore::endorsement_summaries` trong một transaction
`REPEATABLE READ, READ ONLY`: (1) tác phẩm + publication + số hàng có `revision_id` = revision hiện
hành; (2) các bình luận `= ANY($ids)` **join thread của chính tác phẩm** + số hàng `selected` →
use case gọi `work_summary`/`comment_summary`: tác phẩm không công khai → `404`; bình luận thiếu,
khác tác phẩm hoặc ẩn → `Unavailable`. Hai câu SQL bất kể số ID; plan ở
[bằng chứng](evidence/lve-postgres/README.md).

## Phản biện trước khi viết (findings và điều chỉnh)

| # | Câu hỏi | Kết luận và nguồn | Điều chỉnh trong lát |
| --- | --- | --- | --- |
| F1 | Anchor đủ để không đánh giá nhầm revision? | Đủ: request mang `observed_revision_id`, lõi so với publication **đã khóa** và lưu revision đã kiểm (`work.rs`). Mất ACK + tái bản: gửi lại cho `409 revision_changed` dù lần trước đã commit | Hợp đồng ghi rõ “sau kết quả không chắc, đọc lại own state”; test `republication_needs_reconfirmation…` |
| F2 | No-op là trạng thái quan sát hay ý định mới? | Hai thứ khác nhau: `Reaffirmed` ghi và fence, `AlreadyCurrent` không ghi (`endorsement.rs`) | HTTP tách bằng `accepted`; cả hai `200` |
| F3 | Receipt bị hiểu là trạng thái hiện tại? | Không có receipt (CHOICE-03 B′); một response cũ chỉ được áp khi `version` lớn hơn | Không thêm bảng receipt; không tái dùng `discussion_command_receipts` (CHECK `submit_comment`, FK bình luận) |
| F4 | Thứ tự được bảo đảm ở phạm vi nào? | Theo (actor, đích); không giữa đích/actor; hai thiết bị: ai được chấp nhận trước | Ghi trong contract |
| F5 | Ranh giới app/store có giữ facts tới commit? | Có nếu store gọi lõi **trong** transaction; app không đọc trước rồi ghi | Port chỉ có lệnh nguyên tử; app không nhận facts trước khi ghi |
| F6 | Skeleton có giả readiness? | Lát gộp nối `PgStore` thật (CHOICE-14 không còn cần chọn); production `401` | Test router với `NoAuthority`, Identity giả và DB down |
| F7 | Hàng vắng có phải khóa? | Không; hai lần ghi đầu cùng thấy vắng | `INSERT … ON CONFLICT DO NOTHING` + đọc lại + quyết định lại; test hai thứ tự commit |
| F8 | Lock order có đảo với submission/moderation/close? | Endorsement lấy cùng tiền tố (publication → thread → bình luận) ở chế độ SHARE rồi chỉ hàng của mình | Test chờ khóa theo cả hai chiều với close, moderation, rút/tái bản |
| F9 | Scope bình luận chỉ do SQL? | Mutation bỏ `thread_id` cho thấy lõi chấp nhận nếu facts lấy work từ thread của route | Facts lấy work từ thread **của bình luận**; mutation đó giờ vẫn bị lõi từ chối |
| F10 | Đọc own state 80 bình luận có quét bảng? | Ở 18 k hàng planner chọn seq scan (191 trang); ở 1 M chuyển sang dò khóa chính | Giữ `LEFT JOIN`; ghi số đo |

Không cần đổi seam public của PR1: mọi quyết định gọi đúng `decide_work_endorsement`,
`decide_comment_endorsement`, `own_*_view`, `*_summary`.

## Schema, toàn vẹn và lưu giữ

- **CHOICE-04 A:** `work_endorsements (work_id, actor_id, revision_id NULL, version ≥ 2)` và
  `comment_endorsements (comment_id, actor_id, selected, version ≥ 2)`. FK đúng loại; anchor
  `(work_id, revision_id)` → `revisions (work_id, id)` nên revision luôn thuộc đúng tác phẩm; PK là
  một hàng mỗi (actor, đích). Bảng bình luận không lưu work/thread dư, nên không thể mâu thuẫn. B
  (một bảng, FK nullable + CHECK đúng một đích) bị loại: hai anchor khác kiểu, thêm NULL/CHECK mà
  truy vấn không bớt.
- **Lưu giữ (chốt cho lát):** hàng tồn tại chỉ sau ý định được chấp nhận (version ≥ 2; vắng = version
  1). Bỏ chọn giữ hàng với version mới; hàng **không bao giờ bị xóa** — trigger statement-level chặn
  `DELETE` (kể cả không khớp hàng nào) và `TRUNCATE`; trigger hàng chặn đổi danh tính và mọi UPDATE
  không tăng version. Không có receipt nên không có gì khác phải giữ. Xóa tài khoản/purge đích cần
  policy và migration riêng (LVE-UC-17), lát này không mở rộng.
- Tự ghi nhận, visibility và tắt bình luận là policy của lõi (đổi theo policy version), được kiểm lại
  trong mỗi transaction, không phải constraint SQL.

## Tổng hợp, đối soát và cache

- **CHOICE-05 A:** đếm từ hàng chuẩn khi đọc (partial index `work_endorsements_anchor_idx`,
  `comment_endorsements_selected_idx`). Không counter, outbox hay cache nên không có projection nào
  lệch để dựng lại. B bị loại vì hot-target tuần tự hóa người ghi (16 client: 6,9 k so với 8,4 k tps);
  C vì chưa có nhu cầu bất đồng bộ. Số đo và ngưỡng xem lại: [bằng chứng](evidence/lve-postgres/README.md).
- **Truy vấn đối soát** (độc lập với đường đọc, dùng trong test
  `summaries_equal_an_independent_recount_of_canonical_rows`):

  ```sql
  SELECT e.work_id, count(*) FILTER (WHERE e.revision_id = p.revision_id) AS effective
    FROM work_endorsements e JOIN publications p USING (work_id) GROUP BY e.work_id;
  SELECT comment_id, count(*) FILTER (WHERE selected) FROM comment_endorsements GROUP BY comment_id;
  ```

- **Cache (CHOICE-13 A):** tổng số công khai và trạng thái riêng là hai endpoint, đều `no-store`
  (tổng số là projection biến động, chưa có validator; trạng thái riêng thuộc tài khoản). Hai endpoint
  READ-01 và owner READ-02 không đổi: body, ETag và `private, no-cache` giữ nguyên (test so ETag trước
  và sau khi ghi nhận). Lỗi endorsement không chạm đường đọc tác phẩm.

## Migration, quyền và backout

- Migration mới, không sửa migration đã áp. Chạy bằng credential migrator (`vink-app migrate`);
  runtime nhận `SELECT, INSERT, UPDATE, DELETE` qua `ALTER DEFAULT PRIVILEGES` sẵn có của
  [`owner.sql`](../../services/postgres/provision/owner.sql), nhưng `DELETE` vẫn bị trigger từ chối và
  runtime không có `TRUNCATE`. `verify postgres` kiểm trên database đã provision: runtime
  INSERT/UPDATE được, `DELETE` → `23000`, `TRUNCATE` → `42501`; App thật trả tổng số `200` bằng role
  runtime, kể cả khi Identity dừng.
- **Tắt khi rollout:** chưa có cờ; trong production mọi ghi đã là `401`. Nếu cần tắt cả đọc, gỡ
  `merge(endorsement)` trong `http/api.rs` và deploy lại — schema giữ nguyên, không mất dữ liệu.
- **Backout schema** (chỉ khi chấp nhận mất mọi ghi nhận; không có migration down tự động, theo quy
  ước repo): `DROP TABLE work_endorsements, comment_endorsements; DROP FUNCTION
  guard_endorsement_update(), reject_endorsement_removal();` rồi xóa dòng
  `20261105000000` khỏi `_sqlx_migrations` bằng migrator, sau khi deploy binary không có migration này.

## Chưa có hoặc phụ thuộc lát khác

Auth authority/session (D-05), lệnh publish/withdraw thật (bằng chứng dùng thao tác SQL lên con trỏ),
cách ly/cấm tài khoản, giới hạn tần suất (LVE-UC-15), ghi danh người ghi nhận (CHOICE-12 B), xếp hạng
(CHOICE-06), UI/icon/copy và consumer web/native (LVE-FR-08, AC-11/12), tổng số cho danh sách nhiều
tác phẩm. Chủ sản phẩm chưa xác nhận CHOICE-02/08/09; lát này không đổi chúng.

## Kiểm chứng (05/11/2026, macOS, PostgreSQL 17.11 scratch)

| Lớp | Lệnh | Kết quả | Phạm vi |
| --- | --- | --- | --- |
| Use case với store ghi lại | `cargo test -p vink-app --lib endorsement_tests` | PASS 9 | Wiring, ánh xạ, thứ tự xác thực/kiểm input; **không** phải bằng chứng SQL |
| Contract | `cargo test -p vink-app --lib contract` | PASS (4 test LVE) | Provider serialize khớp fixture; request từ chối actor/count/role/work |
| Router | `cargo test -p vink-app --lib http::` | PASS | `401` khi không authority, `403` Origin, DB down → `503` không phải `0`, Identity giả |
| PostgreSQL thật | `cargo xtask verify postgres` | PASS | 13 test LVE: SQL trực tiếp, vòng đời, race hai kết nối theo hai thứ tự commit, rút/tái bản, đóng bình luận, kiểm duyệt, quyền riêng tư, HTTP; role runtime và App + Identity thật |
| Độ nhạy (tay, khi phát triển) | Mutation trong adapter | Bỏ đọc-lại sau INSERT thua, bỏ khóa thread, bỏ khóa publication, bỏ scope thread: mỗi cái làm đúng một test FAIL | Không phải bước của runner |
| Plan / hot-target | `psql -f queries.sql`, `pgbench` | Ghi ở [bằng chứng](evidence/lve-postgres/README.md) | Dataset tổng hợp, một máy |

Lõi, model và Kani không đổi trong lát này; `discussion` và `kani` không cần chạy lại vì nguồn của
chúng không đổi.

## Ánh xạ nghiệm thu

| AC | Bằng chứng ở lát này | Còn thiếu |
| --- | --- | --- |
| P2-01 | Một hợp đồng `PUT` desired-state + version; hai batch read | — |
| P2-02 | Port nguyên tử; adapter gọi `decide_*` trong transaction | — |
| P2-03 | `accepted` tách `Reaffirmed`/`AlreadyCurrent`; không receipt; `Conflict` mang trạng thái lưu | — |
| P2-04 | `deny_unknown_fields`, actor từ resolver, batch ≤ 100, scope bình luận theo thread | — |
| P2-05 | Hai endpoint `no-store`; READ-01 không đổi | — |
| P2-06 | `401`/`503` trung thực; không stub | Auth thật (D-05) |
| P2-07 | Fixture + test ánh xạ có assertion | — |
| P3-01…07 | Như bảng kiểm chứng | Lệnh publish/withdraw thật; tài khoản |
| P3-08 | Tài liệu này và PR không nhận auth/UI/epic | — |
| #73 AC-03 | Seam thuần có production caller (adapter + HTTP) | Caller production chỉ chạy được với principal thật (D-05) |
| #73 AC-07 | Migration/FK/unique/version/role/backout + SQL trực tiếp | — (backout là thủ tục ghi lại, không chạy tự động) |
| #73 AC-08 | Trùng, chạm đúp, nhiều tab, ABA, mất ACK, đồng thời, rút/tái bản, kiểm duyệt, đóng bình luận | Thu hồi quyền tài khoản (chưa có cơ chế) |
| #73 AC-09 | Inventory, batch không N+1, plan, hot-target, dataset/môi trường | Đo trên hạ tầng thật |
| #73 AC-10 | Tổng số theo policy, visibility hiện hành thắng | Xếp hạng/cursor (CHOICE-06 OPEN) |
| #73 AC-13 | Tách cache, own state theo actor, guest `401`, lý do vô hiệu | Abuse/retention theo tài khoản, consent ghi danh, session fence phía client |
| #73 AC-14 | Đối soát, backout, docs, feature map | UI/rollout thật; epic giữ mở |
