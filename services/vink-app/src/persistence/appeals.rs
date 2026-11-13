//! Durable work appeals (MOD-10). Commands share the author/worker work lock, standing fence,
//! and pure AppealBook. No notification or private-snapshot access is implied by an appeal.
use sqlx::{PgConnection, PgPool, query, query_as, query_scalar};
use time::OffsetDateTime;
use uuid::Uuid;

use super::moderation::{load_moderation_directory, lock_admin_seat};
use super::moderation_write::{
    DecisionSource, apply_pointer_effect, lock_work, sync_job_states, write_decision,
    write_publication_pointers,
};
use super::postgres::{classify, integrity};
use super::principal::require_active;
use crate::application::moderation::ModerationActionError as Error;
use crate::application::ports::StoreError;
use crate::domain::document::DocumentData;
use crate::domain::ids::{AppealId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::appeal::{
    AppealBook, AppealFacts, AppealRefusal, AppealState, AppealVerdict, CurrentDecision, DecidedBy,
    Filed, Notice,
};
use crate::domain::moderation::case::{CloseReason, ReviewSubject};
use crate::domain::moderation::{
    DecisionProvenance, DecisionReason, DecisionVersion, WorkModeration,
};
use crate::domain::publication::{Audience, Moderation, Publication};
use crate::domain::revision::ContentHash;

fn refusal(e: AppealRefusal) -> Error {
    match e {
        AppealRefusal::NotFound | AppealRefusal::UnknownAppeal => Error::NotFound,
        AppealRefusal::Authority(a) => Error::Forbidden(a.as_str()),
        AppealRefusal::Recused => Error::Forbidden("recused"),
        AppealRefusal::ReasonRequired => Error::ValidationFailed("reason_required"),
        AppealRefusal::InvalidFacts => Error::Internal,
        other => Error::Conflict(other.as_str()),
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppealRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub revision_id: Uuid,
    pub decision_version: i64,
    pub state: String,
    pub resolution_reason: Option<String>,
    pub created_at: OffsetDateTime,
    content_digest: String,
    appellant_id: Uuid,
    decided_by: String,
    moderator_id: Option<Uuid>,
    close_reason: Option<String>,
}

impl AppealRecord {
    fn facts(&self) -> Result<AppealFacts, StoreError> {
        let invalid = || integrity("invalid work appeal", self.id);
        let state = match self.state.as_str() {
            "open" => AppealState::Open,
            "upheld" => AppealState::Upheld,
            "reversed" => AppealState::Reversed,
            "closed" => AppealState::Closed(match self.close_reason.as_deref() {
                Some("withdrawn") => CloseReason::Withdrawn,
                Some("target_gone") => CloseReason::TargetGone,
                Some("superseded") => CloseReason::Superseded,
                _ => return Err(invalid()),
            }),
            _ => return Err(invalid()),
        };
        let decided_by = match self.decided_by.as_str() {
            "evaluator" => DecidedBy::Evaluator,
            "moderator" => DecidedBy::Moderator(PrincipalId::from_uuid(
                self.moderator_id.ok_or_else(invalid)?,
            )),
            _ => return Err(invalid()),
        };
        Ok(AppealFacts {
            id: AppealId::from_uuid(self.id),
            subject: ReviewSubject::WorkRevision {
                work: WorkId::from_uuid(self.work_id),
                revision: RevisionId::from_uuid(self.revision_id),
                digest: ContentHash::parse(&self.content_digest).ok_or_else(invalid)?,
                decision: DecisionVersion::new(self.decision_version).ok_or_else(invalid)?,
            },
            appellant: PrincipalId::from_uuid(self.appellant_id),
            decided_by,
            state,
            notice: Notice::NotSent,
        })
    }
}

async fn records(conn: &mut PgConnection, work: WorkId) -> Result<Vec<AppealRecord>, StoreError> {
    query_as("SELECT * FROM work_appeals WHERE work_id = $1 ORDER BY created_at, id")
        .bind(work.as_uuid())
        .fetch_all(conn)
        .await
        .map_err(classify)
}

async fn book(conn: &mut PgConnection, work: WorkId) -> Result<AppealBook, StoreError> {
    let facts = records(conn, work)
        .await?
        .iter()
        .map(AppealRecord::facts)
        .collect::<Result<_, _>>()?;
    AppealBook::restore(facts).map_err(|e| integrity("appeal book", e))
}

fn current(
    publication: &Publication,
    moderation: &WorkModeration,
    revision: RevisionId,
) -> Result<CurrentDecision, Error> {
    let d = moderation
        .facts()
        .revisions
        .iter()
        .find(|r| r.revision == revision)
        .ok_or(Error::NotFound)?;
    let r = publication
        .facts()
        .revisions
        .iter()
        .find(|r| r.snapshot.revision == revision)
        .ok_or(Error::NotFound)?;
    let decided_by = match &d.provenance {
        Some(DecisionProvenance::AiApproved | DecisionProvenance::AiRejected(_)) => {
            Some(DecidedBy::Evaluator)
        }
        Some(DecisionProvenance::Moderator { actor, .. }) => Some(DecidedBy::Moderator(*actor)),
        Some(DecisionProvenance::Admin { actor, .. }) => Some(DecidedBy::Admin(*actor)),
        _ => None,
    };
    Ok(CurrentDecision {
        subject: ReviewSubject::WorkRevision {
            work: publication.facts().work,
            revision,
            digest: d.digest.clone(),
            decision: d.version,
        },
        owner: publication.facts().owner,
        decided_by,
        adverse: r.moderation == Moderation::Rejected,
        withdrawn: publication.facts().audience == Audience::Private
            || !publication.facts().lifecycle.is_active(),
    })
}

pub async fn file(
    pool: &PgPool,
    actor: PrincipalId,
    work: WorkId,
    revision: RevisionId,
    seen_version: DecisionVersion,
) -> Result<(AppealRecord, bool), Error> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let owner = lock_work(&mut tx, work).await?.ok_or(Error::NotFound)?;
    if owner != actor {
        return Err(Error::NotFound);
    }
    require_active(&mut tx, actor).await?;
    let (p, m, _) = super::publication::load_paired_state(&mut tx, owner, work).await?;
    let current = current(&p, &m, revision)?;
    let ReviewSubject::WorkRevision { digest, .. } = &current.subject else {
        return Err(Error::Internal);
    };
    let seen = ReviewSubject::WorkRevision {
        work,
        revision,
        digest: digest.clone(),
        decision: seen_version,
    };
    let appeals = book(&mut tx, work).await?;
    let (next, filed) = appeals
        .file(actor, &current, &seen, AppealId::from_uuid(Uuid::now_v7()))
        .map_err(refusal)?;
    let (id, replayed) = match filed {
        Filed::Recorded(id) => (id, false),
        Filed::Existing(id) => (id, true),
    };
    if !replayed {
        let facts = next.get(id).ok_or(Error::Internal)?;
        let (by, moderator) = match facts.decided_by {
            DecidedBy::Evaluator => ("evaluator", None),
            DecidedBy::Moderator(p) => ("moderator", Some(p.as_uuid())),
            DecidedBy::Admin(_) => return Err(Error::Internal),
        };
        query("INSERT INTO work_appeals (id, work_id, revision_id, decision_version, content_digest, appellant_id, decided_by, moderator_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(id.as_uuid()).bind(work.as_uuid()).bind(revision.as_uuid()).bind(seen_version.get())
            .bind(digest.as_str()).bind(actor.as_uuid()).bind(by).bind(moderator).execute(&mut *tx).await.map_err(classify)?;
    }
    let record = records(&mut tx, work)
        .await?
        .into_iter()
        .find(|a| a.id == id.as_uuid())
        .ok_or(Error::Internal)?;
    tx.commit().await.map_err(classify)?;
    Ok((record, replayed))
}

pub async fn conclude(
    pool: &PgPool,
    actor: PrincipalId,
    id: AppealId,
    expected: DecisionVersion,
    verdict: AppealVerdict,
    reason: DecisionReason,
) -> Result<AppealRecord, Error> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let work: Option<Uuid> = query_scalar("SELECT work_id FROM work_appeals WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
    let work = WorkId::from_uuid(work.ok_or(Error::NotFound)?);
    let owner = lock_work(&mut tx, work).await?.ok_or(Error::NotFound)?;
    // Seat removal holds the seat before touching the principal and revalidating cases.
    let has_seat = lock_admin_seat(&mut tx, actor).await?;
    require_active(&mut tx, actor).await?;
    if !has_seat {
        return Err(Error::Forbidden("not_admin"));
    }
    let appeals = book(&mut tx, work).await?;
    let appeal = appeals.get(id).ok_or(Error::NotFound)?;
    let ReviewSubject::WorkRevision {
        revision, decision, ..
    } = appeal.subject
    else {
        return Err(Error::Internal);
    };
    if decision != expected {
        return Err(Error::Conflict("stale_decision"));
    }
    let (p, m, _) = super::publication::load_paired_state(&mut tx, owner, work).await?;
    let current = current(&p, &m, revision)?;
    let directory = load_moderation_directory(&mut tx, OffsetDateTime::now_utc()).await?;
    let (next, conclusion) = appeals
        .conclude(
            &directory,
            actor,
            id,
            &current,
            verdict,
            Some(reason.clone()),
            OffsetDateTime::now_utc(),
        )
        .map_err(refusal)?;
    let applied = m
        .apply_appeal_conclusion(&p, &conclusion)
        .map_err(|_| Error::Conflict("stale_decision"))?;
    let version = applied
        .moderation
        .facts()
        .revisions
        .iter()
        .find(|r| r.revision == revision)
        .ok_or(Error::Internal)?
        .version;
    query("UPDATE work_appeals SET state = $2, concluded_by = $3, resolution_reason = $4, resulting_decision_version = $5, updated_at = now() WHERE id = $1 AND state = 'open'")
        .bind(id.as_uuid()).bind(next.get(id).ok_or(Error::Internal)?.state.as_str()).bind(actor.as_uuid()).bind(reason.as_str()).bind(version.get())
        .execute(&mut *tx).await.map_err(classify)?;
    write_decision(
        &mut tx,
        applied.publication.facts(),
        applied.moderation.facts(),
        revision,
        DecisionSource {
            appeal: Some(id),
            ..Default::default()
        },
    )
    .await?;
    write_publication_pointers(&mut tx, applied.publication.facts()).await?;
    apply_pointer_effect(&mut tx, work, applied.effects.pointer).await?;
    sync_job_states(&mut tx, applied.moderation.facts()).await?;
    query("UPDATE moderation_cases SET state = 'closed', close_reason = 'superseded', version = version + 1, updated_at = now() WHERE work_id = $1 AND revision_id = $2 AND state IN ('queued','claimed','escalated')")
        .bind(work.as_uuid()).bind(revision.as_uuid()).execute(&mut *tx).await.map_err(classify)?;
    let record = records(&mut tx, work)
        .await?
        .into_iter()
        .find(|a| a.id == id.as_uuid())
        .ok_or(Error::Internal)?;
    tx.commit().await.map_err(classify)?;
    Ok(record)
}

pub async fn list_own(
    pool: &PgPool,
    actor: PrincipalId,
    work: WorkId,
) -> Result<Vec<AppealRecord>, Error> {
    let mut tx = super::discussion::read_snapshot(pool).await?;
    let owner: Option<Uuid> = query_scalar("SELECT owner_id FROM works WHERE id = $1")
        .bind(work.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
    if owner != Some(actor.as_uuid()) {
        return Err(Error::NotFound);
    }
    Ok(records(&mut tx, work).await?)
}

pub async fn list_open(pool: &PgPool, actor: PrincipalId) -> Result<Vec<AppealRecord>, Error> {
    let mut tx = super::discussion::read_snapshot(pool).await?;
    load_moderation_directory(&mut tx, OffsetDateTime::now_utc())
        .await?
        .require_admin(actor, OffsetDateTime::now_utc())
        .map_err(|e| Error::Forbidden(e.as_str()))?;
    Ok(query_as("SELECT a.* FROM work_appeals a JOIN work_publications p ON p.work_id = a.work_id WHERE a.state = 'open' AND a.appellant_id <> $1 AND p.audience = 'public' AND p.lifecycle = 'active' ORDER BY a.created_at, a.id")
        .bind(actor.as_uuid()).fetch_all(&mut *tx).await.map_err(classify)?)
}

pub async fn detail(
    pool: &PgPool,
    actor: PrincipalId,
    id: AppealId,
) -> Result<(AppealRecord, String, DocumentData), Error> {
    let mut tx = super::discussion::read_snapshot(pool).await?;
    load_moderation_directory(&mut tx, OffsetDateTime::now_utc())
        .await?
        .require_admin(actor, OffsetDateTime::now_utc())
        .map_err(|e| Error::Forbidden(e.as_str()))?;
    let row: AppealRecord = query_as("SELECT a.* FROM work_appeals a JOIN work_publications p ON p.work_id = a.work_id WHERE a.id = $1 AND a.state = 'open' AND p.audience = 'public' AND p.lifecycle = 'active'")
        .bind(id.as_uuid()).fetch_optional(&mut *tx).await.map_err(classify)?.ok_or(Error::NotFound)?;
    if row.appellant_id == actor.as_uuid() {
        return Err(Error::Forbidden("recused"));
    }
    let (title, document): (String, sqlx::types::Json<DocumentData>) = query_as("SELECT title, document FROM revisions WHERE work_id = $1 AND id = $2 AND content_hash = $3")
        .bind(row.work_id).bind(row.revision_id).bind(&row.content_digest).fetch_optional(&mut *tx).await.map_err(classify)?.ok_or(Error::NotFound)?;
    Ok((row, title, document.0))
}
