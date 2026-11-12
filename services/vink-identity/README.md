# Vink Identity

Tiến trình riêng sở hữu **authentication facts** và trạng thái credential/bảo mật. Mặc định chưa
có authority (`authority_not_configured`). Authority native hỗ trợ mã email và username/mật khẩu
qua cùng tài khoản và phiên opaque ([ADR D-05, Provisional](../../docs/architecture/auth-authority.md)).
Identity lưu địa chỉ đã xác minh, challenge, hash mật khẩu và outbox trong `vink_identity`. Mã được gửi bởi worker của
`serve` qua SMTP (nhà cung cấp mặc định đề xuất: Resend) hoặc outbox cục bộ khi phát triển. Đường
mail production đã có code và bằng chứng giao thức, nhưng D-05 vẫn **Provisional** (readiness báo
`production_authentication: not_accepted` với SMTP, `not_implemented` với outbox cục bộ); vận hành
xem [runbook](../../docs/development/auth-operations.md). Ranh giới database/process:
[ranh giới PostgreSQL và Identity](../../docs/architecture/identity-postgres-boundaries.md).

| Thuộc Identity | Thuộc App (`vink-app`) |
| --- | --- |
| Tài khoản (subject), địa chỉ email đã xác minh, challenge mã một lần, phiên (chỉ lưu hash); xác minh credential, trả fact `subject` + thời điểm | Principal Vink ổn định, binding `(authority, subject)` → principal, đình chỉ, carrier cookie/Bearer |
| Disable tài khoản (lệnh vận hành), thu hồi phiên khi logout/disable/đổi email | Quyền admin/biên tập/kiểm duyệt/xuất bản, tác phẩm, nháp, publication trong `vink` |

Identity không cấp quyền nghiệp vụ, không đọc bảng App; App không đọc bảng Identity. Hai crate
không import nhau (`verify quick` kiểm manifest và SQL); không dùng chung password database,
session hay cookie với Cantos/VOT.

| File | Vai trò |
| --- | --- |
| [src/main.rs](src/main.rs) | `vink-identity [serve\|migrate\|account …\|maintenance prune\|mail status]`; socket, worker, prune hằng giờ, shutdown, mã thoát 0/1/2 |
| [src/config.rs](src/config.rs) | Cấu hình, kiểm hợp lệ, che URL |
| [src/http.rs](src/http.rs) | Health, contract v1, xác thực caller theo peer uid, deadline |
| [src/contract.rs](src/contract.rs) | Kiểu wire canonical; lỗi có message cố định |
| [src/authority.rs](src/authority.rs) | Authority native và khả năng theo từng phương thức |
| [src/email_code.rs](src/email_code.rs) | Chính sách thuần: chuẩn hóa địa chỉ, mã 6 số, handle, credential phiên, hash, quyết định một lần thử |
| [src/accounts.rs](src/accounts.rs) | Challenge và outbox nguyên tử, giới hạn gửi, tài khoản, phiên, đổi email, thêm credential, disable/enable và retention |
| [src/password.rs](src/password.rs) | Chuẩn hóa O-12@r2, Argon2id và tương thích hash cũ khi đăng nhập |
| [src/delivery.rs](src/delivery.rs) | Outbox `mail_deliveries`: chính sách retry thuần, claim `SKIP LOCKED` + lease, worker at-least-once |
| [src/mail.rs](src/mail.rs) | Transport: SMTP (`lettre`, TLS bắt buộc ngoài loopback, mật khẩu đọc từ file mỗi lần gửi) và outbox cục bộ (file `0600`, chỉ dev) |
| [src/persistence.rs](src/persistence.rs) | `MIGRATOR` riêng, pool/lifecycle từ `vink-store-postgres` |
| [migrations/](migrations/) | Schema Identity, gồm `account_passwords` và binding phiên cho challenge enrollment |
| [migrations-rollback/](migrations-rollback/) | Script rollback theo phiên bản. Rollback enrollment giữ mật khẩu đã thêm và dọn mã enrollment cùng delivery |
| [contracts/rpc/v1](contracts/rpc/v1/README.md) | Contract riêng tư (draft) và fixtures do provider sở hữu |

## Topology và caller

Không có cổng TCP. `serve` nghe HTTP/1.1 trên Unix socket `VINK_IDENTITY_SOCKET`. Thư mục chứa
socket phải tồn tại, không cấp quyền cho other và không cho group ghi (`chmod g-w,o-rwx`); socket
được tạo với mode `0660`. Chỉ socket không ai nghe (`ECONNREFUSED`) mới được thay; socket đang
phục vụ, không xác định được hoặc file thường thì giữ nguyên. Khi dừng, tiến trình chỉ gỡ đúng
file socket nó đã bind (so device/inode), không gỡ socket của instance thay thế.

