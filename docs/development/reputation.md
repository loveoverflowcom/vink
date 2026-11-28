# Điểm Vink (Reputation V1) — registry triển khai và bàn giao

Epic [#600](https://github.com/loveoverflowcom/vink/issues/600). Yêu cầu: [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1)
REP-01..14. Đặc tả: [miền](../architecture/reputation.md), [SR-01](reputation-science.md),
[API dự thảo](../../services/vink-app/contracts/http/v1/reputation.md).

## 1. Bốn chiều trạng thái

| Chiều | Trạng thái hiện tại |
| --- | --- |
| Sản phẩm | [VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml) Q01..Q44 `PRODUCT_CONFIRMED` — [signoff](../../decisions/signoffs/VDR-REPUTATION.r1.toml) ngày 28/11/2026, SHA-256 `2c009b7ca218269251edc6bf363914e5c1d4123cbfd49c18f4820231ba0cc3ee`. |
| Kiến trúc | [VDR-REPUTATION-CONTRACT@r1](../../decisions/proposals/VDR-REPUTATION-CONTRACT.r1.toml) K1..K9 `Provisional` — [signoff](../../decisions/signoffs/VDR-REPUTATION-CONTRACT.r1.toml), SHA-256 `7d60f8ae7ed146e033ce8ed352feaf6ac1798fd041693692500a2277231b0963`. Mỗi lát chứng minh lựa chọn của mình; chưa `Accepted`. |
| Triển khai | `core` từ #644: `services/vink-app/src/domain/reputation` (công thức fixed-point, entitlement/anchor, máy Q41, lịch, maturity, gate bảng). Outbox nguồn (#645): migration `20261208000000_reputation_outbox`, producer Thấm/Ưng, trigger marker, `vink-app reputation status|activate|deactivate`. Projection (#646): consumer `vink-worker`, entitlement/ledger/profile, maturity, `vink-app reputation backfill|reconcile|correct`. Thống kê kỳ, route: `NOT_IMPLEMENTED`. |
| Bằng chứng | SR-01 tổng hợp; unit + golden + Quint replay + Kani cho pure core (mục 4). PostgreSQL, HTTP, benchmark: `NOT_RUN`. |

<!-- vdr: active VDR-REPUTATION@r1 -->
<!-- vdr: active VDR-REPUTATION-CONTRACT@r1 -->

Không công khai runtime trước bằng chứng privacy/performance của S06 ([#648](https://github.com/loveoverflowcom/vink/issues/648)).

## 2. Audit nguồn — develop@b69f34bc (28/11/2026)

| Nguồn | Hiện trạng | Hệ quả cho Reputation |
| --- | --- | --- |
| Ghi Thấm/Ưng — `persistence/endorsement.rs` `set_endorsement` | Một transaction: publication `FOR SHARE` → thread → comment → principal (`require_active`) → advisory lock theo command key → dòng endorsement `FOR UPDATE` → activation Notifications. Ghi `work_endorsements`/`comment_endorsements` (`selected`, `version`, `accepted_order`, `updated_at = now()`) và `endorsement_receipts` (`created_at = now()`). | S03 ghi outbox ở cuối transaction này khi `version` đổi; thời điểm accepted là `now()` của transaction, trùng receipt. Không đổi thứ tự khóa. |
| Receipt endorsement | Append-only, khóa `(actor_id, command_key)`, có `committed_version/order`; no-op cũng có receipt (order tăng, version giữ). Không có sequence toàn cục. | Không dùng receipt làm cursor. Q32: chỉ version đổi là chuyển trạng thái; receipt là bằng chứng backfill. |
| DEL-05 — `persistence/account_deletion.rs` `purge_principal` | `UPDATE` trực tiếp `selected = false`, `version + 1`, `accepted_order + 1`, `updated_at = clock_timestamp()` cho mọi Thấm/Ưng của actor; **không ghi receipt**; phát retraction Notifications. | Sự kiện trung tính terminal riêng, ghi cùng transaction purge; không suy ra từ receipt. |
| Publication — `publications`, `work_publications` | Con trỏ revision công khai hiện hành; `audience`, `lifecycle` (`active`/`author_deleted`/`removed`), `hidden_revision_id`, `generation`. Thấm chỉ ghi được trên revision hiện hành công khai. | Event trung tính khi con trỏ, audience, lifecycle hoặc hidden revision đổi. Revision bị thay thế vẫn là anchor (Q19). |
| Nhận xét — `comments` | `review` (`pending`/`approved`/`rejected`) và `removal` (`author`/`moderation`, terminal). Ưng chỉ nhận được khi tác phẩm công khai và nhận xét đã duyệt, còn hiện diện. | Event trung tính khi `review` hoặc `removal` đổi, và khi tác phẩm đổi trạng thái công khai. |
| Standing — `principal_suspensions`, `principal::require_active` | Một dòng active = đình chỉ toàn bộ; mọi request mất authority. Không có loại hạn chế một phần. | Người nhận bị đình chỉ: ẩn khỏi đầu ra công khai (Q08). Actor bị đình chỉ: tương tác cũ giữ nguyên (Q14). |
| Tuổi principal | `principals.created_at timestamptz NOT NULL DEFAULT now()`, ghi khi tạo principal (`persistence/principal.rs`, `dev_seed.rs`). | Nguồn Q30 có sẵn trong App, không cần Identity. Principal cũ có `created_at` lúc tạo dòng. |
| Xóa người nhận — `account_deletions` | Fence vĩnh viễn theo `principal_id`, purge hồ sơ. | Purge projection/index/cache cá nhân, loại khỏi mọi kỳ (Q21). |
| Notifications — `notification_events` | Producer Thấm (#346) và Ưng (#365) qua activation gate, retention và relay sang service riêng. | Không phải authority: có thể tắt, không phủ lifecycle, retention khác. |
| Outbox thảo luận — `discussion_events/deliveries` | Mẫu claim/lease/retry trong `vink-worker`. | Tham khảo mẫu claim; không dùng chung bảng. |
| Worker — `bin/vink-worker.rs` | Các loop được giám sát và pass dọn dẹp mỗi giờ. | S04/S05 thêm consumer, maturity, snapshot/expiry vào cùng tiến trình. |
| Đọc hồ sơ — `http/profile.rs` | Hồ sơ tác giả công khai. | S06 chọn gắn điểm vào hồ sơ hay route riêng, theo caller thật. |

## 3. Lát triển khai

| Lát | Issue | Phạm vi | Gate |
| --- | --- | --- | --- |
| REP-S01 | [PR #601](https://github.com/loveoverflowcom/vink/pull/601) | SSOT, SR-01, audit nguồn, hai proposal + signoff | Xong khi #601 merge |
| REP-S02 | [#644](https://github.com/loveoverflowcom/vink/issues/644) | `domain::reputation`: công thức fixed-point, entitlement/anchor, eligibility, máy Q41, lịch, golden vectors, Quint + replay, Kani | `core`; không I/O |
| REP-S03 | [#645](https://github.com/loveoverflowcom/vink/issues/645) | Migration, outbox Reputation ghi cùng transaction ở mọi nguồn trong mục 2, công tắc activation | `persistence-api` (outbox) |
| REP-S04 | [#646](https://github.com/loveoverflowcom/vink/issues/646) | Consumer `vink-worker`, projection hiện hành, maturity Q42, backfill Q33, correction CLI Q38, rebuild/reconcile | `worker-provider` (projection) |
| REP-S05 | [#647](https://github.com/loveoverflowcom/vink/issues/647) | Credit tuần/tháng, Top Rating/Top Growth, snapshot/dự phòng/rebuild, retention/expiry | S04 merged |
| REP-S06 | [#648](https://github.com/loveoverflowcom/vink/issues/648) | HTTP đọc, privacy/cache/cursor, qualification PG + HTTP, benchmark, runbook | S05 merged; không public trước gate |

Phụ thuộc tuần tự S01 → S02 → S03 → S04 → S05 → S06. Design, Leptos Web và CMP thuộc epic khác.

## 4. Bằng chứng S01

| Lệnh / kịch bản | Kết quả | Boundary |
| --- | --- | --- |
| `cargo xtask decision validate` | PASS (ghi SHA nguồn trong PR) | Registry |
| `cargo xtask decision propose VDR-REPUTATION new …`, `… VDR-REPUTATION-CONTRACT new …` | SHA-256 `2c009b7c…`, `7d60f8ae…` | Registry |
| `cargo xtask decision signoff VDR-REPUTATION@r1 …`, `… VDR-REPUTATION-CONTRACT@r1 …` | `PRODUCT_CONFIRMED`, `PROVISIONAL`; `current: true` | Registry |
| `python3 -I services/vink-app/bench/reputation-sr01/qualify.py` (hai lần) | Cùng output; 0 sai lệch f64 trên 10^6 cặp; khoảng cách micro nhỏ nhất 4,8 × 10^-7 | Tổng hợp, không phải code Rust |

Không chạy Quint, Kani, PostgreSQL, HTTP hay benchmark trong S01; chúng thuộc S02..S06.

## 4a. Pure core S02 (#644)

| Owner | Quy tắc |
| --- | --- |
| `domain/reputation/formula.rs` | `score_v1`: ln nguyên fixed-point 96 bit (atanh), trường hợp R nguyên tính chính xác; `ComponentCount` [0, 2^40), `ScoreMicros::display`, `GrowthTenths::of` checked. |
| `domain/reputation/calendar.rs` | Tuần thứ Hai / tháng lịch UTC+07:00, [start, end), `first_full_period` (Q33), `maturity_threshold` (Q30). |
| `domain/reputation/credit.rs` | Máy Q41 `Unstarted/Unseen/Active/WithdrawnPending(p)/Burned`, khởi động lười tại kỳ đầy đủ đầu tiên (baseline), kẹp kỳ đơn điệu. |
| `domain/reputation/entitlement.rs` | `apply(record, cause, facts, periods)`: so effective đã lưu với facts đọc lại; Community/Neutral/Terminal/Maturity → delta hiện hành và hai bước credit. Caller áp maturity trước sự kiện sau ngưỡng của cùng actor. |
| `domain/reputation/board.rs` | `rating_admits`, `qualify_growth` (≥ 3 actor net dương, gom theo actor), `RatingKey`/`GrowthKey`. |

| Lệnh | Kết quả | Boundary |
| --- | --- | --- |
| `cargo test -p vink-app --lib -- domain::reputation` | PASS 23 test, gồm 2 063 golden vectors | unit/golden |
| `cargo xtask verify discussion` | PASS; model `behavior/reputation/reputation.qnt` typecheck + 1 000 mẫu × 40 bước, 5 kịch bản, 2 negative control, replay Rust | model + replay |
| `cargo kani --harness domain::reputation::proofs::*` | 3/3 SUCCESSFUL | kani |
| `cargo clippy -p vink-app --lib --tests -- -D warnings` | PASS | lint |
| `cargo xtask verify quick`, `cargo xtask verify kani` (toàn lane) | NOT_RUN: dừng theo yêu cầu chủ sản phẩm, chờ verify lại | — |

Bộ sinh golden phải xử lý chính xác lũy thừa của 11: Decimal 60 chữ số cho f(1 610 500) = 4,999…9 và làm
`floor` hụt một micro; đường Rust và `qualify.py` đều tính các điểm này bằng số nguyên.

## 4b. Outbox nguồn S03 (#645)

| Nguồn | Cách ghi | Nội dung |
| --- | --- | --- |
| Thấm/Ưng — `persistence/endorsement.rs` | Rust, cuối transaction, chỉ khi version đổi; khóa cuối là `reputation_activation` FOR KEY SHARE | `tham`/`ung`: actor, người nhận, anchor, selected/version/order, `accepted_at = now()` (trùng receipt), ngữ cảnh as-of: work public, hidden revision, comment public |
| `publications` (INSERT/UPDATE/DELETE), `work_publications` (audience, lifecycle, pointer, hidden) | Trigger AFTER ROW | `work`: work public, hidden revision as-of |
| `comments` (review/removal) | Trigger AFTER ROW | `comment`: approved và còn hiện diện |
| `principal_suspensions`, `account_deletions` | Trigger AFTER ROW | `principal`: đình chỉ toàn bộ, đã xóa (DEL-05 terminal, purge người nhận) |

Trigger chạy sau khi câu lệnh đã khóa dòng, nên marker được đánh số sau mọi giao dịch Thấm/Ưng đang giữ dòng
đó FOR SHARE: với giao dịch xung đột, thứ tự `id` là thứ tự commit. Purge DEL-05 không phát event cho từng
endorsement; marker `principal_deleted` đến sau các endorsement đang chạy của actor vì xóa tài khoản khóa dòng
principal trước. Activation tắt → không ghi gì; bật/tắt có audit trong `reputation_activation_changes`.
Rollback: `migrations-rollback/20261208000000_reputation_outbox.down.sql` từ chối khi đang bật hoặc còn event.

| Lệnh | Kết quả | Boundary |
| --- | --- | --- |
| `cargo check -p vink-app --tests`, `cargo clippy -p vink-app --lib --bins --tests -- -D warnings` | PASS | compile/lint |
| `cargo xtask verify postgres` (gồm `tests/postgres/reputation_outbox.rs`, 7 test), `verify quick` | NOT_RUN: dừng theo yêu cầu chủ sản phẩm, chờ verify lại | PostgreSQL |
| Đo overhead ghi Thấm trước/sau (AC-07 #645) | NOT_RUN | scratch DB |

## 4c. Projection S04 (#646)

Migration `20261209000000_reputation_projection`: `reputation_readiness`, facts as-of (`reputation_work_facts`,
`reputation_comment_facts`, `reputation_principal_facts`), `reputation_anchors` (Thấm theo revision, có fence
order), `reputation_entitlements` (hai máy Q41), `reputation_profiles` (I, C, `score_micros`), `reputation_ledger`
(net mỗi entitlement mỗi kỳ ∈ {−1, 0, 1}) và `reputation_corrections` (append-only).

| Owner | Hành vi |
| --- | --- |
| `persistence/reputation_projection.rs` `consume_batch` | Khóa advisory một batch, event theo `id`, áp `entitlement::apply`, xóa event trong cùng transaction; maturity trước sự kiện sau ngưỡng của actor và quét actor bị giữ. |
| `backfill` | REPEATABLE READ khi producer đang bật: facts, anchor, entitlement, profile; xóa event mà snapshot đã phản ánh; đặt `ready_at` và kỳ đầy đủ đầu tiên. Từ chối khi chưa bật hoặc đã sẵn sàng. |
| `reconcile` | Đếm lại I/C từ entitlement hiệu lực, báo lệch, `fix` ghi lại. |
| `correct` | Phạm vi `impact|contribution:<actor>:<target>` hoặc `pair:<actor>:<recipient>` tại lúc apply; retire (Terminal), xóa ledger của entitlement (viết lại lịch sử), audit, idempotent theo key; số nguồn không đổi. |
| `reputation_worker.rs` | Vòng lặp trong `vink-worker`, `VINK_WORKER_REPUTATION=disabled` để dừng. |

Thứ tự vận hành: `vink-app reputation activate <note>` → `vink-app reputation backfill dry-run` → `… apply`
→ worker tự áp event; `vink-app reputation reconcile check` định kỳ. Tắt khẩn: `deactivate` (producer) và
`VINK_WORKER_REPUTATION=disabled` (consumer). Runbook đầy đủ thuộc #648.

Giới hạn đã biết: maturity áp theo trạng thái đã xử lý tới batch chứa ngưỡng, nên lệch tối đa một batch so với
as-of chính xác; dựng lại toàn bộ từ nguồn (giữ lịch sử credit) chưa có — `reconcile` chỉ đếm lại profile.

| Lệnh | Kết quả | Boundary |
| --- | --- | --- |
| `cargo clippy -p vink-app --lib --bins --tests -- -D warnings`, `cargo check -p vink-app --tests` | PASS | compile/lint |
| `cargo xtask verify postgres` (gồm `tests/postgres/reputation_projection.rs`, 6 test), `verify quick` | NOT_RUN: dừng theo yêu cầu chủ sản phẩm, chờ verify lại | PostgreSQL |

## 5. Acceptance của epic theo lát

| AC (#600) | Lát chịu trách nhiệm |
| --- | --- |
| AC-01 Shape/SSOT | S01 |
| AC-02 Pure Domain, AC-12 máy credit | S02 |
| AC-03 Nguồn, AC-04 PostgreSQL | S03 |
| AC-05 Worker/recovery, AC-13 correction | S04 |
| AC-11 kỳ, AC-14 top/snapshot | S05 |
| AC-06 Privacy, AC-07 API, AC-08 cô lập, AC-09 hiệu năng, AC-10 bàn giao | S06 (AC-06/08 kiểm thêm ở mỗi lát chạm nguồn) |

## 6. Còn mở

- Kỹ thuật, chọn theo bằng chứng ở từng lát: tên bảng/cột, lease outbox, checkpoint, nhịp snapshot và refill,
  index/cursor, số đo WAL/khóa/độ trễ (mục tiêu nghiên cứu p95 ghi nguồn tăng ≤ 5%, độ trễ worker < 2 phút,
  không phải SLA).
- Sản phẩm: sửa nhận xét hoãn tới COM-10/LVE-FR-04. Ngưỡng 3 actor và 7 ngày là heuristic.
