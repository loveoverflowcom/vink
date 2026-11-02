//! Migration lifecycle around a consumer-supplied `sqlx` [`Migrator`].
//!
//! Serving never migrates: a server compares the history with the migrations embedded in its
//! binary ([`schema_state`]) and treats anything but [`SchemaState::Current`] as not ready.
//! Only [`migrate`], run with the owner's migration credential, writes the history.

use std::fmt;

use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::PgPool;

use crate::is_unavailable;

/// sqlx's history table, in schema `public`. A fixed infrastructure convention rather than a
/// service identity: renaming it would orphan every history already applied.
pub const HISTORY_TABLE: &str = "_sqlx_migrations";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaState {
    /// Every embedded migration is applied with an identical checksum, and nothing else is.
    Current,
    /// The database is reachable but some embedded migration is not applied yet.
    Pending,
    /// This binary cannot continue the history; neither serving nor migrating proceeds.
    Diverged(Divergence),
}

impl SchemaState {
    /// Stable label for readiness responses and logs.
    pub fn label(self) -> &'static str {
        match self {
            SchemaState::Current => "current",
            SchemaState::Pending => "pending_migrations",
            SchemaState::Diverged(_) => "diverged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divergence {
    /// A migration started but did not finish (`success = false`).
    Failed { version: i64 },
    /// An applied migration differs from the embedded one with the same version.
    ChecksumMismatch { version: i64 },
    /// An applied version this binary does not embed, for example from a newer build.
    Unknown { version: i64 },
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Divergence::Failed { version } => write!(f, "migration {version} did not finish"),
            Divergence::ChecksumMismatch { version } => write!(
                f,
                "applied migration {version} differs from the one embedded in this binary"
            ),
            Divergence::Unknown { version } => write!(
                f,
                "applied migration {version} is unknown to this binary (applied by a newer build?)"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Versions this run applied, in order; empty when the schema was already current.
    pub applied: Vec<i64>,
    /// Highest embedded version, which is now applied.
    pub latest: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("database unavailable: {0}")]
    Unavailable(String),
    #[error("schema history diverged: {0}; nothing was applied")]
    Diverged(Divergence),
    /// SQLSTATE 42501. The caller must not retry with a more privileged credential.
    #[error(
        "the migration credential lacks a privilege ({0}); provision it as owner of the schema objects instead of using a superuser"
    )]
    Privilege(String),
    #[error("{0}")]
    Failed(String),
}

impl From<sqlx::Error> for MigrationError {
    fn from(error: sqlx::Error) -> Self {
        let privilege =
            matches!(&error, sqlx::Error::Database(db) if db.code().as_deref() == Some("42501"));
        if is_unavailable(&error) {
            MigrationError::Unavailable(error.to_string())
        } else if privilege {
            MigrationError::Privilege(error.to_string())
        } else {
            MigrationError::Failed(error.to_string())
        }
    }
}

struct Applied {
    version: i64,
    checksum: Vec<u8>,
    success: bool,
}

fn embedded(migrator: &Migrator) -> Vec<(i64, &[u8])> {
    migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .map(|m| (m.version, m.checksum.as_ref()))
        .collect()
}

/// Pure comparison of an applied history with the embedded migrations.
fn compare(embedded: &[(i64, &[u8])], applied: &[Applied]) -> SchemaState {
    for row in applied {
        if !row.success {
            return SchemaState::Diverged(Divergence::Failed {
                version: row.version,
            });
        }
        match embedded.iter().find(|(version, _)| *version == row.version) {
            None => {
                return SchemaState::Diverged(Divergence::Unknown {
                    version: row.version,
                });
            }
            Some((_, checksum)) if *checksum != row.checksum.as_slice() => {
                return SchemaState::Diverged(Divergence::ChecksumMismatch {
                    version: row.version,
                });
            }
            Some(_) => {}
        }
    }
    let complete = embedded
        .iter()
        .all(|(version, _)| applied.iter().any(|row| row.version == *version));
    if complete {
        SchemaState::Current
    } else {
        SchemaState::Pending
    }
}

/// The applied history, or `None` before the first migration created the table.
async fn history(pool: &PgPool) -> Result<Option<Vec<Applied>>, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(format!("public.{HISTORY_TABLE}"))
        .fetch_one(pool)
        .await?;
    if !exists {
        return Ok(None);
    }
    let rows: Vec<(i64, Vec<u8>, bool)> = sqlx::query_as(
        "SELECT version, checksum, success FROM public._sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    Ok(Some(
        rows.into_iter()
            .map(|(version, checksum, success)| Applied {
                version,
                checksum,
                success,
            })
            .collect(),
    ))
}

