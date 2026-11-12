//! Evaluation job execution against PostgreSQL (#159, MOD-01..08, MOD-14).
//!
//! A job is claimed in a short transaction (lease, `claim_generation`, attempt count). The
//! evaluator runs outside any transaction. The result is then applied in one commit that takes
//! the shared lock order (see [`super::moderation_write`]), reads the automation mode under a
//! share lock and re-checks that the job is still `leased` by this exact claim. A reclaim after
//! the lease expired bumps `claim_generation`, so a late finisher of the old claim changes
//! nothing. A pause, a private withdrawal or a human decision committed first is seen here and
//! the stale result has no effect.

use sqlx::{PgPool, query, query_as};
use uuid::Uuid;

use super::moderation_write::{
    DecisionSource, apply_pointer_effect, lock_work, locked_automation_mode, open_work_case,
    sync_job_states, write_decision, write_publication_pointers,
};
use super::postgres::{classify, integrity};
use crate::application::ports::StoreError;
use crate::domain::ids::{EvaluationId, RevisionId, WorkId};
use crate::domain::moderation::automation::AutomationMode;
use crate::domain::moderation::case::CaseOrigin;
use crate::domain::moderation::{
    AppliedEvaluation, EvaluationRefusal, EvaluationTarget, EvaluatorCapability, EvaluatorOutcome,
    EvaluatorResult, ModerationError, PolicyReasonCode, PolicyVersion,
};
use crate::domain::revision::ContentHash;

/// A claimed job: the exact binding the finisher must still hold at commit.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub work_id: Uuid,
    pub revision_id: Uuid,
    pub policy_version: i64,
    pub content_digest: String,
    pub claim_generation: i64,
    pub attempts: i32,
    pub max_attempts: i32,
    pub scenario: Option<String>,
}

/// What finishing a claim did (for logs and tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    /// The claim no longer holds (reclaimed, rerouted, withdrawn or decided elsewhere).
    Stale,
    Applied(AppliedEvaluation),
    /// A retryable failure: a fresh evaluation id was opened for the same submission.
    Retried,
    /// The attempts of this submission are spent: the revision stays pending in the human queue.
    Exhausted,
    /// No evaluator verdict (no provider, automation paused): the human queue decides.
    Rerouted,
    /// The core refused the result; the job is closed with the refusal code.
    Refused(EvaluationRefusal),
}

/// Mock verdicts accepted only where the deployment allows a scripted evaluator (local/test).
pub const MOCK_SCENARIOS: [&str; 6] = [
    "approve",
    "reject",
    "needs_admin",
    "invalid_output",
    "timeout",
    "provider_error",
];

/// Claim the oldest eligible job while automation is enabled. A job whose lease expired is
/// reclaimed with the same evaluation id and the next `claim_generation`; every claim counts as
/// one attempt. `default_scenario` binds a mock verdict to the job durably on its first claim.
pub async fn claim_job(
    pool: &PgPool,
    worker_id: &str,
    lease_seconds: i64,
    default_scenario: Option<&str>,
) -> Result<Option<ClaimedJob>, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let job: Option<ClaimedJob> = query_as(
        "WITH next AS (
             SELECT j.id
               FROM work_evaluation_jobs j
              WHERE j.available_at <= now()
                AND (j.state = 'open' OR (j.state = 'leased' AND j.lease_expires_at <= now()))
                AND EXISTS (SELECT 1 FROM moderation_automation_state a
                             WHERE a.id = 1 AND a.mode = 'enabled')
              ORDER BY j.available_at, j.id
              LIMIT 1
              FOR UPDATE SKIP LOCKED
         )
         UPDATE work_evaluation_jobs j
            SET state = 'leased',
                claim_generation = j.claim_generation + 1,
                attempts = j.attempts + 1,
                claimed_by = $1,
                lease_expires_at = now() + make_interval(secs => $2),
                scenario = COALESCE(j.scenario, $3),
                updated_at = now()
           FROM next
          WHERE j.id = next.id
      RETURNING j.id, j.work_id, j.revision_id, j.policy_version, j.content_digest,
                j.claim_generation, j.attempts, j.max_attempts, j.scenario",
    )
    .bind(worker_id)
    .bind(lease_seconds as f64)
    .bind(default_scenario)
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    tx.commit().await.map_err(classify)?;
    Ok(job)
}

