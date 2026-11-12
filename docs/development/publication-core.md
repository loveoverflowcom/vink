# Publication intent core — PUB-CORE-01 (#150)

**Phạm vi:** Rust thuần + application planning seam, chưa publication runtime. Nguồn luật là
AUD-01/02, PUB-01/02/05, MOD-05 và INV-01/02/07/08 trong [PRD](../../prd/requirements.md),
[đặc tả](../architecture/publication-moderation.md), [DecisionDelta](../architecture/product-decisions-20261110.md).
Không đổi D-06/D-07/D-12 thành Accepted và không quyết định các điểm OPEN.

## Owner và lựa chọn

- **CHOICE-01 B:** aggregate `Publication`, private fields, `restore(PublicationFacts)` fallible.
  Dữ liệu được kiểm work/revision, duplicates, intent/generation, approval/pointer và terminal
  consistency trước khi dùng. Đánh đổi: restoration API lớn hơn hàm facts đơn, đổi lại caller
  không thể thay state bằng field setter hoặc Deserialize. Restore kiểm cấu trúc, không chứng
  minh authenticity/audit của một adapter cung cấp facts.
- **CHOICE-02 B:** `application::publication::plan_publication` là owner-local use case nhỏ,
  xác thực bound principal/standing rồi gọi `Publication::decide_author`. Có test caller thật cho
  r1/r2/private/stale. Chưa cần thêm command port khi chưa có persistence consumer; không đổi
  public-read port `PublicationStore` hiện hữu và không có generic save-state API.
- **CHOICE-03 B:** chuyển canonical M1 về `behavior/publication`, giữ toàn bộ scenario và hai
  negative controls; runner chung và pin Quint không đổi. Replay subset theo bảng action trong
  [owner README](../../services/vink-app/behavior/publication/README.md), không claim whole model.

Baseline kỹ thuật được chọn, **không nâng thành PRODUCT_CONFIRMED**: tạo private ở generation 1;
accepted audience command luôn tăng generation kể cả reaffirmation; stale luôn từ chối; overflow
không plan. Inactive lifecycle chỉ là facts đầu vào, không có lệnh khôi phục/xóa/gỡ ở PR này.

## API tối thiểu cho #151

Source: [`domain/publication.rs`](../../services/vink-app/src/domain/publication.rs) và
[`application/publication.rs`](../../services/vink-app/src/application/publication.rs).

| API/type | Contract |
| --- | --- |
| `Snapshot` | `WorkId` + `RevisionId` + canonical `RevisionContent`; hash v1 giữ nguyên |
| `PublicationFacts` / `Publication::restore` | Audience, lifecycle, generation, exact intent, public pointer với generation đã cho phép nó, revision-bound moderation facts |
| `AuthorCommand::SaveSnapshot` | Thêm immutable snapshot Unsubmitted; không evaluation, không sửa pointer/generation |
| `RequestPublic { revision, expected }` | Owner + active + existing exact revision + current generation; rejected không plan; Unsubmitted thành Pending + EvaluationRequest; Pending/NeedsAdmin chỉ chờ; Approved promote |
| `SetPrivate { expected }` | Tăng generation; clear intent/pointer; giữ content và moderation facts |
| `AuthorTransition` / `Effects` | State mới, closed outcome, tối đa một revision/hash evaluation request và typed pointer effect; chưa dispatch/commit |
| `ReviewedRevision` / `record_review` | **Đầu vào đã admitted**, work/revision/hash, expected moderation và ReviewOutcome; kiểm binding/lifecycle/current fact rồi giữ quyết định và chạy guard cho đúng revision |
| `promotion_eligible(PublishIntent)` | Active + Public + exact current revision/generation + Approved |
| `readable_revision()` | Active + Public + pointer của revision Approved; không đòi pointer = current intent |

`ReviewedRevision` **không phải evaluator output, capability hoặc quyền admin**. #151 phải xác minh
schema, job, policy, decision version, admin override và reason trước khi gọi; `expected: Moderation`
không chống decision-version ABA và không thay được version guard của #151. Input không mang raw
provider result hay authority bit. Review một revision khác current intent chỉ lưu fact; không
được promote revision khác như side effect. Quyết định không-Approved trên revision đang public
clear đúng pointer đó; reject r2 không clear r1. Đây là application/domain seam, không có service
consumer, provider, admin role/grant hay route mới.

## Promotion khác public read

| Facts | Promote | Đọc hiện hành |
| --- | --- | --- |
| r1 Approved + pointer r1, intent r2 Pending/Rejected/NeedsAdmin, Public/Active | r2 không đủ; r1 không được promote bằng intent r2 | r1 |
| r2 Approved + exact current intent r2 | r2 | r2 sau transition |
| Private, không intent, callback Approved của r2 đã admitted | Không | Không; quyết định Approved được giữ |
| Intent generation cũ, revision lạ/khác work, owner sai | Refusal, không plan/effect | Không đổi state cũ |
| AuthorDeleted/Removed | Không | Không; không có lệnh hồi sinh |

