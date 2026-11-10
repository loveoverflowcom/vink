# Outbox và worker của thảo luận — lát DISC-04 (#120)

**Ngày:** 10/11/2026. **Nguồn:** epic [#120](https://github.com/loveoverflowcom/vink/issues/120),
lát DISC-04 do người dùng giao thực thi. **Baseline:** `develop@35fe23d` (sau #126, #127, #144).
Lát này là **hạ tầng bền + một consumer shadow/audit**. Không bật đăng-trước, không đổi D-08, không
có hậu kiểm có hiệu lực, AI provider hay thông báo sản phẩm. Bình luận được lưu vẫn luôn `pending`.

Liên quan: [admission DISC-01](discussion-admission.md), [đọc public/riêng DISC-02/03](discussion-reads.md),
[ownership thảo luận](discussion-ownership.md), [ranh giới Identity/PostgreSQL](identity-postgres-boundaries.md).
Model và replay: [`outbox.qnt`](../../services/vink-app/behavior/discussion/README.md#model-outbox-disc-04-120).

## 1. Đối chiếu tại HEAD thực thi (AC-01)

**GapMap** (capability → owner → contract → implementation → test → gap):

| Capability | Owner tại `35fe23d` | Trước lát này | Lát này |
| --- | --- | --- | --- |
| Đường gửi + admission + provenance | `persistence/discussion.rs::submit_comment`, `domain/discussion/submission.rs` | Có (#127): admission trước receipt, fence đình chỉ, `admission_policy/reason` | Thêm `outbox::enqueue` trong cùng transaction, sau INSERT bình luận |
| Đọc public/overlay riêng | DISC-02/03 phần 2 (#144) | Có; không chạm queue | Không đổi; kiểm không rò dữ liệu queue (T11) |
| Hậu kiểm (`PostReviewTicket`) | `admission.rs` | Contract thuần; ticket chỉ sinh khi admission **approved** | Không dùng: dưới `premoderation.v1` không có ticket hợp lệ nào (§3 CHOICE-03) |
| Worker | `src/bin/vink-worker.rs` | Chỉ prune rate limit mỗi giờ; không job thảo luận | Vòng outbox có giám sát + retention; prune cũ giữ nguyên |
| Outbox/delivery/receipt | — | Không có trong App (Identity có `mail_deliveries` riêng, #143) | Bảng mới của App (§4) |
| Fence đình chỉ tại commit | `persistence/principal.rs` (#143/#127) | Có cho gửi, gỡ, bật/tắt, kiểm duyệt | Worker không viết thay người dùng nên không cần; consumer **đọc** standing hiện tại |

**ProducerConsumerMap / EventInventory:**

| Transition | Event | Consumer | Authority | Payload | Dedup identity | Retention |
| --- | --- | --- | --- | --- | --- | --- |
| `submit_comment` → `Created` | `comment.submitted` schema 1 | `admission_shadow.v1` | Capability dịch vụ của worker App: chỉ đọc `comments`/receipt/đình chỉ, chỉ ghi bảng observation của mình | `comment_id`, `source_version` (= 1), type, schema | `UNIQUE (event_type, comment_id, source_version)`; effect: `PRIMARY KEY (comment_id, candidate_policy)` | Event + delivery: xóa khi mọi delivery `succeeded/skipped` > 30 ngày; dead/đang mở không bao giờ; observation và audit requeue không prune |

Replay đúng lệnh (`Replayed`), từ chối (`KeyReused`, `CommentsClosed`, …) và rollback **không** sinh
event: chỉ nhánh `Insert` đi tới `enqueue`. Gỡ, kiểm duyệt, bật/tắt và rút bài chưa sinh event; không
có consumer hợp lệ nào cần chúng trong lát này.

## 2. Phản biện trước khi triển khai

| Finding | Hậu quả nếu bỏ qua | Quyết định |
| --- | --- | --- |
| Hậu kiểm là consumer "tự nhiên" nhưng ticket chỉ có khi approved | Tạo ticket giả từ bình luận pending hoặc vừa được người duyệt là bịa authority | Không làm consumer hậu kiểm; consumer shadow/audit có output thật (§3) |
| Một `processed_at` cho cả event | Effect B lỗi bị che bởi A thành công | Delivery riêng cho mỗi consumer, trạng thái/attempt/generation riêng |
| Chỉ kiểm token lúc ACK | Worker cũ đã ghi effect trước khi bị từ chối ACK | Fence **trước** effect, dưới khóa hàng, cùng transaction với effect và outcome; SQL lặp lại fence |
| Lease dựa đồng hồ worker | Đồng hồ lệch → hai chủ | Thời gian từ `now()` của PostgreSQL; lease chỉ là timer liveness, generation là fence an toàn |
| Effect lỗi giữa chừng để lại dấu | Retry ghi chồng | Handler chạy trong `SAVEPOINT`; kết quả khác `Applied` rollback về savepoint |
| Commit thành công nhưng mất phản hồi | Worker ghi retry hoặc chạy lại | Lần ghi kế tiếp thấy delivery không còn `leased` → `Stale`, không ghi gì |
| Binary cũ gặp consumer mới (backout) | Dead-letter hàng loạt việc không phải của mình | Claim chỉ lọc consumer binary này có handler; còn lại hiện ở `unhandled` |
| NOTIFY như queue | Mất wakeup = mất việc | Không dùng NOTIFY; polling luôn tìm được backlog |
| Worker chạy bằng role đặc quyền | Một lỗi worker có quyền DDL | Production từ chối role đặc quyền (khi khởi động và khi queue trả lời lần đầu) |

## 3. Lựa chọn (DecisionMap)

| CHOICE (prompt DISC-04) | Chọn | Vì sao / đánh đổi / phương án loại |
| --- | --- | --- |
| CHOICE-01 Event và delivery | **A** — event bất biến + `discussion_deliveries` theo (event, consumer) | Test T6 cần "A xong, B lỗi → chỉ B retry" trên cùng event; B (một hàng/effect lặp metadata) gọn hơn khi chỉ một consumer nhưng phải giữ provenance nhất quán bằng tay. Đổi lại: một join khi claim (0,06 ms, §8) và retention phải chờ mọi delivery |
| CHOICE-02 Đánh thức worker | **A** — polling, backoff 1 s → 30 s có jitter, reset khi có việc | Consumer duy nhất là audit; độ trễ ≤ ~37 s khi nhàn chấp nhận được; idle đo ~13 transaction/120 s. B (LISTEN/NOTIFY) cần connection riêng, đăng ký lại khi reconnect và vẫn phải poll; để DISC-05/hậu kiểm thật quyết khi có SLA. Đây là lựa chọn cho CHOICE-05 của epic (#120), không ghi đè ID lịch sử |
| CHOICE-03 Consumer đầu tiên | **A** — `admission_shadow.v1` (shadow/audit) | Đo điều kiện 7 của delta D-08 ([admission §5](discussion-admission.md#5-delta-com-02d-08-đề-xuất--proposed-chưa-accepted)): ứng viên `trusted-cohort.v1` quyết gì so với quyết định người. Dùng `decide_initial_review` thật; trust/rules/classifier **không có nguồn** nên là `Unknown/Unassessed` (không bịa) → ứng viên chỉ giữ lại hoặc từ chối. Đây là chỗ nguồn chậm (rules, classifier) sẽ cắm vào, ngoài transaction gửi. B (hậu kiểm) loại: không có ticket hợp lệ khi chỉ `premoderation.v1` hiệu lực |
| CHOICE-04 Mức tái sử dụng | **A** — queue nằm cạnh owner thảo luận trong App | Identity giữ `mail_deliveries` (một hàng/challenge, code trong hàng, thư ngoài DB). Yêu cầu khác nhau (fan-out, effect trong DB, requeue có audit). Không có primitive chung, không chia sẻ bảng, credential hay authority. Chỉ học pattern claim `SKIP LOCKED` + lease của #143 |

## 4. Thiết kế

**Lõi thuần** — `services/vink-app/src/domain/discussion/outbox.rs`: `EventKind`/`Consumer` (mã có
version), `DeliveryState` (`ready`, `leased`, `succeeded`, `skipped`, `dead`), `ClaimGeneration`
(không dựng từ số thô), `OutboxPolicy` (mặc định 8 attempt, lease 30 s, backoff 5 s → 10 phút,
deadline 7 ngày), `decide_claim`, `check_ownership`, `decide_completion`, `decide_requeue`,
`observe_shadow`. Không đọc clock/SQL/env; jitter là input và bị chặn ở `[0, base]`.
"Retry scheduled" = `ready` với `available_at` tương lai. Unknown/unsupported không bao giờ thành
`succeeded`: schema lạ → `dead/unsupported_event`, dữ liệu hỏng → `dead/invalid_payload`.

**Schema** — migration `20261114000000_discussion_outbox`: `discussion_events` (chỉ ID, type,
schema, version; không body/author/lý do), `discussion_deliveries` (PK `(event_id, consumer)`,
`attempts`, `generation`, `available_at` = hạn lease khi `leased`, `deadline_from`, `expired_leases`,
mã `outcome/last_error` theo regex, CHECK trạng thái kết thúc ⇔ `finished_at` ⇔ `outcome`),
`discussion_delivery_requeues` (audit), `comment_admission_observations` (receipt của effect).
Index: claim `(available_at) WHERE state IN ('ready','leased')`, dead letter, `events(created_at)`.

```text
Producer (một transaction App)            Worker
──────────────────────────────            ────────────────────────────────────────────────
publication FOR SHARE                     T_claim (ngắn): SELECT … d JOIN e
thread FOR SHARE                            WHERE state IN (ready,leased) AND available_at <= now()
target FOR SHARE                            AND consumer = ANY(handlers) ORDER BY available_at
principal FOR UPDATE (fence đình chỉ)       LIMIT free_slots FOR UPDATE OF d SKIP LOCKED
receipt INSERT                            → decide_claim: generation+1, attempts+1, lease | dead
comments INSERT                           COMMIT  (không giữ transaction trong lúc làm việc)
discussion_events INSERT    ◀── mới
discussion_deliveries INSERT ◀── mới      T_effect: SET LOCAL lock_timeout 2s, statement_timeout 5s
COMMIT                                      delivery FOR UPDATE (PK)  → check_ownership
                                            SAVEPOINT; handler đọc comments/receipt/đình chỉ
                                            (không khóa), INSERT … ON CONFLICT DO NOTHING
                                            → decide_completion → UPDATE … WHERE generation = $g
                                          COMMIT (effect + outcome cùng lúc)
                                          Lỗi transaction → T_fail mới, lại fence → retry/dead
```

**Thứ tự khóa:** worker khóa hàng `discussion_deliveries` trước (claim, effect, requeue; retention qua
`discussion_events` `SKIP LOCKED`); không đường bình luận nào khóa delivery. Handler đọc `comments`,
receipt, đình chỉ **không khóa** và chỉ ghi bảng của mình; FK của observation lấy `FOR KEY SHARE` trên
hàng bình luận được quan sát, nên **một hàng đó** tuần tự hóa ngắn với kiểm duyệt/gỡ đang giữ
`FOR UPDATE` (bị chặn bởi `lock_timeout` 2 s và timeout handler; quá hạn → retry `database`). Không có
chu trình vì các đường đó không bao giờ chờ delivery. Producer chỉ chèn hàng mới, không worker nào đã
khóa được; FK `discussion_events → comments` lấy `FOR KEY SHARE` trên hàng vừa chèn trong cùng
transaction. Test T11 giữ khóa mọi delivery và vẫn đọc/gửi/gỡ < 2 s.

**Worker** — `src/outbox_worker.rs`, chạy trong `vink-worker` (không binary thứ hai): tối đa
`VINK_WORKER_OUTBOX_CONCURRENCY` (1..=6, mặc định 4; pool worker có 10 kết nối và claim/heartbeat/retention cần phần còn lại) delivery cùng lúc; claim đúng số slot trống;
mỗi delivery là một task có timeout 10 s (< lease 30 s). Panic của handler bị giữ trong task và ghi
`retry/panic`; panic hay kết thúc bất thường của chính vòng lặp làm process thoát `1` để service
manager khởi động lại. Tắt: dừng claim, chờ tối đa 10 s, abort phần còn lại (transaction rollback,
lease tự hết). Tín hiệu SIGINT/SIGTERM được lắng nghe trước mọi probe khởi động. Trạng thái khởi
động: `ready` (queue trả lời), `disabled`, `unavailable`, `schema_not_current`,
`refused_privileged_role` (production, thoát `2`). DB sập không làm process chết: vòng lặp backoff và
log `outbox queue reachable` khi phục hồi.

**Consumer `admission_shadow.v1`** — đọc trạng thái hiện tại của bình luận, digest của receipt gửi và
standing hiện tại của người viết; gọi `observe_shadow`: thiếu bình luận → `skipped/target_missing`;
đã gỡ → `skipped/target_removed`; còn lại ghi một observation (outcome, reason, `observed_version`,
`observed_review`). Không bao giờ ghi `comments`; quyết định người mới hơn được **quan sát**, không bị
ghi đè; bị gỡ không hồi sinh. **Đây là shadow/audit, chưa phải hậu kiểm production.** Giới hạn:
facts lấy tại thời điểm đánh giá, không phải lúc gửi (authority trust tương lai phải cung cấp trust
*tại thời điểm gửi* để shadow trung thực); hôm nay mọi observation là `pending/trust_unknown` hoặc
`refused/suspended`.

## 5. Lỗi, retry, dead-letter, requeue

| Lớp lỗi | Phân loại | Kết quả |
| --- | --- | --- |
| DB lỗi/transaction hỏng trong effect | `Transient::Database` | Retry có backoff + jitter; hết 8 attempt hoặc quá deadline → `dead/retry_exhausted` hoặc `deadline_exceeded` |
| Handler quá 10 s | `Transient::Timeout` | Transaction bị drop → rollback; ghi retry chờ khóa tối đa 6 s (> `statement_timeout` 5 s của lần bị bỏ). Không ghi được → lease 30 s hết hạn, attempt vẫn đã đếm |
| Panic | `Transient::Panic` | Như trên (vòng có giám sát ghi; `run_once` và task bị abort khi drain quá hạn để lease hết hạn) |
| Crash/kill/mất lease | Attempt đã đếm lúc claim | Reclaim sau lease, `expired_leases + 1`, `last_error = lease_expired` |
| Schema/type lạ | `DeadReason::UnsupportedEvent` | `dead`, operator requeue sau nâng cấp |
| Dữ liệu sai (state, digest) | `DeadReason::InvalidPayload` | `dead` |
| Target gỡ/thiếu | `SkipReason` | `skipped`, không phải lỗi, không phải thành công |
| Consumer không có handler | Không claim | `unhandled` trong status; chờ binary sở hữu nó |
| Kết quả mơ hồ (mất phản hồi commit) | — | Lần ghi kế tiếp `Stale`, không ghi |

**Requeue** chỉ cho `dead` (`vink-app outbox requeue <event> <consumer> <ghi chú>`): `ready` ngay,
attempts = 0, deadline mới, **generation giữ nguyên**, ghi audit cùng transaction. `succeeded`/`skipped`
bị từ chối (effect đã xong không chạy lại), `ready`/`leased` bị từ chối (đang có chủ). Không route HTTP
nào requeue được.

**Fail-closed:** consumer shadow không có nghĩa vụ fail-closed (không đổi hiển thị). Khi hậu kiểm có
hiệu lực được thêm, dead letter của nó **không** được coi là đã hoàn thành MOD-03: hết retry phải
thành `Unresolved → hold` tại version của ticket, và nếu không ghi được hold thì delivery phải còn mở
(không ACK). Lõi hiện chưa có kiểu "exhaustion = hold"; đây là việc của lát hậu kiểm.

**Kill switch/rollout:** `VINK_WORKER_OUTBOX=disabled` chỉ tạm dừng giao; App vẫn ghi event, không gì
bị xóa. Không có policy mới nào được bật; tắt consumer không hồi tố bình luận đã duyệt.

## 6. Quyền, riêng tư, vận hành

- Worker dùng role runtime (SELECT/INSERT/UPDATE/DELETE theo `services/postgres/provision`), không
  DDL; test T11 chạy outbox bằng role chỉ có các grant đó. Production kiểm role **trước** lần claim đầu
  và fail closed (không biết → không claim, backoff; đặc quyền → dừng, process thoát `1`).
- Event bất biến (trigger từ chối UPDATE; retention vẫn DELETE được), audit requeue append-only (trigger
  từ chối UPDATE/DELETE), kể cả với credential runtime. `database_role` của audit là credential vận
  hành dùng chung; ghi chú requeue phải nêu người làm và lý do.
- Không đọc DB Identity, không dùng phiên người gửi; đăng xuất không hủy nghĩa vụ đã commit.
- Event/delivery/observation không chứa body, email, phiên hay lý do kiểm duyệt; không route nào đọc
  chúng; log chỉ có mã và số đếm (test process kiểm không có body hay comment ID trong log).
- Đọc public/overlay/SSR không phụ thuộc queue (T11).

**Quan sát:** log `outbox heartbeat` mỗi 60 s: `eligible`, `scheduled`, `leased`, `dead`, `unhandled`,
`oldest_eligible_seconds`, `oldest_open_seconds` và bộ đếm `claimed`, `reclaimed`, `succeeded`,
`skipped`, `retried`, `dead_lettered`, `stale_refused`, `timeouts`, `panics`, `database_errors`.
Không có endpoint metrics (worker không có HTTP); không label theo ID.

### Runbook

| Việc | Lệnh / cách làm |
| --- | --- |
| Xem sức khỏe | `vink-app outbox status` (tab: số và tuổi), log heartbeat |
| Xem dead letter | `vink-app outbox dead [<n>]` (event, consumer, type/schema, mã, attempts, thời điểm) |
| Requeue | Sửa nguyên nhân (deploy binary hỗ trợ schema, sửa dữ liệu) rồi `vink-app outbox requeue <event-uuid> <consumer> <ghi chú>` |
| Tạm dừng | `VINK_WORKER_OUTBOX=disabled` rồi restart worker; event tiếp tục tích lũy |
| Drain/restart | SIGTERM: dừng claim, chờ ≤ 10 s; việc dở được reclaim sau lease 30 s |
| Retention | `vink-worker` mỗi giờ, hoặc `vink-app maintenance prune` (lô 1.000) |
| Cảnh báo đề xuất | `oldest_eligible_seconds` > 600, `dead` > 0, `unhandled` > 0 sau deploy, `database_errors` tăng liên tục |

**Rollout/backout:** migration chỉ thêm bảng (không khóa bảng cũ). Thứ tự: migrate → App mới → worker
mới. Binary App mới trên schema cũ **fail closed** (gửi bình luận lỗi và rollback toàn bộ; test T12),
nên migrate trước. **Chỉ rollback binary là không đủ:** binary cũ thấy migration lạ nên báo schema
`diverged` và không ready (phát hiện ở review, §9). Backout: dừng App mới (không còn event mới) → để worker mới drain
(hoặc requeue/hoàn tất dead letter) → dừng worker → chạy
[`20261114000000_discussion_outbox.down.sql`](../../services/vink-app/migrations-rollback/20261114000000_discussion_outbox.down.sql)
→ chạy binary cũ. Script khóa `ACCESS EXCLUSIVE` cả hai bảng queue rồi **từ chối** khi còn delivery
`ready`, `leased` hoặc `dead` (không bỏ việc đang nợ, kể cả việc một App mới vừa ghi); khi đã drain, nó
xóa event/delivery đã xong, audit requeue và observation shadow (dữ liệu đo, không phải quyết định
kiểm duyệt); bình luận, receipt và kiểm duyệt giữ nguyên, roll-forward áp lại được. Bình luận trước
migration không có event (không backfill), vẫn đọc/kiểm duyệt bình thường.

## 7. Bằng chứng

Môi trường: macOS 26 (Darwin 25.5), Apple M4 10 nhân, 16 GB; PostgreSQL 17.11 Homebrew cục bộ
(`shared_buffers` 128 MB); Rust theo `rust-toolchain`; Quint 0.33.1; Kani 0.67.0.

| Kịch bản | Test (`tests/postgres/outbox.rs` trừ khi ghi khác) | Kết quả |
| --- | --- | --- |
| T1 commit/rollback/enqueue lỗi | `a_committed_comment_owes_its_consumers_and_a_rolled_back_one_owes_nothing` (trigger fixture làm INSERT delivery lỗi → không bình luận/receipt/event; lệnh lặp lại sau đó tạo đúng một) | PASS |
| T2 replay/đồng thời | `replays_and_concurrent_submissions_never_duplicate_a_logical_event` (8 retry cùng key, 8 lệnh khác key) | PASS |
| T3 tranh claim/crash | `competing_workers_claim_disjoint_work_and_a_crash_is_reclaimed` (`SKIP LOCKED` không chờ, hai claim rời nhau, không `idle in transaction`, reclaim) | PASS |
| T4 worker mất claim | `a_worker_that_lost_its_claim_writes_neither_effect_nor_outcome` (complete/retry×3/dead-letter của A đều `Stale`, handler không chạy, không observation) | PASS |
| T5 crash quanh effect, mất phản hồi | `crashes_around_the_effect_and_lost_commit_responses_never_duplicate_it` | PASS |
| T6 hai effect | `one_effect_failing_retries_only_that_effect` (consumer thứ hai là **fixture**, không phải sản phẩm) | PASS |
| T7 poison/hết retry/deadline/requeue/cleanup/consumer lạ | `poison_exhaustion_and_deadlines_end_in_dead_letters_that_an_operator_requeues` | PASS |
| T8 quyết định người mới hơn, gỡ, rút bài, đình chỉ | `old_events_never_overwrite_newer_human_decisions_or_revive_removed_comments`, `a_suspended_writer_is_observed_as_refused_without_touching_the_comment`, `a_moderation_in_flight_delays_the_observation_without_deadlock_or_overwrite` (moderator giữ hàng `FOR UPDATE` → worker chờ ≤ `lock_timeout`, retry `database`, sau commit quan sát version 2) | PASS |
| T9 backlog lúc khởi động, mất kết nối, restart | `the_worker_process_drains_backlog_survives_outages_kills_and_restarts` (binary `vink-worker` thật; `pg_terminate_backend` mọi kết nối của worker); `…reports_unavailable_queues…` | PASS; NOTIFY không dùng |
| T10 shutdown/kill/panic/timeout | `timeouts_panics_and_shutdown_never_acknowledge_unfinished_work`; SIGKILL + restart trong test process | PASS. Giới hạn: SIGKILL trong test process xảy ra khi worker đã nhàn, không chính xác giữa một effect; cửa sổ đó được phủ ở mức transaction bởi T5 và test drain/abort |
| T11 quyền, redaction, không chặn đọc | `a_stuck_worker_never_blocks_public_reads_or_new_comments`, `the_restricted_runtime_role_is_enough_to_run_the_outbox`, production + superuser → thoát 2 | PASS |
| T12 nâng cấp/backout | `upgrading_keeps_old_comments_and_new_ones_start_owing_work` (bình luận trước migration, binary mới trên schema cũ fail closed, schema event mới → dead → requeue); `the_schema_rollback_refuses_while_work_is_owed_and_keeps_comments` (down script từ chối khi còn nợ, chạy sau drain, roll-forward lại); chuỗi rollback của `abuse::prune_and_rollback…` | PASS. Backout bằng binary cũ thật: NOT_RUN; quy trình §6 đòi chạy script rollback vì binary cũ thấy schema `diverged` |

**Negative control** (đã khôi phục byte-exact sau mỗi lần):

| Đột biến | Kết quả |
| --- | --- |
| Bỏ so sánh generation ở `check_ownership` **và** ở `WHERE` của SQL | T4 FAIL: `left: Succeed, right: Stale(Reclaimed)` — worker cũ ghi effect và ACK lên claim mới |
| Chỉ bỏ so sánh ở lõi, giữ SQL | T4 FAIL bằng `Integrity("fenced delivery update: 0")`: lớp SQL từ chối và transaction rollback effect |
| Bỏ PK observation + `ON CONFLICT` | T5 FAIL: lần áp lại trả `Applied` thay vì `AlreadyApplied` (effect trùng) |
| Bỏ so sánh generation ở lõi (Kani/replay) | Kani `outbox_completion_is_fenced…` FAIL `assertion failed: owner`; replay Quint lệch ở bước 8 (`succeeded` thay vì `stale`) |
| Quint `mutatedUnfencedStep` | Vi phạm `effectAtMostOnce` ở transition 6; counterexample replay qua production bị từ chối (`stale`) |

**Model và proof:** `outbox.qnt` — 2 worker, 1 event × 2 delivery, lease 2 tick, backoff 1→2 tick,
3 attempt, clock ≤ 8; simulation seed 41, 1.000 × 40 bước; scenario 26 bước đạt đủ 12 outcome
(claim, reclaim, stale, retry, dead tại claim và tại completion, requeue/từ chối, crash). Giả định:
mỗi action là một commit tuần tự hóa; deadline không đạt trong model (Kani/unit phủ). Kani: hai harness
mới trên mọi trạng thái, `u32` attempts, cặp generation `i64`, instant và jitter `i64` (oracle tuổi
`i128`, có `cover!` cho stale và retry). Kani/Quint **không** chứng minh SQL, mạng hay vòng đời process.

## 8. Số đo (baseline an toàn, không phải SLA)

Dataset: 100.000 bình luận/event; delivery 98.000 `succeeded`, 1.000 `dead`, 900 `ready` đến hạn, 100
`leased`. Build release, PostgreSQL 17.11 cục bộ (§7).

| Truy vấn | `EXPLAIN (ANALYZE, BUFFERS)` |
| --- | --- |
| Claim `LIMIT 4 … SKIP LOCKED` | Index `discussion_deliveries_claimable_idx` + PK event; 0,06 ms, 27 buffer hit |
| Claim khi không có việc | 0,04 ms, 20 buffer |
| Khóa fenced theo PK | 0,07 ms |
| Đọc của handler shadow | 0,02 ms |
| Backlog (heartbeat, 60 s/lần) | Seq scan 100k hàng, 6,8 ms — chấp nhận ở tần suất này; index nếu bảng lớn hơn nhiều |
| Retention lô 1.000 (mỗi giờ) | 33 ms; hash anti-join quét deliveries |

- **Chi phí producer** (pgbench, 4 client, 15 s, loopback): chỉ INSERT bình luận 0,17–0,20 ms/tx; thêm
  event + delivery 0,33–0,63 ms/tx (bảng tăng tới ~1,4 triệu hàng giữa các lần chạy làm dao động).
  Tức thêm ~0,15–0,45 ms và hai round trip cho mỗi lệnh gửi; lệnh gửi thật có thêm ~8 câu lệnh khác.
- **Thông lượng worker:** 2.000 delivery xử lý trong 2,28 s (concurrency 4, ~880/s), mỗi bình luận
  đúng một observation.
- **Idle:** ~13 transaction trong 120 s khi nhàn (claim mỗi ≤ ~37 s + heartbeat).
- Lock wait: không đo được chờ trong T11 (mọi thao tác < 2 s khi worker giữ khóa delivery).

## 9. Review đối kháng

Tự review và một lượt review **độc lập** (subagent chỉ đọc mã, không nhận kết luận của tác giả) trên
diff. Không có lỗi nào ở fence, tính nguyên tử của producer, savepoint hay xử lý mất phản hồi commit.
Các phát hiện và cách xử lý:

| Phát hiện | Mức | Xử lý |
| --- | --- | --- |
| Tự review: rollback ngầm khi drop transaction sau lỗi handler | Thấp | `tx.rollback()` tường minh trước khi ghi lỗi |
| Tự review: tài liệu nói worker không bao giờ chờ khóa bình luận; thực ra FK observation lấy `FOR KEY SHARE` | Thấp | Sửa tài liệu §4; test `a_moderation_in_flight…` |
| Tự review: `shutdown.changed()` lỗi khi sender bị drop → vòng quay nóng | Thấp | Coi sender đóng là dừng |
| Độc lập: backout chỉ bằng binary làm App cũ không ready (`diverged`) | Trung bình | Sửa runbook §6, migration và script rollback |
| Độc lập: concurrency 32 với pool 10 → hết thời gian chờ kết nối, mất attempt | Trung bình–thấp | Trần 6 (`MAX_CONCURRENCY`) |
| Độc lập: kiểm role sau claim và fail open khi lỗi | Thấp | Kiểm trước claim, fail closed |
| Độc lập: script rollback có race giữa kiểm và `DROP` | Thấp | `LOCK TABLE … ACCESS EXCLUSIVE` trước khi kiểm |
| Độc lập: "bất biến/append-only" chỉ là lời | Thấp | Trigger cho event và audit requeue; test trong T7 |
| Độc lập: sau timeout, ghi retry thường không lấy được khóa trong 2 s | Thấp (liveness) | Ghi lỗi chờ khóa 6 s; tài liệu §5 nói rõ trường hợp còn lại rơi về lease |

## 10. Trạng thái AC và phần còn lại

| AC | Trạng thái |
| --- | --- |
| AC-01 | Đạt: §1; không trùng queue Identity hay phần DISC-02/03 |
| AC-02 | Đạt: T1, T2 |
| AC-03 | Đạt: T4 + negative control hai lớp |
| AC-04 | Đạt trong biên DB của App: T3, T5, T6, T7; không tuyên bố exactly-once ngoài transaction |
| AC-05 | Đạt cho **shadow/audit**; hậu kiểm có hiệu lực chưa có (không có ticket hợp lệ) |
| AC-06 | Đạt: T9, T10, runbook; giới hạn SIGKILL ghi ở §7 |
| AC-07 | Đạt: T8, T11; D-08 không đổi, không đăng-trước |
| AC-08 | Đạt với các NOT_RUN đã ghi (backout bằng binary cũ thật, Apalache); runbook backout đã sửa sau review (§9) |

**Tiếp theo:** hậu kiểm có hiệu lực (consumer mới với exhaustion → hold và không ACK khi chưa ghi được
hold) chỉ sau delta D-08; authority trust có lịch sử theo thời điểm; event cho gỡ/kiểm duyệt khi có
consumer cần. **DISC-05:** counter/cache/projection, LISTEN/NOTIFY nếu có SLA độ trễ, index backlog
khi bảng lớn, metrics endpoint.
