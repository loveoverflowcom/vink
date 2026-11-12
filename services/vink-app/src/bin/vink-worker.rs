//! `vink-worker` — background process entrypoint.
//!
//! Jobs: the discussion outbox (DISC-04, #120; `vink_app::outbox_worker`), with separate consumers
//! for shadow admission observations and explicit operator post-review (never approval), and
//! an hourly housekeeping pass: expired sign-in rate-limit windows (D-05) and discussion events
//! whose deliveries all finished more than [`EVENT_RETENTION_DAYS`] days ago and owe no review
//! finalization. Admission post-review has no producer; no AI provider is configured.
//! Identity's mail outbox is delivered by `vink-identity serve`, never here: the App does not read
//! the Identity database.
//!
//! Supervision: the outbox loop is a task this process owns and joins. On SIGINT/SIGTERM it stops
//! claiming and drains within its bound; if the loop itself ends or panics before a shutdown was
//! requested, the process exits with a failure so that its service manager restarts it.
//! A production worker refuses to run the outbox with a privileged database role.

use std::process::ExitCode;
use std::time::Duration;

use tokio::sync::watch;
use vink_app::application::Vink;
use vink_app::config::{Config, Profile};
use vink_app::outbox_worker::{OutboxSettings, OutboxWorker};
use vink_app::persistence::{
    PgStore, SchemaState, lazy_pool, outbox, role_is_privileged, schema_state,
};
use vink_app::runtime;

const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);
/// Finished events are kept this long for audit and incident review.
const EVENT_RETENTION_DAYS: i32 = 30;
/// Events deleted per statement, so retention never holds locks long.
const PRUNE_BATCH: i64 = 1_000;

fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("vink-worker: {error}");
            return ExitCode::from(2);
        }
    };
    let outbox = match OutboxSettings::from_lookup(|key| std::env::var(key).ok()) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("vink-worker: invalid configuration: {error}");
            return ExitCode::from(2);
        }
    };
    let evaluation =
        match vink_app::evaluation_worker::EvaluationSettings::from_lookup(config.profile, |key| {
            std::env::var(key).ok()
        }) {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!("vink-worker: invalid configuration: {error}");
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
    tokio.block_on(run(config, outbox, evaluation))
}

/// What the startup probe found. `ready` is logged only when the queue tables answered.
async fn outbox_status(
    pool: &sqlx::PgPool,
    profile: Profile,
    settings: &OutboxSettings,
) -> &'static str {
    if !settings.enabled {
        return "disabled";
    }
    let probe = tokio::time::timeout(Duration::from_secs(5), async {
        Ok::<_, sqlx::Error>((schema_state(pool).await?, role_is_privileged(pool).await?))
    })
    .await;
    match probe {
        Ok(Ok((_, true))) if profile == Profile::Production => "refused_privileged_role",
        Ok(Ok((SchemaState::Current, _))) => match outbox::backlog(pool, &[]).await {
            Ok(_) => "ready",
            Err(_) => "unavailable",
        },
        Ok(Ok(_)) => "schema_not_current",
        Ok(Err(_)) | Err(_) => "unavailable",
    }
}

async fn run(
    config: Config,
    settings: OutboxSettings,
    eval_settings: vink_app::evaluation_worker::EvaluationSettings,
) -> ExitCode {
    // Listen for SIGINT/SIGTERM before any probe, so a signal during startup is a clean stop.
    let (stop, mut stopped) = watch::channel(false);
    let signals = tokio::spawn(async move {
        runtime::shutdown_signal().await;
        let _ = stop.send(true);
    });
    tokio::task::yield_now().await;
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
    let status = outbox_status(&pool, config.profile, &settings).await;
    tracing::info!(
        database,
        outbox = status,
        outbox_concurrency = settings.concurrency,
        evaluation = eval_settings.enabled,
        consumers = vink_app::domain::discussion::outbox::Consumer::ALL
            .map(|consumer| consumer.code())
            .join(","),
        jobs = "discussion_outbox,rate_limit_prune,outbox_retention,work_evaluation",
        post_review = "operator_rereview.v1",
        admission_post_review = "no_producer",
        "vink-worker started"
    );
    if status == "refused_privileged_role" {
        tracing::error!(
            "the outbox needs the restricted runtime role, not a migrator or superuser; exiting"
        );
        pool.close().await;
        return ExitCode::from(2);
    }
    // An unavailable database or an older schema is not fatal: the loop polls with backoff and
    // logs once the queue answers, so the worker recovers by itself after an outage or migration.
    let mut outbox = settings.enabled.then(|| {
        let worker = OutboxWorker::production(pool.clone(), settings)
            .require_restricted_role(config.profile == Profile::Production);
        tokio::spawn(worker.run(stopped.clone()))
    });
    let mut evaluation = eval_settings.enabled.then(|| {
        let worker =
            vink_app::evaluation_worker::EvaluationWorker::new(pool.clone(), eval_settings);
        tokio::spawn(worker.run(stopped.clone()))
    });
    let vink = Vink::new(PgStore::new(pool.clone()));
    let mut prune = tokio::time::interval(PRUNE_INTERVAL);
    prune.tick().await;
    let mut code = ExitCode::SUCCESS;
    while !*stopped.borrow() {
        tokio::select! {
            _ = stopped.changed() => {}
            joined = async { outbox.as_mut().expect("guarded").await }, if outbox.is_some() => {
                match joined {
                    Ok(()) => tracing::error!("outbox loop ended without a shutdown request"),
                    Err(error) => tracing::error!(panicked = error.is_panic(), "outbox loop failed"),
                }
                outbox = None;
                code = ExitCode::FAILURE;
                break;
            }
            joined = async { evaluation.as_mut().expect("guarded").await }, if evaluation.is_some() => {
                match joined {
                    Ok(()) => tracing::info!("evaluation worker loop finished"),
                    Err(error) => tracing::error!(panicked = error.is_panic(), "evaluation worker loop failed"),
                }
                evaluation = None;
            }
            _ = prune.tick() => {
                match vink.prune_rate_limits().await {
                    Ok(rows) => tracing::info!(rows, "rate-limit windows pruned"),
                    Err(_) => tracing::warn!("rate-limit prune could not reach the database"),
                }
                match outbox::prune(&pool, EVENT_RETENTION_DAYS, PRUNE_BATCH).await {
                    Ok(rows) => tracing::info!(rows, "finished outbox events pruned"),
                    Err(_) => tracing::warn!("outbox retention could not reach the database"),
                }
            }
        }
    }
    // The outbox loop sees the same signal; wait for its drain.
    if let Some(task) = outbox
        && let Err(error) = task.await
    {
        tracing::error!(
            panicked = error.is_panic(),
            "outbox loop failed during drain"
        );
        code = ExitCode::FAILURE;
    }
    if let Some(task) = evaluation
        && let Err(error) = task.await
    {
        tracing::error!(
            panicked = error.is_panic(),
            "evaluation loop failed during drain"
        );
        code = ExitCode::FAILURE;
    }
    signals.abort();
    pool.close().await;
    tracing::info!("vink-worker stopped");
    code
}
