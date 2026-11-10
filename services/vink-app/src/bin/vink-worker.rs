//! `vink-worker` — background process entrypoint.
//!
//! Scope today: configuration, logging, a database/schema probe, graceful shutdown and one
//! housekeeping job: an hourly prune of expired sign-in rate-limit windows (D-05). Moderation
//! jobs are not implemented and the worker never reports them as running. Identity's mail
//! outbox is delivered by `vink-identity serve`, never here: the App does not read the Identity
//! database. Discussion is Vink-owned; there is no external forum to reconcile.

use std::process::ExitCode;
use std::time::Duration;

use vink_app::application::Vink;
use vink_app::config::Config;
use vink_app::persistence::{PgStore, lazy_pool, schema_state};
use vink_app::runtime;

const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);

fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("vink-worker: {error}");
            return ExitCode::from(2);
        }
    };
    runtime::init_tracing(config.log_format);
    let tokio = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            tracing::error!(%error, "cannot start async runtime");
            return ExitCode::FAILURE;
        }
    };
    tokio.block_on(run(config))
}

async fn run(config: Config) -> ExitCode {
    let pool = match lazy_pool(&config.database_url) {
        Ok(pool) => pool,
        Err(error) => {
            tracing::error!(%error, "DATABASE_URL is not usable");
            return ExitCode::from(2);
        }
    };
    let database = match tokio::time::timeout(Duration::from_secs(5), schema_state(&pool)).await {
        Ok(Ok(state)) => state.label(),
        Ok(Err(_)) | Err(_) => "unavailable",
    };
    tracing::info!(
        database,
        jobs = "rate_limit_prune",
        ai_moderation = "not_implemented",
        "vink-worker ready"
    );
    let vink = Vink::new(PgStore::new(pool.clone()));
    let shutdown = runtime::shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            () = tokio::time::sleep(PRUNE_INTERVAL) => {
                match vink.prune_rate_limits().await {
                    Ok(rows) => tracing::info!(rows, "rate-limit windows pruned"),
                    Err(_) => tracing::warn!("rate-limit prune could not reach the database"),
                }
            }
        }
    }
    pool.close().await;
    tracing::info!("vink-worker stopped");
    ExitCode::SUCCESS
}
