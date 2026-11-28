//! Điểm Vink consumer loop (#646): drains `reputation_events` through
//! [`crate::persistence::reputation_projection::consume_batch`] and matures held actors.
//!
//! `VINK_WORKER_REPUTATION=disabled` stops the consumer (technical stop): events wait and the
//! projection lags; producers are governed separately by `vink-app reputation activate`. Before
//! `vink-app reputation backfill` commits, a pass only checks readiness and sleeps.

use std::time::Duration;

use sqlx::postgres::PgPool;
use tokio::sync::watch;

use crate::application::ports::StoreError;
use crate::persistence::reputation_projection::{BatchReport, consume_batch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationSettings {
    pub enabled: bool,
    pub batch: i64,
    pub poll_min: Duration,
    pub poll_max: Duration,
}

impl Default for ReputationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            batch: 200,
            poll_min: Duration::from_millis(500),
            poll_max: Duration::from_secs(10),
        }
    }
}

impl ReputationSettings {
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let mut settings = Self::default();
        match get("VINK_WORKER_REPUTATION").as_deref().map(str::trim) {
            None | Some("" | "enabled") => settings.enabled = true,
            Some("disabled") => settings.enabled = false,
            Some(other) => {
                return Err(format!(
                    "VINK_WORKER_REPUTATION: expected `enabled` or `disabled`, got `{other}`"
                ));
            }
        }
        Ok(settings)
    }
}

pub struct ReputationWorker {
    pool: PgPool,
    settings: ReputationSettings,
}

impl ReputationWorker {
    pub fn new(pool: PgPool, settings: ReputationSettings) -> Self {
        Self { pool, settings }
    }

    /// One pass with the given batch size (tests call this directly).
    pub async fn pass(pool: &PgPool, batch: i64) -> Result<BatchReport, StoreError> {
        consume_batch(pool, batch).await
    }

    /// Supervised loop for a tokio task.
    pub async fn run(self, mut stopped: watch::Receiver<bool>) {
        if !self.settings.enabled {
            tracing::warn!("reputation consumer disabled by configuration: events wait");
            return;
        }
        tracing::info!(batch = self.settings.batch, "reputation consumer started");
        let mut backoff = self.settings.poll_min;
        loop {
            if *stopped.borrow() {
                tracing::info!("reputation consumer stopping on signal");
                break;
            }
            let wait = match Self::pass(&self.pool, self.settings.batch).await {
                Ok(report) if report.events > 0 || report.matured > 0 => {
                    tracing::info!(
                        events = report.events,
                        matured = report.matured,
                        "reputation batch applied"
                    );
                    backoff = self.settings.poll_min;
                    None
                }
                Ok(_) => {
                    let wait = backoff;
                    backoff = (backoff * 2).min(self.settings.poll_max);
                    Some(wait)
                }
                Err(error) => {
                    tracing::error!(%error, "reputation consumer pass error");
                    Some(self.settings.poll_max)
                }
            };
            if let Some(wait) = wait {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = stopped.changed() => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_parse_the_switch() {
        assert!(ReputationSettings::from_lookup(|_| None).unwrap().enabled);
        let off = ReputationSettings::from_lookup(|key| {
            (key == "VINK_WORKER_REPUTATION").then(|| "disabled".to_owned())
        })
        .unwrap();
        assert!(!off.enabled);
        assert!(ReputationSettings::from_lookup(|_| Some("maybe".to_owned())).is_err());
    }
}
