//! PostgreSQL adapter of the Vink-owned discussion. Each write is one transaction that locks the
//! rows its decision depends on, loads them as domain facts, calls the pure decision and writes
//! only what that decision returns. The schema enforces the two tiers on its own
//! (`migrations/20261103000000_discussion.sql`), so a defect here cannot store a third tier.
//!
//! Lock order, shared by every path:
//! - Discussion writes: publication -> thread -> comment row -> principal row (`FOR UPDATE`).
//! - Moderation decisions: Level 1 directory `FOR SHARE` locks (`moderation_admin_seats` ->
//!   `discussion_moderator_grants` -> `moderation_groups` -> `moderation_group_members` ->
//!   `moderation_grants`) -> Level 2 comment row (`FOR UPDATE`) -> Level 3 actor principal row
//!   (`FOR UPDATE`). Directory mutations take Level 1 -> Level 3, preventing AB-BA deadlocks
//!   while serializing revocation, group removal, seat removal and expiry with decisions.
//!
//! A submission holds share locks on the publication, thread and target rows; a mode change
//! updates the thread row and a moderation decision locks only the comment row, so no two
//! discussion paths wait on each other in reverse. Literary endorsement (`endorsement.rs`) takes
//! the same prefix with share locks and then only its own key lock and row, so it never waits
//! on these paths in reverse either.
//!
//! Every write then takes the standing fence (#120): the actor's principal row FOR UPDATE
//! (`principal::locked_standing`, or `require_active` for writes whose decision does not model
//! standing: own removal, mode change, moderation), which a suspension's foreign key waits for.
//! It comes after the row locks of that path and before its first write. For a submission it
//! comes after the shared locks, so a moderator or writer holding a comment row while touching their own
//! principal through a foreign key never waits on a submission that waits for that comment row,
//! and before the receipt and comment inserts, whose foreign keys reference the principal. The
//! receipt claim and the insert come last. Admission runs inside `decide_submission`; the store
//! writes exactly its initial state and provenance (migration `20261111000000_comment_admission`
//! refuses anything else). A created comment's `comment.submitted` event and deliveries
//! (`outbox::enqueue`, DISC-04) are new rows written after the insert, before the commit; they
//! take no lock that another discussion path holds.

use sqlx::postgres::PgPool;
// Dynamic statements below are concatenations of this module's constants, never input.
use sqlx::{AssertSqlSafe, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::moderation::load_moderation_directory;
use super::outbox;
use super::postgres::classify;
use super::principal::{locked_standing, require_active};
use crate::application::discussion::{
    AuthorName, CommentItem, Guarded, ModerationAction, ModerationQueue, NewComment, OwnComment,
    OwnPage, OwnPlacement, OwnRead, OwnStatus, QueueItem, ReplyPage, RootEntry, RootItem, RootPage,
    SubmitOutcome,
};
use crate::application::ports::{DiscussionStore, StoreError};
use crate::domain::discussion::admission::AdmissionFacts;
use crate::domain::discussion::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Removal, Review, Transition,
};
use crate::domain::discussion::moderation::{GrantNote, ModerationReason};
use crate::domain::discussion::outbox::EventKind;
use crate::domain::discussion::page::{PREVIEW_REPLIES, PageCursor, PageLimit};
use crate::domain::discussion::policy::{Denied, authorize_mode_change, authorize_writer_removal};
use crate::domain::discussion::settings::{
    CommentsMode, DiscussionSettings, ModeChange, SettingsError, SettingsGeneration,
};
use crate::domain::discussion::submission::{
    PayloadDigest, Receipt, SubmissionDecision, SubmissionFacts, TargetFacts, decide_submission,
};
use crate::domain::discussion::visibility::{
    CommentView, ReplyContext, RootListing, own_overlay, reply_context, reply_listed, root_listing,
    view,
};
use crate::domain::discussion::{CommentPlacement, StoredPosition};
use crate::domain::ids::{CommentId, PrincipalId, ThreadId, WorkId};
use crate::domain::moderation::authority::{
    AuthorityBasis, AuthorityRefusal, GrantOrigin, ModerationDirectory, ReviewCapability,
    ReviewTarget,
};

const SUBMIT: &str = "submit_comment";

/// The adapter's clock for admission facts, in microseconds since the Unix epoch (saturating).
fn now_micros(now: OffsetDateTime) -> i64 {
    i64::try_from(now.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX)
}

const fn action_code(action: ModerationAction) -> &'static str {
    match action {
        ModerationAction::Approve => "approve",
        ModerationAction::Reject => "reject",
        ModerationAction::Remove => "remove",
    }
}

#[derive(sqlx::FromRow)]
struct QueueRow {
    id: Uuid,
    work_id: Uuid,
    work_title: Option<String>,
    is_root: bool,
    author_id: Uuid,
    display_name: String,
    body: Option<String>,
    created_at: OffsetDateTime,
    moderation_version: i64,
    owner_id: Uuid,
}

impl QueueRow {
    fn item(
        self,
        moderator: PrincipalId,
        directory: &ModerationDirectory,
        now: OffsetDateTime,
    ) -> Result<QueueItem, StoreError> {
        let writer = PrincipalId::from_uuid(self.author_id);
        let work_id = WorkId::from_uuid(self.work_id);
        let work_author = PrincipalId::from_uuid(self.owner_id);
        let can_decide = directory
            .resolve(
                moderator,
                &ReviewTarget::comment(work_id, writer, work_author),
                now,
            )
            .is_ok();
        Ok(QueueItem {
            comment_id: CommentId::from_uuid(self.id),
            work_id,
            work_title: self.work_title,
            is_root: self.is_root,
            writer: AuthorName {
                id: writer,
                display_name: self.display_name,
            },
            body: self
                .body
                .ok_or_else(|| integrity("pending comment without text", self.id))?,
            created_at: self.created_at,
            version: ModerationVersion::new(self.moderation_version)
                .ok_or_else(|| integrity("stored version", self.moderation_version))?,
            can_decide,
        })
    }
}

