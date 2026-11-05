//! PostgreSQL adapter of literary endorsement (LVE, #73). Each write is one transaction:
//! current facts under locks → the exact pure decision (`decide_endorsement`) → the accepted
//! state and its receipt written atomically → commit → the durable answer. The schema
//! (`migrations/20261105000000_literary_endorsement.sql`, `20261105120000_…_parity.sql`) holds
//! the structural rules on its own: typed target foreign keys, one row per (actor, target), a
//! revision of the same work, an order that only advances, a selection matching its version's
//! parity (the only reachable states, so SQL counts never see a row no command produced), rows
//! and receipts that are never deleted.
//!
//! Lock order, the prefix of the discussion adapter's: publication (FOR SHARE) → thread (FOR
//! SHARE) → comment (FOR SHARE) → the actor's command-key namespace (transaction advisory lock)
//! → the actor's record (FOR UPDATE). A withdrawal or republish (pointer update) and a
//! moderation decision (comment UPDATE) either commit before the decision reads its facts or wait
//! until it commits; endorsement never takes their rows in the reverse order. The advisory lock
//! makes one command key have one winner across ALL targets of the actor, which a lock on the
//! target alone could not. No network call happens inside these transactions.
//!
//! An absent record is not a lock on a future row. A first command decides from the initial
//! state and INSERTs with `ON CONFLICT DO NOTHING`; if a concurrent first command (another key)
//! committed meanwhile, the INSERT writes nothing and the transaction reads the winner's row under
//! FOR UPDATE and decides again. The UPDATE path is conditional on the order it decided from.
//!
//! The current capability is "the principal has a Vink record": no ban, quarantine or revocation
//! mechanism exists yet, so there is nothing else to recheck. Counts are read from the canonical
//! rows in the same transaction (CHOICE-05 A).

use sqlx::{AssertSqlSafe, Postgres, Transaction};
use uuid::Uuid;

use super::PgStore;
use super::discussion::{integrity, read_snapshot, state};
use super::postgres::classify;
use crate::application::endorsement::CommentBatch;
use crate::application::ports::{
    CommentSummaryFacts, EndorsementStore, EndorsementSummaryFacts, EndorsementWriteRecord,
    OwnCommentFacts, OwnEndorsementFacts, StoreError,
};
use crate::domain::discussion::{CommentPlacement, StoredPosition};
use crate::domain::endorsement::{
    AcceptedOrder, EndorsementCommand, EndorsementCommandKey, EndorsementCount, EndorsementFacts,
    EndorsementIntent, EndorsementOutcome, EndorsementReceipt, EndorsementState, EndorsementTarget,
    EndorsementTargetFacts, StoredEndorsement, decide_endorsement,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, ThreadId, WorkId};

/// A first command can lose the INSERT to a concurrent first command at most once: afterwards
/// the winner's row exists (rows are never deleted) and every read locks it.
const CLAIM_ATTEMPTS: usize = 2;

/// Seed of the advisory-lock hash for the endorsement command-key namespace.
const KEY_LOCK_SEED: i64 = 7_313;

fn tokens(selected: bool, version: i64, order: i64) -> Result<EndorsementState, StoreError> {
    EndorsementState::restore(selected, version, order)
        .ok_or_else(|| integrity("stored endorsement tokens", format!("{version}/{order}")))
}

