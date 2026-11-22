//! The Notification database transactions. Every write to an inbox first locks that inbox's row
//! (T-03/T-05), then applies [`crate::rollup`]'s answer and commits it together with the event's
//! receipt (contract §1.3) or the command's stored answer (§2.2).
//!
//! **Ingest.** Parse and verify the digest before reading anything (`rejected_invalid` needs no
//! database). In one transaction: insert the receipt (`ON CONFLICT DO NOTHING`; a concurrent
//! duplicate waits on the first one's key), lock the recipients' inboxes in ascending order, apply
//! the effect, record which effect it was, commit. An existing receipt with the same digest is
//! `already_applied`; with another digest `payload_mismatch`; nothing is written either way. Any
//! database failure rolls the whole event back and answers `transient_unavailable`.
//!
//! **Clock.** Every function takes `now`; the server passes the wall clock, tests a fixed one.
//! Expired groups are invisible and never joined even before the purge deletes them.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{AssertSqlSafe, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::envelope::{self, Event, EventType, SemanticKind, Target};
use crate::rollup::{
    self, COMMAND_HORIZON, Counting, GroupKey, MARK_HORIZON, Mark, Member, RECEIPT_HORIZON,
    ReadRefusal, Snapshot, ThamStep,
};

/// What the service answers for one event (contract §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestStatus {
    Applied,
    AlreadyApplied,
    PayloadMismatch,
    RejectedInvalid,
    TransientUnavailable,
}

impl IngestStatus {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::AlreadyApplied => "already_applied",
            Self::PayloadMismatch => "payload_mismatch",
            Self::RejectedInvalid => "rejected_invalid",
            Self::TransientUnavailable => "transient_unavailable",
        }
    }
}

/// The stored effect of an applied event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Effect {
    Noted,
    Stale,
    Late,
    Reconcile,
    Signal,
}

impl Effect {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Noted => "noted",
            Self::Stale => "stale",
            Self::Late => "late",
            Self::Reconcile => "reconcile",
            Self::Signal => "signal",
        }
    }
}

/// One answered event, with its stored effect when it was applied now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ingested {
    pub status: IngestStatus,
    pub effect: Option<Effect>,
}

/// Parses, verifies and applies one envelope sent for `event_id`.
pub async fn ingest(pool: &PgPool, event_id: Uuid, text: &str, now: OffsetDateTime) -> Ingested {
    let event = match envelope::parse(text, event_id) {
        Ok(event) => event,
        Err(invalid) => {
            tracing::info!(reason = invalid.code(), "envelope refused");
            return Ingested {
                status: IngestStatus::RejectedInvalid,
                effect: None,
            };
        }
    };
    match apply(pool, &event, now).await {
        Ok(ingested) => ingested,
        Err(error) => {
            tracing::warn!(
                error = error_class(&error),
                "event not applied; the relay will retry"
            );
            Ingested {
                status: IngestStatus::TransientUnavailable,
                effect: None,
            }
        }
    }
}

/// A class for logs that never carries values (constraint details may contain IDs).
pub fn error_class(error: &sqlx::Error) -> &'static str {
    match error {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("40001") => "serialization_failure",
            Some("40P01") => "deadlock",
            Some("23505") => "unique_violation",
            Some("23514") => "check_violation",
            Some("57014") => "query_canceled",
            Some(_) => "database",
            None => "database",
        },
        sqlx::Error::PoolTimedOut => "pool_timeout",
        sqlx::Error::Io(_) | sqlx::Error::Tls(_) | sqlx::Error::PoolClosed => "connection",
        _ => "other",
    }
}

async fn apply(pool: &PgPool, event: &Event, now: OffsetDateTime) -> Result<Ingested, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO ingestion_receipts
             (event_id, payload_digest, event_type, effect, occurred_at, received_at, expires_at)
         VALUES ($1, $2, $3, 'noted', $4, $5, $6)
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(event.event_id)
    .bind(&event.payload_digest)
    .bind(event.event_type.code())
    .bind(event.occurred_at)
    .bind(now)
    .bind(now + RECEIPT_HORIZON)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        let stored: String =
            sqlx::query_scalar("SELECT payload_digest FROM ingestion_receipts WHERE event_id = $1")
                .bind(event.event_id)
                .fetch_one(&mut *tx)
                .await?;
        tx.rollback().await?;
        let status = if stored == event.payload_digest {
            IngestStatus::AlreadyApplied
        } else {
            tracing::warn!("same event ID with another payload; quarantined by the relay");
            IngestStatus::PayloadMismatch
        };
        return Ok(Ingested {
            status,
            effect: None,
        });
    }
    let effect = match event.event_type {
        EventType::CommentRootApproved | EventType::CommentReplyApproved => {
            approval(&mut tx, event, now).await?
        }
        EventType::WorkThamChanged => tham(&mut tx, event, now).await?,
        EventType::WorkModerationDecided | EventType::CommentModerationAdverse => {
            decision(&mut tx, event, now).await?
        }
        EventType::Invalidation => invalidation(&mut tx, event, now).await?,
        // Refused by the parser; never stored.
        EventType::AdminNoticeReserved => Effect::Noted,
    };
    sqlx::query("UPDATE ingestion_receipts SET effect = $2 WHERE event_id = $1")
        .bind(event.event_id)
        .bind(effect.code())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Ingested {
        status: IngestStatus::Applied,
        effect: Some(effect),
    })
}

