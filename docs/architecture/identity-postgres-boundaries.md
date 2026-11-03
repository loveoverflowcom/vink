# Ranh giới PostgreSQL và Identity

**Ngày:** 02/11/2026 · **Trạng thái:** Người dùng đã yêu cầu tách Identity và PostgreSQL thành
owner riêng; cấu trúc dưới đây là đề xuất đang triển khai trên nhánh
`codex/identity-postgres-boundaries` (một PR, ba commit). Đây **không** phải lựa chọn auth
authority: native password, OIDC hay Kanidm vẫn là quyết định riêng (DiscourseConnect bị loại
03/11/2026 vì Vink tự sở hữu bình luận); D-05 và
S-03 trong [decision log](decisions.md) giữ trạng thái “Cần spike + lựa chọn”.

Base đối chiếu: `develop@5afb00b29c85965dc115718e6ada5a4dcd95399b`. Tham khảo có kiểm chứng:
Cantos `develop@7649017c413bbe72fb77c51abdf4858d253b6ae0` (`services/README.md`,
`packages/store-turso/README.md`) và vot-workspace `develop@f3fea641a5fd226b883c4e32224ad8c97b8323f0`
(`ops/production/compose.yaml`). Chỉ lấy ý tưởng ranh giới; không chép engine, secret hay deployment.

## HOW — trước thay đổi

| Bước | Source tại base | Ghi chú |
| --- | --- | --- |
| Cấu hình | `Config::from_env` đọc một `DATABASE_URL` cho `serve`, `migrate`, `dev-seed` và `vink-worker` | Không có credential riêng cho migration |
| Pool | `persistence::lazy_pool` (10 kết nối, acquire 3s, kết nối lần đầu dùng); `main.rs::connect` cho lệnh một lần | Server khởi động được khi DB down và báo not-ready |
| Migration | `MIGRATOR = sqlx::migrate!("./migrations")`; `vink-app migrate` gọi `MIGRATOR.run` | sqlx khóa advisory, tạo `_sqlx_migrations`, từ chối checksum lệch/version lạ |
| Readiness | `schema_state` so version thành công trong `_sqlx_migrations` với migration nhúng | Không phân biệt checksum lệch hay version mới hơn binary |
| SQL | `PgStore` trong `persistence/postgres.rs`; `classify` đổi SQLSTATE 08/53/57 và lỗi I/O/pool thành `Unavailable`, `works_owner_id_fkey` thành `UnknownPrincipal`, còn lại `Integrity` | Phân loại kết nối là hạ tầng chung; tên constraint là nghiệp vụ |
| Container | `infra/dev/compose.yaml`: `postgres:17.11-alpine`, project `vink-dev`, volume `vink-postgres`, DB `vink`, `127.0.0.1:55432` | `POSTGRES_USER=vink` là **superuser**; app, migration, seed và test đều dùng nó |
| Test/CI | `#[sqlx::test]` và `verify web` tạo database tạm bằng `DATABASE_URL`; CI dùng superuser của service container | Quyền tạo database nằm cùng credential với app |

Lỗi DB/schema trên đường này: DB down → `Unavailable` → API `503 dependency_unavailable`,
readiness `503`; thiếu migration → readiness `pending_migrations`; migration lỗi → `migrate`
thoát 1. Không có lớp nào kiểm role đang chạy có phải superuser hay không.

## WHY — vì sao tách

- **Phạm vi phá hủy.** Runtime đang là superuser: một lỗi SQL hay injection có thể DDL, đọc mọi
  database của instance, tạo role hoặc xóa database. Tách runtime/migrator/operator làm quyền
  DDL và quyền cụm nằm ngoài tiến trình phục vụ request.
- **Identity có độ nhạy khác.** Khi authority được chọn, Identity giữ credential/trạng thái bảo
  mật với quyền truy cập, backup và retention khác tác phẩm. Database riêng với role riêng giúp
  App không đọc được bảng Identity và ngược lại (ACC-01, ACC-02, INV-05), không phụ thuộc vào
  việc mọi query “tự giác” đúng schema.
- **Hạ tầng chung sẽ bị chép.** Service thứ hai cần cùng pool, vòng đời migration và phân loại
  lỗi kết nối. Tách một package nhận registry từ caller (tiền lệ `cantos-store-turso`) tránh chép
  mà vẫn giữ SQL, row mapping và migrations ở service sở hữu.
- **Container là dependency của nhiều service**, không còn là “hạ tầng dev” riêng của App.

