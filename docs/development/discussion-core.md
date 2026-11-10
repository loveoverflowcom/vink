# Bình luận hai tầng — core, application, PostgreSQL và HTTP (D2T-01, D2T-02a/b)

**Owner:** Vink, module `discussion` trong `vink-app` — quyết định của chủ sản phẩm ngày
03/11/2026 ([ADR ownership](../architecture/discussion-ownership.md)). Discourse không chạy trong
hệ thống. **Lát cắt 1** (#49, nền `develop@8045d45`) thêm core cấu trúc và replay Quint.
**Lát D2T-02a** (nền `develop@31a62b1`) thêm domain vòng đời, use case application, migration
và adapter PostgreSQL. **D2T-02b** thêm HTTP API v1 + contract fixtures
([contract](../../services/vink-app/contracts/http/v1/README.md#discussion-d2t-02b)): đọc, gửi, gỡ
của chính mình, bật/tắt; không có route kiểm duyệt. Production chưa có auth authority nên lệnh
ghi trả `401`. **D2T-02c** thêm authority kiểm duyệt: grant trong database do lệnh vận hành
`vink-app moderator` cấp/thu hồi, kiểm lại trong transaction của từng quyết định, tự né bình luận
của mình và tác phẩm của mình, lý do bắt buộc khi từ chối/gỡ (người viết thấy lý do từ chối),
hàng đợi + route kiểm duyệt. D2T-03 bổ sung UI đọc/giao tiếp ghi tại Web reader; production auth và UI kiểm duyệt chưa có.
**Lát hoàn thiện core #41** (nền `develop@33688b1`) thêm quyết định thuần cho reaction
(D2T-INV-08), model Quint `reaction.qnt`, receipt/retry trong `lifecycle.qnt` (D2T-INV-06) và
Kani cho cấu trúc, receipt, reaction, tally và cursor; không đổi SQL, migration, API hay UI.
Issue: [#41](https://github.com/loveoverflowcom/vink/issues/41),
[#43](https://github.com/loveoverflowcom/vink/issues/43).
**Lát DISC-01 của [#120](https://github.com/loveoverflowcom/vink/issues/120)** (nền `develop@47131a9`)
thêm admission thuần có version (`domain/discussion/admission.rs`): `premoderation.v1` là policy
hiệu lực duy nhất, ứng viên đăng-trước `trusted-cohort.v1` chỉ chạy shadow, hậu kiểm gắn version
(`hold_for_review`), model `admission.qnt` + replay và ba harness Kani. Không đổi SQL, migration,
API hay UI; bình luận lưu vẫn `pending`. Quyết định và delta D-08 đề xuất:
[admission](../architecture/discussion-admission.md).
**Lát DISC-02/03 phần 2 của #120** (nền `develop@8a58089`) tách đường đọc: public projection
(`GET …/discussion/public`, không resolve actor, kiểu Rust chỉ dựng được khi không có dữ kiện người
xem) và overlay riêng tư (`GET …/discussion/me`: own pending/rejected theo cursor `o1.` riêng, hoặc
đối soát ≤ 20 ID theo trạng thái hiện hành); route composed v1 giữ nguyên. Cursor chính tắc, có
scope và giới hạn 56 byte. Index partial `comments_own_unpublished_idx` được chọn theo số đo.
Fence đình chỉ cho gỡ/bật-tắt/kiểm duyệt đã có từ PR #143; lát này thêm race test hai thứ tự và
test ghi đồng thời không deadlock. Quyết định, số đo và ma trận riêng tư:
[discussion-reads](../architecture/discussion-reads.md).

## HOW: từ lệnh tới hàng trong database

```text
Vink::submit_comment(actor, {work, target?, body, command_key})       application/discussion.rs
  actor bắt buộc → CommandKey::parse → CommentBody::parse (NFC)       domain/discussion/{submission,body}.rs
  PayloadDigest::of_submission; CommentId/ThreadId do server sinh
  → PgStore::submit_comment (một transaction)                          persistence/discussion.rs
      publications FOR SHARE → discussion_threads (tạo nếu thiếu) FOR SHARE → target FOR SHARE
      → principals FOR UPDATE + đọc đình chỉ (fence standing, #120)    persistence/principal.rs
      → claim receipt (ON CONFLICT DO NOTHING)
      → decide_submission(ADMISSION, facts)  ← quyết định thuần duy nhất, admission trước
      → INSERT comments (trạng thái đầu + provenance của admission) + receipt, COMMIT;
        mọi nhánh khác ROLLBACK
```

Từ DISC-02/03 (#120) store ghi đúng `Admission::initial_state` và mã `admission_policy`/
`admission_reason`; migration `20261111000000_comment_admission.sql` tự buộc hàng mới sinh ra đúng
như admission (present, version 1, approved chỉ khi `trusted_and_clear`), chỉ cho
`premoderation.v1` trong pilot và giữ provenance bất biến. Người bị đình chỉ trước commit bị từ
chối (`403 principal_suspended`), kể cả retry đúng key, và không để lại bình luận, receipt hay
binding thread. Chi tiết và lựa chọn: [admission](../architecture/discussion-admission.md#7-lát-disc-0203--đường-gửi).

Database tự giữ hai tầng, không phụ thuộc Rust (migration `20261103000000_discussion.sql`):
FK ghép `(root_id, thread_id, root_kind)` → `(id, thread_id, kind)` buộc root là **root cùng
thread**; FK `(reply_to_id, root_id, thread_id)` buộc context là **reply cùng root**, xóa context
chỉ `SET NULL (reply_to_id)` (không cascade); `replied_to_reply` bất biến phân biệt context đã bị
xóa với trả lời trực tiếp root. Trigger chặn đổi placement/author/thread, giữ gỡ là terminal và
`moderation_version` không giảm. Audit kiểm duyệt append-only.

**Race đã xử lý:** tác giả tắt bình luận (UPDATE hàng thread) và người đọc gửi bình luận (FOR SHARE
cùng hàng) tuần tự hóa: tắt commit trước thì lệnh gửi thấy `Closed`; ngược lại bình luận đã tồn
tại. Tám lần gửi đồng thời cùng khóa idempotency chỉ tạo một bình luận; các lần còn lại replay.

**Chưa xử lý:** chưa có lệnh rút bài (test xóa con trỏ publication để mô phỏng); thông báo, outbox,
cache/invalidation thời gian thực, ranking và sticker chưa tồn tại. Reaction mới có quyết định
thuần (`domain/discussion/reaction.rs`), chưa có bảng, store, route, contract hay UI (#43/#45). D2T-03 thêm projection UI và acknowledgment có scope ở consumer; bằng chứng nằm riêng tại [UI thảo luận](discussion-ui-evidence.md).

## Lựa chọn trong lát này

- **Tiền kiểm:** bình luận mới luôn `pending` (COM-02, D-08). Không có sửa bình luận (D-08 chặn sửa
  công khai cho tới khi có gate). Quyết định duyệt/từ chối/gỡ gắn `ModerationVersion`. Từ #120,
  “luôn `pending`” là kết quả của `decide_initial_review(premoderation.v1)`; store/HTTP vẫn viết cứng
  `pending` (audit ở [admission](../architecture/discussion-admission.md#1-baseline-đối-chiếu-tại-head-thực-thi-ac-01)).
- **Hai trục:** `removal` (null/author/moderation) tách `review` (pending/approved/rejected). Chỉ
  present + approved là công khai; người viết thấy trạng thái bình luận của mình.
- **Gỡ:** người viết gỡ của mình → xóa chữ khỏi database. Kiểm duyệt gỡ → giữ chữ cho audit, không
  đường đọc nào trả về. Root bị ẩn còn reply công khai → điểm neo không chữ/danh tính.
- **Tác giả tác phẩm:** chỉ bật/tắt (`CommentsMode`, `SettingsGeneration`). Gỡ bình luận người
  khác: `NotCommentWriter` khi bình luận đang công khai, `NotFound` khi bị ẩn; không có grant kiểm
  duyệt qua quyền sở hữu tác phẩm.
- **Generation của ý định được chấp nhận** (bật/tắt và reaction; chủ sản phẩm xác nhận
  05/11/2026, theo sửa fence accepted intent của PR #79): yêu cầu mang đúng
  generation hiện tại luôn được chấp nhận và tăng 1, kể cả khi trạng thái đã đúng (`Reaffirmed`,
  có ghi; reaction không đổi count). Generation khác + trạng thái đã đúng → `AlreadyCurrent`, không
  ghi; generation khác + trạng thái khác → `Stale`. Luật cũ kiểm “trùng trạng thái” trước
  generation nên một yêu cầu “giữ nguyên” không tạo fence: đang tắt@g1, tab A gửi bật@g1 đến muộn,
  tab B gửi tắt@g1 đến trước (no-op, g vẫn 1), A khớp g1 và mở lại bình luận, trái COM-07 “tab cũ
  không đè lựa chọn mới”. Toggle Web hiện tại luôn gửi trạng thái ngược nên không tự sinh bước B,
  nhưng HTTP là desired state nên client khác hoặc một PUT bị gửi lại muộn gặp được. Hệ quả HTTP:
  PUT trùng chế độ ở đúng `expected_generation` trả `200` với `generation` + 1 (ghi hàng thread);
  mã lỗi và shape JSON không đổi, không có migration.
- **Tắt:** ngừng nhận root/reply mới, không ẩn bình luận đã duyệt; tác phẩm mới mặc định bật
  (chủ sản phẩm xác nhận 03/11/2026).
- **Phân trang:** root và reply cùng thứ tự `created_at ASC, id ASC`; cursor `c1.<micros>.<uuid>`;
  page 1..50 (mặc định 20); preview 3 reply mỗi root với `preview_next` để đọc tiếp. Một trang dùng
  snapshot `REPEATABLE READ READ ONLY` để đếm, root và preview khớp nhau.
- **Idempotency:** khóa `[A-Za-z0-9_-]{8,128}`, phạm vi (actor, operation). Digest SHA-256 của
  work + target + body đã NFC. Replay trả ID + trạng thái của chính người viết, không trả chữ.
  Retention của receipt chưa có.
- **Reaction (đề xuất, chờ chủ sản phẩm xác nhận):** desired state `Some(kind)`/`None`, không
  toggle; tối đa một reaction mỗi (người, bình luận) trong catalog đóng `heart`/`moved`/
  `thoughtful`/`applause`, chọn loại khác là thay thế. Mỗi thay đổi thật cần generation của
  (người, bình luận) mà client đã thấy và tăng đúng 1, nên bản trùng đến muộn trả `Stale` thay vì
  đảo lựa chọn mới; store tương lai phải giữ hàng đã gỡ (kind rỗng + generation), không xóa. Giới
  hạn tương tác: chỉ bình luận công khai của tác phẩm công khai (bình luận ẩn trả như không tồn
  tại), mọi yêu cầu của người viết trên bình luận của mình là `OwnComment` (không bao giờ có hàng,
  kể cả do reaffirm), tác giả tắt bình luận thì không thêm/đổi reaction nhưng luôn gỡ hoặc
  reaffirm được của mình; retry đúng trạng thái hiện tại vẫn `AlreadyCurrent` sau khi tắt.
  Generation theo luật ý định được chấp nhận ở trên. `ReactionTally` là projection: chỉ đổi theo delta, không âm/không tràn, không quyết định
  ACL/xếp hạng/gỡ, bình luận ẩn không lộ số đếm. Chưa có rate limit theo thời gian (cần clock).

Endorsement văn học là module riêng, không đổi reaction thành Hữu ích hoặc cạnh tranh slot emotion.
[Registry LVE-PR-01](endorsement-core.md) giữ model/trace/proof và quyết định order/no-op của #73.
Runner discussion/Kani dùng chung, D2T semantics/checklist ở đây giữ nguyên.

## Registry traceability của #41

Đây là bảng mapping duy nhất cho D2T-INV trong repo. Requirements/ADR sở hữu quyết định; model
Quint sở hữu transition cấu trúc đã chọn; Rust hiện thực; migration giữ ràng buộc độc lập;
fixture sinh từ model là oracle để replay input qua Rust, không là implementation thứ hai.

| ID / nguồn | Model Quint | Rust production / test | PostgreSQL (`tests/postgres/discussion.rs`) | Kani | Gap còn lại |
| --- | --- | --- | --- | --- | --- |
| D2T-INV-01 — #41 | `structure.apply`, `rootIsRoot`/`canonicalContext`/`noCycles` | `reply_placement`, `root_placement`, `DiscussionThread`; enumeration 20^4 và trace replay; ID trùng root của target là `CommentExists` | `the_database_alone_rejects_a_third_tier_and_cross_scope_references` | [`reply_placement_keeps_two_tiers_inside_one_thread`](../../services/vink-app/src/domain/discussion/proofs.rs): mọi placement lưu được (4 ID × 2 thread), kết quả luôn qua `from_stored` | Model cấu trúc không có removal; neo/context ẩn (INV-05) chưa có model |
| D2T-INV-02 — #41 | Một thread là assumption | `decide_submission` (target tra trong thread), `CommentPlacement::from_stored` | Cùng test: cross-thread root, context khác root, đổi thread bị chặn | Cùng harness (thread khác → `TargetNotFound`); [`submission_receipts_take_precedence_and_inserts_pass_every_gate`](../../services/vink-app/src/domain/discussion/proofs.rs): insert reply luôn dưới root của target cùng thread | Chưa tenant; work binding qua `discussion_threads.work_id` unique |
| D2T-INV-03 — COM-03, COM-07, INV-05 | `lifecycle.setMode`/`submit`, `modeAuthority`, `confirmedModeFence`, `premoderatedSubmission`, `scopedWrites`; trace `comments_mode` (gồm reaffirm rồi yêu cầu cũ bị `stale`); counterexample `mode-noop-fence` (luật no-op cũ) | `authorize_mode_change`, `DiscussionSettings::change_mode`, `ModeChange::write`, actor bắt buộc ở application; `lifecycle_replay` gồm replay counterexample bị production từ chối | `closing_waits_for_an_in_flight_submission_and_vice_versa`, `the_work_author_toggles_comments_but_has_no_power_over_others` (reaffirm được lưu, bật lại muộn → `409`), `the_http_api_keeps_every_discussion_outcome_distinct`, `a_withdrawn_work_closes_its_discussion…` | [`comments_mode_changes_are_generation_bound`](../../services/vink-app/src/domain/discussion/proofs.rs): một yêu cầu, mọi generation `i64`; [`a_confirmed_mode_request_fences_every_earlier_request`](../../services/vink-app/src/domain/discussion/proofs.rs): hai yêu cầu | Auth authority thật (D-05), lệnh rút bài thật; đình chỉ của Vink có fence tại commit cho lệnh gửi (#120), chưa cho gỡ/bật-tắt/kiểm duyệt |
| D2T-INV-04 — COM-02, MOD-02/04 | `viewerProjection`, `premoderatedSubmission`; replay so `view` cho 4 người xem | `lifecycle`, `visibility::view`; lý do chỉ cho `OwnRejected` | `a_new_comment_is_private_until_approved…`, `sql_visibility_filters_match_the_domain_projection`, `rejection_reasons_reach_only_their_writer…` | [`viewer_projection_never_reveals_a_hidden_comment`](../../services/vink-app/src/domain/discussion/proofs.rs): mọi trạng thái × quan hệ người xem | SSR anonymous có source D2T-03; chưa realtime invalidation/notification; sửa bình luận chặn theo D-08 |
| D2T-INV-05 — PUB-03 | Một phần: `removeOwn`, gỡ là terminal trong `versionedLifecycle`; không có neo/context | `remove_by_author`, `root_listing`, `reply_context` | `hidden_roots_anchor_public_replies_and_hidden_contexts_degrade`; SET NULL context ở test toàn vẹn | [`writer_removal_is_terminal_and_hides_the_comment`](../../services/vink-app/src/domain/discussion/proofs.rs); `reply_context`/`can_reply_to` trong harness projection | Purge/retention, restore, quote/snippet |
| D2T-INV-06 — COM-01/04 | `lifecycle.submitWith`, `idempotentSubmission`, `premoderatedSubmission` (receipt trước gate đóng); trace `submission_retry`; counterexample `receipt-retry` | `CommandKey`, `PayloadDigest`, `decide_submission` (receipt trước mọi gate); replay dùng digest thật của (work, target, body NFC/NFD) | `concurrent_retries_with_one_key_create_exactly_one_comment`, idempotency trong test pending | [`submission_receipts_take_precedence_and_inserts_pass_every_gate`](../../services/vink-app/src/domain/discussion/proofs.rs): mọi digest 32 byte × receipt × công khai × mode × target | Retention receipt, endpoint trạng thái lệnh; receipt của reaction không cần (generation fence) |
| D2T-INV-07 — MOD-02, INV-07, COM-08 | `lifecycle.moderate`, `writeAuthority`, `versionedLifecycle`; trace `moderation_authority`; counterexample `author-recusal` | `ModerationAction::apply`, `authorize_moderation` (grant + tự né), `ModerationVersion` | `moderation_is_version_bound_audited…`, `moderation_authority_is_a_revocable_grant_rechecked_at_commit` (thu hồi đang chạy vs quyết định đang chạy), `moderators_recuse_from_their_own_comments_and_their_own_works`, `the_http_moderation_api_keeps_every_outcome_distinct` | [`moderation_decisions_are_version_bound_and_removal_is_terminal`](../../services/vink-app/src/domain/discussion/proofs.rs): mọi version `i64` × hành động | Worker/job fencing, appeal/mở lại, cảnh báo backlog |
| D2T-INV-08 — COM-06 | [`reaction.qnt`](../../services/vink-app/behavior/discussion/reaction.qnt): `noBlindToggle`, `generationFence`, `confirmedIntentFence`, `interactionLimits`, `tallyMatchesReactions`, `noSelfReaction`, `hiddenCountsStayHidden`, `scopedWrites`; trace `desired_state`; counterexample `reaction-toggle`, `reaction-unfenced`, `reaction-noop-fence` | `reaction::{decide_reaction, ActorReaction::desire, ReactionTally::apply, public_tally}`; unit tests và `reaction_replay` (gồm replay counterexample bị production từ chối) | — (chưa có bảng; #43) | `reaction_changes_are_desired_state_and_generation_bound`, `a_confirmed_reaction_request_fences_every_earlier_request` (hai yêu cầu), `reaction_admission_enforces_the_interaction_limits`, `reaction_tallies_follow_deltas_without_underflow_or_wrapping` | Semantics là đề xuất; persistence/khóa/đồng thời, API/contract, UI, rate limit, catalog do sản phẩm chốt |
| D2T-INV-09 — #41 | Chưa có | `PageCursor` (`at_micros` là seam chung của `decode`), `PageLimit`, `PREVIEW_REPLIES`; biên `time` ±9999 năm | `pages_follow_the_keyset_order_and_previews_continue_on_the_reply_page` (ties theo ID) | [`refined_scalars_accept_exactly_their_predicate`](../../services/vink-app/src/domain/discussion/proofs.rs): `PageLimit` với mọi `Option<u32>`; [`page_cursors_round_trip_every_representable_position`](../../services/vink-app/src/domain/discussion/proofs.rs): mọi `i64` micros × mọi UUID | Thứ tự keyset nằm ở SQL (`postgres`); parser chuỗi cursor chỉ có unit test; ranking “Nổi bật”, epoch/snapshot, benchmark |
| D2T-INV-10 — #45 | Chưa có consumer model | `apps/vink-web/src/discussion_state.rs`: scope, edit generation, logical key và physical attempt; unit regressions | Không thêm test DB; receipt/idempotency theo INV-06 | Chưa có | Consumer browser evidence riêng; CMP projection mẫu, chưa native integration/Quint interleavings/production auth/OS IME |
| D2T-INV-11 — #25 | [EMO lifecycle owner](expressions.md#registry-emo--một-model-lifecycle), không model comment | `CommentBody` giữ Unicode; `vink-expressions` sở hữu manifest/policy riêng, Web pilot dùng thật | — | Owner-local EMO harnesses, xem evidence PR | Chưa typed sticker/reaction persistence và composer #43/#45; không nhận proof EMO là proof discussion |
| D2T-INV-12 — COM-02, D-08, MOD-02/03/05, #120 | [`admission.qnt`](../../services/vink-app/behavior/discussion/admission.qnt): `admissionRecordsProvenance`, `refusalWritesNothing`, `premoderationAdmitsPending`, `approvalNeedsEveryGuard`, `shadowOnlyMeasures`, `ticketsOnlyForPublishFirst`, `postReviewFenced`, `removalTerminal`, `scopedVersionedWrites`; trace `rollout`; counterexample `admission-shadow`, `admission-stale-trust`, `admission-unfenced` | `admission::{decide_initial_review, decide_admission, PolicySelection, Admission, decide_post_review}`, `CommentState::hold_for_review`; unit tests, vét cạn facts × policy, `admission_replay` (gồm ba counterexample bị production từ chối) | `admission_provenance_is_recorded_and_held_by_the_schema`, `a_suspended_writer_is_refused_at_commit_and_nothing_is_written`, `suspension_and_submission_serialize_on_the_principal` (hai thứ tự), `upgrade_records_premoderation_for_existing_comments` | [`admission_approves_only_when_every_guard_passes`](../../services/vink-app/src/domain/discussion/proofs.rs), `shadow_selection_keeps_premoderation_effective`, `post_review_only_withdraws_the_observed_approval` | Ứng viên chỉ shadow (chưa có cả shadow trong deployment); chưa authority trust, rules engine, classifier, store quan sát shadow, outbox/worker hậu kiểm, rate limit; delta D-08 PROPOSED |
| D2T-INV-13 — #120 DISC-04, MOD-03 | [`outbox.qnt`](../../services/vink-app/behavior/discussion/outbox.qnt): `effectAtMostOnce`, `staleWritesNothing`, `fenceMonotone`, `scoped`; trace `lease_reclaim_and_two_effects`; counterexample `outbox-unfenced` | `domain/discussion/outbox.rs` (`decide_claim`, `check_ownership`, `decide_completion`, `decide_requeue`, `observe_shadow`); unit + `outbox_replay` | [`tests/postgres/outbox.rs`](../../services/vink-app/tests/postgres/outbox.rs): T1–T12 gồm process `vink-worker` thật, role runtime, rollback có guard | `outbox_claims_advance_the_fence_or_end_the_delivery`, `outbox_completion_is_fenced_and_never_turns_failure_into_success` | Consumer duy nhất là shadow/audit; hậu kiểm có hiệu lực (exhaustion → hold), event cho gỡ/kiểm duyệt, LISTEN/NOTIFY, metrics endpoint ([quyết định](../architecture/discussion-outbox.md)) |

Các symbol model, fixture và lệnh thực thi của lát cắt 1 được chỉ mục ở
[owner-local behavior](../../services/vink-app/behavior/discussion/README.md).
Các hàng “Chưa có” vẫn là gap mở, không phải invariant đã PASS nhờ default state rỗng.

## Kiểm chứng theo lớp

Lệnh được sở hữu tại [development guide](README.md), runner ở `xtask`; CI gọi cùng lệnh.

1. **Domain thuần (`quick`):** unit tests cho từng module, gồm duyệt mọi chuỗi 4 lệnh của vòng đời,
   bảng quyết định đầy đủ của `decide_submission` theo receipt × publication × mode × target, và
   enumeration 20^4 của cấu trúc. Đây là finite enumeration của code, không gọi là Kani.
2. **Application (`quick`):** store ghi nhận để kiểm xác thực trước validate, input thô không tới
   store, ID do server sinh và mỗi kết quả giữ mã lỗi riêng. Không chứng minh luật của store.
3. **Quint (`discussion`):** model cấu trúc (lát cắt 1), model vòng đời `lifecycle.qnt`
   (D2T-02: tiền kiểm, gỡ, version, bật/tắt, grant tại commit, tự né; receipt/retry D2T-INV-06),
   `reaction.qnt` (D2T-INV-08), `admission.qnt` (D2T-INV-12: policy, shadow, hậu kiểm) và
   `outbox.qnt` (D2T-INV-13: claim, fence generation, retry, dead letter, requeue). Simulation ngẫu nhiên có giới hạn, negative control có
   counterexample thật và replay input qua các quyết định Rust production, không phải model
   checking; khóa và transaction vẫn chỉ có bằng chứng ở lane `postgres`.
4. **PostgreSQL thật (`postgres`):** ràng buộc bằng SQL trực tiếp bỏ qua Rust, race có khóa tường
   minh, idempotency đồng thời, kiểm duyệt có audit, neo/context, keyset có ties, và so sánh bộ lọc
   SQL với projection domain trên mọi trạng thái × người xem. Chạy với superuser của harness;
   ràng buộc/trigger áp cho mọi role.
5. **Kani (`kani`):** harness owner-local trên `src/domain` kiểm từng quyết định thuần với toàn
   miền input (version/generation/cursor `i64`, digest 32 byte, count `u32`, mọi trạng thái ×
   người xem × loại reaction; ID trong vài giá trị ký hiệu đủ để bằng/khác nhau), cột Kani ở
   bảng trên; 19 harness thảo luận, gồm ba harness admission (#120) với mọi instant `i64` và oracle
   tuổi `i128`, và hai harness outbox DISC-04 (claim/fence/retry trên mọi trạng thái và instant). Không
   thay Quint cho chuỗi lệnh hay `postgres` cho khóa. **Apalache/model checking NOT_RUN.** D2T-03 có browser scenarios trong lane Web; trạng thái từng run ghi riêng, không suy PASS từ source.
6. **HTTP (`quick` + `postgres`):** fixture tests hai phía (provider serialize so field-for-field,
   consumer `vink-web` decode giữ đủ field và phân loại mã lỗi), và
   `the_http_api_keeps_every_discussion_outcome_distinct` qua router thật trên PostgreSQL: thứ tự
   xác thực → Origin → body, `no-store`, 201/200 replay, 422 theo field, 403/404/409 theo mã.

7. **Consumer D2T-03:** [LayoutSpec](../design/discussion-ui.md), controller unit tests, Web/CMP projection scenarios và `discussionReadingAndWrites` trong browser smoke. [Bằng chứng riêng](discussion-ui-evidence.md) phân biệt local fixture/PostgreSQL, intercepted HTTP fixture, browser, model và native.

Pin Kani, lệnh và negative control: [development guide](README.md#kani).
Kết quả lệnh thực tế được ghi trong PR; sự có mặt của test/CI không tự là PASS.
