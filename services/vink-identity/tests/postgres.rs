//! Identity on real PostgreSQL. Each test gets a fresh database (`#[sqlx::test]`) from the test
//! harness credential that `cargo xtask verify postgres` passes as `DATABASE_URL`. Role
//! separation and the real Unix socket are exercised by the lane's process checks.

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use vink_identity::authority::Authority;
use vink_identity::http::{IdentityState, Peer, router};
use vink_identity::persistence::{SchemaState, migrate, schema_state};

const CALLER: u32 = 501;

#[path = "postgres/delivery.rs"]
mod delivery;
#[path = "postgres/email_code.rs"]
mod email_code;
#[path = "postgres/enrollment.rs"]
mod enrollment;
#[path = "postgres/password.rs"]
mod password;
#[path = "support/smtp_sink.rs"]
mod smtp_sink;

fn app(pool: PgPool, caller: Option<u32>) -> Router {
    router(IdentityState::new(pool, Authority::NotConfigured, caller))
        .layer(axum::Extension(ConnectInfo(Peer { uid: Some(CALLER) })))
}

async fn ready(router: &Router) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[sqlx::test(migrations = false)]
async fn serve_is_not_ready_until_identity_migrates_and_migrating_twice_is_a_no_op(pool: PgPool) {
    let router = app(pool.clone(), Some(CALLER));
    let (status, body) = ready(&router).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["database"], "ok");
    assert_eq!(body["checks"]["schema"], "pending_migrations");

    let first = migrate(&pool).await.unwrap();
    assert_eq!(
        first.applied,
        vec![
            20261102000000,
            20261107000000,
            20261112000000,
            20261113000000,
            20261114000000
        ]
    );
    assert!(migrate(&pool).await.unwrap().applied.is_empty());

    // The schema is now current, but this pool is the test harness superuser: serving with a
    // privileged role is refused. Ready (200) with the provisioned runtime role is checked by
    // `verify postgres` against the real binary.
    let (status, body) = ready(&router).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["checks"]["schema"], "current");
    assert_eq!(body["checks"]["role"], "privileged");
    assert_eq!(body["authority"], "not_configured");
    assert_eq!(body["production_authentication"], "not_implemented");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_current_schema_without_a_caller_is_still_not_ready(pool: PgPool) {
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);
    let (status, body) = ready(&app(pool, None)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["schema"], "current");
    assert_eq!(body["checks"]["callers"], "not_configured");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn identity_owns_only_credential_and_session_tables(pool: PgPool) {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    // No principal, permission, work or draft: those are the App's.
    assert_eq!(
        tables,
        [
            "_sqlx_migrations",
            "account_emails",
            "account_passwords",
            "accounts",
            "email_challenges",
            "mail_deliveries",
            "password_login_attempts",
            "sessions"
        ]
    );
}

#[sqlx::test(migrations = false)]
async fn an_app_history_in_the_database_is_diverged_not_current(pool: PgPool) {
    // As if Identity were pointed at the App database: a version Identity does not embed.
    sqlx::query(
        "CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL,
             installed_on TIMESTAMPTZ NOT NULL DEFAULT now(), success BOOLEAN NOT NULL,
             checksum BYTEA NOT NULL, execution_time BIGINT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (20261030000000, 'literary core', true, '\\x00', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = ready(&app(pool.clone(), Some(CALLER))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["schema"], "diverged");
    assert!(
        migrate(&pool).await.is_err(),
        "a foreign history is never migrated"
    );
}
