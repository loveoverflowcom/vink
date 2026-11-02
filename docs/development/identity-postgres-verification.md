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

## Commit 2 — Identity service độc lập

**Predecessor:** commit 1 `6357922c149cb6f4e97358613ef8aa83f85c2d59` nằm trong ancestry của nhánh
(`git merge-base --is-ancestor`), remote nhánh trùng SHA đó. Trước khi viết code, chạy lại
`cargo xtask verify postgres` trên chính `6357922` với cluster PostgreSQL 17.11 tạm mới: exit 0,
16/16 PASS. CI của PR #26 trên `6357922`: `quick` và `postgres` PASS (runner có `psql` 15+);
`web` FAIL với `Failed to fetch dynamically imported module …/pkg/vink.js` — cùng chữ ký lỗi với
CI `develop` tại `5afb00b` và ba merge trước đó, nên là lỗi có từ trước, không do PR này.

**Cây được kiểm:** `6357922` + diff commit 2 (chưa commit lúc chạy). Môi trường như commit 1;
không cần Podman cho commit 2.

**Contract:** `vink.identity.v1` draft tại
[services/vink-identity/contracts/rpc/v1](../../services/vink-identity/contracts/rpc/v1/README.md),
revision = nội dung commit 2. **Migration ownership:** `services/vink-identity/migrations`
(`20261102000000_identity_baseline`, không bảng), áp dụng bằng `vink_identity_migrator` lên
`vink_identity`; App migrations không đổi. **Authority:** không có; production authentication
NOT_IMPLEMENTED. Câu trả lời `authenticated`/`rejected` chỉ được tạo bởi fixture unit test có nhãn
`test-fixture`, không có trong binary; mọi kiểm tra process dùng binary thật nên chỉ thấy
`authority_not_configured`.

| Lệnh / kịch bản | Kết quả | Phạm vi |
| --- | --- | --- |
| `cargo test -p vink-identity --lib` (trong `verify quick`) | 15 test PASS (chạy lặp 5 lần trước sửa review: ổn định) | Router thật, peer uid gắn như accept loop gắn; socket thật trong thư mục tạm; DB outage giả lập bằng cổng 1 |
| Ca âm: tạm cho middleware caller cho qua mọi peer | 2 test FAIL (`the_caller_is_the_peer_user_and_nothing_else`, `versions_paths_and_methods_are_explicit`); đã hoàn tác | Độ nhạy của test caller |
| `cargo xtask verify quick` | exit 0, 5/5 PASS | fmt, clippy SSR/WASM, unit, repository checks (luật App ⇄ Identity mới) |
| Ca âm: file probe chứa `SELECT … FROM works JOIN working_drafts` trong `services/vink-identity/src` | repository checks FAIL đúng 2 vi phạm; probe đã xóa | Độ nhạy luật “Identity không chạm bảng App” |
| `strings target/debug/vink-identity` tìm `fixture-credential-accepted`, `fixture-subject-0001` | 0 lần (chuỗi `authority_not_configured` có 1) | Fixture dương không có trong binary |
| `VINK_TEST_DATABASE_URL=… cargo xtask verify postgres` | exit 0, 24/24 PASS (chạy lại sau mọi sửa review) | PostgreSQL thật; store 6 test, App 12, Identity 4 + 7 bước binary thật trên Unix socket thật với role tạm đã provision |
| Ca âm: tạm đặt `header_read_timeout(None)` | lane FAIL đúng bước lifecycle: “a stalled request head was not closed within 5 s”; đã khôi phục | Độ nhạy hồi quy P3-1 |
| Ca âm: probe `SELECT id FROM⏎ public.\"works\"` | repository checks FAIL `works`; probe đã xóa | Lint sau khi siết (P3-4) |

Bảy bước binary Identity: cấu hình thiếu/sai thoát 2 nêu đúng biến (thiếu URL/socket, `migrate` chỉ
có URL runtime/App, caller không phải số, scheme sai không lộ password, thư mục socket mở cho other,
file thường tại đường dẫn socket được giữ nguyên); `migrate` bằng runtime thoát 1 và không tạo bảng,
migrator áp dụng đúng `[20261102000000]` rồi `[]`, migrator Identity sang database App bị `permission
denied`, chạy registry Identity trên lịch sử App bị `diverged` và lịch sử App không đổi; `serve`:
readiness 200 `schema=current`, capabilities `not_configured`, verify `501`, payload hỏng/rỗng/subject
giả `400`, 9000 byte `413`, `/identity/v2` `404 unsupported_version`, upload treo `503
deadline_exceeded` trong ≤ 4 s; uid khác và thiếu caller `401` cả khi có `x-user-id`/`x-vink-principal`/
`x-forwarded-for`/`authorization` giả, header giả với caller đúng vẫn `501`; `pg_terminate_backend`
mọi kết nối Identity rồi readiness tự về 200; DB không tới được, role Identity trỏ database App
(`unavailable`) và role App trên database App (`schema=diverged`) đều `503`, live vẫn `200`; SIGTERM
thoát 0, log `stopped`, socket bị gỡ; instance thứ hai thoát 1 “already serving” mà instance đầu vẫn
ready; SIGKILL để lại socket, lần chạy sau báo “replacing a stale socket” và ready; quét mọi log/stderr
không thấy password tạm, password test harness hay chuỗi credential gửi vào.