type Tx<'a> = Transaction<'a, Postgres>;

/// Locks the inboxes of `recipients` (creating missing ones) in ascending ID order, so any two
/// transactions take shared recipients in the same order.
async fn lock_inboxes(
    tx: &mut Tx<'_>,
    recipients: &BTreeSet<Uuid>,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    if recipients.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = recipients.iter().copied().collect();
    sqlx::query(
        "INSERT INTO inboxes (recipient, created_at, updated_at)
         SELECT id, $2, $2 FROM unnest($1::uuid[]) AS id
         ON CONFLICT (recipient) DO NOTHING",
    )
    .bind(&ids)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "SELECT recipient FROM inboxes WHERE recipient = ANY($1) ORDER BY recipient FOR UPDATE",
    )
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?;
    Ok(())
}

/// Takes the next value of the recipient's sequence. The caller holds the inbox lock.
async fn next_seq(
    tx: &mut Tx<'_>,
    recipient: Uuid,
    now: OffsetDateTime,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE inboxes SET last_seq = last_seq + 1, updated_at = $2
         WHERE recipient = $1 RETURNING last_seq",
    )
    .bind(recipient)
    .bind(now)
    .fetch_one(&mut **tx)
    .await
}

#[derive(Debug, Clone, Copy)]
struct OpenGroup {
    group_id: Uuid,
    target_order: i64,
}

/// The open group of a key, if it is still within its age. An expired open group is closed here
/// (no revival); the caller then starts a new one.
async fn open_group(
    tx: &mut Tx<'_>,
    recipient: Uuid,
    key: &GroupKey,
    now: OffsetDateTime,
) -> Result<Option<OpenGroup>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT group_id, target_order, expires_at FROM notification_groups
         WHERE recipient = $1 AND event_type = $2 AND target_key = $3 AND open
         FOR UPDATE",
    )
    .bind(recipient)
    .bind(key.event_type.code())
    .bind(&key.target_key)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let group_id: Uuid = row.try_get("group_id")?;
    let expires_at: OffsetDateTime = row.try_get("expires_at")?;
    if expires_at <= now {
        sqlx::query("UPDATE notification_groups SET open = false WHERE group_id = $1")
            .bind(group_id)
            .execute(&mut **tx)
            .await?;
        return Ok(None);
    }
    Ok(Some(OpenGroup {
        group_id,
        target_order: row.try_get("target_order")?,
    }))
}

/// Starts a new open group with one signal at `seq`: one item, or `actor` as its first member
/// (the caller then writes the membership row and refreshes the preview).
async fn create_group(
    tx: &mut Tx<'_>,
    recipient: Uuid,
    key: &GroupKey,
    event: &Event,
    seq: i64,
    now: OffsetDateTime,
) -> Result<Uuid, sqlx::Error> {
    // A replay after a restore never makes a group outlive 180 days from the original event.
    let created = rollup::signal_time(event.occurred_at, now);
    let (actors, items, preview) = match (rollup::counting(key.event_type), event.actor) {
        (Counting::Actors, Some(actor)) => (1, 0, vec![actor]),
        _ => (0, 1, Vec::new()),
    };
    let group_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO notification_groups
             (group_id, recipient, event_type, target_key, target, target_order, work_id,
              publication_epoch, action_required, open, created_at, expires_at, last_signal_at,
              group_version, signal_seq, first_seq, read_through, actor_count, item_count,
              actors_preview)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, $11, $10, $12, $12, $12, 0,
                 $13, $14, $15)",
    )
    .bind(group_id)
    .bind(recipient)
    .bind(key.event_type.code())
    .bind(&key.target_key)
    .bind(&event.target_json)
    .bind(event.source_order)
    .bind(rollup::work_of(event.target))
    .bind(event.stamp.map(|stamp| stamp.epoch))
    .bind(event.action_required)
    .bind(created)
    .bind(rollup::expiry(created))
    .bind(seq)
    .bind(actors)
    .bind(items)
    .bind(&preview)
    .execute(&mut **tx)
    .await?;
    Ok(group_id)
}

