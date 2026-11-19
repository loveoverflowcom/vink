//! PostgreSQL adapter for Social Follow Graph (#294).
//!
//! Global lock order for `set_follow(actor, target)`:
//! 1. `principals` for `(min(actor, target), max(actor, target))` `FOR UPDATE` in ascending UUID
//!    order, followed by separate standing checks on both (matches `principal::locked_standing`).
//! 2. `author_profiles` for `(min(actor, target), max(actor, target))` `FOR SHARE` in ascending
//!    UUID order.
//! 3. Transaction advisory lock on `(actor, command_id)` so one command ID has one winner across
//!    targets of the actor.
//! 4. `author_follows(actor, target) FOR UPDATE` -> `decide_follow` -> upsert edge + insert
//!    append-only `author_follow_receipts`, then `author_follows(target, actor) FOR SHARE` for
//!    Mutual.
//!
//! `retire_edges` locks `principals(owner) FOR UPDATE` first (via `handle::retire`), retires
//! `author_profiles(owner)`, then locks and deactivates active edges of `owner` in
//! `(follower_id, followee_id)` order without acquiring locks on peer principals.

use std::collections::BTreeSet;

use sqlx::types::Json;
use sqlx::{PgConnection, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::discussion::{integrity, read_snapshot};
use super::handle::ready;
use super::postgres::classify;
use super::principal::require_active;
use crate::application::follow::{
    FollowError, FollowObserved, FollowReceiptView, FollowToken, MembershipBatch, MembershipItem,
    OwnRelationshipList, OwnRelationshipPerson, OwnRelationshipsSummary, PublicRelationshipList,
    PublicRelationships, SetFollowInput, SetFollowResponse, ViewerRelationship,
};
use crate::application::handle::AuthorRef;
use crate::application::ports::StoreError;
use crate::domain::follow::{
    DEFAULT_FOLLOW_PAGE_SIZE, FollowCommand, FollowCommandId, FollowCount, FollowCursor,
    FollowCursorScope, FollowDirection, FollowFacts, FollowIntent, FollowOrder, FollowOutcome,
    FollowPagePosition, FollowReceipt, FollowRefusal, FollowState, MAX_MEMBERSHIP_BATCH,
    MAX_STORE_MEMBERSHIP_CANDIDATES, StoredFollow, can_read_count, can_read_list, cut_follow_page,
    decide_follow, principal_lock_pair,
};
use crate::domain::handle;
use crate::domain::ids::PrincipalId;
use crate::domain::profile::ProfilePolicies;

const CLAIM_ATTEMPTS: usize = 2;
const FOLLOW_KEY_LOCK_SEED: i64 = 29_401;

fn count(raw: i64) -> Result<i64, StoreError> {
    FollowCount::new(raw)
        .map(FollowCount::get)
        .ok_or_else(|| integrity("follow count", raw))
}

fn state_tokens(following: bool, version: i64, order: i64) -> Result<FollowState, StoreError> {
    FollowState::restore(following, version, order)
        .ok_or_else(|| integrity("stored follow tokens", format!("{version}/{order}")))
}

async fn count_active_followers(
    conn: &mut PgConnection,
    target_id: Uuid,
) -> Result<i64, StoreError> {
    let raw: i64 = sqlx::query_scalar(
        "SELECT count(*)
           FROM author_follows f
           JOIN author_profiles fp ON fp.principal_id = f.follower_id
          WHERE f.followee_id = $1 AND f.active AND fp.current_handle IS NOT NULL",
    )
    .bind(target_id)
    .fetch_one(conn)
    .await
    .map_err(classify)?;
    count(raw)
}

async fn count_active_following(
    conn: &mut PgConnection,
    target_id: Uuid,
) -> Result<i64, StoreError> {
    let raw: i64 = sqlx::query_scalar(
        "SELECT count(*)
           FROM author_follows f
           JOIN author_profiles fp ON fp.principal_id = f.followee_id
          WHERE f.follower_id = $1 AND f.active AND fp.current_handle IS NOT NULL",
    )
    .bind(target_id)
    .fetch_one(conn)
    .await
    .map_err(classify)?;
    count(raw)
}

pub(super) async fn public_relationships_summary(
    conn: &mut PgConnection,
    target_id: Uuid,
    handle: &str,
    policies: ProfilePolicies,
) -> Result<PublicRelationships, StoreError> {
    let followers = if can_read_count(policies, FollowDirection::Followers, false) {
        Some(count_active_followers(conn, target_id).await?)
    } else {
        None
    };
    let following = if can_read_count(policies, FollowDirection::Following, false) {
        Some(count_active_following(conn, target_id).await?)
    } else {
        None
    };
    Ok(PublicRelationships::from_policies(
        policies, handle, followers, following,
    ))
}

async fn require_active_actor_read(
    conn: &mut PgConnection,
    actor: PrincipalId,
) -> Result<(String, String), FollowError> {
    let row: Option<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT p.display_name, s.current_handle,
                EXISTS (SELECT 1 FROM principal_suspensions ps
                         WHERE ps.principal_id = p.id AND ps.lifted_at IS NULL) AS suspended
           FROM principals p
           LEFT JOIN author_profiles s ON s.principal_id = p.id
          WHERE p.id = $1",
    )
    .bind(actor.as_uuid())
    .fetch_optional(&mut *conn)
    .await
    .map_err(classify)?;
    let Some((pen_name, current_handle, suspended)) = row else {
        return Err(StoreError::UnknownPrincipal.into());
    };
    if suspended {
        return Err(StoreError::PrincipalSuspended.into());
    }
    let Some(handle) = current_handle else {
        return Err(FollowError::ActorUnavailable);
    };
    Ok((pen_name, handle))
}

