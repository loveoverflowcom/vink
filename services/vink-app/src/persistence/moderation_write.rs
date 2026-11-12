//! Shared write helpers for work moderation (#159): one lock order, one decision writer and one
//! case writer for the author path, the evaluation worker and human reviewers.
//!
//! Lock order (same as the author commands in `publication.rs`, discussion and endorsement):
//! `publications` FOR UPDATE -> `work_publications` FOR UPDATE -> `works` FOR SHARE ->
//! `principals` FOR UPDATE (standing fence) -> `moderation_automation_state` FOR SHARE ->
//! `work_evaluation_jobs` FOR UPDATE -> `moderation_cases` FOR UPDATE. Every path that touches
//! a work's moderation takes the prefix it needs in this order, so no two paths wait on each
//! other in opposite directions. `work_publications` always has a row once a revision was
//! submitted (request-public writes it), so the work lock never degenerates to nothing for a
//! work that has jobs or cases.

use sqlx::{PgConnection, query, query_as, query_scalar};
use time::OffsetDateTime;
use uuid::Uuid;

use super::postgres::{classify, integrity};
use crate::application::ports::StoreError;
use crate::domain::ids::{CaseId, EvaluationId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::authority::{AuthorityBasis, ModerationDirectory};
use crate::domain::moderation::automation::AutomationMode;
use crate::domain::moderation::case::{CaseOrigin, CaseState, CaseVerdict, ReviewCase};
use crate::domain::moderation::{
    AdminVerdict, DecisionProvenance, JobState, NeedsAdminCause, RevisionDecisionFacts,
    WorkModerationFacts,
};
use crate::domain::publication::{Moderation, PointerEffect, PublicationFacts};

/// Take the work prefix of the lock order. Returns the owner, or `None` for an unknown work.
pub(crate) async fn lock_work(
    conn: &mut PgConnection,
    work: WorkId,
) -> Result<Option<PrincipalId>, StoreError> {
    let _: Option<Uuid> =
        query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR UPDATE")
            .bind(work.as_uuid())
            .fetch_optional(&mut *conn)
            .await
            .map_err(classify)?;
    let _: Option<Uuid> =
        query_scalar("SELECT work_id FROM work_publications WHERE work_id = $1 FOR UPDATE")
            .bind(work.as_uuid())
            .fetch_optional(&mut *conn)
            .await
            .map_err(classify)?;
    let owner: Option<Uuid> = query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
        .bind(work.as_uuid())
        .fetch_optional(&mut *conn)
        .await
        .map_err(classify)?;
    Ok(owner.map(PrincipalId::from_uuid))
}

/// The automation mode read in the applying commit (MOD-14). A share lock: a pause (UPDATE of
/// this row) waits for this commit, or this commit waits for the pause and then sees it.
pub(crate) async fn locked_automation_mode(
    conn: &mut PgConnection,
) -> Result<AutomationMode, StoreError> {
    let mode: String =
        query_scalar("SELECT mode FROM moderation_automation_state WHERE id = 1 FOR SHARE")
            .fetch_one(&mut *conn)
            .await
            .map_err(classify)?;
    match mode.as_str() {
        "enabled" => Ok(AutomationMode::Enabled),
        "paused" => Ok(AutomationMode::Paused),
        other => Err(integrity("unknown automation mode", other)),
    }
}

/// Persist the publication pointers the core produced (readable, hidden, review-withdrawn page,
/// once-public set and epoch). Audience, lifecycle and intent stay with the author commands.
pub(crate) async fn write_publication_pointers(
    conn: &mut PgConnection,
    facts: &PublicationFacts,
) -> Result<(), StoreError> {
    let ever_public: Vec<Uuid> = facts.ever_public.iter().map(|r| r.as_uuid()).collect();
    query(
        "UPDATE work_publications
            SET pointer_revision_id = $1,
                pointer_intent_generation = $2,
                hidden_revision_id = $3,
                hidden_intent_generation = $4,
                review_withdrawn_revision_id = $5,
                review_withdrawn_intent_generation = $6,
                ever_public = $7,
                public_epoch = $8,
                updated_at = now()
          WHERE work_id = $9",
    )
    .bind(facts.pointer.map(|p| p.revision.as_uuid()))
    .bind(facts.pointer.map(|p| p.intent_generation.get()))
    .bind(facts.hidden.map(|p| p.revision.as_uuid()))
    .bind(facts.hidden.map(|p| p.intent_generation.get()))
    .bind(facts.review_withdrawn.map(|p| p.revision.as_uuid()))
    .bind(facts.review_withdrawn.map(|p| p.intent_generation.get()))
    .bind(&ever_public)
    .bind(facts.public_epoch.get())
    .bind(facts.work.as_uuid())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

/// Apply the pointer effect on the public projection, with the metadata frozen on the revision.
pub(crate) async fn apply_pointer_effect(
    conn: &mut PgConnection,
    work: WorkId,
    effect: PointerEffect,
) -> Result<(), StoreError> {
    match effect {
        PointerEffect::Set(p) => {
            let meta_id: Option<Uuid> = query_scalar(
                "SELECT metadata_snapshot_id FROM revisions WHERE id = $1 AND work_id = $2",
            )
            .bind(p.revision.as_uuid())
            .bind(work.as_uuid())
            .fetch_optional(&mut *conn)
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
            .bind(work.as_uuid())
            .bind(p.revision.as_uuid())
            .bind(p.intent_generation.get())
            .bind(meta_id)
            .execute(&mut *conn)
            .await
            .map_err(classify)?;
        }
        PointerEffect::Clear => {
            query("DELETE FROM publications WHERE work_id = $1")
                .bind(work.as_uuid())
                .execute(&mut *conn)
                .await
                .map_err(classify)?;
        }
        PointerEffect::Unchanged => {}
    }
    Ok(())
}

const fn moderation_str(moderation: Moderation) -> Option<&'static str> {
    match moderation {
        Moderation::Unsubmitted => None,
        Moderation::Pending => Some("pending"),
        Moderation::Approved => Some("approved"),
        Moderation::Rejected => Some("rejected"),
        Moderation::NeedsAdmin => Some("needs_admin"),
    }
}

const fn verdict_str(verdict: AdminVerdict) -> &'static str {
    match verdict {
        AdminVerdict::Approve => "approve",
        AdminVerdict::Reject => "reject",
    }
}

