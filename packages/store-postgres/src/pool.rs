//! Connection pools for serving processes and one-shot commands.

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

/// How a process uses its database. `application_name` appears in `pg_stat_activity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSettings {
    pub application_name: &'static str,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

impl PoolSettings {
    /// A serving process (HTTP server, worker): a request fails quickly while the database is down.
    pub const fn serving(application_name: &'static str) -> Self {
        Self {
            application_name,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(3),
        }
    }

    /// A one-shot command (migrate, seed): few connections, more patience at start.
    pub const fn command(application_name: &'static str) -> Self {
        Self {
            application_name,
            max_connections: 2,
            acquire_timeout: Duration::from_secs(10),
        }
    }
}

fn split(
    url: &str,
    settings: PoolSettings,
) -> Result<(PgPoolOptions, PgConnectOptions), sqlx::Error> {
    let connect: PgConnectOptions = url.parse()?;
    let pool = PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(settings.acquire_timeout);
    Ok((pool, connect.application_name(settings.application_name)))
}

/// A pool that connects on first use, so a server starts (and reports not-ready) while the
/// database is down instead of failing to boot. Only a malformed URL fails here.
pub fn lazy_pool(url: &str, settings: PoolSettings) -> Result<PgPool, sqlx::Error> {
    let (pool, connect) = split(url, settings)?;
    Ok(pool.connect_lazy_with(connect))
}

/// Connects now, for commands that can do nothing useful without the database.
pub async fn connect(url: &str, settings: PoolSettings) -> Result<PgPool, sqlx::Error> {
    let (pool, connect) = split(url, settings)?;
    pool.connect_with(connect).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::is_unavailable;

    #[tokio::test]
    async fn lazy_pool_boots_without_a_database_and_reports_it_unavailable() {
        let settings = PoolSettings {
            acquire_timeout: Duration::from_millis(300),
            ..PoolSettings::serving("vink-store-postgres-test")
        };
        // Nothing listens on port 1.
        let pool = lazy_pool("postgres://nobody:unused@127.0.0.1:1/none", settings).unwrap();
        let error = sqlx::query("SELECT 1").execute(&pool).await.unwrap_err();
        assert!(is_unavailable(&error), "{error:?}");
        assert!(lazy_pool("postgres://nobody@127.0.0.1:port/none", settings).is_err());
    }
}
