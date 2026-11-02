//! `vink-app [serve|migrate|dev-seed]` — HTTP/SSR server and explicit database commands.
//!
//! `serve` and `dev-seed` use the runtime credential (`DATABASE_URL`); `migrate` uses only the
//! migration credential (`VINK_MIGRATE_DATABASE_URL`). Exit codes: 0 success, 1 runtime
//! failure, 2 invalid configuration or usage.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use vink_app::application::Vink;
use vink_app::config::{Config, MigrateConfig, Profile};
use vink_app::http::{AppState, router};
use vink_app::persistence::{self, PgStore, lazy_pool};
use vink_app::{dev_seed, identity, runtime};

const USAGE: &str = "usage: vink-app [serve|migrate|dev-seed]";

enum Loaded {
    App(Config),
    Migrate(MigrateConfig),
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if !matches!(command.as_str(), "serve" | "migrate" | "dev-seed") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let loaded = match command.as_str() {
        "migrate" => MigrateConfig::from_env().map(Loaded::Migrate),
        _ => Config::from_env().map(Loaded::App),
    };
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("vink-app: {error}");
            return ExitCode::from(2);
        }
    };
    runtime::init_tracing(match &loaded {
        Loaded::App(config) => config.log_format,
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
            Loaded::Migrate(config) => migrate(&config).await,
            Loaded::App(config) if command == "dev-seed" => seed(&config).await,
            Loaded::App(config) => serve(config).await,
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

enum Failure {
    Usage(String),
    Runtime(String),
}

async fn connect(database_url: &str, command: &'static str) -> Result<sqlx::PgPool, Failure> {
    persistence::connect(database_url, command)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot connect to PostgreSQL: {error}")))
}

async fn migrate(config: &MigrateConfig) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app migrate").await?;
    let report = persistence::migrate(&pool)
        .await
        .map_err(|error| Failure::Runtime(format!("migration failed: {error}")))?;
    pool.close().await;
    tracing::info!(applied = ?report.applied, latest = report.latest, "migrations applied");
    Ok(())
}

async fn seed(config: &Config) -> Result<(), Failure> {
    if config.profile != Profile::Local {
        return Err(Failure::Usage(
            "dev-seed writes synthetic data and requires VINK_PROFILE=local".into(),
        ));
    }
    let pool = connect(&config.database_url, "vink-app dev-seed").await?;
    dev_seed::apply(&pool)
        .await
        .map_err(|error| Failure::Runtime(format!("seed failed (run migrate first?): {error}")))?;
    tracing::info!(
        work = %dev_seed::SAMPLE_WORK,
        author = %dev_seed::SAMPLE_AUTHOR,
        "development fixture present"
    );
    Ok(())
}

async fn serve(config: Config) -> Result<(), Failure> {
    let pool = lazy_pool(&config.database_url)
        .map_err(|error| Failure::Usage(format!("DATABASE_URL is not usable: {error}")))?;
    let resolver = identity::from_config(&config);
    if config.dev_principal.is_some() {
        tracing::warn!(
            "local identity fixture enabled: loopback requests act as VINK_DEV_PRINCIPAL; this is not authentication"
        );
        if config.identity.is_some() {
            tracing::warn!(
                "the local fixture takes precedence; Identity is only probed by readiness"
            );
        }
    }
    let probe = config.identity.as_ref().map(|identity| {
        identity::IdentityClient::new(identity.socket.clone(), identity.server_uid)
    });
    let state = AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        Arc::from(resolver),
        &config.public_origin,
        &config.site_root,
    )
    .with_identity_probe(probe)
    .with_restricted_role_required(config.profile == Profile::Production);
    if !config.site_root.join("pkg").join("vink.js").exists() {
        tracing::warn!(
            site_root = %config.site_root.display(),
            "browser bundle missing; pages render but will not hydrate (run `cargo xtask build`)"
        );
    }
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot bind {}: {error}", config.bind)))?;
    tracing::info!(
        bind = %config.bind,
        origin = %config.public_origin,
        profile = ?config.profile,
        "vink-app listening"
    );
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(runtime::shutdown_signal())
    .await
    .map_err(|error| Failure::Runtime(format!("server error: {error}")))?;
    pool.close().await;
    tracing::info!("vink-app stopped");
    Ok(())
}
