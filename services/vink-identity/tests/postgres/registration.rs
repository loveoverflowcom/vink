//! Registration proof and session ownership through the real Identity router, PostgreSQL and private local outbox.

use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;
use vink_identity::accounts::{AccountStore, StoreError};
use vink_identity::authority::{Authority, MethodCapabilities};
use vink_identity::delivery::{DeliveryPolicy, Worker};
use vink_identity::email_code::{ChallengeHandle, Code, EmailAddress, Policy};
use vink_identity::http::{IdentityState, Peer, router};
use vink_identity::mail::LocalOutbox;
use vink_identity::outcome::{ChallengeOutcome, NewSession, PasswordRegistrationError};
use vink_identity::password::{Password, Username};

const PASSWORD: &str = "mat-khau-dai-2026";
const EMAIL: &str = "ban-thao@example.test";
const PREFIX: &str = "/identity/v1/password/register";
const CALLER: u32 = 501;

struct Rig {
    store: AccountStore,
    http: Router,
    worker: Worker,
    outbox: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.outbox);
    }
}

impl Rig {
    fn new(pool: &PgPool) -> Self {
        let outbox =
            std::env::temp_dir().join(format!("vink-registration-outbox-{}", Uuid::now_v7()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&outbox)
            .unwrap();
        let wake = Arc::new(Notify::new());
        let policy = Policy {
            max_email_changes_per_window: 20,
            ..Policy::default()
        };
        let store = AccountStore::new(pool.clone(), policy, wake.clone());
        let authority = Authority::new_native(
            store.clone(),
            MethodCapabilities {
                email_code: true,
                password: true,
            },
        );
        let http = router(IdentityState::new(pool.clone(), authority, Some(CALLER)))
            .layer(axum::Extension(ConnectInfo(Peer { uid: Some(CALLER) })));
        let worker = Worker::new(
            pool.clone(),
            Arc::new(LocalOutbox::open(&outbox).unwrap()),
            DeliveryPolicy::default(),
            wake,
        );
        Self {
            store,
            http,
            worker,
            outbox,
        }
    }

    async fn rpc(&self, path: &str, payload: Value) -> (StatusCode, Value) {
        let response = self
            .http
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            if body.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&body).unwrap()
            },
        )
    }

    async fn code(&self, handle: &ChallengeHandle, purpose: &str) -> Code {
        self.worker.run_once().await.unwrap();
        let text = std::fs::read_to_string(self.outbox.join(format!("{}.txt", handle.id.simple())))
            .unwrap();
        assert!(
            text.lines()
                .any(|line| line == format!("X-Vink-Purpose: {purpose}"))
        );
        Code::parse(
            text.lines()
                .find_map(|line| line.strip_prefix("X-Vink-Code: "))
                .unwrap(),
        )
        .unwrap()
    }

    async fn sign_in_proof(&self, email: &str) -> (ChallengeHandle, Code) {
        let ChallengeOutcome::Issued { handle, .. } = self
            .store
            .start_sign_in(&EmailAddress::parse(email).unwrap())
            .await
            .unwrap()
        else {
            panic!("no issuance")
        };
        let code = self.code(&handle, "sign_in").await;
        (handle, code)
    }
}

pub(super) async fn proof(
    pool: &PgPool,
    store: &AccountStore,
    address: &EmailAddress,
) -> (ChallengeHandle, Code) {
    let rig = Rig::new(pool);
    let ChallengeOutcome::Issued { handle, .. } = store.start_registration(address).await.unwrap()
    else {
        panic!("expected issuance")
    };
    let code = rig.code(&handle, "register_password").await;
    (handle, code)
}

