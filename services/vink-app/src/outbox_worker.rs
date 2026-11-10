//! The discussion outbox loop that `vink-worker` supervises (DISC-04, #120).
//!
//! **Wake-up (#120 DISC-04 CHOICE-02 A).** Polling only: when a pass claims nothing, the next
//! poll waits from [`OutboxSettings::poll_min`] doubling to [`OutboxSettings::poll_max`], with
//! jitter, and resets as soon as work appears. There is no LISTEN/NOTIFY, so no notification can
//! be lost: a backlog left by a crash, an outage or a restart is found by the next poll.
//!
//! **Bounds.** At most [`OutboxSettings::concurrency`] deliveries are in flight, and a pass claims
//! only as many as there are free slots, so nothing waits inside the process while its lease runs
//! out. Each delivery runs as its own task with a timeout shorter than the lease; the statements
//! of its transaction are bounded too (`persistence::outbox`). A handler panic is contained in
//! its task and recorded as a retryable failure; a panic of this loop ends the process.
//!
//! **Shutdown.** On the signal the loop stops claiming, waits up to
//! [`OutboxSettings::drain`] for the tasks in flight, then aborts the rest. An aborted task's
//! transaction rolls back, so nothing is acknowledged that was not committed; its lease expires
//! and any worker reclaims it, spending that attempt.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::postgres::PgPool;
use tokio::sync::watch;
use tokio::task::{Id, JoinSet};
use uuid::Uuid;

use crate::application::ports::StoreError;
use crate::domain::discussion::outbox::{Completion, OutboxPolicy, Transient};
use crate::persistence::outbox::{
    AdmissionShadow, Backlog, Claimed, EffectHandler, backlog, claim, complete, record_failure,
};
use crate::persistence::role_is_privileged;

/// The worker's pool has 10 connections (`PoolSettings::serving`). Each delivery uses one at a
/// time; the claim, the heartbeat and retention need the others, so a delivery never waits for a
/// connection long enough to lose its attempt to an acquire timeout.
pub const MAX_CONCURRENCY: usize = 6;

/// Worker settings read from the environment; everything else uses [`OutboxPolicy::DEFAULT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxSettings {
    /// `VINK_WORKER_OUTBOX=enabled|disabled` (default enabled). Disabling only pauses delivery:
    /// the App keeps writing events, and nothing owed is deleted.
    pub enabled: bool,
    /// `VINK_WORKER_OUTBOX_CONCURRENCY`, 1..=[`MAX_CONCURRENCY`] (default 4).
    pub concurrency: usize,
    pub policy: OutboxPolicy,
    /// Bound on one delivery's fenced transaction; shorter than the lease.
    pub handler_timeout: Duration,
    pub poll_min: Duration,
    pub poll_max: Duration,
    pub drain: Duration,
    /// How often the loop logs queue health.
    pub heartbeat: Duration,
}

impl Default for OutboxSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            concurrency: 4,
            policy: OutboxPolicy::DEFAULT,
            handler_timeout: Duration::from_secs(10),
            poll_min: Duration::from_secs(1),
            poll_max: Duration::from_secs(30),
            drain: Duration::from_secs(10),
            heartbeat: Duration::from_secs(60),
        }
    }
}

impl OutboxSettings {
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let mut settings = Self::default();
        match get("VINK_WORKER_OUTBOX").as_deref().map(str::trim) {
            None | Some("" | "enabled") => {}
            Some("disabled") => settings.enabled = false,
            Some(other) => {
                return Err(format!(
                    "VINK_WORKER_OUTBOX: expected `enabled` or `disabled`, got `{other}`"
                ));
            }
        }
        if let Some(raw) = get("VINK_WORKER_OUTBOX_CONCURRENCY").filter(|v| !v.trim().is_empty()) {
            settings.concurrency = raw
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=MAX_CONCURRENCY).contains(n))
                .ok_or_else(|| {
                    format!(
                        "VINK_WORKER_OUTBOX_CONCURRENCY: expected 1..={MAX_CONCURRENCY}, got `{raw}`"
                    )
                })?;
        }
        Ok(settings)
    }
}

/// In-process counters, logged with each heartbeat. Counts only; no identifier is a label.
#[derive(Debug, Default)]
pub struct Stats {
    pub claimed: AtomicU64,
    pub reclaimed: AtomicU64,
    pub succeeded: AtomicU64,
    pub skipped: AtomicU64,
    pub retried: AtomicU64,
    pub dead_lettered: AtomicU64,
    pub stale_refused: AtomicU64,
    pub timeouts: AtomicU64,
    pub panics: AtomicU64,
    pub database_errors: AtomicU64,
}

