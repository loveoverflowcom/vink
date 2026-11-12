# Publication — canonical M1, M3, M4 quản trị và scoped Rust replay (#150, #151, #157, ADMIN-MODERATION-20261111)

M1 (`publication.qnt`) và M3 (`tombstone.qnt`) đã chuyển nguyên model, vectors và negative controls
từ `../target`; M4 quản trị (`governance.qnt`, `ADMIN-MODERATION-20261111`) thay `../target/appeal.qnt`.
Không có policy copy. Trạng thái là `PARTIAL_PURE_CORE_CONFORMANCE`, **không** phải whole-model
conformance hay runtime acceptance.

Runner chung ở [`../target/verify.mjs`](../target/verify.mjs) giữ Quint **0.33.1** và lockfile duy
nhất của `../discussion`: typecheck, simulation TypeScript seed **41**, **1.000 samples × tối đa
40 bước**, các scenario xuất ITF thật, và negative controls. Vectors được xuất từ ITF thật, không JS reducer.

`cargo xtask verify discussion` đòi chọn đủ mười test replay của publication/moderation/tombstone/governance
trong `REPLAYS`:

- `domain::publication::replay::quint_publication_author_inputs_and_admitted_decision_effects_replay_through_production`:
  Đủ **64/64 bước** của `publication-vectors.json` (3 scenario: `audience_moderation_override` 44,
  `policy_rereview` 9, `moderation_removal` 11; theo nhóm: `author` 28, `ai_result` 16, `admin_decide` 10,
  `admin_remove` 3, `reevaluate` 5, `policy_bump` 2) chạy qua `Publication` và `WorkModeration`
  production, gồm job bị thu hồi và ca đóng khi “Chỉ mình tôi”, gửi lại và duyệt lại theo policy mới
  của một approval cũ (PUB-06).
- `domain::publication::replay::the_moved_intent_counterexample_is_refused_by_the_production_publication_guard`:
  replay `publication-moved-intent-counterexample.json` (`mutatedIntentStep` → `pointerMovesLawfully`);
  ý định công khai đã trỏ r2 nên approval của r1 không được đưa r1 lên.
- `domain::moderation::replay::the_admin_late_result_counterexample_is_refused_by_the_production_moderation_guard`:
  replay `publication-late-result-counterexample.json` (`mutatedOverrideStep`); callback AI `reject`
  đến muộn sau khi admin đã `approve` bị từ chối `overridden` và giữ nguyên quyết định admin.
- `domain::moderation::replay::quint_evaluator_vectors_replay_publication_steps_and_check_comment_contract`:
  chạy cả 24 bước của `../target/evaluator-vectors.json` (đủ 9 lớp `ScriptKind`) qua
  `ScriptedEvaluator`: 15 bước `publication.qnt` đi qua full consumer (`WorkModeration` +
  `Publication`, gồm `withdrawn` sau khi chuyển riêng tư), 9 bước `comment_edit.qnt` đi qua shared
  validator `validate_evaluation_result`.
- `application::publication::replay::quint_tombstone_scenarios_replay_against_production_access_and_discussion`:
  Replay cả 5 scenario (đủ **53/53 bước**: `author_deleted_public_work` 14,
  `hidden_then_public_then_deleted` 16, `never_public` 4, `moderation_removed` 7,
  `review_withdrawn_then_resubmitted` 12) qua `Publication`,
  `WorkModeration`, `plan_work_access` (`PublicBody`, `AuthorHidden`, `ReviewWithdrawn`, `AuthorTombstone`, `Unavailable`),
  `plan_comment_view`, `plan_comment_review_context`, `check_comment_submission`,
  `check_own_comment_removal` và `Publication::should_notify_author`. Từ chối mức tác phẩm đến từ lỗi
  production, không từ guard của harness; shell `publish` giả định một approval đã được admit
  (đường evaluator/admin thuộc M1).
- `application::publication::replay::the_removed_as_hidden_counterexample_is_refused_by_the_production_access_plan`:
  replay `tombstone-removed-as-hidden-counterexample.json` (`mutatedRemovedStep` →
  `noExceptionForUnavailableWork`); mutant cho bài bị admin gỡ hiện trang ẩn và bình luận; production
  trả `WorkAccessPlan::Unavailable`.
- `application::publication::replay::the_hidden_notice_counterexample_is_refused_by_the_production_notification_fence`:
  replay `tombstone-hidden-notice-counterexample.json` (`mutatedNoticeStep` → `noNoticeFromHiddenPeriod`);
  công khai lại không báo cho tác giả quyết định đã có trong lúc ẩn (`should_notify_author` dùng `public_epoch`).
