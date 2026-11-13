# Khôi phục mật khẩu qua email — đặc tả #181

## 1. Phạm vi, trạng thái và nguồn

Lát `AUTH-RECOVERY-SPEC-01` chỉ đặc tả Web → App → Identity. Không thêm route, schema, UI hay
quyền khôi phục trong runtime; không mở production. Chủ sản phẩm xác nhận ngày 13/11/2026:
“Có thể lấy lại mật khẩu qua email, cần luồng riêng” ([issue #181](https://github.com/loveoverflowcom/vink/issues/181)).

| Chiều | Kết luận của lượt này |
| --- | --- |
| Sản phẩm | Kênh email và luồng riêng đã được xác nhận ở issue; chi tiết dưới đây là proposal chưa duyệt |
| Quyết định | `O-11@r1` vẫn `OPEN`; [O-11@r2](../../decisions/proposals/O-11.r2.toml) `PENDING`, không có signoff; `D-05@r1` vẫn `PROVISIONAL` |
| Triển khai | Recovery và gắn email cho tài khoản mật khẩu: `NOT_IMPLEMENTED`; password login/enrollment đã có |
| Bằng chứng | Trace source tại `4c78601b9f4d1361d9faf9dc0d2be290978e54d4`; kiểm tài liệu/registry, không tuyên bố runtime PASS |

<!-- vdr: active O-11@r1 -->
<!-- vdr: active O-12@r2 -->
<!-- vdr: active D-05@r1 -->

Registry trước thay đổi: snapshot SHA-256
`70a6a63ebbf19622599853050ca0910a3dbd99fdca9b4ceab4f87bfede908107`.
`O-11@r1` SHA `e8e933cc7781916464b1c069efa77c3b75fff266ed9b17cc6f3bdf1e4daee258`,
nguồn `af3c34b:docs/architecture/product-decisions-20261110.md:87`;
`O-12@r2` SHA `8cbd2660d039ddaf05fdabe29c99489aec9cdfa5ea4581a8075bcdcd3f5fa84f`;
`D-05@r1` SHA `17ea6afdc3bbbef29b47b17180451a9ad151eca660483d9b3cd0f0cfe057530a`.
Proposal r2 SHA `af0838ef8909a38ebebfd2c459c693ae9b08dabd965e27b790e442c1d1515500`;
snapshot sau proposal `2027159b784ae787aac547c71a70bc6190a541411f9dda2ce9958747422dc10a`.
Chỉ `decision signoff` với lời chủ sản phẩm cho đúng revision/hash mới đổi hiệu lực.

## 2. HOW/WHY: đường có thật và khoảng trống

- [`AccountStore::register_password`](../../services/vink-identity/src/accounts.rs) tạo `accounts`
  + `account_passwords` + session cùng transaction, **không tạo `account_emails`**. Đăng ký không
  hỏi email. Đăng nhập email có thể tìm/tạo một tài khoản khác; nhập cùng bút danh không gộp account.
- Đăng nhập email: `start_sign_in` → challenge/outbox → `complete_sign_in` tiêu thụ code, tìm/tạo
  account theo địa chỉ và phát session. Nó không được dùng làm reset hay chứng minh email thuộc
  tài khoản username đang muốn lấy lại.
- Enrollment: phiên hiện tại → `start_password_enrollment` chọn email đã xác minh của account →
  `enroll_password` khóa account/session/challenge và chèn credential. Purpose `enroll_password`
  và issuing session tách khỏi `sign_in`; trùng username không tiêu thụ proof. Đây không phải recovery.
- Identity có `email-change/*` riêng tư trong [contract RPC](../../services/vink-identity/contracts/rpc/v1/README.md).
  `complete_email_change` chỉ `UPDATE account_emails`; không chèn hàng đầu tiên và App/Web chưa có
  đường gắn email. Không tái dùng nguyên thao tác này cho tài khoản password-only rồi báo đã gắn.
- [App Auth](../../services/vink-app/src/http/auth.rs) không có route recovery; cookie
  `__Host-vink_sign_in`/`__Host-vink_enroll` phục vụ hai purpose khác. [Mail purpose](../../services/vink-identity/src/mail.rs)
  có `SignIn`, `ChangeEmail`, `EnrollPassword`, không có recovery. Không chỉ thay nhãn mail.
- [SessionOwner](../../apps/vink-web/src/session.rs) và [AuthGate](../../apps/vink-web/src/account/flow.rs)
  giữ generation và lượt ghi; giữ chữ cho cùng principal, không chuyển draft/receipt giữa người.
  Account mapping của App vẫn là `(authority, subject)` → principal; email/username không là ID.

Lý do ghi nhận: USER-08 chọn username/password chính; D-05 giữ native authority, opaque session,
binding/standing riêng của App. O-12 chốt NFKC và độ dài. Các PR [#168](https://github.com/loveoverflowcom/vink/pull/168)
và [#178](https://github.com/loveoverflowcom/vink/pull/178) bổ sung enrollment/Web, không cung cấp reset.
Chọn tái dùng hạ tầng thay vì provider mới là suy luận kỹ thuật từ các seam này, không phải signoff.

## 3. Quyết định đề xuất và phần cần chủ sản phẩm duyệt

| Điểm | Phương án và lựa chọn đề xuất | Trạng thái |
| --- | --- | --- |
| Chứng minh hộp thư | **A: code một lần + secret handle trong cookie**. Tái dùng outbox/throttles nhưng purpose riêng, không secret trong URL. B: link một lần dễ mở từ thư nhưng cần chống link scanner, referrer/history và truy cập chéo trình duyệt; không chọn cho lát đầu | Kỹ thuật `DESIGN_PROPOSED` |
| Có email trước khi quên | **A: email khôi phục tùy chọn**, thêm sau đăng nhập và nhắc rõ khi chưa gắn. B: bắt buộc xác minh lúc đăng ký, cải thiện coverage nhưng đổi điều kiện gia nhập. Đề xuất A, không tự sửa đăng ký | `open_product`, chờ duyệt O-11@r2 |
| Phiên sau reset | **A: thu hồi tất cả phiên và đăng nhập lại**. B: giữ phiên hiện tại hoặc tự đăng nhập mới, ít gián đoạn nhưng khó phân biệt phiên bị chiếm. Đề xuất A; không gỡ đình chỉ/disable | `open_product`, chờ duyệt O-11@r2 |

Hai câu cần chốt trước runtime: có giữ email **tùy chọn** và cho gắn sau đăng ký không? Sau reset
có **đăng xuất mọi thiết bị, đăng nhập lại** không? Các mục này không ngăn việc giao đặc tả/proposal.
Không suy quyền thực hiện từ việc merge PR tài liệu. Không hứa admin giúp lấy lại account khi mất
hộp thư; quy trình phục hồi thủ công nếu cần phải được thiết kế riêng.

## 4. Thuật ngữ và quyền

| Tên sản phẩm | Owner/khái niệm đề xuất | Ranh giới |
| --- | --- | --- |
| Email khôi phục đã xác minh | `account_emails` + email generation | Không phải username hay bút danh; chỉ account đúng proof có quyền gắn |
| Mã đặt lại | purpose đề xuất `reset_password` | Không đổi thành `sign_in`, `enroll_password`, `change_email` |
| Yêu cầu đang chờ | challenge + secret handle + credential/email version tại issuance | ID không đủ quyền; code và handle phải cùng request |
| Đặt lại đã hoàn tất | receipt hẹp gắn challenge/operation ID | Không là session, không chứng minh mật khẩu vẫn hiện hành sau lần thay đổi khác |

| Actor × hành động | Đối tượng/điều kiện | Fence bền vững đề xuất |
| --- | --- | --- |
| Người đã đăng nhập gắn email | Đúng account theo phiên, fresh password reauthentication; email nhận code chưa thuộc account khác | Khóa account/session, recheck expiry/disable và credential version; code purpose attach riêng; unique address; insert đúng account |
| Người đã có verified email | Không đi qua attach lần đầu; đổi email là use case riêng | Không tự gộp hai account, không đổi chủ email bằng last-write-wins |
| Khách xin reset | Địa chỉ hợp lệ cú pháp; không cần biết username | Uniform public response; chỉ enqueue mail thật nếu address hiện gắn account có password, không tạo account |
| Người có code + handle đặt lại | Đúng purpose, account, email generation, credential version, chưa hết hạn/tiêu thụ/đủ lượt thử | Khóa account → credential/email → challenge theo một thứ tự, recheck DB clock sau chờ lock; thay hash + tăng version + tiêu thụ + revoke cùng transaction |
| Admin/AI/bút danh giống nhau | Không có quyền reset qua endpoint này | Không bypass proof; không suy quyền từ display name, App principal hoặc response UI |

Fresh password reauthentication là proof ngắn hạn ràng buộc phiên/credential version, không chỉ cờ
ở Web. Luồng attach cũng cần chống account khác đang dùng email: trả cùng câu không tiết lộ
chủ sở hữu, unique constraint quyết định; không tạo account mới nếu address xung đột. Tài khoản
mã email hiện có đã có verified email; không tự thêm credential nếu chưa có password (dùng enrollment).
Email attach/change và policy revoke là dependency **chưa triển khai**, không được trình bày như sẵn có.

## 5. State, transaction và hai request đồng thời

Ba trục độc lập: tài khoản (`enabled/disabled`), email (`missing/verified/generation`), request
(`issued/closed/completed/unknown`). `mail_queued/delivered/failed` chỉ ở vận hành, không chứng minh
người dùng nhận thư. Tham số kỹ thuật đề xuất kế thừa policy hiện tại: code 6 số, 10 phút,
5 lần sai/challenge; 3 yêu cầu/15 phút theo địa chỉ, thêm trần mạng và global trước account lookup.
Các trần cụ thể phải cấu hình và đo ở implementation, không được bỏ throttling vì endpoint ẩn danh.

| Sự kiện | Kết quả đích và dữ liệu giữ lại |
| --- | --- |
| Xin mã địa chỉ hợp lệ (có/không account; password-only/email-only/disabled) | Cùng `202`, shape và TTL; handle ngẫu nhiên cùng độ dài. Chỉ account eligible được mail reset; không có tài khoản mới hay session |
| Code sai | Tăng attempts trong transaction, báo “Mã chưa đúng”; chưa đổi credential; lần sai thứ 5 đóng request |
| Mã quá hạn, dùng lại, wrong purpose/handle hoặc generation cũ | Chung “Mã không còn dùng được”; không cho biết nguyên nhân nhạy cảm, không đổi password |
| Password không đạt O-12 | Lỗi tại trường; không tiêu thụ code; không log giá trị. Server là authority, UI chỉ hướng dẫn |
| Code đúng + password hợp lệ | Hash Argon2id ngoài lock có concurrency bound; dưới lock kiểm lại mọi predicate và clock, thay hash/version và consume atomically. Username, account, subject, principal và dữ liệu không đổi |
| Gửi lại | Với handle cũ hợp lệ của chính client: request mới dùng secret/code mới, transaction đóng request cũ đó và hủy mail chưa gửi. Yêu cầu ẩn danh không có handle không được đóng request của client khác. Email generation ngăn code cũ sống lại khi đổi đi rồi quay về |
| Hai lần gửi mã đồng thời | Serialize throttles và resend của cùng handle; các request độc lập có thể cùng hiệu lực trên cùng credential generation. UI gắn code với đúng handle, không khẳng định thư đến cuối là request hiện tại. Không có global latest-request gate cho phép người lạ vô hiệu mã đang dùng |
| Hai lần reset cùng request | Một transaction thắng; request còn lại đóng, không thay hash lần hai |
| Hai request reset khác nhau | Cùng credential generation: completion đầu tiên thắng và tăng version, completion còn lại bị từ chối dưới lock. Không để hai reset nối tiếp ghi đè bằng proof cùng generation |
| Reset đua với login/enrollment/email change/disable | Dùng cùng lock order/account fence với owner hiện có. Login phải recheck credential version sau verify để hash cũ không phát session sau reset. Disable thắng thì reset không cấp quyền; email change invalidates proof dù đổi lại cùng address |
| Commit/rollback/crash | Consume, hash/version, revoke và receipt cùng Identity DB transaction; lỗi rollback giữ credential cũ. Không báo hoàn tất nếu transaction chưa xác nhận |

Theo phương án phiên A: revoke mọi session Identity cùng transaction, không phát cookie phiên mới;
đóng reset/enrollment proof liên quan và ngăn **mọi** sign-in code được phát trước reset tái tạo
phiên sau reset bằng invalidation/generation fence. Không xóa principal hoặc quyền nội dung.
App không có transaction xuyên hai DB: yêu cầu riêng tư tiếp theo xác minh Identity và server
standing/commit fence hiện hữu; mô tả rõ write đã được chấp nhận trước reset có thể hoàn tất,
không hứa reset rollback những write đang chạy. **Delta bắt buộc của sign-in hiện hữu:**
`complete_sign_in` hiện khóa challenge trước rồi tìm/tạo account và phát phiên, chưa lấy account
fence theo thứ tự reset ở trên. Implementation phải thiết kế lại lookup → account lock → challenge
recheck cho account đã tồn tại (giữ đường provisioning mới đúng đắn), không chỉ thêm reset route.
Kiểm transaction sign-in/disable/email-change/enrollment không đảo lock gây deadlock; sau chờ phải
kiểm generation/expiry lại trước issue session. Consumer Web dùng SessionOwner để refresh và
loại private projection; không tự reset editor hoặc copy draft sang người khác.

## 6. Hợp đồng transport và mất phản hồi (đề xuất, chưa phải API v1)

Chỉ thêm vào owner contract khi triển khai; không sửa fixture hiện hành thành bằng chứng của API mới.
App dự kiến `POST /api/v1/auth/password/reset/challenges`, `POST …/reset/complete`,
`GET …/reset/status`; Identity có RPC tương ứng trên Unix socket với caller UID hiện hữu.
Cookie mới `__Host-vink_reset` là `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, không `Domain`;
handle không vào URL, JSON trả browser, SSR, storage hoặc BroadcastChannel. Payload completion
chỉ code + password mới + operation ID + request ID công khai, không account/principal/username do browser tự chọn.
Request ID được trả cùng `202` cho cả dummy và thật; ID không chứa secret và không đủ quyền.
Server so ID trong form/header với challenge trong cookie **trước** attempt hoặc đọc receipt; không
được thử code vào request khác do cookie vừa bị tab khác ghi đè.
App giữ Origin check, body limit và `Cache-Control: no-store` cho mọi kết quả/lỗi.

- Xin mã: cú pháp sai `422` tại email; trần địa chỉ/mạng/global `429` không phụ thuộc account tồn tại.
  `202` chỉ ghi nhận, không nói “đã gửi thành công”. DB/provider chưa commit → `503`; lỗi SMTP sau
  commit vẫn cùng `202`, retry bounded tới TTL, không public delivery status dùng để dò account.
  Nhánh unknown/disabled/email-only phải có **dummy challenge có state**: cùng handle/TTL, attempts,
  cửa sổ throttle và lifecycle lỗi như challenge thật, nhưng không thể thay credential/phát phiên.
  Sai code ở cả hai nhánh trả cùng `422` tới giới hạn rồi cùng `409`; không trả `409` ngay chỉ vì
  account không eligible. Complete/status không được trở thành oracle thay cho issuance. Xử lý
  invalid password/shape theo cùng thứ tự ở cả hai nhánh. Có lịch thực thi đủ tương đương, tránh
  early-return/timing rõ; kiểm thực tế bằng cả test instrumented lẫn đo response, không hứa constant time.
- Xác minh + đặt lại: UI có thể đi qua bước code nhưng chưa nói code đúng trước completion.
  Completion atomically quyết định proof và password, không cấp một reset token thứ hai.
  `204` xác nhận commit; code sai `422`, request đóng `409`; `429` kèm `Retry-After`;
  `501` không cấu hình, `503` hoặc timeout là chưa rõ kết quả, không coi là thành công/thất bại chắc chắn.
- Receipt bounded: status yêu cầu chính secret handle, operation ID ngẫu nhiên đã gắn với request,
  không account lookup. Chỉ trả `completed` cho operation đã commit tương ứng, `not_completed`
  khi DB xác nhận chưa có receipt, hoặc `unknown/unavailable`. Giữ cookie receipt trong TTL đối soát
  thay vì clear ngay lúc completion. Không trả email, username, hash, phiên mới hay account existence.
  `completed` không xác nhận password còn hiện hành nếu sau đó có reset khác.
- Mất ACK: không tự replay. Đọc status đúng operation; completed → quay về login; unavailable →
  thử status lại; not_completed → người dùng chủ động thử completion khi request còn hợp lệ hoặc
  xin mã mới. Receipt hết hạn/mất handle → thử login bằng password mới hoặc bắt đầu recovery mới,
  không đoán từ nội dung thông báo. Lặp POST với proof đã consume không chạy lại hash update.
- Nhiều tab/cookie: chỉ một cookie reset hiện hành trong browser; AuthGate không ngăn được
  Set-Cookie muộn. Mỗi form giữ request ID và operation ID của mình; status cũng gửi hai ID công
  khai trong header (không secret). Nếu cookie bị tab khác hoặc response issuance cũ ghi đè, server
  trả lỗi chung `recovery_context_changed`, không tăng attempts của request khác, không trả receipt
  khác và không ghi credential. UI dừng form này, nêu “Yêu cầu đã thay đổi trên trình duyệt này”,
  cho kiểm login hoặc chủ động bắt đầu lại; không tự replay hay khôi phục handle bằng storage.
  Nếu mất handle của một reset đã commit, dùng đường fallback login/new request ở trên. Giới hạn
  nhiều tab này là UX đích có chủ ý, không hứa chỉ generation local là đủ hoặc mọi receipt luôn đọc được.
- Đang đăng nhập A mà mở reset: không tự thay phiên hoặc đăng nhập B; copy nêu reset độc lập và
  yêu cầu quay về login có chủ ý. Response cũ bị loại khỏi form mới bằng AuthGate generation;
  server effect đã commit vẫn phải đối soát, abort browser không là rollback.

## 7. Dữ liệu và vận hành

Identity sở hữu password hash/version, challenge, email binding/generation và receipt. App chỉ có
adapter, public rate limiter và cookie; không sao chép credential store. Migration mới additive,
không sửa lịch sử. Receipt chỉ có ID ngẫu nhiên, outcome/timestamp, version và binding cần thiết;
không lưu password hoặc digest có thể dò password. Thông số retention đề xuất kế thừa
[runbook](../development/auth-operations.md#8-retention): challenge/receipt 7 ngày sau expiry,
code trong outbox xóa ngay trạng thái cuối; hết TTL browser chỉ còn đường bắt đầu lại. Việc xử lý
thư reset đã xếp hàng sau revoke/gửi lại phải được worker kiểm trước send; thư đã ra SMTP không
thể thu hồi, nhưng code không còn quyền. Không dùng email gửi thành công làm authority.

Không log password, OTP, handle, recipient, body, request JSON hoặc raw provider error. Tracing
chỉ operation class, outcome an toàn và correlation ID đã đánh giá không tiết lộ dữ liệu.
Header/query/body ở proxy, panic Debug và telemetry phải được kiểm, không chỉ log trong Rust.
Không AI/provider mới; không gửi draft hay thông tin tài khoản cho AI. Production gate giữ tắt.

## 8. Bàn giao implementation và kịch bản kiểm

| Owner | Delta đích | Bằng chứng bắt buộc khi triển khai |
| --- | --- | --- |
| Identity `accounts.rs`, `password.rs`, `mail.rs`, contract/migrations mới | Attach verified email; purpose reset, generation/lock, atomic consume/hash/revoke/receipt, outbox | PostgreSQL thật: rollback, concurrency, resend cùng handle, old login/sign-in proof, disable/email change, receipt TTL và mail failure |
| App `http/auth.rs`, `identity/client.rs`, abuse owner + contract v1 | Adapter cookie/Origin/no-store/throttle; không tạo chủ thể | Contract tests + App/Identity thật, no enumeration, wrong purpose, missing/expired cookie, request size |
| Web `account/{flow,forms,browser}.rs`, `api.rs` | [LayoutSpec](../design/password-recovery.md), AuthGate/SessionOwner, error/retry | Browser thật App + Identity + scratch DB/outbox, không dùng mock success cho proof |
| Docs/registry | Signoff đúng revision, PRD/ADR và hiện trạng | `decision validate`, `decision refs`, review semantic diff và links |

Ma trận implementation phải có: account password không email, email đã gắn, address account khác,
email-only chưa có credential, disabled và suspended; sai/hết hạn/reuse/cross-purpose; gửi lại đồng
thời, hai completion đồng thời; đổi email đi/quay về; login kiểm hash cũ bị giữ tới sau reset;
SMTP retry/expiry; dummy vs thật ở issuance/complete/status (code sai, expiry, 5 attempts, rate
limit, password invalid, malformed input); hai tab xin mã với ACK đảo thứ tự, Set-Cookie cũ ghi đè
cookie mới và tab khác overwrite cookie receipt; crash trước/sau commit và ACK bị mất. Đo account/subject/principal/pen name,
draft ID/body trước/sau; login mới đúng, session cũ bị từ chối; recovery không gỡ đình chỉ.
Web giữ response A rồi đổi mode/phiên sang B, không xóa chữ mới hoặc phục hồi dữ liệu A.

Implementation chạy `cargo xtask verify quick`, `cargo xtask verify postgres`, `cargo xtask verify web`;
chọn scoped lệnh phụ theo [workflow](../development/README.md), nhưng không thay full lane bằng tên
lệnh tưởng tượng. Ghi source SHA, DB/loopback/outbox thật vs interception, UI viewport/theme/font;
IME hệ điều hành, password manager, screen reader và production mail/HTTPS không chạy thì `NOT_RUN`.

Lượt spec chỉ kiểm tài liệu/decision; không dùng screenshot hay source trace thay transaction test.
Không tick/đóng epic, không coi #181 là runtime acceptance và không đổi D-05 thành Accepted.

### Đối chiếu acceptance của issue (tài liệu, không runtime)

| AC #181 | Đầu ra reviewable | Giới hạn |
| --- | --- | --- |
| AC-01 | §2–4 và LayoutSpec: entry riêng, đường attach trước recovery và người mất email | Điều kiện email tùy chọn đang chờ product signoff; chưa có attach/reset runtime |
| AC-02 | §4–6: actor/state, consume/hash/revoke/receipt, resend/concurrency và mất ACK | Tất cả transaction/fence là thiết kế cần PostgreSQL thật kiểm ở PR sau |
| AC-03 | §5–7: uniform response, throttles/O-12/mail failure và secret hygiene | Chưa đo timing, mailbox delivery, proxy logging hoặc browser |
| AC-04 | O-11@r2 pending + liên kết ACC-03, D-05, Auth Web/operations | Không signoff, không đổi D-05, không tick tracker thay chủ sản phẩm |
