//! PostgreSQL adapter: the only module that issues SQL.
//!
//! Pools and the migration lifecycle come from `vink-store-postgres`; the App's schema, SQL,
//! row mapping and constraint classification stay here. These wrappers bind that lifecycle to
//! the App's own migrations, so no other registry reaches the App database.

mod abuse;
mod discovery;
mod discussion;
mod endorsement;
pub mod evaluation;
pub mod moderation;
pub(crate) mod moderation_write;
pub mod outbox;
pub mod post_review;
pub(crate) mod postgres;
mod principal;
pub(crate) mod publication;
mod taxonomy;

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPool;
use vink_store_postgres::PoolSettings;

pub use discovery::DISCOVERY_SEARCH_SQL;
pub use discussion::{
    GrantOutcome, ModeratorRecord, active_moderators, grant_moderator, revoke_moderator,
};
pub use postgres::PgStore;
pub use principal::{
    SuspensionOutcome, SuspensionRecord, active_suspensions, reinstate_principal, suspend_principal,
};
pub use taxonomy::{SnapshotCapture, capture_metadata_snapshot, change_catalog};
pub use vink_store_postgres::{MigrationError, MigrationReport, SchemaState, role_is_privileged};

/// Versioned migrations, embedded at build time from `services/vink-app/migrations`.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// A pool that connects on first use, so the process starts (and reports not-ready) while the
/// database is down instead of failing to boot.
pub fn lazy_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    vink_store_postgres::lazy_pool(database_url, PoolSettings::serving("vink-app"))
}

/// Connects now, for one-shot commands (`migrate`, `dev-seed`).
pub async fn connect(database_url: &str, command: &'static str) -> Result<PgPool, sqlx::Error> {
    vink_store_postgres::connect(database_url, PoolSettings::command(command)).await
}

/// Compares the applied history with [`MIGRATOR`]; read-only, safe for the runtime role.
pub async fn schema_state(pool: &PgPool) -> Result<SchemaState, sqlx::Error> {
    vink_store_postgres::schema_state(pool, &MIGRATOR).await
}

/// Applies [`MIGRATOR`] with the migration credential behind `pool`.
pub async fn migrate(pool: &PgPool) -> Result<MigrationReport, MigrationError> {
    vink_store_postgres::migrate(pool, &MIGRATOR).await
}
