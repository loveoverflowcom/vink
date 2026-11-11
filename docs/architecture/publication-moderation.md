# Xuất bản, audience và kiểm duyệt bằng trợ lý AI — đặc tả đích

**Ngày:** 10/11/2026, bổ sung 11/11/2026 · **Nguồn:** `PRODUCT-DECISIONS-20261110`,
`ADMIN-MODERATION-20261111` ([DecisionDelta](product-decisions-20261110.md), §7),
USER-02/03/05/06/07/14 · **Yêu cầu:** AUD-01/02, PUB-01..07, MOD-01..15, COM-11
([requirements](../../prd/requirements.md)) ·
**Quyết định:** D-06, D-07, D-12, D-19 ([decision log](decisions.md)). Từ 11/11/2026, D-06/07/19
tra ra USER/ADM của kho TOML; phần thu hồi của D-12 là `D-12@r1`.

<!-- vdr: active D-12@r1 -->

**Trạng thái:** `TARGET_BEHAVIOR`. Luật ghi “đã chốt” là `PRODUCT_CONFIRMED`; chi tiết ghi “đề
xuất” là `DESIGN_PROPOSED`. #150 có [lõi publication thuần](../development/publication-core.md) và
#151 có [contract evaluator + scripted mock + quyền áp quyết định thuần](../development/moderation-core.md);
chưa có publication/moderation runtime. M1, M3 (`tombstone.qnt`) và M4 quản trị (`governance.qnt`,
thay `appeal.qnt`) canonical ở [`behavior/publication`](../../services/vink-app/behavior/publication/README.md);
lõi quản trị ở [ADMIN-CORE-01](../development/moderation-core.md#admin-moderation-core--admin-core-01). Tài liệu này không chấp nhận ADR nào và không bật gì ở production.

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
| Quản trị viên (admin) | Admin tổng: quản lý quyền kiểm duyệt, kết luận phản đối, dừng/bật tự động, gỡ cả bài, điều tra riêng | Người trực có SLA |
| Moderator | Người có grant duyệt tác phẩm hoặc bình luận, trong phạm vi grant | Vai trò ngầm định của tài khoản, tác giả bài |
| Grant | Quyền duyệt `works` hoặc `comments`, cho cá nhân hoặc nhóm, toàn Vink hoặc một `WorkId`, có thể hết hạn | Cờ boolean toàn cục |
| Ca (review case) | Một việc duyệt cho đúng một bản của tác phẩm hoặc bình luận, có phiên bản ca và tối đa một người nhận | Quyết định |
| Hồ sơ phản đối (appeal) | Yêu cầu của chủ nội dung trên đúng một phiên bản quyết định bất lợi | Ca duyệt bản mới |
| Hồ sơ điều tra | Quyền đọc từng lần một bản từng công khai của bài đã riêng tư, có căn cứ và audit | Quyền đọc bản nháp |
| Trang ẩn | Trang ở URL cũ của bài đang công khai đã chuyển riêng tư: “Nội dung đã được ẩn” + bình luận | Trang thay thế sau khi xóa |

## 3. Bốn trục độc lập

Không gộp thành một `status`. Mỗi trục có owner và lệnh riêng; bản công khai chỉ đổi qua guard ở
cuối mục.

| Trục | Giá trị | Ai đổi | Ghi chú |
| --- | --- | --- | --- |
| Audience | `private` \| `public` | Tác giả | Mặc định `private` (đề xuất). Mỗi lệnh được chấp nhận tăng `generation` |
| Quyết định của từng bản | `none` → `pending` → `approved` \| `rejected` \| `needs_admin` | Evaluator (kết quả hợp lệ khi tự động đang bật), moderator trong grant, admin | `by` ∈ {`ai`, `moderator`, `admin`}; gắn policy version; bản mới bắt đầu `none` |
| Bản công khai | 0 hoặc một bản | Chỉ guard xuất bản | Không lệnh nào ghi trực tiếp |
| Vòng đời | `active` \| `author_deleted` \| `removed` | Tác giả (xóa), admin (gỡ) | Hai trạng thái sau là terminal trong đặc tả này |

**Lệnh của tác giả**

| Lệnh | Điều kiện | Kết quả |
| --- | --- | --- |
| Lưu bản | Bài đang hoạt động | Tạo bản mới `none`. **Không** gọi evaluator, không gửi gì ra ngoài (MOD-05, INV-08) |
| Công khai bản r (kèm generation đã thấy) | Bản tồn tại; generation khớp, nếu không → `stale` | `none` → `pending` + tạo evaluation; `pending` có ca mở → ghi ý định, chờ; `pending` mà ca đã đóng (vì chuyển riêng tư) → mở lượt mới (`resubmitted`); `needs_admin` → về hàng chờ người; `approved` còn hiệu lực ở policy hiện hành → công khai ngay; `approved` ở policy cũ → AI duyệt lại (quyết định của AI) hoặc về hàng chờ người (quyết định của người); `rejected` → từ chối (viết bản mới hoặc phản đối) |
| Chỉ mình tôi (kèm generation) | generation khớp | Đóng đọc công khai ngay; bỏ ý định; **đóng** evaluation và phản đối đang mở của tác phẩm; **giữ** các quyết định đã có. Nếu bài đang công khai: URL cũ thành trang ẩn (§6) |
| Xóa bài | Bài đang hoạt động | Mục 6 |

**Guard xuất bản** — đọc lại tại commit của mọi lệnh có thể đổi bản công khai (PUB-01, INV-01):

```text
publish(r) hợp lệ ⇔ lifecycle = active
                  ∧ decision(r) = approved
                  ∧ audience = public
                  ∧ ý định công khai hiện tại đúng là r
```

Hệ quả: Approved không tự công khai bài đang “Chỉ mình tôi”; bản r1 vẫn công khai khi r2 chờ hoặc
bị từ chối (PUB-02); người duyệt một bản không có ý định công khai thì bản đó vẫn chưa công khai.
**11/11/2026:** kết quả AI hay kết luận của người tới **sau** khi chuyển riêng tư bị từ chối
(`withdrawn` / `case_closed`), không còn ghi `approved_unpublished` cho bản đã rút (PUB-06).

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

## 5. Quản trị viên, moderator và phản đối

Luật mục này là `PRODUCT_CONFIRMED` từ 10/11/2026 (MOD-08..10) và 11/11/2026 (MOD-11..15,
`ADMIN-MODERATION-20261111`). Tên kiểu và mã từ chối là `DESIGN_PROPOSED`.

**Không có người trực, không SLA (MOD-08).** `needs_admin` là trạng thái của bản, không phải bằng
chứng có người đã nhận. Không chuỗi nào hứa thời hạn.

### 5.1. Vai trò và ủy quyền (MOD-09, MOD-11)

- **Admin tổng** quản lý quyền kiểm duyệt (cấp/thu hồi grant, quản lý nhóm), kết luận phản đối,
  dừng/bật chế độ tự động, gỡ cả bài và mở hồ sơ điều tra. Admin cũng làm được mọi việc của
  moderator.
- **Grant** gồm: grantee (một principal hoặc một nhóm do admin quản lý), capability `works` hoặc
  `comments` (độc lập: grant bình luận không duyệt được tác phẩm và ngược lại), phạm vi toàn Vink
  hoặc một `WorkId` (bình luận của bài W thuộc phạm vi W), hạn tùy chọn. Grant bình luận hiện hành
  (`vink-app moderator grant`, bảng `moderator_grants`) được **kế thừa** như grant `comments` toàn Vink.
- **Kiểm lại mỗi lần đọc hoặc xử lý:** admin còn hiệu lực hoặc có ít nhất một grant chưa thu hồi,
  chưa hết hạn, đúng capability và phạm vi, và (với nhóm) người đó **hiện** là thành viên; tài khoản
  không bị khóa. Căn cứ quyền (admin hoặc grant cụ thể) được ghi cùng mỗi kết luận.
- **Tự né:** không ai duyệt nội dung do mình viết; không ai duyệt bình luận thuộc bài do mình
  viết (kể cả admin). Mã: `recused`.

### 5.2. Ca duyệt và nhận ca (MOD-12)

Ca được mở khi một bản cần người: evaluator trả `uncertain`/`invalid_output`, lỗi đã hết lượt thử,
chế độ tự động đang dừng, quyết định của người cần xem lại vì policy đổi, hay moderator chủ động
can thiệp trên bản đang đăng. Trạng thái của ca tách ba mức mà lịch sử phân biệt:
**đã ghi nhận** (`queued`), **đã có người nhận** (`claimed`), **đã có kết luận** (`concluded`);
thêm `escalated` (chờ admin) và `closed` (đóng vì tác giả chuyển riêng tư, bài bị xóa/gỡ).

| Lệnh | Ai | Điều kiện | Kết quả |
| --- | --- | --- | --- |
| Nhận ca | Người có quyền với ca | `queued`; `expected` = phiên bản ca | `claimed` bởi người đó + căn cứ quyền |
| Trả ca | Người nhận | `claimed` bởi chính họ | `queued` |
| Ghi chú nội bộ | Người nhận, admin | Ca chưa đóng | Ghi chú tách khỏi lý do gửi người viết |
| Chuyển lên admin | Người nhận | `claimed` bởi chính họ | `escalated`; chỉ admin nhận được |
| Admin nhận lại | Admin | Ca chưa kết luận/đóng; lý do bắt buộc | `claimed` bởi admin |
| Kiểm lại quyền | Hệ thống | Người nhận mất quyền thực tế | `queued` (`authority_lost`); nếu còn grant hợp lệ khác thì giữ ca và đổi căn cứ |
| Kết luận | Người nhận | `claimed` bởi chính họ, quyền còn hiệu lực, `expected` ca + phiên bản quyết định khớp | `concluded`; quyết định mới áp qua owner (publication hoặc discussion) |

Mọi lệnh đổi trạng thái tăng phiên bản ca; lệnh mang phiên bản cũ nhận `stale_case`, nên hai
moderator tranh ca hay kết luận đồng thời thì chỉ một được áp dụng.

### 5.3. Can thiệp (MOD-13)

- Moderator `works` duyệt, từ chối hoặc ghi đè quyết định AI của một bản đã gửi duyệt, và bác
  **bản đang đăng** để ngừng hiển thị bản đó. Quyết định của người (`moderator` hoặc `admin`) trên
  một bản thắng mọi kết quả AI tới sau hoặc retry cho cùng bản.
- **Chỉ admin gỡ cả bài**: kết thúc quyền đọc qua URL cũ, kể cả bình luận (case C ở §6).
- Moderator `comments` duyệt/từ chối bình luận và **ẩn rồi khôi phục đúng bản đã duyệt**: ẩn chỉ
  áp cho bình luận đang được duyệt; khôi phục chỉ khi bình luận vẫn đúng như lúc ẩn (cùng phiên bản,
  chưa bị người viết gỡ, chưa đổi quyết định). Khôi phục dùng lại quy tắc hiển thị gốc và phản hồi
  hiện hành. Gỡ của người viết và gỡ theo cơ chế cũ (terminal) vẫn có hiệu lực.
- Người duyệt không: quyết định trên bản chỉ lưu riêng, công khai thay tác giả, hồi sinh bài đã
  xóa/gỡ, bỏ qua audit. Mỗi kết luận ghi actor, căn cứ quyền, lý do bắt buộc (cho từ chối, ẩn, gỡ,
  đảo AI) và phiên bản; ghi chú nội bộ không gửi người viết.

### 5.4. Phản đối (MOD-10)

| Bước | Có trong đặc tả | Có thật hiện nay |
| --- | --- | --- |
| Mở dialog | Không ghi gì | Không có UI |
| Tạo hồ sơ | Gắn đúng phiên bản quyết định bất lợi; lưu bền; trả “đã ghi nhận, **chưa gửi**” | Core thuần |
| Gửi thông báo cho admin | **Không có trong giai đoạn này**: không trạng thái nào được ghi “đã gửi” | Không |
| Admin nhận | Suy ra từ hành động của admin trên hồ sơ | Core thuần |
| Admin kết luận | Giữ nguyên hoặc đảo, lý do bắt buộc; tạo phiên bản quyết định mới `by admin` | Core thuần |

- Chỉ chủ nội dung tạo được (người khác: `not_found`). Phản đối được quyết định bất lợi
  (`rejected`, ẩn bình luận) của **AI hoặc moderator**; quyết định của admin là cấp cuối
  (`final_decision`); quyết định không bất lợi: `nothing_to_appeal`.
- Một hồ sơ cho mỗi phiên bản quyết định: gửi lại cùng key hay key khác trả hồ sơ đã có;
  phiên bản quyết định đã đổi: `stale_decision`.
- Khi hồ sơ mở, chỉ admin kết luận trên quyết định đó; moderator nhận `appeal_open`.
- Kết luận của admin tạo phiên bản quyết định mới; AI tới muộn bị `overridden`. Bản mới của tác
  giả có ca riêng; kết luận đảo cho bản cũ chỉ công khai bản đó nếu ý định công khai **hiện hành**
  vẫn trỏ đúng bản (guard §3), nếu không là `approved_unpublished`.
- Tác giả chuyển riêng tư đóng hồ sơ phản đối của tác phẩm (`withdrawn`); kết luận sau đó bị từ chối.

Copy đề xuất khi chưa có delivery (chưa dựng UI):

> Cần quản trị viên xem xét. Kênh gửi yêu cầu chưa được kết nối; chưa có thông báo được gửi.

### 5.5. Dừng và bật chế độ tự động (MOD-14)

Admin dừng AI áp quyết định (có phiên bản, lý do). Khi dừng: lượt gửi mới vào thẳng hàng chờ
người; evaluation đang mở chuyển sang hàng chờ người (`rerouted`); kết quả AI tới sau đó bị từ chối
`automation_paused` và không ghi gì. Bật lại chỉ áp cho lượt gửi mới: ca đã chuyển tiếp tục theo
luồng người duyệt. Dừng tự động **không** dừng moderator/admin.

### 5.6. Gửi duyệt theo phiên bản (PUB-06)

Tạo, lưu, sửa bài riêng tư không mở ca. Chỉ yêu cầu công khai của tác giả kích hoạt kiểm duyệt
snapshot. Bản đã đăng vẫn hiển thị khi bản sửa đang chờ hoặc bị từ chối. Chuyển riêng tư đóng
evaluation (`withdrawn`), ca và phản đối đang mở của tác phẩm, giữ lịch sử quyết định. Công khai
lại:

| Quyết định hiện có của bản r | Kết quả |
| --- | --- |
| `pending`, evaluation còn mở | Chờ (`waiting`) |
| `pending`, evaluation đã đóng do chuyển riêng tư | Lượt AI mới (`resubmitted`), hoặc hàng chờ người nếu tự động đang dừng |
| `needs_admin` | Về hàng chờ người |
| `approved` ở policy hiện hành | Công khai ngay |
| `approved` của AI ở policy cũ | Lượt AI mới cho đúng snapshot (`rereview`), chưa công khai |
| `approved` của người ở policy cũ | Về hàng chờ người có thẩm quyền (`rereview`), chưa công khai |

### 5.7. Điều tra riêng (MOD-15)

Một admin đang có quyền, **không** là tác giả bài, mở hồ sơ khi có căn cứ vi phạm cụ thể, nguồn
bằng chứng và đúng ID của một bản **từng công khai** của bài đã chuyển riêng tư. Mỗi lần đọc: kiểm
lại quyền admin, xung đột lợi ích, hồ sơ còn mở, chưa hết hạn, đúng bản; ghi audit (người đọc, hồ
sơ, bản, thời điểm) **trước** khi trả đúng snapshot đó. Hồ sơ chỉ ghi kết luận và căn cứ, không
chép nội dung. Quyền kết thúc khi hồ sơ đóng hoặc điều kiện truy cập hết hiệu lực. Mặc định quyền
theo từng lần đọc; không có quyền đọc bản nháp hay bản chưa từng công khai.

## 6. Xóa, riêng tư, gỡ, trang ẩn và tombstone

Bốn trường hợp phải tách (PUB-05, đã chốt; B đổi ngày 11/11/2026):

| | Trường hợp | Trang ở URL cũ (người khác) | Nội dung, preview, listing | Bình luận đọc được |
| --- | --- | --- | --- | --- |
| A | Tác giả **xóa** bài từng công khai (đang công khai hoặc đang ở trang ẩn) | Trang thay thế: “Nội dung đã được tác giả xoá” | Không | Bình luận đã duyệt, chưa gỡ. Người viết thấy thêm bình luận chờ/bị từ chối **của mình** |
| B | Tác giả chuyển **Chỉ mình tôi** bài **đang công khai** | Trang ẩn: “Nội dung đã được ẩn” | Không | Như A. Công khai lại thì bài trở lại |
| C | Admin **gỡ** cả bài vì vi phạm | Không tìm thấy | Không | Không |
| D | Chưa từng công khai | Không tìm thấy | Không | Không |

- “Listing” gồm Khám phá, tìm kiếm, hashtag, Đề xuất, Top 100, bài liên quan và bộ sưu tập.
- “Preview” là mọi thứ **hệ thống** sinh từ nội dung bài: snippet, trích dẫn, thẻ chia sẻ, chữ
  trong thông báo. Chúng không được là đường đọc lại bài. Chữ người khác tự viết hay sao chép vào
  bình luận là nội dung của bình luận đó, theo vòng đời riêng; không hứa thu hồi mọi bản sao và
  không xóa hàng loạt lời bình.
- Xóa, ẩn hay gỡ bài **không** xóa hay sửa bình luận của ai.
- A và B không phải audience thứ ba: đó là ngữ nghĩa của “tác giả đã xóa” và của “chuyển riêng tư
  sau khi từng công khai”.

<!-- vdr: active O-03@r2 -->

- Trên trang ẩn và trang thay thế (COM-11): đọc bình luận, xử lý ca đã gửi (kiểm duyệt dùng
  snapshot bình luận, **không** đưa thân bài cho evaluator hay người duyệt), người viết tự gỡ được.
  Không bình luận, phản hồi, bản sửa hay “Hữu ích” mới (O-03).
- Bài đang công khai bị moderator **bác bản đang đăng**: bản đó ngừng hiển thị; URL cũ theo O-17
  (đề xuất tối thiểu: không tìm thấy cho tới khi có bản được duyệt).
- Khi công khai lại, thông báo cho tác giả bài chỉ áp dụng cho sự kiện **mới** sau lúc công khai;
  sự kiện phát sinh lúc ẩn bị bỏ qua (O-19 giữ danh mục sự kiện).
- Tombstone/trang ẩn không hiện tiêu đề, tên tác giả, thẻ hay hashtag (O-09).
- Còn `OPEN`: retention/purge vật lý (O-08), khôi phục bài đã xóa/gỡ (O-10).

## 7. Bảng quyền

| Hành động | Tác giả bài | Người khác / khách | Evaluator | Moderator (trong grant) | Admin |
| --- | --- | --- | --- | --- | --- |
| Đọc bản nháp, bản chưa công khai | Có | Không | Chỉ bản được gửi đánh giá | Chỉ bản có ca trong phạm vi | Chỉ bản có ca; bản từng công khai qua hồ sơ điều tra (§5.7) |
| Lưu bản, đổi audience, xóa bài | Có | Không (không phân biệt được với không tồn tại) | Không | Không | Không |
| Nộp kết quả đánh giá | Không | Không | Có, khi tự động đang bật, qua kiểm ở mục 4 | — | — |
| Nhận ca, trả ca, ghi chú, chuyển admin | Không | Không | Không | Có (tự né nội dung của mình) | Có, kèm nhận lại có lý do |
| Quyết định/ghi đè AI một bản | Không | Không | Không | Có, kèm lý do | Có, kèm lý do |
| Bác bản đang đăng | Không | Không | Không | Có (`works`) | Có |
| Gỡ cả bài vì vi phạm | Không | Không | Không | **Không** | Có |
| Ẩn/khôi phục bình luận đã duyệt | Không | Không | Không | Có (`comments`) | Có |
| Phản đối quyết định | Có (nội dung của mình; quyết định AI hoặc moderator) | Không | Không | — | — |
| Kết luận phản đối | Không | Không | Không | Không | Có (cấp cuối) |
| Cấp/thu hồi grant, quản lý nhóm, dừng/bật tự động | Không | Không | Không | Không | Có |
| Mở hồ sơ điều tra, đọc bản từng công khai | Không | Không | Không | Không | Có (không xung đột, từng lần đọc) |
| Sửa/xóa bình luận người khác | **Không** (COM-08) | Không | Không | Chỉ ẩn/gỡ theo kiểm duyệt | Chỉ ẩn/gỡ theo kiểm duyệt |

## 8. Use case

| # | Tình huống | Kết quả phải giữ | Scenario trong model |
| --- | --- | --- | --- |
| 1 | Lưu riêng rồi không làm gì | Không evaluation nào được tạo | `publication` bước 1–2 |
| 2 | Công khai r1, evaluator timeout, thử lại, duyệt | r1 chỉ công khai sau kết quả hợp lệ | bước 4–12 |
| 3 | r1 công khai; r2 bị từ chối | Người đọc vẫn thấy r1 | bước 13–16 |
| 4 | Tab cũ gửi “công khai” sau khi tab mới đã đổi | `stale`, không đổi gì | bước 18, 28 |
| 5 | Policy đổi khi đang chờ | Kết quả cũ bị bỏ; hỏi lại | bước 20–22 |
| 6 | Evaluator không chắc; admin duyệt; kết quả AI muộn từ chối | Quyết định admin giữ nguyên | bước 23–26 |
| 7 | Về “Chỉ mình tôi” trong lúc chờ; kết quả duyệt tới | `withdrawn`, không quyết định mới; công khai lại → lượt mới | bước 33–37 |
| 8 | Tác giả xóa; kết quả/admin tới sau | Không hồi sinh | `audience_moderation_override` cuối |
| 9 | Xóa bài có ba bình luận (duyệt, từ chối, chờ) | Trang thay thế giữ đúng bình luận đã duyệt | `tombstone` scenario A |
| 10 | Bài đang công khai chuyển riêng tư rồi mở link cũ | Trang ẩn + bình luận đã duyệt; gửi mới bị chặn; công khai lại thì bài trở lại | scenario B |
| 11 | Ẩn rồi xóa | Bình luận vẫn đọc qua URL cũ | scenario B |
| 12 | Bản AI duyệt ở policy cũ, bản người duyệt ở policy cũ, rồi công khai lại | AI duyệt lại đúng snapshot / về hàng chờ người; không công khai trước kết luận | `publication` `policy_rereview` |
| 13 | Thu hồi grant hoặc rời nhóm khi đang xử lý | Kết luận bị từ chối; ca về hàng chờ (trừ khi còn grant khác) | `governance` `grants_and_claims` |
| 14 | Hai moderator tranh ca | Người sau `stale_case` | `governance` `grants_and_claims` |
| 15 | AI đến muộn sau người duyệt, sau dừng tự động, sau chuyển riêng tư | `overridden` / `automation_paused` / `withdrawn` | `governance` `automation_and_late_results`, `publication` |
| 16 | Phản đối trùng, phản đối bản cũ, phản đối quyết định của moderator | Một hồ sơ; `stale_decision`; admin kết luận cuối | `governance` `appeals` |
| 17 | Đọc điều tra sai bản hoặc hết quyền | Từ chối, không audit “đã đọc” | `governance` `investigation` |

## 9. Bằng chứng và gap

Model, invariant, negative control và lệnh chạy: [README publication](../../services/vink-app/behavior/publication/README.md)
(M1, M3, M4 `governance.qnt`) và [README target](../../services/vink-app/behavior/target/README.md).
Bảng yêu cầu → invariant → gap: [registry](../development/product-decisions-registry.md).
Bằng chứng M1 gồm model và scoped pure Rust replay (#150; [coverage](../development/publication-core.md)).
M3 và M4 (quản trị kiểm duyệt, 11/11/2026) có replay qua core thuần. Các model target còn lại chỉ có
bằng chứng model. #158 có migration/API cho snapshot, ý định công khai và trạng thái tác giả; chưa
có migration cho grant/ca/phản đối/chế độ tự động/điều tra hay trường trang ẩn, chưa có worker,
evaluator/authority runtime, UI hay kênh thông báo.