- `application::publication::replay::review_withdrawn_counterexample_cannot_expose_a_public_body`:
  replay `tombstone-withdrawn-body-counterexample.json` (`mutatedWithdrawnStep` → `bodyOnlyWhilePublic`);
  body của bản bị bác không được trả qua trang thông báo.
- `domain::moderation::governance_replay::quint_governance_scenarios_replay_through_production`:
  đủ **105/105 bước** của `governance-vectors.json` (`grants_and_claims` 39, `automation_and_late_results` 16,
  `appeals` 31, `investigation` 19) qua `ModerationDirectory`, `ReviewCase`, `AppealBook`, `Automation`,
  `Investigation` và các owner quyết định (`WorkModeration` + `Publication`; `CommentState` cho bình luận).
  Sau mỗi bước toàn bộ state model được chiếu lại từ production facts và so sánh.
- `domain::moderation::governance_replay::the_governance_counterexamples_are_refused_by_production`:
  năm negative control (`mutatedAuthorityStep`, `mutatedClaimStep`, `mutatedAppealStep`, `mutatedPauseStep`,
  `mutatedReadStep`) replay prefix rồi production từ chối bước bị đột biến.

Harness glue của M4 (ghi rõ trong `governance_replay.rs`): lệnh `submit` trừu tượng trên target đã gửi
trả `not_needed` (tái xuất bản thuộc M1); tạo bình luận không gate theo quyền đọc của bài (thuộc M3);
kết quả evaluator của bình luận đi qua `validate_evaluation_result` rồi lifecycle vì chưa có comment
evaluator consumer (`#154`), và ai quyết định bình luận do harness theo dõi; scenario điều tra bắt đầu
từ fixture công khai r1 qua lệnh production. Lý do bắt buộc thiếu và các biên không model hóa được phủ
bởi unit test riêng `domain::moderation::governance_tests`.

Các bảng dưới mô tả ranh giới **pure replay**. Cột cuối là phần không được replay chứng minh,
không phải danh sách runtime còn thiếu; hiện trạng SQL/worker/Web ở
[moderation-runtime](../../../../docs/development/moderation-runtime.md).

## Coverage bảng action M1

