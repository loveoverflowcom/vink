//! `vink-identity [serve|migrate|account …|maintenance prune|mail status]` — private Identity
//! server, its explicit migration command and operator commands.
//!
//! `serve` uses the runtime credential and never migrates; with the email-code authority it also
//! runs the mail delivery worker and an hourly retention prune. `migrate` uses only the migration
//! credential. Operator commands use the runtime credential; no contract operation disables an
//! account. Exit codes: 0 success, 1 runtime failure, 2 invalid configuration or usage.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, watch};
use uuid::Uuid;
use vink_identity::accounts::{self, AccountChange, AccountStore, Retention};
use vink_identity::authority::Authority;
use vink_identity::config::{
    AuthorityChoice, MailChoice, MailConfig, MigrateConfig, OperatorConfig, ServeConfig,
};
use vink_identity::delivery::{self, DeliveryPolicy, Worker};
use vink_identity::email_code::{Code, EmailAddress, Policy};
use vink_identity::http::{IdentityState, router};
use vink_identity::mail::{LocalOutbox, Message, Purpose, Smtp, Transport};
use vink_identity::server::{self, SocketError};
use vink_identity::{persistence, runtime};

const USAGE: &str = "usage: vink-identity [serve|migrate]\n       vink-identity account disable <subject-uuid>\n       vink-identity account enable <subject-uuid>\n       vink-identity maintenance prune\n       vink-identity mail status\n       vink-identity mail test <address>";

/// How often `serve` prunes closed records.
const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);

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

/// Operator commands that take no argument.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Maintenance {
    Prune,
    MailStatus,
    /// Sends one transport-check message (no code) through the configured sender.
    MailTest(EmailAddress),
}

fn parse_maintenance(command: &str, args: &[String]) -> Result<Maintenance, String> {
    match (command, args.first().map(String::as_str), args.len()) {
        ("maintenance", Some("prune"), 1) => Ok(Maintenance::Prune),
        ("mail", Some("status"), 1) => Ok(Maintenance::MailStatus),
        ("mail", Some("test"), 2) => EmailAddress::parse(&args[1])
            .map(Maintenance::MailTest)
            .ok_or_else(|| "`mail test` needs a mailbox address".into()),
        ("maintenance", ..) => Err("expected `maintenance prune`".into()),
        _ => Err("expected `mail status` or `mail test <address>`".into()),
    }
}

