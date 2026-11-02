# PostgreSQL của Vink

Owner của container PostgreSQL và provisioning database/role cho mọi service Vink. Quyết định
phạm vi và lý do: [ranh giới PostgreSQL và Identity](../../docs/architecture/identity-postgres-boundaries.md).
Lệnh phát triển chung vẫn nằm ở [docs/development](../../docs/development/README.md).

| File | Vai trò |
| --- | --- |
| [compose.yaml](compose.yaml) | Định nghĩa container duy nhất: `postgres:17.11-alpine`, project `vink-dev`, volume external, healthcheck; **không publish cổng** |
| [compose.local.yaml](compose.local.yaml) | Overlay local: password bootstrap dùng một lần, cổng `127.0.0.1:55432` |
| [provision.sh](provision.sh) | Tạo/kiểm database và role của App và Identity; chạy lại an toàn |
| [provision/owner.sql](provision/owner.sql) | Provisioning một owner, gồm kiểm tra trước/sau; `provision.sh` gọi hai lần |
| [provision.env.example](provision.env.example) | Cấu hình mẫu; giá trị local dùng một lần, không phải secret production |

## Mô hình

Một instance, mỗi owner một database và hai role đăng nhập. Không role nào là thành viên role
khác; `PUBLIC` không còn `CONNECT`/`TEMPORARY` trên database owner.

| Database | Migrator (`VINK_MIGRATE_DATABASE_URL`) | Runtime (`DATABASE_URL`) |
| --- | --- | --- |
| `vink` (App, có sẵn) | `vink_app_migrator`: sở hữu object trong `public`, được `CREATE` schema | `vink_app_runtime`: DML trên bảng, chỉ `SELECT` `_sqlx_migrations` |
| `vink_identity` (migrations của [vink-identity](../vink-identity/README.md); hiện chỉ baseline) | `vink_identity_migrator` (`VINK_IDENTITY_MIGRATE_DATABASE_URL`) | `vink_identity_runtime` (`VINK_IDENTITY_DATABASE_URL`) |

Operator/bootstrap là superuser `vink` do image tạo khi khởi tạo volume. Chỉ dùng nó cho
provisioning, backup và test harness local; không đưa vào app. Không role service nào có
SUPERUSER, CREATEDB, CREATEROLE, REPLICATION hay BYPASSRLS; không `dblink`/`postgres_fdw`.

## Local lần đầu

Cần Podman hoặc Docker có Compose, và `psql` 15+ trên host (`brew install libpq` hoặc gói
`postgresql-client`). Thay `podman compose` bằng `docker compose` hoặc `podman-compose` nếu cần.

```bash
podman volume create vink-dev_vink-postgres
```

```bash
podman compose -f services/postgres/compose.yaml -f services/postgres/compose.local.yaml up -d
```

```bash
services/postgres/provision.sh --env-file services/postgres/provision.env.example
```

```bash
cp .env.example .env
```

```bash
cargo xtask db migrate
```

Volume được tạo một lần, có chủ đích: Compose không tự tạo volume external, nên chạy sai project
hay sai đường dẫn sẽ báo `External volume ... does not exist` thay vì khởi tạo cluster rỗng. Volume
external cũng không bị `down -v` xóa. Healthcheck chỉ cho biết server nhận kết nối; schema và
quyền được kiểm bằng `provision.sh --check` và `/health/ready`.

## Chuyển từ infra/dev

File cũ `infra/dev/compose.yaml` tạo volume `<project>_vink-postgres`, mặc định project
`vink-dev`. Dữ liệu nằm trong volume, không nằm trong file Compose.

1. Tìm volume đang giữ dữ liệu: `podman volume ls --format '{{.Name}}' | grep vink-postgres`.
   - `vink-dev_vink-postgres`: giữ nguyên lệnh ở trên, không cần biến nào.
   - Từng chạy với `-p <tên>` (hoặc `COMPOSE_PROJECT_NAME`): volume là `<tên>_vink-postgres`;
     đặt `VINK_POSTGRES_VOLUME=<tên>_vink-postgres` cho mọi lệnh Compose, có thể giữ `-p <tên>`.
2. Dừng container cũ bằng `down` **không** có `-v`, rồi `up -d` bằng hai file mới. Container cũ
   đang chạy cùng project cũng được thay; volume giữ nguyên.
