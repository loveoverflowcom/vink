# Vận hành xác thực (D-05) — runbook

**Trạng thái:** D-05 **Provisional** ([ADR](../architecture/auth-authority.md)). Đường mail
production, giới hạn lạm dụng, fence đình chỉ tại commit và retention đã có code và bằng chứng
cục bộ ([bằng chứng](auth-production-evidence.md)); **chưa** bật đăng nhập production. Tài liệu này
mô tả cách cấu hình, giám sát, xoay secret, xử lý sự cố và rollback khi chủ sản phẩm quyết định bật.

## 1. Luồng và owner

| Bước | Owner | Bảo đảm |
| --- | --- | --- |
| `POST /api/v1/auth/sign-in/challenges` | App | Kiểm `Origin`; đếm theo mạng nguồn (10/15 phút) trong DB App **trước** khi gọi Identity; lỗi đếm = `503` |
| Phát challenge | Identity | Trần theo địa chỉ (3/15 phút), theo tài khoản (3 lần đổi email/giờ), toàn cục (200/10 phút); challenge + ý định gửi commit **cùng** transaction |
| Gửi mã | Worker trong `vink-identity serve` | Claim `SKIP LOCKED` + lease 45 s; gửi ngoài transaction; tối đa 5 lần, backoff 5→60 s; `5xx` dừng ngay; mã hết hạn/đã dùng bị hủy; trạng thái cuối xóa mã |
| `POST …/sign-in/complete` | App → Identity | Đếm theo mạng (30/15 phút); 5 lần sai đóng challenge; phiên mới thay phiên cũ của trình duyệt (thu hồi phiên cũ) |
| Mỗi request riêng tư | App → Identity | `verify-credential` + binding + đình chỉ; Identity/DB không trả lời = `503`, không bao giờ ẩn danh |
| Mỗi lệnh ghi được bảo vệ | App (PostgreSQL) | Fence đình chỉ tại commit: nháp, tag, bình luận (gửi/xóa), chế độ bình luận, kiểm duyệt, ghi nhận |

Gửi thư là **at-least-once**: crash sau khi relay nhận nhưng trước khi ghi kết quả có thể tạo một
bản trùng cùng mã, cùng `Message-ID` — vô hại vì mã vẫn cần bí mật trong cookie của trình duyệt đã
xin mã. `202` của API nghĩa là "đã ghi ý định gửi", không phải "đã vào hộp thư".

## 2. Cấu hình production

### Identity (Resend qua SMTP)

Resend nhận SMTP ở `smtp.resend.com`, cổng `465` (TLS ngay) hoặc `587` (STARTTLS), username
`resend`, password là API key. Dùng API key **chỉ có quyền gửi** (sending access), giới hạn theo
domain gửi, và domain đã xác minh SPF/DKIM ở Resend (DMARC khuyến nghị) trước khi bật.

```bash
install -m 600 -o vink-identity /dev/stdin /run/secrets/vink-identity-smtp   # dán API key, Ctrl-D
```

```dotenv
VINK_IDENTITY_AUTHORITY=email-code
VINK_IDENTITY_MAIL_TRANSPORT=smtp
VINK_IDENTITY_SMTP_PROVIDER=resend        # = host smtp.resend.com, tls/465, user resend
VINK_IDENTITY_SMTP_PASSWORD_FILE=/run/secrets/vink-identity-smtp
VINK_IDENTITY_MAIL_FROM=Vink <no-reply@<domain-đã-xác-minh>>
# Tùy chọn: VINK_IDENTITY_SMTP_SECURITY=starttls (587), VINK_IDENTITY_SMTP_TIMEOUT_SECONDS=10,
#           VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT=200
```

**Mạng có firewall chặn TLS** (đã gặp 10/11/2026 trên mạng văn phòng: Fortinet thay chứng chỉ của
`smtp.resend.com:465` bằng chứng chỉ do CA Fortinet ký): Identity từ chối — đúng thiết kế, không bao
giờ tin CA lạ — và báo `tls`. Dùng `VINK_IDENTITY_SMTP_SECURITY=starttls` (cổng 587 nhận chứng chỉ
thật) hoặc chạy từ mạng không bị chặn; **không** tắt kiểm chứng chỉ.

### Đổi nhà cung cấp mail

Outbox, worker, chính sách retry và nội dung thư không biết nhà cung cấp; worker chỉ giữ một
`MailSender` ([mail.rs](../../services/vink-identity/src/mail.rs)).
- Nhà cung cấp SMTP khác: đặt `VINK_IDENTITY_SMTP_HOST`/`_SECURITY`/`_USERNAME` tường minh (bỏ
  `VINK_IDENTITY_SMTP_PROVIDER`), hoặc thêm một dòng vào `SMTP_PRESETS` trong
  [config.rs](../../services/vink-identity/src/config.rs). Không sửa code khác.
- Nhà cung cấp chỉ có API HTTP: thêm một `impl MailSender` và một nhánh `MailChoice`; outbox, worker
  và contract không đổi.
- Sau khi đổi: `vink-identity mail test <hộp-thư-thử>` (§4).

Quy tắc kiểm khi khởi động (thoát `2`, không in secret): mật khẩu chỉ qua file `0600`
(`VINK_IDENTITY_SMTP_PASSWORD` dạng giá trị bị từ chối); `none` (không TLS) chỉ cho relay loopback;
username và file mật khẩu phải đi cùng nhau; `MAIL_FROM` phải là mailbox RFC 5322. Khởi động
**không** gọi mạng: readiness không chứng minh egress, DNS, TLS hay credential đúng — xác nhận bằng
một lần gửi thật (§4).

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