enum Loaded {
    Serve(ServeConfig),
    Migrate(MigrateConfig),
    Account(OperatorConfig, AccountCommand),
    Maintenance(OperatorConfig, Maintenance),
    MailTest(MailConfig, EmailAddress),
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
        "maintenance" | "mail" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            match parse_maintenance(&command, &args) {
                Ok(Maintenance::MailTest(to)) => {
                    MailConfig::from_env().map(|config| Loaded::MailTest(config, to))
                }
                Ok(task) => {
                    OperatorConfig::from_env().map(|config| Loaded::Maintenance(config, task))
                }
                Err(error) => {
                    eprintln!("vink-identity {command}: {error}\n{USAGE}");
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
        Loaded::Account(config, _) | Loaded::Maintenance(config, _) => config.log_format,
        Loaded::MailTest(config, _) => config.log_format,
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
            Loaded::Maintenance(config, task) => maintenance(&config, task).await,
            Loaded::MailTest(config, to) => mail_test(&config, to).await,
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

async fn maintenance(config: &OperatorConfig, task: Maintenance) -> Result<(), Failure> {
    let pool = persistence::connect(&config.database_url)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot connect to PostgreSQL: {error}")))?;
    let result = match task {
        Maintenance::Prune => accounts::prune(&pool, Retention::default())
            .await
            .map(|pruned| {
                println!(
                    "pruned {} expired challenge(s) with their deliveries and {} closed session(s)",
                    pruned.challenges, pruned.sessions
                );
            })
            .map_err(|_| Failure::Runtime("prune failed".into())),
        Maintenance::MailStatus => delivery::backlog(&pool)
            .await
            .map(|queue| {
                println!(
                    "pending {}; oldest pending {}; failed in the last hour {}",
                    queue.pending,
                    queue
                        .oldest_pending_seconds
                        .map_or("-".into(), |s| format!("{s} s")),
                    queue.failed_last_hour
                );
            })
            .map_err(|_| Failure::Runtime("cannot read the mail queue".into())),
        Maintenance::MailTest(_) => Err(Failure::Usage("mail test takes no database".into())),
    };
    pool.close().await;
    result
}

/// `vink-identity mail test <address>`: one transport-check message through exactly the sender
/// `serve` would use, outside the outbox. Proves egress, DNS, TLS, credential and sender domain;
/// the relay accepting it is not proof that it reached the inbox.
async fn mail_test(config: &MailConfig, to: EmailAddress) -> Result<(), Failure> {
    let sender = transport(&config.mail)?;
    let message = Message {
        delivery: Uuid::now_v7(),
        to,
        code: Code::parse("000000").expect("six digits"),
        purpose: Purpose::TransportCheck,
        expires_at: time::OffsetDateTime::now_utc(),
    };
    match sender.send(&message).await {
        Ok(()) => {
            println!(
                "accepted by {} (message {}); check the mailbox — acceptance is not delivery",
                sender.label(),
                message.delivery.simple()
            );
            Ok(())
        }
        Err(failure) => Err(Failure::Runtime(format!(
            "mail test failed: {}{}",
            failure.label(),
            failure
                .status()
                .map_or(String::new(), |status| format!(" (SMTP {status})"))
        ))),
    }
}

/// Builds the transport the worker uses. Only what needs no network is checked here.
fn transport(mail: &MailChoice) -> Result<Transport, Failure> {
    match mail {
        MailChoice::LocalOutbox(dir) => {
            let outbox = LocalOutbox::open(dir).map_err(|message| {
                Failure::Usage(format!("VINK_IDENTITY_LOCAL_OUTBOX: {message}"))
            })?;
            tracing::warn!(
                "email-code authority with the local outbox: codes are files on this machine; \
                 this is not production mail"
            );
            Ok(Arc::new(outbox))
        }
        MailChoice::Smtp(settings) => {
            let smtp = Smtp::new(settings.clone())
                .map_err(|message| Failure::Usage(format!("VINK_IDENTITY_SMTP_*: {message}")))?;
            tracing::info!(
                host = %settings.host,
                port = settings.port,
                security = settings.security.label(),
                authenticated = settings.username.is_some(),
                "email-code authority mails through SMTP"
            );
            Ok(Arc::new(smtp))
        }
    }
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
    let (stop, stopped) = watch::channel(false);
    let stop = Arc::new(stop);
    let mut background = tokio::task::JoinSet::new();
    let (authority, label, mail) = match &config.authority {
        AuthorityChoice::NotConfigured => (Authority::NotConfigured, "not_configured", None),
        AuthorityChoice::Password => {
            let wake = Arc::new(Notify::new());
            background.spawn(prune_periodically(pool.clone(), stopped.clone()));
            (
                Authority::new_password(AccountStore::new(pool.clone(), Policy::default(), wake)),
                "password",
                None,
            )
        }
        AuthorityChoice::EmailCode {
            mail,
            global_challenge_limit,
        } => {
            let transport = transport(mail)?;
            let mut policy = Policy::default();
            if let Some(limit) = global_challenge_limit {
                policy.max_challenges_globally = *limit;
            }
            let wake = Arc::new(Notify::new());
            let worker = Worker::new(
                pool.clone(),
                transport,
                DeliveryPolicy::default(),
                wake.clone(),
            );
            background.spawn(worker.run(stopped.clone()));
            background.spawn(prune_periodically(pool.clone(), stopped.clone()));
            (
                Authority::new_email_code(AccountStore::new(pool.clone(), policy, wake)),
                "email_code",
                Some(mail.label()),
            )
        }
        AuthorityChoice::Native {
            mail,
            global_challenge_limit,
        } => {
            let transport = transport(mail)?;
            let mut policy = Policy::default();
            if let Some(limit) = global_challenge_limit {
                policy.max_challenges_globally = *limit;
            }
            let wake = Arc::new(Notify::new());
            let worker = Worker::new(
                pool.clone(),
                transport,
                DeliveryPolicy::default(),
                wake.clone(),
            );
            background.spawn(worker.run(stopped.clone()));
            background.spawn(prune_periodically(pool.clone(), stopped.clone()));
            (
                Authority::new_native(
                    AccountStore::new(pool.clone(), policy, wake),
                    vink_identity::authority::MethodCapabilities {
                        email_code: true,
                        password: true,
                    },
                ),
                "native",
                Some(mail.label()),
            )
        }
    };
    tracing::info!(
        socket = %config.socket.display(),
        caller_uid = ?config.caller_uid,
        authority = label,
        mail_transport = mail.unwrap_or("not_configured"),
        production_authentication = match mail {
            Some("smtp") => "not_accepted",
            _ => "not_implemented",
        },
        "vink-identity listening"
    );
    let mut state = IdentityState::new(pool.clone(), authority, config.caller_uid);
    if let Some(label) = mail {
        state = state.with_mail_transport(label);
    }
    let signal = {
        let stop = stop.clone();
        async move {
            runtime::shutdown_signal().await;
            let _ = stop.send(true);
        }
    };
    server::serve(listener, bound, router(state), signal).await;
    let _ = stop.send(true);
    // The worker finishes the batch in flight (each send is bounded); anything unfinished is
    // retried after its lease by the next process.
    let drained = tokio::time::timeout(DeliveryPolicy::default().lease, async {
        while background.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        tracing::warn!(
            "background tasks did not stop in time; unfinished deliveries will be retried"
        );
    }
    pool.close().await;
    tracing::info!("vink-identity stopped");
    Ok(())
}

/// Deletes closed records every [`PRUNE_INTERVAL`] (first run after one interval).
async fn prune_periodically(pool: sqlx::PgPool, mut stopped: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            () = tokio::time::sleep(PRUNE_INTERVAL) => {}
            _ = stopped.changed() => return,
        }
        match accounts::prune(&pool, Retention::default()).await {
            Ok(pruned) => tracing::info!(
                challenges = pruned.challenges,
                sessions = pruned.sessions,
                "retention prune"
            ),
            Err(_) => tracing::warn!("retention prune could not reach the database"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn maintenance_arguments_are_exact() {
        assert_eq!(
            parse_maintenance("maintenance", &args(&["prune"])),
            Ok(Maintenance::Prune)
        );
        assert_eq!(
            parse_maintenance("mail", &args(&["status"])),
            Ok(Maintenance::MailStatus)
        );
        for (command, bad) in [
            ("maintenance", &[][..]),
            ("maintenance", &["prune", "now"][..]),
            ("maintenance", &["vacuum"][..]),
            ("mail", &["retry"][..]),
            ("mail", &["test"][..]),
            ("mail", &["test", "not-an-address"][..]),
        ] {
            assert!(parse_maintenance(command, &args(bad)).is_err(), "{bad:?}");
        }
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
