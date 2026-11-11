# Registry — quyết định sản phẩm 10/11/2026

**Nguồn:** `PRODUCT-DECISIONS-20261110` · **Quyết định và lựa chọn:**
[kho TOML](../../decisions/README.md) và [DecisionDelta lịch sử](../architecture/product-decisions-20261110.md) · **Model:**
[`services/vink-app/behavior/target`](../../services/vink-app/behavior/target/README.md).

Registry này nối **yêu cầu → model/invariant/scenario → gap implementation** và ghi bằng chứng đã
chạy. Giữ tách bạch **bốn chiều trạng thái**: (1) quyết định sản phẩm (`PRODUCT_CONFIRMED` / `DEFERRED` / `OPEN`),
(2) quyết định kiến trúc (`DESIGN_PROPOSED` / `Provisional` / `Accepted`), (3) mức triển khai
(`core` / `persistence-api` / `worker-provider` / `web` / `cmp` / `NOT_IMPLEMENTED`), và (4) bằng chứng
đã chạy gắn SHA/boundary. Một baseline kỹ thuật trong pure core (như `O-03`) không biến điểm `OPEN`
thành `PRODUCT_CONFIRMED`.

- **Pure core + replay (`#150`, `#151`, `#157`):** hiện thực lõi publication, moderation evaluator/admin
  và tombstone access thuần, `ScriptedEvaluator`, replay đủ 51/51 bước của M1, 30/30 bước của M3 và
  21 bước `evaluator-vectors.json` ([publication-core](publication-core.md), [moderation-core](moderation-core.md)).
- **Persistence + API tác giả (`#158`):** hiện thực migration `20261116000000_work_publication_persistence`,
  `persistence::publication` và HTTP API v1 cho chụp snapshot, `request-public` (ghi job `open`),
  `set-private` và đọc trạng thái của tác giả ([publication-runtime](publication-runtime.md)).
- **Auth backend (`#152`):** hiện thực credential username/mật khẩu native trong Identity + App
  ([ADR D-05 §12](../architecture/auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính)).

Bằng chứng “model” nghĩa là typecheck + simulation có biên + scenario + negative
control của Quint, không phải code, PostgreSQL, API hay trình duyệt. Registry của code đang chạy vẫn ở
[discussion-core](discussion-core.md), [endorsement-core](endorsement-core.md),
[taxonomy-core](taxonomy-core.md) và không bị sửa để khớp hướng đích.

## 1. Yêu cầu → model → gap

Scenario viết dạng `model/scenario`; số trong ngoặc là bước trong `*-vectors.json`.

### 1.1. Audience, xuất bản, kiểm duyệt, quản trị viên (M1)

