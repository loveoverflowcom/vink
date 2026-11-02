# Bằng chứng ranh giới PostgreSQL/Identity

Bằng chứng chạy thật cho PR `codex/identity-postgres-boundaries` (một PR, ba commit), theo
[quyết định phạm vi](../architecture/identity-postgres-boundaries.md). Mỗi mục ghi revision/cây
được kiểm, lệnh, mã thoát và giới hạn. Credential local trong bảng là giá trị dùng một lần của
container/cluster tạm; credential test được che (`***`). SHA của commit chứa tài liệu này được báo
ở PR/handoff, không ghi vào chính commit.

## Commit 1 — PostgreSQL và tầng lưu trữ dùng chung

**BASE_SHA:** `5afb00b29c85965dc115718e6ada5a4dcd95399b` (`develop`, = merge-base). Nhánh
`codex/identity-postgres-boundaries` tạo từ base; mọi lệnh dưới đây chạy trên cây làm việc =
base + diff của commit 1 (chưa commit lúc chạy, không có thay đổi ngoài nhiệm vụ). Lệnh cuối
cùng chạy lại sau sửa đổi cuối của code.

**Môi trường:** macOS 26.5.2 arm64; Rust 1.99.1 (`rust-toolchain.toml`); Node 26.12.0; Google
Chrome 156.0.8078.79; `psql` 17.11 (Homebrew).

- PostgreSQL tạm cho lane: Homebrew 17.11 `initdb` trong thư mục scratch, chỉ TCP `127.0.0.1:55433`,
  `scram-sha-256` cho TCP; `VINK_TEST_DATABASE_URL=postgres://vink:***@127.0.0.1:55433/postgres`.
  Đã dừng và xóa sau khi kiểm.
- Compose thật: Podman 6.1.1 (machine `applehv`), `podman-compose` 1.6.0, image
  `docker.io/library/postgres:17.11-alpine` digest
  `sha256:a717692f944862397bd102c6abee9b3f7927adbf7ee5f3a93e50b65aea0c5396`. Máy không có volume
  `vink-dev_*` trước khi kiểm; các volume thử do phiên này tạo đã được xóa, danh sách volume trở
  về như ban đầu (không chạm volume của project khác).
- Binary “trước thay đổi”: build `vink-app` từ worktree tạm ở `5afb00b` để ghi dữ liệu cũ.

### Lane xtask

| Lệnh | Kết quả | Phạm vi |
| --- | --- | --- |
| `cargo xtask verify quick` | exit 0; PASS fmt, clippy server, clippy wasm, unit/contract, repository checks | Thuần + repository; không có database |
| `cargo xtask verify quick` với manifest probe `packages/zz-boundary-probe` phụ thuộc `axum` và `vink-app` | exit 1; repository checks báo đúng hai vi phạm; probe đã xóa | Ca âm cho luật phụ thuộc của `packages/*` |
| `VINK_TEST_DATABASE_URL=… cargo xtask verify postgres` | exit 0; 16 bước PASS (bảng dưới) | PostgreSQL thật, binary thật, `provision.sh` thật |
| `VINK_TEST_DATABASE_URL=… VINK_CHROME=… cargo xtask verify web` | exit 1. PASS prerequisites, build, DB tạm create/migrate/seed (đường mới: `VINK_TEST_DATABASE_URL` + `VINK_MIGRATE_DATABASE_URL`); smoke `main` FAIL “timed out waiting for dark theme applied” sau khi SSR, hydration, CodeMirror, lưu/reload/xung đột đã `ok`; console có `Failed to fetch dynamically imported module …/pkg/vink.js`; các bước sau NOT_RUN | Chrome headless, server thật; app dùng credential test harness trên DB tạm |
| Cùng lệnh trên worktree base `5afb00b` (`DATABASE_URL` = cùng cluster) | exit 1, FAIL ở **cùng** assertion với cùng lỗi console | Lỗi môi trường có từ trước (Chrome 156 macOS), không do commit này; cần chạy lại trên CI Linux |

Các bước `verify postgres`: PostgreSQL available; `vink-store-postgres` lifecycle (5 test);
App persistence/HTTP (12 test); build; config sai thoát 2 (serve, worker, và `migrate` khi chỉ
có `DATABASE_URL`); `psql` 15+; scratch sạch trước; DB App cũ do một superuser migrate + seed +
dữ liệu NFD; `provision.sh` nhận quyền DB cũ (NOTICE `adopting` 6 bảng + `reject_revision_update()`)
và tạo App mới + Identity; chạy lại với password khác không đổi verifier trong `pg_authid`, `--check`
PASS; `migrate` bằng runtime thoát 1 “lacks a privilege” và không tạo bảng lịch sử, migrator
migrate/rerun, seed bằng runtime; snapshot JSON dữ liệu + lịch sử DB cũ giống hệt trước/sau,
runtime đọc lại tiêu đề NFD đúng byte; runtime: DML được, `CREATE TABLE`/`TEMP`/`SCHEMA`/`ALTER`/
`DROP`/`TRUNCATE`/ghi `_sqlx_migrations`/`CREATE EXTENSION dblink` bị từ chối, bảy kết nối chỉ đổi
dbname bị `42501`, migrator không `CREATE DATABASE`/`ROLE`; ca âm cấp `CREATE` thừa làm `--check`
FAIL và runtime tạo được bảng (probe nhìn thấy quyền thừa), sau khi thu hồi `--check` PASS; worker
bằng runtime báo `database="current"`, SIGTERM thoát 0; scratch đã xóa.