const fn cause_str(cause: NeedsAdminCause) -> &'static str {
    match cause {
        NeedsAdminCause::Uncertain => "uncertain",
        NeedsAdminCause::InvalidOutput => "invalid_output",
    }
}

/// Where a decision came from, for the history row.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DecisionSource {
    pub evaluation: Option<EvaluationId>,
    pub case: Option<CaseId>,
}

/// Write the revision's decision exactly as the core holds it (version, policy, provenance and
/// the publication's moderation state) and append the history row for that version. The row is
/// upserted: a resubmission after a private withdrawal keeps the core's version, never resets it.
pub(crate) async fn write_decision(
    conn: &mut PgConnection,
    publication: &PublicationFacts,
    moderation: &WorkModerationFacts,
    revision: RevisionId,
    source: DecisionSource,
) -> Result<(), StoreError> {
    let state = publication
        .revisions
        .iter()
        .find(|r| r.snapshot.revision == revision)
        .and_then(|r| moderation_str(r.moderation))
        .ok_or_else(|| integrity("decision for unsubmitted revision", revision))?;
    let decision: &RevisionDecisionFacts = moderation
        .revisions
        .iter()
        .find(|r| r.revision == revision)
        .ok_or_else(|| integrity("decision facts missing", revision))?;

    let mut kind = None;
    let mut reason_code = None;
    let mut cause = None;
    let mut actor = None;
    let mut verdict = None;
    let mut reason = None;
    let mut grant = None;
    match &decision.provenance {
        None => {}
        Some(DecisionProvenance::AiApproved) => kind = Some("ai_approved"),
        Some(DecisionProvenance::AiRejected(code)) => {
            kind = Some("ai_rejected");
            reason_code = Some(code.as_str());
        }
        Some(DecisionProvenance::AiNeedsAdmin(c)) => {
            kind = Some("ai_needs_admin");
            cause = Some(cause_str(*c));
        }
        Some(DecisionProvenance::Admin {
            actor: a,
            verdict: v,
            reason: r,
        }) => {
            kind = Some("admin");
            actor = Some(a.as_uuid());
            verdict = Some(verdict_str(*v));
            reason = Some(r.as_str().to_owned());
        }
        Some(DecisionProvenance::Moderator {
            actor: a,
            grant: g,
            verdict: v,
            reason: r,
        }) => {
            kind = Some("moderator");
            actor = Some(a.as_uuid());
            verdict = Some(verdict_str(*v));
            reason = Some(r.as_str().to_owned());
            grant = Some(g.as_uuid());
        }
        Some(DecisionProvenance::PolicyRereview) => kind = Some("policy_rereview"),
        Some(DecisionProvenance::LegacyBootstrap) => kind = Some("legacy_bootstrap"),
    }

    query(
        "INSERT INTO work_revision_moderation (
             work_id, revision_id, decision_version, moderation, policy_version,
             provenance_kind, policy_reason_code, needs_admin_cause,
             admin_actor, admin_verdict, admin_reason, moderator_grant_id,
             created_at, updated_at
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), now())
         ON CONFLICT (work_id, revision_id) DO UPDATE SET
             decision_version = EXCLUDED.decision_version,
             moderation = EXCLUDED.moderation,
             policy_version = EXCLUDED.policy_version,
             provenance_kind = EXCLUDED.provenance_kind,
             policy_reason_code = EXCLUDED.policy_reason_code,
             needs_admin_cause = EXCLUDED.needs_admin_cause,
             admin_actor = EXCLUDED.admin_actor,
             admin_verdict = EXCLUDED.admin_verdict,
             admin_reason = EXCLUDED.admin_reason,
             moderator_grant_id = EXCLUDED.moderator_grant_id,
             updated_at = now()",
    )
    .bind(publication.work.as_uuid())
    .bind(revision.as_uuid())
    .bind(decision.version.get())
    .bind(state)
    .bind(decision.policy.get())
    .bind(kind)
    .bind(reason_code)
    .bind(cause)
    .bind(actor)
    .bind(verdict)
    .bind(reason.as_deref())
    .bind(grant)
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    // A resubmission keeps the version of a still-pending revision: the history already holds it.
    query(
        "INSERT INTO work_revision_decision_history (
             id, work_id, revision_id, decision_version, moderation, policy_version,
             provenance_kind, policy_reason_code, needs_admin_cause, actor_id,
             moderator_grant_id, reason, evaluation_id, case_id, created_at
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, now())
         ON CONFLICT (work_id, revision_id, decision_version) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(publication.work.as_uuid())
    .bind(revision.as_uuid())
    .bind(decision.version.get())
    .bind(state)
    .bind(decision.policy.get())
    .bind(kind)
    .bind(reason_code)
    .bind(cause)
    .bind(actor)
    .bind(grant)
    .bind(reason.as_deref())
    .bind(source.evaluation.map(|e| e.as_uuid()))
    .bind(source.case.map(|c| c.as_uuid()))
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

