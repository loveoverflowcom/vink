//! Notifications v1 relay (#346, T-02 CHOICE-16 A): a supervised loop of the App's worker that
//! moves committed notification obligations to the Notification Service (#347) with at least
//! once delivery and a fenced acknowledgement (contract §1.3).
//!
//! **One pass.** Claim up to [`RelaySettings::batch`] due deliveries in a short transaction that
//! commits before any I/O; send their stored envelopes (plus digest) in one call through the
//! [`NotificationSink`] port, outside any App transaction and bounded by
//! [`RelaySettings::send_timeout`] (shorter than the lease); then record each answer in its own
//! fenced transaction. The receiver commits its receipt and effect before answering; the App
//! marks the delivery done only after that answer and only while its claim is still current.
//!
//! **Failures.** A lost answer, a timeout or a crash leaves the lease to expire; the next claim
//! (this or another process) advances the generation and re-sends the same `(event_id,
//! payload_digest)`, which the receiver reports `already_applied`. A late answer for an old
//! generation writes nothing. `payload_mismatch` and `rejected_invalid` are dead letters for an
//! operator (`vink-app notifications dead|requeue`); transient answers back off with a cap.
//!
//! **Bounds.** One call in flight per relay, at most `batch` events per call; several relay
//! processes may run (claims use `SKIP LOCKED`). On shutdown the loop stops claiming and waits up
//! to [`RelaySettings::drain`] for the pass in flight; an abandoned pass records nothing and its
//! leases expire.
//!
//! **Transport.** T-07 (Unix socket with peer credentials vs. internal HTTP with a service
//! token) belongs to #347. Until that receiver exists no production sink is configured and
//! `vink-worker` does not start this loop; tests drive it with receiver fixtures.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::postgres::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

use crate::application::ports::StoreError;
use crate::domain::discussion::outbox::OutboxPolicy;
use crate::domain::notification::{IngestStatus, RelayCompletion, RelayFailure, RelayResult, wire};
use crate::persistence::notification::{ClaimedNotification, claim, record};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One event as sent: its identity, digest and the wire envelope (canonical + digest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub event_id: Uuid,
    pub payload_digest: String,
    pub wire: String,
}

/// The receiver's answer for one event of a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestReceipt {
    pub event_id: Uuid,
    pub status: IngestStatus,
}

/// Why a whole call failed before any per-event answer was known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkFailure {
    /// Refused, unreachable or broken; the receiver may or may not have applied events.
    Unavailable,
}

/// The authenticated transport to the Notification Service (#347, T-07). An implementation
/// returns only after the receiver committed (or refused) each event it answers for.
pub trait NotificationSink: Send + Sync {
    fn deliver<'a>(
        &'a self,
        events: &'a [Outgoing],
    ) -> BoxFuture<'a, Result<Vec<IngestReceipt>, SinkFailure>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelaySettings {
    /// Events per call and per claim, 1..=[`MAX_BATCH`].
    pub batch: usize,
    pub policy: OutboxPolicy,
    /// Bound on one call; shorter than the lease so a live call never outlives its claim.
    pub send_timeout: Duration,
    pub poll_min: Duration,
    pub poll_max: Duration,
    pub drain: Duration,
}

pub const MAX_BATCH: usize = 100;

impl Default for RelaySettings {
    fn default() -> Self {
        Self {
            batch: 50,
            policy: OutboxPolicy::DEFAULT,
            send_timeout: Duration::from_secs(10),
            poll_min: Duration::from_secs(1),
            poll_max: Duration::from_secs(30),
            drain: Duration::from_secs(10),
        }
    }
}

impl RelaySettings {
    /// Bounds the relay relies on: a batch within limits and a send shorter than the lease.
    pub fn validate(self) -> Result<Self, String> {
        if !(1..=MAX_BATCH).contains(&self.batch) {
            return Err(format!("batch must be 1..={MAX_BATCH}"));
        }
        let lease = u128::try_from(self.policy.lease_micros()).unwrap_or(0);
        if self.send_timeout.as_micros() >= lease {
            return Err("send_timeout must be shorter than the lease".into());
        }
        if self.poll_min.is_zero() || self.poll_max < self.poll_min {
            return Err("poll bounds must be positive and ordered".into());
        }
        Ok(self)
    }
}