### Provisioning — nhánh từ chối (thủ công, cluster tạm)

| Tình huống | Kết quả |
| --- | --- |
| Role mới thiếu password | psql exit 3: “role … does not exist and VINK_PROVISION_MIGRATOR_PASSWORD is not set”; không tạo role/database |
| Role runtime có sẵn là SUPERUSER | Dừng ở kiểm tra trước, gợi ý `ALTER ROLE … NOSUPERUSER …`; không đổi role |
| Database có sẵn chứa bảng thuộc role thứ ba | Dừng: “objects in n_app.public belong to neither the operator nor the migrator”; không tạo role, không đổi owner |

### Compose thật

| Kịch bản | Lệnh chính | Quan sát |
| --- | --- | --- |
| Người dùng từng chạy file cũ với `-p vinkmig` | `podman-compose -p vinkmig -f <infra/dev/compose.yaml@5afb00b> up -d`; binary base `migrate`, `dev-seed`, `POST`/`PUT /api/v1/drafts` (nháp tiếng Việt); `down` (không `-v`) | Volume `vinkmig_vink-postgres`, snapshot S0 |
| File mới, không `-p`, không biến volume | `podman-compose -f services/postgres/compose.yaml -f services/postgres/compose.local.yaml up -d` | exit 1 `External volume [vink-dev_vink-postgres] does not exist`; không tạo volume, không tạo container |
| File mới, `-p vinkmig`, `VINK_POSTGRES_VOLUME=vinkmig_vink-postgres` | `up -d` | healthy; cổng `127.0.0.1:55432->5432/tcp`; `SecurityOpt [no-new-privileges]`; snapshot = S0 |
| Provisioning DB cũ | `services/postgres/provision.sh --env-file services/postgres/provision.env.example` | exit 0; tạo 4 role, `vink_identity`; adopt 6 bảng + 1 function |
| Migrate bằng migrator | `vink-app migrate` với `VINK_MIGRATE_DATABASE_URL` migrator | `applied=[]`, exit 0; snapshot = S0 |
| Phục vụ bằng runtime | `vink-app serve` với `DATABASE_URL` runtime | `/health/ready` 200 `schema=current`; nháp do app base lưu đọc lại đúng tiêu đề/version; danh sách công khai có tác phẩm mẫu |
| Restart | `podman-compose … restart` | healthy; snapshot = S0; readiness 200 |
| Kết nối chéo | `psql` runtime App → `vink_identity`; DDL bằng runtime | `permission denied for database "vink_identity"`; `permission denied for schema public` |
| Project mặc định giữ volume | File cũ không `-p` (tạo `vink-dev_vink-postgres`), base migrate+seed, `down`; file mới không biến | healthy; snapshot = trước; nhãn volume `com.docker.compose.project=vink-dev` |
| Volume mới | `podman volume create`; `up`; `provision.sh`; `migrate`; `dev-seed` bằng runtime; `--check`; `restart` | `applied=[20261030000000]`; `--check` PASS ngay sau migrate đầu (quyền ghi lịch sử đã thu); dữ liệu còn sau restart; cả hai DB `UTF8` |
| `down -v` với volume external (probe riêng trước đó) | `podman-compose … down -v` rồi `up -d` | Volume còn, dữ liệu Unicode còn |

### Giới hạn và chưa chạy

- `docker compose` (Docker Engine) chưa được chạy: máy không có Docker. Cú pháp dùng (`external`,
  `name` có biến, nhiều `-f`, `security_opt`) là Compose spec chuẩn; hành vi trên Docker là
  NOT_RUN.
- Lane `web` vẫn chạy app bằng credential test harness trên DB tạm, không bằng role runtime đã
  provision; role separation qua browser là việc của commit 3.
- CI chưa có kết quả cho nhánh này lúc ghi; xem checks của PR. Runner `ubuntu-24.04` được giả định
  có `psql` 15+ (bước `psql --version` sẽ FAIL nếu không).
- Readiness không kiểm role đang chạy có phải superuser; provisioning và lane kiểm điều đó.
- PostgreSQL managed không có superuser, backup/restore từng owner, Identity service/contract và
  auth authority: chưa làm (commit 2/3; D-05/S-03 vẫn mở).
