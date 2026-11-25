//! Publication locks precede principal locks. The actor lock is also the quota
//! fence across *different* works; count after acquiring it under READ COMMITTED.
mod cursor;

use sqlx::{PgConnection, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{PgStore, discussion::read_snapshot, postgres::classify, principal::require_active};
use crate::application::{
    ports::StoreError,
    profile::AuthorAttribution,
    saved_works::{ListItem, SavedPage, SavedWorksError, SavedWorksStore},
};
use crate::domain::{
    ids::{PrincipalId, RevisionId, WorkId},
    principal::Standing,
    saved_work::*,
};
use cursor::Anchor;

const PAGE_SIZE: usize = 20;

fn invalid_row() -> StoreError {
    StoreError::Integrity("invalid saved work projection".into())
}

fn restore(
    key: SavedWorkKey,
    generation: i64,
    saved_at: Option<OffsetDateTime>,
) -> Result<SavedWork, StoreError> {
    SavedGeneration::new(generation)
        .and_then(|g| SavedWork::restore(key, g, saved_at))
        .ok_or_else(invalid_row)
}

async fn load(conn: &mut PgConnection, key: SavedWorkKey) -> Result<SavedWork, StoreError> {
    let pair: Option<(i64, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT generation, saved_at FROM saved_works WHERE principal_id = $1 AND work_id = $2",
    )
    .bind(key.principal.as_uuid())
    .bind(key.work.as_uuid())
    .fetch_optional(conn)
    .await
    .map_err(classify)?;
    match pair {
        Some((g, at)) => restore(key, g, at),
        None => Ok(SavedWork::initial(key)),
    }
}

// Retirement can retain the principal record and its binding permanently.
async fn require_live_owner(conn: &mut PgConnection, actor: PrincipalId) -> Result<(), StoreError> {
    let suspended: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM principal_suspensions WHERE principal_id = a.id AND lifted_at IS NULL)
         FROM public_author_attribution a WHERE a.id = $1 AND NOT a.retired",
    ).bind(actor.as_uuid()).fetch_optional(conn).await.map_err(classify)?;
    match suspended {
        None => Err(StoreError::UnknownPrincipal),
        Some(true) => Err(StoreError::PrincipalSuspended),
        Some(false) => Ok(()),
    }
}

async fn lock_author(tx: &mut Transaction<'_, Postgres>, author: Uuid) -> Result<(), StoreError> {
    sqlx::query("SELECT id FROM principals WHERE id = $1 FOR KEY SHARE")
        .bind(author)
        .fetch_one(&mut **tx)
        .await
        .map_err(classify)?;
    Ok(())
}

