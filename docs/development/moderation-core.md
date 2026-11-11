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
- **Appeal & notification (`#153+`):** core thuần của phản đối có từ `ADMIN-CORE-01` bên dưới; chưa
  có bảng, receipt lưu bền hay kênh thông báo (MOD-10).
- **Production AI & MOD-06:** chưa có provider AI thật, bộ đánh giá tiếng Việt hay cơ chế bật/tắt
  auto-approval ở production (`/health` và worker tiếp tục báo `ai_moderation: not_implemented`).

## Admin moderation core — ADMIN-CORE-01

**Phạm vi:** `ADMIN-MODERATION-20261111` ([DecisionDelta §7](../architecture/product-decisions-20261110.md#7-bổ-sung-11112026--quản-trị-kiểm-duyệt),
[đặc tả §5–§8](../architecture/publication-moderation.md#5-quản-trị-viên-moderator-và-phản-đối)):
admin tổng, grant moderator, nhận ca, can thiệp, phản đối, dừng tự động và điều tra riêng (MOD-09..15,
PUB-06). Chỉ có **Rust thuần + model M4 + replay**; không migration, store, API/CLI, worker hay UI.
Tên kiểu, mã từ chối và baseline kỹ thuật là `DESIGN_PROPOSED`; luật là `PRODUCT_CONFIRMED`.

### Module và hợp đồng

| Module | Hợp đồng chính |
| --- | --- |
| [`authority`](../../services/vink-app/src/domain/moderation/authority.rs) | `ModerationDirectory::restore(DirectoryFacts)` kiểm grant/nhóm trùng, grantee nhóm không tồn tại, grant kế thừa sai dạng (`InvalidInheritance`: chỉ `Comments` toàn Vink cho một principal). `resolve` trả `VerifiedAuthority` (không constructor công khai; doctest `compile_fail` + lawful twin) theo thứ tự: tạm khóa → không có cơ sở (`NotAuthorized`) → tự né nội dung của mình hoặc bình luận trên bài của mình (`Recused`, kể cả admin). Grant `Works`/`Comments` độc lập, toàn Vink hoặc một `WorkId`, hết hạn tại `expires_at`, thu hồi giữ dòng audit; nhóm xét theo thành viên tại lúc kiểm. `grant`/`revoke`/`set_membership` kiểm lại `AdminSeat`, trả `DirectoryAudit`; lặp lại → `Unchanged` |
| [`case`](../../services/vink-app/src/domain/moderation/case.rs) | `ReviewCase` cho một `ReviewSubject` chính xác (revision + digest + `DecisionVersion`, hoặc bình luận + `ModerationVersion`). Mọi lệnh được chấp nhận tăng `CaseVersion`; phiên bản cũ → `StaleCase` (hai người tranh ca, đúng một thắng). `claim`/`release`/`note` (`InternalNote` ≤ 2.000 ký tự, không gửi người viết)/`escalate`/`take_over` (chỉ admin, bắt buộc lý do)/`revalidate` (mất mọi cơ sở → về hàng chờ; còn grant khác → giữ ca với cơ sở mới)/`conclude` (fence phiên bản quyết định → `StaleDecision`; phản đối đang mở → chỉ admin; `Reject` cần lý do). `close` đóng ca đang mở; ca đã kết luận/đóng trả `CaseClosed` và không đổi |
| [`appeal`](../../services/vink-app/src/domain/moderation/appeal.rs) | `AppealBook::file`: chỉ chủ nội dung, trên quyết định bất lợi của AI hoặc moderator, đúng phiên bản đã thấy; quyết định admin là cuối (`FinalDecision`); một hồ sơ mỗi phiên bản (retry với key khác trả `Filed::Existing`); `Notice::NotSent` vì không có kênh. `conclude`: chỉ admin, không phải chủ nội dung, bắt buộc lý do, đúng phiên bản; `close_open` khi bài chuyển riêng tư |
| [`automation`](../../services/vink-app/src/domain/moderation/automation.rs) | `Automation::set`: admin, fence `AutomationVersion`, bắt buộc lý do, trả `AutomationAudit`. Khi dừng: việc mới và job đang mở sang hàng chờ người (`WorkModeration::reroute_to_humans`, `JobState::Rerouted`), kết quả tới sau bị `AutomationPaused`; bật lại chỉ áp lượt gửi mới |
| [`investigation`](../../services/vink-app/src/domain/moderation/investigation.rs) | `Investigation::open`: admin không phải tác giả, `ViolationBasis` + `EvidenceSource` (≤ 500 ký tự), đúng ID revision từng công khai của bài đang riêng tư. `read` kiểm lại mọi điều kiện rồi trả `ReadAudit` **trước** snapshot (adapter phải commit audit rồi mới trả); `close` chỉ người mở, ghi kết luận + căn cứ, không sao chép văn bản |
| [`WorkModeration`](../../services/vink-app/src/domain/moderation.rs) | `apply_human_decision` (moderator/admin dưới `VerifiedAuthority`; `DecisionBy::Moderator`), `apply_case_conclusion`, `apply_appeal_conclusion` (tạo phiên bản admin mới; công khai chỉ khi ý định hiện hành trỏ đúng bản), `decide_author_with` (thu hồi job → `JobState::Withdrawn` khi riêng tư, gửi lại, duyệt lại theo policy → `DecisionBy::PolicyRereview`, route sang người khi dừng), `reroute_to_humans`; từ chối `withdrawn`, `automation_paused`, `case_closed` |
| [`discussion::hide`](../../services/vink-app/src/domain/discussion/hide.rs) | Moderator bình luận ẩn đúng bản đã duyệt (`HiddenApproval`) và khôi phục chỉ khi bình luận vẫn đúng trạng thái đã ẩn; gỡ của người viết và gỡ terminal cũ giữ nguyên |

### Bằng chứng

| Lớp | Nguồn |
| --- | --- |
| Model M4 | [`governance.qnt`](../../services/vink-app/behavior/publication/governance.qnt): 4 scenario, 105 bước, 5 negative control (`conclusionNeedsLiveAuthority`, `claimNeverStolen`, `oneAppealPerDecision`, `pausedAiWritesNothing`, `readsAreAuthorized`); thay `target/appeal.qnt` |
| Replay | `domain::moderation::governance_replay` (2 test, trong `REPLAYS` của `verify discussion`): 105/105 bước qua production, chiếu lại toàn bộ state sau mỗi bước; 5 counterexample bị production từ chối |
| Unit test | `domain::moderation::governance_tests`: restore lỗi (kể cả `InvalidInheritance`), phạm vi/hạn/thu hồi/nhóm/tạm khóa/tự né, `resolve_preferring`, audit và `Unchanged`/`UnknownGrant`/`UnknownGroup`/`DuplicateGrant`, `AdminSeat` cũ bị kiểm lại; `InternalNote` rỗng/dài/ký tự điều khiển, `ReviewCase::restore`, tranh ca, `take_over` không lý do, `revalidate` (giữ/đổi cơ sở/mất quyền), `conclude` (`WrongSubject`, `StaleDecision`, `AppealOpen`, thiếu lý do), `close` lặp; mọi từ chối của `AppealBook`; `Automation` (`Stale`, `Unchanged`, `ReasonRequired`, `VersionExhausted`); `Investigation` (`NeverPublic`, `Conflict`, `Expired`, `WrongRevision`, `WrongWork`, `NotInvestigator`, `ConditionLapsed` sau khi công khai lại, `close` lặp); `discussion::hide::tests` |

### Còn `NOT_IMPLEMENTED`

- Migration/store: grant + nhóm (kế thừa `moderator_grants` thành grant `Comments` toàn Vink), ca + ghi
  chú, hồ sơ phản đối, chế độ tự động có phiên bản, hồ sơ điều tra + audit đọc (commit trước khi trả
  snapshot), bản ghi ẩn bình luận; provenance `moderator`/`policy_rereview` và trạng thái job
  `withdrawn`/`rerouted` trong SQL.
- Runtime: `EvaluatorCapability::evaluator()` và `decide_author` vẫn mặc định `Enabled`; worker chưa đọc
  chế độ tự động từ store; chưa có CLI/API admin cho grant, ca, phản đối, dừng/bật, điều tra.
- Comment evaluator consumer (`#154`): M4 dùng shared validator + lifecycle cho bình luận.
- O-17 (URL cũ sau khi moderator bác bản đang đăng) chỉ là đề xuất tối thiểu; O-18, O-19 còn `OPEN`.
- Design/UI, báo cáo độc giả, quản lý policy, khóa tài khoản: đợt sau.
