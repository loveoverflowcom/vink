# vink-store-postgres

Hạ tầng PostgreSQL dùng chung cho các service Vink. Consumer truyền URL kết nối, tên ứng dụng và
`Migrator` nhúng của chính nó; crate không biết database, role, schema hay luật nghiệp vụ nào.
SQL, row mapping, port và migrations ở lại service sở hữu. Chiều phụ thuộc là
`services/* → vink-store-postgres`; `verify quick` từ chối package phụ thuộc service, web hay
framework HTTP. Lý do tách: [ranh giới PostgreSQL và Identity](../../docs/architecture/identity-postgres-boundaries.md).

Consumer hiện tại: [vink-app](../../services/vink-app/src/persistence/mod.rs) và
[vink-identity](../../services/vink-identity/src/persistence.rs). Mỗi wrapper gắn vòng đời với
`MIGRATOR` của owner để registry khác không chạm database của mình.

```rust
use sqlx::migrate::Migrator;
use vink_store_postgres::{PoolSettings, SchemaState};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn ready(runtime_url: &str) -> Result<bool, sqlx::Error> {
    let pool = vink_store_postgres::lazy_pool(runtime_url, PoolSettings::serving("ledger"))?;
    Ok(vink_store_postgres::schema_state(&pool, &MIGRATOR).await? == SchemaState::Current)
}
```

| API | Bảo đảm | Bằng chứng |
| --- | --- | --- |
| `lazy_pool`, `connect`, `PoolSettings` | Pool kết nối khi dùng lần đầu (server khởi động khi DB down) hoặc ngay (lệnh một lần); `application_name` theo process | unit test pool tới cổng không có server |
| `schema_state` | Chỉ đọc. `Current` khi mọi migration nhúng đã áp dụng cùng checksum và không có gì khác; `Pending`; `Diverged` khi có migration dở dang, checksum khác hoặc version binary không biết | unit test so sánh thuần + `tests/postgres.rs` |
| `migrate` | Từ chối lịch sử diverged trước khi chạy; thiếu quyền là `Privilege`, không thử lại bằng role khác; sau khi chạy chỉ owner được ghi `_sqlx_migrations`; kiểm lại `Current` | `tests/postgres.rs`, provisioning trong `verify postgres` |
| `role_is_privileged` | Chỉ đọc. Role đang kết nối có superuser/CREATEDB/CREATEROLE/REPLICATION/BYPASSRLS hoặc `CREATE` trên database/schema `public` không — readiness dùng để từ chối chạy bằng credential migrator/operator | `tests/postgres.rs` (superuser), role tạm trong `verify postgres` |
| `is_unavailable` | I/O, TLS, pool, SQLSTATE 08/53/57 là không khả dụng; còn lại để adapter phân loại | unit test + phiên bị `pg_terminate_backend` |

Bảng lịch sử là `public._sqlx_migrations`, quy ước của sqlx. Đổi tên sẽ làm mất liên kết với
lịch sử đã áp dụng, nên đây là hằng số hạ tầng chứ không phải danh tính service.

## Không thuộc crate này

Bảng, query, DTO hay lỗi nghiệp vụ (Work, Principal, Session, credential); tên database/role;
provisioning (thuộc [services/postgres](../../services/postgres/README.md)); ORM, cache hay pool
toàn cục. Thêm API chỉ khi ít nhất một service thật dùng nó.

## Kiểm chứng

`cargo xtask verify quick` chạy unit test; `cargo xtask verify postgres` chạy
`tests/postgres.rs` trên PostgreSQL thật với registry test riêng (`tests/migrations`).
