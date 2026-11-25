//! Background evaluation worker for work publication reviews (#159, #536, MOD-01..09, MOD-14).
//!
//! Claims jobs from `work_evaluation_jobs`, asks the evaluator outside any database lock, and
//! applies the answer through [`crate::persistence::evaluation::finish_job`], which re-checks
//! the claim, the automation mode and the decision fence in the applying commit.
//!
//! No real AI provider exists. The owner's bootstrap of 25/11/2026 (#536) runs the consumer with
//! the scripted mock by default in every profile, production included: the mock reads no content,
//! calls no network and approves each valid submission (`approve` bound to the job on its first
//! claim). It is not content moderation. Three switches stay apart:
//!
//! - `VINK_WORKER_EVALUATION=disabled` stops the consumer (technical stop): new jobs wait `open`
//!   and nobody reviews them until it runs again.
//! - `VINK_EVALUATOR=manual` keeps the consumer running without an evaluator: every claimed job
//!   goes to the human queue and a scenario bound earlier is never reused.
//! - An admin pause (MOD-14, `moderation_automation_state`) is the business stop. It lives in the
//!   database, wins over this configuration and survives restarts; the worker never writes it.
//!
//! The local profile also honours the other scripted verdicts (`VINK_MOCK_EVALUATION_SCENARIO`);
//! production accepts only `approve`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::application::ports::StoreError;
use crate::config::Profile;
use crate::persistence::evaluation::{
    ClaimedJob, JobOutcome, MOCK_SCENARIOS, PRODUCTION_MOCK_SCENARIOS, claim_job, finish_job,
    scripted_outcome,
};
use sqlx::postgres::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

/// Who gives the verdict for a claimed job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evaluator {
    /// The scripted mock: the job-bound scenario decides; no content is read.
    Mock,
    /// No evaluator: the human queue decides every job.
    Manual,
}

impl Evaluator {
    /// Label for logs and operations; never claims an AI review.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "scripted_mock",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationSettings {
    /// Whether the consumer runs (`VINK_WORKER_EVALUATION`).
    pub enabled: bool,
    pub lease_duration: Duration,
    pub retry_backoff: Duration,
    pub poll_min: Duration,
    pub poll_max: Duration,
    /// Who gives the verdict (`VINK_EVALUATOR`).
    pub evaluator: Evaluator,
    /// Job-bound scripted verdicts honoured in mock mode (production: `approve` only).
    pub mock_scenarios: &'static [&'static str],
    /// Scripted verdict bound to a job on its first claim in mock mode.
    pub mock_scenario: Option<String>,
}

impl Default for EvaluationSettings {
    /// The production defaults of the #536 bootstrap: consumer on, mock approve.
    fn default() -> Self {
        Self {
            enabled: true,
            lease_duration: Duration::from_secs(30),
            retry_backoff: Duration::from_secs(2),
            poll_min: Duration::from_millis(500),
            poll_max: Duration::from_secs(5),
            evaluator: Evaluator::Mock,
            mock_scenarios: &PRODUCTION_MOCK_SCENARIOS,
            mock_scenario: Some("approve".to_owned()),
        }
    }
}

