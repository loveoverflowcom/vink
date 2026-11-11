# Xuất bản, audience và kiểm duyệt bằng trợ lý AI — đặc tả đích

**Ngày:** 10/11/2026 · **Nguồn:** `PRODUCT-DECISIONS-20261110`
([DecisionDelta](product-decisions-20261110.md)), USER-02/03/05/06/07/14 ·
**Yêu cầu:** AUD-01/02, PUB-01..05, MOD-01..10 ([requirements](../../prd/requirements.md)) ·
**Quyết định:** D-06, D-07, D-12, D-19 ([decision log](decisions.md)).

**Trạng thái:** `TARGET_BEHAVIOR`. Luật ghi “đã chốt” là `PRODUCT_CONFIRMED`; chi tiết ghi “đề
xuất” là `DESIGN_PROPOSED`. #150 có [lõi publication thuần](../development/publication-core.md) và
#151 có [contract evaluator + scripted mock + quyền áp quyết định thuần](../development/moderation-core.md);
chưa có publication/moderation runtime. M1 canonical ở
[`behavior/publication`](../../services/vink-app/behavior/publication/README.md); `tombstone.qnt`
và `appeal.qnt` vẫn ở [`behavior/target`](../../services/vink-app/behavior/target/README.md). Tài liệu này không chấp nhận ADR nào và không bật gì ở production.

## 1. Hiện trạng (`CURRENT_IMPLEMENTATION`, base `develop@4aeb083` + lõi thuần #150/#151)

