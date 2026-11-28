//! App-side reads of the Notifications Gateway (#348): the batch facts that decide what a viewer
//! may see of inbox groups (T-06 CHOICE-20 A), the private decision details of P-12A and the
//! public Thấm list of P-08 R3. Every read is one REPEATABLE READ snapshot with a fixed number of
//! statements, whatever the number of groups (no N+1).

use std::collections::HashMap;

use sqlx::{PgConnection, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::discussion::read_snapshot;
use super::postgres::classify;
use crate::application::ports::StoreError;
use crate::application::profile::AuthorAttribution;
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};
use crate::domain::notification_inbox::{CommentFacts, ReadFacts, Readable, WorkFacts};

/// Largest page of the public Thấm list.
pub const MAX_ENDORSER_PAGE: i64 = 50;

/// A page of the people currently Thấm-ing a public revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorserPage {
    pub total_count: i64,
    pub items: Vec<AuthorAttribution>,
    /// The opaque position after the last item, when more members follow.
    pub next: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndorserRead {
    Page(EndorserPage),
    /// No public revision, or a first page asked for a revision that is not the public one.
    NotFound,
    /// A continued cursor whose revision is no longer the public one.
    RevisionChanged(RevisionId),
}

/// The private detail of a work decision for its owner (P-12A, P-11a/b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkDecisionDetail {
    pub title: String,
    pub outcome: &'static str,
    pub action_required: bool,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub can_appeal: bool,
    /// Whether this decision is still the revision's current one (a later one reversed it).
    pub is_current: bool,
    pub decided_at: OffsetDateTime,
}

/// The revision/version identity of one item in the author's decision history.
pub struct VersionedWorkDecision {
    pub revision_id: Uuid,
    pub decision_version: i64,
    pub detail: WorkDecisionDetail,
}

fn work_decision_query(
    viewer: PrincipalId,
    work: WorkId,
    revision: Option<RevisionId>,
    decision_version: Option<i64>,
) -> sqlx::query::Query<'static, sqlx::Postgres, sqlx::postgres::PgArguments> {
    sqlx::query(
            "SELECT h.revision_id, h.decision_version, h.moderation, h.provenance_kind, h.policy_reason_code, h.reason, h.created_at,
                    r.title,
                    (m.decision_version = h.decision_version) AS is_current,
                    (m.decision_version = h.decision_version AND m.moderation = 'rejected'
                     AND m.provenance_kind IN ('ai_rejected', 'moderator')
                     AND wp.audience = 'public' AND wp.lifecycle = 'active') AS can_appeal
             FROM work_revision_decision_history h
             JOIN works w ON w.id = h.work_id AND w.owner_id = $4
             JOIN revisions r ON r.work_id = h.work_id AND r.id = h.revision_id
             JOIN work_revision_moderation m
               ON m.work_id = h.work_id AND m.revision_id = h.revision_id
             LEFT JOIN work_publications wp ON wp.work_id = h.work_id
             WHERE h.work_id = $1 AND ($2::uuid IS NULL OR h.revision_id = $2)
               AND ($3::bigint IS NULL OR h.decision_version = $3)
               AND h.moderation IN ('approved', 'rejected')
             ORDER BY h.revision_id, h.decision_version DESC",
    )
    .bind(work.as_uuid())
    .bind(revision.map(|id| id.as_uuid()))
    .bind(decision_version)
    .bind(viewer.as_uuid())
}

fn work_decision_from_row(row: &sqlx::postgres::PgRow) -> Result<WorkDecisionDetail, StoreError> {
    let moderation: String = row.try_get("moderation").map_err(classify)?;
    let provenance: String = row.try_get("provenance_kind").map_err(classify)?;
    let rejected = moderation == "rejected";
    // Only what the author already sees on their publication state (MOD-04): the policy
    // code of an AI rejection, or the human reviewer's stated reason. Never who decided.
    let (reason_code, reason_message) = match provenance.as_str() {
        "ai_rejected" => (row.try_get("policy_reason_code").map_err(classify)?, None),
        "admin" | "moderator" => (None, row.try_get("reason").map_err(classify)?),
        _ => (None, None),
    };
    Ok(WorkDecisionDetail {
        title: row.try_get("title").map_err(classify)?,
        outcome: if rejected { "rejected" } else { "approved" },
        action_required: rejected,
        reason_code,
        reason_message,
        can_appeal: row
            .try_get::<Option<bool>, _>("can_appeal")
            .map_err(classify)?
            .unwrap_or(false),
        is_current: row.try_get("is_current").map_err(classify)?,
        decided_at: row.try_get("created_at").map_err(classify)?,
    })
}