/// Compares the database's history with the migrations embedded in `migrator`. Read-only; the
/// runtime credential needs only `SELECT` on the history table.
pub async fn schema_state(pool: &PgPool, migrator: &Migrator) -> Result<SchemaState, sqlx::Error> {
    let applied = history(pool).await?.unwrap_or_default();
    Ok(compare(&embedded(migrator), &applied))
}

/// Applies pending migrations with the credential behind `pool`, then leaves write access to the
/// history with its owner alone. A diverged history is refused before anything runs, and a
/// missing privilege is reported as [`MigrationError::Privilege`], never retried as another role.
pub async fn migrate(
    pool: &PgPool,
    migrator: &Migrator,
) -> Result<MigrationReport, MigrationError> {
    let embedded = embedded(migrator);
    let before = history(pool).await?.unwrap_or_default();
    if let SchemaState::Diverged(divergence) = compare(&embedded, &before) {
        return Err(MigrationError::Diverged(divergence));
    }
    migrator.run(pool).await.map_err(|error| match error {
        MigrateError::Execute(error) => MigrationError::from(error),
        MigrateError::ExecuteMigration(error, version) => match MigrationError::from(error) {
            MigrationError::Failed(detail) => {
                MigrationError::Failed(format!("migration {version} failed: {detail}"))
            }
            other => other,
        },
        MigrateError::Dirty(version) => MigrationError::Diverged(Divergence::Failed { version }),
        MigrateError::VersionMismatch(version) => {
            MigrationError::Diverged(Divergence::ChecksumMismatch { version })
        }
        MigrateError::VersionMissing(version) => {
            MigrationError::Diverged(Divergence::Unknown { version })
        }
        other => MigrationError::Failed(other.to_string()),
    })?;
    restrict_history_writes(pool).await?;
    let after = history(pool).await?.unwrap_or_default();
    let state = compare(&embedded, &after);
    if state != SchemaState::Current {
        return Err(MigrationError::Failed(format!(
            "schema is {} after migrating",
            state.label()
        )));
    }
    Ok(MigrationReport {
        applied: after
            .iter()
            .map(|row| row.version)
            .filter(|version| !before.iter().any(|row| row.version == *version))
            .collect(),
        latest: embedded.iter().map(|(version, _)| *version).max(),
    })
}

/// Revokes every non-owner write grant on the history table. Default privileges that give a
/// runtime role DML on the owner's new tables also cover the history table sqlx creates.
async fn restrict_history_writes(pool: &PgPool) -> Result<(), sqlx::Error> {
    let statements: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT format('REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON TABLE %s FROM %s',
                                c.oid::regclass,
                                CASE WHEN a.grantee = 0 THEN 'PUBLIC' ELSE quote_ident(r.rolname) END)
           FROM pg_class c
           CROSS JOIN LATERAL aclexplode(c.relacl) a
           LEFT JOIN pg_roles r ON r.oid = a.grantee
          WHERE c.oid = to_regclass('public._sqlx_migrations')
            AND a.grantee <> c.relowner
            AND a.privilege_type IN ('INSERT', 'UPDATE', 'DELETE', 'TRUNCATE')",
    )
    .fetch_all(pool)
    .await?;
    for statement in statements {
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(version: i64, checksum: &[u8], success: bool) -> Applied {
        Applied {
            version,
            checksum: checksum.to_vec(),
            success,
        }
    }

    const EMBEDDED: [(i64, &[u8]); 2] = [(1, b"one"), (2, b"two")];

    #[test]
    fn empty_or_partial_history_is_pending() {
        assert_eq!(compare(&EMBEDDED, &[]), SchemaState::Pending);
        assert_eq!(
            compare(&EMBEDDED, &[row(1, b"one", true)]),
            SchemaState::Pending
        );
        assert_eq!(compare(&[], &[]), SchemaState::Current);
    }

    #[test]
    fn identical_history_is_current() {
        assert_eq!(
            compare(&EMBEDDED, &[row(1, b"one", true), row(2, b"two", true)]),
            SchemaState::Current
        );
    }

    #[test]
    fn foreign_history_is_diverged_even_when_everything_embedded_is_applied() {
        let newer = [
            row(1, b"one", true),
            row(2, b"two", true),
            row(3, b"three", true),
        ];
        assert_eq!(
            compare(&EMBEDDED, &newer),
            SchemaState::Diverged(Divergence::Unknown { version: 3 })
        );
        assert_eq!(
            compare(&EMBEDDED, &[row(1, b"one", true), row(2, b"edited", true)]),
            SchemaState::Diverged(Divergence::ChecksumMismatch { version: 2 })
        );
        assert_eq!(
            compare(&EMBEDDED, &[row(1, b"one", true), row(2, b"two", false)]),
            SchemaState::Diverged(Divergence::Failed { version: 2 })
        );
        assert_eq!(
            SchemaState::Diverged(Divergence::Failed { version: 2 }).label(),
            "diverged"
        );
    }
}