/// Records a new signal on an existing open group: newer target, stamp and versions.
async fn signal_group(
    tx: &mut Tx<'_>,
    group: OpenGroup,
    event: &Event,
    seq: i64,
    items: i32,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    let newer = event.source_order >= group.target_order;
    sqlx::query(
        "UPDATE notification_groups SET
             target = CASE WHEN $2 THEN $3 ELSE target END,
             target_order = GREATEST(target_order, $4),
             publication_epoch = CASE WHEN $2 THEN $5 ELSE publication_epoch END,
             action_required = CASE WHEN $2 THEN $6 ELSE action_required END,
             item_count = item_count + $7,
             group_version = $8, signal_seq = $8, last_signal_at = GREATEST(last_signal_at, $9)
         WHERE group_id = $1",
    )
    .bind(group.group_id)
    .bind(newer)
    .bind(&event.target_json)
    .bind(event.source_order)
    .bind(event.stamp.map(|stamp| stamp.epoch))
    .bind(event.action_required)
    .bind(items)
    .bind(seq)
    .bind(rollup::signal_time(event.occurred_at, now))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Recomputes an actor-counted group's count and preview from its members at version `seq`.
/// A group left without any active contribution closes (P-03A): hidden, never read.
async fn refresh_members(tx: &mut Tx<'_>, group_id: Uuid, seq: i64) -> Result<i32, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT actor_id, contribution, active, last_seq FROM group_members WHERE group_id = $1",
    )
    .bind(group_id)
    .fetch_all(&mut **tx)
    .await?;
    let members = rows
        .iter()
        .map(|row| {
            Ok(Member {
                actor: row.try_get("actor_id")?,
                contribution: row.try_get("contribution")?,
                active: row.try_get("active")?,
                last_seq: row.try_get("last_seq")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    let (count, preview) = rollup::preview(&members);
    sqlx::query(
        "UPDATE notification_groups SET actor_count = $2, actors_preview = $3,
             group_version = $4, open = open AND $2 > 0
         WHERE group_id = $1",
    )
    .bind(group_id)
    .bind(count)
    .bind(&preview)
    .bind(seq)
    .execute(&mut **tx)
    .await?;
    Ok(count)
}

async fn upsert_member(
    tx: &mut Tx<'_>,
    group_id: Uuid,
    actor: Uuid,
    contribution: Uuid,
    source_order: i64,
    seq: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO group_members (group_id, contribution, actor_id, active, source_order, last_seq)
         VALUES ($1, $2, $3, true, $4, $5)
         ON CONFLICT (group_id, contribution, actor_id)
         DO UPDATE SET active = true, source_order = EXCLUDED.source_order, last_seq = EXCLUDED.last_seq",
    )
    .bind(group_id)
    .bind(contribution)
    .bind(actor)
    .bind(source_order)
    .bind(seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Root and reply approvals (actor-counted, contribution = the comment).
async fn approval(
    tx: &mut Tx<'_>,
    event: &Event,
    now: OffsetDateTime,
) -> Result<Effect, sqlx::Error> {
    let (Some(key), Some((actor, comment))) =
        (rollup::group_key(event), rollup::contribution(event))
    else {
        return Ok(Effect::Noted);
    };
    let fence: Option<i64> =
        sqlx::query_scalar("SELECT source_order FROM comment_fences WHERE comment_id = $1")
            .bind(comment)
            .fetch_optional(&mut **tx)
            .await?;
    if fence.is_some_and(|fence| fence >= event.source_order) {
        return Ok(Effect::Stale);
    }
    if rollup::is_late(event.occurred_at, now) {
        return Ok(Effect::Late);
    }
    if !event.stamp.is_some_and(|stamp| stamp.readable) {
        return Ok(Effect::Noted);
    }
    let recipients: BTreeSet<Uuid> = event.recipients.iter().copied().collect();
    lock_inboxes(tx, &recipients, now).await?;
    let mut effect = Effect::Stale;
    for recipient in recipients {
        let group = open_group(tx, recipient, &key, now).await?;
        if let Some(group) = group {
            let known: Option<i64> = sqlx::query_scalar(
                "SELECT source_order FROM group_members
                 WHERE group_id = $1 AND contribution = $2 AND actor_id = $3",
            )
            .bind(group.group_id)
            .bind(comment)
            .bind(actor)
            .fetch_optional(&mut **tx)
            .await?;
            if known.is_some_and(|known| known >= event.source_order) {
                continue;
            }
        }
        let seq = next_seq(tx, recipient, now).await?;
        let group_id = match group {
            Some(group) => {
                signal_group(tx, group, event, seq, 0, now).await?;
                group.group_id
            }
            None => create_group(tx, recipient, &key, event, seq, now).await?,
        };
        upsert_member(tx, group_id, actor, comment, event.source_order, seq).await?;
        refresh_members(tx, group_id, seq).await?;
        effect = Effect::Signal;
    }
    Ok(effect)
}

/// Thấm on a work revision: P-02B suppression and P-03A reconciliation (T-04 A).
async fn tham(tx: &mut Tx<'_>, event: &Event, now: OffsetDateTime) -> Result<Effect, sqlx::Error> {
    let (Some(key), Some((actor, revision)), Target::WorkEndorsement { work, .. }) = (
        rollup::group_key(event),
        rollup::contribution(event),
        event.target,
    ) else {
        return Ok(Effect::Noted);
    };
    let late = rollup::is_late(event.occurred_at, now);
    let eligible = !late && event.stamp.is_some_and(|stamp| stamp.readable);
    let selected = event.semantic == SemanticKind::Add;
    let recipients: BTreeSet<Uuid> = event.recipients.iter().copied().collect();
    lock_inboxes(tx, &recipients, now).await?;
    let mut effect = Effect::Stale;
    for recipient in recipients {
        let row = sqlx::query(
            "SELECT selected, source_order, signaled FROM tham_marks
             WHERE recipient = $1 AND work_id = $2 AND revision_id = $3 AND actor_id = $4
             FOR UPDATE",
        )
        .bind(recipient)
        .bind(work)
        .bind(revision)
        .bind(actor)
        .fetch_optional(&mut **tx)
        .await?;
        let mark = row
            .map(|row| {
                Ok::<_, sqlx::Error>(Mark {
                    selected: row.try_get("selected")?,
                    source_order: row.try_get("source_order")?,
                    signaled: row.try_get("signaled")?,
                })
            })
            .transpose()?;
        let step = rollup::tham_step(
            mark,
            event.semantic,
            event.source_order,
            event.first_selection.unwrap_or(false),
            eligible,
        );
        if step == ThamStep::Stale {
            continue;
        }
        let next = rollup::next_mark(mark, step, event.source_order, selected);
        sqlx::query(
            "INSERT INTO tham_marks
                 (recipient, work_id, revision_id, actor_id, selected, source_order, signaled,
                  updated_at, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (recipient, work_id, revision_id, actor_id) DO UPDATE SET
                 selected = EXCLUDED.selected, source_order = EXCLUDED.source_order,
                 signaled = EXCLUDED.signaled, updated_at = EXCLUDED.updated_at,
                 expires_at = EXCLUDED.expires_at",
        )
        .bind(recipient)
        .bind(work)
        .bind(revision)
        .bind(actor)
        .bind(next.selected)
        .bind(next.source_order)
        .bind(next.signaled)
        .bind(now)
        .bind(now + MARK_HORIZON)
        .execute(&mut **tx)
        .await?;
        let group = open_group(tx, recipient, &key, now).await?;
        match step {
            ThamStep::Signal => {
                let seq = next_seq(tx, recipient, now).await?;
                let group_id = match group {
                    Some(group) => {
                        signal_group(tx, group, event, seq, 0, now).await?;
                        group.group_id
                    }
                    None => create_group(tx, recipient, &key, event, seq, now).await?,
                };
                upsert_member(tx, group_id, actor, revision, event.source_order, seq).await?;
                refresh_members(tx, group_id, seq).await?;
                effect = Effect::Signal;
            }
            ThamStep::Reconcile { active } => {
                effect = effect.max(if late {
                    Effect::Late
                } else {
                    Effect::Reconcile
                });
                let Some(group) = group else { continue };
                let changed = sqlx::query(
                    "UPDATE group_members SET active = $4, source_order = $5
                     WHERE group_id = $1 AND contribution = $2 AND actor_id = $3 AND active <> $4",
                )
                .bind(group.group_id)
                .bind(revision)
                .bind(actor)
                .bind(active)
                .bind(event.source_order)
                .execute(&mut **tx)
                .await?
                .rows_affected();
                if changed > 0 {
                    let seq = next_seq(tx, recipient, now).await?;
                    refresh_members(tx, group.group_id, seq).await?;
                }
            }
            ThamStep::Stale => {}
        }
    }
    Ok(effect)
}

/// Work decisions and adverse comment decisions (item-counted, no actor).
async fn decision(
    tx: &mut Tx<'_>,
    event: &Event,
    now: OffsetDateTime,
) -> Result<Effect, sqlx::Error> {
    let Some(key) = rollup::group_key(event) else {
        return Ok(Effect::Noted);
    };
    if rollup::is_late(event.occurred_at, now) {
        return Ok(Effect::Late);
    }
    let recipients: BTreeSet<Uuid> = event.recipients.iter().copied().collect();
    lock_inboxes(tx, &recipients, now).await?;
    for recipient in recipients {
        let group = open_group(tx, recipient, &key, now).await?;
        let seq = next_seq(tx, recipient, now).await?;
        match group {
            Some(group) => signal_group(tx, group, event, seq, 1, now).await?,
            None => {
                create_group(tx, recipient, &key, event, seq, now).await?;
            }
        }
    }
    Ok(Effect::Signal)
}

/// Invalidation by target (§4.2). A comment's contributions leave every live group they are in
/// (unread or read: history keeps only what is still valid) and a fence stops an older approval
/// from restoring them. Work and principal invalidations store nothing: the Gateway checks
/// current visibility and standing at every read (N4).
async fn invalidation(
    tx: &mut Tx<'_>,
    event: &Event,
    now: OffsetDateTime,
) -> Result<Effect, sqlx::Error> {
    let Target::InvalidComment { comment, .. } = event.target else {
        return Ok(Effect::Noted);
    };
    sqlx::query(
        "INSERT INTO comment_fences (comment_id, source_order, expires_at) VALUES ($1, $2, $3)
         ON CONFLICT (comment_id) DO UPDATE SET
             source_order = GREATEST(comment_fences.source_order, EXCLUDED.source_order),
             expires_at = GREATEST(comment_fences.expires_at, EXCLUDED.expires_at)",
    )
    .bind(comment)
    .bind(event.source_order)
    .bind(now + RECEIPT_HORIZON)
    .execute(&mut **tx)
    .await?;
    let affected = sqlx::query(
        "SELECT DISTINCT g.recipient, g.group_id FROM group_members m
         JOIN notification_groups g ON g.group_id = m.group_id
         WHERE m.contribution = $1 AND m.active AND m.source_order < $2 AND g.expires_at > $3
           AND g.event_type IN ('comment.root.approved', 'comment.reply.approved')",
    )
    .bind(comment)
    .bind(event.source_order)
    .bind(now)
    .fetch_all(&mut **tx)
    .await?;
    if affected.is_empty() {
        return Ok(Effect::Noted);
    }
    let mut groups: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for row in &affected {
        groups
            .entry(row.try_get("recipient")?)
            .or_default()
            .push(row.try_get("group_id")?);
    }
    let recipients: BTreeSet<Uuid> = groups.keys().copied().collect();
    lock_inboxes(tx, &recipients, now).await?;
    let mut effect = Effect::Noted;
    for (recipient, group_ids) in groups {
        for group_id in group_ids {
            let changed = sqlx::query(
                "UPDATE group_members SET active = false
                 WHERE group_id = $1 AND contribution = $2 AND active AND source_order < $3",
            )
            .bind(group_id)
            .bind(comment)
            .bind(event.source_order)
            .execute(&mut **tx)
            .await?
            .rows_affected();
            if changed > 0 {
                let seq = next_seq(tx, recipient, now).await?;
                refresh_members(tx, group_id, seq).await?;
                effect = Effect::Reconcile;
            }
        }
    }
    Ok(effect)
}

// ---------------------------------------------------------------------------------------------
// Gateway reads and commands (contract §2.2–2.3).

/// Why a Gateway call did not succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxError {
    /// The snapshot token is malformed or ahead of what this inbox committed.
    InvalidSnapshot,
    /// The cursor is malformed.
    InvalidCursor,
    /// The group does not exist for this recipient (or expired).
    NotFound,
    /// The client claims a group version the group never had.
    InvalidVersion,
    /// Same command key with another command or parameters.
    IdempotencyKeyReused,
    Unavailable,
}

impl From<sqlx::Error> for InboxError {
    fn from(error: sqlx::Error) -> Self {
        tracing::warn!(error = error_class(&error), "inbox call failed");
        Self::Unavailable
    }
}

/// The badge count (P-09A): unread, visible groups whose newest signal is not yet Seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Count {
    pub unseen_group_count: i64,
    pub as_of: Snapshot,
}

pub async fn count(
    pool: &PgPool,
    recipient: Uuid,
    now: OffsetDateTime,
) -> Result<Count, InboxError> {
    let row = sqlx::query(
        "SELECT i.last_seq,
                (SELECT count(*) FROM notification_groups g
                  WHERE g.recipient = i.recipient AND g.expires_at > $2
                    AND g.signal_seq > g.read_through AND g.signal_seq > i.seen_seq
                    AND g.actor_count + g.item_count > 0) AS unseen
         FROM inboxes i WHERE i.recipient = $1",
    )
    .bind(recipient)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some(row) => Count {
            unseen_group_count: row.try_get("unseen")?,
            as_of: Snapshot(row.try_get("last_seq")?),
        },
        None => Count {
            unseen_group_count: 0,
            as_of: Snapshot(0),
        },
    })
}

/// One inbox row as the Gateway receives it (identifiers and versions only).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupView {
    pub group_id: Uuid,
    pub event_type: String,
    pub target: Value,
    pub group_version: i64,
    pub signal_version: i64,
    pub unread: bool,
    pub unseen: bool,
    pub actor_count: i32,
    pub item_count: i32,
    pub actors_preview: Vec<Uuid>,
    /// Comment groups only (#348): the newest active contributions, newest first, at most
    /// [`MAX_CONTRIBUTIONS`], so the Gateway can fall back to an older valid comment when the
    /// newest one lost visibility before its invalidation arrived. `None` for other types.
    pub contributions: Option<Contributions>,
    pub action_required: bool,
    pub publication_epoch: Option<i64>,
    pub created_at: OffsetDateTime,
    pub last_signal_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

impl GroupView {
    pub fn json(&self) -> Value {
        let at = |t: OffsetDateTime| {
            t.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        };
        json!({
            "group_id": self.group_id,
            "event_type": self.event_type,
            "target": self.target,
            "group_version": self.group_version,
            "signal_version": self.signal_version,
            "unread": self.unread,
            "unseen": self.unseen,
            "actor_count": self.actor_count,
            "item_count": self.item_count,
            "actors_preview": self.actors_preview,
            "contributions": self.contributions.as_ref().map(Contributions::json),
            "action_required": self.action_required,
            "publication_epoch": self.publication_epoch,
            "created_at": at(self.created_at),
            "last_signal_at": at(self.last_signal_at),
            "expires_at": at(self.expires_at),
        })
    }
}

/// Active contributions of a comment group (newest first) and how many are active in total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contributions {
    /// `(comment_id, actor_id)`, newest signal first.
    pub newest: Vec<(Uuid, Uuid)>,
    pub total: i64,
}

impl Contributions {
    fn json(&self) -> Value {
        json!({
            "newest": self
                .newest
                .iter()
                .map(|(comment, actor)| json!({ "comment_id": comment, "actor_id": actor }))
                .collect::<Vec<_>>(),
            "total": self.total,
        })
    }
}

/// Contributions returned per comment group; the Gateway's fallback depth.
pub const MAX_CONTRIBUTIONS: i64 = 16;

/// A page of the inbox read in one snapshot: the snapshot token, the Seen watermark and the
/// groups newest signal first.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub snapshot: Snapshot,
    pub seen: Snapshot,
    pub groups: Vec<GroupView>,
    pub next_cursor: Option<String>,
}