/// Persist job states the core moved away from `Open` (applied, failed, withdrawn, rerouted).
/// A job another path already finished keeps its state.
pub(crate) async fn sync_job_states(
    conn: &mut PgConnection,
    moderation: &WorkModerationFacts,
) -> Result<(), StoreError> {
    for job in &moderation.jobs {
        if job.state == JobState::Open {
            continue;
        }
        query(
            "UPDATE work_evaluation_jobs
                SET state = $1, lease_expires_at = NULL, updated_at = now()
              WHERE id = $2 AND state IN ('open', 'leased')",
        )
        .bind(job.state.as_str())
        .bind(job.id.as_uuid())
        .execute(&mut *conn)
        .await
        .map_err(classify)?;
    }
    Ok(())
}

pub(crate) const fn origin_str(origin: CaseOrigin) -> &'static str {
    match origin {
        CaseOrigin::EvaluatorUncertain => "evaluator_uncertain",
        CaseOrigin::InvalidOutput => "invalid_output",
        CaseOrigin::RetriesExhausted => "retries_exhausted",
        CaseOrigin::AutomationPaused => "automation_paused",
        CaseOrigin::PolicyRereview => "policy_rereview",
        CaseOrigin::ReviewerIntervention => "reviewer_intervention",
    }
}

/// Open the human case for a revision at its current decision version, unless one is open.
pub(crate) async fn open_work_case(
    conn: &mut PgConnection,
    moderation: &WorkModerationFacts,
    owner: PrincipalId,
    revision: RevisionId,
    origin: CaseOrigin,
) -> Result<(), StoreError> {
    let decision = moderation
        .revisions
        .iter()
        .find(|r| r.revision == revision)
        .ok_or_else(|| integrity("case for undecided revision facts", revision))?;
    query(
        "INSERT INTO moderation_cases (
             id, subject_kind, work_id, revision_id, content_digest, decision_version,
             target_content_owner, target_work_owner, origin, version, state,
             created_at, updated_at
         )
         VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, $7, 1, 'queued', now(), now())
         ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
         DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(moderation.work.as_uuid())
    .bind(revision.as_uuid())
    .bind(decision.digest.as_str())
    .bind(decision.version.get())
    .bind(owner.as_uuid())
    .bind(origin_str(origin))
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

