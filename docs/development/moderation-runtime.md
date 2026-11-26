# Work moderation runtime — MOD-RUNTIME-01 (#159 và phần hoàn thiện sau #170)

**Phạm vi:** lưu quyết định kiểm duyệt tác phẩm, consumer đánh giá trong `vink-worker`, ghế admin,
grant và nhóm, ca duyệt, dừng/bật tự động và trang ở URL cũ sau khi bản đang đăng bị bác
(`O-17@r2`), trang ẩn, phản đối quyết định và chi tiết kiểm duyệt dành cho tác giả. Nguồn luật: MOD-01..15, PUB-05/06, INV-08 trong [PRD](../../prd/requirements.md),
[đặc tả](../architecture/publication-moderation.md), [DecisionDelta](../architecture/product-decisions-20261110.md)
và [registry quyết định](../../decisions/README.md).

Bốn chiều trạng thái:

1. **Product decision:** MOD-07..15 theo DecisionDelta 10/11 và 11/11; `O-17@r2` là
   `PRODUCT_CONFIRMED`. `O-18@r1` (thời hạn grant mặc định) vẫn `OPEN`. Runtime dùng `expires_at`
   tùy chọn theo từng grant. Ngoại lệ bootstrap mock ngày 25/11/2026 ([#536](https://github.com/loveoverflowcom/vink/issues/536),
   PRD MOD-06) có nguồn là lời chủ sản phẩm trong issue. Proposal
   [`VDR-WORK-MOCK-EVALUATOR-BOOTSTRAP@r1`](../../decisions/proposals/VDR-WORK-MOCK-EVALUATOR-BOOTSTRAP.r1.toml)
   giữ `PENDING` tới khi có signoff đúng revision.
2. **Architecture decision:** không có ADR mới. Thứ tự khóa, lease và hàng ca duyệt dưới đây là
   thiết kế triển khai, không phải quyết định `Accepted`.
3. **Implementation level:** `persistence-api` + `worker-provider` (chỉ evaluator kịch bản; từ
   #536 chạy mặc định ở mọi profile, production chỉ có `approve`) + `web` + governance cho bình luận: reader thay thế, hàng ca tác phẩm, phản đối,
   dừng/bật tự động, và nối quyền kiểm duyệt bình luận (`admin` seat, grant cá nhân/nhóm
   `comments` toàn Vink hoặc theo `WorkId`, kế thừa `discussion_moderator_grants`, hàng chờ theo
   phạm vi + tự né và provenance quyết định). Chưa có AI provider thật, điều tra riêng, ca bình
   luận, UI kiểm duyệt bình luận hay UI quản lý grant/nhóm.
4. **Verification evidence:** xem mục 7.

## 1. Lưu trữ (`20261117000000_work_moderation_runtime.sql`)

| Bảng/cột | Nội dung |
| --- | --- |
| `work_publications` | Thêm `hidden_*`, `review_withdrawn_*`, `ever_public`, `public_epoch`; backfill theo con trỏ cũ. |
| `work_revision_moderation` | Mỗi quyết định giữ `policy_version`, provenance `moderator` (kèm `moderator_grant_id`) và `policy_rereview`. Gửi lại một quyết định pending giữ version. Mở lại approval dưới policy mới tăng đúng một version theo [quy tắc re-review và backout](publication-runtime.md#duyệt-lại-khi-policy-thay-đổi-275). |
| `work_revision_decision_history` | Mỗi `decision_version` một dòng chỉ thêm: trạng thái, provenance, người quyết, grant, lý do, `evaluation_id`, `case_id` hoặc `appeal_id`. |
| `work_evaluation_jobs` | Thêm `leased`, `withdrawn`, `rerouted`, `quarantined`, `claim_generation`, `claimed_by`, lease, `available_at`, `attempts` / `max_attempts` (3), `scenario`, `last_error`. |
| `moderation_admin_seats` / `_audit` | Ghế admin do CLI cấp. Chỉ thay đổi thật mới ghi audit. |
| `moderation_groups`, `moderation_group_members`, `moderation_grants` | Grant `works`/`comments` cho người hoặc nhóm, toàn Vink hoặc một `WorkId`, có thể có hạn. Grant bình luận cũ trong `discussion_moderator_grants` được đọc như grant kế thừa. |
| `moderation_directory_audit` | Cấp, thu hồi, tạo nhóm, thêm/bớt thành viên. |
| `comment_moderation_decisions` (`20261119000000`) | Audit chỉ thêm. `CHECK` và hai FK buộc đúng dạng `legacy_grant`, `governance_grant` hoặc `admin`. Trigger khi ghi kiểm tra actor, ghế hoặc grant, capability, phạm vi và hiệu lực tại `decided_at`. Dòng legacy giữ nguyên ID grant và lịch sử. |
| `moderation_cases`, `moderation_case_notes` | Tối đa một ca mở mỗi revision. Ca gắn digest và `decision_version`. Ghi chú nội bộ chỉ được lưu khi CAS version thành công. |
| `moderation_automation_state` / `_audit` | Chế độ, version, người đổi và lý do. |

Script rollback từ chối chạy khi đã có ca, grant, nhóm, ghế admin, audit, lịch sử quyết định,
provenance chỉ con người tạo, trạng thái job mới hay trang bị gỡ đang được phục vụ.

## 2. Thứ tự khóa

Mọi đường ghi cùng chiều với lệnh tác giả #158, discussion và endorsement
([`moderation_write`](../../services/vink-app/src/persistence/moderation_write.rs)):

`publications` FOR UPDATE → `work_publications` FOR UPDATE → `works` FOR SHARE →
`principals` FOR UPDATE (fence standing) → `moderation_automation_state` FOR SHARE →
`work_evaluation_jobs` FOR UPDATE → `moderation_cases` FOR UPDATE.

- `request-public` đọc chế độ tự động trong commit: khi đang dừng, revision vào hàng chờ người
  (`automation_paused`) và không tạo job.
- Kết luận ca đọc `work_id` của ca không khóa, rồi khóa tiền tố tác phẩm trước khi khóa ca.
- Dừng tự động khóa độc quyền dòng chế độ, rồi chuyển mọi job `open`/`leased` sang `rerouted`
  và mở ca cho revision còn chờ, trong cùng commit.

| Đường ghi | Thứ tự khóa liên quan tới duyệt bình luận |
| --- | --- |
| Quyết định bình luận | `FOR SHARE` trên ghế admin → grant legacy → nhóm → thành viên → grant governance, theo ID khi có nhiều dòng → `comments FOR UPDATE` trong phạm vi → `principals FOR UPDATE` → đọc standing bằng statement riêng → lấy `clock_timestamp()` → quyết định và audit trong một commit. |
| Thu hồi grant governance | Grant `FOR NO KEY UPDATE` → standing của admin → cập nhật grant, audit và kiểm lại ca. `FOR NO KEY UPDATE` chặn quyết định bình luận, đồng thời cho phép FK của ca lấy `FOR KEY SHARE`. |
| Thu hồi grant legacy | `UPDATE` grant đang hiệu lực. |
| Xóa thành viên nhóm | Nhóm `FOR NO KEY UPDATE` → standing của admin → xóa thành viên, audit và kiểm lại ca. |
| Bỏ ghế admin | `DELETE` ghế → audit (FK lấy `FOR KEY SHARE` trên principal) → kiểm lại ca. Kết luận phản đối tác phẩm lấy khóa ghế trước standing để giữ cùng chiều. |
| Đình chỉ principal | Ghi suspension lấy `FOR KEY SHARE` trên principal qua FK, xung đột với standing `FOR UPDATE`. |
| Người viết tự gỡ | Khóa bình luận → standing của người viết → gỡ. |
| Worker hậu kiểm | Khóa delivery → ticket → bình luận → receipt dịch vụ. Xem [thứ tự khóa hậu kiểm](discussion-post-review.md#khóa-và-commit). |

Nếu thay đổi quyền hoặc đình chỉ commit trước khi lấy được khóa tương ứng, lần duyệt thấy trạng thái
mới. Khi quyết định đã giữ khóa, thay đổi cạnh tranh chờ giao dịch kết thúc. Quyết định chỉ xét các
ghế, grant và thành viên đã khóa. Quyền mới xuất hiện trong lúc chờ có hiệu lực khi được khóa ở lần
kiểm kế tiếp. Grant hết hạn khi chờ bị loại tại mốc `decided_at`. Core có thể chọn grant hợp lệ khác
trong cùng giao dịch, và audit ghi căn cứ đã chọn.

Down script lấy `ACCESS EXCLUSIVE` trên bảng audit và cho phép backout khi chỉ có provenance
legacy. Sau quyết định bằng ghế admin hoặc grant governance, script từ chối để giữ lịch sử. Khi đó
dùng bản sửa hoặc migration roll-forward giữ ba dạng audit.

## 3. Consumer đánh giá trong `vink-worker`

- **Claim:** giao dịch ngắn chỉ claim khi chế độ là `enabled`. Claim tăng `claim_generation`
  và `attempts`, đặt lease (30 s). Lease hết hạn thì claim lại với cùng `EvaluationId`.
- **Evaluator** chạy ngoài giao dịch. Chỉ có evaluator kịch bản (`approve`, `reject`,
  `needs_admin`, `invalid_output`, `timeout`, `provider_error`). Kịch bản gắn bền vào job ở
  lần claim đầu.
- **Áp kết quả** trong một commit theo thứ tự khóa ở mục 2. Commit kiểm job còn `leased` bởi đúng
  `claim_generation` và `claimed_by`, đọc chế độ dưới khóa chia sẻ rồi gọi
  `apply_evaluator_result` với `EvaluatorCapability::under(mode)`. Claim đã bị thay, job bị
  reroute/rút hoặc quyết định đã có người làm đều không đổi gì. Core từ chối thì job đóng
  với mã lý do.
- **Thử lại nghiệp vụ** (`timeout`, `provider_error`): job hiện tại chuyển `failed` và `reevaluate`
  mở `EvaluationId` mới cho cùng lần gửi, mang theo số lượt. Hết 3 lượt thì revision vẫn `pending`
  và có ca `retries_exhausted`.
- **Không có kết quả** (không có evaluator hoặc đang dừng): job chuyển `rerouted` và người duyệt
  quyết định. Không bao giờ duyệt thay evaluator.
- **Cấu hình:** xem [mục 3.1](#31-mock-bootstrap-và-công-tắc-vận-hành-536).
- **Pilot AI đối chiếu song song (chưa triển khai):** thiết kế cho thử nghiệm AI ngoài trên môi trường
  `App + PostgreSQL` cô lập xem tại [đặc tả pilot AI kiểm duyệt tác phẩm](../architecture/moderation-ai-pilot.md)
  (`DESIGN_PROPOSED`, `NOT_IMPLEMENTED`, proposal [`VDR-WORK-AI-MODERATION-PILOT@r2`](../../decisions/proposals/VDR-WORK-AI-MODERATION-PILOT.r2.toml) `PENDING`);
  worker chính hiện tại vẫn không đọc nội dung bài trong `ClaimedJob` và `/health/ready` giữ `ai_moderation: not_implemented`.
  Vì mock bật mặc định từ #536, preflight pilot phải đặt tường minh `VINK_EVALUATOR=manual` hoặc
  admin dừng tự động. Pilot không được dựa vào mặc định.

### 3.1. Mock bootstrap và công tắc vận hành (#536)

Chủ sản phẩm chốt ngày 25/11/2026: “Để tạo PR mock evaluator và cho phép, trên cả vps hay
production nếu muốn. Mặc định là bật cho mock đã, tôi sửa sau.” ([#536](https://github.com/loveoverflowcom/vink/issues/536)).
Khi operator không đặt biến nào, consumer chạy với mock kịch bản `approve` ở mọi profile, kể cả
production. Mock không đọc nội dung, không gọi mạng và không cần khóa API. Nó duyệt mọi lần gửi
công khai hợp lệ của tác phẩm. Đây **không** phải AI hay kiểm duyệt nội dung. Mọi fence vẫn giữ
nguyên: validation, auth, standing, ý định công khai, revision/digest, lease, policy, claim và
quyết định. Lưu nháp và snapshot đơn thuần không tạo job. Bình luận không đổi.

| Biến (compose) | Giá trị | Ý nghĩa |
| --- | --- | --- |
| `VINK_WORKER_EVALUATION` (`VINK_PROD_WORKER_EVALUATION` / `VINK_TEST_WORKER_EVALUATION`) | `enabled` (mặc định, cả khi rỗng), `disabled` | Dừng kỹ thuật consumer. Khi `disabled`, job mới nằm `open`, không ai xét và không có ca người cho tới khi consumer chạy lại. |
| `VINK_EVALUATOR` (`VINK_PROD_EVALUATOR` / `VINK_TEST_EVALUATOR`) | `mock` (mặc định), `manual` | Với `manual`, consumer vẫn chạy nhưng không có evaluator. Mỗi job chuyển `rerouted` và mở ca `evaluator_uncertain`. Kịch bản đã gắn khi mock còn chạy không được dùng lại. |
| `VINK_MOCK_EVALUATION_SCENARIO` | production: chỉ `approve`; local: thêm `reject`, `needs_admin`, `invalid_output`, `timeout`, `provider_error` | Kịch bản gắn vào job ở lần claim đầu. Đặt cùng `manual` thì worker từ chối khởi động. Ở production, job mang kịch bản khác `approve` được chuyển cho người duyệt. |

Giá trị khác làm `vink-worker` dừng khi khởi động (exit 2). Compose production/test lấy hai biến có
tiền tố realm từ release env. `environment:` thắng `worker-runtime.env`, nên opt-out phải đặt trong
release env (`/etc/vink-prod/release.env` hoặc env render của test).

**Dừng nghiệp vụ** dùng admin dừng tự động (MOD-14, `POST /api/v1/moderation/automation/mode`).
Trạng thái nằm trong `moderation_automation_state`, thắng cấu hình và còn hiệu lực qua restart vì
worker không ghi bảng này. Khi dừng, job đang chờ chuyển cho người. Bật lại chỉ áp cho lần gửi mới.
Job đã `rerouted` và ca đã nhận không quay lại mock.

**Provenance.** `provenance_kind = 'ai_approved'` là nhãn vai trò actor evaluator. Nhãn này không
chứng minh có AI. Truy nguồn theo `work_revision_decision_history.evaluation_id` → `work_evaluation_jobs.id`.
`scenario` khác `NULL` nghĩa là verdict đến từ mock kịch bản: kịch bản được gắn trong commit claim,
trước khi áp. Chọn `CHOICE-02 A` (dùng liên kết hiện có, không migration). Job và lịch sử cùng xóa
theo tác phẩm. PR nối provider thật phải thêm provenance riêng và không dùng `scenario` làm dấu.

```sql
SELECT h.work_id, h.revision_id, h.decision_version, h.provenance_kind, j.id AS job, j.scenario
  FROM work_revision_decision_history h
  JOIN work_evaluation_jobs j ON j.id = h.evaluation_id
 WHERE h.provenance_kind LIKE 'ai_%';
```

Log khởi động ghi `evaluation`, `evaluator` (`scripted_mock` hoặc `manual`), `evaluation_mock` và
`ai_moderation=not_implemented`. Mỗi job ghi `evaluator`, `scripted_verdict`, `scenario` và outcome;
không ghi nội dung bài hay secret. `/health/ready` của App giữ `ai_moderation: not_implemented`.

**Rủi ro đã nhận:** mọi nội dung qua validation có thể lên công khai, kể cả spam hoặc vi phạm.
Đường can thiệp hiện có: admin dừng tự động, `manual`, moderator/admin bác bản đang đăng (MOD-13)
và admin gỡ. Không trình bày kết quả mock là AI đã kiểm duyệt.

## 4. HTTP API (`no-store`)

| Method | Path | Quyền |
| --- | --- | --- |
| `GET` | `/api/v1/moderation/works/queue` | Có grant `works` hoặc ghế admin. Chỉ liệt kê ca mà người gọi có quyền và không phải tự né. |
| `GET` | `/api/v1/moderation/comments` | Có ghế admin, grant cá nhân/nhóm `comments` (global hoặc `WorkId`) hoặc grant kế thừa. Lọc sẵn phạm vi tác phẩm và tự né trước `pending_total` và phân trang (`can_decide` luôn `true` trên từng mục trả về). |
| `POST` | `/api/v1/moderation/comments/{id}/decisions` | Kiểm lại standing, phạm vi, hạn và tự né (`403 moderator_recused`) sau khóa. Với grant theo tác phẩm, ID ngoài phạm vi và ID vắng mặt cùng trả `403 not_moderator`. Audit ghi provenance đã dùng. |
| `GET` | `/api/v1/moderation/works/cases/{id}` | Như trên. Trả snapshot đã gửi (`document`, `content_digest`), lịch sử ghi chú. Ca trên tác phẩm của chính mình trả `403 recused`. |
| `POST` | `.../cases/{id}/claim`, `release`, `note`, `escalate`, `takeover` | Theo core `ReviewCase`, fence `expected_version`. |
| `POST` | `.../cases/{id}/conclude` | `approved`/`rejected` (bác cần lý do), fence ca + `DecisionVersion`. |
| `GET`/`POST` | `/api/v1/moderation/grants`, `POST .../grants/{id}/revoke` | Admin. |
| `POST` | `/api/v1/moderation/groups`, `.../groups/{id}/members`; `DELETE .../members/{principal}` | Admin. |
| `GET` | `/api/v1/moderation/automation`; `POST .../automation/mode` | Admin; lý do bắt buộc khi dừng. |

Mỗi lệnh là một giao dịch có fence standing và audit. Người nhận ca mất mọi căn cứ quyền thì ca
trở về hàng chờ:

- thu hồi grant, bỏ ghế admin hoặc bớt thành viên nhóm: chuyển ngay trong commit đó;
- grant hết hạn hoặc bị đình chỉ: chuyển khi lệnh tiếp theo của họ bị từ chối, và lệnh đó vẫn trả
  `403`.

Còn căn cứ khác thì người nhận giữ ca, và kết luận ghi căn cứ đang dùng.

## 5. Trang ở URL cũ (`O-17@r2`)

- `GET /api/v1/works/{id}/publication` trả `404` ngay khi con trỏ bị rút, trước khi xét ETag.
- `GET /api/v1/works/{id}/reader` trả bản đang đăng hoặc hai dạng thay thế với `no-store`:
  `review_withdrawn` có `work_id`, `author_name`; `hidden` chỉ có `work_id`.
- Chuyển bài đang công khai sang riêng tư phục vụ “Nội dung đã được ẩn”, HTTP 200 + `noindex`.
  Không có tên tác giả, tiêu đề, thân bài hay thẻ. Bình luận đủ điều kiện vẫn đọc được, người viết
  vẫn tự gỡ được; bình luận mới bị `409 hidden_read_only`. Công khai lại theo quyết định còn hiệu
  lực thay trang ẩn bằng bản đã duyệt. Bài chưa từng công khai vẫn trả không tìm thấy.
- Web/SSR: HTTP 200, `noindex`, `no-store`, tên tác giả, tiêu đề “Bản này đã bị gỡ sau khi kiểm
  duyệt.” và thảo luận hai tầng. Không có thân bài, tiêu đề, thẻ hay metadata của bản bị gỡ. Client
  xóa cache tab và thẻ Khám phá của tác phẩm.
- Thảo luận: khách đọc bình luận đủ điều kiện. Người viết vẫn thấy và tự gỡ được bình luận của
  mình. Bình luận đã gửi vẫn được xử lý. Bình luận mới và trả lời bị chặn ở server.
- Gửi duyệt một revision mới **giữ** trang cho tới khi bản đó được duyệt và đúng ý định công khai.
  `set-private` đóng trang và rút cả tên tác giả.
- Chỉ quyết định bác (`Rejected`) mới tạo trang. Một kết quả chuyển người duyệt trên bản đang
  đăng chỉ rút con trỏ.

## 6. Phản đối quyết định tác phẩm (MOD-10)

Migration mới `20261118000000_work_appeals.sql` không sửa migration đã merge. `work_appeals`
ràng buộc duy nhất `(work_id, revision_id, decision_version)`; lịch sử quyết định tham chiếu
`appeal_id`. Rollback chỉ cho phép khi chưa có hồ sơ; không xóa lịch sử đã phát sinh.

- Tác giả gửi phản đối bản bị evaluator/moderator bác qua
  `POST /api/v1/works/{work}/revisions/{revision}/appeals`, kèm `expected_decision_version`.
  Gửi lặp hoặc đồng thời trả cùng hồ sơ. Quyết định admin là cuối cùng.
- Tác giả đọc lịch sử bằng `GET /api/v1/works/{work}/appeals`. Admin đọc hàng chờ, snapshot đã gửi
  và kết luận bằng `GET /api/v1/moderation/appeals`, `GET .../{id}`, `POST .../{id}/conclude`.
  Mọi response là `no-store`; snapshot không vào HTML SSR. Admin tự né tác phẩm của mình.
- Adapter gọi `AppealBook` và `apply_appeal_conclusion`, khóa tác phẩm cùng thứ tự ở mục 2,
  fence standing/ghế admin/phiên bản quyết định trong commit. `uphold`/`reverse` tạo quyết định
  admin mới và lịch sử; chỉ khôi phục bản công khai khi ý định hiện hành vẫn trỏ đúng revision.
- Ca duyệt thường có phản đối đang mở trả `409 appeal_open`; admin dùng đường kết luận phản đối.
  Chuyển riêng tư đóng phản đối cùng giao dịch, chặn snapshot và mọi kết luận muộn.
- Không có kênh thông báo: `notice = not_sent`. UI nói rõ đã ghi nhận hồ sơ, chưa gửi thông báo.

## 7. Web kiểm duyệt

Từ [#555](https://github.com/loveoverflowcom/vink/issues/555), Product Web dành cho độc giả và tác giả.
Trang Tôi không thăm dò quyền hàng chờ. Ba URL cũ của nhân sự dẫn tới trang 404;
`/:author` chỉ phân giải hồ sơ khi segment bắt đầu bằng `@`. Studio Admin được hoãn.

| Đường dẫn | Người dùng và chức năng |
| --- | --- |
| `/ban-viet?review={id}` | Tác giả mở chi tiết trong Bàn viết để xem trạng thái, lý do, lịch sử quyết định và phản đối. |
| `/ban-viet/{id}/kiem-duyet` | URL tương thích chuyển đến chi tiết trong Bàn viết. |
| `/api/v1/moderation/*` | Nhân sự dùng phiên được cấp quyền, theo hợp đồng các mục trên. Server kiểm standing, phạm vi, tự né và version tại lệnh. |

Khi mở lại chi tiết hoặc sau khi gửi phản đối, Bàn viết đọc snapshot mới. Nếu kết quả ghi
không rõ hoặc xung đột, chỉ đọc để đối chiếu. “Thử lại” xuất hiện khi lượt đọc có thể phục hồi.
Đổi phiên hoặc đổi đích kết thúc quyền áp dụng kết quả của lượt trước.
Chọn audience/gửi duyệt/chuyển
riêng tư nằm ở [vùng công khai của Bàn viết](publication-author-web.md). Chi tiết
[#477](https://github.com/loveoverflowcom/vink/issues/477) dùng chung DTO trạng thái và nhãn quyết định
với vùng đó. Khi mở chi tiết, `GET …/publication/review` trả trạng thái công khai, lịch sử quyết định
và hồ sơ phản đối từ cùng một snapshot PostgreSQL. Lịch sử chỉ trả quyết định đã kết luận, theo đúng
revision/version và phép chiếu lý do dành cho tác giả của P-12A. API dùng `no-store` và trả `404`
ngoài phạm vi sở hữu.
URL thông báo tác phẩm thêm `revision` và `decision` vào URL chi tiết. Xem
[LayoutSpec R4.4](../design/redesign-r4/desk-moderation.md).

## 8. Rollout

Dừng tiến trình ghi cũ trước khi migrate, rồi chạy App và worker cùng phiên bản. Từ #536, consumer
chạy mặc định với mock `approve`. Không có migration hay backfill: ca người đang chờ và job đã
`rerouted` giữ nguyên. Nếu consumer từng tắt trong khi tự động vẫn bật, các job `open` tích lũy sẽ
được mock duyệt khi worker mới chạy. Trước khi deploy, đếm
`SELECT count(*) FROM work_evaluation_jobs WHERE state IN ('open', 'leased')`. Muốn người xét
backlog đó, admin dừng tự động trước (job chuyển thành ca người), rồi mới chạy worker mới. Muốn mọi
lần gửi qua người, đặt `VINK_{PROD,TEST}_EVALUATOR=manual`. Quay lui: đặt `manual` hoặc
`VINK_{PROD,TEST}_WORKER_EVALUATION=disabled` trong release env rồi tạo lại container worker. Cần theo dõi tuổi backlog
`work_evaluation_jobs`, lease quá hạn, ca `queued`, phản đối `open` và job `quarantined`.
CLI `vink-app admin add|list|remove` nhả connection trước khi đóng pool; không còn treo sau khi
đã in kết quả (nguyên nhân lane Web của merge #170 bị hủy sau timeout).

## 9. Bằng chứng và giới hạn

- #536: `evaluation_worker::tests` phủ ma trận cấu hình (không biến/rỗng, `disabled`, `manual`,
  kịch bản production/local, giá trị sai, `manual` + kịch bản). `tests/postgres/work_moderation.rs`
  dùng đúng cài đặt production: request-public → mock → công khai đúng revision và đọc qua reader,
  provenance theo job, admin dừng qua hai lần khởi động, `manual` không dùng lại kịch bản đã gắn,
  kịch bản khác `approve` ở production chuyển người, rút riêng tư và quyết định người trước finish.
  Kết quả chạy theo SHA ghi trong PR.
- `cargo xtask verify postgres`: `tests/postgres/work_moderation.rs` phủ claim cũ/dừng giữa claim
  và commit, gửi khi dừng, retry, ca/grant/nhóm/tự né; thêm CLI admin có timeout, reader hidden,
  ghi bình luận bị chặn/tự gỡ/công khai lại, phản đối gửi đồng thời, admin final, kết luận đồng
  thời, phiên bản cũ, riêng tư và ý định đã chuyển sang bản mới. Chuỗi upgrade/backout dùng
  migration thật; không phải model evidence. `tests/postgres/comment_moderation_governance.rs` phủ
  đường kiểm duyệt bình luận với admin/grant cá nhân/nhóm/kế thừa, tách `works`/`comments`, phạm
  vi `WorkId`, tự né, fallback provenance, phân trang/cursor khi thu hẹp quyền, tác phẩm ẩn/rút
  duyệt (`O-03@r2`), schema provenance/upgrade/backout và hậu kiểm. Các test dùng khóa và
  `pg_blocking_pids` cho thu hồi grant, bỏ ghế, rời nhóm, quyền mới xuất hiện trong lúc chờ và
  quan hệ khóa với FK của ca hoặc kết luận phản đối.
- `cargo xtask verify discussion`: M3 có `ReviewWithdrawn`, 53/53 bước qua pure production và
  counterexample `mutatedWithdrawnStep` (không được để lộ body). M1 64 và M4 105 giữ nguyên.
  Simulation có biên; Apalache `NOT_RUN`. M3 không mô hình hóa HTML hay tên tác giả.
- `cargo xtask verify web --scenario review`: ca duyệt, kết luận, đảo quyết định qua API;
  SSR/hydration/cache trang bị gỡ, tác giả gửi phản đối trong Bàn viết, ẩn bài và công khai lại. Ảnh desktop/mobile ở output lane; snapshot riêng không nằm trong SSR.
- Kết quả chạy, SHA và CI của phần hoàn thiện ở [PR #171](https://github.com/loveoverflowcom/vink/pull/171).
  Danh sách trên là phạm vi suite; chỉ kết quả trên đúng SHA mới là bằng chứng PASS. CI merge #170
  (`25dc464`) bị hủy ở Web sau CLI cấp ghế admin, không phải bằng chứng smoke reader đã qua.

## 10. Hai queue hữu hạn (#205)

Lựa chọn kỹ thuật: [VDR-MODERATION-QUEUE-PAGINATION@r1](../../decisions/proposals/VDR-MODERATION-QUEUE-PAGINATION.r1.toml), **PENDING**. Model/replay [ReadOwner/PageChain](../../apps/vink-web/behavior/README.md) chỉ phủ primitive; quyền/rows trong snapshot vẫn cần PostgreSQL, authority fingerprint/session clear cần browser.

Mức triển khai hiện tại `persistence-api`. Phần Web dưới đây ghi lại implementation lịch sử #205 trước khi #555 gỡ consumer staff; không thay quyết định sản phẩm hay nhận ADR Accepted.
`ADM-01@r1 PRODUCT_CONFIRMED`, `O-18@r1 OPEN`; snapshot registry tại base `6de515d`:
`cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`.

- Ca tác phẩm `/api/v1/moderation/works/queue/page` và admin open appeals
  `/api/v1/moderation/appeals/page` trả `items`, `next_cursor`, `authority_key`.
  Mặc định 50, tối đa 100; oldest-first `(created_at,id)`; lookahead chỉ từ tập được phép.
- CHOICE-01 A: envelope trên route `/page`; route cũ giữ array hữu hạn và `Link rel=next`.
  Chỉ tương thích dạng array với bundle cũ: bundle đó bỏ qua Link và chỉ thấy trang đầu. Rollout
  cần server mới trước bundle mới và tải lại tab kiểm duyệt cũ trước khi tiếp tục duyệt. Không
  fallback lỗi thành empty. [Contract/fixtures](../../services/vink-app/contracts/http/v1/README.md#workappeal-queue-continuation-205).
- CHOICE-02 B: transaction repeatable-read read-only; directory chỉ tải facts liên quan actor,
  `ModerationDirectory::capability_scope` tạo projection global/work IDs và SQL lọc trước limit.
  Admin/standing/expiry và rows cùng snapshot; tự né chủ nội dung/chủ tác phẩm, public-active.
  Không load tất cả ca hoặc N+1 từng ca. Phạm vi quyền đổi giữa trang có fingerprint mới; Web bỏ
  chuỗi private cũ kể cả khi actor vẫn được 200 nhờ grant khác. Cursor không giữ quyền.
- QueuePanel có owner/ticket/PageChain riêng mỗi vùng, session generation và disposal fence.
  Refresh supersede append, reset chain minh bạch; lỗi tạm giữ trang/cursor, invalid cursor cho
  restart, 401/403/404 xóa dữ liệu. Lifecycle hints chỉ đọc có backoff, không chạy lại lệnh.
  Automation/reason không remount theo paging. Detail/command vẫn có authority/version fence riêng.
  Link giữ key theo ID nhưng nhãn đọc reactive từ rows hiện tại, nên refresh cập nhật trạng thái
  ca đã được người khác nhận mà không thay phần tử link.
- Sau đo global 6.000 ca thấy sort toàn hàng (~3,966 ms ở corpus local), migration mới
  `20261121000000_moderation_queue_order.sql` thêm partial index `(created_at,id)` cho open work cases.
  Sparse grants dùng index work/revision hiện có; appeal dùng `work_appeals_open`. Backout chỉ bỏ
  derived index, không xóa queue/audit. Plan/timing là dữ liệu local, không SLA production.
  Review #223 đo thêm generic prepared plan: nullable cursor/global predicate có thể làm quét
  toàn corpus. Hai queue dùng `SET LOCAL plan_cache_mode = force_custom_plan` trong transaction
  đọc (cùng mẫu với discovery); không thay cấu hình session/pool. Regression gọi mỗi queue tám
  lần trên một connection, kiểm `pg_prepared_statements` và khôi phục setting cả khi mất quyền.

Inventory: hàng bình luận đã có PageCursor/PageLimit và không bị thay ở lát này. Author appeal
history `/works/{work}/appeals`, revision/decision history và ghi chú chi tiết vẫn theo contract
hiện có, chưa được phân trang trong #205. Không có private durable cache, SSR private payload,
offline moderation, native consumer hay thay luật grant/expiry/recusal/admin.

[LayoutSpec](../design/moderation-queue-pagination.md). Bằng chứng final code và giới hạn được ghi
trong [evidence #205](evidence/moderation-queue-205/README.md).