async fn command_access(
    tx: &mut Transaction<'_, Postgres>,
    command: SetSaved,
) -> Result<SavedWorkAccess, StoreError> {
    let SavedWorkKey { principal, work } = command.key;
    // Lock the existing public pointer. Never accept an initially absent pointer that
    // appears later in this transaction without a lock. DELETE needs no target lock.
    let author: Option<Uuid> = if command.desired == SaveIntent::Saved {
        sqlx::query_scalar(
            "SELECT w.owner_id FROM publications p JOIN works w ON w.id = p.work_id
                             WHERE p.work_id = $1 FOR SHARE OF p, w",
        )
        .bind(work.as_uuid())
        .fetch_optional(&mut **tx)
        .await
        .map_err(classify)?
    } else {
        None
    };
    // Follow uses the same UUID order. A shared lock fences author retirement
    // without serializing saves by different readers of a popular author.
    if let Some(id) = author.filter(|id| *id < principal.as_uuid()) {
        lock_author(tx, id).await?;
    }
    require_active(tx, principal).await?;
    if let Some(id) = author.filter(|id| *id > principal.as_uuid()) {
        lock_author(tx, id).await?;
    }
    require_live_owner(tx, principal).await?;
    if author.is_none() {
        return Ok(SavedWorkAccess::Gone);
    }
    let revision: Option<Uuid> = sqlx::query_scalar(
        "SELECT revision_id FROM saved_work_targets WHERE work_id = $1 AND kind = 'readable'",
    )
    .bind(work.as_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(
        revision.map_or(SavedWorkAccess::Gone, |id| SavedWorkAccess::Public {
            work,
            revision: RevisionId::from_uuid(id),
        }),
    )
}

#[derive(sqlx::FromRow)]
struct PageRow {
    work_id: Uuid,
    generation: i64,
    saved_at: OffsetDateTime,
    cursor_key: Uuid,
    kind: String,
    revision_id: Option<Uuid>,
    title: Option<String>,
    form: Option<String>,
    display_name: Option<String>,
    handle: Option<String>,
}

impl PageRow {
    fn project(self, principal: PrincipalId) -> Result<ListItem, StoreError> {
        let work = WorkId::from_uuid(self.work_id);
        let membership = restore(
            SavedWorkKey { principal, work },
            self.generation,
            Some(self.saved_at),
        )?;
        let access = match self.kind.as_str() {
            "readable" => SavedWorkAccess::Public {
                work,
                revision: RevisionId::from_uuid(self.revision_id.ok_or_else(invalid_row)?),
            },
            "hidden" => SavedWorkAccess::Hidden,
            "author_deleted" => SavedWorkAccess::AuthorDeleted,
            _ => return Err(invalid_row()),
        };
        let row = membership
            .project(principal, Standing::Active, access)
            .ok_or_else(invalid_row)?;
        Ok(match row {
            SavedRow::Readable {
                work,
                generation,
                saved_at,
                ..
            } => ListItem::Readable {
                work_id: work.as_uuid(),
                generation: generation.get(),
                saved_at,
                title: self.title.ok_or_else(invalid_row)?,
                form: self.form,
                author: AuthorAttribution::project(
                    self.display_name.ok_or_else(invalid_row)?,
                    self.handle,
                    false,
                ),
            },
            SavedRow::Hidden { work, generation } => ListItem::Hidden {
                work_id: work.as_uuid(),
                generation: generation.get(),
            },
            SavedRow::AuthorDeleted { work, generation } => ListItem::AuthorDeleted {
                work_id: work.as_uuid(),
                generation: generation.get(),
            },
        })
    }
}

impl SavedWorksStore for PgStore {
    async fn membership(&self, key: SavedWorkKey) -> Result<SavedWork, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        require_live_owner(&mut tx, key.principal).await?;
        let state = load(&mut tx, key).await?;
        tx.commit().await.map_err(classify)?;
        Ok(state)
    }

    async fn set_membership(&self, command: SetSaved) -> Result<SavedWork, SavedWorksError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let access = command_access(&mut tx, command).await?;
        let pair = load(&mut tx, command.key).await?;
        let adding = command.desired == SaveIntent::Saved && pair.saved_at().is_none();
        let (count, now): (i64, OffsetDateTime) = sqlx::query_as(
            "SELECT CASE WHEN $2 THEN (SELECT count(*) FROM saved_works
                        WHERE principal_id = $1 AND saved_at IS NOT NULL) ELSE 0 END, clock_timestamp()",
        )
        .bind(command.key.principal.as_uuid())
        .bind(adding)
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        let count = usize::try_from(count).map_err(|_| invalid_row())?;
        let outcome = pair
            .decide(command, Standing::Active, access, count, now)
            .map_err(SavedWorksError::Decision)?;
        let state = match outcome {
            SavedChange::Applied(next) | SavedChange::Reaffirmed(next) => {
                sqlx::query(
                    "INSERT INTO saved_works (principal_id, work_id, generation, saved_at)
                    VALUES ($1,$2,$3,$4) ON CONFLICT (principal_id, work_id) DO UPDATE
                    SET generation = EXCLUDED.generation, saved_at = EXCLUDED.saved_at",
                )
                .bind(command.key.principal.as_uuid())
                .bind(command.key.work.as_uuid())
                .bind(next.generation().get())
                .bind(next.saved_at())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
                next
            }
            SavedChange::AlreadyCurrent(now) => now,
        };
        tx.commit().await.map_err(classify)?;
        Ok(state)
    }

    async fn list_memberships(
        &self,
        owner: PrincipalId,
        cursor: Option<&str>,
    ) -> Result<SavedPage, SavedWorksError> {
        let mut tx = read_snapshot(self.pool()).await?;
        require_live_owner(&mut tx, owner).await?;
        let after = if let Some(raw) = cursor {
            let (anchor, tag) = Anchor::parse(raw).ok_or(SavedWorksError::InvalidCursor)?;
            let key: Option<Uuid> = sqlx::query_scalar(
                "SELECT cursor_key FROM saved_works WHERE principal_id = $1 AND work_id = $2",
            )
            .bind(owner.as_uuid())
            .bind(anchor.work)
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
            if !key.is_some_and(|key| anchor.verifies(owner, key, tag)) {
                return Err(SavedWorksError::InvalidCursor);
            }
            Some(anchor)
        } else {
            None
        };
        let mut rows: Vec<PageRow> = sqlx::query_as(include_str!("saved_works/list.sql"))
            .bind(owner.as_uuid())
            .bind(after.as_ref().map(|a| a.at))
            .bind(after.as_ref().map(|a| a.work))
            .bind((PAGE_SIZE + 1) as i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let next_cursor = if rows.len() > PAGE_SIZE {
            rows.truncate(PAGE_SIZE);
            rows.last().map(|last| {
                Anchor {
                    work: last.work_id,
                    at: last.saved_at,
                }
                .encode(owner, last.cursor_key)
            })
        } else {
            None
        };
        let items = rows
            .into_iter()
            .map(|row| row.project(owner))
            .collect::<Result<_, _>>()?;
        Ok(SavedPage { items, next_cursor })
    }
}
