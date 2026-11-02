//! Identity's own database: migrations embedded from `services/vink-identity/migrations`, pools
//! and the migration lifecycle from `vink-store-postgres`. Identity never opens the App database.

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPool;
use vink_store_postgres::PoolSettings;

pub use vink_store_postgres::{MigrationError, MigrationReport, SchemaState, role_is_privileged};

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Connects on first use, so the server starts and reports not-ready while the database is down.
pub fn lazy_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    vink_store_postgres::lazy_pool(database_url, PoolSettings::serving("vink-identity"))
}

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    vink_store_postgres::connect(database_url, PoolSettings::command("vink-identity migrate")).await
}

/// Read-only comparison of the applied history with [`MIGRATOR`].
pub async fn schema_state(pool: &PgPool) -> Result<SchemaState, sqlx::Error> {
    vink_store_postgres::schema_state(pool, &MIGRATOR).await
}

/// Applies [`MIGRATOR`] with the Identity migration credential behind `pool`.
pub async fn migrate(pool: &PgPool) -> Result<MigrationReport, MigrationError> {
    vink_store_postgres::migrate(pool, &MIGRATOR).await
}
