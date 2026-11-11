# Đặc tả đích — quyết định sản phẩm 10/11/2026

**Trạng thái năm model còn trong thư mục: `TARGET_SPEC_NOT_IMPLEMENTED`.** Năm model ở đây
chưa có domain consumer hay Rust replay cho aggregate của chúng. M1 đã chuyển nguyên vẹn sang
[`../publication`](../publication/README.md): #150 và #151 replay 46/51 bước của M1 (author,
evaluator, admin override, reevaluate, policy bump và cả hai negative control) qua lõi thuần
`Publication` + `WorkModeration`, đồng thời replay `evaluator-vectors.json` qua `ScriptedEvaluator`
(12 bước `publication.qnt` qua full consumer, 9 bước `comment_edit.qnt` chỉ kiểm hợp đồng/script);
delete/remove lifecycle và năm model còn lại vẫn là model-only.
Một canonical model cho mỗi policy; không có bản publication cạnh tranh ở thư mục này.

Phân vai: PRD/decision log giữ ý định và quyết định; model ở đây giữ state/transition/invariant đã
chọn; Rust (khi có) là implementation; mock evaluator và sơ đồ Markdown **không** phải nguồn này.

## Model

| Model | Phạm vi | Yêu cầu | Đặc tả văn bản |
| --- | --- | --- | --- |
| [`../publication/publication.qnt`](../publication/publication.qnt) (M1, replay một phần) | Một bài: audience ≠ quyết định ≠ bản công khai ≠ vòng đời; kết quả evaluator đúng/sai/cũ/trùng; admin override; xóa/riêng tư; không hồi sinh | AUD-01/02, PUB-01/02, MOD-02/03/05/07/08/09, INV-01/02/03/07/08 | [xuất bản và kiểm duyệt](../../../../docs/architecture/publication-moderation.md) |
| `collections.qnt` (M2) | Phép AND bài × bộ sưu tập; chủ/khách/người khác; thêm/bỏ; đổi audience khi đang đọc/phân trang; link và số đếm | COL-01..06 | [bộ sưu tập](../../../../docs/architecture/collections.md) |
| `tombstone.qnt` (M3) | Tác giả xóa ≠ riêng tư ≠ kiểm duyệt gỡ ≠ chưa từng công khai; trang thay thế; bình luận hợp lệ | PUB-03/04/05 | [mục 6](../../../../docs/architecture/publication-moderation.md#6-xóa-riêng-tư-gỡ-và-tombstone) |
| `comment_edit.qnt` (M4) | Bản hiển thị và bản chờ tách nhau; sửa/duyệt/từ chối/override/xóa; hai tab, retry, callback cũ; dấu “Đã sửa”; vote theo bản | COM-02/08/09/10, MOD-09, LVE-FR-04 | [sửa bình luận](../../../../docs/architecture/comment-editing.md) |
| `discovery.qnt` (M5) | Eligibility trước ranking; Đề xuất ≠ Top; thứ tự/tie ổn định; trần; snapshot và thu hồi | RANK-01..04, TOP-01..04 | [Đề xuất và Top 100](../../../../docs/architecture/discovery-ranking.md) |
| `appeal.qnt` (M6) | Case gắn decision version; không nhân case; dialog/mock không thành “đã gửi”; override có provenance; AI cũ không đè | MOD-04/08/09/10 | [mục 5](../../../../docs/architecture/publication-moderation.md#5-quản-trị-viên-và-phản-đối) |

Mỗi model: miền hữu hạn ghi ở đầu file, giả định actor/authority, action có guard, invariant có
tên, scenario đạt tới được, scenario lỗi/race, và negative control. **Giả định chung, không phải
bằng chứng:** mỗi action là một commit đã tuần tự hóa; actor đúng là người họ nói (auth thuộc
D-05); evaluator và admin là capability chưa tồn tại; không có đồng hồ, deadline hay SLA.

## Chạy

Dùng chung pin Quint **0.33.1** và lockfile của `../discussion` (không có package thứ hai). Yêu
cầu Node.js ≥ 22. Lane local/CI là `cargo xtask verify discussion`: bước “publication canonical model + remaining target specifications” chạy runner này sau khi đã cài dependency. Chạy riêng từ repo root:

```sh
npm ci --ignore-scripts --no-audit --no-fund --prefix services/vink-app/behavior/discussion
node services/vink-app/behavior/target/verify.mjs
```

Runner, cho từng model:

1. `quint typecheck` (gồm cả action mutation).
2. Random simulation `step`: seed `41`, **1.000 samples × tối đa 40 transition**, backend
   `typescript`, kiểm invariant tổng (`publicationSafe`, `collectionsSafe`, `tombstoneSafe`,
   `commentEditSafe`, `discoverySafe`, `appealSafe`).
3. Scenario xác định (1 sample) xuất ITF thật → `*-vectors.json`
   (`vink-target-spec-vectors-v1`): mỗi bước gồm command, outcome và state sau commit. JS chỉ
   chuyển kiểu ITF, không tính lại transition. Runner đòi đủ danh sách outcome và các điều kiện
   non-vacuity ghi trong `verify.mjs` (ví dụ: r1 vẫn công khai khi r2 bị từ chối; khách đọc đúng
   bình luận đã duyệt trên tombstone; bốn dòng bảng chân trị của bộ sưu tập).
4. Negative control: action mutation chỉ bỏ **một** guard, chạy với invariant phải vi phạm; đòi
   exit 1, thông báo `Invariant violated`, ITF `violation` và nội dung counterexample cụ thể. Lỗi
   parse/type/runtime không được tính.
5. `evaluator-vectors.json` (`vink-moderation-evaluator-script-v1`): các kết quả evaluator trong
   trace của `publication` và `comment_edit`, phân loại theo chín kịch bản của mock và điều App
   phải làm; runner đòi đủ chín loại và không kịch bản khác `Approve` nào dẫn tới công khai.

Mặc định chỉ kiểm drift. Sau khi chủ động sửa model và đã đọc output:

```sh
node services/vink-app/behavior/target/verify.mjs --write
```

Simulation là kiểm **bounded random executions**, không phải exhaustive model checking hay proof.
Apalache/model checking: **NOT_RUN**. Không có khẳng định liveness nào.

## Scenario và negative control

| Model | Scenario (số bước) | Negative control → invariant vi phạm |
| --- | --- | --- |
| `publication` | `audience_moderation_override` (40), `moderation_removal` (11) | `mutatedOverrideStep` (bỏ guard admin override: kết quả AI muộn đảo quyết định admin) → `adminOverrideWins`; `mutatedIntentStep` (bỏ guard ý định: duyệt là công khai dù tác giả đã về riêng tư) → `publicNeedsEverything` |
| `tombstone` | `author_deleted_public_work` (14), `private_then_deleted` (10), `moderation_removed` (6) | `mutatedPrivateStep` (coi “riêng tư” như “tác giả xóa”: link cũ đọc được bình luận) → `noExceptionForHiddenWork` |
| `collections` | `permission_truth_table` (36) | `mutatedOrStep` (AND thành OR: khách thấy bài công khai trong bộ sưu tập riêng tư) → `closedCollectionLeaksNothing` |
| `comment_edit` | `edit_review_override` (31), `removal_during_review` (9) | `mutatedOverwriteStep` (bản sửa ghi đè chữ công khai trước khi duyệt) → `pendingNeverPublic` |
| `discovery` | `recommend_and_top` (25) | `mutatedStaleAccessStep` (chỉ kiểm quyền lúc chụp snapshot) → `servedIsReadableNow`; `mutatedFilterLastStep` (lấy top toàn cục rồi mới lọc) → `snapshotIsTopOfFilteredSet` |
| `appeal` | `appeal_escalation_overturn` (21), `appeal_upheld` (5) | `mutatedDuplicateStep` (bỏ receipt/kiểm case đang mở: retry tạo case thứ hai) → `oneOpenCasePerDecision`; `mutatedDeliveryStep` (mock báo “đã gửi”) → `nothingIsDelivered` |

## Không được chứng minh

Auth và authority thật; khóa, transaction, isolation và mọi thứ của PostgreSQL; nhiều bài/bình
luận/tác giả đồng thời ngoài miền nhỏ; thời gian thực, retry có backoff, deadline; chất lượng kiểm
duyệt của bất kỳ evaluator nào; công thức xếp hạng với dữ liệu thật (cap = 3 thay cho 100); grammar
hashtag (không có model); username/mật khẩu (không có model); UI; liveness (“cuối cùng sẽ được
xử lý”). Model không quyết định các điểm `OPEN` của DecisionDelta: nó chỉ dùng đề xuất tối thiểu.

## Đường hợp nhất khi có runtime

Thư mục này là **tạm**. Khi một PR triển khai phần nào, PR đó chuyển model tương ứng sang thư mục
owner, thêm Rust replay gọi đúng hàm production, thêm test replay vào danh sách bắt buộc của lane,
và xóa model khỏi đây. Không giữ hai model cho cùng một policy.

| Model | Về đâu | Khi |
| --- | --- | --- |
| `comment_edit.qnt` | `../discussion`, cạnh `lifecycle.qnt`; phần quyết định của evaluator hợp nhất vào `admission.qnt` | PR sửa bình luận / evaluator cho bình luận |
| `publication.qnt` | Đã chuyển sang `../publication` trong #150; vectors và hai counterexample đi cùng | Author/publication có replay một phần; #151 tiếp tục evaluator/admin trên cùng model |
| `tombstone.qnt`, `appeal.qnt` | Thư mục behavior của owner xuất bản | PR tombstone, phản đối |
| `collections.qnt` | Thư mục behavior của module bộ sưu tập | PR bộ sưu tập |
| `discovery.qnt` | Cạnh owner discovery | PR Đề xuất/Top 100 |

Baseline của `../discussion` (model, fixture, replay) **không** bị sửa để khớp các đặc tả này.
Bảng yêu cầu → invariant → gap và bằng chứng đã chạy:
[registry](../../../../docs/development/product-decisions-registry.md).