pub(super) async fn register(
    pool: &PgPool,
    store: &AccountStore,
    user: &Username,
    password: &Password,
) -> Result<Result<NewSession, PasswordRegistrationError>, StoreError> {
    let email = EmailAddress::parse(&format!("{}@example.test", Uuid::now_v7())).unwrap();
    let (handle, code) = proof(pool, store, &email).await;
    store
        .register_password(&email, &handle, &code, user, password)
        .await
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn registration_proof_is_bound_to_mailbox_purpose_and_one_commit(pool: PgPool) {
    let rig = Rig::new(&pool);
    let (status, _) = rig
        .rpc(PREFIX, json!({"username":"writer", "password":PASSWORD}))
        .await;
    assert_ne!(status, StatusCode::OK);
    let (status, body) = rig
        .rpc(&format!("{PREFIX}/challenges"), json!({"email": EMAIL}))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let handle = ChallengeHandle::parse(body["challenge"].as_str().unwrap()).unwrap();
    let code = rig.code(&handle, "register_password").await;
    let (signin, signin_code) = rig.sign_in_proof(EMAIL).await;
    let req = |email: &str, handle: &ChallengeHandle, code: &Code| {
        json!({
            "email":email, "challenge":handle.encode(), "code":code.expose(), "username":"writer", "password":PASSWORD
        })
    };
    let wrong = Code::parse(if code.expose() == "000000" {
        "999999"
    } else {
        "000000"
    })
    .unwrap();
    assert_eq!(
        rig.rpc(PREFIX, req(EMAIL, &handle, &wrong)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for invalid in [
        req("other@example.test", &handle, &code),
        req(EMAIL, &signin, &signin_code),
    ] {
        assert_eq!(rig.rpc(PREFIX, invalid).await.0, StatusCode::CONFLICT);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let (a, b) = tokio::join!(
        rig.rpc(PREFIX, req(EMAIL, &handle, &code)),
        rig.rpc(PREFIX, req(EMAIL, &handle, &code))
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    let verified = if a.0 == StatusCode::OK { a.1 } else { b.1 };
    let (status, binding) = rig
        .rpc(
            "/identity/v1/email/verified",
            json!({"credential":verified["session"]["credential"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(binding["verified_email"], EMAIL);
    let (other, other_code) = proof(&pool, &rig.store, &EmailAddress::parse(EMAIL).unwrap()).await;
    assert_eq!(
        rig.rpc(PREFIX, req(EMAIL, &other, &other_code)).await.0,
        StatusCode::CONFLICT
    );
    let spent: bool =
        sqlx::query_scalar("SELECT consumed_at IS NOT NULL FROM email_challenges WHERE id=$1")
            .bind(other.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(spent);
    let expired_address = EmailAddress::parse("expired@example.test").unwrap();
    let (expired, answer) = proof(&pool, &rig.store, &expired_address).await;
    sqlx::query("UPDATE email_challenges SET created_at=now()-interval '2 hours',expires_at=now()-interval '1 hour' WHERE id=$1")
        .bind(expired.id).execute(&pool).await.unwrap();
    assert_eq!(
        rig.rpc(PREFIX, req(expired_address.as_str(), &expired, &answer))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = false)]
async fn upgrade_retires_only_invalid_subjects_and_replays_existing_receipts(pool: PgPool) {
    use vink_identity::{accounts::review_legacy_email_accounts, persistence::MIGRATOR};
    MIGRATOR.run_to(20261205000000, &pool).await.unwrap();
    let valid = Uuid::now_v7();
    let missing = Uuid::now_v7();
    let malformed = Uuid::now_v7();
    sqlx::query("INSERT INTO accounts(id) VALUES ($1),($2),($3)")
        .bind(valid)
        .bind(missing)
        .bind(malformed)
        .execute(&pool)
        .await
        .unwrap();
    // Legacy lower-case uniqueness admits two byte sequences for the same canonical mailbox.
    sqlx::query("INSERT INTO account_emails VALUES ($1,'văn@example.test',now()),($2,$3,now())")
        .bind(valid)
        .bind(malformed)
        .bind("va\u{306}n@example.test")
        .execute(&pool)
        .await
        .unwrap();
    let verification: time::OffsetDateTime =
        sqlx::query_scalar("SELECT verified_at FROM account_emails WHERE account_id=$1")
            .bind(valid)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("ALTER TABLE account_emails ALTER COLUMN verified_at DROP NOT NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert!(review_legacy_email_accounts(&pool, true).await.is_err());
    let touched: i64 = sqlx::query_scalar("SELECT count(*) FROM account_deletions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(touched, 0);
    sqlx::query("ALTER TABLE account_emails ALTER COLUMN verified_at SET NOT NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        review_legacy_email_accounts(&pool, false).await.unwrap(),
        (3, 2)
    );
    assert!(MIGRATOR.run(&pool).await.is_err());
    assert_eq!(
        review_legacy_email_accounts(&pool, true).await.unwrap(),
        (3, 2)
    );
    assert_eq!(
        review_legacy_email_accounts(&pool, true).await.unwrap(),
        (1, 0)
    );
    let rig = Rig::new(&pool);
    let receipts = rig.store.pending_deletions(None).await.unwrap();
    assert_eq!(receipts.len(), 2);
    assert!(
        receipts
            .iter()
            .all(|r| [missing, malformed].contains(&r.account_id))
    );
    assert!(MIGRATOR.run(&pool).await.is_err());
    for receipt in receipts {
        rig.store
            .finish_deletion(receipt.operation_id)
            .await
            .unwrap();
        rig.store
            .finish_deletion(receipt.operation_id)
            .await
            .unwrap();
        assert_eq!(
            rig.store.deletion(receipt.operation_id).await.unwrap(),
            Some(receipt)
        );
    }
    MIGRATOR.run(&pool).await.unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    let after: (Uuid, String, time::OffsetDateTime) =
        sqlx::query_as("SELECT account_id,address,verified_at FROM account_emails")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, (valid, "văn@example.test".into(), verification));
    assert!(
        sqlx::query("INSERT INTO accounts(id) VALUES ($1)")
            .bind(Uuid::now_v7())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE account_emails SET address='bad' WHERE account_id=$1")
            .bind(valid)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM account_emails WHERE account_id=$1")
            .bind(valid)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO account_emails VALUES ($1,'revived@example.test',now())")
            .bind(missing)
            .execute(&pool)
            .await
            .is_err()
    );
}