### Review độc lập và sửa

Reviewer: một agent riêng, chỉ đọc (không phải approval của người); được giao code và yêu cầu, không
được giao kết luận. Kết quả: không P0–P2; bốn P3 và các giới hạn coverage. Mỗi finding được tái
hiện hoặc lần trong code trước khi sửa:

| Finding | Tái hiện / lần vết | Sửa | Kiểm lại |
| --- | --- | --- | --- |
| P3-1: không timeout đọc request head; SIGTERM treo vô hạn | Binary trước sửa: gửi dở request line rồi SIGTERM → vẫn chạy sau 8 s, SIGKILL để lại socket | Vòng accept riêng ([server.rs](../../services/vink-identity/src/server.rs)): hyper HTTP/1 với `header_read_timeout` 2 s, shutdown chờ tối đa 5 s rồi đóng | Binary sau sửa: head dở bị đóng sau 2,0 s; SIGTERM khi còn head treo thoát sau 2,1 s, socket được gỡ; thêm bước lifecycle + ca âm |
| P3-2: gỡ socket không kiểm còn là của mình; mọi lỗi connect bị coi là socket cũ | Lần trong code: `remove_file` vô điều kiện, `connect().is_ok()` | Chỉ gỡ khi device/inode trùng file đã bind; chỉ `ECONNREFUSED` là socket cũ; lỗi khác từ chối | Unit test: socket của instance sau không bị gỡ, socket sống/file thường giữ nguyên |
| P3-3: xtask ngắt kết nối `vink-identity` ở mọi database | Lần trong code: thiếu điều kiện database | Thêm `datname` = database Identity tạm | Lane PASS |
| P3-4: lint ranh giới bỏ sót `[dependencies.x]`, package đổi tên, `FROM⏎ public."works"` | Probe trước khi siết không bị bắt | Manifest: cấm mọi lần nhắc tên crate kia; SQL: bỏ nháy/escape, gộp khoảng trắng, bỏ `public.` | Ca âm FAIL đúng |

Giới hạn coverage reviewer nêu và cách xử lý: readiness không phân biệt credential runtime/migrator →
thêm `vink_store_postgres::role_is_privileged`, readiness Identity đòi `role = restricted` (binary chạy
bằng migrator: `503 privileged`; bằng runtime: `200 restricted`); thư mục socket group-writable → bị
từ chối; chuỗi fixture gửi vào binary thật → `501`; path giải mã hỏng → envelope `not_found`; oracle
`applied` trong test Identity đổi thành literal `[20261102000000]`. Bỏ dùng `MockConnectInfo` trong
code production (peer chỉ đến từ accept loop).

**Còn mở / chưa làm:** uid caller xác thực user, không phải tiến trình — mọi tiến trình chạy bằng uid
đó và tới được socket đều qua; config không cấm uid 0 hay uid của chính Identity (local dev dùng cùng
user). App chưa kiểm uid server (commit 3). Không có authority thật, mapping subject → principal,
chế độ mạng/mTLS, hay đăng nhập: production authentication NOT_IMPLEMENTED; S-03, ACC-01/02, D-05 không
đổi trạng thái. `verify web` không đổi phạm vi ở commit này (lỗi có từ trước, xem trên).

## Commit 3 — Tích hợp, kiểm chứng và review toàn PR

**Predecessors:** `BASE_SHA=5afb00b29c85965dc115718e6ada5a4dcd95399b` = merge-base với `develop`;
HEAD trước commit 3 = `255ce77d56fdc175b4c35e0916a2bfaf27708e8b` (= remote); commit 1 `6357922` và
commit 2 `255ce77` nằm trong ancestry; cây sạch khi bắt đầu. CI của `255ce77`: `postgres` PASS,
`quick` FAIL — `server::tests::removal_spares_a_successor_socket` trên Linux. Nguyên nhân: sau khi
listener đóng, Linux tái dùng ngay inode của file socket, nên so device/inode (bản sửa P3-2 của
commit 2) không phân biệt được socket của instance sau. Sửa trong commit này: socket được gỡ **lúc
bắt đầu shutdown, khi listener còn giữ inode**, rồi mới đóng listener và drain; test dựng đúng kịch
bản (listener đầu còn mở khi đường dẫn bị thay). Sau sửa, `verify postgres` chạy lại trên cluster
tạm mới: 24/24 PASS (contract, quyền DB, migrations, đường từ chối). `web` FAIL trên CI của PR này
và của `develop` là lỗi script smoke có từ trước; phiên riêng đã sửa ở PR #27 (chỉ đổi
`browser-smoke.mjs` và feature map), CI `web` của #27 PASS.