pub const MAX_PAGE: i64 = 50;

/// Keyset position after a group: its newest signal and ID.
fn cursor_token(signal_seq: i64, group_id: Uuid) -> String {
    format!("c{signal_seq}.{group_id}")
}

fn parse_cursor(token: &str) -> Option<(i64, Uuid)> {
    let (seq, id) = token.strip_prefix('c')?.split_once('.')?;
    let seq = Snapshot::parse(&format!("s{seq}"))?.0;
    let parsed = Uuid::try_parse(id).ok()?;
    (parsed.hyphenated().to_string() == id).then_some((seq, parsed))
}

pub async fn page(
    pool: &PgPool,
    recipient: Uuid,
    limit: i64,
    cursor: Option<&str>,
    now: OffsetDateTime,
) -> Result<Page, InboxError> {
    let after = match cursor {
        Some(token) => Some(parse_cursor(token).ok_or(InboxError::InvalidCursor)?),
        None => None,
    };
    let limit = limit.clamp(1, MAX_PAGE);
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let inbox = sqlx::query("SELECT last_seq, seen_seq FROM inboxes WHERE recipient = $1")
        .bind(recipient)
        .fetch_optional(&mut *tx)
        .await?;
    let (last_seq, seen_seq): (i64, i64) = match inbox {
        Some(row) => (row.try_get("last_seq")?, row.try_get("seen_seq")?),
        None => (0, 0),
    };
    let (after_seq, after_id) = after.unwrap_or((i64::MAX, Uuid::max()));
    let rows = sqlx::query(
        "SELECT group_id, event_type, target, group_version, signal_seq, read_through,
                actor_count, item_count, actors_preview, action_required, publication_epoch,
                created_at, last_signal_at, expires_at
         FROM notification_groups
         WHERE recipient = $1 AND expires_at > $2 AND actor_count + item_count > 0
           AND (signal_seq, group_id) < ($3, $4)
         ORDER BY signal_seq DESC, group_id DESC
         LIMIT $5",
    )
    .bind(recipient)
    .bind(now)
    .bind(after_seq)
    .bind(after_id)
    .bind(limit + 1)
    .fetch_all(&mut *tx)
    .await?;
    let mut groups = rows
        .iter()
        .map(|row| view(row, seen_seq))
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    attach_contributions(&mut tx, &mut groups).await?;
    tx.commit().await?;
    let next_cursor = if groups.len() > usize::try_from(limit).unwrap_or(0) {
        groups.pop();
        groups
            .last()
            .map(|last| cursor_token(last.signal_version, last.group_id))
    } else {
        None
    };
    Ok(Page {
        snapshot: Snapshot(last_seq),
        seen: Snapshot(seen_seq),
        groups,
        next_cursor,
    })
}