/// Counts only; no identifier is ever a label.
#[derive(Debug, Default)]
pub struct RelayStats {
    pub claimed: AtomicU64,
    pub reclaimed: AtomicU64,
    pub acked: AtomicU64,
    pub already_applied: AtomicU64,
    pub retried: AtomicU64,
    pub dead_lettered: AtomicU64,
    pub stale_refused: AtomicU64,
    pub send_failures: AtomicU64,
    pub database_errors: AtomicU64,
}

impl RelayStats {
    fn add(counter: &AtomicU64, n: usize) {
        counter.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub fn get(counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    fn count(&self, completion: RelayCompletion) {
        let counter = match completion {
            RelayCompletion::Acked {
                already_applied: false,
            } => &self.acked,
            RelayCompletion::Acked {
                already_applied: true,
            } => &self.already_applied,
            RelayCompletion::Retry { .. } => &self.retried,
            RelayCompletion::DeadLetter(_) => &self.dead_lettered,
            RelayCompletion::Stale(_) => &self.stale_refused,
        };
        Self::add(counter, 1);
    }
}

/// What one pass did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelayPass {
    pub claimed: usize,
    pub completions: Vec<(Uuid, RelayCompletion)>,
}

fn jitter_micros(cap: i64) -> i64 {
    let random = (Uuid::now_v7().as_u128() & u128::from(u64::MAX)) as u64;
    let cap = u64::try_from(cap.max(1)).unwrap_or(1);
    i64::try_from(random % cap).unwrap_or(0)
}

#[derive(Clone)]
pub struct NotificationRelay {
    pool: PgPool,
    sink: Arc<dyn NotificationSink>,
    settings: RelaySettings,
    stats: Arc<RelayStats>,
}

impl NotificationRelay {
    pub fn new(
        pool: PgPool,
        sink: Arc<dyn NotificationSink>,
        settings: RelaySettings,
    ) -> Result<Self, String> {
        Ok(Self {
            pool,
            sink,
            settings: settings.validate()?,
            stats: Arc::default(),
        })
    }

    pub fn stats(&self) -> &RelayStats {
        &self.stats
    }

    /// Claims, sends once and records. Used by the loop, tests and one-shot operator runs.
    pub async fn run_once(&self) -> Result<RelayPass, StoreError> {
        let policy = self.settings.policy;
        let batch = claim(
            &self.pool,
            i64::try_from(self.settings.batch).unwrap_or(1),
            policy,
        )
        .await?;
        RelayStats::add(&self.stats.claimed, batch.claimed.len());
        RelayStats::add(&self.stats.reclaimed, batch.reclaimed);
        RelayStats::add(&self.stats.dead_lettered, batch.dead_lettered);
        let mut pass = RelayPass {
            claimed: batch.claimed.len(),
            completions: Vec::new(),
        };
        if batch.claimed.is_empty() {
            return Ok(pass);
        }
        let mut sendable: Vec<&ClaimedNotification> = Vec::new();
        let mut outgoing: Vec<Outgoing> = Vec::new();
        let mut results: HashMap<Uuid, RelayResult> = HashMap::new();
        for claimed in &batch.claimed {
            match wire(&claimed.envelope, &claimed.payload_digest) {
                Some(wire) => {
                    sendable.push(claimed);
                    outgoing.push(Outgoing {
                        event_id: claimed.event_id,
                        payload_digest: claimed.payload_digest.clone(),
                        wire,
                    });
                }
                None => {
                    results.insert(
                        claimed.event_id,
                        RelayResult::Failed(RelayFailure::InvalidStored),
                    );
                }
            }
        }
        if !outgoing.is_empty() {
            let answered =
                tokio::time::timeout(self.settings.send_timeout, self.sink.deliver(&outgoing))
                    .await;
            let failure = match answered {
                Ok(Ok(receipts)) => {
                    let mut by_id: HashMap<Uuid, IngestStatus> = HashMap::new();
                    for receipt in receipts {
                        by_id.entry(receipt.event_id).or_insert(receipt.status);
                    }
                    for claimed in &sendable {
                        let result = by_id
                            .get(&claimed.event_id)
                            .map_or(RelayResult::Failed(RelayFailure::MissingResult), |status| {
                                RelayResult::Ingested(*status)
                            });
                        results.insert(claimed.event_id, result);
                    }
                    None
                }
                Ok(Err(SinkFailure::Unavailable)) => Some(RelayFailure::Transport),
                Err(_elapsed) => Some(RelayFailure::Timeout),
            };
            if let Some(failure) = failure {
                RelayStats::add(&self.stats.send_failures, 1);
                tracing::info!(
                    failure = failure.code(),
                    events = sendable.len(),
                    "notification send failed; deliveries will retry"
                );
                for claimed in &sendable {
                    results.insert(claimed.event_id, RelayResult::Failed(failure));
                }
            }
        }
        for claimed in &batch.claimed {
            let Some(result) = results.get(&claimed.event_id).copied() else {
                continue;
            };
            let jitter = jitter_micros(policy.base_backoff_micros());
            match record(&self.pool, claimed, result, policy, jitter).await {
                Ok(completion) => {
                    self.stats.count(completion);
                    log_completion(claimed, completion);
                    pass.completions.push((claimed.event_id, completion));
                }
                Err(_) => {
                    // The lease expires and the delivery is re-sent; the receiver's receipt
                    // makes the duplicate one effect.
                    RelayStats::add(&self.stats.database_errors, 1);
                    tracing::warn!(
                        attempts = claimed.attempts,
                        "notification outcome not recorded; the lease will expire"
                    );
                }
            }
        }
        Ok(pass)
    }