3. Kiểm thử trên bản sao trước khi đụng database đang dùng (chỉ cần quyền operator; dừng app vì
   `TEMPLATE` cần database nguồn không có kết nối):

   ```bash
   psql -h 127.0.0.1 -p 55432 -U vink -d postgres -c 'CREATE DATABASE vink_copy TEMPLATE vink'
   ```

   ```bash
   VINK_APP_DATABASE=vink_copy VINK_APP_MIGRATOR_ROLE=vink_copy_migrator VINK_APP_RUNTIME_ROLE=vink_copy_runtime VINK_APP_MIGRATOR_PASSWORD=copy-only VINK_APP_RUNTIME_PASSWORD=copy-only services/postgres/provision.sh --env-file services/postgres/provision.env.example
   ```

   Xem NOTICE `adopting ...`, chạy app với role của bản sao nếu muốn, rồi xóa **bản sao**
   (`DROP DATABASE vink_copy`, `DROP ROLE vink_copy_migrator, vink_copy_runtime`).
4. Chạy `provision.sh` cho database thật, cập nhật `.env` theo `.env.example` (`DATABASE_URL`
   runtime, `VINK_MIGRATE_DATABASE_URL`, `VINK_TEST_DATABASE_URL`), rồi `cargo xtask db migrate`.
   Migration đã áp dụng không chạy lại; dữ liệu và lịch sử giữ nguyên.

Không dùng `down -v`, xóa volume, reset database hay sửa migration cũ để “làm lại cho sạch”.

## Provisioning

`provision.sh` kết nối bằng biến libpq (`PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD` hoặc
`~/.pgpass`, `PGDATABASE=postgres`) của operator, rồi với từng owner:

1. Kiểm tra trước, không ghi: tên hợp lệ, đang là superuser, role có sẵn không vượt quyền và
   không là thành viên role khác, database (nếu có) UTF8, không thuộc role service, không có
   `dblink`/`postgres_fdw`/foreign table, object trong `public` chỉ thuộc operator hoặc migrator.
2. Tạo role thiếu với password từ môi trường. Role đã có giữ nguyên thuộc tính và password.
3. Tạo database thiếu (`template0`, UTF8, owner là operator). `CREATE DATABASE` không nằm trong
   transaction; lỗi giữa chừng để lại phần đã làm, chạy lại sẽ hoàn tất.
4. Trong một transaction: thu `CONNECT`/`TEMPORARY` của `PUBLIC`, cấp `CONNECT` cho hai role, chuyển
   object operator đã tạo trong `public` cho migrator, cấp DML và default privileges cho runtime,
   chỉ để owner ghi `_sqlx_migrations`.
5. Kiểm tra sau, không ghi; lỗi nào cũng làm lệnh thoát khác 0 và liệt kê đủ.

`--check` chỉ chạy bước 1 và 5. Password có thể là chữ thường hoặc SCRAM verifier
(`SCRAM-SHA-256$...`). Chuỗi gửi tới server khi tạo role, nên ở production nên dùng verifier và
không bật `log_statement` cho DDL lúc provisioning. Lần migrate đầu tạo `_sqlx_migrations` dưới
default privileges; `vink-store-postgres` thu lại quyền ghi của role khác sau mỗi lần migrate.

Sau provisioning, chỉ migrate bằng role migrator. Migrate bằng superuser sẽ tạo object không
thuộc migrator: runtime không có quyền trên chúng và `--check` báo lỗi; chạy lại `provision.sh` bằng
chính superuser đó để migrator nhận lại quyền sở hữu.

`postgres` và `template1` vẫn mở `CONNECT` cho `PUBLIC` theo mặc định PostgreSQL và không chứa dữ
liệu Vink; script chỉ báo NOTICE. Object ngoài schema `public` không được nhận hay cấp quyền.

## Production

Chỉ dùng `compose.yaml`, không dùng overlay local. Cung cấp `POSTGRES_PASSWORD` (hoặc
`POSTGRES_PASSWORD_FILE`) từ secret store khi khởi tạo volume, đặt `VINK_POSTGRES_VOLUME` và project
riêng (`-p`), không publish cổng database ra Internet; service khác kết nối qua mạng nội bộ.
Provisioning chạy từ máy operator bằng `psql`. PostgreSQL managed không có superuser thật cần một
quy trình tương đương được kiểm riêng; chưa có ở đây. Backup/restore theo từng owner và giới hạn
nhất quán giữa hai database thuộc commit 3 của PR này.

## Kiểm chứng

`cargo xtask verify postgres` chạy provisioning thật với tên tạm trên server của
`VINK_TEST_DATABASE_URL`: database App tạo theo cách cũ rồi được nhận quyền, App mới, Identity;
chạy lại; migrate/seed bằng binary thật; thử DDL và kết nối chéo bằng chính các role; một ca âm
chứng minh `--check` bắt được quyền thừa. Bằng chứng chạy và Compose thật:
[identity-postgres-verification](../../docs/development/identity-postgres-verification.md).
