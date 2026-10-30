//! `vink-worker` — background process entrypoint.
//!
//! Scope today: configuration, logging, a database/schema probe and graceful shutdown. No job
//! kind is registered: moderation, outbox delivery and Discourse reconciliation are not
//! implemented, and the worker never reports them as running.

use std::process::ExitCode;
use std::time::Duration;

use vink_app::config::Config;
use vink_app::persistence::{SchemaState, lazy_pool, schema_state};
use vink_app::runtime;

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
        Ok(Ok(SchemaState::Current)) => "current",
        Ok(Ok(SchemaState::Pending)) => "pending_migrations",
        Ok(Err(_)) | Err(_) => "unavailable",
    };
    tracing::info!(
        database,
        jobs = "none",
        ai_moderation = "not_implemented",
        discourse = "not_implemented",
        "vink-worker ready"
    );
    runtime::shutdown_signal().await;
    pool.close().await;
    tracing::info!("vink-worker stopped");
    ExitCode::SUCCESS
}