/// One bounded query for every comment group of the page, in the page's snapshot.
async fn attach_contributions(
    tx: &mut Tx<'_>,
    groups: &mut [GroupView],
) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = groups
        .iter()
        .filter(|group| is_comment_group(&group.event_type))
        .map(|group| group.group_id)
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query(
        "SELECT group_id, contribution, actor_id, total FROM (
             SELECT group_id, contribution, actor_id,
                    row_number() OVER (PARTITION BY group_id
                                       ORDER BY last_seq DESC, contribution DESC) AS rank,
                    count(*) OVER (PARTITION BY group_id) AS total
             FROM group_members WHERE group_id = ANY($1) AND active) ranked
         WHERE rank <= $2
         ORDER BY group_id, rank",
    )
    .bind(&ids)
    .bind(MAX_CONTRIBUTIONS)
    .fetch_all(&mut **tx)
    .await?;
    let mut found: BTreeMap<Uuid, Contributions> = BTreeMap::new();
    for row in &rows {
        let entry = found
            .entry(row.try_get("group_id")?)
            .or_insert_with(|| Contributions {
                newest: Vec::new(),
                total: 0,
            });
        entry.total = row.try_get("total")?;
        entry
            .newest
            .push((row.try_get("contribution")?, row.try_get("actor_id")?));
    }
    for group in groups
        .iter_mut()
        .filter(|group| is_comment_group(&group.event_type))
    {
        group.contributions = Some(found.remove(&group.group_id).unwrap_or(Contributions {
            newest: Vec::new(),
            total: 0,
        }));
    }
    Ok(())
}