impl Stats {
    fn add(counter: &AtomicU64, n: usize) {
        counter.fetch_add(n as u64, Ordering::Relaxed);
    }

    fn get(counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    fn count(&self, completion: Completion) {
        let counter = match completion {
            Completion::Succeed { .. } => &self.succeeded,
            Completion::Skip(_) => &self.skipped,
            Completion::Retry { .. } => &self.retried,
            Completion::DeadLetter(_) => &self.dead_lettered,
            Completion::Stale(_) => &self.stale_refused,
        };
        Self::add(counter, 1);
    }
}

/// Jitter in `[0, cap)` microseconds from the random bits of a UUIDv7. Not a secret.
fn jitter_micros(cap: i64) -> i64 {
    let random = (Uuid::now_v7().as_u128() & u128::from(u64::MAX)) as u64;
    let cap = u64::try_from(cap.max(1)).unwrap_or(1);
    i64::try_from(random % cap).unwrap_or(0)
}

fn jittered(base: Duration) -> Duration {
    let quarter = i64::try_from(base.as_micros() / 4).unwrap_or(i64::MAX);
    base + Duration::from_micros(u64::try_from(jitter_micros(quarter)).unwrap_or(0))
}

#[derive(Clone)]
pub struct OutboxWorker {
    pool: PgPool,
    handlers: Arc<HashMap<String, Arc<dyn EffectHandler>>>,
    settings: OutboxSettings,
    stats: Arc<Stats>,
    restricted_role: bool,
}

impl OutboxWorker {
    /// Production: refuse to deliver with a privileged role (migrator or superuser). Checked when
    /// the queue first answers, so a database that was down at startup is checked too.
    #[must_use]
    pub fn require_restricted_role(mut self, required: bool) -> Self {
        self.restricted_role = required;
        self
    }

    /// The production consumers.
    pub fn production(pool: PgPool, settings: OutboxSettings) -> Self {
        Self::with_handlers(pool, settings, vec![Arc::new(AdmissionShadow)])
    }

