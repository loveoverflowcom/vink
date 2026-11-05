//! PostgreSQL adapter of literary endorsement (LVE, #73). Each write is one transaction:
//! current facts under locks → the exact pure decision (`decide_work_endorsement`,
//! `decide_comment_endorsement`) → a conditional write of `Transition::write()` → commit → the
//! durable answer. The schema (`migrations/20261105000000_literary_endorsement.sql`) holds the
//! structural rules on its own: typed target foreign keys, one row per (actor, target), a
//! revision of the same work, versions that only advance, rows that are never deleted.
//!
//! Lock order, the prefix of the discussion adapter's: publication (FOR SHARE) → thread (FOR
//! SHARE) → comment (FOR SHARE) → the actor's endorsement row (FOR UPDATE). A withdrawal or
//! republish (pointer update), a comments close (thread UPDATE) and a moderation decision
//! (comment UPDATE) therefore either commit before the decision reads its facts or wait until
//! it commits; endorsement never takes their rows in the reverse order. No network call happens
//! inside these transactions.
//!
//! An absent row is not a lock on a future row. A first intent decides from
//! [`Endorsement::ABSENT`] and INSERTs with `ON CONFLICT DO NOTHING`; if a concurrent first intent
//! of the same actor committed meanwhile, the INSERT writes nothing and the transaction reads the
//! winner's row under FOR UPDATE and decides again. The UPDATE path is conditional on the version
//! it decided from. A unique key alone would not stop a stale intent: the version does.
//!
//! Counts are read from the canonical rows in the same transaction (CHOICE-05 A): a work counts
//! selections of its current publication's revision, a comment its selected rows.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::PgStore;
use super::discussion::{ThreadRow, integrity, read_snapshot, state};
use super::postgres::classify;
use crate::application::endorsement::CommentBatch;
use crate::application::ports::{
    CommentEndorsementWrite, CommentSummaryFacts, EndorsementStore, EndorsementSummaryFacts,
    OwnCommentFacts, OwnEndorsementFacts, StoreError, WorkEndorsementWrite,
};
use crate::domain::endorsement::comment::{
    CommentDecision, CommentFacts, CommentRequest, decide_comment_endorsement,
};
use crate::domain::endorsement::work::{
    WorkDecision, WorkFacts, WorkRequest, decide_work_endorsement,
};
use crate::domain::endorsement::{
    CommentAnchor, Endorsement, EndorsementCount, EndorsementVersion,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

/// A first intent can lose the INSERT to a concurrent first intent at most once: afterwards the
/// winner's row exists (rows are never deleted) and every read locks it.
const CLAIM_ATTEMPTS: usize = 2;

fn version(raw: i64) -> Result<EndorsementVersion, StoreError> {
    EndorsementVersion::new(raw).ok_or_else(|| integrity("stored endorsement version", raw))
}

fn count(raw: i64) -> Result<EndorsementCount, StoreError> {
    u32::try_from(raw)
        .map(EndorsementCount::restore)
        .map_err(|_| integrity("endorsement count", raw))
}

/// A resolved principal without a Vink record is not an authenticated actor, whatever the
/// decision would say. Principals are never deleted, so no lock is needed.
async fn require_principal(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
) -> Result<(), StoreError> {
    let known: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM principals WHERE id = $1)")
        .bind(actor.as_uuid())
        .fetch_one(&mut **tx)
        .await
        .map_err(classify)?;
    if known {
        Ok(())
    } else {
        tracing::info!("principal has no Vink record");
        Err(StoreError::UnknownPrincipal)
    }
}

fn ids(batch: &CommentBatch) -> Vec<Uuid> {
    batch.ids().iter().map(|id| id.as_uuid()).collect()
}

/// The work, its current publication's revision and either its effective count (summaries) or
/// the actor's row (own state).
#[derive(sqlx::FromRow)]
struct WorkRow {
    id: Uuid,
    owner_id: Uuid,
    published: Option<Uuid>,
    effective: Option<i64>,
    anchor: Option<Uuid>,
    version: Option<i64>,
}

impl WorkRow {
    fn facts(&self) -> WorkFacts {
        WorkFacts {
            work: WorkId::from_uuid(self.id),
            author: PrincipalId::from_uuid(self.owner_id),
            published: self.published.map(RevisionId::from_uuid),
        }
    }
}