    /// The supervised loop. Returns after `shutdown` turns true and the drain ends.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let mut idle = self.settings.poll_min;
        loop {
            if *shutdown.borrow() {
                break;
            }
            let pass = self.run_once();
            tokio::pin!(pass);
            let finished = tokio::select! {
                finished = &mut pass => Some(finished),
                _ = shutdown.changed() => None,
            };
            let finished = match finished {
                Some(finished) => finished,
                None => {
                    match tokio::time::timeout(self.settings.drain, &mut pass).await {
                        Ok(_) => tracing::info!("notification relay drained its pass"),
                        Err(_) => tracing::warn!(
                            "notification relay drain timed out; its leases will expire"
                        ),
                    }
                    break;
                }
            };
            let wait = match finished {
                Ok(pass) if pass.claimed > 0 => {
                    idle = self.settings.poll_min;
                    Duration::ZERO
                }
                Ok(_) => {
                    idle = (idle * 2).min(self.settings.poll_max);
                    idle
                }
                Err(_) => {
                    RelayStats::add(&self.stats.database_errors, 1);
                    idle = (idle * 2).min(self.settings.poll_max);
                    idle
                }
            };
            if wait.is_zero() {
                continue;
            }
            tokio::select! {
                changed = shutdown.changed() => if changed.is_err() { break; },
                () = tokio::time::sleep(wait) => {}
            }
        }
        tracing::info!(
            acked = RelayStats::get(&self.stats.acked),
            already_applied = RelayStats::get(&self.stats.already_applied),
            retried = RelayStats::get(&self.stats.retried),
            dead_lettered = RelayStats::get(&self.stats.dead_lettered),
            stale_refused = RelayStats::get(&self.stats.stale_refused),
            "notification relay stopped"
        );
    }
}

fn log_completion(claimed: &ClaimedNotification, completion: RelayCompletion) {
    match completion {
        RelayCompletion::Acked { .. } => tracing::debug!(
            outcome = completion.outcome(),
            "notification delivery acknowledged"
        ),
        RelayCompletion::Retry { error, .. } => tracing::info!(
            attempts = claimed.attempts,
            error,
            "notification delivery retry scheduled"
        ),
        RelayCompletion::DeadLetter(reason) => tracing::warn!(
            attempts = claimed.attempts,
            reason = reason.code(),
            "notification delivery dead-lettered"
        ),
        RelayCompletion::Stale(reason) => tracing::info!(
            reason = reason.code(),
            "notification result refused: the claim is no longer ours"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_keep_a_send_inside_its_lease() {
        assert!(RelaySettings::default().validate().is_ok());
        let slow = RelaySettings {
            send_timeout: Duration::from_secs(30),
            ..RelaySettings::default()
        };
        assert!(slow.validate().is_err());
        for batch in [0, MAX_BATCH + 1] {
            assert!(
                RelaySettings {
                    batch,
                    ..RelaySettings::default()
                }
                .validate()
                .is_err()
            );
        }
        let unordered = RelaySettings {
            poll_min: Duration::from_secs(5),
            poll_max: Duration::from_secs(1),
            ..RelaySettings::default()
        };
        assert!(unordered.validate().is_err());
    }
}