/// Level 1 directory share locks for `actor` in canonical table order before locking any comment
/// or principal row: `moderation_admin_seats` -> `discussion_moderator_grants` ->
/// `moderation_groups` -> `moderation_group_members` -> `moderation_grants`.
async fn lock_actor_authority(
    conn: &mut sqlx::PgConnection,
    actor: PrincipalId,
) -> Result<(), StoreError> {
    let actor_id = actor.as_uuid();
    let _ = sqlx::query_scalar::<_, Uuid>(
        "SELECT principal_id FROM moderation_admin_seats WHERE principal_id = $1 FOR SHARE",
    )
    .bind(actor_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    let _ = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM discussion_moderator_grants
          WHERE principal_id = $1 AND revoked_at IS NULL
          ORDER BY id FOR SHARE",
    )
    .bind(actor_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    let group_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT g.id
           FROM moderation_groups g
           JOIN moderation_group_members m ON m.group_id = g.id
          WHERE m.principal_id = $1
          ORDER BY g.id FOR SHARE OF g",
    )
    .bind(actor_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    let _ = sqlx::query_scalar::<_, Uuid>(
        "SELECT group_id FROM moderation_group_members
          WHERE principal_id = $1
          ORDER BY group_id FOR SHARE",
    )
    .bind(actor_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    let _ = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM moderation_grants
          WHERE NOT revoked
            AND capability = 'comments'
            AND (
                (grantee_kind = 'principal' AND grantee_id = $1)
                OR (grantee_kind = 'group' AND grantee_id = ANY($2))
            )
          ORDER BY id FOR SHARE",
    )
    .bind(actor_id)
    .bind(&group_ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

fn provenance_for_authority(
    directory: &ModerationDirectory,
    basis: AuthorityBasis,
) -> Result<(&'static str, Option<Uuid>, Option<Uuid>), StoreError> {
    match basis {
        AuthorityBasis::Admin => Ok(("admin", None, None)),
        AuthorityBasis::Grant(grant_id) => {
            let grant = directory
                .facts()
                .grants
                .iter()
                .find(|g| g.id == grant_id)
                .ok_or_else(|| integrity("verified grant missing from directory", grant_id))?;
            match grant.origin {
                GrantOrigin::InheritedComment => {
                    Ok(("legacy_grant", Some(grant_id.as_uuid()), None))
                }
                GrantOrigin::Admin => Ok(("governance_grant", None, Some(grant_id.as_uuid()))),
            }
        }
    }
}

pub(super) fn integrity(what: &str, detail: impl std::fmt::Display) -> StoreError {
    let error = StoreError::Integrity(format!("{what}: {detail}"));
    tracing::error!(%error, "discussion integrity error");
    error
}

fn review(raw: &str) -> Result<Review, StoreError> {
    match raw {
        "pending" => Ok(Review::Pending),
        "approved" => Ok(Review::Approved),
        "rejected" => Ok(Review::Rejected),
        other => Err(integrity("stored review", other)),
    }
}

pub(super) const fn review_code(review: Review) -> &'static str {
    match review {
        Review::Pending => "pending",
        Review::Approved => "approved",
        Review::Rejected => "rejected",
    }
}

fn removal(raw: Option<&str>) -> Result<Option<Removal>, StoreError> {
    match raw {
        None => Ok(None),
        Some("author") => Ok(Some(Removal::ByAuthor)),
        Some("moderation") => Ok(Some(Removal::ByModeration)),
        Some(other) => Err(integrity("stored removal", other)),
    }
}

const fn removal_code(removal: Option<Removal>) -> Option<&'static str> {
    match removal {
        None => None,
        Some(Removal::ByAuthor) => Some("author"),
        Some(Removal::ByModeration) => Some("moderation"),
    }
}

pub(super) fn state(
    raw_review: &str,
    raw_removal: Option<&str>,
    version: i64,
) -> Result<CommentState, StoreError> {
    Ok(CommentState::restore(
        review(raw_review)?,
        removal(raw_removal)?,
        ModerationVersion::new(version).ok_or_else(|| integrity("stored version", version))?,
    ))
}

#[derive(sqlx::FromRow)]
struct ThreadRow {
    id: Uuid,
    comments_open: bool,
    settings_generation: i64,
}

impl ThreadRow {
    fn settings(&self) -> Result<DiscussionSettings, StoreError> {
        let generation = SettingsGeneration::new(self.settings_generation)
            .ok_or_else(|| integrity("stored settings generation", self.settings_generation))?;
        let mode = if self.comments_open {
            CommentsMode::Open
        } else {
            CommentsMode::Closed
        };
        Ok(DiscussionSettings::restore(mode, generation))
    }
}

#[derive(sqlx::FromRow)]
struct TargetRow {
    id: Uuid,
    root_id: Option<Uuid>,
    reply_to_id: Option<Uuid>,
    review: String,
    removal: Option<String>,
    moderation_version: i64,
}

#[derive(sqlx::FromRow)]
struct StateRow {
    review: String,
    removal: Option<String>,
    moderation_version: i64,
}

impl StateRow {
    fn state(&self) -> Result<CommentState, StoreError> {
        state(
            &self.review,
            self.removal.as_deref(),
            self.moderation_version,
        )
    }
}

/// A root or reply as read for a viewer, with its context when it is a reply.
#[derive(sqlx::FromRow)]
struct ItemRow {
    root_id: Option<Uuid>,
    id: Uuid,
    author_id: Uuid,
    display_name: String,
    body: Option<String>,
    review: String,
    removal: Option<String>,
    moderation_version: i64,
    created_at: OffsetDateTime,
    reply_to_id: Option<Uuid>,
    replied_to_reply: bool,
    context_review: Option<String>,
    context_removal: Option<String>,
    context_version: Option<i64>,
    context_author_name: Option<String>,
    last_reason: Option<String>,
    public_replies: i64,
}

impl ItemRow {
    fn state(&self) -> Result<CommentState, StoreError> {
        state(
            &self.review,
            self.removal.as_deref(),
            self.moderation_version,
        )
    }

    fn view(&self, viewer: Option<PrincipalId>) -> Result<CommentView, StoreError> {
        Ok(view(
            PrincipalId::from_uuid(self.author_id),
            self.state()?,
            viewer,
        ))
    }

    fn context(&self) -> Result<ReplyContext, StoreError> {
        let context = match (self.reply_to_id, &self.context_review, self.context_version) {
            (Some(id), Some(raw_review), Some(version)) => Some((
                CommentId::from_uuid(id),
                state(raw_review, self.context_removal.as_deref(), version)?,
            )),
            _ => None,
        };
        Ok(reply_context(self.replied_to_reply, context))
    }

    /// Builds a readable item. The SQL filter selected this row; the domain projection must agree,
    /// otherwise the two disagree about visibility and the read fails closed.
    fn item(
        &self,
        viewer: Option<PrincipalId>,
        context: ReplyContext,
    ) -> Result<CommentItem, StoreError> {
        let seen = self.view(viewer)?;
        if !reply_listed(seen) {
            return Err(integrity(
                "visibility filter disagrees with domain projection",
                self.id,
            ));
        }
        Ok(CommentItem {
            id: CommentId::from_uuid(self.id),
            view: seen,
            author: AuthorName {
                id: PrincipalId::from_uuid(self.author_id),
                display_name: self.display_name.clone(),
            },
            body: self
                .body
                .clone()
                .ok_or_else(|| integrity("present comment without text", self.id))?,
            created_at: self.created_at,
            // Only a public context may name its writer.
            context_author: match context {
                ReplyContext::Reply(_) => self.context_author_name.clone(),
                ReplyContext::Root | ReplyContext::Unavailable => None,
            },
            context,
            // The moderator's reason reaches only the writer, only while the comment is rejected.
            rejection_reason: (seen == CommentView::OwnRejected)
                .then(|| self.last_reason.clone())
                .flatten(),
        })
    }
}

/// Columns shared by root and reply reads. `$viewer` is the reading principal or NULL.
const ITEM_COLUMNS: &str = "c.root_id, c.id, c.author_id, a.display_name, c.body, c.review, \
     c.removal, c.moderation_version, c.created_at, c.reply_to_id, c.replied_to_reply, \
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id \
       ORDER BY d.version DESC LIMIT 1) AS last_reason";

/// Public replies of root `c`; equals `visibility::view(..) == Public` for each reply.
const PUBLIC_REPLIES: &str = "(SELECT count(*) FROM comments r WHERE r.root_id = c.id \
     AND r.kind = 'reply' AND r.review = 'approved' AND r.removal IS NULL)";

/// Visible to viewer `$2`: present and (approved or written by the viewer). Mirrors
/// `visibility::view`; the PostgreSQL suite compares the two over every state.
const VISIBLE_TO_VIEWER: &str = "c.removal IS NULL AND (c.review = 'approved' OR c.author_id = $2)";

/// One of the viewer's own comments with what the overlay needs about its root and context.
#[derive(sqlx::FromRow)]
struct OwnRow {
    id: Uuid,
    author_id: Uuid,
    body: Option<String>,
    review: String,
    removal: Option<String>,
    moderation_version: i64,
    created_at: OffsetDateTime,
    root_id: Option<Uuid>,
    root_author_id: Option<Uuid>,
    root_review: Option<String>,
    root_removal: Option<String>,
    root_version: Option<i64>,
    root_public_replies: i64,
    reply_to_id: Option<Uuid>,
    replied_to_reply: bool,
    context_review: Option<String>,
    context_removal: Option<String>,
    context_version: Option<i64>,
    context_author_name: Option<String>,
    last_reason: Option<String>,
}

impl OwnRow {
    /// Re-checks the SQL scope against the domain: a row of another writer, or one the list
    /// should not hold, fails the read instead of reaching the viewer.
    fn item(self, viewer: PrincipalId, unpublished_only: bool) -> Result<OwnComment, StoreError> {
        let current = state(
            &self.review,
            self.removal.as_deref(),
            self.moderation_version,
        )?;
        let Some(own) = own_overlay(PrincipalId::from_uuid(self.author_id), current, viewer) else {
            return Err(integrity("overlay row of another writer", self.id));
        };
        if unpublished_only && !own.unpublished {
            return Err(integrity(
                "overlay filter disagrees with domain projection",
                self.id,
            ));
        }
        let placement = match self.root_id {
            None => OwnPlacement::Root,
            Some(root_id) => {
                let (Some(author), Some(raw_review), Some(version)) = (
                    self.root_author_id,
                    self.root_review.as_deref(),
                    self.root_version,
                ) else {
                    return Err(integrity("reply without its root", self.id));
                };
                let root_state = state(raw_review, self.root_removal.as_deref(), version)?;
                let root_view = view(PrincipalId::from_uuid(author), root_state, Some(viewer));
                let context = reply_context(
                    self.replied_to_reply,
                    match (
                        self.reply_to_id,
                        self.context_review.as_deref(),
                        self.context_version,
                    ) {
                        (Some(id), Some(raw), Some(version)) => Some((
                            CommentId::from_uuid(id),
                            state(raw, self.context_removal.as_deref(), version)?,
                        )),
                        _ => None,
                    },
                );
                OwnPlacement::Reply {
                    root_id: CommentId::from_uuid(root_id),
                    root_listed: root_listing(root_view, self.root_public_replies).is_some(),
                    context,
                    // Only a public context may name its writer.
                    context_author: match context {
                        ReplyContext::Reply(_) => self.context_author_name,
                        ReplyContext::Root | ReplyContext::Unavailable => None,
                    },
                }
            }
        };
        Ok(OwnComment {
            id: CommentId::from_uuid(self.id),
            status: OwnStatus::from(current),
            created_at: self.created_at,
            placement,
            body: if own.text {
                Some(
                    self.body
                        .ok_or_else(|| integrity("present comment without text", self.id))?,
                )
            } else {
                None
            },
            rejection_reason: own.reason.then_some(self.last_reason).flatten(),
        })
    }
}

/// Columns of [`OwnRow`]. `c` is the viewer's comment, `r` its root, `x` its context.
const OWN_COLUMNS: &str = "c.id, c.author_id, c.body, c.review, c.removal, c.moderation_version, \
     c.created_at, c.root_id, r.author_id AS root_author_id, r.review AS root_review, \
     r.removal AS root_removal, r.moderation_version AS root_version, \
     CASE WHEN c.root_id IS NULL THEN 0 ELSE (SELECT count(*) FROM comments pr \
       WHERE pr.root_id = c.root_id AND pr.kind = 'reply' AND pr.review = 'approved' \
         AND pr.removal IS NULL) END AS root_public_replies, \
     c.reply_to_id, c.replied_to_reply, x.review AS context_review, \
     x.removal AS context_removal, x.moderation_version AS context_version, \
     xa.display_name AS context_author_name, \
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id \
       ORDER BY d.version DESC LIMIT 1) AS last_reason";

const OWN_FROM: &str = "comments c
       LEFT JOIN comments r ON r.id = c.root_id
       LEFT JOIN comments x ON x.id = c.reply_to_id
       LEFT JOIN principals xa ON xa.id = x.author_id";

/// Present and unpublished: equals `own_overlay(..).unpublished` for the viewer's own rows.
const OWN_UNPUBLISHED: &str = "c.removal IS NULL AND c.review <> 'approved'";

/// Reads whether the work is public and its thread, if any. `None`: missing or not public.
async fn readable_thread(
    tx: &mut Transaction<'_, Postgres>,
    work_id: WorkId,
) -> Result<Option<(PrincipalId, Option<ThreadRow>)>, StoreError> {
    let work: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT w.owner_id, (p.work_id IS NOT NULL OR (wp.lifecycle = 'active' AND
                      (wp.hidden_revision_id IS NOT NULL OR
                       (wp.audience = 'public' AND wp.review_withdrawn_revision_id IS NOT NULL))))
           FROM works w
           LEFT JOIN publications p ON p.work_id = w.id
           LEFT JOIN work_publications wp ON wp.work_id = w.id
          WHERE w.id = $1",
    )
    .bind(work_id.as_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    let Some((owner, true)) = work else {
        return Ok(None);
    };
    let thread: Option<ThreadRow> = sqlx::query_as(
        "SELECT id, comments_open, settings_generation FROM discussion_threads WHERE work_id = $1",
    )
    .bind(work_id.as_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(Some((PrincipalId::from_uuid(owner), thread)))
}

/// One consistent snapshot for a page: settings, roots, counts and previews agree.
pub(super) async fn read_snapshot(pool: &PgPool) -> Result<Transaction<'_, Postgres>, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    Ok(tx)
}

fn page_bounds(after: Option<PageCursor>) -> (Option<OffsetDateTime>, Option<Uuid>) {
    match after {
        Some(cursor) => (Some(cursor.created_at()), Some(cursor.id().as_uuid())),
        None => (None, None),
    }
}

/// Splits a `limit + 1` fetch into the page and the cursor after its last item.
fn paged<T>(
    mut rows: Vec<T>,
    limit: usize,
    key: impl Fn(&T) -> (OffsetDateTime, Uuid),
) -> (Vec<T>, Option<PageCursor>) {
    let next = (rows.len() > limit).then(|| {
        rows.truncate(limit);
        let (at, id) = key(&rows[limit - 1]);
        PageCursor::after(at, CommentId::from_uuid(id))
    });
    (rows, next)
}

impl DiscussionStore for PgStore {
    async fn submit_comment(
        &self,
        actor: PrincipalId,
        comment: &NewComment,
    ) -> Result<SubmitOutcome, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let work = comment.work_id.as_uuid();

        // A share lock on the pointer serializes with a future withdraw/republish of the work.
        let work_public = sqlx::query_scalar::<_, Uuid>(
            "SELECT work_id FROM publications WHERE work_id = $1 FOR SHARE",
        )
        .bind(work)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?
        .is_some();
        let hidden = if work_public {
            false
        } else {
            sqlx::query_scalar::<_, bool>(
                "SELECT lifecycle = 'active' AND hidden_revision_id IS NOT NULL
                   FROM work_publications WHERE work_id = $1 FOR SHARE",
            )
            .bind(work)
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?
            .unwrap_or(false)
        };
        if work_public {
            // COM-01: one thread per work, bound on first use; a concurrent first comment waits
            // on the unique key and then reuses the winner's thread.
            sqlx::query(
                "INSERT INTO discussion_threads (id, work_id) VALUES ($1, $2)
                 ON CONFLICT (work_id) DO NOTHING",
            )
            .bind(comment.thread_candidate.as_uuid())
            .bind(work)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        }
        // Share lock: a mode change (UPDATE) waits for this submission or vice versa.
        let thread: Option<ThreadRow> = sqlx::query_as(
            "SELECT id, comments_open, settings_generation
               FROM discussion_threads WHERE work_id = $1 FOR SHARE",
        )
        .bind(work)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;

        let thread_id = thread
            .as_ref()
            .map_or(comment.thread_candidate, |row| ThreadId::from_uuid(row.id));
        let target = match (comment.target, &thread) {
            (Some(target), Some(_)) => {
                let row: Option<TargetRow> = sqlx::query_as(
                    "SELECT id, root_id, reply_to_id, review, removal, moderation_version
                       FROM comments WHERE id = $1 AND thread_id = $2 FOR SHARE",
                )
                .bind(target.as_uuid())
                .bind(thread_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
                row.map(|row| target_facts(thread_id, row)).transpose()?
            }
            _ => None,
        };
        // Standing fence: after the shared locks, before any insert that references the actor.
        let standing = locked_standing(&mut tx, actor).await?;

        // Claim the key. A concurrent holder of the same key makes this wait, then read its receipt.
        let claimed = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO discussion_command_receipts
                    (actor_id, operation, command_key, payload_digest, comment_id)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (actor_id, operation, command_key) DO NOTHING
             RETURNING comment_id",
        )
        .bind(actor.as_uuid())
        .bind(SUBMIT)
        .bind(comment.key.as_str())
        .bind(comment.digest.as_bytes().as_slice())
        .bind(comment.comment_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?
        .is_some();
        let receipt = if claimed {
            None
        } else {
            let (digest, comment_id): (Vec<u8>, Uuid) = sqlx::query_as(
                "SELECT payload_digest, comment_id FROM discussion_command_receipts
                  WHERE actor_id = $1 AND operation = $2 AND command_key = $3",
            )
            .bind(actor.as_uuid())
            .bind(SUBMIT)
            .bind(comment.key.as_str())
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
            Some(Receipt {
                digest: PayloadDigest::from_bytes(&digest)
                    .ok_or_else(|| integrity("stored payload digest length", digest.len()))?,
                comment_id: CommentId::from_uuid(comment_id),
            })
        };

        // The pilot has no trust authority or content signals: standing is the only fact with a
        // source, and every policy admits an active principal's comment as pending.
        let admission = AdmissionFacts::standing_only(
            standing.into(),
            comment.digest,
            now_micros(OffsetDateTime::now_utc()),
        );
        let facts = SubmissionFacts {
            receipt,
            work_public,
            settings: thread
                .as_ref()
                .map(ThreadRow::settings)
                .transpose()?
                .unwrap_or(DiscussionSettings::INITIAL),
            target,
            admission,
        };

        let (placement, admission) = match decide_submission(
            thread_id,
            comment.comment_id,
            comment.digest,
            comment.target,
            comment.selection,
            facts,
        ) {
            // No observation store exists yet, so a shadow decision is not recorded; the pilot
            // selection has none.
            SubmissionDecision::Insert {
                placement,
                admission,
                shadow: _,
            } => (placement, admission),
            SubmissionDecision::Refused(refusal) => return Ok(SubmitOutcome::Refused(refusal)),
            SubmissionDecision::Replay(comment_id) => {
                let row: StateRow = sqlx::query_as(
                    "SELECT review, removal, moderation_version FROM comments WHERE id = $1",
                )
                .bind(comment_id.as_uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(classify)?;
                return Ok(SubmitOutcome::Replayed {
                    comment_id,
                    state: row.state()?,
                });
            }
            // Dropping `tx` rolls back the claimed receipt and any thread binding.
            SubmissionDecision::KeyReused => return Ok(SubmitOutcome::KeyReused),
            SubmissionDecision::WorkNotPublic => {
                return Ok(if hidden {
                    SubmitOutcome::HiddenReadOnly
                } else {
                    SubmitOutcome::WorkNotFound
                });
            }
            SubmissionDecision::CommentsClosed => return Ok(SubmitOutcome::CommentsClosed),
            SubmissionDecision::TargetUnavailable => return Ok(SubmitOutcome::TargetUnavailable),
            SubmissionDecision::IdentityCollision => {
                return Err(integrity(
                    "generated comment ID collides",
                    comment.comment_id,
                ));
            }
        };

        let submitted = admission.initial_state();
        let root_id = (!placement.is_root()).then(|| placement.root_id().as_comment_id().as_uuid());
        let created_at: OffsetDateTime = sqlx::query_scalar(
            "INSERT INTO comments (id, thread_id, kind, root_id, root_kind, reply_to_id,
                                   replied_to_reply, author_id, body, review, removal,
                                   moderation_version, admission_policy, admission_reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             RETURNING created_at",
        )
        .bind(placement.id().as_uuid())
        .bind(placement.thread_id().as_uuid())
        .bind(if placement.is_root() { "root" } else { "reply" })
        .bind(root_id)
        .bind(root_id.map(|_| "root"))
        .bind(placement.reply_to().map(|id| id.as_uuid()))
        .bind(placement.reply_to().is_some())
        .bind(actor.as_uuid())
        .bind(comment.body.as_str())
        .bind(review_code(submitted.review()))
        .bind(removal_code(submitted.removal()))
        .bind(submitted.version().get())
        .bind(admission.policy().code())
        .bind(admission.reason().code())
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        // DISC-04: the event and its deliveries commit with the comment or not at all.
        outbox::enqueue(
            &mut tx,
            EventKind::CommentSubmitted,
            placement.id(),
            submitted.version(),
        )
        .await?;
        tx.commit().await.map_err(classify)?;
        Ok(SubmitOutcome::Created {
            comment_id: placement.id(),
            created_at,
            state: submitted,
        })
    }

    async fn remove_own_comment(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comment_id: CommentId,
    ) -> Result<Guarded<Transition, LifecycleError>, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let row: Option<(Uuid, String, Option<String>, i64, bool)> = sqlx::query_as(
            "SELECT c.author_id, c.review, c.removal, c.moderation_version,
                    (p.work_id IS NOT NULL OR (wp.lifecycle = 'active' AND
                      (wp.hidden_revision_id IS NOT NULL OR
                       (wp.audience = 'public' AND wp.review_withdrawn_revision_id IS NOT NULL))))
               FROM comments c
               JOIN discussion_threads t ON t.id = c.thread_id
               LEFT JOIN publications p ON p.work_id = t.work_id
               LEFT JOIN work_publications wp ON wp.work_id = t.work_id
              WHERE c.id = $1 AND t.work_id = $2
              FOR UPDATE OF c",
        )
        .bind(comment_id.as_uuid())
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some((author, raw_review, raw_removal, version, work_public)) = row else {
            return Ok(Guarded::NotFound);
        };
        let writer = PrincipalId::from_uuid(author);
        let current = state(&raw_review, raw_removal.as_deref(), version)?;
        let actor_view = if work_public {
            view(writer, current, Some(actor))
        } else {
            CommentView::Unavailable
        };
        if let Err(denied) = authorize_writer_removal(actor, writer, actor_view) {
            return Ok(Guarded::Denied(denied));
        }
        let decided = current.remove_by_author();
        if let Ok(Transition::Changed(next)) = decided {
            require_active(&mut tx, actor).await?;
            // The writer's words are dropped, not just hidden.
            sqlx::query(
                "UPDATE comments SET removal = $2, body = NULL, moderation_version = $3,
                        updated_at = now()
                  WHERE id = $1",
            )
            .bind(comment_id.as_uuid())
            .bind(removal_code(next.removal()))
            .bind(next.version().get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            tx.commit().await.map_err(classify)?;
        }
        Ok(Guarded::Decided(decided))
    }

    async fn set_comments_mode(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        expected: SettingsGeneration,
        mode: CommentsMode,
    ) -> Result<Guarded<ModeChange, SettingsError>, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let owner: Option<Uuid> = sqlx::query_scalar("SELECT owner_id FROM works WHERE id = $1")
            .bind(work_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
        let Some(owner) = owner else {
            return Ok(Guarded::NotFound);
        };
        if let Err(denied) = authorize_mode_change(actor, PrincipalId::from_uuid(owner)) {
            return Ok(Guarded::Denied(denied));
        }
        // The author may set the mode before publishing, so the thread is bound here too.
        sqlx::query(
            "INSERT INTO discussion_threads (id, work_id) VALUES ($1, $2)
             ON CONFLICT (work_id) DO NOTHING",
        )
        .bind(ThreadId::generate().as_uuid())
        .bind(work_id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        let thread: ThreadRow = sqlx::query_as(
            "SELECT id, comments_open, settings_generation
               FROM discussion_threads WHERE work_id = $1 FOR UPDATE",
        )
        .bind(work_id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        let decided = thread.settings()?.change_mode(expected, mode);
        // A reaffirmation is written too: its generation advance fences older requests.
        if let Some(next) = decided.ok().and_then(ModeChange::write) {
            require_active(&mut tx, actor).await?;
            sqlx::query(
                "UPDATE discussion_threads
                    SET comments_open = $2, settings_generation = $3, updated_at = now()
                  WHERE id = $1",
            )
            .bind(thread.id)
            .bind(next.accepts_new_comments())
            .bind(next.generation().get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(Guarded::Decided(decided))
    }

    async fn moderate_comment(
        &self,
        actor: PrincipalId,
        comment_id: CommentId,
        expected: ModerationVersion,
        action: ModerationAction,
        reason: Option<&ModerationReason>,
    ) -> Result<Guarded<Transition, LifecycleError>, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        // Authority at commit: hold Level 1 directory share locks before touching comments
        // (Level 2) or principals (Level 3), matching moderation mutations to stay deadlock-free.
        lock_actor_authority(&mut tx, actor).await?;
        let pre_now: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
        let directory_pre = load_moderation_directory(&mut tx, pre_now).await?;
        if !directory_pre.holds_live_capability(actor, ReviewCapability::Comments, pre_now) {
            return Ok(Guarded::Denied(Denied::NotModerator));
        }
        if directory_pre.facts().suspended.contains(&actor) {
            return Err(StoreError::PrincipalSuspended);
        }

        let row: Option<(Uuid, Uuid, String, Option<String>, i64, Uuid)> = sqlx::query_as(
            "SELECT t.work_id, c.author_id, c.review, c.removal, c.moderation_version, w.owner_id
               FROM comments c
               JOIN discussion_threads t ON t.id = c.thread_id
               JOIN works w ON w.id = t.work_id
              WHERE c.id = $1
              FOR UPDATE OF c",
        )
        .bind(comment_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some((work_id, writer, raw_review, raw_removal, version, work_author)) = row else {
            return Ok(Guarded::NotFound);
        };

        // Standing fence after comment lock: an uncommitted suspension waits here and refuses.
        require_active(&mut tx, actor).await?;
        let commit_now: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
        let directory = load_moderation_directory(&mut tx, commit_now).await?;
        let verified = match directory.resolve(
            actor,
            &ReviewTarget::comment(
                WorkId::from_uuid(work_id),
                PrincipalId::from_uuid(writer),
                PrincipalId::from_uuid(work_author),
            ),
            commit_now,
        ) {
            Ok(verified) => verified,
            Err(AuthorityRefusal::Suspended) => return Err(StoreError::PrincipalSuspended),
            Err(AuthorityRefusal::Recused) => return Ok(Guarded::Denied(Denied::Recused)),
            Err(AuthorityRefusal::NotAuthorized | AuthorityRefusal::NotAdmin) => {
                return Ok(Guarded::Denied(Denied::NotModerator));
            }
        };
        let (authority_kind, grant_id, governance_grant_id) =
            provenance_for_authority(&directory, verified.basis())?;

        let decided = action.apply(
            state(&raw_review, raw_removal.as_deref(), version)?,
            expected,
        );
        if let Ok(Transition::Changed(next)) = decided {
            sqlx::query(
                "UPDATE comments SET review = $2, removal = $3, moderation_version = $4,
                        updated_at = now()
                  WHERE id = $1",
            )
            .bind(comment_id.as_uuid())
            .bind(review_code(next.review()))
            .bind(removal_code(next.removal()))
            .bind(next.version().get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            sqlx::query(
                "INSERT INTO comment_moderation_decisions
                        (comment_id, version, action, moderator_id, authority_kind, grant_id, governance_grant_id, reason)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(comment_id.as_uuid())
            .bind(next.version().get())
            .bind(action_code(action))
            .bind(actor.as_uuid())
            .bind(authority_kind)
            .bind(grant_id)
            .bind(governance_grant_id)
            .bind(reason.map(ModerationReason::as_str))
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            tx.commit().await.map_err(classify)?;
        }
        Ok(Guarded::Decided(decided))
    }

    async fn moderation_queue(
        &self,
        actor: PrincipalId,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> Result<Option<ModerationQueue>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let now: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
        let directory = load_moderation_directory(&mut tx, now).await?;
        if !directory.holds_live_capability(actor, ReviewCapability::Comments, now) {
            return Ok(None);
        }
        if directory.facts().suspended.contains(&actor) {
            return Err(StoreError::PrincipalSuspended);
        }
        let Some((global_scope, work_ids)) =
            directory.capability_scope(actor, ReviewCapability::Comments, now)
        else {
            return Ok(None);
        };
        let work_uuids: Vec<Uuid> = work_ids.iter().map(|id| id.as_uuid()).collect();
        let pending_total: i64 = sqlx::query_scalar(
            "SELECT count(*)
               FROM comments c
               JOIN discussion_threads t ON t.id = c.thread_id
               JOIN works w ON w.id = t.work_id
              WHERE c.review = 'pending'
                AND c.removal IS NULL
                AND c.author_id <> $1
                AND w.owner_id <> $1
                AND ($2::boolean OR t.work_id = ANY($3::uuid[]))",
        )
        .bind(actor.as_uuid())
        .bind(global_scope)
        .bind(&work_uuids)
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        let (after_at, after_id) = page_bounds(after);
        let rows: Vec<QueueRow> = sqlx::query_as(
            "SELECT c.id, t.work_id, rv.title AS work_title, c.kind = 'root' AS is_root,
                    c.author_id, a.display_name, c.body, c.created_at, c.moderation_version,
                    w.owner_id
               FROM comments c
               JOIN discussion_threads t ON t.id = c.thread_id
               JOIN works w ON w.id = t.work_id
               JOIN principals a ON a.id = c.author_id
               LEFT JOIN publications p ON p.work_id = w.id
               LEFT JOIN revisions rv ON rv.id = p.revision_id
              WHERE c.review = 'pending'
                AND c.removal IS NULL
                AND c.author_id <> $1
                AND w.owner_id <> $1
                AND ($2::boolean OR t.work_id = ANY($3::uuid[]))
                AND ($4::timestamptz IS NULL OR (c.created_at, c.id) > ($4, $5))
              ORDER BY c.created_at, c.id
              LIMIT $6",
        )
        .bind(actor.as_uuid())
        .bind(global_scope)
        .bind(&work_uuids)
        .bind(after_at)
        .bind(after_id)
        .bind(i64::from(limit.get()) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let (rows, next) = paged(rows, usize::from(limit.get()), |row| {
            (row.created_at, row.id)
        });
        let items = rows
            .into_iter()
            .map(|row| row.item(actor, &directory, now))
            .collect::<Result<_, _>>()?;
        Ok(Some(ModerationQueue {
            pending_total,
            items,
            next,
        }))
    }

    async fn root_page(
        &self,
        work_id: WorkId,
        viewer: Option<PrincipalId>,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> Result<Option<RootPage>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let Some((owner, thread)) = readable_thread(&mut tx, work_id).await? else {
            return Ok(None);
        };
        let viewer_is_work_author = viewer == Some(owner);
        let Some(thread) = thread else {
            return Ok(Some(RootPage {
                settings: DiscussionSettings::INITIAL,
                viewer_is_work_author,
                roots: Vec::new(),
                next: None,
            }));
        };
        let viewer_id = viewer.map(|viewer| viewer.as_uuid());
        let (after_at, after_id) = page_bounds(after);
        let rows: Vec<ItemRow> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {ITEM_COLUMNS}, NULL::text AS context_review, NULL::text AS context_removal,
                    NULL::bigint AS context_version, NULL::text AS context_author_name, {PUBLIC_REPLIES} AS public_replies
               FROM comments c JOIN principals a ON a.id = c.author_id
              WHERE c.thread_id = $1 AND c.kind = 'root'
                AND ($3::timestamptz IS NULL OR (c.created_at, c.id) > ($3, $4))
                AND (({VISIBLE_TO_VIEWER}) OR {PUBLIC_REPLIES} > 0)
              ORDER BY c.created_at, c.id
              LIMIT $5"
        )))
        .bind(thread.id)
        .bind(viewer_id)
        .bind(after_at)
        .bind(after_id)
        .bind(i64::from(limit.get()) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        let (rows, next) = paged(rows, usize::from(limit.get()), |row| {
            (row.created_at, row.id)
        });

        let root_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let previews: Vec<ItemRow> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {ITEM_COLUMNS}, x.review AS context_review, x.removal AS context_removal,
                    x.moderation_version AS context_version, xa.display_name AS context_author_name, 0::bigint AS public_replies
               FROM unnest($1::uuid[]) AS parent(id)
              CROSS JOIN LATERAL (
                    SELECT c.* FROM comments c
                     WHERE c.root_id = parent.id AND c.kind = 'reply' AND {VISIBLE_TO_VIEWER}
                     ORDER BY c.created_at, c.id
                     LIMIT $3) c
               JOIN principals a ON a.id = c.author_id
               LEFT JOIN comments x ON x.id = c.reply_to_id
               LEFT JOIN principals xa ON xa.id = x.author_id
              ORDER BY c.root_id, c.created_at, c.id"
        )))
        .bind(&root_ids)
        .bind(viewer_id)
        .bind(i64::from(PREVIEW_REPLIES) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;

        let mut roots = Vec::with_capacity(rows.len());
        for row in &rows {
            let entry = match root_listing(row.view(viewer)?, row.public_replies) {
                Some(RootListing::Shown(_)) => {
                    RootEntry::Comment(row.item(viewer, ReplyContext::Root)?)
                }
                Some(RootListing::Anchor) => RootEntry::Anchor {
                    id: CommentId::from_uuid(row.id),
                    created_at: row.created_at,
                },
                None => {
                    return Err(integrity(
                        "root filter disagrees with domain projection",
                        row.id,
                    ));
                }
            };
            let replies: Vec<&ItemRow> = previews
                .iter()
                .filter(|reply| reply.root_id == Some(row.id))
                .collect();
            let (replies, preview_next) = paged(replies, usize::from(PREVIEW_REPLIES), |reply| {
                (reply.created_at, reply.id)
            });
            let preview = replies
                .into_iter()
                .map(|reply| reply.item(viewer, reply.context()?))
                .collect::<Result<_, _>>()?;
            roots.push(RootItem {
                entry,
                public_replies: row.public_replies,
                preview,
                preview_next,
            });
        }
        Ok(Some(RootPage {
            settings: thread.settings()?,
            viewer_is_work_author,
            roots,
            next,
        }))
    }

    async fn reply_page(
        &self,
        work_id: WorkId,
        root_id: CommentId,
        viewer: Option<PrincipalId>,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> Result<Option<ReplyPage>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let Some((_, Some(thread))) = readable_thread(&mut tx, work_id).await? else {
            return Ok(None);
        };
        let viewer_id = viewer.map(|viewer| viewer.as_uuid());
        let root: Option<ItemRow> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {ITEM_COLUMNS}, NULL::text AS context_review, NULL::text AS context_removal,
                    NULL::bigint AS context_version, NULL::text AS context_author_name, {PUBLIC_REPLIES} AS public_replies
               FROM comments c JOIN principals a ON a.id = c.author_id
              WHERE c.id = $1 AND c.thread_id = $2 AND c.kind = 'root'"
        )))
        .bind(root_id.as_uuid())
        .bind(thread.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        // Replies of a root the viewer cannot list (hidden, no public replies) are not readable.
        let Some(root) = root else {
            return Ok(None);
        };
        if root_listing(root.view(viewer)?, root.public_replies).is_none() {
            return Ok(None);
        }
        let (after_at, after_id) = page_bounds(after);
        let rows: Vec<ItemRow> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {ITEM_COLUMNS}, x.review AS context_review, x.removal AS context_removal,
                    x.moderation_version AS context_version, xa.display_name AS context_author_name, 0::bigint AS public_replies
               FROM comments c
               JOIN principals a ON a.id = c.author_id
               LEFT JOIN comments x ON x.id = c.reply_to_id
               LEFT JOIN principals xa ON xa.id = x.author_id
              WHERE c.root_id = $1 AND c.kind = 'reply' AND {VISIBLE_TO_VIEWER}
                AND ($3::timestamptz IS NULL OR (c.created_at, c.id) > ($3, $4))
              ORDER BY c.created_at, c.id
              LIMIT $5"
        )))
        .bind(root_id.as_uuid())
        .bind(viewer_id)
        .bind(after_at)
        .bind(after_id)
        .bind(i64::from(limit.get()) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let (rows, next) = paged(rows, usize::from(limit.get()), |row| {
            (row.created_at, row.id)
        });
        let replies = rows
            .iter()
            .map(|row| row.item(viewer, row.context()?))
            .collect::<Result<_, _>>()?;
        Ok(Some(ReplyPage { replies, next }))
    }

    async fn own_comments(
        &self,
        work_id: WorkId,
        viewer: PrincipalId,
        read: &OwnRead,
    ) -> Result<Option<OwnPage>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let Some((_, thread)) = readable_thread(&mut tx, work_id).await? else {
            return Ok(None);
        };
        let Some(thread) = thread else {
            return Ok(Some(OwnPage {
                items: Vec::new(),
                next: None,
            }));
        };
        // `c.author_id = $2` scopes every row to the viewer; `OwnRow::item` re-checks it.
        let (rows, next, unpublished_only) = match read {
            OwnRead::Unpublished { after, limit } => {
                let (after_at, after_id) = page_bounds(*after);
                let rows: Vec<OwnRow> = sqlx::query_as(AssertSqlSafe(format!(
                    "SELECT {OWN_COLUMNS} FROM {OWN_FROM}
                      WHERE c.thread_id = $1 AND c.author_id = $2 AND {OWN_UNPUBLISHED}
                        AND ($3::timestamptz IS NULL OR (c.created_at, c.id) > ($3, $4))
                      ORDER BY c.created_at, c.id
                      LIMIT $5"
                )))
                .bind(thread.id)
                .bind(viewer.as_uuid())
                .bind(after_at)
                .bind(after_id)
                .bind(i64::from(limit.get()) + 1)
                .fetch_all(&mut *tx)
                .await
                .map_err(classify)?;
                let (rows, next) = paged(rows, usize::from(limit.get()), |row| {
                    (row.created_at, row.id)
                });
                (rows, next, true)
            }
            OwnRead::Exact(ids) => {
                let ids: Vec<Uuid> = ids.iter().map(CommentId::as_uuid).collect();
                let rows: Vec<OwnRow> = sqlx::query_as(AssertSqlSafe(format!(
                    "SELECT {OWN_COLUMNS} FROM {OWN_FROM}
                      WHERE c.thread_id = $1 AND c.author_id = $2 AND c.id = ANY($3::uuid[])
                      ORDER BY c.created_at, c.id"
                )))
                .bind(thread.id)
                .bind(viewer.as_uuid())
                .bind(&ids)
                .fetch_all(&mut *tx)
                .await
                .map_err(classify)?;
                (rows, None, false)
            }
        };
        tx.commit().await.map_err(classify)?;
        let items = rows
            .into_iter()
            .map(|row| row.item(viewer, unpublished_only))
            .collect::<Result<_, _>>()?;
        Ok(Some(OwnPage { items, next }))
    }
}

