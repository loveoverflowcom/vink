# Vận hành xác thực (D-05) — runbook

**Trạng thái:** D-05 **Provisional** ([ADR](../architecture/auth-authority.md)). Đường gửi production
là Resend HTTPS API. [Bằng chứng SMTP cũ](auth-production-evidence.md) không chứng minh
API mới. Tài liệu mô tả cấu hình và rollback; không phải bằng chứng đã bật production.

## 1. Luồng và owner

| Bước | Owner | Bảo đảm |
| --- | --- | --- |
| `POST /api/v1/auth/sign-in/challenges` | App | Kiểm `Origin`; đếm theo mạng nguồn (10/15 phút) trong DB App **trước** khi gọi Identity; lỗi đếm = `503` |
| Phát challenge | Identity | Trần theo địa chỉ (3/15 phút), theo tài khoản (3 lần đổi email/giờ), toàn cục (200/10 phút); challenge + ý định gửi commit **cùng** transaction |
| Gửi mã | Worker trong `vink-identity serve` | Claim `SKIP LOCKED` + lease 45 s; gửi ngoài transaction; tối đa 5 lần, backoff 5→60 s; HTTP `5xx` thử lại, tôn trọng `Retry-After` nếu còn hạn; mã hết hạn/đã dùng bị hủy; trạng thái cuối xóa mã |
| `POST …/sign-in/complete` | App → Identity | Đếm theo mạng (30/15 phút); 5 lần sai đóng challenge; phiên mới thay phiên cũ của trình duyệt (thu hồi phiên cũ) |
| Mỗi request riêng tư | App → Identity | `verify-credential` + binding + đình chỉ; Identity/DB không trả lời = `503`, không bao giờ ẩn danh |
| Mỗi lệnh ghi được bảo vệ | App (PostgreSQL) | Fence đình chỉ tại commit: nháp, tag, bình luận (gửi/xóa), chế độ bình luận, kiểm duyệt, ghi nhận |

