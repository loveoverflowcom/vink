# Admission bình luận theo policy có version — lát DISC-01 (#120)

**Ngày:** 09/11/2026. **Nguồn:** epic [#120](https://github.com/loveoverflowcom/vink/issues/120),
lát DISC-01 do người dùng giao thực thi (PR đầu). **Baseline:** `develop@47131a9`.
Đây là lựa chọn triển khai có phạm vi **pure core**: không phê duyệt delta COM-02/D-08, không
chuyển D-05 sang Accepted, không bật đăng-trước. Bình luận được lưu vẫn luôn `pending`.
Lát tiếp theo nối đường gửi vào seam này: [§7 DISC-02/03](#7-lát-disc-0203--đường-gửi).
Registry bằng chứng và gap: [bình luận hai tầng](../development/discussion-core.md#registry-traceability-của-41);
brief hành vi UI cho thiết kế sau: [discussion-submission-brief](../design/discussion-submission-brief.md).

## 1. Baseline đối chiếu tại HEAD thực thi (AC-01)

Các khẳng định ở §1 của epic được đọc lại trên source, không suy từ lời issue:

| Khẳng định | Source tại `47131a9` | Kết luận |
| --- | --- | --- |
| Người viết đã thấy own pending/rejected | `VISIBLE_TO_VIEWER` trong `persistence/discussion.rs`: `removal IS NULL AND (review = 'approved' OR author_id = $2)`; `visibility::view` | Đúng; giữ làm regression, không dựng lại |
| Preview reply đã batch | `unnest($1::uuid[]) … CROSS JOIN LATERAL … LIMIT $3` trong cùng file | Đúng; chưa có số đo (DISC-05) |
| Paging ASC, mặc định 20/tối đa 50, preview 3 | `page.rs`: `DEFAULT_PAGE`, `MAX_PAGE`, `PREVIEW_REPLIES` | Đúng; keyset không là snapshot xuyên request |
| Grant kiểm duyệt kiểm lại tại commit, tự né | `discussion_moderator_grants … FOR SHARE`; `policy::authorize_moderation` | Đúng |
| Đình chỉ chặn người gửi | `http/mod.rs` `Actor`: principal bị đình chỉ thành `None` **khi resolve request**; transaction gửi không đọc lại | Chỉ là kiểm trước transaction. Fence tại commit vẫn là blocker D-05 |

**Audit hardcode `pending` (bàn giao AC-09 cho DISC-02/03).** Trạng thái đầu hiện được viết cứng ở:
`PgStore::submit_comment` (`CommentState::SUBMITTED` khi INSERT), `Vink::submit_comment`
(`SubmitOutcome::Created` → `OwnStatus::Pending`), doc của `SubmissionDecision::Insert`, fixture
`discussion.comment.submit.response.json` (`"status": "pending"`), nhánh copy “Đang chờ duyệt” của
`apps/vink-web/src/discussion_state.rs` và index hàng đợi `WHERE review = 'pending'`. Bảng
`comments` chưa có cột provenance (policy, reason). Các chỗ này **vẫn đúng** vì `premoderation.v1`
là policy hiệu lực duy nhất. Trước khi một policy khác được chọn được, DISC-02/03 phải đưa INSERT
qua `Admission::initial_state`, lưu provenance và trả trạng thái thật ở replay/HTTP/consumer.

## 2. Phản biện trước khi triển khai

| Finding | Hậu quả nếu bỏ qua | Quyết định của lát |
| --- | --- | --- |
| `pending` là hằng số rải ở domain, store, application, fixture | Đổi policy phải sửa nhiều lớp; dễ còn một lớp nói “pending” khi đã duyệt | Một quyết định thuần có version; audit các lớp ngoài ở §1, chưa đổi chúng |
| “Email đã xác minh → đáng tin” | Địa chỉ đã xác minh không chứng minh lịch sử sạch | `Trust` chỉ đến từ authority phía server; không có giá trị suy từ email, số vote hay độ dài |
| Thiếu/hỏng tín hiệu dễ thành “approved” hoặc “0” | Lỗi provider mở cửa đăng-trước | Mọi fact thiếu, cũ, tương lai, bị thu hồi, của payload khác hoặc bị gắn cờ → `pending`; standing không chắc → **từ chối**, không ghi hàng pending |
| AI là tín hiệu không đáng tin (MOD-05) | Prompt injection/hallucination tự duyệt | Classifier chỉ có thể giữ lại (flag), không bao giờ làm sạch; chỉ rules tất định có version mới làm sạch |
| Hậu kiểm muộn ghi đè quyết định người | Job cũ ẩn bình luận người đã duyệt lại, hoặc hồi sinh bình luận đã gỡ | Ticket mang version quan sát; chỉ rút duyệt tại đúng version; gỡ là terminal; không bao giờ duyệt |
| Shadow dễ rò thành hiệu lực | Đo thử mà vô tình bật đăng-trước | `PolicySelection` chỉ dựng được với premoderation hiệu lực; Kani + negative control Quint |

## 3. Lựa chọn và phương án loại

| CHOICE (#120) | Chọn cho lát này | Lý do / đánh đổi / bằng chứng còn thiếu |
| --- | --- | --- |
| CHOICE-01 Risk-policy rollout | **A** — admission thuần có version + đánh giá shadow; tiền kiểm 100% vẫn hiệu lực | B cần authority trust có thu hồi, fence standing tại commit, worker hậu kiểm, provenance bền và delta D-08; chưa có thứ nào. A không đổi hành vi người dùng, cho số liệu so sánh khi có store. Không auto-approve dự phòng |
| CHOICE-06 Trình bày gửi/thử lại (chỉ văn bản) | **A** — giữ kết quả gửi có scope cạnh composer tới khi xác nhận/đối soát | Khớp controller D2T-03 hiện có (receipt riêng, `#reply-recovery`). B (hàng local theo submission rồi dedup theo ID server) hợp di động hơn nhưng cần merge/paging/session/restart state; để designer xét lại. Không code UI, không ép UUIDv7 |
| CHOICE-02 public/private delivery | OPEN → DISC-02 | Lát này không đổi read model hay contract |
| CHOICE-03 aggregate, CHOICE-04 cache/validator | OPEN → DISC-05 | Chưa có benchmark; giữ count-on-read và đọc DB trực tiếp |
| CHOICE-05 worker wakeup | OPEN → DISC-04 | Ticket hậu kiểm là contract thuần; chưa outbox, lease hay worker |

## 4. Contract của production seam

Owner: `services/vink-app/src/domain/discussion/admission.rs`. Không I/O, không đọc clock, không
nhận trường nào từ client. Không có route hoặc store gọi seam trong lát này.

- `AdmissionPolicy`: `premoderation.v1` (baseline COM-02/D-08) và `trusted-cohort.v1` (**ứng viên,
  PROPOSED**). Đổi luật = thêm biến thể với code mới; code cũ giữ nghĩa cho provenance đã lưu.
- `decide_initial_review(policy, AdmissionFacts)`: `standing` khác `Active` → `Refuse(Suspended |
  StandingUnknown)` dưới mọi policy. `premoderation.v1` → `Pending/policy_premoderation`.
  `trusted-cohort.v1` báo guard hỏng **đầu tiên** theo thứ tự: thread bị giữ duyệt → bất kỳ flag
  nào (rules hoặc classifier, mọi payload/tuổi) → trust (`unknown`, `not_trusted`, `revoked`, quá
  tuổi/tương lai) → rules (`unassessed`, payload khác, quá tuổi/tương lai) → `Approved/trusted_and_clear`.
- Độ tươi: `at <= now` và `now - at <= max_age` bằng `checked_sub`; tuổi tràn `i64` là quá cũ.
  Ngưỡng **ứng viên** chỉ để đo shadow: attestation 24 giờ, rules 5 phút. Không phải SLA.
- `Admission`: `policy`, `review`, `reason` (code ổn định), `initial_state()` (present, version 1),
  `post_review()` → `PostReviewTicket` chỉ khi được duyệt.
- `PolicySelection`: chỉ `PREMODERATION` hoặc `shadowing(candidate)`; trường private, có doctest
  `compile_fail` + lawful twin. `decide_admission` trả quyết định hiệu lực và quyết định shadow
  riêng; shadow không bao giờ là trạng thái lưu.
- Hậu kiểm: `decide_post_review(ticket, state, finding)`. `Clear` → `Keep` (không ghi). `Flagged`
  hoặc `Unresolved` (hết retry/quá hạn, MOD-03: **fail closed**) → `CommentState::hold_for_review`
  tại version của ticket: approved → pending, version + 1; đã ẩn ở đúng version → `Unchanged`;
  version khác → `Stale` (quyết định người thắng); đã gỡ → `Removed`.

**Authority của hậu kiểm (đề xuất):** capability dịch vụ của App (worker), không phải phiên ủy
quyền của người dùng; chỉ hành động qua ticket và quyết định thuần trên; không thể duyệt; quyết
định người luôn thắng nhờ version. Người viết đăng xuất không hủy hậu kiểm của bình luận đã commit.

## 5. Delta COM-02/D-08 đề xuất — PROPOSED, chưa Accepted

Bật `trusted-cohort.v1` làm policy hiệu lực chỉ hợp lệ khi **đủ cả**:

1. Chủ sản phẩm ghi quyết định delta D-08 và sửa câu chữ COM-02 tương ứng.
2. Authority trust phía server: cấp/thu hồi có audit và provenance; không từ email, vote hay độ dài.
3. Standing đọc trong transaction gửi (fence đình chỉ tại commit của D-05).
4. Bộ rules tất định có version cho tín hiệu nội dung; classifier chỉ được giữ lại.
5. Provenance (policy, reason) bền trong `comments`; replay/HTTP/consumer trả trạng thái thật (AC-09).
6. Outbox + worker hậu kiểm (DISC-04): lease/fencing, at-least-once, ticket theo version, `Unresolved`
   → hold, dead-letter, tuổi hàng đợi; MOD-06 có bộ đánh giá tiếng Việt và chế độ tắt.
7. Số liệu shadow: tỷ lệ shadow-approve so với quyết định người, false positive theo loại, tuổi
   hàng đợi và thời gian duyệt trong pilot thật.
8. Kill switch = quay về `PolicySelection::PREMODERATION`; bình luận đã duyệt trước đó **không bị ẩn
   hồi tố**, ticket còn nợ vẫn được hậu kiểm (model `rollout` giữ đúng điều này).
9. Thay đổi code có review thêm constructor chọn ứng viên làm hiệu lực.

Thiếu bất kỳ mục nào: giữ tiền kiểm, không có đường vòng auto-approve.

## 6. Bằng chứng của lát và phần chưa có

| Lớp | Bằng chứng | Giới hạn |
| --- | --- | --- |
| Unit (`quick`) | `admission::tests`: bảng chân trị từng guard, biên độ tươi gồm `i64::MIN/MAX`, classifier không làm sạch, shadow không hiệu lực, hậu kiểm, code provenance khác nhau, vét cạn 24.276 tổ hợp facts × policy với oracle độc lập; `lifecycle::tests::a_hold_withdraws_only_the_approval_it_observed` | Finite enumeration, không phải proof |
| Quint (`discussion`) | `admission.qnt`: simulation seed 41, 1.000 × 40; scenario `rollout` 45 transition đạt đủ 13 reason/refusal và 9 outcome; ba mutant có counterexample thật (shadow rò, trust quá tuổi, hậu kiểm không fence) | Commit tuần tự hóa là giả định; không model checking |
| Replay Rust (`discussion`) | `admission_replay`: input model qua `decide_admission`/`decide_initial_review`, `ModerationAction::apply`, `remove_by_author`, `decide_post_review`; ba counterexample bị production từ chối. Tạm bỏ kiểm tuổi trust, bỏ fence version của hold hoặc áp shadow làm replay đỏ tại `rollout step 34`, `23`, `2` | Shell bộ nhớ thay authority/store/worker |
| Kani (`kani`) | `admission_approves_only_when_every_guard_passes`, `shadow_selection_keeps_premoderation_effective`, `post_review_only_withdraws_the_observed_approval`: mọi instant `i64`, hai digest 32 byte ký hiệu, mọi trạng thái/version; oracle tuổi bằng `i128` | Một quyết định mỗi harness; không chuỗi, khóa hay I/O |

**NOT_RUN / chưa có:** PostgreSQL và Web (lát không đổi SQL, migration, API hay UI), Apalache.
Chưa có authority trust, rules engine, classifier, provenance bền, outbox/worker, rate limit/chống
spam có owner, overlay public/private, counter/cache. AC-02 và AC-07 có bằng chứng trong phạm vi
trên; AC-01 chỉ cho owner thảo luận; AC-08 được giữ cho lát này (bằng chứng tách lớp, NOT_RUN ghi
rõ); AC-03..06 và AC-09..13 thuộc các lát sau; AC-14 một phần (bảng này và registry). Không tick
checklist của epic từ PR này.

## 7. Lát DISC-02/03 — đường gửi

**Nền:** nhánh DISC-01 (`08b44ff`, PR #126). Lát này xử lý audit hardcode ở §1 cho **lệnh gửi**;
read model public/private (CHOICE-02), số đo truy vấn và giới hạn cursor/HTTP (AC-10) để phần sau.

| Thay đổi | Owner | Vì sao / phương án loại |
| --- | --- | --- |
| `decide_submission` chạy admission **trước** mọi gate: standing khác active → `Refused`, kể cả khi đã có receipt; `Insert` mang `Admission` (và shadow, chưa lưu) | `domain/discussion/submission.rs` | Một quyết định thuần cho cả placement lẫn trạng thái đầu. Đặt receipt trước standing sẽ cho người bị đình chỉ replay; vô hại về dữ liệu nhưng trái “thiếu standing → từ chối” |
| `PgStore` ghi `Admission::initial_state` + `admission_policy`/`admission_reason`; application trả `status` từ trạng thái đã lưu, không còn `OwnStatus::Pending` cố định | `persistence/discussion.rs`, `application/discussion.rs` | AC-09. Không thêm provenance vào HTTP: mã là audit nội bộ (brief UI) |
| Fence standing: `principals FOR UPDATE` rồi đọc đình chỉ ở câu lệnh mới, dùng chung với lưu thẻ nháp | `persistence/principal.rs::locked_standing` | Xung đột với `FOR KEY SHARE` mà FK của `principal_suspensions` lấy: mọi cách chèn đình chỉ đều tuần tự với lệnh gửi. Advisory lock bị loại vì phải sửa mọi writer đình chỉ |
| Thứ tự khóa: publication → thread → target (đều share) → principal → receipt → insert | cùng file | Principal sau các khóa share: moderator/người viết đang giữ hàng bình luận rồi chạm principal của mình qua FK không chờ ngược một lệnh gửi đang chờ hàng đó. Principal trước receipt/insert: tránh nâng cấp `FOR KEY SHARE` → `FOR UPDATE` (deadlock giữa hai lệnh gửi của cùng người) |
| Migration `20261111000000_comment_admission` | `migrations/` | Backfill `premoderation.v1`/`policy_premoderation` đúng sự thật cho mọi bình luận cũ; bỏ default nên writer phải nêu provenance; CHECK cặp mã đã biết; CHECK **chỉ** `premoderation.v1` (gỡ nó thuộc delta D-08, §5 mục 9); trigger buộc hàng mới present, version 1, approved chỉ khi `trusted_and_clear`, và provenance bất biến |

**Đánh đổi đã chấp nhận:** các lệnh gửi của **cùng** một principal tuần tự hóa trên hàng
principal, và chờ cả lần lưu thẻ nháp của người đó (và ngược lại); người khác không bị ảnh hưởng.
Xem lại nếu đo thấy chờ đáng kể. Gỡ, bật/tắt và quyết định kiểm duyệt **chưa** có fence đình chỉ
tại commit; Identity thu hồi phiên nhanh tới đâu vẫn là việc của D-05.

**Migration — lock budget và backout:** `ADD COLUMN` có default hằng chỉ đổi catalog; CHECK và
trigger giữ `ACCESS EXCLUSIVE` trên `comments` trong lúc kiểm hàng cũ, ngắn với cỡ dữ liệu pilot.
Backout: xóa trigger, hàm, hai constraint và hai cột; không đối tượng nào khác phụ thuộc. Không
sửa migration đã áp dụng.

**Bằng chứng của lát** (PostgreSQL 17.11 cục bộ):

| Claim | Kiểm |
| --- | --- |
| Schema tự giữ provenance và trạng thái sinh ra | `admission_provenance_is_recorded_and_held_by_the_schema`: SQL trực tiếp — approved/rejected/version 2/đã gỡ khi sinh ra, ứng viên dù nhất quán, cặp mã lạ, thiếu cột (`23502`), sửa provenance đều bị từ chối; dạng hợp lệ đi qua cùng đường; duyệt vẫn tăng version |
| Đình chỉ tại commit, không ghi gì | `a_suspended_writer_is_refused_at_commit_and_nothing_is_written`: bình luận đầu (binding thread cũng rollback), retry đúng key, reply, và HTTP `403 principal_suspended` `no-store` |
| Race hai thứ tự qua store thật | `suspension_and_submission_serialize_on_the_principal`: đình chỉ chưa commit → lệnh gửi chờ rồi bị từ chối; lệnh gửi reply-to-reply đang giữ principal (dừng ở FK tới root) → lệnh `suspend` thật chờ, bình luận commit trước, lệnh sau bị từ chối |
| Nâng cấp dữ liệu cũ | `upgrade_records_premoderation_for_existing_comments`: migrator tới trước lát này, ba hàng cũ (pending, approved v2, đã gỡ) → provenance premoderation, review/removal/version/chữ giữ nguyên |
| Negative control | Thay fence bằng `Standing::Active`: hai test đình chỉ FAIL (assert từ chối; race hết hạn chờ “no backend became blocked”), khôi phục byte-exact |

Unit/Kani/replay cập nhật theo seam mới: bảng vét cạn `decide_submission` thêm trục standing;
`submission_receipts_take_precedence_and_inserts_pass_every_gate` kiểm từ chối standing đứng trước
receipt và insert luôn mang admission premoderation dù shadow nói gì; `lifecycle_replay` dùng
`Admission::initial_state` (fixture Quint không đổi vì model không có đình chỉ).