/// The scripted verdict for a claim. Without an allowed mock there is no evaluator: `None`.
pub fn scripted_outcome(claim: &ClaimedJob, allow_mock: bool) -> Option<EvaluatorOutcome> {
    if !allow_mock {
        return None;
    }
    match claim.scenario.as_deref().map(str::trim)? {
        "approve" => Some(EvaluatorOutcome::Approve),
        "reject" => Some(EvaluatorOutcome::Reject(PolicyReasonCode::SafetyPolicy)),
        "needs_admin" => Some(EvaluatorOutcome::Uncertain),
        "invalid_output" => Some(EvaluatorOutcome::InvalidOutput),
        "timeout" => Some(EvaluatorOutcome::Timeout),
        "provider_error" => Some(EvaluatorOutcome::ProviderError),
        _ => None,
    }
}

/// Apply the evaluator's answer for `claim` (`None`: no verdict, route to people) in one commit.
pub async fn finish_job(
    pool: &PgPool,
    worker_id: &str,
    claim: &ClaimedJob,
    outcome: Option<EvaluatorOutcome>,
    retry_backoff_seconds: i64,
) -> Result<JobOutcome, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let work = WorkId::from_uuid(claim.work_id);
    let revision = RevisionId::from_uuid(claim.revision_id);
    let evaluation = EvaluationId::from_uuid(claim.id);

    let Some(owner) = lock_work(&mut tx, work).await? else {
        // The work is gone (its jobs cascade); nothing to apply.
        tx.rollback().await.map_err(classify)?;
        return Ok(JobOutcome::Stale);
    };
    let mode = locked_automation_mode(&mut tx).await?;

    // The claim fence: still leased by this exact claim.
    let held: Option<(String, i64, Option<String>)> = query_as(
        "SELECT state, claim_generation, claimed_by
           FROM work_evaluation_jobs WHERE id = $1 FOR UPDATE",
    )
    .bind(claim.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    let holds = held.as_ref().is_some_and(|(state, generation, by)| {
        state == "leased"
            && *generation == claim.claim_generation
            && by.as_deref() == Some(worker_id)
    });
    if !holds {
        tx.rollback().await.map_err(classify)?;
        return Ok(JobOutcome::Stale);
    }

    let (publication, moderation, _) =
        super::publication::load_paired_state(&mut tx, owner, work).await?;

    // No verdict (no evaluator configured) or paused: the human queue decides; the revision stays
    // pending and nothing is decided on the evaluator's behalf.
    let outcome = match (mode, outcome) {
        (AutomationMode::Paused, _) => None,
        (_, outcome) => outcome,
    };
    let Some(outcome) = outcome else {
        let public = publication.facts().audience == crate::domain::publication::Audience::Public
            && publication.facts().lifecycle.is_active();
        let (rerouted, undecided) = moderation.reroute_to_humans();
        query(
            "UPDATE work_evaluation_jobs
                SET state = $1, lease_expires_at = NULL, updated_at = now()
              WHERE id = $2",
        )
        .bind(if public { "rerouted" } else { "withdrawn" })
        .bind(claim.id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        if public && undecided.contains(&revision) {
            let origin = if mode == AutomationMode::Paused {
                CaseOrigin::AutomationPaused
            } else {
                CaseOrigin::EvaluatorUncertain
            };
            open_work_case(&mut tx, rerouted.facts(), owner, revision, origin).await?;
        }
        tx.commit().await.map_err(classify)?;
        return Ok(JobOutcome::Rerouted);
    };

    let digest = ContentHash::parse(&claim.content_digest)
        .ok_or_else(|| integrity("invalid digest in job", &claim.content_digest))?;
    let policy = PolicyVersion::new(claim.policy_version)
        .ok_or_else(|| integrity("invalid policy version in job", claim.policy_version))?;
    let result = EvaluatorResult {
        evaluation_id: evaluation,
        target: EvaluationTarget::WorkRevision {
            work,
            revision,
            digest,
        },
        policy,
        outcome,
    };

    let transition = match moderation.apply_evaluator_result(
        &publication,
        Some(EvaluatorCapability::under(mode)),
        &result,
    ) {
        Ok(t) => t,
        Err(ModerationError::Refused(refusal)) => {
            // The result has no effect on decisions or pointers; the job closes with the reason.
            let state = match refusal {
                EvaluationRefusal::Withdrawn => "withdrawn",
                EvaluationRefusal::AutomationPaused => "rerouted",
                EvaluationRefusal::TargetGone
                | EvaluationRefusal::Overridden
                | EvaluationRefusal::StalePolicy
                | EvaluationRefusal::Superseded => "withdrawn",
                _ => "quarantined",
            };
            query(
                "UPDATE work_evaluation_jobs
                    SET state = $1, last_error = $2, lease_expires_at = NULL, updated_at = now()
                  WHERE id = $3",
            )
            .bind(state)
            .bind(refusal.as_str())
            .bind(claim.id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            tx.commit().await.map_err(classify)?;
            return Ok(JobOutcome::Refused(refusal));
        }
        Err(other) => {
            tx.rollback().await.map_err(classify)?;
            return Err(integrity("evaluator result rejected", format!("{other:?}")));
        }
    };

    if transition.outcome == AppliedEvaluation::Retryable {
        let error = match outcome {
            EvaluatorOutcome::Timeout => "timeout",
            _ => "provider_error",
        };
        query(
            "UPDATE work_evaluation_jobs
                SET state = 'failed', last_error = $1, lease_expires_at = NULL, updated_at = now()
              WHERE id = $2",
        )
        .bind(error)
        .bind(claim.id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        if claim.attempts < claim.max_attempts {
            // A business retry opens a fresh evaluation id for the same submission and policy;
            // the attempt count carries over (3 per submission and policy by default).
            let (_, ticket) = transition
                .moderation
                .reevaluate(&transition.publication, revision, EvaluationId::generate())
                .map_err(|e| integrity("reevaluate after retryable failure", format!("{e:?}")))?;
            query(
                "INSERT INTO work_evaluation_jobs (
                     id, work_id, revision_id, content_digest, policy_version, state,
                     claim_generation, attempts, max_attempts, scenario, available_at,
                     created_at, updated_at
                 )
                 VALUES ($1, $2, $3, $4, $5, 'open', 0, $6, $7, $8,
                         now() + make_interval(secs => $9), now(), now())",
            )
            .bind(ticket.id.as_uuid())
            .bind(claim.work_id)
            .bind(claim.revision_id)
            .bind(&claim.content_digest)
            .bind(ticket.policy.get())
            .bind(claim.attempts)
            .bind(claim.max_attempts)
            .bind(claim.scenario.as_deref())
            .bind((retry_backoff_seconds * i64::from(claim.attempts)) as f64)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            tx.commit().await.map_err(classify)?;
            return Ok(JobOutcome::Retried);
        }
        open_work_case(
            &mut tx,
            transition.moderation.facts(),
            owner,
            revision,
            CaseOrigin::RetriesExhausted,
        )
        .await?;
        tx.commit().await.map_err(classify)?;
        return Ok(JobOutcome::Exhausted);
    }

    // A decided outcome: decision, history, pointers and job state in this commit.
    let next_pub = transition.publication.facts();
    let next_mod = transition.moderation.facts();
    write_decision(
        &mut tx,
        next_pub,
        next_mod,
        revision,
        DecisionSource {
            appeal: None,
            evaluation: Some(evaluation),
            case: None,
        },
    )
    .await?;
    write_publication_pointers(&mut tx, next_pub).await?;
    apply_pointer_effect(&mut tx, work, transition.effects.pointer).await?;
    sync_job_states(&mut tx, next_mod).await?;
    if let AppliedEvaluation::NeedsAdmin { cause } = transition.outcome {
        let origin = match cause {
            crate::domain::moderation::NeedsAdminCause::Uncertain => CaseOrigin::EvaluatorUncertain,
            crate::domain::moderation::NeedsAdminCause::InvalidOutput => CaseOrigin::InvalidOutput,
        };
        open_work_case(&mut tx, next_mod, owner, revision, origin).await?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(JobOutcome::Applied(transition.outcome))
}