Worker xử lý **at-least-once**; lần thử lại giữ payload và `Idempotency-Key` theo delivery ID.
Timeout không chứng minh provider chưa nhận. Resend giữ khóa [24 giờ](https://resend.com/docs/dashboard/emails/idempotency-keys),
không bảo đảm exactly-once. `202` từ Vink nghĩa là đã ghi ý định gửi; `sent` nghĩa là provider
chấp nhận, không phải thư đã tới hộp thư.

## 2. Cấu hình production

### Identity (Resend HTTPS API)

```dotenv
VINK_IDENTITY_AUTHORITY=native
VINK_IDENTITY_MAIL_TRANSPORT=resend-api
VINK_IDENTITY_RESEND_API_KEY_FILE=/run/secrets/vink-resend/api-key
VINK_IDENTITY_RESEND_FROM=no-reply@your-verified-domain
# Tùy chọn: VINK_IDENTITY_RESEND_TIMEOUT_SECONDS=10 (1–30)
#           VINK_IDENTITY_RESEND_PROXY=http://vink-resend-egress:3128
```

Cấp [key chỉ quyền gửi](https://resend.com/docs/dashboard/api-keys/introduction), giới hạn cho
[domain đã xác minh](https://resend.com/docs/dashboard/domains/introduction) của từng môi trường.
File nằm ngoài repo (`0600`), mount chỉ đọc, chỉ tiến trình Identity đọc được. Client gửi
`POST https://api.resend.com/emails`, xác minh TLS; không fallback SMTP hay `local-outbox`.
Khởi động kiểm cấu hình cục bộ, readiness không chứng minh DNS/egress, credential hoặc email đến.

Xem hướng dẫn [test](../../infra/test/README.md) và [production](../../infra/production/README.md)
để tách môi trường, mạng egress và secret.
Giữ `From` và mẫu thư ổn định khi còn delivery chờ; worker không gửi lại payload đã đổi.

### App

```dotenv
VINK_PROFILE=production
VINK_IDENTITY_SOCKET=/run/vink/identity.sock
VINK_IDENTITY_SERVER_UID=<uid của vink-identity>
VINK_TRUSTED_PROXIES=<IP/CIDR của reverse proxy>   # để trống nếu App nhận kết nối trực tiếp
VINK_AUTH_SIGN_IN=disabled                          # mặc định ở production; xem §3
```

`VINK_TRUSTED_PROXIES` chỉ được chứa proxy **của mình**; proxy phải **nối thêm** địa chỉ client vào
`X-Forwarded-For` (không chuyển nguyên header client gửi). App lấy hop không tin cậy ngoài cùng bên
phải; header `Forwarded` (RFC 7239) không được đọc. Đặt sai (tin cả Internet, ví dụ `0.0.0.0/0`)
cho phép client tự chọn bucket — khi đó giới hạn theo mạng mất tác dụng, chỉ còn trần của Identity.

## 3. Khóa kích hoạt

Production mặc định `disabled`. App trả `501 authority_not_configured` cho route tạo phiên;
phiên hiện hữu vẫn xác minh và đăng xuất được. Khi operator đặt `enabled` theo từng môi trường,
đăng nhập/đăng ký mở cho mọi người, không có invite hay allowlist. Email xác minh không là
điều kiện mặc định của đăng ký bằng mật khẩu; mã email bổ sung. Không tự đổi điều kiện O-11.

Thử trước trên một origin HTTPS cô lập với cơ sở dữ liệu, key và domain gửi riêng.
Vòng qualification này vẫn chạy được sau khi ngừng host test công khai theo
[quy trình Cloudflare/VPS](../../infra/production/cloudflare-edge.md). Đối chiếu
mật khẩu, gắn email và mã email, bút danh, Bàn viết và đăng xuất; kiểm SSR/hydration, cookie
`Secure`/`HttpOnly`/`SameSite`, CSRF, refresh và hai tab trên đúng origin HTTPS. Xem
[ADR §9](../architecture/auth-authority.md#9-blocker-trước-khi-accepted) cho các gate ngoài code.

### 3.1 Production username, mật khẩu và mã email

Ngày 22/11/2026, production ở image `a9d6a4f9` mở đăng nhập và đăng ký bằng username/mật khẩu,
sau đó bật mã email từ `no-reply@mail.vink.forum`. App dùng `VINK_PROD_AUTH_SIGN_IN=enabled`.
Identity dùng `native` qua overlay Resend. Worker dùng `disabled`. Đăng xuất thu hồi session ở
Identity. Trạng thái quyết định D-05 vẫn là `Provisional`.

Diễn tập cùng digest trên PostgreSQL scratch qua đăng ký mật khẩu, đăng nhập, đăng xuất, thu hồi
cookie, CSRF và sai mật khẩu. Trên HTTPS production, đăng nhập mật khẩu sai trả `401`, sai Origin
trả `403`, đăng xuất khách trả `204` với cookie hết hạn. Thử mã email thật tại `/toi`: thư vào
Inbox, mã tạo phiên đến bước đặt bút danh, đăng xuất và reload trả về màn hình đăng nhập.
Identity báo `resend_api` với hàng đợi rỗng sau phép thử.

Xem [bàn giao cấu hình production](../../infra/production/production-handoff.md) để lấy lại
cấu hình VPS, khôi phục khóa từ Google Secret Manager và giữ overlay khi đổi image.
Để dừng cấp phiên mới, đặt `VINK_PROD_AUTH_SIGN_IN=disabled` rồi recreate riêng App bằng
cùng tập manifest và release env. Giữ Identity để phiên hiện hữu xác minh và đăng xuất.

## 4. Kiểm chứng gửi thật (trước khi bật)

1. Chủ dự án tự chạy `vink-identity mail test <hộp-thư-thử>` đến hộp thư do mình kiểm soát.
   Dùng cùng transport với worker, không có mã và không cần DB. Ghi riêng provider accepted
   và mailbox delivered (xác nhận trong hộp thư).
2. Sau khi mở trên test, xin mã thật và xác minh trên trình duyệt ban đầu. Kiểm `From`,
   tiêu đề tiếng Việt, mã, session/bút danh và thư có vào spam không.
3. `vink-identity mail status` phải về `pending 0`; log chỉ có delivery ID, loại và mã số lỗi.
   Không đưa key, mã đăng nhập, địa chỉ hay nội dung provider vào bằng chứng. Mock HTTP không
   thay thế phép kiểm thật; phép gửi thật do chủ dự án thực hiện.

## 5. Giám sát

| Tín hiệu | Nguồn | Ngưỡng gợi ý |
| --- | --- | --- |
| `mail_queue.pending`, `oldest_pending_seconds` | `/health/ready` của Identity, `vink-identity mail status` | Thư chờ > 60 s: relay chậm/sập |
| `mail_queue.failed_last_hour` | như trên | > 0 liên tục: domain/credential/relay |
| `code delivery failed; retrying` (`failure`, `status`) | log Identity | Nhiều `connect`/`tls`: egress/DNS; `429`/HTTP `5xx`: provider giới hạn/lỗi |
| `sign-in code refused: throttle` (`throttle=global`) | log Identity | Có thể bị flood: xem §6.2 |
| `sign-in request refused: network ceiling` | log App | Một mạng thử liên tục |
| `write refused at commit: principal suspended` | log App | Đình chỉ đang chặn ghi (mong đợi) |

Readiness Identity báo `mail_transport` và `production_authentication` (`not_accepted` với Resend API,
`not_implemented` với outbox cục bộ) cho tới khi D-05 Accepted. Backlog thư **không** làm readiness
`503`: xác minh phiên không phụ thuộc mail.

## 6. Sự cố

### 6.1 Resend không nhận thư
Không cần can thiệp ngay: API vẫn `202`, thư được giữ và thử lại tối đa 5 lần trong hạn mã;
mã sống 10 phút. Sau đó thư `failed`, người dùng xin mã mới (tối đa 3 mã/15 phút/địa chỉ). Nếu sự
cố kéo dài, cân nhắc tạm `VINK_AUTH_SIGN_IN=disabled` để người dùng thấy thông báo rõ thay vì chờ thư.

### 6.2 Flood xin mã
Trần toàn cục chặn mọi mã mới khi vượt (suy giảm có chủ ý, bảo vệ chi phí/uy tín domain). Hạ
`VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT` nếu cần, chặn ở edge/WAF (giới hạn băng thông/kết nối là
việc của edge; App không chống DDoS thể tích), kiểm `VINK_TRUSTED_PROXIES` đúng.

### 6.3 Tài khoản bị lạm dụng hoặc chiếm
1. `vink-app principal suspend <principal> <lý do>` — có hiệu lực **ngay tại commit** cho mọi lệnh
   ghi được bảo vệ và ở request kế tiếp cho đọc riêng tư.
2. `vink-identity account disable <subject>` — thu hồi mọi phiên trong cùng transaction.
Thứ tự này quan trọng: disable ở Identity không fence được request App **đang chạy** (hai database,
không có giao dịch xuyên owner); đình chỉ ở App thì có.

### 6.4 Lộ API key mail
Cấp key chỉ quyền gửi cho cùng domain/môi trường. Ghi file riêng `0600` đúng owner rồi
thay atomically trong thư mục secret mount; worker đọc lại ở lần gửi kế tiếp. Nếu mount một
file thay vì thư mục, recreate container để thấy inode mới. Smoke bằng hộp thư được phép, rồi
thu hồi key cũ (nếu đang bị lạm dụng, thu hồi trước). Không đưa key vào issue, log, ảnh, Git
hay shell history.

### 6.5 Lộ DB Identity
DB chỉ có hash phiên/mã; mã đang chờ gửi lưu rõ trong `mail_deliveries.code` tới khi gửi xong
(không đủ để trả lời challenge vì thiếu bí mật client). Thu hồi toàn bộ phiên bằng
`UPDATE sessions SET revoked_at = now(), revoked_reason = 'logout' WHERE revoked_at IS NULL` (role
migrator/DBA) nếu nghi ngờ hash bị dùng sai.

## 7. Xoay secret

| Secret | Cách xoay | Ảnh hưởng |
| --- | --- | --- |
| API key Resend | §6.4 | Không |
| Mật khẩu role PostgreSQL | Đổi ở PostgreSQL → cập nhật URL → restart tiến trình | Gián đoạn ngắn; phiên giữ nguyên |
| Phiên | Không có khóa ký (phiên opaque, chỉ lưu hash): không cần xoay; thu hồi theo §6.3/§6.5 | — |

## 8. Retention

| Dữ liệu | Giữ | Xóa bởi |
| --- | --- | --- |
| Challenge + thư | 7 ngày sau hết hạn; mã xóa ngay khi thư tới trạng thái cuối | Identity `serve` (mỗi giờ), `vink-identity maintenance prune` |
| Phiên | 30 ngày sau khi đóng (hết hạn/thu hồi) | như trên |
| Bộ đếm rate limit (khóa giả danh hóa) | 1 ngày sau cửa sổ | `vink-worker` (mỗi giờ), `vink-app maintenance prune` |
| Tài khoản, địa chỉ đã xác minh, principal, binding | Không xóa tự động | — |

## 9. Rollback

Tắt gate riêng của môi trường và recreate App, giữ Identity phục vụ phiên cũ. Kiểm `/toi`
đóng, route tạo phiên trả `501`, phiên hợp lệ vẫn xác minh/đăng xuất và dữ liệu không đổi.
Gate chỉ chặn tạo phiên mới; thư đã xếp hàng hoặc hành động từ phiên đang mở không tự dừng.
Nếu cần dừng mọi lần gửi, ngắt egress của proxy Resend riêng theo
[hướng dẫn production](../../infra/production/README.md), không tắt Identity hay PostgreSQL.

Không downgrade schema để rollback cấu hình. Khi rollback binary sau migration, sao lưu theo
[owner](../../services/postgres/README.md) và chỉ dùng down migration tương ứng đúng version
sau khi đối chiếu guard lịch sử. Với migration Resend, đóng cổng tạo phiên và ngừng nhận intent
mail mới; đợi delivery hiện có xử lý xong hoặc hết hạn, dừng các worker Identity và dùng
[down migration Resend](../../services/vink-identity/migrations-rollback/20261120000000_resend_payload_fence.down.sql).
Không chạy lẫn worker SMTP cũ và worker Resend mới: worker cũ không hiểu ràng buộc payload,
có thể gửi trùng; không rollback cả outbox hay chỉnh tài khoản chỉ để tắt auth.

## Recovery #181/#329: gắn email đã có, reset chưa có

Gắn email lần đầu, đăng nhập mã email và thêm mật khẩu có proof riêng; không phải reset.
[Đặc tả recovery](../architecture/password-recovery.md) đề xuất purpose/receipt/retention riêng,
chống dò account, xử lý thư gửi lại và revoke phiên. Chưa có route, migration hoặc quyền vận hành
reset; không dùng lệnh disable/enable hay sửa trực tiếp password hash để giả lập recovery đã nghiệm
thu. Các mục vận hành và retention ở trên là hiện trạng, không tự bao phủ bảng/route đề xuất.