/// The same author-visible projection as the notification detail, using the caller's snapshot.
pub(super) async fn author_work_decisions(
    conn: &mut PgConnection,
    viewer: PrincipalId,
    work: WorkId,
) -> Result<Vec<VersionedWorkDecision>, StoreError> {
    work_decision_query(viewer, work, None, None)
        .fetch_all(conn)
        .await
        .map_err(classify)?
        .iter()
        .map(|row| {
            Ok(VersionedWorkDecision {
                revision_id: row.try_get("revision_id").map_err(classify)?,
                decision_version: row.try_get("decision_version").map_err(classify)?,
                detail: work_decision_from_row(row)?,
            })
        })
        .collect()
}

/// The private detail of an adverse comment decision for its writer (P-12A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDecisionDetail {
    pub work: WorkId,
    pub work_title: Option<String>,
    pub outcome: &'static str,
    pub reason_message: Option<String>,
    /// The writer's own text while it is still present; `None` once removed (never restored).
    pub comment_body: Option<String>,
    pub body_format: Option<crate::domain::discussion::body::CommentFormat>,
    pub is_current: bool,
    pub decided_at: OffsetDateTime,
}

fn attribution(row: &sqlx::postgres::PgRow) -> Result<AuthorAttribution, sqlx::Error> {
    let name: Option<String> = row.try_get("display_name")?;
    let handle: Option<String> = row.try_get("handle")?;
    let retired: Option<bool> = row.try_get("retired")?;
    Ok(match name {
        Some(name) => AuthorAttribution::project(name, handle, retired.unwrap_or(false)),
        // No public profile row at all: a neutral label, never an identifier.
        None => AuthorAttribution::Unavailable(UNAVAILABLE_LABEL.into()),
    })
}

/// Label of a reader whose public identity cannot be shown (contract example 5).
pub const UNAVAILABLE_LABEL: &str = "Người đọc Vink";