fn is_comment_group(event_type: &str) -> bool {
    matches!(
        event_type,
        "comment.root.approved" | "comment.reply.approved"
    )
}

fn view(row: &PgRow, seen_seq: i64) -> Result<GroupView, sqlx::Error> {
    let signal_seq: i64 = row.try_get("signal_seq")?;
    let read_through: i64 = row.try_get("read_through")?;
    let unread = signal_seq > read_through;
    Ok(GroupView {
        group_id: row.try_get("group_id")?,
        event_type: row.try_get("event_type")?,
        target: row.try_get("target")?,
        group_version: row.try_get("group_version")?,
        signal_version: signal_seq,
        unread,
        unseen: unread && signal_seq > seen_seq,
        actor_count: row.try_get("actor_count")?,
        item_count: row.try_get("item_count")?,
        actors_preview: row.try_get("actors_preview")?,
        contributions: None,
        action_required: row.try_get("action_required")?,
        publication_epoch: row.try_get("publication_epoch")?,
        created_at: row.try_get("created_at")?,
        last_signal_at: row.try_get("last_signal_at")?,
        expires_at: row.try_get("expires_at")?,
    })
}

/// The three idempotent ACK commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Seen {
        up_to: String,
    },
    Read {
        group_id: Uuid,
        observed_version: i64,
    },
    MarkAllRead {
        up_to: String,
    },
}

