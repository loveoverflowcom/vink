//! Background evaluation worker for work publication reviews (#159, MOD-01..08).
//!
//! Claims pending jobs from `work_evaluation_jobs`, evaluates them outside database locks,
//! and applies decisions atomically with publication fences and audit.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::application::ports::StoreError;
use crate::config::Profile;
use crate::domain::ids::{EvaluationId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::{
    EvaluationTarget, EvaluatorCapability, EvaluatorOutcome, EvaluatorResult, PolicyReasonCode,
    PolicyVersion, ScriptKind,
};
use crate::domain::publication::{Audience, PointerEffect};
use crate::domain::revision::ContentHash;
use crate::persistence::postgres::classify;
use sqlx::postgres::PgPool;
use sqlx::{query, query_as, query_scalar};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationSettings {
    pub enabled: bool,
    pub concurrency: usize,
    pub lease_duration: Duration,
    pub poll_min: Duration,
    pub poll_max: Duration,
    pub drain: Duration,
}

impl Default for EvaluationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            concurrency: 2,
            lease_duration: Duration::from_secs(30),
            poll_min: Duration::from_millis(500),
            poll_max: Duration::from_secs(5),
            drain: Duration::from_secs(10),
        }
    }
}

impl EvaluationSettings {
    pub fn from_lookup(
        profile: Profile,
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let mut settings = Self::default();
        if profile == Profile::Production {
            settings.enabled = false;
        }
        match get("VINK_WORKER_EVALUATION").as_deref().map(str::trim) {
            None => {}
            Some("" | "enabled") => settings.enabled = true,
            Some("disabled") => settings.enabled = false,
            Some(other) => {
                return Err(format!(
                    "VINK_WORKER_EVALUATION: expected `enabled` or `disabled`, got `{other}`"
                ));
            }
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

#[derive(sqlx::FromRow)]
struct ClaimedJob {
    id: Uuid,
    work_id: Uuid,
    revision_id: Uuid,
    policy_version: i64,
    content_digest: String,
    claim_generation: i64,
    attempts: i32,
    max_attempts: i32,
    scenario: Option<String>,
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

    /// Run one pass of claiming and evaluating a single job. Returns true if a job was handled.
    pub async fn run_once(pool: &PgPool, worker_id: &str) -> Result<bool, StoreError> {
        // 1. Claim one eligible job
        let mut tx = pool.begin().await.map_err(classify)?;

        let job: Option<ClaimedJob> = query_as(
            "SELECT id, work_id, revision_id, policy_version, content_digest,
                    claim_generation, attempts, max_attempts, scenario
               FROM work_evaluation_jobs
              WHERE available_at <= now()
                AND (state = 'open' OR (state = 'leased' AND lease_expires_at <= now()))
              ORDER BY available_at ASC
              LIMIT 1
              FOR UPDATE SKIP LOCKED",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;

        let Some(job) = job else {
            tx.rollback().await.map_err(classify)?;
            return Ok(false);
        };

        let next_generation = job.claim_generation + 1;
        let next_attempts = job.attempts + 1;

        query(
            "UPDATE work_evaluation_jobs
                SET state = 'leased',
                    claim_generation = $1,
                    attempts = $2,
                    claimed_by = $3,
                    lease_expires_at = now() + interval '30 seconds',
                    updated_at = now()
              WHERE id = $4",
        )
        .bind(next_generation)
        .bind(next_attempts)
        .bind(worker_id)
        .bind(job.id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;

        tx.commit().await.map_err(classify)?;

        // 2. Check automation mode outside transaction
        let auto_mode: Option<String> =
            query_scalar("SELECT mode FROM moderation_automation_state WHERE id = 1")
                .fetch_optional(pool)
                .await
                .map_err(classify)?;

        let is_paused = auto_mode.as_deref() == Some("paused");
        if is_paused {
            let mut tx = pool.begin().await.map_err(classify)?;
            query(
                "UPDATE work_evaluation_jobs
                    SET state = 'rerouted', updated_at = now()
                  WHERE id = $1",
            )
            .bind(job.id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            let owner_id: Option<Uuid> = query_scalar("SELECT owner_id FROM works WHERE id = $1")
                .bind(job.work_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;

            if let Some(owner) = owner_id {
                let dec_ver: i64 = query_scalar(
                    "SELECT decision_version FROM work_revision_moderation WHERE work_id = $1 AND revision_id = $2",
                )
                .bind(job.work_id)
                .bind(job.revision_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?
                .unwrap_or(1);

                query(
                    "INSERT INTO moderation_cases (
                         id, subject_kind, work_id, revision_id, content_digest, decision_version,
                         target_content_owner, target_work_owner, origin, version, state,
                         created_at, updated_at
                     )
                     VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'automation_paused', 1, 'queued', now(), now())
                     ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
                     DO NOTHING",
                )
                .bind(Uuid::now_v7())
                .bind(job.work_id)
                .bind(job.revision_id)
                .bind(&job.content_digest)
                .bind(dec_ver)
                .bind(owner)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
            }

            tx.commit().await.map_err(classify)?;
            return Ok(true);
        }

        // 3. Resolve scenario
        let env_scenario = std::env::var("VINK_MOCK_EVALUATION_SCENARIO").ok();
        let scenario_str = job
            .scenario
            .as_deref()
            .or(env_scenario.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let script_kind = match scenario_str {
            Some("approve") => Some(ScriptKind::Approve),
            Some("reject") => Some(ScriptKind::Reject),
            Some("needs_admin") => Some(ScriptKind::NeedsAdmin),
            Some("timeout") => Some(ScriptKind::Timeout),
            Some("provider_error") => Some(ScriptKind::ProviderError),
            _ => None,
        };

        // If no valid scenario: route to human review case
        let Some(kind) = script_kind else {
            let mut tx = pool.begin().await.map_err(classify)?;
            query(
                "UPDATE work_evaluation_jobs
                    SET state = 'rerouted', updated_at = now()
                  WHERE id = $1",
            )
            .bind(job.id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            let owner_id: Option<Uuid> = query_scalar("SELECT owner_id FROM works WHERE id = $1")
                .bind(job.work_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;

            if let Some(owner) = owner_id {
                let dec_ver: i64 = query_scalar(
                    "SELECT decision_version FROM work_revision_moderation WHERE work_id = $1 AND revision_id = $2",
                )
                .bind(job.work_id)
                .bind(job.revision_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?
                .unwrap_or(1);

                query(
                    "INSERT INTO moderation_cases (
                         id, subject_kind, work_id, revision_id, content_digest, decision_version,
                         target_content_owner, target_work_owner, origin, version, state,
                         created_at, updated_at
                     )
                     VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'evaluator_uncertain', 1, 'queued', now(), now())
                     ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
                     DO NOTHING",
                )
                .bind(Uuid::now_v7())
                .bind(job.work_id)
                .bind(job.revision_id)
                .bind(&job.content_digest)
                .bind(dec_ver)
                .bind(owner)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
            }

            tx.commit().await.map_err(classify)?;
            return Ok(true);
        };

        // 4. Handle Timeout / ProviderError
        if matches!(kind, ScriptKind::Timeout | ScriptKind::ProviderError) {
            let mut tx = pool.begin().await.map_err(classify)?;
            let err_name = match kind {
                ScriptKind::Timeout => "timeout",
                ScriptKind::ProviderError => "provider_error",
                _ => unreachable!(),
            };

            if next_attempts < job.max_attempts {
                // Retry with backoff
                query(
                    "UPDATE work_evaluation_jobs
                        SET state = 'open',
                            available_at = now() + (interval '1 second' * $1),
                            last_error = $2,
                            updated_at = now()
                      WHERE id = $3",
                )
                .bind(next_attempts * 2)
                .bind(err_name)
                .bind(job.id)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
            } else {
                // Retries exhausted: fail job and open human case
                query(
                    "UPDATE work_evaluation_jobs
                        SET state = 'failed',
                            last_error = 'retries_exhausted',
                            updated_at = now()
                      WHERE id = $1",
                )
                .bind(job.id)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;

                let owner_id: Option<Uuid> =
                    query_scalar("SELECT owner_id FROM works WHERE id = $1")
                        .bind(job.work_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(classify)?;

                if let Some(owner) = owner_id {
                    let dec_ver: i64 = query_scalar(
                        "SELECT decision_version FROM work_revision_moderation WHERE work_id = $1 AND revision_id = $2",
                    )
                    .bind(job.work_id)
                    .bind(job.revision_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?
                    .unwrap_or(1);

                    query(
                        "INSERT INTO moderation_cases (
                             id, subject_kind, work_id, revision_id, content_digest, decision_version,
                             target_content_owner, target_work_owner, origin, version, state,
                             created_at, updated_at
                         )
                         VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'retries_exhausted', 1, 'queued', now(), now())
                         ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
                         DO NOTHING",
                    )
                    .bind(Uuid::now_v7())
                    .bind(job.work_id)
                    .bind(job.revision_id)
                    .bind(&job.content_digest)
                    .bind(dec_ver)
                    .bind(owner)
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?;
                }
            }

            tx.commit().await.map_err(classify)?;
            return Ok(true);
        }

        // 5. Apply Evaluator Decision (Approve, Reject, or NeedsAdmin) in transaction
        let mut tx = pool.begin().await.map_err(classify)?;

        // Lock work_publications FOR UPDATE
        let _: Option<Uuid> =
            query_scalar("SELECT work_id FROM work_publications WHERE work_id = $1 FOR UPDATE")
                .bind(job.work_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;

        let owner_id: Uuid = query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
            .bind(job.work_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;

        let work_id = WorkId::from_uuid(job.work_id);
        let rev_id = RevisionId::from_uuid(job.revision_id);
        let eval_id = EvaluationId::from_uuid(job.id);

        let (pub_state, mod_state, _) = crate::persistence::publication::load_paired_state(
            &mut tx,
            PrincipalId::from_uuid(owner_id),
            work_id,
        )
        .await?;

        // If author set private, withdraw the job
        if pub_state.facts().audience == Audience::Private {
            query(
                "UPDATE work_evaluation_jobs
                    SET state = 'withdrawn', updated_at = now()
                  WHERE id = $1",
            )
            .bind(job.id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            tx.commit().await.map_err(classify)?;
            return Ok(true);
        }

        // Needs admin
        if kind == ScriptKind::NeedsAdmin {
            query(
                "UPDATE work_evaluation_jobs
                    SET state = 'rerouted', updated_at = now()
                  WHERE id = $1",
            )
            .bind(job.id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            let dec_ver = mod_state
                .facts()
                .revisions
                .iter()
                .find(|r| r.revision == rev_id)
                .map(|r| r.version.get())
                .unwrap_or(1);

            query(
                "INSERT INTO moderation_cases (
                     id, subject_kind, work_id, revision_id, content_digest, decision_version,
                     target_content_owner, target_work_owner, origin, version, state,
                     created_at, updated_at
                 )
                 VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'evaluator_uncertain', 1, 'queued', now(), now())
                 ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
                 DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(job.work_id)
            .bind(job.revision_id)
            .bind(&job.content_digest)
            .bind(dec_ver)
            .bind(owner_id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            tx.commit().await.map_err(classify)?;
            return Ok(true);
        }

        // Evaluate decision outcome
        let outcome = match kind {
            ScriptKind::Approve => EvaluatorOutcome::Approve,
            ScriptKind::Reject => EvaluatorOutcome::Reject(
                PolicyReasonCode::parse("prohibited_content")
                    .unwrap_or(PolicyReasonCode::SafetyPolicy),
            ),
            _ => unreachable!(),
        };

        let digest = ContentHash::parse(&job.content_digest).ok_or_else(|| {
            crate::persistence::postgres::integrity("invalid digest in job", &job.content_digest)
        })?;

        let result = EvaluatorResult {
            evaluation_id: eval_id,
            target: EvaluationTarget::WorkRevision {
                work: work_id,
                revision: rev_id,
                digest,
            },
            policy: PolicyVersion::new(job.policy_version).ok_or_else(|| {
                crate::persistence::postgres::integrity(
                    "invalid policy version",
                    job.policy_version,
                )
            })?,
            outcome,
        };

        let trans = match mod_state.apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &result,
        ) {
            Ok(t) => t,
            Err(e) => {
                // If decision version already moved or job superseded, withdraw job
                query(
                    "UPDATE work_evaluation_jobs
                        SET state = 'withdrawn', last_error = $1, updated_at = now()
                      WHERE id = $2",
                )
                .bind(format!("{e:?}"))
                .bind(job.id)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;

                tx.commit().await.map_err(classify)?;
                return Ok(true);
            }
        };

        let next_mod = trans.moderation.facts();
        let next_pub = trans.publication.facts();
        let next_dec = next_mod
            .revisions
            .iter()
            .find(|r| r.revision == rev_id)
            .ok_or_else(|| {
                crate::persistence::postgres::integrity(
                    "revision missing in moderation facts",
                    rev_id.to_string(),
                )
            })?;

        // Update work_revision_moderation
        let (mod_status, prov_kind, policy_reason) = match kind {
            ScriptKind::Approve => ("approved", "ai_approved", None),
            ScriptKind::Reject => ("rejected", "ai_rejected", Some("safety_policy")),
            _ => unreachable!(),
        };

        query(
            "UPDATE work_revision_moderation
                SET decision_version = $1,
                    moderation = $2,
                    provenance_kind = $3,
                    policy_reason_code = $4,
                    policy_version = $5,
                    updated_at = now()
              WHERE work_id = $6 AND revision_id = $7",
        )
        .bind(next_dec.version.get())
        .bind(mod_status)
        .bind(prov_kind)
        .bind(policy_reason)
        .bind(next_dec.policy.get())
        .bind(job.work_id)
        .bind(job.revision_id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;

        // Update work_publications
        let pointer_rev = next_pub.pointer.map(|p| p.revision.as_uuid());
        let pointer_gen = next_pub.pointer.map(|p| p.intent_generation.get());
        let withdrawn_rev = next_pub.review_withdrawn.map(|p| p.revision.as_uuid());
        let withdrawn_gen = next_pub.review_withdrawn.map(|p| p.intent_generation.get());
        let ever_public_uuids: Vec<Uuid> =
            next_pub.ever_public.iter().map(|r| r.as_uuid()).collect();

        query(
            "UPDATE work_publications
                SET pointer_revision_id = $1,
                    pointer_intent_generation = $2,
                    review_withdrawn_revision_id = $3,
                    review_withdrawn_intent_generation = $4,
                    ever_public = $5,
                    public_epoch = $6,
                    updated_at = now()
              WHERE work_id = $7",
        )
        .bind(pointer_rev)
        .bind(pointer_gen)
        .bind(withdrawn_rev)
        .bind(withdrawn_gen)
        .bind(&ever_public_uuids)
        .bind(next_pub.public_epoch.get())
        .bind(job.work_id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;

        // Pointer effect on publications table
        match trans.effects.pointer {
            PointerEffect::Set(p) => {
                let meta_id: Option<Uuid> = query_scalar(
                    "SELECT metadata_snapshot_id FROM revisions WHERE id = $1 AND work_id = $2",
                )
                .bind(p.revision.as_uuid())
                .bind(job.work_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;

                query(
                    "INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
                     VALUES ($1, $2, $3, now(), $4)
                     ON CONFLICT (work_id) DO UPDATE SET
                         revision_id = EXCLUDED.revision_id,
                         generation = EXCLUDED.generation,
                         published_at = EXCLUDED.published_at,
                         metadata_snapshot_id = EXCLUDED.metadata_snapshot_id",
                )
                .bind(job.work_id)
                .bind(p.revision.as_uuid())
                .bind(p.intent_generation.get())
                .bind(meta_id)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
            }
            PointerEffect::Clear => {
                query("DELETE FROM publications WHERE work_id = $1")
                    .bind(job.work_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?;
            }
            PointerEffect::Unchanged => {}
        }

        // Mark job applied
        query(
            "UPDATE work_evaluation_jobs
                SET state = 'applied', updated_at = now()
              WHERE id = $1",
        )
        .bind(job.id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;

        tx.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Supervised loop for tokio task.
    pub async fn run(self, mut stopped: watch::Receiver<bool>) {
        if !self.settings.enabled {
            tracing::info!("evaluation worker disabled by configuration");
            return;
        }

        tracing::info!(worker_id = %self.worker_id, "evaluation worker started");
        let mut backoff = self.settings.poll_min;

        loop {
            if *stopped.borrow() {
                tracing::info!("evaluation worker stopping on signal");
                break;
            }

            match Self::run_once(&self.pool, &self.worker_id).await {
                Ok(true) => {
                    self.processed_count.fetch_add(1, Ordering::Relaxed);
                    backoff = self.settings.poll_min;
                }
                Ok(false) => {
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