**Chi phí vận hành:** năm credential (operator + migrator/runtime cho mỗi owner); thêm bước
provisioning chạy lại được; `migrate` cần credential riêng; hai database phải backup/restore và
không có transaction nguyên tử xuyên hai owner; một instance vẫn là một failure domain và chia
tài nguyên; database cũ cần bước nhận quyền sở hữu object. Local setup thêm hai lệnh (tạo volume,
provisioning).

## Quyết định phạm vi cho PR này

1. **Một instance, hai database theo owner:** `vink` (App, database nghiệp vụ hiện có, giữ
   nguyên dữ liệu và lịch sử migration) và `vink_identity` (Identity; từ commit 2 chỉ có
   migration baseline, chưa có bảng).
   Không cấp quyền chéo, không foreign key xuyên database, không `dblink`/`postgres_fdw`.
2. **Ba loại credential:**

   | Credential | Ví dụ local | Quyền |
   | --- | --- | --- |
   | Bootstrap/operator | `vink` (superuser có sẵn của image) | Provisioning, backup; test harness local. Không cấp cho app |
   | Migrator mỗi owner | `vink_app_migrator`, `vink_identity_migrator` | `CONNECT` DB của mình, `CREATE` trong schema `public`, sở hữu object; không superuser/CREATEDB/CREATEROLE |
   | Runtime mỗi owner | `vink_app_runtime`, `vink_identity_runtime` | `CONNECT` DB của mình, DML trên bảng, chỉ `SELECT` lịch sử migration; không DDL, không TEMP, không DB khác |

   `PUBLIC` mất `CONNECT`/`TEMPORARY` trên hai database owner. Role không là thành viên role nào
   (kể cả `pg_read_all_data`). Provisioning không đổi role có sẵn hoặc password hiện hữu; role có
   thuộc tính vượt mức thì dừng để người vận hành sửa có chủ ý.
3. **Cấu hình App:** `DATABASE_URL` là credential runtime (`serve`, `vink-worker`, `dev-seed`);
   `VINK_MIGRATE_DATABASE_URL` là credential duy nhất `vink-app migrate` đọc, không fallback sang
   `DATABASE_URL` hay superuser. `VINK_TEST_DATABASE_URL` chỉ dành cho `cargo xtask verify` tạo
   database/role tạm; binary không đọc biến này.
4. **Container:** [services/postgres](../../services/postgres/README.md) sở hữu định nghĩa duy
   nhất. File gốc không publish cổng; overlay local publish `127.0.0.1:55432`. Volume là external
   với tên mặc định `vink-dev_vink-postgres` (project `vink-dev` cũ), nên đổi đường dẫn hay `-p`
   không lặng lẽ tạo cluster rỗng. `infra/dev/compose.yaml` được thay bằng hướng dẫn chuyển.
5. **Package:** [packages/store-postgres](../../packages/store-postgres/README.md) (crate
   `vink-store-postgres`) chứa pool, vòng đời migration, trạng thái schema và phân loại lỗi kết
   nối. Caller truyền `Migrator`; package không biết database, role hay bảng nghiệp vụ.
6. **Identity process (commit 2):** [services/vink-identity](../../services/vink-identity/README.md)
   là binary riêng (`serve`/`migrate`), database `vink_identity`, migrations riêng (hiện chỉ có
   baseline, không bảng). Contract riêng tư
   [v1 draft](../../services/vink-identity/contracts/rpc/v1/README.md) do provider sở hữu; tên
   path/field là thiết kế mới. Không authority nên mọi xác minh trả `authority_not_configured`
   (`501`), tách khỏi `rejected` (`200`) và `dependency_unavailable`/`deadline_exceeded` (`503`).
7. **Caller:** chỉ Unix socket, không TCP; caller là OS user của tiến trình kết nối (peer
   credential) so với `VINK_IDENTITY_CALLER_UID`, thiếu cấu hình thì đóng. Chọn theo topology hiện
   tại (cùng host/volume) và tiền lệ Cantos; không có secret để phân phối, không cổng nào có thể vô
   tình publish. Đánh đổi: chỉ cùng host; xác thực theo uid nên mọi tiến trình chạy bằng uid caller
   và tới được socket đều qua; chế độ mạng (mTLS) chưa thiết kế. Consumer (commit 3) nên kiểm cả uid
   của server qua socket. Readiness từ chối role có quyền DDL/superuser.