Mỗi kết nối phải gửi xong request head trong 2 s; mỗi lời gọi contract (kể cả upload body) bị
giới hạn 2 s. SIGTERM ngừng nhận kết nối, chờ tối đa 5 s cho request đang chạy rồi đóng phần còn lại.

Caller được xác thực bằng **user của tiến trình kết nối** do kernel báo (`SO_PEERCRED` /
`getpeereid`), so với `VINK_IDENTITY_CALLER_UID`. Không header, trường body, địa chỉ loopback hay
mạng Docker nào là bằng chứng caller. Thiếu `VINK_IDENTITY_CALLER_UID` thì mọi lời gọi contract
nhận `401 caller_unauthenticated` và readiness là `503`. Health (`/health/live`, `/health/ready`) trả
lời mọi tiến trình tới được socket và không chứa subject/credential.

Khi chạy trong container, App và Identity cần cùng host và chia sẻ thư mục socket (volume), App
chạy bằng uid đã cấu hình. Chế độ mạng (ví dụ mTLS) chưa được thiết kế.

## Cấu hình

| Biến | Dùng cho | Ý nghĩa |
| --- | --- | --- |
| `VINK_IDENTITY_DATABASE_URL` | `serve` | Role runtime `vink_identity_runtime` trên `vink_identity` |
| `VINK_IDENTITY_MIGRATE_DATABASE_URL` | `migrate` | Role migrator `vink_identity_migrator`; không fallback sang URL runtime |
| `VINK_IDENTITY_SOCKET` | `serve` | Đường dẫn socket, tối đa 100 byte |
| `VINK_IDENTITY_CALLER_UID` | `serve` | UID duy nhất được gọi contract; không đặt = đóng |
| `VINK_IDENTITY_AUTHORITY` | `serve` | Không đặt thì chưa có authority. `email-code` và `password` bật từng phương thức. `native` bật cả hai và cho phép enrollment. `email-code` và `native` cần mail transport |
| `VINK_IDENTITY_MAIL_TRANSPORT` | `serve` | `smtp` hoặc `local-outbox` (dev). Bỏ trống mà có `VINK_IDENTITY_LOCAL_OUTBOX` = outbox cục bộ |
| `VINK_IDENTITY_LOCAL_OUTBOX` | `serve` | Thư mục `0700` nhận thư chứa mã (chỉ cục bộ) |
| `VINK_IDENTITY_SMTP_PROVIDER` | `serve`, `smtp` | Preset nhà cung cấp (`resend` = `smtp.resend.com`, `tls`/465, user `resend`); biến tường minh vẫn thắng |
| `VINK_IDENTITY_SMTP_HOST` | `serve`, `smtp` | Relay, ví dụ `smtp.resend.com` (không cần khi có preset) |
| `VINK_IDENTITY_SMTP_SECURITY` | `serve`, `smtp` | `starttls` (mặc định, cổng 587), `tls` (465) hoặc `none` (chỉ host loopback) |
| `VINK_IDENTITY_SMTP_PORT` | `serve`, `smtp` | Mặc định theo `SECURITY` |
| `VINK_IDENTITY_SMTP_USERNAME` / `VINK_IDENTITY_SMTP_PASSWORD_FILE` | `serve`, `smtp` | Đặt cả hai hoặc không; file `0600`, đọc lại ở mỗi lần gửi. `VINK_IDENTITY_SMTP_PASSWORD` (giá trị) bị từ chối |
| `VINK_IDENTITY_MAIL_FROM` | `serve`, `smtp` | Mailbox gửi, domain đã xác minh ở nhà cung cấp |
| `VINK_IDENTITY_SMTP_TIMEOUT_SECONDS` | `serve`, `smtp` | 1–30, mặc định 10 |
| `VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT` | `serve` | Trần mã/10 phút trên mọi địa chỉ (mặc định 200) |
| `VINK_LOG_FORMAT` | mọi lệnh | `text` (mặc định) hoặc `json`; `RUST_LOG` chỉnh filter |

