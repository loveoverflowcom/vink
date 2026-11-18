//! Lock order: principal standing fence, profile, then new namespace key. No key is
//! transferred or released. Receipts and pointers commit together.
use super::{PgStore, postgres::classify, principal::require_active};
use crate::application::handle::*;
use crate::application::ports::StoreError;
use crate::domain::handle::{self, ChangeError};
use crate::domain::ids::PrincipalId;
use crate::domain::principal::PenName;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

type Tx<'a> = Transaction<'a, Postgres>;

async fn reserve(tx: &mut Tx<'_>, principal: Uuid, handle: &str) -> Result<bool, StoreError> {
    Ok(sqlx::query("INSERT INTO author_handle_names(handle, principal_id) VALUES ($1,$2) ON CONFLICT (handle) DO NOTHING")
        .bind(handle).bind(principal).execute(&mut **tx).await.map_err(classify)?.rows_affected() == 1)
}

pub(super) async fn provision(
    tx: &mut Tx<'_>,
    principal: Uuid,
    pen: &PenName,
) -> Result<usize, StoreError> {
    allocate(
        tx,
        principal,
        (0..16).map(|_| {
            let random = Uuid::new_v4();
            handle::candidate(pen, random.as_u128() as u32)
        }),
    )
    .await
}

async fn allocate(
    tx: &mut Tx<'_>,
    principal: Uuid,
    candidates: impl Iterator<Item = String>,
) -> Result<usize, StoreError> {
    for (attempt, name) in candidates.enumerate() {
        if reserve(tx, principal, &name).await? {
            sqlx::query("INSERT INTO author_profiles(principal_id,current_handle) VALUES ($1,$2)")
                .bind(principal)
                .bind(name)
                .execute(&mut **tx)
                .await
                .map_err(classify)?;
            return Ok(attempt + 1);
        }
    }
    Err(StoreError::Unavailable(
        "author name generation exhausted".into(),
    ))
}

#[derive(Debug, Default)]
pub struct BackfillReport {
    pub created: usize,
    pub attempts: usize,
    pub existing: usize,
    pub invalid: usize,
    pub unbound: i64,
}

/// Bounded transactions. Committed profiles themselves are the durable checkpoint;
/// restarting from the beginning also rechecks legacy names that needed remediation.
pub async fn backfill(pool: &PgPool) -> Result<BackfillReport, StoreError> {
    let mut report = BackfillReport::default();
    let mut after: Option<Uuid> = None;
    loop {
        let mut tx = pool.begin().await.map_err(classify)?;
        let batch: Vec<(Uuid,String)> = sqlx::query_as("SELECT p.id,p.display_name FROM principals p JOIN principal_bindings b ON b.principal_id=p.id
            WHERE ($1::uuid IS NULL OR p.id > $1) ORDER BY p.id LIMIT 100 FOR UPDATE OF p")
            .bind(after).fetch_all(&mut *tx).await.map_err(classify)?;
        if batch.is_empty() {
            tx.commit().await.map_err(classify)?;
            break;
        }
        for (principal, display) in batch {
            after = Some(principal);
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT FROM author_profiles WHERE principal_id=$1)",
            )
            .bind(principal)
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
            if exists {
                report.existing += 1;
                continue;
            }
            let Some(pen) = PenName::parse(&display) else {
                continue;
            };
            if pen.as_str() != display {
                sqlx::query("UPDATE principals SET display_name=$2 WHERE id=$1")
                    .bind(principal)
                    .bind(pen.as_str())
                    .execute(&mut *tx)
                    .await
                    .map_err(classify)?;
            }
            report.attempts += provision(&mut tx, principal, &pen).await?;
            report.created += 1;
        }
        tx.commit().await.map_err(classify)?;
    }
    report.unbound = sqlx::query_scalar("SELECT count(*) FROM principals p WHERE NOT EXISTS (SELECT FROM principal_bindings b WHERE b.principal_id=p.id)")
        .fetch_one(pool).await.map_err(classify)?;
    // Serialize the final inventory with the deferred trigger of old binding writers.
    // Rows still missing a profile must actually be invalid according to the same parser.
    let mut tx = pool.begin().await.map_err(classify)?;
    sqlx::query("SELECT singleton FROM author_handle_rollout WHERE singleton FOR UPDATE")
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    let mut cursor = None::<Uuid>;
    let mut invalid = 0;
    loop {
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT b.principal_id, p.display_name FROM principal_bindings b
             JOIN principals p ON p.id = b.principal_id
             LEFT JOIN author_profiles s USING (principal_id)
             WHERE s.principal_id IS NULL AND ($1::uuid IS NULL OR b.principal_id > $1)
             ORDER BY b.principal_id LIMIT 100",
        )
        .bind(cursor)
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        if rows.is_empty() {
            break;
        }
        for (id, name) in rows {
            if PenName::parse(&name).is_some() {
                return Err(StoreError::Unavailable(
                    "binding inventory moved; repeat backfill".into(),
                ));
            }
            invalid += 1;
            cursor = Some(id);
        }
    }
    report.invalid = invalid;
    sqlx::query("UPDATE author_handle_rollout SET ready=true WHERE singleton AND NOT ready")
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    tx.commit().await.map_err(classify)?;
    Ok(report)
}