impl PgStore {
    /// Everything the candidates of one inbox batch need, in one snapshot: works (owner, readable
    /// revision, epoch, title), comments (visibility and, only when public on a readable work,
    /// body), owner-only revision titles of decisions, and public identities of actors.
    pub async fn notification_facts(
        &self,
        viewer: PrincipalId,
        works: &[WorkId],
        comments: &[CommentId],
        decision_revisions: &[(WorkId, RevisionId)],
        comment_decisions: &[(CommentId, i64)],
        actors: &[PrincipalId],
    ) -> Result<(ReadFacts, HashMap<PrincipalId, AuthorAttribution>), StoreError> {
        let mut facts = ReadFacts::default();
        let mut identities = HashMap::new();
        let mut tx = read_snapshot(self.pool()).await?;
        if !works.is_empty() {
            let ids: Vec<Uuid> = works.iter().map(|w| w.as_uuid()).collect();
            let rows = sqlx::query(
                "SELECT w.id, w.owner_id, p.revision_id, r.title,
                        COALESCE(wp.public_epoch, 0) AS public_epoch
                 FROM works w
                 LEFT JOIN publications p ON p.work_id = w.id
                 LEFT JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
                 LEFT JOIN work_publications wp ON wp.work_id = w.id
                 WHERE w.id = ANY($1)",
            )
            .bind(&ids)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
            for row in rows {
                let revision: Option<Uuid> = row.try_get("revision_id").map_err(classify)?;
                let title: Option<String> = row.try_get("title").map_err(classify)?;
                let readable = match (revision, title) {
                    (Some(revision), Some(title)) => Some(Readable {
                        revision: RevisionId::from_uuid(revision),
                        epoch: row.try_get("public_epoch").map_err(classify)?,
                        title,
                    }),
                    _ => None,
                };
                facts.works.insert(
                    WorkId::from_uuid(row.try_get("id").map_err(classify)?),
                    WorkFacts {
                        owner: PrincipalId::from_uuid(row.try_get("owner_id").map_err(classify)?),
                        readable,
                    },
                );
            }
        }
        if !comments.is_empty() {
            let ids: Vec<Uuid> = comments.iter().map(|c| c.as_uuid()).collect();
            let rows = sqlx::query(
                "SELECT c.id, t.work_id, c.author_id, c.moderation_version, c.body_format,
                        (c.review = 'approved' AND c.removal IS NULL) AS public,
                        CASE WHEN c.review = 'approved' AND c.removal IS NULL
                                  AND EXISTS (SELECT 1 FROM publications p
                                              WHERE p.work_id = t.work_id)
                             THEN c.body END AS body
                 FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
                 WHERE c.id = ANY($1)",
            )
            .bind(&ids)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
            for row in rows {
                let format = super::discussion::stored_format(
                    row.try_get("body_format").map_err(classify)?,
                )?;
                let source: Option<String> = row.try_get("body").map_err(classify)?;
                facts.comments.insert(
                    CommentId::from_uuid(row.try_get("id").map_err(classify)?),
                    CommentFacts {
                        work: WorkId::from_uuid(row.try_get("work_id").map_err(classify)?),
                        author: PrincipalId::from_uuid(row.try_get("author_id").map_err(classify)?),
                        public: row.try_get("public").map_err(classify)?,
                        moderation_version: row.try_get("moderation_version").map_err(classify)?,
                        body: source.map(|text| format.visible_text(&text)),
                    },
                );
            }
        }
        if !decision_revisions.is_empty() {
            let (work_ids, revision_ids): (Vec<Uuid>, Vec<Uuid>) = decision_revisions
                .iter()
                .map(|(w, r)| (w.as_uuid(), r.as_uuid()))
                .unzip();
            let rows = sqlx::query(
                "SELECT r.id, r.title, r.created_at
                 FROM unnest($1::uuid[], $2::uuid[]) AS wanted(work_id, revision_id)
                 JOIN revisions r ON r.work_id = wanted.work_id AND r.id = wanted.revision_id
                 JOIN works w ON w.id = r.work_id AND w.owner_id = $3",
            )
            .bind(&work_ids)
            .bind(&revision_ids)
            .bind(viewer.as_uuid())
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
            for row in rows {
                let revision = RevisionId::from_uuid(row.try_get("id").map_err(classify)?);
                facts
                    .revision_titles
                    .insert(revision, row.try_get("title").map_err(classify)?);
                facts
                    .revision_dates
                    .insert(revision, row.try_get("created_at").map_err(classify)?);
            }
        }
        if !comment_decisions.is_empty() {
            let (comment_ids, versions): (Vec<Uuid>, Vec<i64>) = comment_decisions
                .iter()
                .map(|(c, v)| (c.as_uuid(), *v))
                .unzip();
            let rows = sqlx::query(
                "SELECT d.comment_id, d.version, d.action
                 FROM unnest($1::uuid[], $2::bigint[]) AS wanted(comment_id, version)
                 JOIN comment_moderation_decisions d
                   ON d.comment_id = wanted.comment_id AND d.version = wanted.version
                 JOIN comments c ON c.id = d.comment_id AND c.author_id = $3
                 WHERE d.action IN ('reject', 'remove')",
            )
            .bind(&comment_ids)
            .bind(&versions)
            .bind(viewer.as_uuid())
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
            for row in rows {
                let action: String = row.try_get("action").map_err(classify)?;
                facts.comment_decisions.insert(
                    (
                        CommentId::from_uuid(row.try_get("comment_id").map_err(classify)?),
                        row.try_get("version").map_err(classify)?,
                    ),
                    action == "remove",
                );
            }
        }
        if !actors.is_empty() {
            let ids: Vec<Uuid> = actors.iter().map(|a| a.as_uuid()).collect();
            let rows = sqlx::query(
                "SELECT wanted.id, a.display_name, a.handle, a.retired
                 FROM unnest($1::uuid[]) AS wanted(id)
                 LEFT JOIN public_author_attribution a ON a.id = wanted.id",
            )
            .bind(&ids)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
            for row in rows {
                identities.insert(
                    PrincipalId::from_uuid(row.try_get("id").map_err(classify)?),
                    attribution(&row).map_err(classify)?,
                );
            }
        }
        tx.commit().await.map_err(classify)?;
        Ok((facts, identities))
    }