async fn lock_follow_row(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
    target: PrincipalId,
) -> Result<Option<FollowState>, StoreError> {
    let row: Option<(bool, i64, i64)> = sqlx::query_as(
        "SELECT active, version, accepted_order
           FROM author_follows
          WHERE follower_id = $1 AND followee_id = $2
            FOR UPDATE",
    )
    .bind(actor.as_uuid())
    .bind(target.as_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    row.map(|(active, version, order)| state_tokens(active, version, order))
        .transpose()
}

async fn write_follow_row(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
    target: PrincipalId,
    from: Option<FollowState>,
    next: FollowState,
) -> Result<bool, StoreError> {
    let affected = match from {
        Some(from) => sqlx::query(
            "UPDATE author_follows
                SET active = $3,
                    version = $4,
                    accepted_order = $5,
                    active_since = CASE
                        WHEN $3 AND NOT active THEN now()
                        WHEN $3 THEN active_since
                        ELSE NULL
                    END,
                    updated_at = now()
              WHERE follower_id = $1 AND followee_id = $2 AND accepted_order = $6",
        )
        .bind(actor.as_uuid())
        .bind(target.as_uuid())
        .bind(next.following())
        .bind(next.version().get())
        .bind(next.order().get())
        .bind(from.order().get())
        .execute(&mut **tx)
        .await
        .map_err(classify)?
        .rows_affected(),
        None => sqlx::query(
            "INSERT INTO author_follows
                    (follower_id, followee_id, active, version, accepted_order, active_since)
             VALUES ($1, $2, $3, $4, $5, CASE WHEN $3 THEN now() ELSE NULL END)
             ON CONFLICT (follower_id, followee_id) DO NOTHING",
        )
        .bind(actor.as_uuid())
        .bind(target.as_uuid())
        .bind(next.following())
        .bind(next.version().get())
        .bind(next.order().get())
        .execute(&mut **tx)
        .await
        .map_err(classify)?
        .rows_affected(),
    };
    if from.is_some() && affected != 1 {
        return Err(integrity("locked follow edge changed", actor));
    }
    Ok(affected == 1)
}

async fn write_receipt(
    tx: &mut Transaction<'_, Postgres>,
    receipt: FollowReceipt,
) -> Result<(), StoreError> {
    let command = receipt.command();
    let committed = receipt.committed();
    sqlx::query(
        "INSERT INTO author_follow_receipts
                (actor_id, command_id, target_id, expected_order, desired_following,
                 committed_following, committed_version, committed_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(receipt.actor().as_uuid())
    .bind(command.command_id.as_uuid())
    .bind(command.target.as_uuid())
    .bind(command.expected_order.get())
    .bind(command.intent.following())
    .bind(committed.following())
    .bind(committed.version().get())
    .bind(committed.order().get())
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PersonRow {
    display_name: String,
    handle: String,
    cursor_key: Uuid,
}

#[derive(sqlx::FromRow)]
struct OwnPersonRow {
    display_name: String,
    handle: String,
    cursor_key: Uuid,
    mutual: bool,
}

/// Resolve an opaque anchor against the same snapshot and projection as the page.
/// An edge which left the list can no longer provide a sort position.
async fn resolve_list_anchor(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    direction: FollowDirection,
    cursor_key: Uuid,
) -> Result<Option<FollowPagePosition>, StoreError> {
    let sql = match direction {
        FollowDirection::Followers => {
            "SELECT f.active_since FROM author_follows f
               JOIN author_profiles p ON p.principal_id = f.follower_id
              WHERE f.cursor_key = $1 AND f.followee_id = $2 AND f.active
                AND p.current_handle IS NOT NULL"
        }
        FollowDirection::Following => {
            "SELECT f.active_since FROM author_follows f
               JOIN author_profiles p ON p.principal_id = f.followee_id
              WHERE f.cursor_key = $1 AND f.follower_id = $2 AND f.active
                AND p.current_handle IS NOT NULL"
        }
    };
    let timestamp: Option<OffsetDateTime> = sqlx::query_scalar(sql)
        .bind(cursor_key)
        .bind(owner)
        .fetch_optional(&mut **tx)
        .await
        .map_err(classify)?;
    Ok(timestamp.map(|active_since| FollowPagePosition {
        active_since,
        cursor_key,
    }))
}

async fn fetch_public_page(
    tx: &mut Transaction<'_, Postgres>,
    target_id: Uuid,
    direction: FollowDirection,
    after: Option<FollowPagePosition>,
    limit: usize,
) -> Result<Vec<PersonRow>, StoreError> {
    let sql = match (direction, after.is_some()) {
        (FollowDirection::Followers, false) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.follower_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.followee_id = $1
                AND f.active
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $2"
        }
        (FollowDirection::Followers, true) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.follower_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.followee_id = $1
                AND f.active
                AND f.active_since <= $2
                AND (f.active_since < $2 OR f.cursor_key > $3)
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $4"
        }
        (FollowDirection::Following, false) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.followee_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.follower_id = $1
                AND f.active
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $2"
        }
        (FollowDirection::Following, true) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.followee_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.follower_id = $1
                AND f.active
                AND f.active_since <= $2
                AND (f.active_since < $2 OR f.cursor_key > $3)
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $4"
        }
    };
    let rows: Vec<PersonRow> = match after {
        None => sqlx::query_as(sql)
            .bind(target_id)
            .bind(limit as i64)
            .fetch_all(&mut **tx)
            .await
            .map_err(classify)?,
        Some(pos) => sqlx::query_as(sql)
            .bind(target_id)
            .bind(pos.active_since)
            .bind(pos.cursor_key)
            .bind(limit as i64)
            .fetch_all(&mut **tx)
            .await
            .map_err(classify)?,
    };
    Ok(rows)
}

