# Work moderation runtime — MOD-RUNTIME-01 (#159 và phần hoàn thiện sau #170)

**Phạm vi:** lưu quyết định kiểm duyệt tác phẩm, consumer đánh giá trong `vink-worker`, ghế admin,
grant và nhóm, ca duyệt, dừng/bật tự động và trang ở URL cũ sau khi bản đang đăng bị bác
(`O-17@r2`), trang ẩn, phản đối quyết định và Web duyệt tác phẩm. Nguồn luật: MOD-01..15, PUB-05/06, INV-08 trong [PRD](../../prd/requirements.md),
[đặc tả](../architecture/publication-moderation.md), [DecisionDelta](../architecture/product-decisions-20261110.md)
và [registry quyết định](../../decisions/README.md).

Bốn chiều trạng thái:

1. **Product decision:** MOD-07..15 theo DecisionDelta 10/11 và 11/11; `O-17@r2` là
   `PRODUCT_CONFIRMED`. `O-18@r1` (thời hạn grant mặc định) vẫn `OPEN`. Runtime dùng `expires_at`
   tùy chọn theo từng grant.
2. **Architecture decision:** không có ADR mới. Thứ tự khóa, lease và hàng ca duyệt dưới đây là
   thiết kế triển khai, không phải quyết định `Accepted`.
3. **Implementation level:** `persistence-api` + `worker-provider` (chỉ evaluator kịch bản cho
   local/test) + `web` + governance cho bình luận: reader thay thế, hàng ca tác phẩm, phản đối,
   dừng/bật tự động, và nối quyền kiểm duyệt bình luận (`admin` seat, grant cá nhân/nhóm
   `comments` toàn Vink hoặc theo `WorkId`, kế thừa `discussion_moderator_grants`, hàng chờ theo
   phạm vi + tự né và provenance quyết định). Chưa có AI provider thật, điều tra riêng, ca bình
   luận, UI kiểm duyệt bình luận hay UI quản lý grant/nhóm.
4. **Verification evidence:** xem mục 7.

## 1. Lưu trữ (`20261117000000_work_moderation_runtime.sql`)

| Bảng/cột | Nội dung |
| --- | --- |
| `work_publications` | Thêm `hidden_*`, `review_withdrawn_*`, `ever_public`, `public_epoch`; backfill theo con trỏ cũ. |
| `work_revision_moderation` | Mỗi quyết định giữ `policy_version`, provenance `moderator` (kèm `moderator_grant_id`) và `policy_rereview`. Gửi lại sau khi chuyển riêng tư giữ nguyên `decision_version` của core. |
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
- **Cấu hình:** consumer tắt mặc định, chỉ bật khi `VINK_WORKER_EVALUATION=enabled`. Nếu
  `VINK_MOCK_EVALUATION_SCENARIO` được đặt ở profile production, worker từ chối khởi động. Ở
  production, `scenario` trên job bị bỏ qua.
- **Pilot AI đối chiếu song song (chưa triển khai):** thiết kế cho thử nghiệm AI ngoài trên môi trường
  `App + PostgreSQL` cô lập xem tại [đặc tả pilot AI kiểm duyệt tác phẩm](../architecture/moderation-ai-pilot.md)
  (`DESIGN_PROPOSED`, `NOT_IMPLEMENTED`, proposal [`VDR-WORK-AI-MODERATION-PILOT@r1`](../../decisions/proposals/VDR-WORK-AI-MODERATION-PILOT.r1.toml) `PENDING`);
  worker chính hiện tại vẫn không đọc nội dung bài trong `ClaimedJob` và `/health/ready` giữ `ai_moderation: not_implemented`.

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

[LayoutSpec](../design/moderation-workspace.md) dùng các primitive và token hiện có:

| Đường dẫn | Người dùng và chức năng |
| --- | --- |
| `/kiem-duyet` | Hàng ca trong phạm vi của người duyệt; admin thêm hàng phản đối và chế độ tự động. |
| `/kiem-duyet/ca/{id}` | Snapshot đã gửi, trạng thái/version, nhận/trả/chuyển admin, ghi chú và kết luận có lý do. |
| `/kiem-duyet/phan-doi/{id}` | Admin đọc snapshot, giữ nguyên hoặc đảo quyết định có lý do. |
| `/ban-viet/{id}/kiem-duyet` | Tác giả xem trạng thái bản gửi/lý do, gửi phản đối và xem lịch sử. |

Đọc riêng tư bằng `LocalResource`, kết quả gắn session scope và route; phản hồi của tài khoản cũ
không được hiển thị. Chỉ dựng nội dung và control riêng tư sau khi tab xác nhận scope tài khoản;
response thành công bằng cookie khi scope còn chưa biết không được dựng form rồi làm mất chữ/focus
khi xác nhận phiên hoàn tất. Browser scenario giữ chậm response tài khoản thật và kiểm tra refresh
cùng tài khoản giữ nguyên ô lý do, chữ và focus. Nút pending khóa gửi trùng, không đổi quyết định lạc quan. Lỗi mạng/CAS giữ
lý do và tải lại trạng thái để đối chiếu. UI không cấp quyền: server vẫn kiểm mọi lệnh.
Chưa có UI cấp grant/nhóm, điều tra riêng, ca bình luận hoặc sửa policy. Chọn audience/gửi duyệt/chuyển
riêng tư nằm ở [vùng công khai của Bàn viết](publication-author-web.md); trang
`/ban-viet/{id}/kiem-duyet` dùng cùng một DTO trạng thái và cùng lời gọi tên quyết định với vùng đó.

## 8. Rollout

Dừng tiến trình ghi cũ trước khi migrate, rồi chạy App và worker cùng phiên bản. Bật consumer khi
có người duyệt. Khi không có evaluator, mọi lần gửi vào hàng chờ người. Cần theo dõi tuổi backlog
`work_evaluation_jobs`, lease quá hạn, ca `queued`, phản đối `open` và job `quarantined`.
CLI `vink-app admin add|list|remove` nhả connection trước khi đóng pool; không còn treo sau khi
đã in kết quả (nguyên nhân lane Web của merge #170 bị hủy sau timeout).

## 9. Bằng chứng và giới hạn

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
- `cargo xtask verify web`: phase `review-withdrawn` đi qua ca duyệt bằng UI, moderator bác bằng
  API, SSR/hydration/cache trang bị gỡ, tác giả gửi phản đối bằng UI, admin đảo bằng UI, ẩn bài và
  công khai lại bằng API. Ảnh desktop/mobile ở output lane; snapshot riêng không nằm trong SSR.
- Kết quả chạy, SHA và CI của phần hoàn thiện ở [PR #171](https://github.com/loveoverflowcom/vink/pull/171).
  Danh sách trên là phạm vi suite; chỉ kết quả trên đúng SHA mới là bằng chứng PASS. CI merge #170
  (`25dc464`) bị hủy ở Web sau CLI cấp ghế admin, không phải bằng chứng smoke reader đã qua.
