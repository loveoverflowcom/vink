//! Background evaluation worker for work publication reviews (#159, MOD-01..08, MOD-14).
//!
//! Claims jobs from `work_evaluation_jobs`, asks the evaluator outside any database lock, and
//! applies the answer through [`crate::persistence::evaluation::finish_job`], which re-checks
//! the claim, the automation mode and the decision fence in the applying commit.
//!
//! No real AI provider exists. Without an allowed mock verdict every job goes to the human
//! queue; nothing is ever approved on the evaluator's behalf. The consumer is off unless
//! `VINK_WORKER_EVALUATION=enabled`, and a scripted verdict (`VINK_MOCK_EVALUATION_SCENARIO`)
//! is refused in the production profile.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::application::ports::StoreError;
use crate::config::Profile;
use crate::persistence::evaluation::{
    ClaimedJob, JobOutcome, MOCK_SCENARIOS, claim_job, finish_job, scripted_outcome,
};
use sqlx::postgres::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationSettings {
    pub enabled: bool,
    pub lease_duration: Duration,
    pub retry_backoff: Duration,
    pub poll_min: Duration,
    pub poll_max: Duration,
    /// Whether job-bound scripted verdicts are honoured (never in production).
    pub allow_mock: bool,
    /// Scripted verdict bound to a job on its first claim (local/test only).
    pub mock_scenario: Option<String>,
}

impl Default for EvaluationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            lease_duration: Duration::from_secs(30),
            retry_backoff: Duration::from_secs(2),
            poll_min: Duration::from_millis(500),
            poll_max: Duration::from_secs(5),
            allow_mock: false,
            mock_scenario: None,
        }
    }
}

impl EvaluationSettings {
    pub fn from_lookup(
        profile: Profile,
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let mut settings = Self {
            allow_mock: profile != Profile::Production,
            ..Self::default()
        };
        match get("VINK_WORKER_EVALUATION").as_deref().map(str::trim) {
            None | Some("" | "disabled") => settings.enabled = false,
            Some("enabled") => settings.enabled = true,
            Some(other) => {
                return Err(format!(
                    "VINK_WORKER_EVALUATION: expected `enabled` or `disabled`, got `{other}`"
                ));
            }
        }
        let scenario = get("VINK_MOCK_EVALUATION_SCENARIO")
            .map(|raw| raw.trim().to_owned())
            .filter(|raw| !raw.is_empty());
        if let Some(scenario) = scenario {
            if profile == Profile::Production {
                return Err(
                    "VINK_MOCK_EVALUATION_SCENARIO: a scripted evaluator is refused in production"
                        .to_owned(),
                );
            }
            if !MOCK_SCENARIOS.contains(&scenario.as_str()) {
                return Err(format!(
                    "VINK_MOCK_EVALUATION_SCENARIO: expected one of {}, got `{scenario}`",
                    MOCK_SCENARIOS.join(", ")
                ));
            }
            settings.mock_scenario = Some(scenario);
        }
        Ok(settings)
    }
}

pub struct EvaluationWorker {
    pool: PgPool,
    settings: EvaluationSettings,
    worker_id: String,
    processed_count: AtomicU64,
}

impl EvaluationWorker {
    pub fn new(pool: PgPool, settings: EvaluationSettings) -> Self {
        let worker_id = format!("eval-worker-{}", Uuid::now_v7());
        Self {
            pool,
            settings,
            worker_id,
            processed_count: AtomicU64::new(0),
        }
    }

    pub fn processed(&self) -> u64 {
        self.processed_count.load(Ordering::Relaxed)
    }

    /// One claim and finish with test-friendly settings (mock allowed, no default scenario, no
    /// retry backoff). Returns whether a job was claimed.
    pub async fn run_once(pool: &PgPool, worker_id: &str) -> Result<bool, StoreError> {
        Ok(Self::pass(pool, worker_id, &Self::test_settings())
            .await?
            .is_some())
    }

    /// Settings for tests: scripted verdicts honoured, no default scenario, no retry backoff.
    pub fn test_settings() -> EvaluationSettings {
        EvaluationSettings {
            allow_mock: true,
            retry_backoff: Duration::ZERO,
            ..EvaluationSettings::default()
        }
    }

