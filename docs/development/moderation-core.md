# Moderation evaluator contract, scripted mock và quyền áp quyết định — MOD-CORE-01 (#151)

**Phạm vi:** Rust thuần (`domain::moderation`) + application orchestration & deterministic local/test
scripted mock (`application::moderation`), nối trực tiếp vào [`Publication::record_review`](../../services/vink-app/src/domain/publication.rs)
của [#150](publication-core.md). Nguồn luật là MOD-01/02/03/05/07/08/09, AUD-02, PUB-01/02 và
INV-01/02/03/07/08 trong [PRD](../../prd/requirements.md), [đặc tả](../architecture/publication-moderation.md)
và [DecisionDelta](../architecture/product-decisions-20261110.md). Không đổi D-07/D-19 thành Accepted,
không thêm bảng SQL/worker consumer/HTTP/CLI, không nối provider AI thật và không bật auto-approval
production (MOD-06).

## Owner và lựa chọn kỹ thuật

- **CHOICE-01 A — Typed envelope có target variants rõ + shared outcome/binding validator:**
  `EvaluationTarget` tách `WorkRevision { work, revision, digest: ContentHash }` và
  `CommentRevision { comment, revision, digest: PayloadDigest }`; cả hai đi qua cùng
  `validate_evaluation_result` để giữ đúng thứ tự guard của `publication.qnt` và `comment_edit.qnt`.
  Chỉ consumer của work revision (`WorkModeration` + `Publication`) được nối thật ở `#151`;
  `CommentRevision` chỉ kiểm tương thích hợp đồng/script (`evaluator-vectors.json`), chưa có
  comment-edit consumer (`#154`).
- **CHOICE-02 B — Capability / validated-command types + fallible state restoration:**
  Giữ pattern của `#150`: `WorkModeration(WorkModerationFacts)` và `AdminAuthority` có field
  private, không `Deserialize` bypass (có doctest `compile_fail` kèm lawful twin), khôi phục qua
  `WorkModeration::restore(&Publication, WorkModerationFacts)` kiểm tra đồng nhất work/revision/hash,
  trạng thái `Moderation`, `DecisionVersion`, `DecisionProvenance` và `EvaluationJobFacts`.
- **CHOICE-03 A — Scripted mock adapter đọc canonical script vectors + typed queue:**
  `ScriptedEvaluator` (`application::moderation`, gắn nhãn `local_test_scripted_evaluator`) đọc
  trực tiếp `vink-moderation-evaluator-script-v1` (`evaluator-vectors.json`) hoặc nhận hàng đợi
  `ScriptedStep` thuộc đủ 9 lớp `ScriptKind`; không mạng, không API key, không `sleep`, không log
  nội dung bản thảo/prompt, và khi hết script luôn trả `EvaluatorOutcome::ProviderError` (`retryable`)
  thay vì fallback `Approve`.

## Hợp đồng và ranh giới (`domain::moderation`, `application::moderation`)

Source: [`domain/moderation.rs`](../../services/vink-app/src/domain/moderation.rs),
[`application/moderation.rs`](../../services/vink-app/src/application/moderation.rs) và
[`domain/ids.rs`](../../services/vink-app/src/domain/ids.rs).

| Thành phần | Contract |
| --- | --- |
| `EvaluationId` | Danh tính nominal cho một lần gửi đánh giá; retry sau lỗi hạ tầng phải cấp `EvaluationId` mới, tách biệt với `DecisionVersion` và `ClaimGeneration` của worker outbox (MOD-08) |
| `PolicyVersion` / `DecisionVersion` | Số nguyên dương đơn điệu (`>= 1`, bắt đầu ở `FIRST = 1`, `next()` kiểm tràn). `DecisionVersion` gắn từng revision đã submit và tăng mỗi khi có quyết định AI hoặc admin được chấp nhận, chặn ABA trên `Moderation` |
| `RawEvaluatorOutput` → `EvaluatorOutcome` | `#[serde(deny_unknown_fields)]`; JSON hỏng, trường lạ, `reject` thiếu `PolicyReasonCode` hợp lệ hoặc `approve` kèm trường phụ đều phân loại thành `InvalidOutput` → `NeedsAdmin`, không bao giờ thành `Approve` và không log văn bản đầu vào (MOD-01, MOD-05) |
| `EvaluationTicket` / `EvaluatorResult` | Gắn `evaluation_id`, `EvaluationTarget` (subject + `RevisionId` + digest) và `PolicyVersion`; kết quả tự thân không mang quyền |
| `validate_evaluation_result` | Kiểm theo đúng thứ tự: `NotEvaluator` → `WrongTarget` (cross-subject) → `UnknownJob` → `Duplicate` (`job.state != Open`) → `WrongTarget` / `WrongRevision` (sai revision hoặc digest) → `TargetGone` (`lifecycle != Active`) → `Overridden` (`by == Admin`) → `StalePolicy` → `Superseded` (`!= Pending`). Mọi nhánh `Err` giữ nguyên jobs, decision và publication |
| `WorkModeration::decide_author` | Ủy quyền cho `Publication::decide_author`; chỉ mở `EvaluationTicket` khi `AuthorOutcome::Submitted` (lưu nháp riêng không mở job, MOD-05/INV-08) |
| `WorkModeration::reevaluate` | Mở job mới với `EvaluationId` mới khi tác phẩm `Active`, revision đang `Pending` và chưa có job `Open` ở policy hiện hành |
| `WorkModeration::apply_evaluator_result` | `Timeout` / `ProviderError` đóng job sang `Failed` (`retryable`), giữ nguyên `Pending` và pointer; `Uncertain` / `InvalidOutput` → `NeedsAdmin`; `Reject(code)` → `Rejected`; `Approve` gọi `Publication::record_review` để quyết định `Published` hay `ApprovedUnpublished` |
| `AdminAuthority` / `AdminDecision` / `apply_admin_decision` | Cần `AdminAuthority` hẹp gắn `(actor, work)` với `actor != owner`, revision đã submit (`!= Unsubmitted`), lifecycle `Active`, đúng `ContentHash`, ` DecisionReason` không rỗng và `expected == current DecisionVersion`; tăng `DecisionVersion`, ghi `DecisionBy::Admin` và ủy quyền pointer cho `Publication::record_review` |
| `ModerationEvaluator` / `ScriptedEvaluator` | Port đồng bộ nhận `&EvaluationTicket`; tương lai worker gọi ngoài DB lock rồi đọc lại standing/binding/version trong transaction apply |

## Ma trận 9 kịch bản của `ScriptedEvaluator`

| `ScriptKind` | Kết quả evaluator nộp | Hành vi áp quyết định | Thay đổi public pointer |
| --- | --- | --- | --- |
| `Approve` | `outcome = Approve` đúng job/revision/digest/policy | `Published` nếu `Publication::promotion_eligible` còn đúng; ngược lại `ApprovedUnpublished` | Chỉ đổi khi tác giả còn public intent trên đúng revision đó |
| `Reject` | `outcome = Reject(PolicyReasonCode)` | `Rejected`, tăng `DecisionVersion` | Không đổi pointer của r1 đang công khai (PUB-02) |
| `NeedsAdmin` | `outcome = Uncertain` | `NeedsAdmin`, tăng `DecisionVersion` | Không đổi |
| `Timeout` | `outcome = Timeout` | `Retryable`, job → `Failed`, revision giữ `Pending` | Không đổi |
| `ProviderError` | `outcome = ProviderError` | `Retryable`, job → `Failed`, revision giữ `Pending` | Không đổi |
| `InvalidOutput` | Payload sai schema / `InvalidOutput` | `NeedsAdmin` (`NeedsAdminCause::InvalidOutput`), tăng `DecisionVersion` | Không đổi |
| `WrongRevision` | Job mở cho r2 nhưng trả r1 hoặc sai digest | Từ chối `WrongRevision`, job vẫn `Open` | Không đổi |
| `DuplicateResult` | Gửi lại trên `EvaluationId` đã `Applied` hoặc `Failed` | Từ chối `Duplicate` | Không đổi |
| `DelayedResult` | Kết quả tới sau `policy_bump`, sau `admin_decide`, sau xóa/gỡ hoặc sau khi bị bản mới thay thế | Từ chối `StalePolicy` / `Overridden` / `TargetGone` / `Superseded` theo đúng thứ tự guard | Không đổi |

## Acceptance và bằng chứng

| AC | Source / kiểm thử thực tế |
| --- | --- |
| AC-01 | `EvaluationTarget`, `EvaluationTicket`, `RawEvaluatorOutput`, `EvaluatorResult`, `WorkModeration::apply_evaluator_result` → `Publication::record_review`; `domain::moderation::tests` |
| AC-02 | `ScriptedEvaluator`, `ScriptKind::ALL`, `CanonicalEvaluatorVectors`; `application::moderation::tests` + `quint_evaluator_vectors_replay_publication_steps_and_check_comment_contract` |
| AC-03 | `validate_evaluation_result` + `unknown_duplicate_cross_target_wrong_revision_wrong_digest_stale_policy_and_superseded_refuse_without_mutation` |
| AC-04 | `EvaluationId`, `DecisionVersion`, `timeout_and_provider_error_consume_the_attempt_without_publishing_and_require_fresh_evaluation_id` |
| AC-05 | `AdminAuthority` (doctest `compile_fail` + lawful twin), `AdminDecision`, `DecisionReason`, `admin_override_requires_narrow_authority_reason_and_expected_version_and_blocks_late_ai_callbacks`, `the_admin_late_result_counterexample_is_refused_by_the_production_moderation_guard` |
| AC-06 | `approval_with_withdrawn_intent_or_inactive_lifecycle_never_publishes_and_r1_stays_public_while_r2_fails`, `the_withdrawn_intent_counterexample_is_refused_by_the_production_publication_guard` |
| AC-07 | `xtask/src/discussion.rs` (`FILTERS` + `REPLAYS`): 46/51 bước M1 chạy qua `Publication` + `WorkModeration`, 21 bước `evaluator-vectors.json` tách rõ 12 bước publication consumer và 9 bước comment contract, giữ đủ 2 negative controls |
| AC-08 | Giữ nguyên `domain/discussion/admission.rs` và `domain/discussion/post_review.rs` (`#148`); ghi rõ các phần `NOT_IMPLEMENTED` bên dưới |

## Phần còn `NOT_IMPLEMENTED` sau `#151`

- **Durable evaluation jobs & apply fencing (`#152+`):** `OutboxEvent` hiện chỉ mang `comment_id`;
  chưa có bảng PostgreSQL cho evaluation jobs, decision history của tác phẩm hay worker consumer
  gọi `ModerationEvaluator` ngoài lock rồi commit kèm standing fence.
- **Admin authority provisioning:** `AdminAuthority` mới là capability hẹp trong bộ nhớ; chưa có
  bảng grant, lệnh CLI hay route HTTP cấp/kiểm tra quyền admin cho tác phẩm ở commit boundary.
- **Comment-edit consumer (`#154`):** `EvaluationTarget::CommentRevision` mới kiểm tương thích hợp
  đồng/script trên 9 bước của `evaluator-vectors.json`; bình luận đang chạy vẫn theo `premoderation.v1`.
- **Appeal & notification (`#153+`):** chưa có case phản đối, receipt hay kênh thông báo (MOD-10).
- **Production AI & MOD-06:** chưa có provider AI thật, bộ đánh giá tiếng Việt hay cơ chế bật/tắt
  auto-approval ở production (`/health` và worker tiếp tục báo `ai_moderation: not_implemented`).
