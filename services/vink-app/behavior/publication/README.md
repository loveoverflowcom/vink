# Publication — canonical M1, M3 và scoped Rust replay (#150, #151, #157)

M1 (`publication.qnt`) và M3 (`tombstone.qnt`) đã chuyển nguyên model, vectors và negative controls
từ `../target`; không có policy copy. Trạng thái là `PARTIAL_PURE_CORE_CONFORMANCE`, **không** phải
whole-model conformance hay runtime acceptance.

Runner chung ở [`../target/verify.mjs`](../target/verify.mjs) giữ Quint **0.33.1** và lockfile duy
nhất của `../discussion`: typecheck, simulation TypeScript seed **41**, **1.000 samples × tối đa
40 bước**, các scenario xuất ITF thật, và negative controls. Vectors được xuất từ ITF thật, không JS reducer.

`cargo xtask verify discussion` đòi chọn đủ sáu test replay của publication/moderation/tombstone trong `REPLAYS`:

- `domain::publication::replay::quint_publication_author_inputs_and_admitted_decision_effects_replay_through_production`:
  Đủ **51/51 bước** của `publication-vectors.json` (`author`: 24, `ai_result`: 13, `admin_decide`: 6,
  `admin_remove`: 3, `reevaluate`: 4, `policy_bump`: 1) chạy qua `Publication` và `WorkModeration` production;
  5 bước delete/remove lifecycle model-only cũ (`lifecycle_fixture` × 2, `outside_scope` × 3) đã chuyển thành
  lệnh thật `AuthorCommand::Delete` và `apply_admin_remove` (#157).
- `domain::publication::replay::the_withdrawn_intent_counterexample_is_refused_by_the_production_publication_guard`:
  replay `publication-withdrawn-intent-counterexample.json` (`mutatedIntentStep`); production trả
  `approved_unpublished` và giữ `pointer = None`.
- `domain::moderation::replay::the_admin_late_result_counterexample_is_refused_by_the_production_moderation_guard`:
  replay `publication-late-result-counterexample.json` (`mutatedOverrideStep`); callback AI `reject`
  đến muộn sau khi admin đã `approve` bị từ chối `overridden` và giữ nguyên quyết định admin.
- `domain::moderation::replay::quint_evaluator_vectors_replay_publication_steps_and_check_comment_contract`:
  chạy cả 21 bước của `../target/evaluator-vectors.json` (đủ 9 lớp `ScriptKind`) qua
  `ScriptedEvaluator`: 12 bước `publication.qnt` đi qua full consumer (`WorkModeration` +
  `Publication`), 9 bước `comment_edit.qnt` đi qua shared validator `validate_evaluation_result`.
- `application::publication::replay::quint_tombstone_scenarios_replay_against_production_access_and_discussion`:
  Replay cả 3 scenario (đủ **30/30 bước**: `author_deleted_public_work` 14, `private_then_deleted` 10,
  `moderation_removed` 6) qua `Publication`, `WorkModeration`, `plan_work_access` (`PublicBody`,
  `AuthorTombstone`, `Unavailable`), `plan_comment_view`, và `check_comment_submission` (#157).
- `application::publication::replay::the_private_as_deleted_counterexample_is_refused_by_the_production_access_plan`:
  replay `tombstone-private-as-deleted-counterexample.json` (`mutatedPrivateStep`); mutant cho phép
  bài chuyển riêng tư hiện tombstone và đọc bình luận; production từ chối với `WorkAccessPlan::Unavailable`
  và `CommentView::Unavailable`.

## Coverage bảng action M1

| Action trong M1 | Rust coverage (#150, #151, #157) | Còn thiếu |
| --- | --- | --- |
| `snapshot` | `WorkModeration::decide_author` → `Publication::decide_author(SaveSnapshot)`, không mở job | SQL snapshot/metadata binding |
| `request_public`, `set_private` | Owner, lifecycle, exact revision, generation, moderation, pointer guard + mở `EvaluationTicket` khi `Submitted` | Transaction + actor standing tại commit; HTTP/UI |
| `ai_result` (cả hợp lệ và bị từ chối) | `WorkModeration::apply_evaluator_result` + `validate_evaluation_result` + `Publication::record_review` | Durable job/result tables, worker consumer, apply transaction fence |
| `admin_decide` (cả hợp lệ và bị từ chối) | `WorkModeration::apply_admin_decision` (`AdminAuthority`, `DecisionVersion`, `DecisionReason`) + `Publication::record_review` | Durable admin grant provisioning, CLI/HTTP, audit row |
| `reevaluate`, `policy_bump` | `WorkModeration::reevaluate` (cấp `EvaluationId` mới), `WorkModeration::bump_policy` | Policy persistence/governance, retry worker |
| `author_delete` | `decide_author(Delete)` bắt `DeletionProvenance` (`PublicTombstone` vs `Hidden`), chuyển audience sang `Private`, tăng generation | Persistence migration / CLI / route (#158) |
| `admin_remove` | `apply_admin_remove` kiểm `AdminAuthority`, xoá pointer/intent, chuyển `Lifecycle::Removed` | Persistence migration / CLI / route (#158) |
| `mutatedIntentStep` | Model NC + production guard refusal (`approved_unpublished`) | Durable commit race |
| `mutatedOverrideStep` | Model NC + production guard refusal (`overridden`) | Durable commit race |

## Coverage bảng action M3 (Tombstone)

| Action trong M3 | Rust coverage (#157) | Còn thiếu |
| --- | --- | --- |
| `author_delete` (từ public) | `DeletionProvenance::PublicTombstone(pointer)`, `WorkAccessPlan::AuthorTombstone`, thread accessible | Persistence tombstone schema & query (#158) |
| `author_delete` (từ private / unpublished) | `DeletionProvenance::Hidden`, `WorkAccessPlan::Unavailable`, thread unavailable | Persistence (#158) |
| `moderation_remove` | `Lifecycle::Removed`, `WorkAccessPlan::Unavailable`, thread unavailable | Persistence (#158) |
| `submit` trên tombstone | `check_comment_submission` trả `TombstoneWriteError::TombstoneReadOnly` | API 409 / UI (#158) |
| `submit` trên unavailable | `check_comment_submission` trả `TombstoneWriteError::WorkUnavailable` | API 404 (#158) |
| `remove_own` trên tombstone | `CommentState::remove_by_author()`, bình luận removed giữ anchor nếu có reply | Discussion API route (#158) |
| `views` projection | `plan_work_access` + `plan_comment_view` khớp chính xác 30/30 bước viewer | SSR / reader UI (#158) |
| `mutatedPrivateStep` | Model NC + production refusal (`Unavailable` thay vì tombstone) | Integration tests |

Pure facts sử dụng UUID/content hợp lệ do harness cấp; actor/session/authentication, serialized
commit, durable job storage và admin grant provisioning là giả định. Simulation có biên không là
exhaustive proof; Apalache **NOT_RUN**. PostgreSQL, HTTP, browser và production **NOT_RUN** trong
scope này. API, choices, AC và bước nối tiếp:
[publication-core](../../../../docs/development/publication-core.md) và
[moderation-core](../../../../docs/development/moderation-core.md).
