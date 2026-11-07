# Vink Identity

Tiến trình riêng sở hữu **authentication facts** và trạng thái credential/bảo mật. Mặc định chưa
có authority (`authority_not_configured`). Spike D-05/S-03
([ADR đề xuất](../../docs/architecture/auth-authority.md)) thêm authority native **mã email một
lần** với tài khoản, địa chỉ đã xác minh, challenge và phiên opaque trong `vink_identity`; mail chỉ
ghi vào outbox cục bộ nên **production authentication: NOT_IMPLEMENTED**. Spike chạy được không có
nghĩa D-05 Accepted hay ACC-01/ACC-02 hoàn tất. Ranh giới database/process:
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
| [src/main.rs](src/main.rs) | `vink-identity [serve\|migrate]`; socket, shutdown, mã thoát 0/1/2 |
| [src/config.rs](src/config.rs) | Cấu hình, kiểm hợp lệ, che URL |
| [src/http.rs](src/http.rs) | Health, contract v1, xác thực caller theo peer uid, deadline |
| [src/contract.rs](src/contract.rs) | Kiểu wire canonical; lỗi có message cố định |
| [src/authority.rs](src/authority.rs) | `Authority::NotConfigured` hoặc `EmailCode`; fixture dương chỉ có trong unit test |
| [src/email_code.rs](src/email_code.rs) | Chính sách thuần: chuẩn hóa địa chỉ, mã 6 số, handle, credential phiên, hash, quyết định một lần thử |
| [src/accounts.rs](src/accounts.rs) | SQL: challenge, tạo tài khoản đồng thời, phiên, logout, đổi email, disable/enable |
| [src/mail.rs](src/mail.rs) | Outbox cục bộ (file `0600` trong thư mục `0700`); chưa có SMTP/API mail |
| [src/persistence.rs](src/persistence.rs) | `MIGRATOR` riêng, pool/lifecycle từ `vink-store-postgres` |
| [migrations/](migrations/) | Baseline + `accounts`, `account_emails`, `email_challenges`, `sessions` |
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
| `VINK_IDENTITY_AUTHORITY` | `serve` | Không đặt = không authority; `email-code` = authority spike (cần outbox) |
| `VINK_IDENTITY_LOCAL_OUTBOX` | `serve` | Thư mục `0700` nhận thư chứa mã (chỉ cục bộ); chỉ hợp lệ với `email-code` |
| `VINK_LOG_FORMAT` | mọi lệnh | `text` (mặc định) hoặc `json`; `RUST_LOG` chỉnh filter |

`vink-identity account disable|enable <subject-uuid>` dùng `VINK_IDENTITY_DATABASE_URL` (role
runtime): disable thu hồi mọi phiên của tài khoản trong cùng transaction; enable không khôi phục
phiên đã thu hồi. Không thao tác contract nào làm việc này.

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

Chạy thử authority spike (mã nằm trong file của outbox, không gửi đi đâu):

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
vét cạn), outbox và socket (quyền thư mục, socket cũ/đang phục vụ/file thường, không gỡ socket của
instance sau). `verify postgres` chạy thêm [tests/postgres/email_code.rs](tests/postgres/email_code.rs)
(mã một lần, đồng thời, throttle, logout, hết hạn, disable, đổi email) và bước đầu-cuối hai binary
thật trong [xtask](../../xtask/src/identity/email_code.rs). `verify postgres` chạy `tests/postgres.rs` và kiểm binary thật trên socket thật với
role đã provision: cấu hình sai, migrate hai lần, sai database/schema, role migrator bị từ chối,
mất kết nối, header gửi dở, SIGTERM/restart, socket cũ, log không lộ credential; và App thật gọi
Identity thật (dừng/khởi động lại độc lập, đọc công khai không phụ thuộc). Bằng chứng:
[identity-postgres-verification](../../docs/development/identity-postgres-verification.md).