fn target_facts(thread_id: ThreadId, row: TargetRow) -> Result<TargetFacts, StoreError> {
    let id = CommentId::from_uuid(row.id);
    let position = match row.root_id {
        None => StoredPosition::Root,
        Some(root_id) => StoredPosition::Reply {
            root_id: CommentId::from_uuid(root_id),
            reply_to: row.reply_to_id.map(CommentId::from_uuid),
        },
    };
    let placement = CommentPlacement::from_stored(id, thread_id, position)
        .map_err(|error| integrity("stored placement", error))?;
    Ok(TargetFacts {
        placement,
        state: state(&row.review, row.removal.as_deref(), row.moderation_version)?,
    })
}

/// Result of the operator's grant command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantOutcome {
    Granted(Uuid),
    /// The principal already holds this active grant; nothing was written.
    AlreadyActive(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeratorRecord {
    pub grant_id: Uuid,
    pub principal: PrincipalId,
    pub display_name: String,
    pub note: String,
    pub granted_at: OffsetDateTime,
}

/// Operator command: makes `principal` a comment moderator. Not reachable from HTTP; the binary's
/// `moderator grant` subcommand calls it with the runtime credential.
pub async fn grant_moderator(
    pool: &PgPool,
    principal: PrincipalId,
    note: &GrantNote,
) -> Result<GrantOutcome, StoreError> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO discussion_moderator_grants (id, principal_id, note) VALUES ($1, $2, $3)
         ON CONFLICT (principal_id) WHERE revoked_at IS NULL DO NOTHING
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(principal.as_uuid())
    .bind(note.as_str())
    .fetch_optional(pool)
    .await
    .map_err(classify)?;
    if let Some(id) = inserted {
        return Ok(GrantOutcome::Granted(id));
    }
    let active: Uuid = sqlx::query_scalar(
        "SELECT id FROM discussion_moderator_grants WHERE principal_id = $1 AND revoked_at IS NULL",
    )
    .bind(principal.as_uuid())
    .fetch_one(pool)
    .await
    .map_err(classify)?;
    Ok(GrantOutcome::AlreadyActive(active))
}