async fn fetch_own_page(
    tx: &mut Transaction<'_, Postgres>,
    actor_id: Uuid,
    direction: FollowDirection,
    after: Option<FollowPagePosition>,
    limit: usize,
) -> Result<Vec<OwnPersonRow>, StoreError> {
    let sql = match (direction, after.is_some()) {
        (FollowDirection::Followers, false) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key,
                    EXISTS (SELECT 1 FROM author_follows rev
                             WHERE rev.follower_id = $1
                               AND rev.followee_id = f.follower_id
                               AND rev.active) AS mutual
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.follower_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.followee_id = $1
                AND f.active
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $2"
        }
        (FollowDirection::Followers, true) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key,
                    EXISTS (SELECT 1 FROM author_follows rev
                             WHERE rev.follower_id = $1
                               AND rev.followee_id = f.follower_id
                               AND rev.active) AS mutual
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.follower_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.followee_id = $1
                AND f.active
                AND f.active_since <= $2
                AND (f.active_since < $2 OR f.cursor_key > $3)
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $4"
        }
        (FollowDirection::Following, false) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key,
                    EXISTS (SELECT 1 FROM author_follows rev
                             WHERE rev.follower_id = f.followee_id
                               AND rev.followee_id = $1
                               AND rev.active) AS mutual
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.followee_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.follower_id = $1
                AND f.active
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $2"
        }
        (FollowDirection::Following, true) => {
            "SELECT p.display_name, ap.current_handle AS handle, f.cursor_key,
                    EXISTS (SELECT 1 FROM author_follows rev
                             WHERE rev.follower_id = f.followee_id
                               AND rev.followee_id = $1
                               AND rev.active) AS mutual
               FROM author_follows f
               JOIN author_profiles ap ON ap.principal_id = f.followee_id
               JOIN principals p ON p.id = ap.principal_id
              WHERE f.follower_id = $1
                AND f.active
                AND f.active_since <= $2
                AND (f.active_since < $2 OR f.cursor_key > $3)
                AND ap.current_handle IS NOT NULL
              ORDER BY f.active_since DESC, f.cursor_key ASC
              LIMIT $4"
        }
    };
    let rows: Vec<OwnPersonRow> = match after {
        None => sqlx::query_as(sql)
            .bind(actor_id)
            .bind(limit as i64)
            .fetch_all(&mut **tx)
            .await
            .map_err(classify)?,
        Some(pos) => sqlx::query_as(sql)
            .bind(actor_id)
            .bind(pos.active_since)
            .bind(pos.cursor_key)
            .bind(limit as i64)
            .fetch_all(&mut **tx)
            .await
            .map_err(classify)?,
    };
    Ok(rows)
}