| Yêu cầu | Invariant (`publication.qnt`) | Scenario / negative control | Implementation và gap |
| --- | --- | --- | --- |
| AUD-01, AUD-02, PUB-01, INV-01 | `publicNeedsEverything`, `pointerMovesLawfully`, `audienceFence` | `audience_moderation_override` (4–12 công khai sau kết quả hợp lệ; 33–35 được duyệt nhưng không công khai; 18, 28 tab cũ `stale`). NC `mutatedIntentStep` | **Pure core #150 + #151 + #157** (replay 51/51 bước M1); **PostgreSQL + HTTP API tác giả #158** (`work_publications`, `/snapshots`, `/publication/request-public`, `/publication/set-private`, `/publication/state`, backfill `legacy_bootstrap`). Chưa có worker tiêu thụ `work_evaluation_jobs` hay Web/CMP UI |
| PUB-02, INV-02 | `failureNeverPublishes`, `decisionsAreVersionBound` | (13–16) r1 công khai khi r2 bị từ chối | **Pure core #150 + #151** (`readable_revision` không đòi current intent = pointer; r1/r2 unit + replay) + **persistence/API #158** (r1 giữ công khai khi chụp/gửi r2, `set-private` xóa con trỏ và trả `404` cả với `If-None-Match` cũ). Chưa có worker/admin route ghi quyết định kiểm duyệt mới |
| MOD-05, INV-08 | `privateSaveNeverEvaluated` | (1–2) lưu riêng không tạo evaluation | **Pure core #150 + #151** (`SaveSnapshot` trả `ticket = None`; `RawEvaluatorOutput` từ chối trường lạ và phân loại output sai thành `InvalidOutput` → `NeedsAdmin`) + **persistence/API #158** (`POST /snapshots` không tạo dòng `work_evaluation_jobs`). Chưa có evaluator worker hay đường gửi nội dung ra ngoài |
| MOD-02, MOD-07, INV-03 | `refusalWritesNothing`, `evaluatorOnlyDecidesPending`, `decisionsAreVersionBound` | (6–12, 21) timeout, trùng, sai revision, job lạ, sai policy; `evaluator-vectors.json` đủ chín loại | **Pure core + mock #151:** `EvaluationTarget`, `EvaluationTicket`, `EvaluatorResult`, `validate_evaluation_result`, `ScriptedEvaluator` (đủ 9 lớp `ScriptKind`); replay 12 bước publication consumer + 9 bước comment contract. `#158` có bảng `work_evaluation_jobs` (chỉ tạo `open` ticket); chưa có bảng kết quả evaluator hay worker consumer |
| MOD-03, MOD-08 | `failureNeverPublishes` | (6, 23) `retryable`, `needs_admin`; `moderation_removal` (3–5) | **Pure core #151:** `Timeout` / `ProviderError` đóng job sang `Failed` (`retryable`), giữ nguyên `Pending` và pointer; `reevaluate` đòi `EvaluationId` mới. Chưa có trần retry DB, watchdog hay UI |
| MOD-09, INV-03 | `adminOverrideWins`, `evaluatorOnlyDecidesPending` | (24–26) admin duyệt, kết quả AI muộn `overridden`; (2) `not_submitted`. NC `mutatedOverrideStep` | **Pure core #151:** `AdminAuthority` hẹp gắn `(actor, work)`, `DecisionVersion`, `DecisionReason`, `apply_admin_decision` + replay NC `mutatedOverrideStep`. Chưa có bảng grant/CLI/HTTP cấp quyền admin bền vững |
| PUB-03, INV-07 | `noResurrection` | (37–40) sau khi xóa: lệnh tác giả, admin và kết quả AI đều không hồi sinh; `moderation_removal` (8–11) | **Pure core #157:** `decide_author(Delete)` và `apply_admin_remove` là transition sản xuất thật, replay đủ 51/51 bước của M1. Chưa có persistence/CLI/route cho delete/remove hay tombstone query (ngoài scope #158) |

### 1.2. Xóa bài và tombstone (M3)

| Yêu cầu | Invariant (`tombstone.qnt`) | Scenario / negative control | Implementation và gap |
| --- | --- | --- | --- |
| PUB-04 | `bodyOnlyWhilePublic`, `tombstoneOnlyAfterAuthorDeletesPublicWork`, `tombstoneKeepsApprovedComments`, `othersSeeOnlyApproved` | `author_deleted_public_work` (8: khách thấy trang thay thế + bình luận 1; người viết thấy thêm bình luận của mình) | **Pure core #157:** `Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(pointer))`, `WorkAccessPlan::AuthorTombstone { last_pointer }` cho phép đọc thread cũ; `plan_comment_view` và `check_comment_submission` kiểm barrier. Chưa có persistence/API/UI cho lệnh xóa tác giả và trang tombstone |
| PUB-05 | `noExceptionForHiddenWork`, `deletionIsTerminal` | `private_then_deleted` (4, 9), `moderation_removed` (4), bước đầu của scenario A (chưa công khai). NC `mutatedPrivateStep` | **Pure core #157:** `Lifecycle::AuthorDeleted(DeletionProvenance::Hidden)` và `Lifecycle::Removed` chiếu `WorkAccessPlan::Unavailable` (`thread_accessible = false`, `404`). Replay 30/30 bước và NC `mutatedPrivateStep` qua production. `#158` đã chặn đọc công khai khi `set-private`; persistence/route cho delete/remove chưa có |
| PUB-05 (không xóa bình luận theo bài; `O-03` còn `OPEN`) | `workChangesKeepComments`, `noNewTextOffPublic` | (9) `tombstone_read_only`; (12) người viết gỡ bình luận của mình | **Pure core #157 (baseline kỹ thuật cho `O-03`, chưa phải `PRODUCT_CONFIRMED`):** `check_comment_submission` từ chối `TombstoneReadOnly`, `check_own_comment_removal` cho phép người viết tự gỡ trên `AuthorTombstone`. Hàng bình luận không bị xóa. Runtime route/HTTP cho tombstone chưa có |

