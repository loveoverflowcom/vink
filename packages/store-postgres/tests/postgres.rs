//! Lifecycle tests on real PostgreSQL. Each test gets a fresh, empty database (`#[sqlx::test]`),
//! so `DATABASE_URL` must be a test harness credential that may create databases; `cargo xtask
//! verify postgres` passes `VINK_TEST_DATABASE_URL`. The registry below is test-only.

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use vink_store_postgres::{
    Divergence, MigrationError, SchemaState, is_unavailable, migrate, role_is_privileged,
    schema_state,
};

static MIGRATOR: Migrator = sqlx::migrate!("./tests/migrations");

async fn history(pool: &PgPool) -> Vec<(i64, Vec<u8>, bool)> {
    sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn fresh_database_migrates_once_and_reruns_as_a_no_op(pool: PgPool) {
    assert_eq!(
        schema_state(&pool, &MIGRATOR).await.unwrap(),
        SchemaState::Pending
    );
    let report = migrate(&pool, &MIGRATOR).await.unwrap();
    assert_eq!(report.applied, vec![1, 2]);
    assert_eq!(report.latest, Some(2));
    assert_eq!(
        schema_state(&pool, &MIGRATOR).await.unwrap(),
        SchemaState::Current
    );
    let first = history(&pool).await;

    let rerun = migrate(&pool, &MIGRATOR).await.unwrap();
    assert!(rerun.applied.is_empty(), "{rerun:?}");
    assert_eq!(history(&pool).await, first);
}

#[sqlx::test(migrations = false)]
async fn partial_history_is_pending_and_completes(pool: PgPool) {
    MIGRATOR.run_to(1, &pool).await.unwrap();
    assert_eq!(
        schema_state(&pool, &MIGRATOR).await.unwrap(),
        SchemaState::Pending
    );
    let report = migrate(&pool, &MIGRATOR).await.unwrap();
    assert_eq!(report.applied, vec![2]);
}

#[sqlx::test(migrations = false)]
async fn only_the_history_owner_may_write_it(pool: PgPool) {
    migrate(&pool, &MIGRATOR).await.unwrap();
    // A broad grant, as default privileges for a runtime role would give.
    sqlx::query("GRANT INSERT, UPDATE, DELETE, TRUNCATE ON _sqlx_migrations TO PUBLIC")
        .execute(&pool)
        .await
        .unwrap();
    migrate(&pool, &MIGRATOR).await.unwrap();
    let writers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_class c CROSS JOIN LATERAL aclexplode(c.relacl) a
          WHERE c.oid = '_sqlx_migrations'::regclass AND a.grantee <> c.relowner
            AND a.privilege_type IN ('INSERT', 'UPDATE', 'DELETE', 'TRUNCATE')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(writers, 0);
}

#[sqlx::test(migrations = false)]
async fn a_diverged_history_is_reported_and_never_migrated(pool: PgPool) {
    migrate(&pool, &MIGRATOR).await.unwrap();
    let clean = history(&pool).await;

    sqlx::query("UPDATE _sqlx_migrations SET checksum = '\\x00' WHERE version = 2")
        .execute(&pool)
        .await
        .unwrap();
    let edited = history(&pool).await;
    let expected = Divergence::ChecksumMismatch { version: 2 };
    assert_eq!(
        schema_state(&pool, &MIGRATOR).await.unwrap(),
        SchemaState::Diverged(expected)
    );
    let error = migrate(&pool, &MIGRATOR).await.unwrap_err();
    assert!(
        matches!(error, MigrationError::Diverged(d) if d == expected),
        "{error}"
    );
    assert_eq!(history(&pool).await, edited, "a refused run writes nothing");

    sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = 2")
        .bind(&clean[1].1)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (3, 'from a newer build', true, '\\x01', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        schema_state(&pool, &MIGRATOR).await.unwrap(),
        SchemaState::Diverged(Divergence::Unknown { version: 3 })
    );

    sqlx::query("UPDATE _sqlx_migrations SET success = false WHERE version = 3")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        migrate(&pool, &MIGRATOR).await.unwrap_err(),
        MigrationError::Diverged(Divergence::Failed { version: 3 })
    ));
}

#[sqlx::test(migrations = false)]
async fn the_test_harness_superuser_is_privileged(pool: PgPool) {
    // Restricted runtime roles are checked with provisioned roles by `verify postgres`.
    assert!(role_is_privileged(&pool).await.unwrap());
}

#[sqlx::test(migrations = false)]
async fn a_terminated_session_is_unavailable_and_a_bad_statement_is_not(pool: PgPool) {
    let error = sqlx::query("SELECT 1 / 0")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(!is_unavailable(&error), "{error:?}");

    let mut connection = pool.acquire().await.unwrap();
    let error = sqlx::query("SELECT pg_terminate_backend(pg_backend_pid())")
        .execute(&mut *connection)
        .await
        .unwrap_err();
    assert!(is_unavailable(&error), "{error:?}");
}
