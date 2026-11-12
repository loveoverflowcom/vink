# Work moderation runtime — MOD-RUNTIME-01 (#159)

**Phạm vi:** lưu quyết định kiểm duyệt tác phẩm, consumer đánh giá trong `vink-worker`, ghế admin,
grant và nhóm, ca duyệt, dừng/bật tự động và trang ở URL cũ sau khi bản đang đăng bị bác
(`O-17@r2`). Nguồn luật: MOD-01..15, PUB-05/06, INV-08 trong [PRD](../../prd/requirements.md),
[đặc tả](../architecture/publication-moderation.md), [DecisionDelta](../architecture/product-decisions-20261110.md)
và [registry quyết định](../../decisions/README.md).

Bốn chiều trạng thái:

1. **Product decision:** MOD-07..15 theo DecisionDelta 10/11 và 11/11; `O-17@r2` là
   `PRODUCT_CONFIRMED`. `O-18@r1` (thời hạn grant mặc định) vẫn `OPEN`: người cấp tự nhập thời hạn.
2. **Architecture decision:** không có ADR mới. Thứ tự khóa, lease và hàng ca duyệt dưới đây là
   thiết kế triển khai, không phải quyết định `Accepted`.
3. **Implementation level:** `persistence-api` + `worker-provider` (chỉ evaluator kịch bản cho
   local/test) + `web` cho trang ở URL cũ. Chưa có AI provider thật, UI quản trị, appeal, điều tra
   riêng, ca bình luận hay trang “Nội dung đã được ẩn”.
4. **Verification evidence:** xem mục 7.

## 1. Lưu trữ (`20261117000000_work_moderation_runtime.sql`)

| Bảng/cột | Nội dung |
| --- | --- |
| `work_publications` | Thêm `hidden_*`, `review_withdrawn_*`, `ever_public`, `public_epoch`; backfill theo con trỏ cũ. |
| `work_revision_moderation` | Mỗi quyết định giữ `policy_version`, provenance `moderator` (kèm `moderator_grant_id`) và `policy_rereview`. Gửi lại sau khi chuyển riêng tư giữ nguyên `decision_version` của core. |
| `work_revision_decision_history` | Mỗi `decision_version` một dòng chỉ thêm: trạng thái, provenance, người quyết, grant, lý do, `evaluation_id` hoặc `case_id`. |
| `work_evaluation_jobs` | Thêm `leased`, `withdrawn`, `rerouted`, `quarantined`, `claim_generation`, `claimed_by`, lease, `available_at`, `attempts` / `max_attempts` (3), `scenario`, `last_error`. |
| `moderation_admin_seats` / `_audit` | Ghế admin do CLI cấp. Chỉ thay đổi thật mới ghi audit. |
| `moderation_groups`, `moderation_group_members`, `moderation_grants` | Grant `works`/`comments` cho người hoặc nhóm, toàn Vink hoặc một `WorkId`, có thể có hạn. Grant bình luận cũ trong `discussion_moderator_grants` được đọc như grant kế thừa. |
| `moderation_directory_audit` | Cấp, thu hồi, tạo nhóm, thêm/bớt thành viên. |
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
- **Cấu hình:** consumer tắt mặc định, chỉ bật khi `VINK_WORKER_EVALUATION=enabled`. Nếu
  `VINK_MOCK_EVALUATION_SCENARIO` được đặt ở profile production, worker từ chối khởi động. Ở
  production, `scenario` trên job bị bỏ qua.

## 4. HTTP API (`no-store`)

| Method | Path | Quyền |
| --- | --- | --- |
| `GET` | `/api/v1/moderation/works/queue` | Có grant `works` hoặc ghế admin. Chỉ liệt kê ca mà người gọi có quyền và không phải tự né. |
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
- `GET /api/v1/works/{id}/reader` trả bản đang đăng hoặc
  `{"status":"review_withdrawn","work_id","author_name"}` với `no-store`.
- Web/SSR: HTTP 200, `noindex`, `no-store`, tên tác giả, tiêu đề “Bản này đã bị gỡ sau khi kiểm
  duyệt.” và thảo luận hai tầng. Không có thân bài, tiêu đề, thẻ hay metadata của bản bị gỡ. Client
  xóa cache tab và thẻ Khám phá của tác phẩm.
- Thảo luận: khách đọc bình luận đủ điều kiện. Người viết vẫn thấy và tự gỡ được bình luận của
  mình. Bình luận đã gửi vẫn được xử lý. Bình luận mới và trả lời bị chặn ở server.
- Gửi duyệt một revision mới **giữ** trang cho tới khi bản đó được duyệt và đúng ý định công khai.
  `set-private` đóng trang và rút cả tên tác giả.
- Chỉ quyết định bác (`Rejected`) mới tạo trang. Một kết quả chuyển người duyệt trên bản đang
  đăng chỉ rút con trỏ.

## 6. Rollout

Dừng tiến trình ghi cũ trước khi migrate, rồi chạy App và worker cùng phiên bản. Bật consumer khi
có người duyệt. Khi không có evaluator, mọi lần gửi vào hàng chờ người. Cần theo dõi tuổi backlog
`work_evaluation_jobs`, lease quá hạn, ca `queued` và job `quarantined`.

## 7. Bằng chứng

`cargo xtask verify postgres` chạy `tests/postgres/work_moderation.rs` (luồng chính, trang ở URL
cũ khi gửi lại, claim cũ/dừng giữa claim và commit, gửi khi đang dừng, thử lại với
`EvaluationId` mới, ghi chú cũ, mất quyền khi kết luận, grant nhóm, tự né, bớt thành viên). Các
test này cùng với chuỗi rollback/upgrade của #158. Unit test cấu hình worker nằm trong
`evaluation_worker.rs`. Model Quint **chưa** có trạng thái `ReviewWithdrawn`: replay M3 không phủ
trang này. Lane `web` chưa có smoke riêng cho trang bị gỡ.