### 1.3. Bộ sưu tập (M2)

| Yêu cầu | Invariant (`collections.qnt`) | Scenario / negative control | Implementation và gap |
| --- | --- | --- | --- |
| COL-03 | `listIsExactlyTheConjunction`, `listedItemsAreReadableMembers` | `permission_truth_table`: bốn dòng bảng chân trị (5, 10, 12, 16) | **Chưa có** khái niệm bộ sưu tập |
| COL-04 | `closedCollectionLeaksNothing`, `countsMatchTheList`, `badgesNeedAllThree` | (10) bài công khai trong bộ sưu tập riêng tư: không tên, không số đếm. NC `mutatedOrStep` | **Chưa có** |
| COL-05 | `linksAreReadable` | (16) hai bài công khai link sang nhau; (18–19) bài chờ duyệt rời link | **Chưa có** |
| COL-06 | `collectionChangesKeepPosts`, `postChangesKeepMembership`, `servedIsReadableNow` | (23–24) đóng bộ sưu tập không đổi bài; (19, 28) trang kế theo quyền hiện tại, `cursor_invalid` | **Chưa có** |
| COL-01, COL-02 | `onlyOwnerWrites`, `ownPostsOnly`, `noDuplicateMembership` | (1, 7, 8) người khác và bài của người khác bị từ chối | **Chưa có** |

### 1.4. Sửa bình luận và kiểm duyệt bình luận (M4)

| Yêu cầu | Invariant (`comment_edit.qnt`) | Scenario / negative control | Implementation và gap |
| --- | --- | --- | --- |
| COM-10 (không ghi đè chữ công khai), COM-02 | `pendingNeverPublic`, `acceptedEditKeepsPublicText`, `publicTextChangesOnlyByDecision` | `edit_review_override` (6) bản sửa chờ, người khác vẫn đọc bản 1. NC `mutatedOverwriteStep` | **Chưa có.** `lifecycle.rs`: “Editing does not exist”; không có bảng revision bình luận |
| COM-10 (dấu “Đã sửa”, lưu không đổi) | `editedMarkTruthful`, `unchangedSaveLeavesNoTrace` | (10) `reaffirmed` không tạo bản/dấu; (17) bản sửa bị từ chối, không dấu; (23) bản viết lại công khai có dấu | **Chưa có** cờ/dấu |
| COM-10 (hai tab, retry, ACK) | `versionFence`, `receiptsAreStable`, `refusalWritesNothing` | (7–9, 11) `replayed`, `key_reused`, `stale` | **Chưa có.** Receipt và version tồn tại cho lệnh **gửi**, chưa cho sửa |
| COM-08 | `onlyWriterCreatesRevisions` | (5) tác giả bài sửa → `not_comment_writer` | Quyền “tác giả không sửa/xóa lời người khác” đã có cho các lệnh hiện hữu; lệnh sửa chưa có |
| COM-09, MOD-09 | `adminOverrideWins`, `removalTerminal` | (20) `superseded`, (24) `overridden`; `removal_during_review` (7) `target_gone` | **Chưa có** evaluator cho bình luận; policy hiệu lực là `premoderation.v1` với người có grant |
| LVE-FR-04 (O-06) | `votesStayWithTheirText`, `countBelongsToPublicText` | (23) bản viết lại công khai: tổng 0, vote cũ vẫn lưu | “Hữu ích” hiện neo ID bình luận (bình luận bất biến); neo theo bản là đề xuất, chưa có |

### 1.5. Đề xuất và Top 100 (M5)