**Cây được kiểm:** `255ce77` + diff commit 3 (chưa commit lúc chạy). Môi trường như trên (macOS 26.5.2
arm64, Rust 1.99.1, PostgreSQL 17.11 Homebrew tạm ở `127.0.0.1:55433`, `pg_dump`/`pg_restore` 17.11,
Chrome 156, Node 26.12.0).

| Lệnh / kịch bản | Kết quả | Phạm vi |
| --- | --- | --- |
| `cargo xtask doctor` (biến môi trường trỏ cluster tạm đã provision bằng tên mặc định) | exit 0; PASS tất cả mục bắt buộc kể cả hai URL Identity; WARN `.env` không có | Cấu hình, kết nối từng role |
| `cargo xtask db migrate` hai lần | Lần 1: App `[20261030000000]`, Identity `[20261102000000]`; lần 2: `[]` cả hai | Migrate theo owner bằng credential migrator |
| `cargo xtask dev` trong process group riêng, rồi SIGINT cả nhóm | Identity ready trên socket (200) trước App; App ready `role=restricted`, `identity=no_authority`; Bàn viết qua local fixture 200; SIGINT: cả hai log `stopped`, socket bị gỡ, không còn tiến trình `serve` | Dev runner quản lý vòng đời/readiness |
| `cargo xtask verify quick` | exit 0, 5/5 PASS; 30 unit test App (gồm 3 test App ↔ Identity giả trên socket thật, 2 test fixture consumer), 15 Identity | Thuần + repository (lint bí mật mới) |
| Ca âm: resolver gộp “Identity không quyết được” thành ẩn danh | 2 test App FAIL; đã hoàn tác | Độ nhạy chống fallback |
| Ca âm: file probe được track chứa URL PostgreSQL có password trông như thật | repository checks FAIL đúng file; probe đã gỡ | Độ nhạy lint bí mật |
| `VINK_TEST_DATABASE_URL=… cargo xtask verify postgres` | exit 0, 27/27 PASS | Thêm: App + Identity thật cùng chạy, App từ chối role migrator và server Identity lạ, backup/restore từng owner |
| `VINK_TEST_DATABASE_URL=… VINK_CHROME=… cargo xtask verify web` với **riêng** `browser-smoke.mjs` của PR #27 (`1a02c0b`) áp tạm, đã hoàn tác sau khi chạy | exit 0, 7/7 PASS, 330 assertion `ok`; bundle 27 file không chứa URL database, tên biến credential hay password local | Chrome headless, server thật; hồi quy save/reload/CAS, guard rời trang, CodeMirror/preview/export, SSR privacy, tìm kiếm/ngữ cảnh #18 |

Bước App + Identity (hai binary thật, profile production, role đã provision): không credential
`401`; `Authorization: Bearer` kèm `x-user-id`/`x-vink-principal` giả `401` (chưa authority);
`/api/v1/works`, `/` và trang đọc `200`, HTML không chứa password, credential gửi vào, `postgres://`
hay `_DATABASE_URL`; dừng Identity → readiness App vẫn `200` với `identity=unavailable`, không
credential vẫn `401`, có credential `503 dependency_unavailable`, đọc công khai vẫn `200`; Identity
khởi động lại → `401` mà không restart App; App restart riêng → ready. App bằng credential migrator
(production) → `503 role=privileged`; App tin uid server khác → có credential `503`, readiness
`identity=unavailable`.

Backup/restore: `pg_dump --format=custom` database App tạm (đã nhận quyền từ bố cục cũ, có dữ liệu
NFD/NFC, revision, con trỏ publication) và database Identity tạm; `pg_restore --exit-on-error` vào hai
database mới khác tên; snapshot dữ liệu + lịch sử bằng nhau, nguồn không đổi, bảng App restore vẫn
thuộc migrator.

**Sai sót quy trình đã sửa:** trong lần kiểm Compose ở commit 1, script ghi dữ liệu bằng binary base
để sót một `vink-app serve` (cổng `127.0.0.1:3999`, binary trong worktree tạm đã xóa) chạy từ 10:50.
Phát hiện khi kiểm dev runner, đã SIGTERM. Không ảnh hưởng dữ liệu hay kết quả kiểm khác (cổng và
database khác).