impl Command {
    /// The canonical request text compared for idempotency.
    fn request(&self) -> String {
        match self {
            Self::Seen { up_to } => format!("seen:{up_to}"),
            Self::Read {
                group_id,
                observed_version,
            } => format!("read:{group_id}:{observed_version}"),
            Self::MarkAllRead { up_to } => format!("mark_all_read:{up_to}"),
        }
    }
}

/// Runs one command for `recipient` under its inbox lock and stores the answer with its key.
/// The same key and request returns the stored answer; another request is refused.
pub async fn command(
    pool: &PgPool,
    recipient: Uuid,
    command_key: &str,
    command: &Command,
    now: OffsetDateTime,
) -> Result<Value, InboxError> {
    let request = command.request();
    let mut tx = pool.begin().await?;
    lock_inboxes(&mut tx, &BTreeSet::from([recipient]), now).await?;
    let stored = sqlx::query(
        "SELECT request, response FROM inbox_commands
         WHERE recipient = $1 AND command_key = $2 AND expires_at > $3",
    )
    .bind(recipient)
    .bind(command_key)
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = stored {
        let previous: String = row.try_get("request")?;
        let response: Value = row.try_get("response")?;
        tx.rollback().await?;
        return if previous == request {
            Ok(response)
        } else {
            Err(InboxError::IdempotencyKeyReused)
        };
    }
    let (last_seq, seen_seq): (i64, i64) = {
        let row = sqlx::query("SELECT last_seq, seen_seq FROM inboxes WHERE recipient = $1")
            .bind(recipient)
            .fetch_one(&mut *tx)
            .await?;
        (row.try_get("last_seq")?, row.try_get("seen_seq")?)
    };
    let snapshot = |token: &str| {
        Snapshot::parse(token)
            .filter(|snapshot| snapshot.within(last_seq))
            .ok_or(InboxError::InvalidSnapshot)
    };
    let response = match command {
        Command::Seen { up_to } => {
            let up_to = snapshot(up_to)?;
            let seen = seen_seq.max(up_to.0);
            sqlx::query("UPDATE inboxes SET seen_seq = $2, updated_at = $3 WHERE recipient = $1")
                .bind(recipient)
                .bind(seen)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            json!({ "seen_snapshot": Snapshot(seen).token() })
        }
        Command::Read {
            group_id,
            observed_version,
        } => {
            let row = sqlx::query(
                "SELECT group_version, signal_seq, read_through FROM notification_groups
                 WHERE group_id = $1 AND recipient = $2 AND expires_at > $3
                 FOR UPDATE",
            )
            .bind(group_id)
            .bind(recipient)
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(InboxError::NotFound)?;
            let group_version: i64 = row.try_get("group_version")?;
            let signal_seq: i64 = row.try_get("signal_seq")?;
            let current: i64 = row.try_get("read_through")?;
            let through = rollup::read_through(current, group_version, *observed_version)
                .map_err(|ReadRefusal::FutureVersion| InboxError::InvalidVersion)?;
            let mut version = group_version;
            if through > current {
                version = next_seq(&mut tx, recipient, now).await?;
                sqlx::query(
                    "UPDATE notification_groups SET read_through = $2, group_version = $3,
                         open = open AND signal_seq > $2
                     WHERE group_id = $1",
                )
                .bind(group_id)
                .bind(through)
                .bind(version)
                .execute(&mut *tx)
                .await?;
            }
            json!({
                "group_id": group_id,
                "group_version": version,
                "unread": signal_seq > through,
            })
        }
        Command::MarkAllRead { up_to } => {
            let up_to = snapshot(up_to)?;
            let pending: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM notification_groups
                 WHERE recipient = $1 AND expires_at > $2 AND first_seq <= $3
                   AND read_through < LEAST(signal_seq, $3)",
            )
            .bind(recipient)
            .bind(now)
            .bind(up_to.0)
            .fetch_one(&mut *tx)
            .await?;
            let mut read = 0;
            if pending > 0 {
                let version = next_seq(&mut tx, recipient, now).await?;
                read = sqlx::query(
                    "UPDATE notification_groups SET read_through = LEAST(signal_seq, $3),
                         group_version = $4, open = open AND signal_seq > $3
                     WHERE recipient = $1 AND expires_at > $2 AND first_seq <= $3
                       AND read_through < LEAST(signal_seq, $3)",
                )
                .bind(recipient)
                .bind(now)
                .bind(up_to.0)
                .bind(version)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            }
            json!({ "up_to_snapshot": up_to.token(), "groups_marked": read })
        }
    };
    sqlx::query(
        "INSERT INTO inbox_commands (recipient, command_key, request, response, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (recipient, command_key) DO UPDATE SET
             request = EXCLUDED.request, response = EXCLUDED.response,
             created_at = EXCLUDED.created_at, expires_at = EXCLUDED.expires_at",
    )
    .bind(recipient)
    .bind(command_key)
    .bind(&request)
    .bind(&response)
    .bind(now)
    .bind(now + COMMAND_HORIZON)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(response)
}