8. **Mapping subject → principal (đề xuất, chưa triển khai):** App sở hữu một binding
   `(authority, subject_id) → principal_id` với unique trên cặp đầu, tạo ở App database khi luồng
   đăng nhập thật tồn tại. Không dùng email/bút danh làm khóa, không tự nối tài khoản theo email,
   không foreign key xuyên database. Tạo binding là idempotent theo unique: retry sau mất phản hồi
   đọc lại binding hiện có; xung đột unique là “đã có chủ”, không ghi đè. Identity và App là hai
   owner nên không có transaction nguyên tử giữa “authority xác nhận” và “App tạo binding”; kết quả
   chưa rõ được giải bằng đọc lại binding, không bằng tạo mới. Hiện chưa có bảng nào vì chưa có
   luồng tiêu thụ.
9. **Tích hợp App (commit 3):** resolver phía server trả về một trong ba kết quả: principal,
   ẩn danh (`401`), hoặc không xác định (`503 dependency_unavailable`). App chỉ hỏi Identity khi
   request trình `Authorization: Bearer` — **carrier tạm thời**, D-05 có thể thay bằng mô hình
   session. Không credential luôn là `401`, kể cả khi Identity dừng. Không authority, bị từ chối
   hay subject chưa có binding cũng là `401`; Identity dừng, quá hạn, trả lời ngoài contract hay
   socket do user khác phục vụ là `503`. Không bao giờ fallback sang principal mẫu. Đọc công
   khai/SSR không gọi Identity; readiness App báo Identity nhưng không phụ thuộc nó. Local fixture
   chỉ còn ở profile local + loopback và vẫn thắng khi được bật.
10. **Chưa quyết định:** auth authority và authority cho suspension (D-05/S-03); có tách instance
   PostgreSQL cho Identity ở production hay không; cách áp dụng trên PostgreSQL managed không có
   superuser; giao thức mạng cho Identity nếu App và Identity khác host.

## Lát cắt

| Commit | Kết quả | Kiểm chứng chính |
| --- | --- | --- |
| 1 `refactor(postgres)` | `services/postgres` (compose, provisioning, runbook), `vink-store-postgres`, App dùng credential runtime/migrator riêng | `verify quick`, `verify postgres` (provisioning fresh + DB cũ, chạy lại, cô lập role), Compose thật |
| 2 `refactor(identity)` | `services/vink-identity` có binary, config, migrations, contract riêng; không auth giả (production auth NOT_IMPLEMENTED) | Executable thật trên socket thật với role đã provision, contract âm, review độc lập |
| 3 `test(architecture)` | App tiêu thụ contract qua client riêng ở chế độ unavailable; `dev`/`db migrate`/`doctor` biết Identity; runbook backup/restore/rollback; kiểm bí mật; review toàn PR | `doctor`, `verify quick|postgres|web`, restore thử, review đối kháng |

Bằng chứng từng commit nằm ở
[identity-postgres-verification](../development/identity-postgres-verification.md).

## Trạng thái

| Phần | Requested | Proposed | Implemented | Runtime-verified |
| --- | --- | --- | --- | --- |
| Tách PostgreSQL theo owner, role runtime/migrator/operator | Có | — | Có | `verify postgres` (cluster tạm), Compose thật |
| Nhận quyền DB cũ, dữ liệu/lịch sử không đổi | Có | — | Có | `verify postgres` + Compose với binary `5afb00b` |
| `vink-store-postgres` | Có | — | Có | `verify quick`/`postgres` |
| Identity process, socket, caller = peer uid | Có | Cơ chế caller | Có | `verify postgres` (binary thật) |
| Contract `vink.identity.v1` | Có | Tên path/field (draft) | Có | Unit + binary thật |
| App ↔ Identity, carrier Bearer | Có | Carrier tạm thời | Có (chế độ unavailable) | `verify postgres` (hai binary thật), unit với Identity giả |
| Mapping subject → principal | — | Thiết kế ở mục 8 | Không | — |
| Auth authority, đăng nhập, session | — | D-05/S-03 mở | Không (NOT_IMPLEMENTED) | — |
| Backup/restore từng owner | Có | — | Runbook + bước lane | `verify postgres` (cùng cluster) |

## Giới hạn đã biết

- `postgres` và `template1` vẫn cho `PUBLIC` kết nối theo mặc định của PostgreSQL; chúng không
  chứa dữ liệu Vink. Provisioning chỉ báo, không đổi database ngoài owner.
- Object ngoài schema `public` (ví dụ `_sqlx_test` do test harness cũ tạo) không được nhận quyền
  sở hữu hay cấp quyền; runtime không có `USAGE` trên chúng.
- `CREATE ROLE ... PASSWORD` gửi chuỗi tới server; production nên truyền SCRAM verifier tạo
  sẵn và không bật `log_statement` cho DDL khi provisioning.
- Healthcheck container chỉ chứng minh server nhận kết nối, không chứng minh schema hay quyền.