    /// Explicit handlers; tests add labelled fixture consumers this way.
    pub fn with_handlers(
        pool: PgPool,
        settings: OutboxSettings,
        handlers: Vec<Arc<dyn EffectHandler>>,
    ) -> Self {
        let handlers = handlers
            .into_iter()
            .map(|handler| (handler.consumer().to_owned(), handler))
            .collect();
        Self {
            pool,
            handlers: Arc::new(handlers),
            settings,
            stats: Arc::default(),
            restricted_role: false,
        }
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    fn consumers(&self) -> Vec<&str> {
        let mut consumers: Vec<&str> = self.handlers.keys().map(String::as_str).collect();
        consumers.sort_unstable();
        consumers
    }

    pub async fn backlog(&self) -> Result<Backlog, StoreError> {
        backlog(&self.pool, &self.consumers()).await
    }

    async fn claim(&self, limit: usize) -> Result<Vec<Claimed>, StoreError> {
        let batch = claim(
            &self.pool,
            &self.consumers(),
            i64::try_from(limit).unwrap_or(i64::MAX),
            self.settings.policy,
        )
        .await?;
        Stats::add(&self.stats.claimed, batch.claimed.len());
        Stats::add(&self.stats.reclaimed, batch.reclaimed);
        Stats::add(&self.stats.dead_lettered, batch.dead_lettered);
        Ok(batch.claimed)
    }

    /// Runs one claimed delivery to a recorded outcome (or leaves it to lease expiry).
    pub async fn process(&self, claimed: Claimed) -> Option<Completion> {
        let Some(handler) = self.handlers.get(&claimed.consumer).cloned() else {
            // Claims are filtered by handled consumers; reaching this is a defect. Leave the
            // lease to expire rather than decide for an unknown consumer.
            tracing::error!(
                consumer = claimed.consumer.as_str(),
                "claimed a delivery without handler"
            );
            return None;
        };
        let policy = self.settings.policy;
        let jitter = jitter_micros(policy.base_backoff_micros());
        let attempt = tokio::time::timeout(
            self.settings.handler_timeout,
            complete(&self.pool, &claimed, handler.as_ref(), policy, jitter),
        )
        .await;
        let completion = match attempt {
            Ok(Ok(completion)) => Ok(completion),
            Ok(Err(_)) => {
                Stats::add(&self.stats.database_errors, 1);
                self.fail(&claimed, Transient::Database).await
            }
            Err(_elapsed) => {
                Stats::add(&self.stats.timeouts, 1);
                self.fail(&claimed, Transient::Timeout).await
            }
        };
        match completion {
            Ok(completion) => {
                self.stats.count(completion);
                log_completion(&claimed, completion);
                Some(completion)
            }
            Err(_) => {
                tracing::warn!(
                    consumer = claimed.consumer.as_str(),
                    attempts = claimed.attempts,
                    "outbox outcome not recorded; the lease will expire and the delivery retry"
                );
                None
            }
        }
    }

    async fn fail(&self, claimed: &Claimed, failure: Transient) -> Result<Completion, StoreError> {
        let policy = self.settings.policy;
        record_failure(
            &self.pool,
            claimed,
            failure,
            policy,
            jitter_micros(policy.base_backoff_micros()),
        )
        .await
    }

    /// Claims up to `concurrency` deliveries, processes them concurrently and waits for all.
    /// Returns how many were claimed. Used by tests and one-shot operator runs.
    pub async fn run_once(&self) -> Result<usize, StoreError> {
        let claimed = self.claim(self.settings.concurrency).await?;
        let count = claimed.len();
        let mut tasks = JoinSet::new();
        for delivery in claimed {
            let worker = self.clone();
            tasks.spawn(async move { worker.process(delivery).await });
        }
        while tasks.join_next().await.is_some() {}
        Ok(count)
    }

    /// The supervised loop. Returns after `shutdown` turns true and the drain ends.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let mut tasks: JoinSet<Option<Completion>> = JoinSet::new();
        let mut in_flight: HashMap<Id, Claimed> = HashMap::new();
        let mut idle = self.settings.poll_min;
        let mut reachable = None::<bool>;
        let mut role_checked = false;
        let mut heartbeat = tokio::time::interval(self.settings.heartbeat);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            if *shutdown.borrow() {
                break;
            }
            let free = self.settings.concurrency.saturating_sub(tasks.len());
            let mut wait = idle;
            // Production checks the role BEFORE the first claim and fails closed: an unknown
            // answer claims nothing and is retried with the same backoff as an outage.
            let mut role_ok = !self.restricted_role || role_checked;
            if !role_ok {
                match role_is_privileged(&self.pool).await {
                    Ok(false) => {
                        role_checked = true;
                        role_ok = true;
                    }
                    Ok(true) => {
                        tracing::error!("outbox refuses a privileged database role; stopping");
                        break;
                    }
                    Err(_) => {
                        Stats::add(&self.stats.database_errors, 1);
                        idle = (idle * 2).min(self.settings.poll_max);
                        wait = idle;
                    }
                }
            }
            if free > 0 && role_ok {
                match self.claim(free).await {
                    Ok(claimed) => {
                        if reachable != Some(true) {
                            tracing::info!("outbox queue reachable");
                            reachable = Some(true);
                        }
                        if claimed.is_empty() {
                            idle = (idle * 2).min(self.settings.poll_max);
                        } else {
                            idle = self.settings.poll_min;
                            wait = Duration::ZERO;
                        }
                        for delivery in claimed {
                            let worker = self.clone();
                            let key = delivery.clone();
                            let handle = tasks.spawn(async move { worker.process(delivery).await });
                            in_flight.insert(handle.id(), key);
                        }
                    }
                    Err(_) => {
                        Stats::add(&self.stats.database_errors, 1);
                        if reachable != Some(false) {
                            tracing::warn!("outbox queue unavailable; retrying with backoff");
                            reachable = Some(false);
                        }
                        idle = (idle * 2).min(self.settings.poll_max);
                        wait = idle;
                    }
                }
            }
            let sleep = tokio::time::sleep(jittered(wait));
            tokio::pin!(sleep);
            tokio::select! {
                // A dropped sender also means stop: never spin on a closed channel.
                changed = shutdown.changed() => if changed.is_err() { break; },
                Some(joined) = tasks.join_next_with_id(), if !tasks.is_empty() => {
                    self.reap(joined, &mut in_flight).await;
                    continue;
                }
                _ = heartbeat.tick() => self.log_heartbeat().await,
                () = &mut sleep => {}
            }
        }
        tracing::info!(in_flight = tasks.len(), "outbox stopped claiming; draining");
        let drained = tokio::time::timeout(self.settings.drain, async {
            while let Some(joined) = tasks.join_next_with_id().await {
                self.reap(joined, &mut in_flight).await;
            }
        })
        .await;
        if drained.is_err() {
            tracing::warn!(
                aborted = tasks.len(),
                "outbox drain timed out; aborted tasks roll back and their leases expire"
            );
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        }
        self.log_heartbeat().await;
        tracing::info!("outbox stopped");
    }

