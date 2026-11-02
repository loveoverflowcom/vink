//! Shared PostgreSQL infrastructure for Vink services.
//!
//! A consumer supplies its connection URL, an application name and its own embedded
//! [`Migrator`](sqlx::migrate::Migrator). This crate knows no database, role, schema or business
//! rule: SQL, row mapping, ports and migrations stay with the owning service, and dependencies
//! point from `services/*` to this package, never back.

mod error;
mod migration;
mod pool;

pub use error::is_unavailable;
pub use migration::{
    Divergence, HISTORY_TABLE, MigrationError, MigrationReport, SchemaState, migrate, schema_state,
};
pub use pool::{PoolSettings, connect, lazy_pool};