/// Persist a case transition under its version fence. Returns `false` when the stored version
/// moved (the caller answers a conflict). Notes are appended by the caller.
pub(crate) async fn write_case(
    conn: &mut PgConnection,
    next: &ReviewCase,
    expected: i64,
) -> Result<bool, StoreError> {
    let facts = next.facts();
    let basis_cols = |basis: AuthorityBasis| match basis {
        AuthorityBasis::Admin => ("admin", None),
        AuthorityBasis::Grant(g) => ("grant", Some(g.as_uuid())),
    };
    let (state, claimed_by, claimed_kind, claimed_grant) = match facts.state {
        CaseState::Queued => ("queued", None, None, None),
        CaseState::Escalated => ("escalated", None, None, None),
        CaseState::Claimed { by, basis } => {
            let (k, g) = basis_cols(basis);
            ("claimed", Some(by.as_uuid()), Some(k), g)
        }
        CaseState::Concluded { .. } => ("concluded", None, None, None),
        CaseState::Closed(_) => ("closed", None, None, None),
    };
    let (concluded_by, concluded_kind, concluded_grant, verdict) = match facts.state {
        CaseState::Concluded { by, basis, verdict } => {
            let (k, g) = basis_cols(basis);
            let v = match verdict {
                CaseVerdict::Approve => "approve",
                CaseVerdict::Reject => "reject",
            };
            (Some(by.as_uuid()), Some(k), g, Some(v))
        }
        _ => (None, None, None, None),
    };
    let close_reason = match facts.state {
        CaseState::Closed(reason) => Some(reason.as_str()),
        _ => None,
    };
    let updated = query(
        "UPDATE moderation_cases
            SET version = $1, state = $2,
                claimed_by = $3, claimed_basis_kind = $4, claimed_basis_grant_id = $5,
                concluded_by = $6, concluded_basis_kind = $7, concluded_basis_grant_id = $8,
                concluded_verdict = $9, close_reason = $10, updated_at = now()
          WHERE id = $11 AND version = $12",
    )
    .bind(facts.version.get())
    .bind(state)
    .bind(claimed_by)
    .bind(claimed_kind)
    .bind(claimed_grant)
    .bind(concluded_by)
    .bind(concluded_kind)
    .bind(concluded_grant)
    .bind(verdict)
    .bind(close_reason)
    .bind(facts.id.as_uuid())
    .bind(expected)
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(updated.rows_affected() == 1)
}

/// Re-check every claimed case whose claimant matches `filter` after a directory change
/// (revocation, removed admin seat, group membership): lost every basis -> back to the queue;
/// another live basis -> kept on it (MOD-11). `directory` is the state after the change.
pub(crate) async fn revalidate_claims(
    conn: &mut PgConnection,
    directory: &ModerationDirectory,
    claimant: Option<PrincipalId>,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    let ids: Vec<Uuid> = query_scalar(
        "SELECT id FROM moderation_cases
          WHERE state = 'claimed' AND ($1::uuid IS NULL OR claimed_by = $1)
          ORDER BY id",
    )
    .bind(claimant.map(|p| p.as_uuid()))
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    for id in ids {
        let Some(case) = super::moderation::load_case_locked(conn, CaseId::from_uuid(id)).await?
        else {
            continue;
        };
        let Ok(transition) = case.revalidate(directory, now) else {
            continue;
        };
        if transition.next.version() != case.version() {
            write_case(conn, &transition.next, case.version().get()).await?;
        }
    }
    Ok(())
}

/// Owner and current decision version of every pending revision whose open job was rerouted.
#[derive(sqlx::FromRow)]
pub(crate) struct ReroutedRevision {
    pub work_id: Uuid,
    pub revision_id: Uuid,
    pub content_digest: String,
    pub decision_version: i64,
    pub owner_id: Uuid,
}

/// Pause reroute (MOD-14) in the pausing commit: every open or leased job leaves the evaluator
/// and each still-undecided revision gets a human case at its current decision version.
pub(crate) async fn reroute_open_jobs(conn: &mut PgConnection) -> Result<usize, StoreError> {
    let rows: Vec<ReroutedRevision> = query_as(
        "WITH rerouted AS (
             UPDATE work_evaluation_jobs
                SET state = 'rerouted', lease_expires_at = NULL, updated_at = now()
              WHERE state IN ('open', 'leased')
          RETURNING work_id, revision_id, content_digest
         )
         SELECT DISTINCT r.work_id, r.revision_id, r.content_digest, m.decision_version, w.owner_id
           FROM rerouted r
           JOIN works w ON w.id = r.work_id
           JOIN work_revision_moderation m
             ON m.work_id = r.work_id AND m.revision_id = r.revision_id
          WHERE m.moderation = 'pending' AND m.provenance_kind IS NULL",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    let count = rows.len();
    for row in rows {
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
        .bind(row.work_id)
        .bind(row.revision_id)
        .bind(&row.content_digest)
        .bind(row.decision_version)
        .bind(row.owner_id)
        .execute(&mut *conn)
        .await
        .map_err(classify)?;
    }
    Ok(count)
}