// ---------------------------------------------------------------------------------------------
// Retention (T-08 CHOICE-22 A) and operator status.

/// Rows deleted by one purge pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Purged {
    pub groups: u64,
    pub receipts: u64,
    pub marks: u64,
    pub fences: u64,
    pub commands: u64,
}

/// Deletes expired rows in batches of `batch` per statement, at most `max_batches` per table.
/// Groups go with their members (P-06A/P-13B); receipts, fences and marks after their own
/// horizons. Inboxes stay: their sequence must never restart, or an old snapshot token could
/// acknowledge newer signals.
pub async fn purge(
    pool: &PgPool,
    now: OffsetDateTime,
    batch: i64,
    max_batches: u32,
) -> Result<Purged, sqlx::Error> {
    let mut purged = Purged::default();
    for (table, counter) in [
        ("notification_groups", &mut purged.groups),
        ("ingestion_receipts", &mut purged.receipts),
        ("tham_marks", &mut purged.marks),
        ("comment_fences", &mut purged.fences),
        ("inbox_commands", &mut purged.commands),
    ] {
        for _ in 0..max_batches {
            // `table` is one of the constant names above, never input.
            let deleted = sqlx::query(AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE ctid IN (
                     SELECT ctid FROM {table} WHERE expires_at <= $1 LIMIT $2)"
            )))
            .bind(now)
            .bind(batch)
            .execute(pool)
            .await?
            .rows_affected();
            *counter += deleted;
            if deleted < u64::try_from(batch).unwrap_or(0) {
                break;
            }
        }
    }
    Ok(purged)
}

/// Sizes and ages for operators and budgets. Counts only; no identifier is ever reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub inboxes: i64,
    pub groups: i64,
    pub open_groups: i64,
    pub expired_groups: i64,
    pub members: i64,
    pub receipts: i64,
    pub marks: i64,
    pub oldest_group_days: Option<i64>,
}

pub async fn status(pool: &PgPool, now: OffsetDateTime) -> Result<Status, sqlx::Error> {
    let row = sqlx::query(
        "SELECT (SELECT count(*) FROM inboxes) AS inboxes,
                (SELECT count(*) FROM notification_groups) AS groups,
                (SELECT count(*) FROM notification_groups WHERE open) AS open_groups,
                (SELECT count(*) FROM notification_groups WHERE expires_at <= $1) AS expired_groups,
                (SELECT count(*) FROM group_members) AS members,
                (SELECT count(*) FROM ingestion_receipts) AS receipts,
                (SELECT count(*) FROM tham_marks) AS marks,
                (SELECT floor(extract(epoch FROM ($1 - min(created_at))) / 86400)::bigint
                   FROM notification_groups) AS oldest_group_days",
    )
    .bind(now)
    .fetch_one(pool)
    .await?;
    Ok(Status {
        inboxes: row.try_get("inboxes")?,
        groups: row.try_get("groups")?,
        open_groups: row.try_get("open_groups")?,
        expired_groups: row.try_get("expired_groups")?,
        members: row.try_get("members")?,
        receipts: row.try_get("receipts")?,
        marks: row.try_get("marks")?,
        oldest_group_days: row.try_get("oldest_group_days")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_and_refuse_anything_else() {
        let id = Uuid::now_v7();
        let token = cursor_token(42, id);
        assert_eq!(parse_cursor(&token), Some((42, id)));
        for bad in [
            "42.x".to_string(),
            format!("c.{id}"),
            format!("c01.{id}"),
            format!("c42.{}", id.to_string().to_uppercase()),
            format!("c42.{}", id.simple()),
            "c42".into(),
        ] {
            assert_eq!(parse_cursor(&bad), None, "{bad}");
        }
    }

    #[test]
    fn commands_have_distinct_canonical_requests() {
        let id = Uuid::nil();
        let read = Command::Read {
            group_id: id,
            observed_version: 3,
        };
        assert_eq!(read.request(), format!("read:{id}:3"));
        assert_ne!(
            Command::Seen { up_to: "s3".into() }.request(),
            Command::MarkAllRead { up_to: "s3".into() }.request()
        );
    }
}