/// Discovery / Feed handoff (#296, #295): filters `candidates` inside the caller's transaction
/// to those actively followed by `viewer` where both profiles remain active.
pub async fn followed_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    viewer: PrincipalId,
    candidates: &[PrincipalId],
) -> Result<BTreeSet<PrincipalId>, StoreError> {
    if candidates.len() > MAX_STORE_MEMBERSHIP_CANDIDATES {
        return Err(StoreError::Integrity(
            "membership candidate batch exceeds limit".into(),
        ));
    }
    if candidates.is_empty() {
        return Ok(BTreeSet::new());
    }
    let ids: Vec<Uuid> = candidates.iter().map(PrincipalId::as_uuid).collect();
    let rows: Vec<Uuid> = sqlx::query_scalar(
        "SELECT f.followee_id
           FROM author_follows f
           JOIN author_profiles vp ON vp.principal_id = f.follower_id
           JOIN author_profiles fp ON fp.principal_id = f.followee_id
          WHERE f.follower_id = $1
            AND f.followee_id = ANY($2::uuid[])
            AND f.active
            AND vp.current_handle IS NOT NULL
            AND fp.current_handle IS NOT NULL",
    )
    .bind(viewer.as_uuid())
    .bind(ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(rows.into_iter().map(PrincipalId::from_uuid).collect())
}

/// Account deletion hook (#297): deactivates up to `limit` active incoming/outgoing edges of
/// `owner` in deterministic order while preserving parity and order history.
pub async fn retire_edges_batch(
    tx: &mut Transaction<'_, Postgres>,
    owner: PrincipalId,
    limit: i64,
) -> Result<u64, StoreError> {
    if limit <= 0 {
        return Ok(0);
    }
    let updated = sqlx::query(
        "WITH target_rows AS (
             SELECT follower_id, followee_id
               FROM author_follows
              WHERE (follower_id = $1 OR followee_id = $1)
                AND active
              ORDER BY follower_id, followee_id
              LIMIT $2
              FOR UPDATE
         )
         UPDATE author_follows f
            SET active = false,
                version = f.version + 1,
                accepted_order = f.accepted_order + 1,
                active_since = NULL,
                updated_at = now()
           FROM target_rows t
          WHERE f.follower_id = t.follower_id
            AND f.followee_id = t.followee_id
            AND f.active",
    )
    .bind(owner.as_uuid())
    .bind(limit)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(updated.rows_affected())
}

/// Retires all active incoming and outgoing edges of `owner` in bounded batches inside `tx`.
pub async fn retire_edges(
    tx: &mut Transaction<'_, Postgres>,
    owner: PrincipalId,
) -> Result<u64, StoreError> {
    const BATCH: i64 = 500;
    let mut total = 0u64;
    loop {
        let affected = retire_edges_batch(tx, owner, BATCH).await?;
        total += affected;
        if affected < BATCH as u64 {
            break;
        }
    }
    Ok(total)
}