/// Operator command: ends `principal`'s active grant. Decisions already committed keep naming it;
/// a decision still in flight either commits first or sees the revocation. `None`: nothing active.
pub async fn revoke_moderator(
    pool: &PgPool,
    principal: PrincipalId,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "UPDATE discussion_moderator_grants SET revoked_at = now()
          WHERE principal_id = $1 AND revoked_at IS NULL
         RETURNING id",
    )
    .bind(principal.as_uuid())
    .fetch_optional(pool)
    .await
    .map_err(classify)
}

pub async fn active_moderators(pool: &PgPool) -> Result<Vec<ModeratorRecord>, StoreError> {
    let rows: Vec<(Uuid, Uuid, String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT g.id, g.principal_id, p.display_name, g.note, g.granted_at
           FROM discussion_moderator_grants g JOIN principals p ON p.id = g.principal_id
          WHERE g.revoked_at IS NULL
          ORDER BY g.granted_at, g.id",
    )
    .fetch_all(pool)
    .await
    .map_err(classify)?;
    Ok(rows
        .into_iter()
        .map(
            |(grant_id, principal, display_name, note, granted_at)| ModeratorRecord {
                grant_id,
                principal: PrincipalId::from_uuid(principal),
                display_name,
                note,
                granted_at,
            },
        )
        .collect())
}
