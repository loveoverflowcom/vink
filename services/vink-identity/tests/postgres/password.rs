//! Native password credentials on real PostgreSQL: registration, duplicate rejection,
//! password hashing, sign-in verification, throttling, account disable and HTTP routes.

use std::sync::Arc;

use argon2::{Argon2, PasswordHasher};
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;
use vink_identity::accounts::{
    AccountStore, PasswordRegistrationError, PasswordSignInRejection, disable_account,
};
use vink_identity::authority::Authority;
use vink_identity::contract::CredentialRejection;
use vink_identity::email_code::Policy;
use vink_identity::http::{IdentityState, Peer, router};
use vink_identity::password::{Password, Username};

const CALLER: u32 = 501;

fn store(pool: &PgPool) -> AccountStore {
    AccountStore::new(pool.clone(), Policy::default(), Arc::new(Notify::new()))
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn registration_and_sign_in_round_trip(pool: PgPool) {
    let store = store(&pool);
    let username = Username::parse("tac_gia_1").unwrap();
    let password = Password::parse("mat-khau-an-toan-123").unwrap();

    let session = store
        .register_password(&username, &password)
        .await
        .unwrap()
        .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM account_passwords WHERE account_id = $1")
            .bind(session.account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);

    // The credential verifies to the new account.
    let verified = store
        .verify(&session.credential.encode())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verified.account_id, session.account_id);

    // Password is stored hashed with Argon2id, never plaintext.
    let row: (String, i32) = sqlx::query_as(
        "SELECT password_hash, version FROM account_passwords WHERE account_id = $1",
    )
    .bind(session.account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.0.starts_with("$argon2id$"));
    assert!(!row.0.contains("mat-khau-an-toan-123"));
    assert_eq!(row.1, 1);

    // Sign in with the right password produces a working session for the same account.
    let signed_in = store
        .sign_in_password(&username, password.expose())
        .await
        .unwrap()
        .expect("unexpected rejection");
    assert_eq!(signed_in.account_id, session.account_id);

    let verified2 = store
        .verify(&signed_in.credential.encode())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verified2.account_id, session.account_id);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn old_raw_hashes_can_still_sign_in_to_the_same_account(pool: PgPool) {
    let store = store(&pool);
    for (name, raw) in [
        ("old_short", "ệệệệ"),
        ("old_noncanonical", "１２３４５６７８９０"),
    ] {
        // Reproduce the stored representation used before O-12@r2, with no normalization.
        let account_id = Uuid::now_v7();
        let hash = Argon2::default()
            .hash_password(raw.as_bytes())
            .unwrap()
            .to_string();
        sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
            .bind(account_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO account_passwords (account_id, username, password_hash) VALUES ($1, $2, $3)")
            .bind(account_id)
            .bind(name)
            .bind(hash)
            .execute(&pool)
            .await
            .unwrap();
        let username = Username::parse(name).unwrap();
        let session = store
            .sign_in_password(&username, raw)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session.account_id, account_id);
        assert!(matches!(
            store
                .sign_in_password(&username, "sai-mat-khau")
                .await
                .unwrap(),
            Err(PasswordSignInRejection::InvalidCredentials)
        ));
    }
    assert!(
        Password::parse("ệệệệ").is_err(),
        "new credentials use O-12@r2"
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn duplicate_username_registration_is_rejected(pool: PgPool) {
    let store = store(&pool);
    let username = Username::parse("but_danh").unwrap();
    let password = Password::parse("mat-khau-rat-manh-1").unwrap();

    store
        .register_password(&username, &password)
        .await
        .unwrap()
        .unwrap();

    // Re-registering the same username is rejected.
    let err = store
        .register_password(&username, &password)
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(err, PasswordRegistrationError::UsernameTaken);

    // Case-insensitivity: parsed username "BUT_DANH" normalizes to "but_danh" and collides.
    let upper = Username::parse("BUT_DANH").unwrap();
    let err2 = store
        .register_password(&upper, &password)
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(err2, PasswordRegistrationError::UsernameTaken);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn wrong_password_and_unknown_username_are_rejected(pool: PgPool) {
    let store = store(&pool);
    let username = Username::parse("nguoi_dung").unwrap();
    let password = Password::parse("mat-khau-chuan-123").unwrap();
    store
        .register_password(&username, &password)
        .await
        .unwrap()
        .unwrap();

    let wrong_pass = Password::parse("mat-khau-sai-hoan-toan").unwrap();
    let outcome = store
        .sign_in_password(&username, wrong_pass.expose())
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        Err(PasswordSignInRejection::InvalidCredentials)
    ));

    let unknown_user = Username::parse("chua_tung_co").unwrap();
    let outcome2 = store
        .sign_in_password(&unknown_user, password.expose())
        .await
        .unwrap();
    assert!(matches!(
        outcome2,
        Err(PasswordSignInRejection::InvalidCredentials)
    ));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn password_sign_in_throttling_after_repeated_failures(pool: PgPool) {
    let store = store(&pool);
    let username = Username::parse("muc_tieu_tan_cong").unwrap();
    let password = Password::parse("mat-khau-chinh-xac").unwrap();
    store
        .register_password(&username, &password)
        .await
        .unwrap()
        .unwrap();

    let wrong_pass = Password::parse("mat-khau-doan-sai").unwrap();
    for _ in 0..10 {
        let outcome = store
            .sign_in_password(&username, wrong_pass.expose())
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            Err(PasswordSignInRejection::InvalidCredentials)
        ));
    }

    // 11th attempt is throttled.
    let outcome11 = store
        .sign_in_password(&username, wrong_pass.expose())
        .await
        .unwrap();
    assert!(matches!(
        outcome11,
        Err(PasswordSignInRejection::RateLimited)
    ));

    // Even with correct password, sign in is throttled until window clears.
    let outcome_correct = store
        .sign_in_password(&username, password.expose())
        .await
        .unwrap();
    assert!(matches!(
        outcome_correct,
        Err(PasswordSignInRejection::RateLimited)
    ));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn disabled_account_cannot_sign_in_with_password_and_active_session_is_revoked(pool: PgPool) {
    let store = store(&pool);
    let username = Username::parse("bi_khoa").unwrap();
    let password = Password::parse("mat-khau-chuan-123").unwrap();
    let session = store
        .register_password(&username, &password)
        .await
        .unwrap()
        .unwrap();

    // Disable the account.
    disable_account(&pool, session.account_id).await.unwrap();

    // Existing session verification is rejected as disabled.
    let verify_result = store.verify(&session.credential.encode()).await.unwrap();
    assert_eq!(verify_result, Err(CredentialRejection::AccountDisabled));

    // Sign in attempt returns AccountDisabled.
    let outcome = store
        .sign_in_password(&username, password.expose())
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        Err(PasswordSignInRejection::AccountDisabled)
    ));
    assert!(matches!(
        store
            .sign_in_password(&username, "sai-mat-khau")
            .await
            .unwrap(),
        Err(PasswordSignInRejection::InvalidCredentials)
    ));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn password_routes_over_http(pool: PgPool) {
    let store = store(&pool);
    let authority = Authority::new_password(store);
    let app = router(IdentityState::new(pool, authority, Some(CALLER)))
        .layer(axum::Extension(ConnectInfo(Peer { uid: Some(CALLER) })));

    // Capabilities route reflects password enabled.
    let response = app
        .clone()
        .oneshot(
            Request::get("/identity/v1/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let caps: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(caps["operations"]["register_password"], "available");
    assert_eq!(caps["operations"]["sign_in_password"], "available");
    assert_eq!(caps["operations"]["sign_in"], "not_configured");

    // Register via HTTP.
    let reg_req = serde_json::json!({
        "username": "nha_van_moi",
        "password": "mat-khau-hop-le-100"
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/identity/v1/password/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&reg_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let reg_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reg_body["outcome"], "authenticated");
    assert_eq!(reg_body["subject"]["authority"], "vink-identity");
    let cred = reg_body["session"]["credential"].as_str().unwrap();

    // Verify session.
    let verify_req = serde_json::json!({ "credential": cred });
    let response = app
        .clone()
        .oneshot(
            Request::post("/identity/v1/verify-credential")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&verify_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let verify_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(verify_body["outcome"], "authenticated");

    // Sign in via HTTP.
    let sign_in_req = serde_json::json!({
        "username": "nha_van_moi",
        "password": "mat-khau-hop-le-100"
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/identity/v1/password/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&sign_in_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let sign_in_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(sign_in_body["outcome"], "authenticated");

    // Invalid password via HTTP answers 200 with outcome rejected and reason invalid_credentials.
    let bad_sign_in = serde_json::json!({
        "username": "nha_van_moi",
        "password": "mat-khau-hoan-toan-sai"
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/identity/v1/password/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&bad_sign_in).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let err_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_body["outcome"], "rejected");
    assert_eq!(err_body["reason"], "invalid_credentials");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_rollback_script_drops_password_tables(pool: PgPool) {
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261113000000_password_credentials.down.sql"),
    )
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(script))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let pw_table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('account_passwords')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pw_table, None);
    let attempts_table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('password_login_attempts')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts_table, None);
}