| Action trong M1 | Rust coverage (#150, #151, #157) | Ngoài bằng chứng replay |
| --- | --- | --- |
| `snapshot` | `WorkModeration::decide_author` → `Publication::decide_author(SaveSnapshot)`, không mở job | SQL snapshot/metadata binding |
| `request_public`, `set_private` | `WorkModeration::decide_author_with`: owner, lifecycle, exact revision, generation, moderation, pointer guard; mở `EvaluationTicket` khi `Submitted` (hoặc đi thẳng hàng chờ người khi tự động đang dừng); `set_private` thu hồi job (`JobState::Withdrawn`) và báo `withdrawn_reviews` để đóng ca/phản đối; công khai lại dùng quyết định còn hiệu lực đúng snapshot + policy | SQL cho `withdrawn`, `hidden`, `ever_public`, `public_epoch`; HTTP/UI |
| `ai_result` (cả hợp lệ và bị từ chối) | `WorkModeration::apply_evaluator_result` + `validate_evaluation_result` + `Publication::record_review` | Durable job/result tables, worker consumer, apply transaction fence |
| `admin_decide` (cả hợp lệ và bị từ chối) | `WorkModeration::apply_admin_decision` (`AdminAuthority`) và `apply_human_decision` / `apply_case_conclusion` (`VerifiedAuthority`, `DecisionBy::Moderator`) + `DecisionVersion`, `DecisionReason`, `Publication::record_review` | Durable grant/ca, CLI/HTTP, audit row |
| `reevaluate`, `policy_bump` | `WorkModeration::reevaluate` (cấp `EvaluationId` mới), `WorkModeration::bump_policy`; công khai lại sau policy mới → lượt mới, quyết định người cũ → `DecisionBy::PolicyRereview` cho người có thẩm quyền | Policy persistence/governance, retry worker; `RevisionDecisionFacts.policy` theo quyết định |
| `author_delete` | `decide_author(Delete)` bắt `DeletionProvenance` (`PublicTombstone` vs `Hidden`), chuyển audience sang `Private`, tăng generation | Persistence migration / CLI / route (#158) |
| `admin_remove` | `apply_admin_remove` kiểm cặp work, `AdminAuthority` scoped và tác giả tự recuse, xoá pointer/intent, chuyển `Lifecycle::Removed` | Persistence migration / CLI / route (#158) |
| `mutatedIntentStep` | Model NC + production guard refusal (pointer không dời sai bản) | Durable commit race |
| `mutatedOverrideStep` | Model NC + production guard refusal (`overridden`) | Durable commit race |

## Coverage bảng action M3 (Tombstone)

| Action trong M3 | Rust coverage (#157, ADMIN-MODERATION-20261111) | Ngoài bằng chứng replay |
| --- | --- | --- |
| `make_private` (bài đang công khai) | `Publication.hidden`, `WorkAccessPlan::AuthorHidden` (“Nội dung đã được ẩn” + bình luận đủ điều kiện), `HiddenReadOnly` | Đã tích hợp reader API/SSR/Web và chặn ghi; xem [runtime](../../../../docs/development/moderation-runtime.md) |
| `author_delete` (từ public hoặc từ trang ẩn của bài từng công khai) | `DeletionProvenance::PublicTombstone(pointer)`, `WorkAccessPlan::AuthorTombstone`, thread accessible | Persistence tombstone/`ever_public` |
| `author_delete` (chưa từng công khai) | `DeletionProvenance::Hidden`, `WorkAccessPlan::Unavailable`, thread unavailable | Persistence |
| `review` bình luận trên trang ẩn/thay thế | `plan_comment_review_context` không đưa thân bài cho người duyệt | API/worker |
| `publish` lại sau khi ẩn | `Publication::should_notify_author` (fence `public_epoch`): chỉ sự kiện mới | `public_epoch` SQL; kênh thông báo (O-19) |
| `moderation_remove` | `Lifecycle::Removed`, `WorkAccessPlan::Unavailable`, thread unavailable | Persistence (#158) |
| `submit` trên tombstone | `check_comment_submission` trả `TombstoneWriteError::TombstoneReadOnly` | API 409 / UI (#158) |
| `submit` trên unavailable | `check_comment_submission` trả `TombstoneWriteError::WorkUnavailable` | API 404 (#158) |
| `remove_own` trên tombstone / unavailable | `check_own_comment_removal` rồi `CommentState::remove_by_author()`; bình luận removed giữ anchor nếu có reply | Discussion API route (#158) |
| `reject_public`, `submit_revision` | `ReviewWithdrawn` giữ thread, chặn body và ghi mới; gửi bản mới giữ trang, duyệt đúng intent mới thay thế; riêng tư rút URL | API/Web evidence ở runtime, tách khỏi model |
| `views` projection | `plan_work_access` + `plan_comment_view` khớp chính xác 53/53 bước viewer | SSR / reader UI |
| `mutatedRemovedStep` | Model NC + production refusal (`Unavailable`, không phải trang ẩn) | Integration tests |
| `mutatedNoticeStep` | Model NC + production refusal (`should_notify_author` = false) | Kênh thông báo |

## Coverage bảng action M4 (quản trị kiểm duyệt)

| Action trong M4 | Rust coverage | Ngoài bằng chứng replay |
| --- | --- | --- |
| `grant`, `revoke`, `join`, `leave`, `tick` | `ModerationDirectory::{grant, revoke, set_membership, resolve}` (hạn, thu hồi, nhóm, tạm khóa, tự né) | Bảng grant/nhóm, kế thừa `moderator_grants`, CLI/API admin |
| `claim`, `release`, `note`, `escalate`, `takeover`, `revalidate`, `conclude` | `ReviewCase` (fence `CaseVersion` + phiên bản quyết định, `InternalNote`, `CaseConclusion`) → `WorkModeration::apply_case_conclusion` / `CommentState` | Bảng ca + ghi chú, worker revalidate |
| `intervene` | `WorkModeration::apply_human_decision` (moderator trong grant; bác bản đang đăng) | API |
| `pause`, `resume`, `ai_result` khi dừng | `Automation::set`, `WorkModeration::reroute_to_humans` (`JobState::Rerouted`), `EvaluationRefusal::AutomationPaused` | Chế độ lưu bền có phiên bản; worker đọc mode từ store |
| `appeal`, `appeal_conclude` | `AppealBook::{file, conclude}` → `WorkModeration::apply_appeal_conclusion` | Bảng hồ sơ phản đối; kênh thông báo |
| `make_private`, `make_public` | `decide_author_with` + `ReviewCase::close` / `AppealBook::close_open` (`CloseReason::Withdrawn`) | Transaction đóng ca cùng commit |
| `open_investigation`, `read_investigation`, `close_investigation` | `Investigation::{open, read, close}` (audit trước snapshot) | Bảng hồ sơ + audit đọc commit trước khi trả |
| 5 negative control | Model NC + production refusal | Durable commit race |

Pure facts sử dụng UUID/content hợp lệ do harness cấp; actor/session/authentication, serialized
commit, durable job storage, grant/ca/phản đối/hồ sơ điều tra lưu bền là giả định. Simulation có biên không là
exhaustive proof; Apalache **NOT_RUN**. PostgreSQL, HTTP, browser và production **NOT_RUN** trong
scope này. API, choices, AC và bước nối tiếp:
[publication-core](../../../../docs/development/publication-core.md) và
[moderation-core](../../../../docs/development/moderation-core.md).
