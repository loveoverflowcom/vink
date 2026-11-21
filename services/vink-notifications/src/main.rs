//! `vink-notifications [serve|migrate|maintenance purge|maintenance status]` — the private
//! Notification Service, its explicit migration command and operator commands.
//!
//! `serve` uses the runtime credential and never migrates: it binds the ingest socket (App relay)
//! and the inbox socket (App Gateway) and purges expired rows hourly. `migrate` uses only the
//! migration credential. Exit codes: 0 success, 1 runtime failure, 2 invalid configuration or usage.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::watch;
use vink_notifications::config::{MigrateConfig, OperatorConfig, ServeConfig};
use vink_notifications::http::{NotificationState, Scope, router};
use vink_notifications::server::{self, SocketError};
use vink_notifications::{persistence, runtime, store};

const USAGE: &str = "usage: vink-notifications [serve|migrate]\n       vink-notifications maintenance purge\n       vink-notifications maintenance status";

/// How often `serve` purges expired rows.
const PURGE_INTERVAL: Duration = Duration::from_secs(3600);
/// Rows per statement and statements per table and pass, so purge never holds locks long.
const PURGE_BATCH: i64 = 1_000;
const PURGE_MAX_BATCHES: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Maintenance {
    Purge,
    Status,
}

fn parse_maintenance(args: &[String]) -> Result<Maintenance, String> {
    match (args.first().map(String::as_str), args.len()) {
        (Some("purge"), 1) => Ok(Maintenance::Purge),
        (Some("status"), 1) => Ok(Maintenance::Status),
        _ => Err("expected `maintenance purge` or `maintenance status`".into()),
    }
}