## Acceptance và evidence

| AC | Source/check thật |
| --- | --- |
| AC-01 | `PublicationFacts`, hai variant Audience, typed generation/intent/pointer/lifecycle, per-revision Moderation; restore tests |
| AC-02 | SaveSnapshot effects NONE, exact revision/hash EvaluationRequest; unit + author replay |
| AC-03 | `readable_revision` vs `promotion_eligible`; r1/r2 table, private retains decisions; unit + application + replay |
| AC-04 | fallible restoration, wrong owner/work/hash/revision, duplicate, stale/reaffirmation, overflow; tests trả Err không transition/effects |
| AC-05 | withdrawn intent unit + NC replay; terminal fixture guards; không claim delete/tombstone runtime |
| AC-06 | `plan_publication` gọi core thật trong use-case test; PublicationStore, revision/hash v1 và metadata code giữ nguyên |
| AC-07 | Canonical M1, required FILTERS/REPLAYS, hai tên test bắt buộc và exact coverage counts; action matrix owner README |
| AC-08 | Registry/feature map/AGENTS/đặc tả tách pure core với runtime gap |

Môi trường kiểm: macOS arm64, rustc 1.99.1, Node 22.15.0, npm 10.9.2, Quint 0.33.1.
Base `origin/develop@e16b1cc94aa87aa90aab145a133b5a1d7425933c`; worktree
`/Users/manhblue/Documents/Codex/2026-11-10/task-9/vink`, branch `codex/publication-intent-core`.
Inputs là base cộng diff #150 (publication domain/application/tests, canonical model relocation,
runner selection và tài liệu). Implementation SHA nằm ở PR để không tự tham chiếu commit.
Build dùng `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` để giảm
artifact trên máy ít dung lượng; không đổi source/profile repository. npm dùng cache local có sẵn (`npm_config_offline=true npm_config_cache=/tmp/vink-task8-npm-cache`).
Lượt quick cuối chạy với quyền Unix socket loopback/local cho Identity test. Các lượt trước đã
**FAIL** vì sandbox chặn socket và vì hết dung lượng ở archive Rust; đã dọn đúng orphan object /
test binary cũ trong target của task, giới hạn 2 build jobs rồi chạy lại lane đầy đủ. Không bỏ test.
Logs local: `/tmp/vink150-focused.log`, `/tmp/vink150-quick-final.log`,
`/tmp/vink150-discussion-final.log`; FAIL cũ ở `/tmp/vink150-quick.log`,
`/tmp/vink150-quick-local-sockets.log`, `/tmp/vink150-discussion.log`.

| Check | Kết quả | Boundary |
| --- | --- | --- |
| `cargo test --locked -p vink-app --lib publication` | PASS (9 tests; cũng được lane quick chọn trên diff cuối) | Pure core, application caller và scoped replay |
| `cargo xtask verify quick` | PASS (9/9 bước, 47 doctest, 579 unit/contract tests) | Repo, server/wasm clippy, doctests, unit/contract |
| `cargo xtask verify discussion` | PASS (5/5 bước, 62 tests, đủ 13 required replays) | Baseline models + canonical M1/target drift + required production replays |
| PostgreSQL / HTTP / browser / Kani | NOT_RUN | Ngoài scope #150; không SQL/UI/Kani change |
| GitHub CI | NOT_RUN, NOT_REQUIRED | Không đọc/poll Actions/checks |
| Production | NOT_RUN | Không deploy/activation |

## Tích hợp tiếp

