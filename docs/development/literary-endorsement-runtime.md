# Ghi nhận văn học (LVE) — application, HTTP API và PostgreSQL

**Lát:** PR2 + PR3 của [#73](https://github.com/loveoverflowcom/vink/issues/73) gộp một PR
([#81](https://github.com/loveoverflowcom/vink/pull/81)), đã port lên seam `decide_endorsement` của
#80 (develop `40f8a30`). **Trạng thái:** hiện thực và kiểm cục bộ; production vẫn trả `401` cho ghi
và trạng thái riêng vì chưa có auth authority (D-05). Không có UI, icon, xếp hạng hay consumer
web/native nào dùng các endpoint này.

Ý nghĩa và lựa chọn lõi: [decision record](../architecture/literary-endorsement.md). Lõi, Quint,
Kani và registry LVE-INV: [endorsement-core](endorsement-core.md). Hợp đồng HTTP chuẩn:
[API v1](../../services/vink-app/contracts/http/v1/README.md#literary-endorsement-lve-73). Số đo:
[bằng chứng PostgreSQL](evidence/lve-postgres/README.md).

## Owner

| Phần | File | Vai trò |
| --- | --- | --- |
| Use case | [`application/endorsement.rs`](../../services/vink-app/src/application/endorsement.rs) | Xác thực actor → kiểm input thành `EndorsementCommand` → **một** lời gọi store → ánh xạ outcome; batch `CommentBatch`; capability tư vấn |
| Port | [`application/ports.rs`](../../services/vink-app/src/application/ports.rs) `EndorsementStore` | Hợp đồng transaction nguyên tử; trả facts + outcome thuần đã áp |
| HTTP | [`http/api.rs`](../../services/vink-app/src/http/api.rs), [`contract.rs`](../../services/vink-app/src/http/contract.rs), [`error.rs`](../../services/vink-app/src/http/error.rs) | Route, DTO `deny_unknown_fields`, mã lỗi, `no-store` |
| Adapter | [`persistence/endorsement.rs`](../../services/vink-app/src/persistence/endorsement.rs) | Khóa → receipt → `decide_endorsement` → ghi state + receipt → commit; đọc batch một snapshot |
| Schema | [`migrations/20261105000000_literary_endorsement.sql`](../../services/vink-app/migrations/20261105000000_literary_endorsement.sql) | Hai bảng state + bảng receipt, FK đúng loại, token hợp lệ, trigger giữ hàng/order |
| Test | [`application/endorsement_tests.rs`](../../services/vink-app/src/application/endorsement_tests.rs), `http::contract::tests::endorsement`, `http::tests`, [`tests/postgres/endorsement.rs`](../../services/vink-app/tests/postgres/endorsement.rs) | Store ghi lại (chạy lõi thật trên facts kịch bản), fixture, router, PostgreSQL thật |

## Hợp đồng Rust (bàn giao)

```rust
// application::ports
pub trait EndorsementStore: Send + Sync {
    fn set_endorsement(&self, actor: PrincipalId, command: EndorsementCommand)
        -> impl Future<Output = Result<EndorsementWriteRecord, StoreError>> + Send;
    fn endorsement_summaries(&self, work_id: WorkId, comments: &CommentBatch)
        -> impl Future<Output = Result<EndorsementSummaryFacts, StoreError>> + Send;
    fn own_endorsements(&self, actor: PrincipalId, work_id: WorkId, comments: &CommentBatch)
        -> impl Future<Output = Result<OwnEndorsementFacts, StoreError>> + Send;
}
// EndorsementWriteRecord { facts: EndorsementFacts, outcome: EndorsementOutcome,
//                          count: EndorsementCount, published: Option<RevisionId> }

// application::endorsement (Vink<S: EndorsementStore>)
pub async fn set_work_endorsement(&self, actor: Option<PrincipalId>, command: SetWorkEndorsement)
    -> Result<EndorsementOutcomeView, EndorsementError>;
pub async fn set_comment_endorsement(&self, actor: Option<PrincipalId>, command: SetCommentEndorsement)
    -> Result<EndorsementOutcomeView, EndorsementError>;
pub async fn endorsement_summaries(&self, work_id: WorkId, comment_ids: Option<&str>)
    -> Result<EndorsementSummaries, EndorsementError>;
pub async fn own_endorsements(&self, actor: Option<PrincipalId>, work_id: WorkId, comment_ids: Option<&str>)
    -> Result<OwnEndorsements, EndorsementError>;
```

`EndorsementResult::{Accepted(state), Replayed { committed, observed }}`. `EndorsementError`:
`Unauthenticated` (401), `NotFound` (404), `SelfEndorsement` (403), `NotPermitted` (403
`endorsement_not_permitted`), `RevisionChanged { published }` (409), `Conflict { observed }` (409
`version_conflict`), `KeyReused` (422 `idempotency_key_reused`), `Invalid(field)` (422),
`Unavailable` (503), `Internal` (500, gồm `InvalidFacts` và `TokenExhausted`).
`StoreError::UnknownPrincipal` → `Unauthenticated`.

## Một đường ghi: `PUT …/discussion/comments/{id}/endorsement/me`

1. `http::api::set_comment_endorsement`: middleware Origin; `Actor` từ resolver hiện có (`503` khi
   Identity không quyết được); không actor → `401` **trước** khi đọc body; ID sai dạng → `404`;
   body `deny_unknown_fields` → `422`.
2. `Vink::set_comment_endorsement`: `command_key` → UUID, `expected_order` → `AcceptedOrder` (≥ 1),
   `endorsed` → Select/Remove, target `Comment { work: route, comment: route }`; gọi
   `EndorsementStore::set_endorsement` đúng một lần.
3. `PgStore` trong **một** transaction (READ COMMITTED): principal tồn tại (capability hiện hành
   = có hàng principal; chưa có cơ chế cấm/thu hồi) → `publications` của tác phẩm trên route `FOR
   SHARE` → `discussion_threads` `FOR SHARE` → bình luận **trong thread đó** `FOR SHARE OF c` →
   `pg_advisory_xact_lock(actor/key)` → receipt của (actor, key) → hàng `(comment, actor)` `FOR
   UPDATE` (chỉ khi bình luận thuộc tác phẩm) → `decide_endorsement` → chỉ khi `Accepted`:
   `UPDATE … WHERE accepted_order = <đã đọc>` hoặc `INSERT … ON CONFLICT DO NOTHING` (thua thì đọc lại
   dưới khóa và quyết định lại, tối đa một lần) + `INSERT` receipt → `count(*)` hàng `selected` nếu
   đích hiển thị → `COMMIT` → trả facts + outcome + count.
4. Use case ánh xạ `Accepted` → `outcome: accepted`; `Replay` → `outcome: replayed` với `committed`
   (lịch sử) và `endorsement` (hiện tại); `Conflict` → `409` kèm trạng thái quan sát; refusal → lỗi
   riêng. Capability tư vấn ở own state do chính `decide_endorsement` tính trên snapshot đọc.

Đường tác phẩm giống vậy, bỏ thread/bình luận; đích là `(work, revision)`; facts là
`works.owner_id` và `publications.revision_id` đã khóa; FK bảo đảm revision thuộc tác phẩm.

## Một đường đọc batch: `GET …/endorsements?comment_ids=`

`http::api::get_endorsement_summaries` (không resolve actor) → `CommentBatch::parse` (≤ 100 mục,
không phần tử rỗng, khử trùng theo thứ tự) → `PgStore::endorsement_summaries` trong một transaction
`REPEATABLE READ, READ ONLY`: (1) tác phẩm + publication + số hàng `selected` của revision hiện hành;
(2) các bình luận `= ANY($ids)` **join thread của chính tác phẩm** + số hàng `selected` → use case
gọi `public_count`: tác phẩm không công khai → `404`; bình luận thiếu, khác tác phẩm hoặc ẩn →
`unavailable`. Hai câu SQL bất kể số ID.

## Phản biện và điều chỉnh

| # | Câu hỏi | Kết luận | Điều chỉnh |
| --- | --- | --- | --- |
| F1 | Anchor đủ để không đánh giá nhầm revision? | Có: revision là một phần của đích (CHOICE-02 B). Chọn revision cũ là `TargetUnavailable`; HTTP nêu revision hiện hành (`409 revision_changed`) vì publication hiện hành là công khai | Ánh xạ chỉ cho Select trên tác phẩm công khai |
| F2 | No-op là quan sát hay ý định mới? | Mọi lệnh được chấp nhận đều tăng order (kể cả cùng trạng thái); không còn `AlreadyCurrent` | `outcome: accepted` luôn là lệnh mới |
| F3 | Replay bị hiểu là trạng thái hiện tại? | `Replay` trả `committed` riêng với `observed`; response đặt trạng thái hiện tại ở `endorsement`, lịch sử ở `committed` | Test replay sau khi bỏ chọn không hồi sinh |
| F4 | Thứ tự bảo đảm ở phạm vi nào? | Theo (actor, đích); khóa lệnh theo actor trên **mọi** đích | Advisory lock theo (actor, key); test cùng key hai đích → một thắng, một `KeyReused` |
| F5 | Ranh giới app/store giữ facts tới commit? | Store gọi lõi trong transaction; app không đọc trước rồi ghi | Port chỉ có lệnh nguyên tử |
| F6 | Skeleton giả readiness? | Lát gộp nối `PgStore` thật; production `401` | Test router với `NoAuthority`, Identity giả, DB down |
| F7 | Hàng vắng có phải khóa? | Không | `INSERT … ON CONFLICT DO NOTHING` + đọc lại + quyết định lại; test hai thứ tự commit |
| F8 | Lock order có đảo với submission/moderation/close? | Cùng tiền tố SHARE rồi chỉ khóa lệnh + hàng của mình | Test chờ khóa hai chiều với kiểm duyệt, rút/tái bản |
| F9 | Gỡ của chính mình khi đích ẩn có lộ đích của tác phẩm khác? | Hàng bình luận chỉ được đọc khi bình luận thuộc tác phẩm trên route | Test: hàng của bình luận tác phẩm khác không gỡ được qua route này |
| F10 | Đọc own state có quét bảng? | Ở ~18 k hàng planner chọn seq scan; ở 1 M dò khóa chính | Giữ `LEFT JOIN`; ghi số đo |

## Schema, toàn vẹn và lưu giữ

- **CHOICE-04 A:** `work_endorsements (work_id, revision_id, actor_id, selected, version,
  accepted_order)` PK `(work_id, revision_id, actor_id)`, FK `(work_id, revision_id)` → revision cùng
  tác phẩm; `comment_endorsements (comment_id, actor_id, …)` PK `(comment_id, actor_id)`. CHECK
  `1 ≤ version ≤ accepted_order` và `accepted_order ≥ 2` (hàng chỉ có sau lệnh được chấp nhận).
  `endorsement_receipts` PK `(actor_id, command_key)`, đích typed nullable + CHECK đúng một đích,
  `committed_order = expected_order + 1`. Bảng bình luận và receipt bình luận không lưu work dư; work
  lấy từ thread. B (một bảng state chung) bị loại: thêm NULL/CHECK mà truy vấn không bớt.
- **Lưu giữ (chốt cho lát):** bỏ chọn giữ hàng; hàng và receipt **không bao giờ bị xóa** — trigger
  statement-level chặn `DELETE`/`TRUNCATE` (receipt chặn cả `UPDATE`), trigger hàng chặn đổi danh
  tính, order không tăng, version giảm. Receipt giữ vô thời hạn vì chưa có policy retention. Purge
  tài khoản/đích cần policy và migration riêng (LVE-UC-17).

## Tổng hợp, đối soát và cache

- **CHOICE-05 A:** đếm hàng `selected` khi đọc (partial index `work_endorsements_selected_idx`,
  `comment_endorsements_selected_idx`). Không counter, outbox hay cache. Hot-target 16 người ghi:
  6,2–6,4 k tps so với counter 5,6 k; 1 người ghi chậm hơn và giảm khi dead tuple tích lũy
  ([bằng chứng](evidence/lve-postgres/README.md)).
- **Truy vấn đối soát** (dùng trong `summaries_equal_an_independent_recount_of_canonical_rows`):

  ```sql
  SELECT e.work_id, count(*) FILTER (WHERE e.selected AND e.revision_id = p.revision_id) AS effective
    FROM work_endorsements e JOIN publications p USING (work_id) GROUP BY e.work_id;
  SELECT comment_id, count(*) FILTER (WHERE selected) FROM comment_endorsements GROUP BY comment_id;
  ```

- **Cache (CHOICE-13 A):** tổng số công khai và trạng thái riêng là hai endpoint, đều `no-store`.
  READ-01 và READ-02 không đổi (test so ETag trước/sau khi ghi nhận).

## Migration, quyền và backout

- Migration mới, chưa từng phát hành trước PR này. Runtime nhận DML qua `ALTER DEFAULT PRIVILEGES`
  của [`owner.sql`](../../services/postgres/provision/owner.sql); `DELETE` bị trigger từ chối, runtime
  không có `TRUNCATE`. `verify postgres` kiểm trên database đã provision (INSERT/UPDATE được,
  `DELETE` → `23000`, `TRUNCATE` → `42501`; App thật trả tổng số `200` bằng role runtime).
- **Tắt khi rollout:** production đã `401` cho ghi. Muốn tắt cả đọc: gỡ `merge(endorsement)` trong
  `http/api.rs` và deploy lại; schema giữ nguyên.
- **Backout schema** (chấp nhận mất mọi ghi nhận và receipt): `DROP TABLE endorsement_receipts,
  work_endorsements, comment_endorsements; DROP FUNCTION guard_endorsement_update(),
  reject_endorsement_removal();` rồi xóa dòng `20261105000000` khỏi `_sqlx_migrations` bằng migrator,
  sau khi deploy binary không có migration này.

## Chưa có hoặc phụ thuộc lát khác

Auth authority/session (D-05) và cơ chế thu hồi capability, lệnh publish/withdraw thật (bằng chứng
dùng SQL lên con trỏ), cách ly/cấm/purge tài khoản, retention receipt, giới hạn tần suất (LVE-UC-15),
ghi danh (CHOICE-12 B), xếp hạng (CHOICE-06), UI/icon/consumer web/native, tổng số cho danh sách
nhiều tác phẩm.

## Kiểm chứng (05/11/2026, macOS, PostgreSQL 17.11 scratch)

| Lớp | Lệnh | Kết quả | Phạm vi |
| --- | --- | --- | --- |
| Use case với store ghi lại | `cargo test -p vink-app --lib endorsement_tests` | PASS 7 | Wiring, ánh xạ outcome của lõi thật; **không** phải bằng chứng SQL |
| Contract | `cargo test -p vink-app --lib contract` | PASS (4 test LVE) | Provider serialize khớp fixture; request từ chối actor/count/role/work, thiếu revision |
| Router | `cargo test -p vink-app --lib http::` | PASS | `401` khi không authority, `403` Origin, DB down → `503` không phải `0`, Identity giả |
| PostgreSQL thật | `cargo xtask verify postgres` | PASS | 12 test LVE: SQL trực tiếp, order/receipt/ABA/mất ACK, 8 bản sao một lệnh, 8 lệnh đầu khác khóa, cùng khóa hai đích, hàng vắng + INSERT chờ theo hai thứ tự commit, chờ khóa hai chiều với kiểm duyệt và rút/tái bản, mỗi revision một đích, gỡ của chính mình khi đích ẩn, đóng bình luận độc lập, quyền riêng tư, đối soát, HTTP; role runtime và App + Identity thật |
| Độ nhạy (tay) | Mutation trong adapter | Bỏ advisory lock theo khóa lệnh, bỏ đọc-lại sau INSERT thua: mỗi cái làm đúng test tương ứng FAIL | Không phải bước của runner |
| Plan / hot-target | `psql -f queries.sql`, `pgbench` | Ở [bằng chứng](evidence/lve-postgres/README.md) | Dataset tổng hợp, một máy |

Lõi, model và Kani của #80 không đổi; lane `discussion`/`kani` chạy lại sau merge để xác nhận.

## Ánh xạ nghiệm thu

| AC | Bằng chứng ở lát này | Còn thiếu |
| --- | --- | --- |
| P2-01…07 | Một hợp đồng `PUT` (key + expected order), hai batch read, port nguyên tử, accepted/replay/conflict tách, `no-store`, fixture | Auth thật (D-05) |
| P3-01…08 | Như bảng kiểm chứng | Lệnh publish/withdraw thật; tài khoản; retention |
| #73 AC-03 | Seam thuần có production caller (adapter + HTTP) | Chỉ chạy được với principal thật khi có D-05 |
| #73 AC-07 | Migration/FK/unique/token/role/backout + SQL trực tiếp | — (backout là thủ tục ghi lại) |
| #73 AC-08 | Trùng, chạm đúp, nhiều tab, ABA, mất ACK, đồng thời, rút/tái bản, kiểm duyệt | Thu hồi capability (chưa có cơ chế) |
| #73 AC-09 | Inventory, batch không N+1, plan, hot-target, dataset/môi trường | Đo trên hạ tầng thật |
| #73 AC-10 | Tổng số theo revision hiện hành; visibility hiện hành thắng receipt | Xếp hạng/cursor (CHOICE-06) |
| #73 AC-13 | Tách cache, own state theo actor, guest `401`, lý do vô hiệu | Abuse/retention theo tài khoản, consent ghi danh, session fence phía client |
| #73 AC-14 | Đối soát, backout, docs, feature map | UI/rollout thật; epic giữ mở |