| Yêu cầu | Invariant (`discovery.qnt`) | Scenario / negative control | Implementation và gap |
| --- | --- | --- | --- |
| RANK-01, RANK-03 | `recommendedIsOrdered`, `moreVotesNeverLower`, `newerNeverLower`, `orderIgnoresClock` | `recommend_and_top` (3–7) vote nâng bài cũ dần lên | **Chưa có.** `GET /works` sắp theo `published_at`; không có khóa xếp hạng |
| RANK-02, RANK-04 | `recommendedIsEligibleOnce`, `editNeverBumps` | (9) bài mất quyền đọc rời danh sách; (18) sửa không bump | **Chưa có.** Tổng LVE theo revision hiện tại đã có và là đầu vào dự kiến |
| TOP-01, TOP-02 | `topIgnoresRecency`, `snapshotCapped` | (4) hai danh sách khác đầu: Top `[1,4,3]`, Đề xuất `[4,3,1,2]` | **Chưa có** |
| TOP-03 | `snapshotIsTopOfFilteredSet` | (8) lọc trước rồi mới lấy trần. NC `mutatedFilterLastStep` | **Chưa có** |
| TOP-04 | `servedIsReadableNow`, `noRepeatAcrossPages`, `servedComesFromSnapshot` | (9–10) bài bị ẩn giữa hai trang bị bỏ; (13–16) vote đổi, không lặp; (21–22) `refresh_required`. NC `mutatedStaleAccessStep` | **Chưa có** cursor mang snapshot; cursor hiện tại là keyset theo thời gian |

### 1.6. Phản đối và escalation (M6)

| Yêu cầu | Invariant (`appeal.qnt`) | Scenario / negative control | Implementation và gap |
| --- | --- | --- | --- |
| MOD-10, MOD-04 | `oneOpenCasePerDecision`, `caseNamesARealDecision`, `receiptsAreStable` | `appeal_escalation_overturn` (3–6) một case sau retry và yêu cầu trùng; (12) `stale_decision`. NC `mutatedDuplicateStep` | **Chưa có** case, appeal hay bảng nào |
| MOD-10 (chưa gửi thật), MOD-08 | `nothingIsDelivered`, `seenOnlyByAdminAction` | (1) dialog không ghi gì; (3, 14) case `not_sent`, `seen = false`. NC `mutatedDeliveryStep` | **Chưa có** kênh; không email, dashboard hay inbox |
| MOD-09 | `adminDecisionIsFinalForAi`, `overrideHasProvenance`, `resolutionIsTerminal` | (8) `reason_required`; (17) đảo quyết định có lý do; (19–20) AI muộn `overridden`; `appeal_upheld` (3–4) | **Chưa có** |

### 1.7. Không có model

