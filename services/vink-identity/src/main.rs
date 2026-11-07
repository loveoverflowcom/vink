//! `vink-identity [serve|migrate|account …]` — private Identity server, its explicit migration
//! command and operator commands on accounts.
//!
//! `serve` uses the runtime credential and never migrates; `migrate` uses only the migration
//! credential. `account disable|enable <subject>` uses the runtime credential; no contract
//! operation disables an account. Exit codes: 0 success, 1 runtime failure, 2 invalid
//! configuration or usage.

use std::process::ExitCode;

use uuid::Uuid;
use vink_identity::accounts::{self, AccountChange, AccountStore};
use vink_identity::authority::Authority;
use vink_identity::config::{AuthorityChoice, MigrateConfig, OperatorConfig, ServeConfig};
use vink_identity::email_code::Policy;
use vink_identity::http::{IdentityState, router};
use vink_identity::mail::LocalOutbox;
use vink_identity::server::{self, SocketError};
use vink_identity::{persistence, runtime};

const USAGE: &str = "usage: vink-identity [serve|migrate]\n       vink-identity account disable <subject-uuid>\n       vink-identity account enable <subject-uuid>";

/// Operator command on an account, parsed before any configuration is read.
#[derive(Debug, PartialEq, Eq)]
enum AccountCommand {
    Disable(Uuid),
    Enable(Uuid),
}

fn parse_account(args: &[String]) -> Result<AccountCommand, String> {
    let subject = args
        .get(1)
        .filter(|_| args.len() == 2)
        .and_then(|raw| Uuid::try_parse(raw).ok());
    match (args.first().map(String::as_str), subject) {
        (Some("disable"), Some(id)) => Ok(AccountCommand::Disable(id)),
        (Some("enable"), Some(id)) => Ok(AccountCommand::Enable(id)),
        _ => Err("expected `disable <subject-uuid>` or `enable <subject-uuid>`".into()),
    }
}

enum Loaded {
    Serve(ServeConfig),
    Migrate(MigrateConfig),
    Account(OperatorConfig, AccountCommand),
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
        "account" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            match parse_account(&args) {
                Ok(operation) => {
                    OperatorConfig::from_env().map(|config| Loaded::Account(config, operation))
                }
                Err(error) => {
                    eprintln!("vink-identity account: {error}\n{USAGE}");
                    return ExitCode::from(2);
                }
            }
        }
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
        Loaded::Account(config, _) => config.log_format,
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
            Loaded::Account(config, operation) => account(&config, operation).await,
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

async fn account(config: &OperatorConfig, command: AccountCommand) -> Result<(), Failure> {
    let pool = persistence::connect(&config.database_url)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot connect to PostgreSQL: {error}")))?;
    let (subject, result) = match command {
        AccountCommand::Disable(id) => (id, accounts::disable_account(&pool, id).await),
        AccountCommand::Enable(id) => (id, accounts::enable_account(&pool, id).await),
    };
    pool.close().await;
    match result.map_err(|_| Failure::Runtime("account command failed".into()))? {
        AccountChange::Changed { sessions_revoked } => {
            println!("{subject}: changed; {sessions_revoked} session(s) revoked");
        }
        AccountChange::Unchanged => println!("{subject}: already in that state; nothing changed"),
        AccountChange::UnknownAccount => {
            return Err(Failure::Usage(format!("{subject}: no such account")));
        }
    }
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
    let (authority, label) = match &config.authority {
        AuthorityChoice::NotConfigured => (Authority::NotConfigured, "not_configured"),
        AuthorityChoice::EmailCode { outbox } => {
            let outbox = LocalOutbox::open(outbox).map_err(|message| {
                Failure::Usage(format!("VINK_IDENTITY_LOCAL_OUTBOX: {message}"))
            })?;
            tracing::warn!(
                "email-code authority with the local outbox: codes are files on this machine; \
                 this is not production authentication"
            );
            (
                Authority::EmailCode(AccountStore::new(pool.clone(), outbox, Policy::default())),
                "email_code",
            )
        }
    };
    tracing::info!(
        socket = %config.socket.display(),
        caller_uid = ?config.caller_uid,
        authority = label,
        production_authentication = "not_implemented",
        "vink-identity listening"
    );
    let state = IdentityState::new(pool.clone(), authority, config.caller_uid);
    server::serve(listener, bound, router(state), runtime::shutdown_signal()).await;
    pool.close().await;
    tracing::info!("vink-identity stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn account_arguments_are_validated_before_any_configuration() {
        let id = "01890a5d-ac96-774b-bcce-b302099a8057";
        let uuid = Uuid::try_parse(id).unwrap();
        assert_eq!(
            parse_account(&args(&["disable", id])),
            Ok(AccountCommand::Disable(uuid))
        );
        assert_eq!(
            parse_account(&args(&["enable", id])),
            Ok(AccountCommand::Enable(uuid))
        );
        for bad in [
            &[][..],
            &["disable"][..],
            &["disable", "an@example.vn"][..],
            &["disable", id, "extra"][..],
            &["delete", id][..],
        ] {
            assert!(parse_account(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