### Review toàn PR và kiểm local tiếp ngày 02/11/2026

Phạm vi: merge-base `5afb00b` → hai commit `6357922`, `255ce77` + toàn bộ cây commit 3 đang dở;
đối chiếu source, callers, contract provider/consumer, provisioning, runbook và assertions. Đây
là review tiếp bởi Codex, không phải approval của người. Không chọn authority, không thêm binding
subject → principal và không đổi luật publication. Không có model Quint trong phạm vi này.

| Finding | Bằng chứng trước sửa | Sửa và kiểm lại |
| --- | --- | --- |
| P2: lệnh thử chuyển quyền trên bản sao thực ra chọn database gốc | Giữ nguyên lệnh trong runbook, dùng `PSQL` ghi lại argv: dù truyền `VINK_APP_DATABASE=vink_copy`, script vẫn nhận `db=vink`, vì `--env-file` source lại tên mặc định | Nạp file trước trong subshell rồi chọn database/role thử cho **cả hai owner**. Chạy đúng code block mới với `PSQL` ghi argv: chỉ `db=vink_copy` và `db=vink_identity_copy`; không chạm database thật |
| P3, có từ trước PR: test worker phụ thuộc bộ lọc log của shell | Lần chạy `verify postgres` đầu có `RUST_LOG=warn` từ môi trường Codex: mọi bước trước worker PASS, nhưng không thấy `vink-worker ready` trong 20 s. Chạy riêng worker với cùng filter: tiến trình vẫn sống, SIGTERM thoát 0, không có log INFO | Test lifecycle đặt `RUST_LOG=info,sqlx=warn` cho riêng process worker, nên assertion không phụ thuộc filter của shell; giữ nguyên logging runtime |

Đồng thời sửa các mô tả cũ: consumer của `vink-store-postgres` gồm cả Identity; feature map có
kiểm role của App; bảng kiểm commit 3 gọi đúng ba lane. Không tìm thấy finding P0/P1 hoặc lỗi cấp
principal trái phép. Kiểm thêm client bị hủy giữa body HTTP: probe với socket thật trả body dở
cho thấy timeout đóng socket; không giữ probe trong source vì không xác nhận được lỗi.

Lần kiểm tiếp dùng macOS arm64, Rust 1.99.1, PostgreSQL/psql/pg_dump/pg_restore 17.11, Node
26.12.0 và Chrome 156.0.8078.79. Cluster riêng chỉ nghe `127.0.0.1:60145`, TCP dùng SCRAM; không
dùng database local đang phục vụ; đã dừng và xóa cluster/credential tạm sau khi kiểm. Kết quả và
giới hạn của lần này được ghi theo cây commit 3;
script smoke của #27 chỉ được áp tạm khi chạy web, sau đó khôi phục đúng byte. Không merge hay
cherry-pick #27 vào nhánh này và không chờ CI.

| Lệnh / kiểm tra trên cây cuối | Kết quả | Phạm vi / giới hạn |
| --- | --- | --- |
| `cargo xtask verify quick` | exit 0; 5/5 PASS, 86 unit/contract test | fmt, clippy SSR/WASM, repository links/metadata/boundaries; không database |
| `VINK_TEST_DATABASE_URL=… VINK_PG_BIN=… cargo xtask verify postgres` với shell vẫn có `RUST_LOG=warn` | exit 0; 27/27 PASS, 22 test PostgreSQL | Role thật, hai binary thật, Unix socket, phục hồi/restart, backup/restore; worker vẫn báo `database="current"` và SIGTERM thoát 0 |
| `VINK_TEST_DATABASE_URL=… VINK_CHROME=… cargo xtask verify web`, riêng smoke từ `1a02c0b` áp tạm | exit 0; 7/7 PASS, 330 assertion `ok`; 27 bundle file được quét | Chrome headless, SSR/hydration, CodeMirror/save/CAS/export và restart. Smoke đã khôi phục đúng byte; thay đổi sau lần này chỉ là test worker và tài liệu |
| `git diff --check`; đối chiếu smoke với `HEAD`; kiểm argv của đúng code block runbook | PASS | Không giữ thay đổi của #27; ví dụ runbook chọn đúng hai database thử |

Không còn finding P0–P2 chưa xử lý trong phạm vi review. Không chạy lại Compose/container trong
lần tiếp này; bằng chứng ở các mục trước vẫn là lần chạy riêng. Không chạy CI hay kiểm lại trên
Linux; production authentication vẫn NOT_IMPLEMENTED. Lane web vẫn dùng test harness trên
database tạm; phân quyền runtime/migrator được kiểm bằng lane postgres.