/// A requested comment of the work's thread, with that thread's settings and either its public
/// count (summaries) or the actor's row (own state).
#[derive(sqlx::FromRow)]
struct CommentRow {
    id: Uuid,
    author_id: Uuid,
    review: String,
    removal: Option<String>,
    moderation_version: i64,
    thread_id: Uuid,
    work_id: Uuid,
    comments_open: bool,
    settings_generation: i64,
    endorsements: Option<i64>,
    selected: Option<bool>,
    version: Option<i64>,
}

impl CommentRow {
    fn facts(&self, work_public: bool) -> Result<CommentFacts, StoreError> {
        let thread = ThreadRow {
            id: self.thread_id,
            comments_open: self.comments_open,
            settings_generation: self.settings_generation,
        };
        Ok(CommentFacts {
            work: WorkId::from_uuid(self.work_id),
            comment: CommentId::from_uuid(self.id),
            work_public,
            writer: PrincipalId::from_uuid(self.author_id),
            state: state(
                &self.review,
                self.removal.as_deref(),
                self.moderation_version,
            )?,
            settings: thread.settings()?,
        })
    }

    fn endorsement(&self) -> Result<Endorsement<CommentAnchor>, StoreError> {
        match (self.selected, self.version) {
            (Some(selected), Some(raw)) => Ok(Endorsement::restore(
                selected.then_some(CommentAnchor),
                version(raw)?,
            )),
            (None, None) => Ok(Endorsement::ABSENT),
            _ => Err(integrity("partial comment endorsement row", self.id)),
        }
    }
}

const COMMENT_COLUMNS: &str = "c.id, c.author_id, c.review, c.removal, c.moderation_version, \
     t.id AS thread_id, t.work_id, t.comments_open, t.settings_generation";

impl EndorsementStore for PgStore {
    async fn set_work_endorsement(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        request: WorkRequest,
    ) -> Result<WorkEndorsementWrite, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_principal(&mut tx, actor).await?;
        // A withdraw/republish (pointer DELETE/UPDATE) waits for this decision, or committed
        // before it and is what this read sees.
        let published: Option<Uuid> =
            sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR SHARE")
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        // The owner never changes; the loaded row's identity is the fact, never the request's.
        let work: Option<(Uuid, Uuid)> =
            sqlx::query_as("SELECT id, owner_id FROM works WHERE id = $1")
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let facts = work.map(|(id, owner)| WorkFacts {
            work: WorkId::from_uuid(id),
            author: PrincipalId::from_uuid(owner),
            published: published.map(RevisionId::from_uuid),
        });

        for _ in 0..CLAIM_ATTEMPTS {
            let row: Option<(Option<Uuid>, i64)> = sqlx::query_as(
                "SELECT revision_id, version FROM work_endorsements
                  WHERE work_id = $1 AND actor_id = $2 FOR UPDATE",
            )
            .bind(work_id.as_uuid())
            .bind(actor.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
            let current = match row {
                Some((anchor, raw)) => {
                    Endorsement::restore(anchor.map(RevisionId::from_uuid), version(raw)?)
                }
                None => Endorsement::ABSENT,
            };
            let decision = decide_work_endorsement(actor, work_id, request, facts, current);
            if let WorkDecision::Transition(transition) = decision
                && let Some(next) = transition.write()
            {
                let anchor = next.selection().map(|revision| revision.as_uuid());
                let written = if row.is_some() {
                    let updated = sqlx::query(
                        "UPDATE work_endorsements
                            SET revision_id = $3, version = $4, updated_at = now()
                          WHERE work_id = $1 AND actor_id = $2 AND version = $5",
                    )
                    .bind(work_id.as_uuid())
                    .bind(actor.as_uuid())
                    .bind(anchor)
                    .bind(next.version().get())
                    .bind(current.version().get())
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?
                    .rows_affected();
                    if updated != 1 {
                        return Err(integrity("locked work endorsement changed", work_id));
                    }
                    true
                } else {
                    sqlx::query(
                        "INSERT INTO work_endorsements (work_id, actor_id, revision_id, version)
                         VALUES ($1, $2, $3, $4)
                         ON CONFLICT (work_id, actor_id) DO NOTHING",
                    )
                    .bind(work_id.as_uuid())
                    .bind(actor.as_uuid())
                    .bind(anchor)
                    .bind(next.version().get())
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?
                    .rows_affected()
                        == 1
                };
                if !written {
                    // A concurrent first intent committed its row: decide again on it.
                    continue;
                }
            }
            let count = match facts.and_then(|facts| facts.published) {
                Some(revision) => count(
                    sqlx::query_scalar(
                        "SELECT count(*) FROM work_endorsements
                          WHERE work_id = $1 AND revision_id = $2",
                    )
                    .bind(work_id.as_uuid())
                    .bind(revision.as_uuid())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?,
                )?,
                None => EndorsementCount::ZERO,
            };
            tx.commit().await.map_err(classify)?;
            return Ok(WorkEndorsementWrite {
                facts,
                decision,
                count,
            });
        }
        Err(integrity("work endorsement claim did not settle", work_id))
    }