impl EvaluationSettings {
    pub fn from_lookup(
        profile: Profile,
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let honoured: &'static [&'static str] = match profile {
            Profile::Production => &PRODUCTION_MOCK_SCENARIOS,
            Profile::Local => &MOCK_SCENARIOS,
        };
        let mut settings = Self {
            mock_scenarios: honoured,
            ..Self::default()
        };
        match get("VINK_WORKER_EVALUATION").as_deref().map(str::trim) {
            None | Some("" | "enabled") => settings.enabled = true,
            Some("disabled") => settings.enabled = false,
            Some(other) => {
                return Err(format!(
                    "VINK_WORKER_EVALUATION: expected `enabled` or `disabled`, got `{other}`"
                ));
            }
        }
        match get("VINK_EVALUATOR").as_deref().map(str::trim) {
            None | Some("" | "mock") => settings.evaluator = Evaluator::Mock,
            Some("manual") => settings.evaluator = Evaluator::Manual,
            Some(other) => {
                return Err(format!(
                    "VINK_EVALUATOR: expected `mock` or `manual`, got `{other}`"
                ));
            }
        }
        let scenario = get("VINK_MOCK_EVALUATION_SCENARIO")
            .map(|raw| raw.trim().to_owned())
            .filter(|raw| !raw.is_empty());
        match (settings.evaluator, scenario) {
            (Evaluator::Manual, Some(_)) => {
                return Err(
                    "VINK_MOCK_EVALUATION_SCENARIO: set only with `VINK_EVALUATOR=mock`; \
                     `manual` sends every job to people"
                        .to_owned(),
                );
            }
            (Evaluator::Manual, None) => {
                settings.mock_scenarios = &[];
                settings.mock_scenario = None;
            }
            (Evaluator::Mock, None) => {}
            (Evaluator::Mock, Some(scenario)) => {
                if !honoured.contains(&scenario.as_str()) {
                    return Err(format!(
                        "VINK_MOCK_EVALUATION_SCENARIO: expected one of {} in the {} profile, got `{scenario}`",
                        honoured.join(", "),
                        match profile {
                            Profile::Production => "production",
                            Profile::Local => "local",
                        }
                    ));
                }
                settings.mock_scenario = Some(scenario);
            }
        }
        Ok(settings)
    }

    /// The scenario bound to a job on its first claim (none in manual mode).
    pub fn default_scenario(&self) -> Option<&str> {
        match self.evaluator {
            Evaluator::Mock => self.mock_scenario.as_deref(),
            Evaluator::Manual => None,
        }
    }

    /// The job-bound verdicts honoured now (none in manual mode).
    pub fn honoured_scenarios(&self) -> &'static [&'static str] {
        match self.evaluator {
            Evaluator::Mock => self.mock_scenarios,
            Evaluator::Manual => &[],
        }
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

    /// One claim and finish with test-friendly settings (every scripted verdict honoured, no
    /// default scenario, no retry backoff). Returns whether a job was claimed.
    pub async fn run_once(pool: &PgPool, worker_id: &str) -> Result<bool, StoreError> {
        Ok(Self::pass(pool, worker_id, &Self::test_settings())
            .await?
            .is_some())
    }

    /// Settings for tests: every scripted verdict honoured, no default scenario (a job without a
    /// bound scenario goes to people), no retry backoff.
    pub fn test_settings() -> EvaluationSettings {
        EvaluationSettings {
            mock_scenarios: &MOCK_SCENARIOS,
            mock_scenario: None,
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
        let lease = i64::try_from(settings.lease_duration.as_secs().max(1)).unwrap_or(30);
        let Some(claim) = claim_job(pool, worker_id, lease, settings.default_scenario()).await?
        else {
            return Ok(None);
        };
        let outcome = Self::evaluate(&claim, settings);
        let backoff = i64::try_from(settings.retry_backoff.as_secs()).unwrap_or(0);
        let finished = finish_job(pool, worker_id, &claim, outcome, backoff).await?;
        tracing::info!(
            job = %claim.id,
            claim_generation = claim.claim_generation,
            attempt = claim.attempts,
            evaluator = settings.evaluator.as_str(),
            scripted_verdict = outcome.is_some(),
            scenario = claim.scenario.as_deref().unwrap_or("none"),
            outcome = ?finished,
            "evaluation job finished"
        );
        Ok(Some(finished))
    }

    /// The evaluator call, outside any transaction. Only the scripted mock exists.
    fn evaluate(
        claim: &ClaimedJob,
        settings: &EvaluationSettings,
    ) -> Option<crate::domain::moderation::EvaluatorOutcome> {
        scripted_outcome(claim, settings.honoured_scenarios())
    }

    /// Supervised loop for a tokio task.
    pub async fn run(self, mut stopped: watch::Receiver<bool>) {
        if !self.settings.enabled {
            tracing::warn!(
                "evaluation consumer disabled by configuration: new submissions wait as open jobs \
                 and reach nobody; use VINK_EVALUATOR=manual or an admin pause to route them to people"
            );
            return;
        }
        tracing::info!(
            worker_id = %self.worker_id,
            evaluator = self.settings.evaluator.as_str(),
            mock = self.settings.default_scenario().unwrap_or("none"),
            ai_moderation = "not_implemented",
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

    fn settings(
        profile: Profile,
        pairs: &'static [(&'static str, &'static str)],
    ) -> EvaluationSettings {
        EvaluationSettings::from_lookup(profile, lookup(pairs)).unwrap()
    }

    fn error(profile: Profile, pairs: &'static [(&'static str, &'static str)]) -> String {
        EvaluationSettings::from_lookup(profile, lookup(pairs)).unwrap_err()
    }

    /// #536 AC-01: without any variable every profile runs the consumer with mock approve.
    #[test]
    fn mock_approve_runs_by_default_in_every_profile() {
        for profile in [Profile::Production, Profile::Local] {
            let settings = settings(profile, &[]);
            assert!(settings.enabled, "{profile:?}");
            assert_eq!(settings.evaluator, Evaluator::Mock);
            assert_eq!(settings.default_scenario(), Some("approve"));
            assert!(settings.honoured_scenarios().contains(&"approve"));
        }
        let empty = settings(
            Profile::Production,
            &[
                ("VINK_WORKER_EVALUATION", " "),
                ("VINK_EVALUATOR", ""),
                ("VINK_MOCK_EVALUATION_SCENARIO", ""),
            ],
        );
        assert_eq!(empty, settings(Profile::Production, &[]));
        assert_eq!(
            settings(Profile::Production, &[]),
            EvaluationSettings::default(),
            "the production defaults are the bootstrap defaults"
        );
    }

    #[test]
    fn production_honours_only_approve() {
        assert_eq!(
            settings(Profile::Production, &[]).honoured_scenarios(),
            &["approve"]
        );
        let explicit = settings(
            Profile::Production,
            &[("VINK_MOCK_EVALUATION_SCENARIO", "approve")],
        );
        assert_eq!(explicit, settings(Profile::Production, &[]));
        for scenario in [
            "reject",
            "needs_admin",
            "invalid_output",
            "timeout",
            "provider_error",
        ] {
            let error = EvaluationSettings::from_lookup(Profile::Production, |key| {
                (key == "VINK_MOCK_EVALUATION_SCENARIO").then(|| scenario.to_owned())
            })
            .unwrap_err();
            assert!(
                error.contains("expected one of approve in the production profile"),
                "{error}"
            );
        }
    }

    #[test]
    fn local_accepts_only_known_scenarios() {
        let settings = settings(
            Profile::Local,
            &[("VINK_MOCK_EVALUATION_SCENARIO", "reject")],
        );
        assert_eq!(settings.default_scenario(), Some("reject"));
        assert_eq!(settings.honoured_scenarios(), &MOCK_SCENARIOS);
        let error = error(
            Profile::Local,
            &[("VINK_MOCK_EVALUATION_SCENARIO", "approve-all")],
        );
        assert!(error.contains("approve-all"), "{error}");
    }

    /// Manual keeps the consumer running, binds nothing and honours no bound scenario.
    #[test]
    fn manual_mode_has_no_evaluator() {
        for profile in [Profile::Production, Profile::Local] {
            let settings = settings(profile, &[("VINK_EVALUATOR", "manual")]);
            assert!(settings.enabled);
            assert_eq!(settings.evaluator, Evaluator::Manual);
            assert_eq!(settings.default_scenario(), None);
            assert!(settings.honoured_scenarios().is_empty());
        }
        let error = error(
            Profile::Local,
            &[
                ("VINK_EVALUATOR", "manual"),
                ("VINK_MOCK_EVALUATION_SCENARIO", "approve"),
            ],
        );
        assert!(error.contains("only with `VINK_EVALUATOR=mock`"), "{error}");
    }

    #[test]
    fn consumer_stops_only_when_disabled_and_invalid_values_fail_startup() {
        let settings = settings(
            Profile::Production,
            &[("VINK_WORKER_EVALUATION", "disabled")],
        );
        assert!(!settings.enabled);
        for (key, value) in [
            ("VINK_WORKER_EVALUATION", "off"),
            ("VINK_WORKER_EVALUATION", "true"),
            ("VINK_EVALUATOR", "ai"),
            ("VINK_EVALUATOR", "none"),
        ] {
            let result = EvaluationSettings::from_lookup(Profile::Production, |k| {
                (k == key).then(|| value.to_owned())
            });
            let error = result.unwrap_err();
            assert!(error.starts_with(key), "{error}");
        }
    }

    #[test]
    fn labels_never_claim_an_ai_review() {
        assert_eq!(Evaluator::Mock.as_str(), "scripted_mock");
        assert_eq!(Evaluator::Manual.as_str(), "manual");
    }
}