    /// The standing fence of the Gateway's inbox commands, read at the App boundary just before
    /// the command goes to the Notification Service (whose database the App cannot lock).
    pub async fn require_active_viewer(&self, viewer: PrincipalId) -> Result<(), StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        super::principal::require_active(&mut tx, viewer).await?;
        tx.commit().await.map_err(classify)
    }

    /// The owner's private detail of one concluded decision version (notification P-12A).
    pub async fn work_decision_detail(
        &self,
        viewer: PrincipalId,
        work: WorkId,
        revision: RevisionId,
        decision_version: i64,
    ) -> Result<Option<WorkDecisionDetail>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let row = work_decision_query(viewer, work, Some(revision), Some(decision_version))
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        row.as_ref().map(work_decision_from_row).transpose()
    }

    /// The writer's private detail of one adverse decision on their comment; `None` for anyone
    /// else (the work's owner included) or a version that was not a rejection or removal.
    pub async fn comment_decision_detail(
        &self,
        viewer: PrincipalId,
        comment: CommentId,
        moderation_version: i64,
    ) -> Result<Option<CommentDecisionDetail>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let row = sqlx::query(
            "SELECT d.action, d.reason, d.decided_at, t.work_id,
                    (c.moderation_version = d.version) AS is_current,
                    CASE WHEN c.removal IS NULL THEN c.body END AS body, c.body_format,
                    (SELECT r.title FROM publications p
                     JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
                     WHERE p.work_id = t.work_id) AS work_title
             FROM comment_moderation_decisions d
             JOIN comments c ON c.id = d.comment_id AND c.author_id = $3
             JOIN discussion_threads t ON t.id = c.thread_id
             WHERE d.comment_id = $1 AND d.version = $2 AND d.action IN ('reject', 'remove')",
        )
        .bind(comment.as_uuid())
        .bind(moderation_version)
        .bind(viewer.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let Some(row) = row else { return Ok(None) };
        let action: String = row.try_get("action").map_err(classify)?;
        let removed = action == "remove";
        let body: Option<String> = if removed {
            None
        } else {
            row.try_get("body").map_err(classify)?
        };
        Ok(Some(CommentDecisionDetail {
            work: WorkId::from_uuid(row.try_get("work_id").map_err(classify)?),
            work_title: row.try_get("work_title").map_err(classify)?,
            outcome: if removed { "removed" } else { "rejected" },
            reason_message: row.try_get("reason").map_err(classify)?,
            body_format: if body.is_some() {
                Some(super::discussion::stored_format(
                    row.try_get("body_format").map_err(classify)?,
                )?)
            } else {
                None
            },
            comment_body: body,
            is_current: row.try_get("is_current").map_err(classify)?,
            decided_at: row.try_get("decided_at").map_err(classify)?,
        }))
    }

    /// One page of the people currently Thấm-ing `revision`, in stable position order. The
    /// public revision is rechecked on every page; membership is Endorsement's current state,
    /// never the inbox (P-08, independent of the 180-day inbox purge).
    pub async fn public_endorsers(
        &self,
        work: WorkId,
        revision: RevisionId,
        after: Option<Uuid>,
        limit: i64,
    ) -> Result<EndorserRead, StoreError> {
        let limit = limit.clamp(1, MAX_ENDORSER_PAGE);
        let mut tx = read_snapshot(self.pool()).await?;
        let published: Option<Uuid> =
            sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1")
                .bind(work.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let read = match published {
            None => EndorserRead::NotFound,
            Some(current) if current != revision.as_uuid() => match after {
                Some(_) => EndorserRead::RevisionChanged(RevisionId::from_uuid(current)),
                None => EndorserRead::NotFound,
            },
            Some(_) => {
                let total_count: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM work_endorsements
                     WHERE work_id = $1 AND revision_id = $2 AND selected",
                )
                .bind(work.as_uuid())
                .bind(revision.as_uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(classify)?;
                let rows = sqlx::query(
                    "SELECT e.cursor_key, a.display_name, a.handle, a.retired
                     FROM work_endorsements e
                     LEFT JOIN public_author_attribution a ON a.id = e.actor_id
                     WHERE e.work_id = $1 AND e.revision_id = $2 AND e.selected
                       AND e.cursor_key > $3
                     ORDER BY e.cursor_key
                     LIMIT $4",
                )
                .bind(work.as_uuid())
                .bind(revision.as_uuid())
                .bind(after.unwrap_or(Uuid::nil()))
                .bind(limit + 1)
                .fetch_all(&mut *tx)
                .await
                .map_err(classify)?;
                let more = rows.len() > usize::try_from(limit).unwrap_or(0);
                let rows = &rows[..rows.len().min(usize::try_from(limit).unwrap_or(0))];
                let next = if more {
                    rows.last()
                        .map(|row| row.try_get("cursor_key"))
                        .transpose()
                        .map_err(classify)?
                } else {
                    None
                };
                let items = rows
                    .iter()
                    .map(attribution)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(classify)?;
                EndorserRead::Page(EndorserPage {
                    total_count,
                    items,
                    next,
                })
            }
        };
        tx.commit().await.map_err(classify)?;
        Ok(read)
    }
}