enum Loaded {
    Serve(ServeConfig),
    Migrate(MigrateConfig),
    Maintenance(OperatorConfig, Maintenance),
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
        "maintenance" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            match parse_maintenance(&args) {
                Ok(task) => {
                    OperatorConfig::from_env().map(|config| Loaded::Maintenance(config, task))
                }
                Err(error) => {
                    eprintln!("vink-notifications maintenance: {error}\n{USAGE}");
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
            eprintln!("vink-notifications: {error}");
            return ExitCode::from(2);
        }
    };
    runtime::init_tracing(match &loaded {
        Loaded::Serve(config) => config.log_format,
        Loaded::Migrate(config) => config.log_format,
        Loaded::Maintenance(config, _) => config.log_format,
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
            Loaded::Maintenance(config, task) => maintenance(&config, task).await,
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
    tracing::info!(applied = ?report.applied, latest = report.latest, "notification migrations applied");
    Ok(())
}

async fn maintenance(config: &OperatorConfig, task: Maintenance) -> Result<(), Failure> {
    let pool = persistence::connect(&config.database_url)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot connect to PostgreSQL: {error}")))?;
    let now = OffsetDateTime::now_utc();
    let result = match task {
        Maintenance::Purge => store::purge(&pool, now, PURGE_BATCH, PURGE_MAX_BATCHES)
            .await
            .map(|purged| {
                println!(
                    "purged {} group(s), {} receipt(s), {} Thấm mark(s), {} fence(s), {} command answer(s)",
                    purged.groups, purged.receipts, purged.marks, purged.fences, purged.commands
                );
            })
            .map_err(|error| Failure::Runtime(format!("purge failed: {}", store::error_class(&error)))),
        Maintenance::Status => store::status(&pool, now)
            .await
            .map(|status| {
                println!(
                    "inboxes {}; groups {} (open {}, expired awaiting purge {}); members {}; receipts {}; Thấm marks {}; oldest group {}",
                    status.inboxes,
                    status.groups,
                    status.open_groups,
                    status.expired_groups,
                    status.members,
                    status.receipts,
                    status.marks,
                    status
                        .oldest_group_days
                        .map_or("-".into(), |days| format!("{days} day(s)"))
                );
            })
            .map_err(|error| Failure::Runtime(format!("status failed: {}", store::error_class(&error)))),
    };
    pool.close().await;
    result
}

fn bind(
    scope: &str,
    path: &std::path::Path,
) -> Result<(tokio::net::UnixListener, server::BoundSocket), Failure> {
    server::bind(path).map_err(|error| match error {
        SocketError::Unsuitable(message) => Failure::Usage(format!("{scope} socket: {message}")),
        SocketError::Unavailable(message) => Failure::Runtime(format!("{scope} socket: {message}")),
    })
}

async fn serve(config: ServeConfig) -> Result<(), Failure> {
    let pool = persistence::lazy_pool(&config.database_url).map_err(|error| {
        Failure::Usage(format!(
            "VINK_NOTIFICATIONS_DATABASE_URL is not usable: {error}"
        ))
    })?;
    let (ingest_listener, ingest_bound) = bind("ingest", &config.ingest.path)?;
    let (inbox_listener, inbox_bound) = match bind("inbox", &config.inbox.path) {
        Ok(bound) => bound,
        Err(failure) => {
            ingest_bound.remove();
            return Err(failure);
        }
    };
    for (key, scope) in [
        ("VINK_NOTIFICATIONS_RELAY_UID", &config.ingest),
        ("VINK_NOTIFICATIONS_GATEWAY_UID", &config.inbox),
    ] {
        if scope.caller_uid.is_none() {
            tracing::warn!(
                "{key} is not set: every call on that socket is refused and readiness stays not_ready"
            );
        }
    }
    tracing::info!(
        ingest_socket = %config.ingest.path.display(),
        relay_uid = ?config.ingest.caller_uid,
        inbox_socket = %config.inbox.path.display(),
        gateway_uid = ?config.inbox.caller_uid,
        group_max_age_days = vink_notifications::rollup::GROUP_MAX_AGE.whole_days(),
        late_event_cutoff_days = vink_notifications::rollup::LATE_EVENT_CUTOFF.whole_days(),
        max_staleness_seconds = vink_notifications::rollup::MAX_STALENESS_SECONDS,
        admin_notice = "type_only",
        "vink-notifications listening"
    );
    let (stop, stopped) = watch::channel(false);
    let stop = Arc::new(stop);
    let purger = tokio::spawn(purge_periodically(pool.clone(), stopped.clone()));
    let signal = {
        let stop = stop.clone();
        async move {
            runtime::shutdown_signal().await;
            let _ = stop.send(true);
        }
    };
    tokio::spawn(signal);
    let when_stopped = |mut stopped: watch::Receiver<bool>| async move {
        while !*stopped.borrow() {
            if stopped.changed().await.is_err() {
                return;
            }
        }
    };
    let ingest = server::serve(
        ingest_listener,
        ingest_bound,
        router(NotificationState::new(
            pool.clone(),
            Scope::Ingest,
            config.ingest.caller_uid,
        )),
        when_stopped(stopped.clone()),
    );
    let inbox = server::serve(
        inbox_listener,
        inbox_bound,
        router(NotificationState::new(
            pool.clone(),
            Scope::Inbox,
            config.inbox.caller_uid,
        )),
        when_stopped(stopped.clone()),
    );
    tokio::join!(ingest, inbox);
    let _ = stop.send(true);
    let _ = purger.await;
    pool.close().await;
    tracing::info!("vink-notifications stopped");
    Ok(())
}

/// Purges expired rows every [`PURGE_INTERVAL`] (first pass after one interval).
async fn purge_periodically(pool: sqlx::PgPool, mut stopped: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            () = tokio::time::sleep(PURGE_INTERVAL) => {}
            _ = stopped.changed() => return,
        }
        match store::purge(
            &pool,
            OffsetDateTime::now_utc(),
            PURGE_BATCH,
            PURGE_MAX_BATCHES,
        )
        .await
        {
            Ok(purged) => tracing::info!(
                groups = purged.groups,
                receipts = purged.receipts,
                marks = purged.marks,
                fences = purged.fences,
                commands = purged.commands,
                "retention purge"
            ),
            Err(error) => {
                tracing::warn!(error = store::error_class(&error), "retention purge failed")
            }
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
        assert_eq!(parse_maintenance(&args(&["purge"])), Ok(Maintenance::Purge));
        assert_eq!(
            parse_maintenance(&args(&["status"])),
            Ok(Maintenance::Status)
        );
        for bad in [&[][..], &["purge", "now"][..], &["vacuum"][..]] {
            assert!(parse_maintenance(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