Ở profile production, `VINK_AUTH_SIGN_IN` mặc định `disabled`: hai route xin/trả mã trả
`501 authority_not_configured`; phiên đã có vẫn được xác minh và đăng xuất được. Chỉ đặt
`enabled` khi đủ các gate ở [ADR §9](../architecture/auth-authority.md#9-blocker-trước-khi-accepted),
tối thiểu: chủ sản phẩm chấp nhận UX mã email, gửi thật qua Resend thành công (§4), UI đăng nhập
([Auth Web](auth-web.md)) có qualification trình duyệt trên HTTPS/domain thật, review bảo mật độc lập và người chịu trách nhiệm vận hành auth.

## 4. Kiểm chứng gửi thật (trước khi bật)

1. Với cấu hình §2: `vink-identity mail test <hộp-thư-thử>` gửi một thư kiểm tra (không chứa mã)
   qua đúng sender của `serve`, không cần database. Thành công = relay đã nhận (`accepted by smtp`);
   lỗi in loại (`tls`, `connect`, `configuration`, `permanent (SMTP 5xx)`…). Hộp thư mô phỏng của
   Resend `delivered@resend.dev` cho phép thử không làm phiền ai.
2. Luồng đầy đủ: xin mã cho hộp thư thử do đội sở hữu; kiểm thư đến (không vào spam), `From`, tiêu
   đề tiếng Việt, mã đúng, header `Auto-Submitted`.
3. `vink-identity mail status` phải về `pending 0`; log có `code delivered` (không có địa chỉ/mã).
4. Ghi kết quả vào [bằng chứng](auth-production-evidence.md) — relay giả trong `verify postgres`
   không thay được bước này.

## 5. Giám sát

| Tín hiệu | Nguồn | Ngưỡng gợi ý |
| --- | --- | --- |
| `mail_queue.pending`, `oldest_pending_seconds` | `/health/ready` của Identity, `vink-identity mail status` | Thư chờ > 60 s: relay chậm/sập |
| `mail_queue.failed_last_hour` | như trên | > 0 liên tục: domain/credential/relay |
| `code delivery failed; retrying` (`failure`, `status`) | log Identity | Nhiều `connect`/`tls`: egress/DNS; `transient 4xx`: relay quá tải |
| `sign-in code refused: throttle` (`throttle=global`) | log Identity | Có thể bị flood: xem §6.2 |
| `sign-in request refused: network ceiling` | log App | Một mạng thử liên tục |
| `write refused at commit: principal suspended` | log App | Đình chỉ đang chặn ghi (mong đợi) |

Readiness Identity báo `mail_transport` và `production_authentication` (`not_accepted` với SMTP,
`not_implemented` với outbox cục bộ) cho tới khi D-05 Accepted. Backlog thư **không** làm readiness
`503`: xác minh phiên không phụ thuộc mail.

## 6. Sự cố

### 6.1 Relay hoặc Resend sập
Không cần can thiệp ngay: API vẫn `202`, thư được giữ và thử lại tối đa 5 lần (khoảng 2–3 phút);
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
Tạo key mới (chỉ quyền gửi) → `install -m 600` đè file mật khẩu (worker đọc lại ở lần gửi sau,
không cần restart) → kiểm một lần gửi → thu hồi key cũ ở Resend. Không bao giờ dán key vào issue,
log, tài liệu, ảnh chụp hay Git; file key nằm ngoài repository.

### 6.5 Lộ DB Identity
DB chỉ có hash phiên/mã; mã đang chờ gửi lưu rõ trong `mail_deliveries.code` tới khi gửi xong
(không đủ để trả lời challenge vì thiếu bí mật client). Thu hồi toàn bộ phiên bằng
`UPDATE sessions SET revoked_at = now(), revoked_reason = 'logout' WHERE revoked_at IS NULL` (role
migrator/DBA) nếu nghi ngờ hash bị dùng sai.

## 7. Xoay secret

| Secret | Cách xoay | Ảnh hưởng |
| --- | --- | --- |
| API key SMTP | §6.4, không restart | Không |
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

Migration mới đều **cộng thêm**; binary cũ coi lịch sử có migration lạ là `diverged` (không ready),
nên rollback binary phải đi kèm script rollback tương ứng:

1. Đặt `VINK_AUTH_SIGN_IN=disabled` (nếu đã bật) — dừng phiên mới, giữ phiên cũ.
2. App: dừng → `psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f services/vink-app/migrations-rollback/20261112000000_auth_rate_limits.down.sql`
   → chạy binary cũ. Chỉ mất bộ đếm rate limit.
3. Identity: dừng → `psql "$VINK_IDENTITY_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f services/vink-identity/migrations-rollback/20261112000000_mail_outbox.down.sql`
   → chạy binary cũ (chỉ có outbox cục bộ). Chỉ mất thư đang chờ; tài khoản, phiên, challenge giữ
   nguyên, không ai bị đăng xuất.
4. Hai script được chạy trên PostgreSQL thật trong `verify postgres`. Luôn `pg_dump` theo owner
   trước khi migrate/rollback ([sao lưu](../../services/postgres/README.md)).