`vink-identity account disable|enable <subject-uuid>` dùng `VINK_IDENTITY_DATABASE_URL` (role
runtime): disable thu hồi mọi phiên của tài khoản trong cùng transaction; enable không khôi phục
phiên đã thu hồi. Không thao tác contract nào làm việc này. `vink-identity maintenance prune` xóa
challenge (kèm thư) quá hạn 7 ngày và phiên đã đóng quá 30 ngày (cũng chạy mỗi giờ trong `serve`);
`vink-identity mail status` in số thư chờ, tuổi thư chờ lâu nhất và số thư lỗi trong giờ qua.
`vink-identity mail test <địa-chỉ>` gửi một thư kiểm tra (không mã) qua đúng sender đã cấu hình,
không cần database — kiểm egress/DNS/TLS/credential trước khi bật.

Biến của App (`DATABASE_URL`, `VINK_MIGRATE_DATABASE_URL`) không được đọc. Lỗi cấu hình thoát `2`
và không in URL.

## Chạy local

Sau khi chạy [provisioning](../postgres/README.md) (tạo `vink_identity` và hai role):

```bash
VINK_IDENTITY_MIGRATE_DATABASE_URL=postgres://vink_identity_migrator:vink-identity-migrator-local-only@127.0.0.1:55432/vink_identity cargo run -q -p vink-identity -- migrate
```

```bash
mkdir -p -m 700 target/identity && VINK_IDENTITY_DATABASE_URL=postgres://vink_identity_runtime:vink-identity-runtime-local-only@127.0.0.1:55432/vink_identity VINK_IDENTITY_SOCKET=target/identity/identity.sock VINK_IDENTITY_CALLER_UID=$(id -u) cargo run -q -p vink-identity -- serve
```

```bash
curl --unix-socket target/identity/identity.sock http://identity/health/ready
```

Chạy thử authority với outbox cục bộ (mã nằm trong file, không gửi đi đâu; worker ghi file sau commit):

```bash
mkdir -p -m 700 target/identity/outbox && VINK_IDENTITY_AUTHORITY=email-code VINK_IDENTITY_LOCAL_OUTBOX=target/identity/outbox VINK_IDENTITY_DATABASE_URL=postgres://vink_identity_runtime:vink-identity-runtime-local-only@127.0.0.1:55432/vink_identity VINK_IDENTITY_SOCKET=target/identity/identity.sock VINK_IDENTITY_CALLER_UID=$(id -u) cargo run -q -p vink-identity -- serve
```

`serve` không tự migrate: thiếu migration là readiness `pending_migrations`; lịch sử lạ (ví dụ trỏ
nhầm database App) là `diverged`. Readiness còn đòi role đang kết nối không có quyền DDL/superuser
(`checks.role = restricted`): chạy `serve` bằng credential migrator hay operator là `503 privileged`. Với `.env` từ `.env.example`, `cargo xtask db migrate` và `cargo xtask dev` migrate và chạy
Identity (xem development guide). App là consumer duy nhất: nó gửi credential được trình qua
client riêng ([services/vink-app/src/identity](../vink-app/src/identity/client.rs)) và kiểm uid
của server.

## Kiểm chứng

`verify quick` chạy unit test contract (caller, header giả, payload, version, deadline, fixtures,
redaction), chính sách mã email (chuẩn hóa NFC, mã, handle, credential, bảng quyết định thử mã
vét cạn), chính sách retry thư (vét cạn số lần × loại lỗi), adapter SMTP với relay giả qua TCP
loopback (chấp nhận, 4xx, 5xx, im lặng, không có relay), cấu hình SMTP và socket. `verify postgres`
chạy thêm [tests/postgres/email_code.rs](tests/postgres/email_code.rs) (mã một lần, đồng thời,
throttle, logout, hết hạn, disable, đổi email, không dò địa chỉ),
[tests/postgres/delivery.rs](tests/postgres/delivery.rs) (outbox cùng transaction, retry/backoff,
lỗi vĩnh viễn, giới hạn lần thử, hủy mã hết hạn, crash/lease/trùng thư, nhiều worker, trần toàn
cục/tài khoản, prune, rollback) và bước đầu-cuối hai binary thật trong
[xtask](../../xtask/src/identity/email_code.rs), gồm gửi qua SMTP loopback, xoay mật khẩu và relay
sập rồi phục hồi. Relay giả không chứng minh deliverability: gửi thật qua Resend cần kiểm riêng
(xem runbook). `verify postgres` chạy `tests/postgres.rs` và kiểm binary thật trên socket thật với
role đã provision: cấu hình sai, migrate hai lần, sai database/schema, role migrator bị từ chối,
mất kết nối, header gửi dở, SIGTERM/restart, socket cũ, log không lộ credential; và App thật gọi
Identity thật (dừng/khởi động lại độc lập, đọc công khai không phụ thuộc). Bằng chứng:
[identity-postgres-verification](../../docs/development/identity-postgres-verification.md).