async fn ready(pool: &PgPool) -> Result<(), StoreError> {
    if sqlx::query_scalar::<_, bool>("SELECT ready FROM author_handle_rollout WHERE singleton")
        .fetch_one(pool)
        .await
        .map_err(classify)?
    {
        Ok(())
    } else {
        Err(StoreError::Unavailable(
            "author profile backfill pending".into(),
        ))
    }
}

async fn own(tx: &mut Tx<'_>, principal: Uuid) -> Result<OwnHandle, HandleError> {
    let (pen_name,handle,setup_complete,remaining_changes,version,aliases,has_public_works) =
        sqlx::query_as("SELECT p.display_name,s.current_handle,s.setup_complete,s.remaining_changes,s.version,
          ARRAY(SELECT n.handle FROM author_handle_names n WHERE n.principal_id=s.principal_id AND n.handle<>s.current_handle ORDER BY n.handle),
          EXISTS(SELECT FROM publications pub JOIN works w ON w.id=pub.work_id WHERE w.owner_id=p.id)
          FROM author_profiles s JOIN principals p ON p.id=s.principal_id WHERE s.principal_id=$1 AND s.current_handle IS NOT NULL")
        .bind(principal).fetch_optional(&mut **tx).await.map_err(classify)?.ok_or(HandleError::Missing)?;
    Ok(OwnHandle {
        principal_id: principal,
        pen_name,
        handle,
        setup_complete,
        remaining_changes,
        version,
        aliases,
        has_public_works,
    })
}

impl PgStore {
    pub async fn own_handle(&self, actor: PrincipalId) -> Result<OwnHandle, HandleError> {
        ready(self.pool()).await?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_active(&mut tx, actor).await?;
        let result = own(&mut tx, actor.as_uuid()).await?;
        tx.commit().await.map_err(classify)?;
        Ok(result)
    }

    pub async fn set_handle(
        &self,
        actor: PrincipalId,
        command: &HandleCommand,
    ) -> Result<HandleReceipt, HandleError> {
        let id = actor.as_uuid();
        if id != command.principal_id {
            return Err(HandleError::WrongOwner);
        }
        if !handle::valid(&command.handle) || command.expected_version < 0 {
            return Err(HandleError::Invalid);
        }
        ready(self.pool()).await?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_active(&mut tx, actor).await?;
        let state = own(&mut tx, id).await?;
        let prior: Option<(String, i64, String, i64, i16)> = sqlx::query_as(
            "SELECT intent,expected_version,requested_handle,accepted_version,remaining_changes
            FROM author_handle_receipts WHERE principal_id=$1 AND command_id=$2",
        )
        .bind(id)
        .bind(command.command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if let Some((intent, expected, name, accepted_version, remaining_changes)) = prior {
            if intent != command.intent.key()
                || expected != command.expected_version
                || name != command.handle
            {
                return Err(HandleError::KeyReused);
            }
            return Ok(HandleReceipt {
                command_id: command.command_id,
                accepted_version,
                handle: name,
                remaining_changes,
            });
        }
        if state.version != command.expected_version {
            return Err(HandleError::Stale(state.version));
        }
        if state.aliases.contains(&command.handle) {
            return Err(HandleError::OwnAlias);
        }
        let same = state.handle == command.handle;
        let changes = handle::change(
            state.setup_complete,
            state.remaining_changes,
            command.intent,
            same,
        )
        .map_err(|err| match err {
            ChangeError::WrongIntent => HandleError::Stale(state.version),
            ChangeError::Exhausted => HandleError::Exhausted,
        })?;
        let mut accepted_version = state.version;
        let remaining_changes = changes.unwrap_or(state.remaining_changes);
        if changes.is_some() {
            if !same && !reserve(&mut tx, id, &command.handle).await? {
                return Err(HandleError::Taken);
            }
            accepted_version = sqlx::query_scalar("UPDATE author_profiles SET current_handle=$2,setup_complete=true,remaining_changes=$3,version=version+1 WHERE principal_id=$1 RETURNING version")
                .bind(id).bind(&command.handle).bind(remaining_changes).fetch_one(&mut *tx).await.map_err(classify)?;
        }
        sqlx::query("INSERT INTO author_handle_receipts(principal_id,command_id,intent,expected_version,requested_handle,accepted_version,remaining_changes) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(id).bind(command.command_id).bind(command.intent.key()).bind(command.expected_version).bind(&command.handle).bind(accepted_version).bind(remaining_changes)
            .execute(&mut *tx).await.map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(HandleReceipt {
            command_id: command.command_id,
            accepted_version,
            handle: command.handle.clone(),
            remaining_changes,
        })
    }

    pub async fn handle_receipt(
        &self,
        actor: PrincipalId,
        key: Uuid,
    ) -> Result<HandleReceipt, HandleError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        require_active(&mut tx, actor).await?;
        let (accepted_version,handle,remaining_changes) = sqlx::query_as("SELECT r.accepted_version,r.requested_handle,r.remaining_changes FROM author_handle_receipts r JOIN author_profiles p USING(principal_id) WHERE r.principal_id=$1 AND r.command_id=$2 AND p.current_handle IS NOT NULL")
            .bind(actor.as_uuid()).bind(key).fetch_optional(&mut *tx).await.map_err(classify)?.ok_or(HandleError::Missing)?;
        Ok(HandleReceipt {
            command_id: key,
            accepted_version,
            handle,
            remaining_changes,
        })
    }

    pub async fn resolve_handle(&self, name: &str) -> Result<Resolution, StoreError> {
        if !handle::valid(name) {
            return Ok(Resolution::Missing);
        }
        ready(self.pool()).await?;
        let row: Option<(Option<String>,String,bool)> = sqlx::query_as("SELECT s.current_handle,p.display_name,
            EXISTS(SELECT FROM publications pub JOIN works w ON w.id=pub.work_id WHERE w.owner_id=p.id)
            FROM author_handle_names n JOIN author_profiles s USING(principal_id) JOIN principals p ON p.id=s.principal_id WHERE n.handle=$1")
            .bind(name).fetch_optional(self.pool()).await.map_err(classify)?;
        Ok(match row {
            None => Resolution::Missing,
            Some((None, _, _)) => Resolution::Gone,
            Some((Some(current), _, _)) if current != name => Resolution::Alias {
                href: format!("/@{current}"),
            },
            Some((Some(handle), pen_name, has_public_works)) => Resolution::Current {
                author: AuthorRef {
                    href: format!("/@{handle}"),
                    handle,
                    pen_name,
                },
                has_public_works,
            },
        })
    }
}

/// App-side hook for the account deletion owner (#297). Its caller verifies deletion
/// and owns the transaction. Retiring a suspended profile is also idempotent.
pub async fn retire(
    tx: &mut Transaction<'_, Postgres>,
    owner: PrincipalId,
) -> Result<(), StoreError> {
    super::principal::locked_standing(tx, owner).await?;
    sqlx::query("UPDATE author_profiles SET current_handle=NULL, version=version+1 WHERE principal_id=$1 AND current_handle IS NOT NULL")
        .bind(owner.as_uuid()).execute(&mut **tx).await.map_err(classify)?;
    Ok(())
}