fn count(raw: i64) -> Result<EndorsementCount, StoreError> {
    EndorsementCount::new(raw).ok_or_else(|| integrity("endorsement count", raw))
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

fn placement(
    id: Uuid,
    thread_id: Uuid,
    root_id: Option<Uuid>,
    reply_to_id: Option<Uuid>,
) -> Result<CommentPlacement, StoreError> {
    let position = match root_id {
        None => StoredPosition::Root,
        Some(root_id) => StoredPosition::Reply {
            root_id: CommentId::from_uuid(root_id),
            reply_to: reply_to_id.map(CommentId::from_uuid),
        },
    };
    CommentPlacement::from_stored(
        CommentId::from_uuid(id),
        ThreadId::from_uuid(thread_id),
        position,
    )
    .map_err(|error| integrity("stored placement", error))
}

/// A requested or targeted comment of the work's thread, with that thread's identity and either
/// its public count (summaries) or the actor's row (own state).
#[derive(sqlx::FromRow)]
struct CommentRow {
    id: Uuid,
    root_id: Option<Uuid>,
    reply_to_id: Option<Uuid>,
    author_id: Uuid,
    review: String,
    removal: Option<String>,
    moderation_version: i64,
    thread_id: Uuid,
    work_id: Uuid,
    endorsements: Option<i64>,
    selected: Option<bool>,
    version: Option<i64>,
    accepted_order: Option<i64>,
}

impl CommentRow {
    fn facts(&self, work_public: bool) -> Result<EndorsementTargetFacts, StoreError> {
        EndorsementTargetFacts::comment(
            WorkId::from_uuid(self.work_id),
            ThreadId::from_uuid(self.thread_id),
            placement(self.id, self.thread_id, self.root_id, self.reply_to_id)?,
            PrincipalId::from_uuid(self.author_id),
            state(
                &self.review,
                self.removal.as_deref(),
                self.moderation_version,
            )?,
            work_public,
        )
        .ok_or_else(|| integrity("comment outside its thread", self.id))
    }

    fn own_state(&self) -> Result<Option<EndorsementState>, StoreError> {
        match (self.selected, self.version, self.accepted_order) {
            (Some(selected), Some(version), Some(order)) => {
                tokens(selected, version, order).map(Some)
            }
            (None, None, None) => Ok(None),
            _ => Err(integrity("partial comment endorsement row", self.id)),
        }
    }
}

const COMMENT_COLUMNS: &str = "c.id, c.root_id, c.reply_to_id, c.author_id, c.review, \
     c.removal, c.moderation_version, t.id AS thread_id, t.work_id";

/// The work, its current publication's revision and either the effective count of that revision
/// (summaries) or the actor's row for it (own state).
#[derive(sqlx::FromRow)]
struct WorkRow {
    id: Uuid,
    owner_id: Uuid,
    published: Option<Uuid>,
    effective: Option<i64>,
    selected: Option<bool>,
    version: Option<i64>,
    accepted_order: Option<i64>,
}

impl WorkRow {
    fn facts(&self) -> EndorsementTargetFacts {
        EndorsementTargetFacts::work(
            WorkId::from_uuid(self.id),
            PrincipalId::from_uuid(self.owner_id),
            self.published.map(RevisionId::from_uuid),
        )
    }
}

#[derive(sqlx::FromRow)]
struct ReceiptRow {
    work_id: Option<Uuid>,
    revision_id: Option<Uuid>,
    comment_id: Option<Uuid>,
    comment_work: Option<Uuid>,
    expected_order: i64,
    selected: bool,
    committed_version: i64,
    committed_order: i64,
}

impl ReceiptRow {
    fn receipt(
        self,
        actor: PrincipalId,
        key: EndorsementCommandKey,
    ) -> Result<EndorsementReceipt, StoreError> {
        let target = match (
            self.work_id,
            self.revision_id,
            self.comment_id,
            self.comment_work,
        ) {
            (Some(work), Some(revision), None, _) => EndorsementTarget::Work {
                work: WorkId::from_uuid(work),
                revision: RevisionId::from_uuid(revision),
            },
            (None, None, Some(comment), Some(work)) => EndorsementTarget::Comment {
                work: WorkId::from_uuid(work),
                comment: CommentId::from_uuid(comment),
            },
            _ => return Err(integrity("receipt target", key.as_uuid())),
        };
        let command = EndorsementCommand {
            key,
            target,
            expected: AcceptedOrder::new(self.expected_order)
                .ok_or_else(|| integrity("receipt expected order", self.expected_order))?,
            intent: if self.selected {
                EndorsementIntent::Select
            } else {
                EndorsementIntent::Remove
            },
        };
        let committed = tokens(self.selected, self.committed_version, self.committed_order)?;
        EndorsementReceipt::restore(actor, command, committed)
            .ok_or_else(|| integrity("receipt does not match its command", key.as_uuid()))
    }
}

/// The actor's record of `target` under FOR UPDATE.
async fn locked_record(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
    target: EndorsementTarget,
) -> Result<Option<EndorsementState>, StoreError> {
    let row: Option<(bool, i64, i64)> = match target {
        EndorsementTarget::Work { work, revision } => sqlx::query_as(
            "SELECT selected, version, accepted_order FROM work_endorsements
              WHERE work_id = $1 AND revision_id = $2 AND actor_id = $3 FOR UPDATE",
        )
        .bind(work.as_uuid())
        .bind(revision.as_uuid())
        .bind(actor.as_uuid()),
        EndorsementTarget::Comment { comment, .. } => sqlx::query_as(
            "SELECT selected, version, accepted_order FROM comment_endorsements
              WHERE comment_id = $1 AND actor_id = $2 FOR UPDATE",
        )
        .bind(comment.as_uuid())
        .bind(actor.as_uuid()),
    }
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    row.map(|(selected, version, order)| tokens(selected, version, order))
        .transpose()
}

/// Writes the accepted state: a conditional UPDATE of the row decided from, or a first INSERT.
/// `false` when a concurrent first command committed the row first (nothing written).
async fn write_record(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
    target: EndorsementTarget,
    from: Option<EndorsementState>,
    next: EndorsementState,
) -> Result<bool, StoreError> {
    let query = match (target, from) {
        (EndorsementTarget::Work { work, revision }, Some(from)) => sqlx::query(
            "UPDATE work_endorsements
                SET selected = $4, version = $5, accepted_order = $6, updated_at = now()
              WHERE work_id = $1 AND revision_id = $2 AND actor_id = $3 AND accepted_order = $7",
        )
        .bind(work.as_uuid())
        .bind(revision.as_uuid())
        .bind(actor.as_uuid())
        .bind(next.selected())
        .bind(next.version().get())
        .bind(next.order().get())
        .bind(from.order().get()),
        (EndorsementTarget::Work { work, revision }, None) => sqlx::query(
            "INSERT INTO work_endorsements
                    (work_id, revision_id, actor_id, selected, version, accepted_order)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (work_id, revision_id, actor_id) DO NOTHING",
        )
        .bind(work.as_uuid())
        .bind(revision.as_uuid())
        .bind(actor.as_uuid())
        .bind(next.selected())
        .bind(next.version().get())
        .bind(next.order().get()),
        (EndorsementTarget::Comment { comment, .. }, Some(from)) => sqlx::query(
            "UPDATE comment_endorsements
                SET selected = $3, version = $4, accepted_order = $5, updated_at = now()
              WHERE comment_id = $1 AND actor_id = $2 AND accepted_order = $6",
        )
        .bind(comment.as_uuid())
        .bind(actor.as_uuid())
        .bind(next.selected())
        .bind(next.version().get())
        .bind(next.order().get())
        .bind(from.order().get()),
        (EndorsementTarget::Comment { comment, .. }, None) => sqlx::query(
            "INSERT INTO comment_endorsements
                    (comment_id, actor_id, selected, version, accepted_order)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (comment_id, actor_id) DO NOTHING",
        )
        .bind(comment.as_uuid())
        .bind(actor.as_uuid())
        .bind(next.selected())
        .bind(next.version().get())
        .bind(next.order().get()),
    };
    let affected = query
        .execute(&mut **tx)
        .await
        .map_err(classify)?
        .rows_affected();
    if from.is_some() && affected != 1 {
        // The row is locked FOR UPDATE: a mismatch here is not a race but a broken invariant.
        return Err(integrity("locked endorsement changed", actor));
    }
    Ok(affected == 1)
}

async fn write_receipt(
    tx: &mut Transaction<'_, Postgres>,
    receipt: EndorsementReceipt,
) -> Result<(), StoreError> {
    let command = receipt.command();
    let (work, revision, comment) = match command.target {
        EndorsementTarget::Work { work, revision } => {
            (Some(work.as_uuid()), Some(revision.as_uuid()), None)
        }
        EndorsementTarget::Comment { comment, .. } => (None, None, Some(comment.as_uuid())),
    };
    sqlx::query(
        "INSERT INTO endorsement_receipts
                (actor_id, command_key, work_id, revision_id, comment_id, expected_order,
                 selected, committed_version, committed_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(receipt.actor().as_uuid())
    .bind(command.key.as_uuid())
    .bind(work)
    .bind(revision)
    .bind(comment)
    .bind(command.expected.get())
    .bind(command.intent.selected())
    .bind(receipt.committed().version().get())
    .bind(receipt.committed().order().get())
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

impl EndorsementStore for PgStore {
    async fn set_endorsement(
        &self,
        actor: PrincipalId,
        command: EndorsementCommand,
    ) -> Result<EndorsementWriteRecord, StoreError> {
        let target = command.target;
        let work_id = target.work();
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_principal(&mut tx, actor).await?;
        // A withdraw/republish (pointer DELETE/UPDATE) waits for this decision, or committed
        // before it and is what this read sees.
        let published: Option<RevisionId> =
            sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR SHARE")
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?
                .map(RevisionId::from_uuid);
        // The owner never changes; the loaded row's identity is the fact, never the request's.
        let owner: Option<(Uuid, Uuid)> =
            sqlx::query_as("SELECT id, owner_id FROM works WHERE id = $1")
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let target_facts = match target {
            EndorsementTarget::Work { .. } => owner.map(|(id, owner)| {
                EndorsementTargetFacts::work(
                    WorkId::from_uuid(id),
                    PrincipalId::from_uuid(owner),
                    published,
                )
            }),
            EndorsementTarget::Comment { comment, .. } => {
                // Share lock on the thread of this work, then the comment INSIDE it, so an ID of
                // another work's comment finds nothing. Its work and thread come from its own
                // thread row, so the pure binding check refuses a foreign comment too.
                let thread: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM discussion_threads WHERE work_id = $1 FOR SHARE",
                )
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
                let row: Option<CommentRow> = match thread {
                    Some(thread) => sqlx::query_as(AssertSqlSafe(format!(
                        "SELECT {COMMENT_COLUMNS}, NULL::bigint AS endorsements,
                                NULL::boolean AS selected, NULL::bigint AS version,
                                NULL::bigint AS accepted_order
                           FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
                          WHERE c.id = $1 AND c.thread_id = $2
                            FOR SHARE OF c"
                    )))
                    .bind(comment.as_uuid())
                    .bind(thread)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?,
                    None => None,
                };
                row.map(|row| row.facts(published.is_some())).transpose()?
            }
        };
        // A comment record is read only for a comment of this work: a retained row of another
        // work's comment is never addressed through this work's route.
        let addressable = match target {
            EndorsementTarget::Work { .. } => true,
            EndorsementTarget::Comment { .. } => target_facts.is_some(),
        };

        // One winner per (actor, command key) across every target of the actor.
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1::text || '/' || $2::text, $3))",
        )
        .bind(actor.as_uuid())
        .bind(command.key.as_uuid())
        .bind(KEY_LOCK_SEED)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        let receipt: Option<ReceiptRow> = sqlx::query_as(
            "SELECT r.work_id, r.revision_id, r.comment_id, t.work_id AS comment_work,
                    r.expected_order, r.selected, r.committed_version, r.committed_order
               FROM endorsement_receipts r
               LEFT JOIN comments c ON c.id = r.comment_id
               LEFT JOIN discussion_threads t ON t.id = c.thread_id
              WHERE r.actor_id = $1 AND r.command_key = $2",
        )
        .bind(actor.as_uuid())
        .bind(command.key.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let receipt = receipt
            .map(|row| row.receipt(actor, command.key))
            .transpose()?;

        for _ in 0..CLAIM_ATTEMPTS {
            let current = if addressable {
                locked_record(&mut tx, actor, target).await?
            } else {
                None
            };
            let facts = EndorsementFacts {
                can_endorse: true,
                target: target_facts,
                current: current.map(|state| StoredEndorsement::restore(actor, target, state)),
                receipt,
            };
            let outcome = decide_endorsement(actor, command, facts);
            if let EndorsementOutcome::Accepted(write) = outcome {
                if !write_record(&mut tx, actor, target, current, write.next().state()).await? {
                    // A concurrent first command committed its row: decide again on it.
                    continue;
                }
                write_receipt(&mut tx, write.receipt()).await?;
            }
            let visible = target_facts.is_some_and(|facts| facts.visible(target));
            let count = if visible {
                let raw: i64 = match target {
                    EndorsementTarget::Work { work, revision } => sqlx::query_scalar(
                        "SELECT count(*) FROM work_endorsements
                          WHERE work_id = $1 AND revision_id = $2 AND selected",
                    )
                    .bind(work.as_uuid())
                    .bind(revision.as_uuid()),
                    EndorsementTarget::Comment { comment, .. } => sqlx::query_scalar(
                        "SELECT count(*) FROM comment_endorsements
                          WHERE comment_id = $1 AND selected",
                    )
                    .bind(comment.as_uuid()),
                }
                .fetch_one(&mut *tx)
                .await
                .map_err(classify)?;
                count(raw)?
            } else {
                EndorsementCount::ZERO
            };
            tx.commit().await.map_err(classify)?;
            return Ok(EndorsementWriteRecord {
                facts,
                outcome,
                count,
                published,
            });
        }
        Err(integrity("endorsement claim did not settle", actor))
    }

    async fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<EndorsementSummaryFacts, StoreError> {
        // One read-only snapshot: the work's publication, the comments' states and every count
        // agree with each other. Two statements whatever the batch size.
        let mut tx = read_snapshot(self.pool()).await?;
        let work: Option<WorkRow> = sqlx::query_as(
            "SELECT w.id, w.owner_id, p.revision_id AS published,
                    (SELECT count(*) FROM work_endorsements e
                      WHERE e.work_id = w.id AND e.revision_id = p.revision_id
                        AND e.selected) AS effective,
                    NULL::boolean AS selected, NULL::bigint AS version,
                    NULL::bigint AS accepted_order
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
                published: None,
                work_count: EndorsementCount::ZERO,
                comments: Vec::new(),
            });
        };
        let rows: Vec<CommentRow> = if comments.ids().is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(AssertSqlSafe(format!(
                "SELECT {COMMENT_COLUMNS},
                        (SELECT count(*) FROM comment_endorsements e
                          WHERE e.comment_id = c.id AND e.selected) AS endorsements,
                        NULL::boolean AS selected, NULL::bigint AS version,
                        NULL::bigint AS accepted_order
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
            published: work.published.map(RevisionId::from_uuid),
            work_count: count(work.effective.unwrap_or_default())?,
            comments: rows
                .iter()
                .map(|row| {
                    Ok(CommentSummaryFacts {
                        comment_id: CommentId::from_uuid(row.id),
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
                    e.selected, e.version, e.accepted_order
               FROM works w
               LEFT JOIN publications p ON p.work_id = w.id
               LEFT JOIN work_endorsements e ON e.work_id = w.id
                     AND e.revision_id = p.revision_id AND e.actor_id = $2
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
                can_endorse: true,
                work: None,
                published: None,
                work_state: None,
                comments: Vec::new(),
            });
        };
        let rows: Vec<CommentRow> = if comments.ids().is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(AssertSqlSafe(format!(
                "SELECT {COMMENT_COLUMNS}, NULL::bigint AS endorsements,
                        e.selected, e.version, e.accepted_order
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
        let work_state = match (work.selected, work.version, work.accepted_order) {
            (Some(selected), Some(version), Some(order)) => Some(tokens(selected, version, order)?),
            (None, None, None) => None,
            _ => return Err(integrity("partial work endorsement row", work.id)),
        };
        let work_public = work.published.is_some();
        Ok(OwnEndorsementFacts {
            can_endorse: true,
            work: Some(work.facts()),
            published: work.published.map(RevisionId::from_uuid),
            work_state,
            comments: rows
                .iter()
                .map(|row| {
                    Ok(OwnCommentFacts {
                        comment_id: CommentId::from_uuid(row.id),
                        facts: row.facts(work_public)?,
                        state: row.own_state()?,
                    })
                })
                .collect::<Result<_, StoreError>>()?,
        })
    }
}