    async fn reap(
        &self,
        joined: Result<(Id, Option<Completion>), tokio::task::JoinError>,
        in_flight: &mut HashMap<Id, Claimed>,
    ) {
        match joined {
            Ok((id, _)) => {
                in_flight.remove(&id);
            }
            Err(error) => {
                let Some(claimed) = in_flight.remove(&error.id()) else {
                    return;
                };
                if error.is_panic() {
                    Stats::add(&self.stats.panics, 1);
                    tracing::error!(
                        consumer = claimed.consumer.as_str(),
                        "outbox handler panicked; its transaction rolled back"
                    );
                    if let Ok(completion) = self.fail(&claimed, Transient::Panic).await {
                        self.stats.count(completion);
                        log_completion(&claimed, completion);
                    }
                }
            }
        }
    }

    async fn log_heartbeat(&self) {
        let stats = &self.stats;
        match self.backlog().await {
            Ok(backlog) => tracing::info!(
                eligible = backlog.eligible,
                scheduled = backlog.scheduled,
                leased = backlog.leased,
                dead = backlog.dead,
                unhandled = backlog.unhandled,
                oldest_eligible_seconds = backlog.oldest_eligible_seconds,
                oldest_open_seconds = backlog.oldest_open_seconds,
                claimed = Stats::get(&stats.claimed),
                reclaimed = Stats::get(&stats.reclaimed),
                succeeded = Stats::get(&stats.succeeded),
                skipped = Stats::get(&stats.skipped),
                retried = Stats::get(&stats.retried),
                dead_lettered = Stats::get(&stats.dead_lettered),
                stale_refused = Stats::get(&stats.stale_refused),
                timeouts = Stats::get(&stats.timeouts),
                panics = Stats::get(&stats.panics),
                database_errors = Stats::get(&stats.database_errors),
                "outbox heartbeat"
            ),
            Err(_) => tracing::warn!(
                database_errors = Stats::get(&stats.database_errors),
                "outbox heartbeat: queue unavailable"
            ),
        }
    }
}

fn log_completion(claimed: &Claimed, completion: Completion) {
    let consumer = claimed.consumer.as_str();
    match completion {
        Completion::Succeed { .. } | Completion::Skip(_) => tracing::debug!(
            consumer,
            outcome = completion.outcome(),
            "outbox delivery finished"
        ),
        Completion::Retry { error, .. } => tracing::info!(
            consumer,
            attempts = claimed.attempts,
            error = error.code(),
            "outbox delivery retry scheduled"
        ),
        Completion::DeadLetter(reason) => tracing::warn!(
            consumer,
            attempts = claimed.attempts,
            reason = reason.code(),
            "outbox delivery dead-lettered"
        ),
        Completion::Stale(reason) => tracing::info!(
            consumer,
            reason = reason.code(),
            "outbox result refused: the claim is no longer ours"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn settings_default_and_validate() {
        assert_eq!(
            OutboxSettings::from_lookup(lookup(&[])),
            Ok(OutboxSettings::default())
        );
        let paused = OutboxSettings::from_lookup(lookup(&[("VINK_WORKER_OUTBOX", "disabled")]));
        assert!(!paused.unwrap().enabled);
        let wide = OutboxSettings::from_lookup(lookup(&[("VINK_WORKER_OUTBOX_CONCURRENCY", "6")]));
        assert_eq!(wide.unwrap().concurrency, MAX_CONCURRENCY);
        for (key, value) in [
            ("VINK_WORKER_OUTBOX", "on"),
            ("VINK_WORKER_OUTBOX_CONCURRENCY", "0"),
            ("VINK_WORKER_OUTBOX_CONCURRENCY", "7"),
            ("VINK_WORKER_OUTBOX_CONCURRENCY", "four"),
        ] {
            assert!(
                OutboxSettings::from_lookup(lookup(&[(key, value)])).is_err(),
                "{key}={value}"
            );
        }
        let defaults = OutboxSettings::default();
        assert!(
            defaults.handler_timeout.as_micros()
                < u128::try_from(defaults.policy.lease_micros()).unwrap(),
            "a live handler must finish before its lease expires"
        );
    }

    #[test]
    fn jitter_stays_below_its_cap() {
        for _ in 0..1_000 {
            let jitter = jitter_micros(5_000_000);
            assert!((0..5_000_000).contains(&jitter));
        }
        assert_eq!(jitter_micros(0), 0);
        let base = Duration::from_secs(4);
        let waited = jittered(base);
        assert!(waited >= base && waited < base + Duration::from_secs(1));
    }
}
