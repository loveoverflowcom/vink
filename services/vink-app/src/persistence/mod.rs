//! PostgreSQL adapter: the only module that issues SQL.

mod postgres;

use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

pub use postgres::PgStore;

/// Versioned migrations, embedded at build time from `services/vink-app/migrations`.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// A pool that connects on first use, so the process starts (and reports not-ready) while the
/// database is down instead of failing to boot.
pub fn lazy_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let options: PgConnectOptions = database_url.parse()?;
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .connect_lazy_with(options.application_name("vink-app")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaState {
    /// Every embedded migration is applied.
    Current,
    /// The database is reachable but some embedded migration is missing or failed.
    Pending,
}

/// Compares applied migrations with the ones embedded in this binary.
pub async fn schema_state(pool: &PgPool) -> Result<SchemaState, sqlx::Error> {
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    if !exists {
        return Ok(SchemaState::Pending);
    }
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(pool)
            .await?;
    let current = MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .all(|m| applied.binary_search(&m.version).is_ok());
    Ok(if current {
        SchemaState::Current
    } else {
        SchemaState::Pending
    })
}
