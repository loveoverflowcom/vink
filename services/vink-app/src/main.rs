//! `vink-app [serve|migrate|dev-seed|moderator …]` — HTTP/SSR server and explicit database commands.
//!
//! `serve`, `dev-seed` and `moderator` use the runtime credential (`DATABASE_URL`); `migrate` uses
//! only the migration credential (`VINK_MIGRATE_DATABASE_URL`). `moderator grant|revoke|list` is
//! the operator's only way to give or take comment-moderation authority; no HTTP route does it. Exit codes: 0 success, 1 runtime
//! failure, 2 invalid configuration or usage.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use vink_app::application::Vink;
use vink_app::config::{Config, MigrateConfig, Profile};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::ids::PrincipalId;
use vink_app::http::{AppState, router};
use vink_app::persistence::{self, PgStore, lazy_pool};
use vink_app::{dev_seed, identity, runtime};

const USAGE: &str = "usage: vink-app [serve|migrate|dev-seed]\n       vink-app moderator grant <principal-uuid> <note>\n       vink-app moderator revoke <principal-uuid>\n       vink-app moderator list";

/// Operator command on comment-moderation grants, parsed before any configuration is read.
#[derive(Debug, PartialEq, Eq)]
enum ModeratorCommand {
    Grant(PrincipalId, GrantNote),
    Revoke(PrincipalId),
    List,
}

fn parse_moderator(args: &[String]) -> Result<ModeratorCommand, String> {
    let principal = |raw: Option<&String>| {
        raw.and_then(|raw| PrincipalId::parse(raw))
            .ok_or_else(|| "expected a principal UUID".to_string())
    };
    match args.first().map(String::as_str) {
        Some("grant") => {
            let note = GrantNote::parse(&args.get(2..).unwrap_or_default().join(" "))
                .ok_or("grant needs a one-line note of at most 200 characters")?;
            Ok(ModeratorCommand::Grant(principal(args.get(1))?, note))
        }
        Some("revoke") if args.len() == 2 => Ok(ModeratorCommand::Revoke(principal(args.get(1))?)),
        Some("list") if args.len() == 1 => Ok(ModeratorCommand::List),
        _ => Err("expected `grant <principal> <note>`, `revoke <principal>` or `list`".into()),
    }
}

enum Loaded {
    App(Config),
    Migrate(MigrateConfig),
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if !matches!(
        command.as_str(),
        "serve" | "migrate" | "dev-seed" | "moderator"
    ) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let moderator_command = if command == "moderator" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_moderator(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app moderator: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
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
            Loaded::App(config) if let Some(operation) = moderator_command => {
                moderator(&config, operation).await
            }
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

async fn moderator(config: &Config, command: ModeratorCommand) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app moderator").await?;
    let unavailable = |error: vink_app::application::ports::StoreError| match error {
        vink_app::application::ports::StoreError::UnknownPrincipal => {
            Failure::Usage("the principal has no Vink record".into())
        }
        other => Failure::Runtime(format!("moderator command failed: {other}")),
    };
    match command {
        ModeratorCommand::Grant(principal, note) => {
            match persistence::grant_moderator(&pool, principal, &note)
                .await
                .map_err(unavailable)?
            {
                persistence::GrantOutcome::Granted(grant) => {
                    println!("granted {grant} to {principal}");
                }
                persistence::GrantOutcome::AlreadyActive(grant) => {
                    println!("{principal} already holds active grant {grant}; nothing changed");
                }
            }
        }
        ModeratorCommand::Revoke(principal) => {
            match persistence::revoke_moderator(&pool, principal)
                .await
                .map_err(unavailable)?
            {
                Some(grant) => println!("revoked {grant} from {principal}"),
                None => println!("{principal} holds no active grant; nothing changed"),
            }
        }
        ModeratorCommand::List => {
            for record in persistence::active_moderators(&pool)
                .await
                .map_err(unavailable)?
            {
                println!(
                    "{}\t{}\t{}\t{}",
                    record.principal, record.display_name, record.granted_at, record.note
                );
            }
        }
    }
    pool.close().await;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn moderator_arguments_are_validated_before_any_configuration() {
        let principal = "22222222-2222-4222-8222-222222222222";
        let id = PrincipalId::parse(principal).unwrap();
        assert_eq!(
            parse_moderator(&args(&["grant", principal, "Ban", "biên", "tập"])),
            Ok(ModeratorCommand::Grant(
                id,
                GrantNote::parse("Ban biên tập").unwrap()
            ))
        );
        assert_eq!(
            parse_moderator(&args(&["revoke", principal])),
            Ok(ModeratorCommand::Revoke(id))
        );
        assert_eq!(
            parse_moderator(&args(&["list"])),
            Ok(ModeratorCommand::List)
        );
        for bad in [
            &["grant", principal][..],
            &["grant", "not-a-uuid", "note"],
            &["revoke"],
            &["revoke", principal, "extra"],
            &["list", "extra"],
            &["promote", principal],
            &[],
        ] {
            assert!(parse_moderator(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
