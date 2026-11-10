# Ghi nhận văn học (LVE) — application, HTTP API và PostgreSQL

**Lát:** PR2 + PR3 của [#73](https://github.com/loveoverflowcom/vink/issues/73) gộp một PR
([#81](https://github.com/loveoverflowcom/vink/pull/81)), đã port lên seam `decide_endorsement` của
#80 (develop `40f8a30`). **Trạng thái:** hiện thực và kiểm cục bộ; production vẫn trả `401` cho ghi
và trạng thái riêng vì chưa có auth authority (D-05). Consumer đầu tiên là reader Web
([adapter WP-04](#web-adapter-wp-04)): `EndorsementAction` cho tác phẩm và từng bình luận, khách chỉ
thấy tổng số + lý do bằng chữ; lệnh thật chỉ chạy với LocalFixture loopback. Glyph Pen Flight R2 A (CHOICE-07) đã được gắn qua
registry/patterns ([spec](../design/pen-flight-r2.md)); chưa xếp hạng hay consumer native.

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
| Schema | [`migrations/20261105000000_literary_endorsement.sql`](../../services/vink-app/migrations/20261105000000_literary_endorsement.sql), [`20261105120000_endorsement_version_parity.sql`](../../services/vink-app/migrations/20261105120000_endorsement_version_parity.sql) | Hai bảng state + bảng receipt, FK đúng loại, token hợp lệ, selection khớp parity version, trigger giữ hàng/order |
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
- **Trạng thái đạt được (`20261105120000`):** lõi bắt đầu OFF/v1 và chỉ tăng version khi selection
  đổi, nên mọi trạng thái đạt được có `selected ⇔ version chẵn`. CHECK `*_version_parity_check` trên
  hai bảng state và receipt (`committed_version`) giữ đúng điều đó; `EndorsementState::restore` từ chối
  cùng hình dạng khi hydrate. Không mã hóa bước chuyển trong SQL (không reducer thứ hai): order có thể
  nhảy hoặc version tăng chẵn bằng SQL trực tiếp vẫn không bị schema chặn, chỉ bị lõi fence.
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
  reject_endorsement_removal();` rồi xóa dòng `20261105000000` và `20261105120000` khỏi
  `_sqlx_migrations` bằng migrator, sau khi deploy binary không có hai migration này. Chỉ gỡ
  parity: `ALTER TABLE … DROP CONSTRAINT *_version_parity_check` trên ba bảng rồi xóa dòng
  `20261105120000`; binary vẫn từ chối hàng sai parity khi hydrate (đọc/ghi riêng trả `500`).
- **Nâng cấp:** `ADD CONSTRAINT … CHECK` kiểm mọi hàng hiện có dưới khóa của migration. Hàng do App
  ghi luôn hợp lệ; database từng bị SQL trực tiếp ghi hàng không đạt được (như bản cũ của
  `scale-1m.sql`) sẽ dừng migration. Không có sửa tự động an toàn: cần xem từng hàng.

## Rà soát domain ↔ API ↔ DB (#73, sau #81/#83)

Nền `develop@a9afecc`. Lần theo cả hai chiều: request → `http::api` (actor, ID, DTO
`deny_unknown_fields`) → `application::endorsement` (key/order/intent → `EndorsementCommand`) →
`PgStore::set_endorsement` (khóa facts → `decide_endorsement` → state + receipt → count → commit) →
`settle` → DTO; và DB → hydrate (`tokens`, `placement`, `ReceiptRow::receipt`) → `public_count`/`own`
→ DTO `no-store`. Không có reducer thứ hai: HTTP/SQL không tự quyết eligibility, order hay replay.

| ID | Nguồn | Quan sát | Rủi ro | Test có sẵn | Bảo đảm thiếu | Phương án | Mức / xử lý |
| --- | --- | --- | --- | --- | --- | --- | --- |
| R1 | `EndorsementState::restore`; `*_tokens_check`, `guard_endorsement_update`; `evidence/…/scale-1m.sql` | Lõi chỉ đạt `selected ⇔ version chẵn`; restore và schema nhận ON@v1, OFF@v2, đảo `selected` không tăng version. Script bằng chứng đã commit ghi OFF@v2 và DB nhận | Tổng số công khai là `count(*) … WHERE selected`, không qua restore, nên đếm hàng không lệnh nào tạo ra; lệnh kế tiếp lấy nó làm nền | `the_database_alone_enforces…` chỉ kiểm token | Hàng bền = trạng thái đạt được | A: chỉ restore (không che SQL count); **B: CHECK parity + restore**; C: trigger mã hóa bước chuyển (reducer thứ hai, loại) | P1 — sửa (B), migration tiến `20261105120000` |
| R2 | `PgStore::set_endorsement` | Lỗi sau khi đã ghi state (INSERT receipt) dựa vào rollback; chỉ có test refusal trước khi ghi | State/count lệch hoặc key bị tiêu nếu sau này commit sớm | `a_refused_or_failed_write…` | Rollback giữa transaction | Thêm test tiêm lỗi (trigger test) cho cả nhánh UPDATE và INSERT | P2 — hành vi đúng, thêm regression |
| R3 | doc module/`decide_endorsement`, header `endorsement-core.md` | Còn ghi "chưa adapter nào gọi", "ngoài PR1" sau #81 | Đọc sai readiness | — | Tài liệu đúng | Cập nhật | P3 — sửa |
| R4 | `application::endorsement::settle` | Replay lệnh Select đã commit khi đích nay ẩn/revision cũ trả `404`/`409 revision_changed` ("nothing was written"), không `replayed` — đúng policy #80 | Client mất ACK không biết Select cũ đã commit; hàng ON của revision cũ hiệu lực lại nếu revision đó được đăng lại | `each_revision…`, test replay | Quyết định sản phẩm | Xem OPEN-R4 | OPEN — giữ hành vi, ghi chú trong code |
| R5 | `settle` (map `RevisionChanged`) | Select với revision không thuộc tác phẩm cũng nhận `409 revision_changed` kèm revision hiện hành | Thông điệp "republished" không đúng với ID rác; không lộ gì (revision hiện hành công khai) | `a_refused_or_failed_write…` | — | A giữ; B adapter kiểm revision ∈ work → `404` | P3 — giữ (A), ghi chú |
| R6 | `persistence::classify` / `vink-store-postgres` | `40P01`/`40001` thành `Integrity` → `500` | Deadlock (chưa tái hiện) sẽ thành lỗi nội bộ thay vì retry | Test chờ khóa hai chiều | Chính sách lỗi chung của store | Chung cho mọi adapter, ngoài LVE | P3 — ghi nhận |
| R7 | `own()` / `GET …/endorsements/me` | Hàng riêng trên đích ẩn/revision cũ đọc ra `unavailable`, không có order; gỡ cần `observed` từ `409` | UX gỡ riêng tư; không lộ dữ liệu | `each_revision…` | Thuộc CHOICE-10 | — | Chốt ở [Web adapter](#web-adapter-wp-04): không UI, chỉ API |

Đã kiểm và **không đổi**: actor từ resolver, `401` trước body; ID sai dạng `404`; binding đích
(bình luận chỉ đọc trong thread của tác phẩm trên route, receipt bình luận lấy work từ thread, FK
revision cùng tác phẩm); fence accepted order và namespace receipt trên mọi đích (advisory lock);
INSERT thua → quyết định lại; thứ tự khóa là tiền tố của discussion; đọc batch ≤ 100, hai câu SQL,
một snapshot `REPEATABLE READ`; `unavailable` ≠ 0; own state chỉ cho đích visible; quyền role runtime.

**Quyết định của lần rà soát:** DECISION_A = A (giữ cấu trúc, thêm kiểm/test); DECISION_B = C chỉ cho
R1 (migration tiến vì tổng số SQL không thể được bảo vệ ở Rust), còn lại A; DECISION_C = A (giữ batch
read, không index/aggregate mới vì không có số đo mới).

**OPEN-R4 (cần product owner, giữ hành vi hiện tại):** A — giữ: Select lịch sử trên đích không còn
visible không replay (riêng tư trước, client đối soát bằng own state của revision hiện hành);
B — replay `200 replayed` với `summary: unavailable` cho chính actor (chỉ lộ dữ liệu của họ, client
biết lệnh cũ đã commit); C — mã riêng `409 replay_target_unavailable` kèm `committed`. Phép thử phân
định: kịch bản mất ACK → tái bản → retry cùng key; đo client có hiển thị đúng "đã ghi nhận bản cũ"
và có gỡ được hàng cũ không.

**Kiểm chứng của lần rà soát** (06/11/2026, Linux x86_64, Rust 1.99.1, Node 24.22.2, Quint 0.33.1
theo lockfile, Kani 0.67.0/CBMC 6.8.0, PostgreSQL 17.11 trong container tạm; `psql`/`pg_dump` 17.11
qua `PATH`/`VINK_PG_BIN`):

| Lệnh / ranh giới | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | PASS — fmt, clippy ×3, doctest type-boundary, 136 unit/contract `vink-app`, repo checks (lần đầu FAIL `fmt` trên test mới, đã `cargo fmt`) |
| `cargo xtask verify discussion` | PASS — 4 model, drift, negative controls, 32 test Rust gồm replay LVE bắt buộc |
| `cargo xtask verify kani` | PASS — App 19/19, expressions 3/3; 42/42 cover SATISFIED; harness `acceptance_fences…` mới kiểm parity |
| `cargo xtask verify postgres` | 47/47 test App trên PostgreSQL thật PASS (14 LVE, gồm 2 mới), provision/role runtime/worker PASS; bước Identity "invalid configuration" **FAIL do môi trường**: đường dẫn worktree dài làm `VINK_IDENTITY_SOCKET` vượt 100 byte; các bước Identity sau đó NOT_RUN. Chạy lại từ checkout có đường dẫn ngắn (≤ ~55 ký tự) |
| Độ nhạy (tay) | Bỏ parity khỏi `restore`: `accepted_states_keep_selection_parity…` và `an_unreachable_stored_state…` FAIL; bỏ migration: `the_database_alone_enforces…` FAIL. Source khôi phục |

Test rollback R2 khóa hành vi đã đúng; không có mutant adapter cho nó. Quint không chứng minh khóa
PostgreSQL; Kani không chứng minh HTTP/auth/SQL; LocalFixture/Identity giả không chứng minh auth thật.

## Web adapter (WP-04)

Nền: PR [#85](https://github.com/loveoverflowcom/vink/pull/85) (`ToggleAction`, `EndorsementAction`).
Quyết định CHOICE-10 A và các lựa chọn của lát: [decision record](../architecture/literary-endorsement.md#lát-web-adapter-wp-04-06112026);
bằng chứng: [web evidence](literary-endorsement-web-evidence.md).

| Phần | File | Vai trò |
| --- | --- | --- |
| DTO | [`contract.rs`](../../apps/vink-web/src/contract.rs) | Biểu diễn riêng của app cho 4 route + lỗi (`EndorsementConflict { observed }`, `RevisionChanged`, `SelfEndorsement`, `EndorsementNotPermitted`); test round-trip mọi fixture LVE của provider |
| Client | [`api.rs`](../../apps/vink-web/src/api.rs) | `endorsements_path`/`endorsement_path` mã hóa từng ID; `load_endorsement_summaries`, `load_own_endorsements`, `set_endorsement` (deadline 30 s như discussion); resource SSR `work_endorsement_resource` và `DiscussionLoad` (trang thảo luận + summaries của bình luận công khai, một lượt sau trang) |
| Cổng SSR | [`server.rs`](../../apps/vink-web/src/server.rs) `PublicReads::endorsement_summaries`, [`http/ssr.rs`](../../services/vink-app/src/http/ssr.rs) | Gọi in-process `Vink::endorsement_summaries` (không resolve actor) rồi chuyển qua JSON như API; không đổi READ-01/02 |
| Reducer | [`endorsement_state.rs`](../../apps/vink-web/src/endorsement_state.rs) | Thuần, không Leptos: `(generation/session, TargetKey, order)` → `EndorsementFacts`; sở hữu `command_key` (UUID v4 từ CSPRNG), `expected_order`, attempt, đối soát |
| Adapter Leptos | [`pages/endorsement.rs`](../../apps/vink-web/src/pages/endorsement.rs) | Context theo trang reader, transport gắn owner của reader (sống qua re-render bình luận, hủy khi rời trang), `WorkEndorsement` sau văn bản |
| Màn | [`pages/mod.rs`](../../apps/vink-web/src/pages/mod.rs), [`pages/discussion.rs`](../../apps/vink-web/src/pages/discussion.rs) | Seed số đếm SSR trước khi render; đọc counts + own state cho mỗi trang gốc/phản hồi trình duyệt tải; slot `CommentEndorsements` của `DiscussionThread` (không đổi thứ tự) |

**Luật của reducer** (test `endorsement_state::tests`, tên `lve_web_*`):

- **Order fence:** token (`selected, version, order`) chỉ được nhận khi `order` không nhỏ hơn order đang
  hiện cho (generation, đích). Bằng order là cùng trạng thái đã commit (chỉ làm mới khả dụng/advisory);
  lớn hơn là mới hơn. ACK/đọc trễ, trùng hay đảo thứ tự không kéo màn lùi. ACK của attempt cũ vẫn có
  thể đóng góp token mới hơn nhưng không kết thúc phase của attempt hiện hành.
- **Số đếm** không có order: summary đọc trước khi một lệnh của đích đó settle bị bỏ cho đích đó; lỗi
  đọc giữ số đã biết, số chưa từng biết thành `Unavailable` (“–”), không bao giờ 0. Số của revision khác
  revision đang hiện không được hiện cho nó.
- **Session:** scope = (work, marker host); đổi scope tăng generation và xóa mọi đích, nên lệnh/đọc cũ
  không settle kể cả khi quay lại cùng marker. Khách (không marker) hoặc sau `401` → `SignedOut`, không
  gửi own read hay lệnh.
- **Bấm:** chỉ khi own state đã biết, không chặn, không Pending/Uncertain; đặt `Pending` trước khi trả
  về (yêu cầu của `ToggleAction`), nên chạm đúp chỉ gửi một lệnh.

| Câu trả lời | Facts sau đó | Ghi chú |
| --- | --- | --- |
| `200 accepted` / `200 replayed` đúng đích | Token `endorsement` (hiện tại) + `summary`; `Idle` | `committed` của replay là lịch sử, không hiện |
| `409 version_conflict` + `endorsement` | Token quan sát; `Idle` nếu trùng ý định, `Failed` nếu khác | Không tự gửi lại; bấm tiếp là ý định mới tại order quan sát |
| `409 revision_changed` | Đích revision cũ `Unavailable`, số “–”; yêu cầu READ-02 kiểm tra lại (hiện “Xem bản mới”) | Đọc summary/own nêu revision khác cũng làm vậy |
| `403 self_endorsement` / `endorsement_not_permitted` | Chặn `OwnContent` / `NotPermitted` (lý do bằng chữ) | Own read advisory `own_content`/`not_permitted` chặn trước khi bấm |
| `404` | `Unavailable`, số “–” | Đọc sau có thể khôi phục nếu đích hiện lại |
| `401` | Mọi đích `SignedOut` | Session không được server nhận |
| `422 idempotency_key_reused` / `422` khác / `403 forbidden` | `Failed` (không ghi gì) | Key cũ bỏ; bấm lại sinh key mới |
| Mất kết nối, timeout 30 s, `503`, `500`, thân trả lời không nêu đúng đích | `Uncertain`, chặn bấm; “Kiểm tra lại” replay đúng lệnh | Attempt mới; câu trả lời của attempt cũ không kết thúc nó |
| Own read lỗi tạm thời | Đích chưa từng đọc → `Unknown` + “Kiểm tra lại” (đọc lại); đã biết → giữ | Không bao giờ thành Off |

**SSR và hydration:** SSR luôn ẩn danh (như thảo luận): tổng số công khai của revision đang hiện và của
bình luận công khai trên trang đầu được đọc in-process và serialize vào payload hydration, nên server
và trình duyệt render cùng facts. Với marker LocalFixture, SSR hiện `Unknown` (chưa đọc own state) cho
tới khi trình duyệt đọc `…/endorsements/me` sau hydration; khách thấy lý do “Cần đăng nhập để …”.
Bình luận ẩn/đang chờ/bị từ chối và gốc ẩn (anchor) không có action.

## Chưa có hoặc phụ thuộc lát khác

Auth authority/session (D-05) và cơ chế thu hồi capability, lệnh publish/withdraw thật (bằng chứng
dùng SQL lên con trỏ), cách ly/cấm/purge tài khoản, retention receipt, giới hạn tần suất (LVE-UC-15),
ghi danh (CHOICE-12 B), xếp hạng (CHOICE-06), consumer native, UI gỡ hàng giữ lại
trên đích ẩn (R7, chỉ API), tổng số cho danh sách nhiều tác phẩm, E2E production (cần auth thật).

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
| #73 AC-13 | Tách cache, own state theo actor, guest `401`, lý do vô hiệu; session/generation fence phía client Web ([adapter](#web-adapter-wp-04)) | Abuse/retention theo tài khoản, consent ghi danh |
| #73 AC-11/12 | Reader Web: SSR + hydration, bàn phím, 320/200%, sáng/tối ([web evidence](literary-endorsement-web-evidence.md)) | [Pen Flight R2 evidence](pen-flight-r2-evidence.md); E2E production cần auth thật; native |
| #73 AC-14 | Đối soát, backout, docs, feature map | Rollout thật; epic giữ mở |
