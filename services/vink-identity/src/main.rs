//! `vink-identity [serve|migrate]` — private Identity server and its explicit migration command.
//!
//! `serve` uses the runtime credential and never migrates; `migrate` uses only the migration
//! credential. Exit codes: 0 success, 1 runtime failure, 2 invalid configuration or usage.

use std::process::ExitCode;

use vink_identity::authority::Authority;
use vink_identity::config::{MigrateConfig, ServeConfig};
use vink_identity::http::{IdentityState, router};
use vink_identity::server::{self, SocketError};
use vink_identity::{persistence, runtime};

const USAGE: &str = "usage: vink-identity [serve|migrate]";

enum Loaded {
    Serve(ServeConfig),
    Migrate(MigrateConfig),
}

enum Failure {
    Usage(String),
    Runtime(String),
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    let loaded = match command.as_str() {
        "serve" => ServeConfig::from_env().map(Loaded::Serve),
        "migrate" => MigrateConfig::from_env().map(Loaded::Migrate),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("vink-identity: {error}");
            return ExitCode::from(2);
        }
    };
    runtime::init_tracing(match &loaded {
        Loaded::Serve(config) => config.log_format,
        Loaded::Migrate(config) => config.log_format,
    });
    let tokio = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(error) => {
            tracing::error!(%error, "cannot start async runtime");
            return ExitCode::FAILURE;
        }
    };
    let result = tokio.block_on(async {
        match loaded {
            Loaded::Serve(config) => serve(config).await,
            Loaded::Migrate(config) => migrate(&config).await,
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            tracing::error!("{message}");
            ExitCode::from(2)
        }
        Err(Failure::Runtime(message)) => {
            tracing::error!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn migrate(config: &MigrateConfig) -> Result<(), Failure> {
    let pool = persistence::connect(&config.database_url)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot connect to PostgreSQL: {error}")))?;
    let report = persistence::migrate(&pool)
        .await
        .map_err(|error| Failure::Runtime(format!("migration failed: {error}")))?;
    pool.close().await;
    tracing::info!(applied = ?report.applied, latest = report.latest, "identity migrations applied");
    Ok(())
}

async fn serve(config: ServeConfig) -> Result<(), Failure> {
    let pool = persistence::lazy_pool(&config.database_url).map_err(|error| {
        Failure::Usage(format!("VINK_IDENTITY_DATABASE_URL is not usable: {error}"))
    })?;
    let (listener, bound) = server::bind(&config.socket).map_err(|error| match error {
        SocketError::Unsuitable(message) => {
            Failure::Usage(format!("VINK_IDENTITY_SOCKET: {message}"))
        }
        SocketError::Unavailable(message) => Failure::Runtime(message),
    })?;
    if config.caller_uid.is_none() {
        tracing::warn!(
            "VINK_IDENTITY_CALLER_UID is not set: every contract call is refused and readiness stays not_ready"
        );
    }
    tracing::info!(
        socket = %config.socket.display(),
        caller_uid = ?config.caller_uid,
        authority = "not_configured",
        production_authentication = "not_implemented",
        "vink-identity listening"
    );
    let state = IdentityState::new(pool.clone(), Authority::NotConfigured, config.caller_uid);
    server::serve(listener, router(state), runtime::shutdown_signal()).await;
    bound.remove();
    pool.close().await;
    tracing::info!("vink-identity stopped");
    Ok(())
}