impl PgStore {
    pub async fn set_follow(
        &self,
        actor: PrincipalId,
        target_handle: &str,
        input: SetFollowInput,
    ) -> Result<SetFollowResponse, FollowError> {
        if !handle::valid(target_handle) {
            return Err(FollowError::NotFound);
        }
        let expected_order =
            FollowOrder::new(input.expected_order).ok_or(FollowError::Invalid("expected_order"))?;
        ready(self.pool()).await?;

        let target_uuid: Option<Uuid> =
            sqlx::query_scalar("SELECT principal_id FROM author_handle_names WHERE handle = $1")
                .bind(target_handle)
                .fetch_optional(self.pool())
                .await
                .map_err(classify)?;

        let mut tx = self.pool().begin().await.map_err(classify)?;
        let Some(target_uuid) = target_uuid else {
            require_active(&mut tx, actor).await?;
            return Err(FollowError::NotFound);
        };
        let target = PrincipalId::from_uuid(target_uuid);

        if target == actor {
            require_active(&mut tx, actor).await?;
            return Err(FollowError::SelfFollow);
        }

        let (lo, hi) = principal_lock_pair(actor, target).expect("checked distinct");
        let lo_known: Option<i32> =
            sqlx::query_scalar("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
                .bind(lo.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let hi_known: Option<i32> =
            sqlx::query_scalar("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
                .bind(hi.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let (actor_known, target_known) = if actor == lo {
            (lo_known.is_some(), hi_known.is_some())
        } else {
            (hi_known.is_some(), lo_known.is_some())
        };
        if !actor_known {
            return Err(StoreError::UnknownPrincipal.into());
        }

        let actor_suspended: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM principal_suspensions
                             WHERE principal_id = $1 AND lifted_at IS NULL)",
        )
        .bind(actor.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        if actor_suspended {
            return Err(StoreError::PrincipalSuspended.into());
        }

        let target_suspended: bool = if target_known {
            sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM principal_suspensions
                                 WHERE principal_id = $1 AND lifted_at IS NULL)",
            )
            .bind(target.as_uuid())
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?
        } else {
            true
        };

        let lo_profile: Option<(Option<String>, Json<ProfilePolicies>)> = sqlx::query_as(
            "SELECT current_handle, policies FROM author_profiles WHERE principal_id = $1 FOR SHARE",
        )
        .bind(lo.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let hi_profile: Option<(Option<String>, Json<ProfilePolicies>)> = sqlx::query_as(
            "SELECT current_handle, policies FROM author_profiles WHERE principal_id = $1 FOR SHARE",
        )
        .bind(hi.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let (actor_profile, target_profile) = if actor == lo {
            (lo_profile, hi_profile)
        } else {
            (hi_profile, lo_profile)
        };

        let actor_eligible = actor_profile.as_ref().is_some_and(|(h, _)| h.is_some());
        let (target_current_handle, target_policies) = match target_profile {
            Some((handle, policies)) => (handle, policies.0),
            None => (None, ProfilePolicies::default()),
        };
        let target_eligible = target_known && target_current_handle.is_some() && !target_suspended;

        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1::text || '/' || $2::text, $3))",
        )
        .bind(actor.as_uuid())
        .bind(input.command_id)
        .bind(FOLLOW_KEY_LOCK_SEED)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;

        #[derive(sqlx::FromRow)]
        struct ReceiptRow {
            target_id: Uuid,
            expected_order: i64,
            desired_following: bool,
            committed_following: bool,
            committed_version: i64,
            committed_order: i64,
        }
        let receipt_row: Option<ReceiptRow> = sqlx::query_as(
            "SELECT target_id, expected_order, desired_following,
                    committed_following, committed_version, committed_order
               FROM author_follow_receipts
              WHERE actor_id = $1 AND command_id = $2",
        )
        .bind(actor.as_uuid())
        .bind(input.command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let receipt = receipt_row
            .map(|row| -> Result<FollowReceipt, StoreError> {
                let cmd = FollowCommand {
                    command_id: FollowCommandId::from_uuid(input.command_id),
                    target: PrincipalId::from_uuid(row.target_id),
                    expected_order: FollowOrder::new(row.expected_order)
                        .ok_or_else(|| integrity("receipt expected_order", row.expected_order))?,
                    intent: FollowIntent::from_following(row.desired_following),
                };
                let committed = state_tokens(
                    row.committed_following,
                    row.committed_version,
                    row.committed_order,
                )?;
                FollowReceipt::restore(actor, cmd, committed)
                    .ok_or_else(|| integrity("follow receipt mismatch", input.command_id))
            })
            .transpose()?;

        let command = FollowCommand {
            command_id: FollowCommandId::from_uuid(input.command_id),
            target,
            expected_order,
            intent: FollowIntent::from_following(input.following),
        };

        for _ in 0..CLAIM_ATTEMPTS {
            let current = lock_follow_row(&mut tx, actor, target).await?;
            let facts = FollowFacts {
                actor_eligible,
                target_eligible,
                current: current.map(|state| StoredFollow::restore(actor, target, state)),
                receipt,
            };
            match decide_follow(actor, command, facts) {
                FollowOutcome::Accepted(write) => {
                    if !write_follow_row(&mut tx, actor, target, current, write.next().state())
                        .await?
                    {
                        continue;
                    }
                    write_receipt(&mut tx, write.receipt()).await?;
                    let state = write.next().state();
                    let reverse_active: bool = sqlx::query_scalar(
                        "SELECT COALESCE(
                             (SELECT active FROM author_follows
                               WHERE follower_id = $1 AND followee_id = $2 FOR SHARE),
                             false
                         )",
                    )
                    .bind(target.as_uuid())
                    .bind(actor.as_uuid())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?;
                    let current_handle = target_current_handle
                        .as_deref()
                        .ok_or_else(|| integrity("missing target handle", target))?;
                    let relationships = public_relationships_summary(
                        &mut tx,
                        target.as_uuid(),
                        current_handle,
                        target_policies,
                    )
                    .await?;
                    tx.commit().await.map_err(classify)?;
                    return Ok(SetFollowResponse {
                        target_handle: current_handle.to_owned(),
                        outcome: "accepted",
                        following: state.following(),
                        mutual: state.following() && reverse_active,
                        pair_version: state.version().get(),
                        accepted_order: state.order().get(),
                        committed: None,
                        relationships,
                    });
                }
                FollowOutcome::Replay {
                    committed,
                    observed,
                } => {
                    let reverse_active: bool = sqlx::query_scalar(
                        "SELECT COALESCE(
                             (SELECT active FROM author_follows
                               WHERE follower_id = $1 AND followee_id = $2 FOR SHARE),
                             false
                         )",
                    )
                    .bind(target.as_uuid())
                    .bind(actor.as_uuid())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?;
                    let current_handle = target_current_handle
                        .as_deref()
                        .ok_or_else(|| integrity("missing target handle", target))?;
                    let relationships = public_relationships_summary(
                        &mut tx,
                        target.as_uuid(),
                        current_handle,
                        target_policies,
                    )
                    .await?;
                    tx.commit().await.map_err(classify)?;
                    return Ok(SetFollowResponse {
                        target_handle: current_handle.to_owned(),
                        outcome: "replayed",
                        following: observed.following(),
                        mutual: observed.following() && reverse_active,
                        pair_version: observed.version().get(),
                        accepted_order: observed.order().get(),
                        committed: Some(FollowToken {
                            following: committed.following(),
                            pair_version: committed.version().get(),
                            accepted_order: committed.order().get(),
                        }),
                        relationships,
                    });
                }
                FollowOutcome::Conflict { observed } => {
                    let reverse_active: bool = sqlx::query_scalar(
                        "SELECT COALESCE(
                             (SELECT active FROM author_follows
                               WHERE follower_id = $1 AND followee_id = $2 FOR SHARE),
                             false
                         )",
                    )
                    .bind(target.as_uuid())
                    .bind(actor.as_uuid())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?;
                    let current_handle = target_current_handle
                        .as_deref()
                        .ok_or_else(|| integrity("missing target handle", target))?;
                    tx.commit().await.map_err(classify)?;
                    return Err(FollowError::Conflict(FollowObserved {
                        target_handle: current_handle.to_owned(),
                        following: observed.following(),
                        mutual: observed.following() && reverse_active,
                        pair_version: observed.version().get(),
                        accepted_order: observed.order().get(),
                    }));
                }
                FollowOutcome::Refused(FollowRefusal::SelfFollow) => {
                    return Err(FollowError::SelfFollow);
                }
                FollowOutcome::Refused(FollowRefusal::ActorUnavailable) => {
                    return Err(FollowError::ActorUnavailable);
                }
                FollowOutcome::Refused(FollowRefusal::TargetUnavailable) => {
                    if target_current_handle.is_none() {
                        return Err(FollowError::Gone);
                    }
                    return Err(FollowError::TargetUnavailable);
                }
                FollowOutcome::Refused(FollowRefusal::KeyReused) => {
                    return Err(FollowError::KeyReused);
                }
                FollowOutcome::Refused(
                    FollowRefusal::InvalidFacts | FollowRefusal::TokenExhausted,
                ) => {
                    return Err(integrity("follow transition refused", actor).into());
                }
            }
        }
        Err(integrity("follow claim did not settle", actor).into())
    }

    pub async fn follow_receipt(
        &self,
        actor: PrincipalId,
        command_id: Uuid,
    ) -> Result<FollowReceiptView, FollowError> {
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;
        require_active_actor_read(&mut tx, actor).await?;

        #[derive(sqlx::FromRow)]
        struct Row {
            target_id: Uuid,
            target_handle: Option<String>,
            target_suspended: bool,
            expected_order: i64,
            desired_following: bool,
            committed_following: bool,
            committed_version: i64,
            committed_order: i64,
            observed_active: Option<bool>,
            observed_version: Option<i64>,
            observed_order: Option<i64>,
            reverse_active: bool,
        }
        let row: Option<Row> = sqlx::query_as(
            "SELECT r.target_id,
                    tp.current_handle AS target_handle,
                    EXISTS (SELECT 1 FROM principal_suspensions ps
                             WHERE ps.principal_id = r.target_id AND ps.lifted_at IS NULL) AS target_suspended,
                    r.expected_order,
                    r.desired_following,
                    r.committed_following,
                    r.committed_version,
                    r.committed_order,
                    f.active AS observed_active,
                    f.version AS observed_version,
                    f.accepted_order AS observed_order,
                    COALESCE((SELECT rev.active FROM author_follows rev
                               WHERE rev.follower_id = r.target_id AND rev.followee_id = r.actor_id), false) AS reverse_active
               FROM author_follow_receipts r
               JOIN author_profiles tp ON tp.principal_id = r.target_id
               LEFT JOIN author_follows f ON f.follower_id = r.actor_id AND f.followee_id = r.target_id
              WHERE r.actor_id = $1 AND r.command_id = $2",
        )
        .bind(actor.as_uuid())
        .bind(command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;

        let Some(row) = row else {
            return Err(FollowError::NotFound);
        };
        let Some(target_handle) = row.target_handle else {
            return Err(FollowError::NotFound);
        };
        if row.target_suspended {
            return Err(FollowError::NotFound);
        }
        let committed = state_tokens(
            row.committed_following,
            row.committed_version,
            row.committed_order,
        )?;
        let observed = match (
            row.observed_active,
            row.observed_version,
            row.observed_order,
        ) {
            (Some(a), Some(v), Some(o)) => state_tokens(a, v, o)?,
            (None, None, None) => FollowState::INITIAL,
            _ => return Err(integrity("partial follow state", row.target_id).into()),
        };
        Ok(FollowReceiptView {
            command_id,
            target_handle: target_handle.clone(),
            expected_order: row.expected_order,
            desired_following: row.desired_following,
            committed: FollowToken {
                following: committed.following(),
                pair_version: committed.version().get(),
                accepted_order: committed.order().get(),
            },
            observed: FollowObserved {
                target_handle,
                following: observed.following(),
                mutual: observed.following() && row.reverse_active,
                pair_version: observed.version().get(),
                accepted_order: observed.order().get(),
            },
        })
    }

    pub async fn viewer_relationship(
        &self,
        actor: PrincipalId,
        handle: &str,
    ) -> Result<ViewerRelationship, FollowError> {
        if !handle::valid(handle) {
            return Err(FollowError::NotFound);
        }
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;
        require_active_actor_read(&mut tx, actor).await?;

        #[derive(sqlx::FromRow)]
        struct TargetRow {
            principal_id: Uuid,
            current_handle: Option<String>,
            suspended: bool,
        }
        let target: Option<TargetRow> = sqlx::query_as(
            "SELECT s.principal_id, s.current_handle,
                    EXISTS (SELECT 1 FROM principal_suspensions ps
                             WHERE ps.principal_id = s.principal_id AND ps.lifted_at IS NULL) AS suspended
               FROM author_handle_names n
               JOIN author_profiles s USING (principal_id)
              WHERE n.handle = $1",
        )
        .bind(handle)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some(target) = target else {
            tx.commit().await.map_err(classify)?;
            return Err(FollowError::NotFound);
        };
        let Some(target_handle) = target.current_handle else {
            tx.commit().await.map_err(classify)?;
            return Err(FollowError::Gone);
        };
        if target.principal_id == actor.as_uuid() {
            let followers_count = count_active_followers(&mut tx, actor.as_uuid()).await?;
            let following_count = count_active_following(&mut tx, actor.as_uuid()).await?;
            tx.commit().await.map_err(classify)?;
            return Ok(ViewerRelationship::SelfProfile {
                target_handle: target_handle.clone(),
                can_follow: false,
                own_relationships: OwnRelationshipsSummary {
                    followers_count,
                    following_count,
                    followers_href: format!(
                        "/@{target_handle}/{}",
                        FollowDirection::Followers.web_segment()
                    ),
                    following_href: format!(
                        "/@{target_handle}/{}",
                        FollowDirection::Following.web_segment()
                    ),
                },
            });
        }
        let pair: Option<(bool, i64, i64)> = sqlx::query_as(
            "SELECT active, version, accepted_order
               FROM author_follows
              WHERE follower_id = $1 AND followee_id = $2",
        )
        .bind(actor.as_uuid())
        .bind(target.principal_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let reverse_active: bool = sqlx::query_scalar(
            "SELECT COALESCE(
                 (SELECT active FROM author_follows
                   WHERE follower_id = $1 AND followee_id = $2),
                 false
             )",
        )
        .bind(target.principal_id)
        .bind(actor.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;

        let state = pair
            .map(|(a, v, o)| state_tokens(a, v, o))
            .transpose()?
            .unwrap_or(FollowState::INITIAL);
        Ok(ViewerRelationship::Other {
            target_handle,
            can_follow: !target.suspended,
            following: state.following(),
            mutual: state.following() && reverse_active,
            pair_version: state.version().get(),
            accepted_order: state.order().get(),
        })
    }

    pub async fn public_relationships(
        &self,
        handle: &str,
        direction: FollowDirection,
        cursor: Result<Option<String>, FollowError>,
    ) -> Result<PublicRelationshipList, FollowError> {
        if !handle::valid(handle) {
            return Err(FollowError::NotFound);
        }
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;

        #[derive(sqlx::FromRow)]
        struct TargetRow {
            principal_id: Uuid,
            pen_name: String,
            current_handle: Option<String>,
            policies: Json<ProfilePolicies>,
        }
        let target: Option<TargetRow> = sqlx::query_as(
            "SELECT s.principal_id, p.display_name AS pen_name, s.current_handle, s.policies
               FROM author_handle_names n
               JOIN author_profiles s USING (principal_id)
               JOIN principals p ON p.id = s.principal_id
              WHERE n.handle = $1",
        )
        .bind(handle)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some(target) = target else {
            tx.commit().await.map_err(classify)?;
            return Err(FollowError::NotFound);
        };
        let Some(current_handle) = target.current_handle else {
            tx.commit().await.map_err(classify)?;
            return Err(FollowError::Gone);
        };
        if current_handle != handle {
            tx.commit().await.map_err(classify)?;
            return Ok(PublicRelationshipList::Alias {
                href: format!("/@{current_handle}/{}", direction.web_segment()),
            });
        }
        let policies = target.policies.0;
        // Rights precede cursor validation so unauthorized probes never distinguish cursors.
        if !can_read_list(policies, direction, false) {
            tx.commit().await.map_err(classify)?;
            return Err(FollowError::ListPrivate);
        }

        let target_id = PrincipalId::from_uuid(target.principal_id);
        let author = AuthorRef::new(target.pen_name, current_handle);
        let scope = FollowCursorScope::public(target_id, direction);
        let after = match cursor?.as_deref() {
            None | Some("") => None,
            Some(raw) => {
                let position = match FollowCursor::decode(raw)
                    .ok()
                    .and_then(|cursor| cursor.anchor(scope))
                {
                    Some(anchor) => {
                        resolve_list_anchor(&mut tx, target.principal_id, direction, anchor).await?
                    }
                    None => None,
                };
                if position.is_none() {
                    tx.commit().await.map_err(classify)?;
                    return Ok(PublicRelationshipList::Restart { author, direction });
                }
                position
            }
        };

        let page_size = DEFAULT_FOLLOW_PAGE_SIZE;
        let rows = fetch_public_page(
            &mut tx,
            target.principal_id,
            direction,
            after,
            page_size + 1,
        )
        .await?;
        let count = if can_read_count(policies, direction, false) {
            Some(match direction {
                FollowDirection::Followers => {
                    count_active_followers(&mut tx, target.principal_id).await?
                }
                FollowDirection::Following => {
                    count_active_following(&mut tx, target.principal_id).await?
                }
            })
        } else {
            None
        };
        tx.commit().await.map_err(classify)?;

        let (rows, next) = cut_follow_page(rows, page_size, scope, |row| row.cursor_key);
        Ok(PublicRelationshipList::Page {
            author,
            direction,
            count,
            people: rows
                .into_iter()
                .map(|row| AuthorRef::new(row.display_name, row.handle))
                .collect(),
            next_cursor: next.map(|cursor| cursor.encode()),
        })
    }

    pub async fn own_relationships(
        &self,
        actor: PrincipalId,
        direction: FollowDirection,
        session_scope: [u8; 32],
        cursor: Option<&str>,
    ) -> Result<OwnRelationshipList, FollowError> {
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;
        let (pen_name, handle) = require_active_actor_read(&mut tx, actor).await?;
        let author = AuthorRef::new(pen_name, handle);
        let scope = FollowCursorScope::own(actor, direction, session_scope);
        let after = match cursor {
            None | Some("") => None,
            Some(raw) => {
                let position = match FollowCursor::decode(raw)
                    .ok()
                    .and_then(|cursor| cursor.anchor(scope))
                {
                    Some(anchor) => {
                        resolve_list_anchor(&mut tx, actor.as_uuid(), direction, anchor).await?
                    }
                    None => None,
                };
                if position.is_none() {
                    tx.commit().await.map_err(classify)?;
                    return Ok(OwnRelationshipList::Restart { author, direction });
                }
                position
            }
        };

        let page_size = DEFAULT_FOLLOW_PAGE_SIZE;
        let rows =
            fetch_own_page(&mut tx, actor.as_uuid(), direction, after, page_size + 1).await?;
        let count = match direction {
            FollowDirection::Followers => count_active_followers(&mut tx, actor.as_uuid()).await?,
            FollowDirection::Following => count_active_following(&mut tx, actor.as_uuid()).await?,
        };
        tx.commit().await.map_err(classify)?;

        let (rows, next) = cut_follow_page(rows, page_size, scope, |row| row.cursor_key);
        Ok(OwnRelationshipList::Page {
            author,
            direction,
            count,
            people: rows
                .into_iter()
                .map(|row| OwnRelationshipPerson {
                    author: AuthorRef::new(row.display_name, row.handle),
                    mutual: row.mutual,
                })
                .collect(),
            next_cursor: next.map(|cursor| cursor.encode()),
        })
    }

    /// Snapshot wrapper of [`followed_in_tx`] for Feed/Discovery (#296, #295).
    pub async fn followed_principals_in(
        &self,
        viewer: PrincipalId,
        candidates: &[PrincipalId],
    ) -> Result<BTreeSet<PrincipalId>, FollowError> {
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;
        require_active_actor_read(&mut tx, viewer).await?;
        let result = followed_in_tx(&mut tx, viewer, candidates).await?;
        tx.commit().await.map_err(classify)?;
        Ok(result)
    }

    /// Batch viewer membership by active handle for #296 / #295.
    pub async fn batch_membership_by_handles(
        &self,
        viewer: PrincipalId,
        handles: &[String],
    ) -> Result<MembershipBatch, FollowError> {
        if handles.len() > MAX_MEMBERSHIP_BATCH {
            return Err(FollowError::Invalid("handles"));
        }
        let mut unique = Vec::with_capacity(handles.len());
        for raw in handles {
            if !handle::valid(raw) {
                return Err(FollowError::Invalid("handles"));
            }
            if !unique.contains(raw) {
                unique.push(raw.clone());
            }
        }
        ready(self.pool()).await?;
        let mut tx = read_snapshot(self.pool()).await?;
        require_active_actor_read(&mut tx, viewer).await?;
        if unique.is_empty() {
            tx.commit().await.map_err(classify)?;
            return Ok(MembershipBatch { items: Vec::new() });
        }

        #[derive(sqlx::FromRow)]
        struct BatchRow {
            handle: String,
            active: Option<bool>,
            version: Option<i64>,
            accepted_order: Option<i64>,
            reverse_active: bool,
        }
        let rows: Vec<BatchRow> = sqlx::query_as(
            "SELECT s.current_handle AS handle,
                    f.active,
                    f.version,
                    f.accepted_order,
                    COALESCE((SELECT rev.active FROM author_follows rev
                               WHERE rev.follower_id = s.principal_id AND rev.followee_id = $1), false) AS reverse_active
               FROM author_profiles s
               LEFT JOIN author_follows f
                      ON f.follower_id = $1 AND f.followee_id = s.principal_id
              WHERE s.current_handle = ANY($2::text[])
                AND s.principal_id <> $1",
        )
        .bind(viewer.as_uuid())
        .bind(&unique)
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;

        let mut items = Vec::with_capacity(rows.len());
        for wanted in &unique {
            if let Some(row) = rows.iter().find(|r| &r.handle == wanted) {
                let state = match (row.active, row.version, row.accepted_order) {
                    (Some(a), Some(v), Some(o)) => state_tokens(a, v, o)?,
                    (None, None, None) => FollowState::INITIAL,
                    _ => return Err(integrity("partial membership follow row", viewer).into()),
                };
                items.push(MembershipItem {
                    handle: row.handle.clone(),
                    following: state.following(),
                    mutual: state.following() && row.reverse_active,
                    pair_version: state.version().get(),
                    accepted_order: state.order().get(),
                });
            }
        }
        Ok(MembershipBatch { items })
    }
}