[#151](moderation-core.md) (`domain::moderation`, `application::moderation`) nối contract evaluator,
`ScriptedEvaluator` (9 lớp `evaluator-vectors.json`), guard precedence, `DecisionVersion` và
`AdminAuthority` vào `Publication::record_review`, mở rộng replay M1 lên 46/51 bước cùng cả hai
negative control (`mutatedIntentStep`, `mutatedOverrideStep`).

## Delete/remove lifecycle và tombstone access — PUB-LIFECYCLE-01 (#157)

**Phạm vi:** Core delete/remove lifecycle, typed deletion provenance, và typed access projection
(`PublicBody(Snapshot)` | `AuthorTombstone { last_pointer }` | `Unavailable`), cùng tầng application
planning kết hợp với `domain::discussion::visibility`.

### Lựa chọn thiết kế

- **CHOICE-01 A (Typed Deletion Provenance):**
  `Lifecycle::AuthorDeleted(DeletionProvenance)` với hai variant:
  `DeletionProvenance::PublicTombstone(PublicPointer)` và `DeletionProvenance::Hidden`.
  Loại bỏ trạng thái vô lý ở mức kiểu dữ liệu (như `Removed` mang tombstone pointer), bảo đảm
  dữ liệu lịch sử thiếu/sai hỏng sẽ fail-closed khi restore.
- **CHOICE-02 A (Typed Access Plan):**
  `Publication::access_plan()` trả về `WorkAccessPlan<'a>` đóng gói logic kiểm tra quyền truy cập:
  `PublicBody(&'a Snapshot)`, `AuthorTombstone { last_pointer }`, hoặc `Unavailable`.
  Các hàm planning ở tầng application (`plan_work_access`, `plan_comment_view`, `check_comment_submission`)
  tiêu thụ kiểu này mà không cần lặp lại điều kiện rẽ nhánh.

### Core transitions & Application Seam

| API / Method | Hợp đồng và hành vi |
| --- | --- |
| `decide_author(Delete)` | Owner + active lifecycle; xoá pointer và intent; chuyển audience sang `Private`; tăng `intent_generation`; ghi nhận `DeletionProvenance::PublicTombstone` nếu tại thời điểm xóa bài có readable revision approved, hoặc `Hidden` nếu chưa từng công khai hoặc đang riêng tư |
| `Publication::admin_remove()` | Xoá pointer và intent; chuyển lifecycle sang `Lifecycle::Removed`; giữ nguyên audience và generation |
| `WorkModeration::apply_admin_remove` | Cặp `WorkModeration`/`Publication` cùng work (`InvalidFacts` nếu lệch); `AdminAuthority` scoped theo `(admin, work)` và tác giả tự recuse như `apply_admin_decision` (`NotAdmin`; tác giả xoá bằng `Delete` để giữ tombstone hợp lệ); uỷ quyền chuyển đổi sang `Publication::admin_remove` |
| `Publication::access_plan()` | `PublicBody(&Snapshot)` khi active + public + readable pointer; `AuthorTombstone` khi author deleted có tombstone; `Unavailable` trong mọi trường hợp khác (riêng tư, deleted hidden, moderation removed) |
| `plan_work_access` | Application entry point trả về `WorkAccessPlan` |
| `plan_comment_view` | Nếu thread không accessible (`Unavailable`), trả về `CommentView::Unavailable`; nếu là tombstone hoặc public body, chuyển tiếp sang `visibility::view` |
| `check_comment_submission` | Barrier chống gửi bình luận: `PublicBody` cho phép gửi; `AuthorTombstone` trả lỗi `TombstoneReadOnly`; `Unavailable` trả lỗi `WorkUnavailable` |
| `check_own_comment_removal` | Cổng mức tác phẩm cho người viết tự gỡ bình luận: cho phép trên `PublicBody` và `AuthorTombstone` (baseline được xác nhận ở `O-03@r2`, `PRODUCT_CONFIRMED`); `Unavailable` trả `WorkUnavailable`. Lifecycle của bình luận vẫn do `CommentState` quyết định |

### Chuyển canonical M3 và hoàn tất 51/51 replay M1

> Lịch sử của #157. Số bước và scenario hiện hành (M1 64, M3 41, NC mới) ở
> [PUB-HIDDEN-01](#trang-ẩn-và-gửi-duyệt-theo-phiên-bản--pub-hidden-01).

1. **Khử hoàn toàn 5 bước model-only trong M1:**
   Tất cả 51 bước của `publication-vectors.json` (`author`: 24, `ai_result`: 13, `admin_decide`: 6,
   `admin_remove`: 3, `reevaluate`: 4, `policy_bump`: 1) đã chạy 100% qua code production thật của
   `Publication` và `WorkModeration`.
2. **Chuyển canonical M3:**
   `tombstone.qnt`, `tombstone-vectors.json`, và `tombstone-private-as-deleted-counterexample.json`
   đã chuyển từ `services/vink-app/behavior/target/` sang `services/vink-app/behavior/publication/`.
3. **Replay 30/30 bước M3 & Negative Control:**
   Replay đủ 3 scenario (`author_deleted_public_work` 14 bước, `private_then_deleted` 10 bước,
   `moderation_removed` 6 bước) qua application planning và `Publication`. Mọi lệnh mức tác phẩm,
   kể cả từ chối (`work_unavailable`) và `make_private` lặp (`unchanged` = `Reaffirmed`), đều gọi
   production rồi map kết quả/lỗi; outcome xoá lấy từ provenance production. Shell `publish` của
   model = yêu cầu công khai + một approval đã được admit cho revision 1 (giả định authority; đường
   evaluator/admin thuộc M1). Replay mutant
   `tombstone-private-as-deleted-counterexample.json` chứng minh bài riêng tư bị xoá không bao giờ
   lộ tombstone hay bình luận.

### Bàn giao cho PR runtime (#158 SQL/API, #159 worker/evaluator)

- **#158 Persistence & API:** Cần lưu `DeletionProvenance` trong bảng `publications` hoặc cột trạng thái
  tương ứng, áp dụng standing fence tại commit, tạo route `DELETE /api/v1/works/{id}` (tác giả) và
  `POST /api/v1/admin/works/{id}/remove` (quản trị viên), và tích hợp reader SSR/UI cho trang tombstone.
- **#159 Worker & Evaluator:** Dispatch durable job khi submit revision; evaluator kết nối worker và áp dụng
  quyết định vào publication fact đã serialize.


## Trang ẩn và gửi duyệt theo phiên bản — PUB-HIDDEN-01

**Phạm vi:** `ADMIN-MODERATION-20261111` cho PUB-05 (sửa), PUB-06, PUB-07, COM-11
([DecisionDelta §7](../architecture/product-decisions-20261110.md#7-bổ-sung-11112026--quản-trị-kiểm-duyệt),
[đặc tả §6](../architecture/publication-moderation.md#6-xóa-riêng-tư-gỡ-trang-ẩn-và-tombstone)). Chỉ có
core thuần, application planning, model và replay ở tài liệu này; phạm vi runtime nằm bên dưới.

### Core

- `PublicationFacts` thêm `hidden` (pointer cuối khi tác giả chuyển một bài đang công khai sang riêng
  tư), `ever_public` (các revision từng đọc được) và `public_epoch` (`PublicEpoch`, tăng mỗi lần bài
  trở lại công khai).
- `WorkAccessPlan::AuthorHidden { last_pointer }`: URL cũ chỉ hiện “Nội dung đã được ẩn” và bình luận
  đủ điều kiện; thân bài, metadata, snippet và listing biến mất. Bài chưa từng công khai hoặc bị admin
  gỡ vẫn là `Unavailable`.
- Xóa từ trang ẩn của bài từng công khai để lại tombstone (`DeletionProvenance::PublicTombstone`), giữ
  bình luận (đóng O-05); bài chưa từng công khai không để lại gì.
- `Publication::should_notify_author`: khi công khai lại chỉ báo sự kiện mới hơn `public_epoch` hiện
  hành (không báo quyết định đã có trong lúc ẩn).
- Gửi duyệt theo phiên bản (PUB-06) do `WorkModeration::decide_author_with` sở hữu: chỉ yêu cầu
  công khai mở ca; chuyển riêng tư thu hồi job (`JobState::Withdrawn`) và báo caller đóng ca/phản đối;
  công khai lại dùng quyết định còn hiệu lực đúng snapshot + policy, policy đổi → lượt mới
  ([moderation-core](moderation-core.md#admin-moderation-core--admin-core-01)).

### Application

- `check_comment_submission` trả `TombstoneWriteError::HiddenReadOnly` trên trang ẩn (đọc, xử lý ca đã
  gửi và tự gỡ vẫn được; tạo mới/sửa/tương tác bị chặn). `TombstoneWriteError::as_str` cho mã ổn định.
- `plan_comment_review_context`: người duyệt bình luận chỉ thấy thân bài khi bài đang công khai
  (`WithWork`), còn lại `CommentOnly`.

### Bằng chứng

M1 64/64 bước (`policy_rereview` mới, NC `pointerMovesLawfully`) và M3 53/53 bước (5 scenario, gồm `ReviewWithdrawn`; NC
`noExceptionForUnavailableWork`, `noNoticeFromHiddenPeriod`, `bodyOnlyWhilePublic`) replay qua production; xem
[behavior/publication](../../services/vink-app/behavior/publication/README.md). Unit test trong
`domain::publication::tests` và `application::publication::tests`.

### Persistence gap (#158 → đợt tích hợp)

#159 ([moderation-runtime](moderation-runtime.md)) thêm migration cho `hidden_*`, `review_withdrawn_*`,
`ever_public`, `public_epoch`, policy theo từng quyết định và trạng thái job `withdrawn`/`rerouted`;
`load_paired_state` đọc đủ các fact này. Phần hoàn thiện sau #170 phục vụ trang ẩn qua reader
API/SSR/Web (`200`, `noindex`, `no-store`), chặn bình luận mới và giữ đọc/tự gỡ bình luận.
`set-private` rút job, đóng ca và phản đối trong cùng commit. Xóa bài/tombstone và admin remove
vẫn chưa có route/runtime; không suy rộng bằng chứng reader thay thế sang các đường này.