| Thứ | Có gì | Nguồn |
| --- | --- | --- |
| Bản nháp | `working_drafts` riêng tư có version, CAS khi lưu | `persistence/postgres.rs` |
| Revision | `revisions` bất biến (trigger chặn UPDATE) có `content_hash` | migration `20261030000000_literary_core` |
| Publication | Một hàng `publications(work_id, revision_id, generation, published_at, metadata_snapshot_id)`; “công khai” = có hàng | cùng migration, `20261108000000_work_taxonomy` |
| Lệnh publish/rút/xóa | **Không có**. Chỉ `dev_seed`, `xtask provision` và fixture SQL ghi `publications`; test giả lập rút bằng `DELETE FROM publications` | `dev_seed.rs`, comment bảng: “Publish/withdraw commands … are not implemented” |
| Audience | Lõi thuần #150; **chưa có** cột/lệnh runtime | `domain/publication.rs` |
| Evaluator, quyết định cho tác phẩm, admin, appeal | Lõi thuần `domain::moderation` + `ScriptedEvaluator` test-only (#151); **chưa có** bảng SQL, worker consumer, CLI/HTTP hay appeal; `/health` và worker báo `ai_moderation: not_implemented` | `domain/moderation.rs`, `application/moderation.rs`, `http/health.rs` |
| Thảo luận khi không có publication | `readable_thread` trả `None` → `404` cho đọc; gửi bị `WorkNotPublic`; hàng bình luận không bị xóa | `persistence/discussion.rs`, `domain/discussion/submission.rs` |

Mọi mục dưới đây mô tả hành vi **đích**; phạm vi lõi thuần đã có và phần chưa tích hợp được tách trong [publication-core](../development/publication-core.md).

## 2. Thuật ngữ

| Từ | Nghĩa | Không phải |
| --- | --- | --- |
| Bài (work) | Một bài viết ngắn/tản văn của một tác giả | Chương, sách |
| Bản (revision) | Snapshot bất biến của tiêu đề + nội dung, có digest | Bản nháp đang sửa |
| Audience | Lựa chọn của tác giả: **Chỉ mình tôi** hoặc **Công khai** | Trạng thái kiểm duyệt |
| Ý định công khai | Audience “Công khai” gắn với **một bản cụ thể** và một generation | “Đăng bản mới nhất” |
| Quyết định (decision) | Kết luận chính sách cho **một bản**: chưa gửi, đang chờ, được duyệt, không được duyệt, cần quản trị viên; ghi ai quyết định | Việc bài có đang công khai |
| Bản công khai (pointer) | Bản mà người khác đang đọc được | Bản mới nhất được duyệt |
| Vòng đời (lifecycle) | Đang hoạt động / tác giả đã xóa / bị kiểm duyệt gỡ | Audience |
| Evaluator | Trợ lý AI: actor dịch vụ, chỉ nộp kết quả đánh giá | Admin; nguồn authority |
| Quản trị viên (admin) | Chủ sản phẩm; quyết định thủ công và override | Người trực có SLA |

## 3. Bốn trục độc lập

Không gộp thành một `status`. Mỗi trục có owner và lệnh riêng; bản công khai chỉ đổi qua guard ở
cuối mục.

| Trục | Giá trị | Ai đổi | Ghi chú |
| --- | --- | --- | --- |
| Audience | `private` \| `public` | Tác giả | Mặc định `private` (đề xuất). Mỗi lệnh được chấp nhận tăng `generation` |
| Quyết định của từng bản | `none` → `pending` → `approved` \| `rejected` \| `needs_admin` | Evaluator (kết quả hợp lệ), admin | `by` ∈ {`ai`, `admin`}; bản mới bắt đầu `none` |
| Bản công khai | 0 hoặc một bản | Chỉ guard xuất bản | Không lệnh nào ghi trực tiếp |
| Vòng đời | `active` \| `author_deleted` \| `removed` | Tác giả (xóa), admin (gỡ) | Hai trạng thái sau là terminal trong đặc tả này |

**Lệnh của tác giả**

| Lệnh | Điều kiện | Kết quả |
| --- | --- | --- |
| Lưu bản | Bài đang hoạt động | Tạo bản mới `none`. **Không** gọi evaluator, không gửi gì ra ngoài (MOD-05, INV-08) |
| Công khai bản r (kèm generation đã thấy) | Bản tồn tại; generation khớp, nếu không → `stale` | `none` → `pending` + tạo evaluation; `pending`/`needs_admin` → ghi ý định, chờ; `approved` → công khai ngay; `rejected` → từ chối (viết bản mới hoặc phản đối) |
| Chỉ mình tôi (kèm generation) | generation khớp | Đóng đọc công khai ngay; bỏ ý định; **giữ** các quyết định đã có |
| Xóa bài | Bài đang hoạt động | Mục 6 |

**Guard xuất bản** — đọc lại tại commit của mọi lệnh có thể đổi bản công khai (PUB-01, INV-01):

```text
publish(r) hợp lệ ⇔ lifecycle = active
                  ∧ decision(r) = approved
                  ∧ audience = public
                  ∧ ý định công khai hiện tại đúng là r
```

Hệ quả: Approved không tự công khai bài đang “Chỉ mình tôi”; bản r1 vẫn công khai khi r2 chờ hoặc
bị từ chối (PUB-02); admin duyệt một bản không có ý định công khai thì bản đó vẫn chưa công khai.

## 4. Contract evaluator và mock

### 4.1. Yêu cầu đánh giá và kết quả (đề xuất tên trường)

App tạo **evaluation** khi tác giả công khai một bản chưa có quyết định (hoặc khi thử lại). Một
evaluation gắn:

| Trường | Ý nghĩa |
| --- | --- |
| `evaluation_id` | Danh tính của lần hỏi (job); mỗi kết quả chỉ áp một lần |
| `target` | Loại và ID: bản của bài, hoặc bản của bình luận |
| `revision_id` + `content_digest` | Đúng nội dung được đánh giá |
| `policy_version` | Policy đang hiệu lực khi hỏi |

Evaluator trả **kết quả có cấu trúc** lặp lại bốn trường trên và thêm `outcome` cùng `reason`
(mã ổn định, không phải văn bản tự do của model):

| `outcome` | Ý nghĩa nghiệp vụ |
| --- | --- |
| `approve` | Đạt policy |
| `reject(reason)` | Vi phạm policy; lý do phải có thể hành động |
| `uncertain` | Không đủ căn cứ → cần quản trị viên |
| lỗi vận chuyển: `timeout`, `provider_error` | Không có kết luận → giữ `pending`, thử lại được |
| `invalid_output` | Output không qua kiểm schema → cần quản trị viên |

Kết quả **không mang authority**. App chỉ biến nó thành quyết định sau khi kiểm theo thứ tự:

1. Evaluation tồn tại và còn mở → nếu không: `unknown_job` / `duplicate`.
2. `revision_id`/digest khớp evaluation → nếu không: `wrong_revision`.
3. Target còn hoạt động (chưa bị xóa/gỡ) → nếu không: `target_gone`.
4. Bản chưa có quyết định của admin → nếu có: `overridden`.
5. `policy_version` còn hiệu lực → nếu không: `stale_policy` (App hỏi lại theo policy mới).
6. Với bình luận: bản vẫn là bản đang chờ → nếu không: `superseded`.

Mọi nhánh từ chối **không ghi gì**. Sau khi ghi quyết định `approved`, App chạy guard xuất bản; kết
quả `approved_unpublished` là bình thường khi tác giả đã rút ý định.

Ràng buộc giữ nguyên: nội dung và output là dữ liệu không tin cậy (MOD-05); không gửi bản nháp
riêng vì autosave; không dùng “văn hay/dở” làm tiêu chí an toàn (MOD-01); confidence do model tự
báo không là xác suất đã hiệu chuẩn.

### 4.2. Mock evaluator

Mục tiêu: kiểm orchestration, state và quyền. **Không** đánh giá chất lượng kiểm duyệt.

| Kịch bản của mock | Thể hiện trong vectors |
| --- | --- |
| `Approve`, `Reject(reason)`, `NeedsAdmin` | `outcome` = `approve` / `reject` / `uncertain` |
| `Timeout`, `ProviderError`, `InvalidOutput` | `timeout` / `provider_error` / `invalid_output` |
| `WrongRevision` | Kết quả mang revision khác evaluation |
| `DuplicateResult` | Cùng evaluation nộp lần hai |
| `DelayedResult` | Kết quả tới sau admin override, sau khi target bị xóa, sau khi policy đổi hoặc sau khi có bản sửa mới |

Mock là **kịch bản tất định**: harness điều khiển outcome, thứ tự callback, revision, evaluation và
thời điểm giả lập; không `sleep`, không mạng, không secret, không model key. Mock không được: luôn
trả `Approve`, tự cấp quyền admin, ghi SQL, hay thay logic domain cần kiểm.

**Lượt này:** #150 thêm seam quyết định đã được xác nhận (`record_review`) và #151 thêm
[`domain::moderation`](../../services/vink-app/src/domain/moderation.rs) +
[`ScriptedEvaluator`](../../services/vink-app/src/application/moderation.rs) đọc trực tiếp
[`evaluator-vectors.json`](../../services/vink-app/behavior/target/evaluator-vectors.json)
(format `vink-moderation-evaluator-script-v1`, 21 kết quả phủ đủ chín loại trên: 12 bước
`publication.qnt` chạy qua full consumer `WorkModeration` + `Publication`, 9 bước `comment_edit.qnt`
kiểm tương thích hợp đồng/script). Chưa dựng queue/worker/provider runtime; chi tiết ở
[moderation-core](../development/moderation-core.md). Seam `AdmissionFacts.classifier` của bình luận
**không** phù hợp: nó là tín hiệu “chỉ được giữ lại, không bao giờ làm sạch” của policy ứng viên cũ,
khác ngữ nghĩa evaluator quyết định.

## 5. Quản trị viên và phản đối

**Quản trị viên (MOD-09, đã chốt).** Admin quyết định thủ công trên **một bản đã được gửi duyệt**
và có thể đảo quyết định của AI. Quyết định của admin trên bản đó thắng mọi kết quả AI tới sau hoặc
được retry cho cùng bản; worker không tự gỡ override. Admin **không**: quyết định trên bản chỉ lưu
riêng, công khai thay tác giả, hồi sinh bài tác giả đã xóa, bỏ qua audit. Mỗi override ghi actor,
lý do bắt buộc và bản/decision version được xét. Dashboard và thao tác production: phát triển sau;
bước đầu (đề xuất) là lệnh operator `vink-app` như các quyền vận hành hiện có.

**Không có người trực, không SLA (MOD-08, đã chốt).** `needs_admin` là trạng thái của bản, không
phải bằng chứng admin đã nhận. Không có chuỗi nào hứa thời hạn.

**Phản đối quyết định của AI (MOD-10, đã chốt hướng; chi tiết đề xuất).**

| Bước | Có trong đặc tả | Có thật hiện nay |
| --- | --- | --- |
| Mở dialog | Không ghi gì | Không có UI |
| Tạo case | Case gắn `decision_version`; lưu bền; trả “đã ghi nhận, **chưa gửi**” | Không |
| Gửi thông báo cho admin | **Không có trong giai đoạn này**: kênh chưa kết nối; không trạng thái nào được ghi “đã gửi” | Không |
| Admin nhận | Chỉ suy ra từ hành động của admin trên case | Không |
| Admin xử lý | Giữ nguyên hoặc đảo, có lý do; tạo decision version mới `by admin` | Không |

Luật của case: chỉ chủ nội dung tạo được (người khác nhận “không tìm thấy”); cùng key + cùng quyết
định → trả lại case cũ; key khác cho cùng quyết định → vẫn case cũ; quyết định đã đổi → từ chối
`stale_decision`; quyết định đang `needs_admin` → `awaiting_admin`; quyết định của admin → là cuối
(O-15). Admin xử lý case mà quyết định đã sang version khác → case đóng `superseded`, không đổi gì.

Copy đề xuất khi chưa có delivery (chưa dựng UI):

> Cần quản trị viên xem xét. Kênh gửi yêu cầu chưa được kết nối; chưa có thông báo được gửi.

## 6. Xóa, riêng tư, gỡ và tombstone

Bốn trường hợp phải tách (PUB-05, đã chốt):

| | Trường hợp | Trang ở URL cũ (người khác) | Nội dung, preview, listing | Bình luận đọc được |
| --- | --- | --- | --- | --- |
| A | Tác giả **xóa** bài đang công khai | Trang thay thế: “Nội dung đã được tác giả xoá” | Không | Bình luận đã duyệt, chưa gỡ. Người viết thấy thêm bình luận chờ/bị từ chối **của mình** |
| B | Tác giả chuyển **Chỉ mình tôi** | Không tìm thấy | Không | Không. Công khai lại thì thảo luận trở lại |
| C | Hệ thống/admin **gỡ** vì vi phạm | Không tìm thấy | Không | Không |
| D | Chưa từng công khai, hoặc xóa khi đang ẩn | Không tìm thấy | Không | Không |

- “Listing” gồm Khám phá, tìm kiếm, hashtag, Đề xuất, Top 100, bài liên quan và bộ sưu tập.
- “Preview” là mọi thứ **hệ thống** sinh từ nội dung bài: snippet, trích dẫn, thẻ chia sẻ, chữ
  trong thông báo. Chúng không được là đường đọc lại bài. Chữ người khác tự viết hay sao chép vào
  bình luận là nội dung của bình luận đó, theo vòng đời riêng; không hứa thu hồi mọi bản sao và
  không xóa hàng loạt lời bình.
- Xóa, ẩn hay gỡ bài **không** xóa hay sửa bình luận của ai.
- A không phải audience thứ ba: đó là ngữ nghĩa của trạng thái “tác giả đã xóa”.
- Đề xuất (O-03/04/05/09): tombstone không nhận bình luận, phản hồi hay vote mới; người viết vẫn
  gỡ được bình luận của mình; kiểm duyệt vẫn quyết định được từng bình luận; xóa khi bài đang “Chỉ
  mình tôi” không để lại tombstone; tombstone không hiện tiêu đề, tên tác giả, thẻ hay hashtag.
- Còn `OPEN`: retention/purge vật lý (O-08), khôi phục (O-10).

## 7. Bảng quyền

| Hành động | Tác giả bài | Người khác / khách | Evaluator | Admin |
| --- | --- | --- | --- | --- |
| Đọc bản nháp, bản chưa công khai | Có | Không | Chỉ bản được gửi đánh giá | Chỉ bản đã gửi duyệt |
| Lưu bản, đổi audience, xóa bài | Có | Không (không phân biệt được với không tồn tại) | Không | Không |
| Nộp kết quả đánh giá | Không | Không | Có, qua kiểm ở mục 4 | — |
| Quyết định/override một bản | Không | Không | Không | Có, kèm lý do |
| Gỡ bài vì vi phạm | Không | Không | Không | Có |
| Phản đối quyết định | Có (bài của mình) | Không | Không | — |
| Xử lý case | Không | Không | Không | Có |
| Sửa/xóa bình luận người khác | **Không** (COM-08) | Không | Không | Chỉ gỡ theo kiểm duyệt |

## 8. Use case

| # | Tình huống | Kết quả phải giữ | Scenario trong model |
| --- | --- | --- | --- |
| 1 | Lưu riêng rồi không làm gì | Không evaluation nào được tạo | `publication` bước 1–2 |
| 2 | Công khai r1, evaluator timeout, thử lại, duyệt | r1 chỉ công khai sau kết quả hợp lệ | bước 4–12 |
| 3 | r1 công khai; r2 bị từ chối | Người đọc vẫn thấy r1 | bước 13–16 |
| 4 | Tab cũ gửi “công khai” sau khi tab mới đã đổi | `stale`, không đổi gì | bước 18, 28 |
| 5 | Policy đổi khi đang chờ | Kết quả cũ bị bỏ; hỏi lại | bước 20–22 |
| 6 | Evaluator không chắc; admin duyệt; kết quả AI muộn từ chối | Quyết định admin giữ nguyên | bước 23–26 |
| 7 | Về “Chỉ mình tôi” trong lúc chờ; kết quả duyệt tới | Được duyệt nhưng không công khai | bước 33–35 |
| 8 | Tác giả xóa; kết quả/admin tới sau | Không hồi sinh | bước 37–40 |
| 9 | Xóa bài có ba bình luận (duyệt, từ chối, chờ) | Tombstone giữ đúng bình luận đã duyệt | `tombstone` scenario A |
| 10 | Chuyển riêng tư rồi mở link cũ | Không trang, không bình luận | scenario B |
| 11 | Phản đối hai lần, rồi admin đảo quyết định | Một case; override có lý do; AI muộn không đè | `appeal` scenario 1 |

## 9. Bằng chứng và gap

Model, invariant, negative control và lệnh chạy: [README target](../../services/vink-app/behavior/target/README.md).
Bảng yêu cầu → invariant → gap: [registry](../development/product-decisions-registry.md).
Bằng chứng M1 gồm model và scoped pure Rust replay (#150; [coverage](../development/publication-core.md)).
Các model khác chỉ có bằng chứng model. Chưa có migration/transaction publication, API, UI, worker,
evaluator/authority runtime hay kênh thông báo.
