# Hậu kiểm do operator yêu cầu — DISC-POST-REVIEW-RUNTIME

Nguồn: prompt người dùng `DISC-POST-REVIEW-RUNTIME`, tiếp nối #145/#126/#127; base thực thi
`develop@f3af600` (Web #147 đã merge), hợp nhất quyết định/model đích #149
từ `develop@5e218d7` trước khi merge. Phạm vi MOD-02/03/05/06, COM-02/03/08 và D-08.
Đây là capability operator mới, không phải admission đăng-trước. D-08 vẫn **PROPOSED**;
`premoderation.v1` và constraint `comments_admission_premoderation_only` giữ nguyên.

## DecisionMap

| Lựa chọn | Chọn và lý do | Phương án khác / giới hạn |
| --- | --- | --- |
| 01 nguồn ticket | A: operator giữ credential App yêu cầu tái kiểm một comment approved/present tại version đã quan sát. Use case: kiểm lại báo cáo chính sách sau một quyết định người | B không đủ trust/rules/quyết định D-08; C chỉ fixture không đáp ứng use case hiện tại. Không tự sinh ticket mỗi lần người duyệt approve |
| 02 kết quả | Operator nộp `clear/flagged/unresolved` qua CLI, note ghi người làm và lý do; authority giống `tag`/`moderator`/`outbox requeue` | Không AI provider/classifier, không Clear giả khi thiếu cấu hình. Structured result là authority operator, không phải email hay quyền tác giả |
| 03 human fence | Mọi approve/reject có expected version hiện tại tăng moderation version + audit, kể cả cùng trạng thái | Epoch riêng thêm trục đồng bộ mà API không có command key để phân biệt replay. Retry version cũ trả `409`, không là intent mới; gỡ đã gỡ vẫn no-op |
| 04 kết thúc | Finalization trong chính transaction đưa delivery về dead, ở claim/complete/record_failure | Queue finalization riêng thêm nghĩa vụ scheduling không cần thiết với effect nội bộ DB. Lỗi DB rollback cả effect/receipt/dead; hết lease sẽ tiếp tục, không đánh giá vô hạn |

## Producer → result → worker → read

1. `vink-app post-review request` → application port → `PgStore::request_review`: khóa command
   key, đọc replay chính xác, kiểm switch, khóa comment, kiểm approved/present + expected version.
   Sinh ticket ID phía server, digest SHA-256 của text đang lưu (`vink.comment-review.v1`), origin
   `operator_rereview`, policy `operator-rereview.v1`, note + database role, giờ tạo/deadline.
   Ticket + event `comment.rereview_requested` schema 1 + delivery `operator_post_review.v1`
   commit nguyên tử. Một nghĩa vụ/comment/version; key khác ở cùng version bị conflict.
2. `post-review result` khóa command key → delivery → ticket. Chỉ nhận trước deadline, khi còn
   mở và không quarantine. Kết quả gắn toàn bộ binding qua composite FK, append-only. Cùng key
   và toàn bộ payload/note → replay, kể cả sau khi đã xử lý; đổi key/payload không ghi đè kết quả.
   Không cướp lease đang chạy; delivery ready được đánh thức sớm. Không network trong transaction.
3. `vink-worker` giữ consumer shadow cũ riêng. Consumer mới khóa/fence delivery, rồi ticket và
   comment; kiểm type/schema/comment/version/origin/policy/digest và receipt. Đọc clock sau các
   khóa. `decide_review` là pure production seam: clear chỉ keep đúng trạng thái quan sát;
   flagged/unresolved → pending/version+1; human mới hơn → superseded; gỡ → removed. Receipt
   service `post_review_service.v1` commit với effect và outcome, không giả principal/grant người.
4. Read hiện có đọc trạng thái canonical: public list/count/preview/context/endorsement bỏ text
   bị hold; own overlay trả pending; moderation queue nhận case. Không đổi status/wire fields,
   không đưa origin/note/digest/audit vào DTO công khai. Reader Web #147 dùng cùng contract.

Digest này gắn text bình luận, khác digest command gửi (gắn cả work/target). Không có API sửa
text; nếu text DB bị thay ngoài contract, worker quarantine thay vì áp kết quả lên chữ khác.
Author remove đã purge body được nhận diện bằng terminal removal, không cần dựng lại nội dung.

## Khóa và commit

```text
submit:         publication SHARE → thread SHARE → target SHARE → principal UPDATE → rows mới
moderator:      directory SHARE (admin/legacy/groups/members/grants) → comment UPDATE → principal UPDATE → human audit
author remove:  comment UPDATE → principal UPDATE
request:        command-key advisory → comment UPDATE → ticket/event/delivery MỚI
result:         command-key advisory → delivery UPDATE → ticket UPDATE → result
worker:         delivery UPDATE → ticket UPDATE → comment UPDATE → service receipt
requeue:        delivery UPDATE → requeue audit (không khóa comment/ticket)
```

Quyết định của người duyệt kiểm tra standing và căn cứ quyền đã khóa tại mốc `decided_at`.
Receipt của worker ghi `post-review_service.v1`. Xem [khóa governance](moderation-runtime.md#2-thứ-tự-khóa).

Không human/submit/remove nào chờ delivery/ticket. Request không chạm delivery đã có; result
không khóa comment. Advisory key không được human/worker lấy. Không có chiều khóa ngược trong
các đường này. Hai batch claim có thể chạm cùng comment, nhưng không chờ delivery của batch kia.
Khóa effect/claim bị giới hạn 2 s, statement 5 s; supervisor timeout 10 s, lease 30 s. Failure
transaction chờ khóa tối đa 6 s. Quyền worker là capability dịch vụ chỉ giữ lại, không approve;
không phụ thuộc phiên/standing của người viết sau khi nghĩa vụ đã commit.

## Deadline, hết retry, poison và phục hồi

- Policy operator v1: **15 phút từ lúc server lập ticket**, bất biến trong envelope. Đến hạn luôn
  `Unresolved → hold`, kể cả có Clear nhưng worker chưa kịp commit. Đây là bound của luồng
  operator này, không SLA tự động kiểm duyệt của sản phẩm. Budget delivery mặc định 8 attempt;
  hết trước deadline cũng chuyển Unresolved. Không có network/evaluator retry ngầm.
- Hết retry tại claim (kể cả crash attempt cuối), complete hoặc record_failure gọi cùng adapter
  finalization dưới khóa/fence hiện hành trước khi ghi dead. Có receipt rồi → không effect lần hai.
  Lỗi DB lúc hold/receipt/dead khiến toàn transaction rollback; lease/debt vẫn tồn tại qua restart.
- Lost ACK sau commit: delivery không còn leased, old generation bị từ chối trước handler.
  Requeue dead giữ generation, ticket/deadline và `deadline_from`; claim sau tăng generation.
  Receipt đã có không bị xóa; human sau hold thắng, requeue không tái giữ quyết định đó.
- Poison/type mới/binding sai: quarantine có reason code, delivery dead, **không receipt hoàn tất**,
  không đoán comment từ envelope sai. Ticket (nếu có) và event giữ lại. Sửa bằng binary/migration
  roll-forward có audit đối chiếu nguồn; requeue sau sửa nguyên nhân. Quarantine là nợ cần người
  vận hành, không phải đánh giá thành công. Không có lệnh sửa provenance tại chỗ.

## Vận hành và backout

```sh
VINK_POST_REVIEW_REQUESTS=enabled vink-app post-review request <comment> <version> <request-key-uuid> '<người vận hành>: <lý do>'
vink-app post-review result <ticket> <result-key-uuid> clear '<người vận hành>: <kết luận theo policy>'
vink-app post-review result <ticket> <result-key-uuid> flagged '<người vận hành>: <lý do cần duyệt lại>'
vink-app post-review status
vink-app post-review pending
vink-app outbox status
vink-app outbox dead
vink-app outbox requeue <ticket> operator_post_review.v1 '<người vận hành>: <đã sửa nguyên nhân>'
```

Startup worker nêu cả hai consumer, `post_review=operator_rereview.v1`,
`admission_post_review=no_producer`; AI vẫn chưa triển khai. Các lệnh dùng config/credential
runtime App như những lệnh operator có sẵn. Lấy version/body
đang công khai qua read owner hiện có trước khi kết luận, giữ ticket ID để nộp kết quả. Note là
audit nội bộ, không ghi body, email, session hay raw output. CLI status chỉ count và tuổi lớn nhất
theo finite states: awaiting_result, result_ready, finalization_due, quarantined, kept, held,
superseded, removed; `overdue` là chỉ số chồng lên các nợ chưa xong. `pending` in tối đa 100 ID,
version, deadline/overdue và quarantine reason; không in body. Outbox status/dead bổ sung lỗi retry.

Creation switch **mặc định disabled**, chỉ `enabled` cho phép request mới; replay cũ và result/drain
vẫn hoạt động khi disabled. `VINK_WORKER_OUTBOX=disabled` là hard stop riêng: backlog còn nguyên,
cần đọc cả hai status trước/sau và lên lịch restart/drain. Không coi hard stop là đã giải quyết.
Muốn dừng rollout: tắt request mới, giữ worker và người trực queue tới khi hết nợ; không đổi admission.

Retention không prune ticket/result/service receipt/quarantine. Event chỉ prune sau khi mọi delivery
đã thành công/skipped đủ tuổi **và** có receipt cho ticket; quarantine luôn giữ event.
Migration mới `20261115000000_comment_post_review`, không sửa migration cũ. Upgrade:
migrate → App/worker mới; binary mới/schema cũ fail closed. Backout bằng
[`down.sql`](../../services/vink-app/migrations-rollback/20261115000000_comment_post_review.down.sql)
chỉ khi chưa từng có ticket/quarantine/delivery mới (khóa bảng trước khi kiểm). Trước down:
tắt creation, dừng App/operator, drain rồi dừng worker. Khi đã có audit,
**roll forward**, kể cả đã drain: không xóa lịch sử kiểm duyệt để chạy binary cũ.

## Bằng chứng và giới hạn

Model bổ sung `post-review.qnt` cùng owner/lane `discussion`: một ticket, hai generation,
deadline 2 tick, clock ≤3; simulation seed 41 ×1000 ×40 và scenario 32 bước. Replay đọc command
đã export rồi gọi `decide_review`, lifecycle và `check_ownership` production; expected state chỉ là
oracle sau quyết định. Authority/ticket producer, serial commit, SHA-256 và locks là giả định shell.
Model lifecycle/admission hiện hữu đổi oracle theo accepted human intent; không sửa oracle để
bỏ qua lỗi. Kani thêm guard binding/deadline và human reaffirmation, compile cùng source production.

PostgreSQL tests: [`post_review.rs`](../../services/vink-app/tests/postgres/post_review.rs).
Fixture lỗi DB/time travel/handler panic được ghi nhãn. Process test chạy **CLI producer/result
thật → worker thật**; chặn receipt INSERT sau comment UPDATE, SIGKILL trong cửa sổ đó, xác nhận
rollback và restart → một receipt/effect → public read. Không dùng kill lúc idle làm bằng chứng.
Kết quả lane cuối và negative controls ghi trong PR, không kế thừa PASS từ PR cũ.

Lớp đã triển khai: domain, application, persistence, operator adapter và process. Nguồn đánh giá
là operator có authority, **không có AI evaluator**. Admission post-review producer chưa active;
evaluator/MOD-06 và rollout D-08 vẫn thiếu. Hướng đích #149 không còn đòi trust cohort;
trust/rules/classifier trong shadow cũ chưa có nguồn và không làm evaluator. Việc human queue có case chưa chứng minh
có người trực hoặc khung xử lý production; production activation/acceptance không thuộc PR này.