    /// One claim and finish under `settings`.
    pub async fn pass(
        pool: &PgPool,
        worker_id: &str,
        settings: &EvaluationSettings,
    ) -> Result<Option<JobOutcome>, StoreError> {
        let default_scenario = settings
            .allow_mock
            .then_some(settings.mock_scenario.as_deref())
            .flatten();
        let lease = i64::try_from(settings.lease_duration.as_secs().max(1)).unwrap_or(30);
        let Some(claim) = claim_job(pool, worker_id, lease, default_scenario).await? else {
            return Ok(None);
        };
        let outcome = Self::evaluate(&claim, settings);
        let backoff = i64::try_from(settings.retry_backoff.as_secs()).unwrap_or(0);
        let finished = finish_job(pool, worker_id, &claim, outcome, backoff).await?;
        tracing::info!(
            job = %claim.id,
            claim_generation = claim.claim_generation,
            attempt = claim.attempts,
            outcome = ?finished,
            "evaluation job finished"
        );
        Ok(Some(finished))
    }

    /// The evaluator call, outside any transaction. Only the scripted local/test evaluator exists.
    fn evaluate(
        claim: &ClaimedJob,
        settings: &EvaluationSettings,
    ) -> Option<crate::domain::moderation::EvaluatorOutcome> {
        scripted_outcome(claim, settings.allow_mock)
    }

    /// Supervised loop for a tokio task.
    pub async fn run(self, mut stopped: watch::Receiver<bool>) {
        if !self.settings.enabled {
            tracing::info!("evaluation worker disabled by configuration");
            return;
        }
        tracing::info!(
            worker_id = %self.worker_id,
            mock = self.settings.mock_scenario.as_deref().unwrap_or("none"),
            "evaluation worker started"
        );
        let mut backoff = self.settings.poll_min;
        loop {
            if *stopped.borrow() {
                tracing::info!("evaluation worker stopping on signal");
                break;
            }
            match Self::pass(&self.pool, &self.worker_id, &self.settings).await {
                Ok(Some(_)) => {
                    self.processed_count.fetch_add(1, Ordering::Relaxed);
                    backoff = self.settings.poll_min;
                }
                Ok(None) => {
                    tokio::select! {
                        _ = tokio::time::sleep(backoff) => {
                            backoff = (backoff * 2).min(self.settings.poll_max);
                        }
                        _ = stopped.changed() => {}
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "evaluation worker pass error");
                    tokio::select! {
                        _ = tokio::time::sleep(self.settings.poll_max) => {}
                        _ = stopped.changed() => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn consumer_is_off_unless_enabled() {
        let settings = EvaluationSettings::from_lookup(Profile::Local, lookup(&[])).unwrap();
        assert!(!settings.enabled);
        assert!(settings.allow_mock);
        let settings = EvaluationSettings::from_lookup(
            Profile::Production,
            lookup(&[("VINK_WORKER_EVALUATION", "enabled")]),
        )
        .unwrap();
        assert!(settings.enabled);
        assert!(
            !settings.allow_mock,
            "production never honours scripted verdicts"
        );
    }

    #[test]
    fn production_refuses_a_mock_scenario() {
        let error = EvaluationSettings::from_lookup(
            Profile::Production,
            lookup(&[
                ("VINK_WORKER_EVALUATION", "enabled"),
                ("VINK_MOCK_EVALUATION_SCENARIO", "approve"),
            ]),
        )
        .unwrap_err();
        assert!(error.contains("refused in production"), "{error}");
    }

    #[test]
    fn local_accepts_only_known_scenarios() {
        let settings = EvaluationSettings::from_lookup(
            Profile::Local,
            lookup(&[("VINK_MOCK_EVALUATION_SCENARIO", "reject")]),
        )
        .unwrap();
        assert_eq!(settings.mock_scenario.as_deref(), Some("reject"));
        assert!(
            EvaluationSettings::from_lookup(
                Profile::Local,
                lookup(&[("VINK_MOCK_EVALUATION_SCENARIO", "approve-all")]),
            )
            .is_err()
        );
    }
}