| Yêu cầu | Vì sao không model | Đặc tả | Implementation và gap |
| --- | --- | --- | --- |
| HTG-01..06 | Quyền của hashtag là hệ quả của “chỉ index bài đủ điều kiện” (đã có trong M1/M5); phần còn lại là hàm thuần trên chuỗi, hợp unit test hơn | [hashtag](../architecture/hashtags.md) | **Chưa có** grammar, projection, tham số lọc, UI |
| ACC-03 | Auth là ranh giới I/O và mật mã; model không thay kiểm chứng PostgreSQL/binary thật | [ADR D-05 §12](../architecture/auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính) | **Đã có backend** username/mật khẩu native trong Identity + App (#152); Web UI / recovery chưa có |
| WR-04, ED-02, D-11 | `DEFERRED` | — | Không triển khai |

## 2. Acceptance của lượt tài liệu/model

| AC | Trạng thái | Căn cứ |
| --- | --- | --- |
| AC-01 15 quyết định được ánh xạ | Đạt | [DecisionDelta §2](../architecture/product-decisions-20261110.md#2-decisiondelta), requirements §14, decision log |
| AC-02 Phân biệt nhãn trạng thái | Đạt | DecisionDelta §1; mỗi đặc tả có dòng trạng thái và mục hiện trạng |
| AC-03 Username/mật khẩu là hướng chính, hiện trạng mã email đúng | Đạt (tài liệu) | ADR D-05 §12, D-05, ACC-03 |
| AC-04 AI auto-approval và admin override có version/authority | Đạt (model) | M1, M4, M6; contract evaluator; không gì được gửi đi |
| AC-05 Xóa ≠ riêng tư ≠ kiểm duyệt gỡ | Đạt (pure core #157) | M3 + negative control; `Lifecycle::AuthorDeleted(DeletionProvenance)` và `WorkAccessPlan` replay 30/30 bước |
| AC-06 AND bộ sưu tập × bài, có bảng chân trị và negative test | Đạt (model) | M2 + negative control |
| AC-07 Sửa bình luận có revision, kiểm duyệt, dấu | Đạt (model) | M4 + negative control |
| AC-08 Đề xuất và Top 100 là hai policy | Đạt (model) | M5 + hai negative control |
| AC-09 Hashtag không đồng nhất với taxonomy; không triển khai nhóm tuổi/nhiều chương/Tạp Chí | Đạt (tài liệu) | hashtags.md, D-09/10/11 |
| AC-10 Quint chạy được, có scenario đạt tới và negative control | Xem mục 3 | — |
| AC-11 Baseline không bị làm yếu | Xem mục 3 | Không file nào trong `behavior/discussion` hay test replay bị sửa |
| AC-12 Kế hoạch PR tiếp theo | Đạt | [DecisionDelta §6.2](../architecture/product-decisions-20261110.md#62-thứ-tự-pr-runtime-đề-xuất) |

“Đạt (model)” nghĩa là hành vi được đặc tả và model chạy; **không** phải tính năng đã có.

## 3. Bằng chứng đã chạy (lượt đặc tả/model 10/11/2026)

Bản ghi lịch sử của lượt đưa 6 model đích vào repo ngày 10/11/2026 trên base
`origin/develop@f3af6005b09372cf699953c93f88e116a23e5466` (bằng chứng của các PR triển khai sau xem tại
[publication-core](publication-core.md), [moderation-core](moderation-core.md),
[publication-runtime](publication-runtime.md) và [ADR D-05 §12](../architecture/auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính)).
Môi trường: macOS 26.5.2 (arm64), Node.js v26.12.0, npm 11.20.0, Quint **0.33.1** (pin + lockfile
của `behavior/discussion`), backend `typescript`, rustc 1.99.1. Không chạy PostgreSQL hoặc trình duyệt và không gọi provider AI.

| Lệnh | Kết quả | Phạm vi và giới hạn |
| --- | --- | --- |
| `node services/vink-app/behavior/target/verify.mjs` | **PASS** | 6 model typecheck; 6 simulation `step` seed `41`, 1.000 samples × ≤ 40 transition; 11 scenario xác định đạt đủ outcome và điều kiện non-vacuity; 9 negative control vi phạm đúng invariant với counterexample được kiểm nội dung; 16 file vectors/counterexample không drift. Thực thi ngẫu nhiên có biên, **không** vét cạn |
| `cargo xtask verify discussion` | **PASS** (5/5 bước) | Baseline không đổi: sáu model thảo luận + drift fixture; 52 test Rust qua, gồm đủ 10 test replay bắt buộc (`REPLAYS`). Bước mới “target specifications (not implemented)” chạy runner ở dòng trên. PASS của lane **không** có nghĩa hành vi đích đã được triển khai |
| `cargo xtask verify quick` | **PASS** (9/9 bước) | Kiểm token UI, manifest biểu cảm, fmt, clippy server/wasm/UI preview, 45 doctest, 568 unit/contract test không cần database và kiểm tra repository (gồm skill, link và ranh giới module) |
| Model checking (Apalache) | NOT_RUN | Không có trong toolchain của repo |
| `verify postgres`, `verify web`, `verify kani`, `verify expressions` | NOT_APPLICABLE | PR không đổi SQL, migration, API, UI, harness Kani hay model biểu cảm |
| Rust replay của model đích | NOT_IMPLEMENTED | Chưa có code production cho các hành vi này |
| GitHub CI | NOT_RUN | Bảng này ghi kết quả kiểm thử cục bộ trước khi mở PR |

Negative control tối thiểu của đề bài và nơi thỏa:

| Yêu cầu của đề bài | Mutant | Invariant vi phạm | Counterexample |
| --- | --- | --- | --- |
| Đổi AND bộ sưu tập/bài thành OR → leak | `collections.qnt` `mutatedOrStep` | `closedCollectionLeaksNothing` (transition 3) | `collections-or-counterexample.json` |
| Bỏ guard revision hoặc admin override → AI cũ thắng sai | `publication.qnt` `mutatedOverrideStep` | `adminOverrideWins` (transition 4) | `publication-late-result-counterexample.json` |
| Đồng nhất author-delete với private → link cũ làm lộ | `tombstone.qnt` `mutatedPrivateStep` | `noExceptionForHiddenWork` (transition 4) | `tombstone-private-as-deleted-counterexample.json` |
| Đưa pending edit vào public projection | `comment_edit.qnt` `mutatedOverwriteStep` | `pendingNeverPublic` (transition 2) | `comment_edit-overwrite-counterexample.json` |
| Chỉ kiểm quyền lúc tạo ranking snapshot | `discovery.qnt` `mutatedStaleAccessStep` | `servedIsReadableNow` (transition 3) | `discovery-stale-access-counterexample.json` |
| (thêm) Approved công khai bài đã về riêng tư | `publication.qnt` `mutatedIntentStep` | `publicNeedsEverything` (transition 4) | `publication-withdrawn-intent-counterexample.json` |
| (thêm) Lấy top toàn cục rồi mới lọc | `discovery.qnt` `mutatedFilterLastStep` | `snapshotIsTopOfFilteredSet` (transition 2) | `discovery-filter-after-cap-counterexample.json` |
| (thêm) Retry tạo case trùng | `appeal.qnt` `mutatedDuplicateStep` | `oneOpenCasePerDecision` (transition 2) | `appeal-duplicate-counterexample.json` |
| (thêm) Mock báo “đã gửi” | `appeal.qnt` `mutatedDeliveryStep` | `nothingIsDelivered` (transition 2) | `appeal-fake-delivery-counterexample.json` |

Mỗi mutant giữ nguyên mọi semantics khác và typecheck được; lỗi parse/type không được tính là
negative control. Không invariant nào dựa vào antecedent không đạt tới: runner đòi các state làm
đúng antecedent xuất hiện trong scenario.

## 4. Gap kiểm thử và triển khai còn lại (hiện trạng nhánh `develop`)

- **Model & Rust replay:** M1 (51/51 bước) và M3 (30/30 bước) đã về `behavior/publication` và replay
  100% qua pure core cùng 21 bước `evaluator-vectors.json` qua `ScriptedEvaluator` (`#150`, `#151`,
  `#157`; xem [publication-core](publication-core.md) và [moderation-core](moderation-core.md)). Bốn
  model còn lại trong `behavior/target` (`collections.qnt`, `comment_edit.qnt`, `discovery.qnt`,
  `appeal.qnt`) mới chỉ có bằng chứng model, chưa có code hay Rust replay.
- **Persistence & HTTP API:** `#158` ([publication-runtime](publication-runtime.md)) đã có PostgreSQL
  và HTTP API v1 cho lệnh tác giả (`snapshots`, `request-public`, `set-private`, `state`) kèm 9 test
  PostgreSQL thật cho khóa/race tác giả; `#152` đã có backend native username/mật khẩu trong Identity +
  App. **Chưa có** persistence/API cho: worker tiêu thụ `work_evaluation_jobs`, kết quả evaluator/admin
  override, xóa bài/tombstone (`PUB-04/05`), sửa bình luận (`COM-10`), bộ sưu tập (`COL`), hashtag
  (`HTG`), xếp hạng (`RANK/TOP`) và phản đối (`MOD-10`).
- **UI & trình duyệt:** Chưa có Web/CMP UI cho chọn audience/xuất bản, trang tombstone, sửa bình luận,
  bộ sưu tập, hashtag, Đề xuất/Top 100, phản đối hay form đăng nhập username/mật khẩu (và làm rõ `O-12`
  ký tự Unicode vs byte UTF-8).
- **AI evaluator & vận hành:** Chưa có bộ đánh giá tiếng Việt (MOD-06), chưa có provider AI thật hay
  kênh thông báo quản trị viên.
- **Model checking (Apalache):** NOT_RUN. Simulation là thực thi ngẫu nhiên có biên, không vét cạn.