    async fn set_comment_endorsement(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comment_id: CommentId,
        request: CommentRequest,
    ) -> Result<CommentEndorsementWrite, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_principal(&mut tx, actor).await?;
        let work_public = sqlx::query_scalar::<_, Uuid>(
            "SELECT work_id FROM publications WHERE work_id = $1 FOR SHARE",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?
        .is_some();
        // Share lock: a comments close (thread UPDATE) waits for this decision or vice versa.
        let thread: Option<(Uuid, Uuid, bool, i64)> = sqlx::query_as(
            "SELECT id, work_id, comments_open, settings_generation
               FROM discussion_threads WHERE work_id = $1 FOR SHARE",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        // The comment is looked up only inside this work's thread, so an ID of another work's
        // comment finds nothing; its work still comes from its OWN thread row, so the pure
        // binding check would refuse a foreign comment even if that scope were lost. Share lock:
        // moderation and removal (UPDATE) are serialized with this decision.
        let facts = match thread {
            Some((thread_id, _, comments_open, settings_generation)) => {
                let target: Option<(Uuid, Uuid, String, Option<String>, i64, Uuid)> =
                    sqlx::query_as(
                        "SELECT c.id, c.author_id, c.review, c.removal, c.moderation_version,
                                t.work_id
                           FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
                          WHERE c.id = $1 AND c.thread_id = $2
                            FOR SHARE OF c",
                    )
                    .bind(comment_id.as_uuid())
                    .bind(thread_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;
                target
                    .map(
                        |(id, author, raw_review, raw_removal, raw_version, comment_work)| {
                            let settings = ThreadRow {
                                id: thread_id,
                                comments_open,
                                settings_generation,
                            }
                            .settings()?;
                            Ok::<_, StoreError>(CommentFacts {
                                work: WorkId::from_uuid(comment_work),
                                comment: CommentId::from_uuid(id),
                                work_public,
                                writer: PrincipalId::from_uuid(author),
                                state: state(&raw_review, raw_removal.as_deref(), raw_version)?,
                                settings,
                            })
                        },
                    )
                    .transpose()?
            }
            None => None,
        };

        for _ in 0..CLAIM_ATTEMPTS {
            let row: Option<(bool, i64)> = sqlx::query_as(
                "SELECT selected, version FROM comment_endorsements
                  WHERE comment_id = $1 AND actor_id = $2 FOR UPDATE",
            )
            .bind(comment_id.as_uuid())
            .bind(actor.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
            let current = match row {
                Some((selected, raw)) => {
                    Endorsement::restore(selected.then_some(CommentAnchor), version(raw)?)
                }
                None => Endorsement::ABSENT,
            };
            let decision =
                decide_comment_endorsement(actor, work_id, comment_id, request, facts, current);
            if let CommentDecision::Transition(transition) = decision
                && let Some(next) = transition.write()
            {
                let selected = next.selection().is_some();
                let written = if row.is_some() {
                    let updated = sqlx::query(
                        "UPDATE comment_endorsements
                            SET selected = $3, version = $4, updated_at = now()
                          WHERE comment_id = $1 AND actor_id = $2 AND version = $5",
                    )
                    .bind(comment_id.as_uuid())
                    .bind(actor.as_uuid())
                    .bind(selected)
                    .bind(next.version().get())
                    .bind(current.version().get())
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?
                    .rows_affected();
                    if updated != 1 {
                        return Err(integrity("locked comment endorsement changed", comment_id));
                    }
                    true
                } else {
                    sqlx::query(
                        "INSERT INTO comment_endorsements (comment_id, actor_id, selected, version)
                         VALUES ($1, $2, $3, $4)
                         ON CONFLICT (comment_id, actor_id) DO NOTHING",
                    )
                    .bind(comment_id.as_uuid())
                    .bind(actor.as_uuid())
                    .bind(selected)
                    .bind(next.version().get())
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?
                    .rows_affected()
                        == 1
                };
                if !written {
                    continue;
                }
            }
            let count = if facts.is_some_and(|facts| facts.visible()) {
                count(
                    sqlx::query_scalar(
                        "SELECT count(*) FROM comment_endorsements
                          WHERE comment_id = $1 AND selected",
                    )
                    .bind(comment_id.as_uuid())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?,
                )?
            } else {
                EndorsementCount::ZERO
            };
            tx.commit().await.map_err(classify)?;
            return Ok(CommentEndorsementWrite {
                facts,
                decision,
                count,
            });
        }
        Err(integrity(
            "comment endorsement claim did not settle",
            comment_id,
        ))
    }

    async fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<EndorsementSummaryFacts, StoreError> {
        // One read-only snapshot: the work's publication, the comments' states and every count
        // agree with each other. Three statements whatever the batch size.
        let mut tx = read_snapshot(self.pool()).await?;
        let work: Option<WorkRow> = sqlx::query_as(
            "SELECT w.id, w.owner_id, p.revision_id AS published,
                    (SELECT count(*) FROM work_endorsements e
                      WHERE e.work_id = w.id AND e.revision_id = p.revision_id) AS effective,
                    NULL::uuid AS anchor, NULL::bigint AS version
               FROM works w LEFT JOIN publications p ON p.work_id = w.id
              WHERE w.id = $1",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some(work) = work else {
            tx.commit().await.map_err(classify)?;
            return Ok(EndorsementSummaryFacts {
                work: None,
                work_count: EndorsementCount::ZERO,
                comments: Vec::new(),
            });
        };
        let rows: Vec<CommentRow> = if comments.ids().is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {COMMENT_COLUMNS},
                        (SELECT count(*) FROM comment_endorsements e
                          WHERE e.comment_id = c.id AND e.selected) AS endorsements,
                        NULL::boolean AS selected, NULL::bigint AS version
                   FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
                  WHERE c.id = ANY($2::uuid[]) AND t.work_id = $1"
            )))
            .bind(work_id.as_uuid())
            .bind(ids(comments))
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?
        };
        tx.commit().await.map_err(classify)?;
        let work_public = work.published.is_some();
        Ok(EndorsementSummaryFacts {
            work: Some(work.facts()),
            work_count: count(work.effective.unwrap_or_default())?,
            comments: rows
                .iter()
                .map(|row| {
                    Ok(CommentSummaryFacts {
                        facts: row.facts(work_public)?,
                        count: count(row.endorsements.unwrap_or_default())?,
                    })
                })
                .collect::<Result<_, StoreError>>()?,
        })
    }

    async fn own_endorsements(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<OwnEndorsementFacts, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        require_principal(&mut tx, actor).await?;
        let work: Option<WorkRow> = sqlx::query_as(
            "SELECT w.id, w.owner_id, p.revision_id AS published, NULL::bigint AS effective,
                    e.revision_id AS anchor, e.version
               FROM works w
               LEFT JOIN publications p ON p.work_id = w.id
               LEFT JOIN work_endorsements e ON e.work_id = w.id AND e.actor_id = $2
              WHERE w.id = $1",
        )
        .bind(work_id.as_uuid())
        .bind(actor.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some(work) = work else {
            tx.commit().await.map_err(classify)?;
            return Ok(OwnEndorsementFacts {
                work: None,
                endorsement: Endorsement::ABSENT,
                comments: Vec::new(),
            });
        };
        let rows: Vec<CommentRow> = if comments.ids().is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {COMMENT_COLUMNS}, NULL::bigint AS endorsements, e.selected, e.version
                   FROM comments c
                   JOIN discussion_threads t ON t.id = c.thread_id
                   LEFT JOIN comment_endorsements e ON e.comment_id = c.id AND e.actor_id = $3
                  WHERE c.id = ANY($2::uuid[]) AND t.work_id = $1"
            )))
            .bind(work_id.as_uuid())
            .bind(ids(comments))
            .bind(actor.as_uuid())
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?
        };
        tx.commit().await.map_err(classify)?;
        let endorsement = match work.version {
            Some(raw) => {
                Endorsement::restore(work.anchor.map(RevisionId::from_uuid), version(raw)?)
            }
            None => Endorsement::ABSENT,
        };
        let work_public = work.published.is_some();
        Ok(OwnEndorsementFacts {
            work: Some(work.facts()),
            endorsement,
            comments: rows
                .iter()
                .map(|row| {
                    Ok(OwnCommentFacts {
                        facts: row.facts(work_public)?,
                        endorsement: row.endorsement()?,
                    })
                })
                .collect::<Result<_, StoreError>>()?,
        })
    }
}
