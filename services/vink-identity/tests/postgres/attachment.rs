//! First-email verification through the real Identity router, PostgreSQL and private local outbox.

use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
use vink_identity::accounts::{
    AccountStore, AttachmentRejection, ChallengeOutcome, NewSession, SignInOutcome, disable_account,
};
use vink_identity::authority::{Authority, MethodCapabilities};
use vink_identity::delivery::{DeliveryPolicy, Worker};
use vink_identity::email_code::{ChallengeHandle, Code, EmailAddress, Policy};
use vink_identity::http::{IdentityState, Peer, router};
use vink_identity::mail::LocalOutbox;
use vink_identity::password::{Password, Username};

const PASSWORD: &str = "mat-khau-dai-2026";
const EMAIL: &str = "ban-thao@example.test";
const PREFIX: &str = "/identity/v1/email/attachment";
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
        let outbox = std::env::temp_dir().join(format!("vink-attach-outbox-{}", Uuid::now_v7()));
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

    async fn register(&self, username: &str) -> NewSession {
        self.store
            .register_password(
                &Username::parse(username).unwrap(),
                &Password::parse(PASSWORD).unwrap(),
            )
            .await
            .unwrap()
            .unwrap()
    }

    async fn start(&self, session: &NewSession, email: &str) -> ChallengeHandle {
        let (status, body) = self
            .rpc(
                &format!("{PREFIX}/challenges"),
                json!({
                    "credential": session.credential.encode(), "session_id": session.session_id,
                    "email": email, "password": PASSWORD,
                }),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "challenge issuance returned {}",
            body["error"]["code"]
        );
        let handle = ChallengeHandle::parse(body["challenge"].as_str().unwrap()).unwrap();
        assert_eq!(body["challenge_id"], handle.id.to_string());
        assert_eq!(body["email"], EmailAddress::parse(email).unwrap().as_str());
        handle
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

    async fn complete(
        &self,
        session: &NewSession,
        handle: &ChallengeHandle,
        request: Uuid,
        code: &Code,
    ) -> (StatusCode, Value) {
        self.rpc(
            &format!("{PREFIX}/complete"),
            json!({
                "credential": session.credential.encode(), "challenge_id": request,
                "challenge": handle.encode(), "code": code.expose(),
            }),
        )
        .await
    }

    async fn status(&self, session: &NewSession) -> (StatusCode, Value) {
        self.rpc(
            &format!("{PREFIX}/status"),
            json!({"credential": session.credential.encode()}),
        )
        .await
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

async fn code_state(pool: &PgPool, id: Uuid) -> (i32, bool) {
    sqlx::query_as(
        "SELECT failed_attempts, consumed_at IS NOT NULL FROM email_challenges WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn owner(pool: &PgPool, address: &str) -> Option<Uuid> {
    sqlx::query_scalar("SELECT account_id FROM account_emails WHERE address = $1")
        .bind(address)
        .fetch_optional(pool)
        .await
        .unwrap()
}

fn wrong(code: &Code) -> Code {
    let digit = (code.expose().as_bytes()[0] - b'0' + 1) % 10;
    Code::parse(&format!("{digit}{}", &code.expose()[1..])).unwrap()
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn attachment_readback_reports_only_a_real_binding_and_preserves_account_and_session(
    pool: PgPool,
) {
    let rig = Rig::new(&pool);
    let a = rig.register("tac_gia").await;
    let (status, body) = rig.status(&a).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"session_id":a.session_id,"password_account":true,"verified_email":null})
    );
    let (status, body) = rig.rpc(&format!("{PREFIX}/challenges"), json!({
        "credential":a.credential.encode(),"session_id":a.session_id,"email":EMAIL,"password":"wrong"
    })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_credentials");
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
    let handle = rig.start(&a, "BAN-THAO@example.test").await;
    let code = rig.code(&handle, "attach_email").await;

    let (status, body) = rig.complete(&a, &handle, Uuid::now_v7(), &code).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "attachment_context_changed");
    assert_eq!(code_state(&pool, handle.id).await, (0, false));
    let (status, body) = rig.complete(&a, &handle, handle.id, &wrong(&code)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_code");
    assert_eq!(code_state(&pool, handle.id).await, (1, false));
    assert_eq!(owner(&pool, EMAIL).await, None);
    // Client observes only the later status; completion's body contains no session or identity.
    assert_eq!(
        rig.complete(&a, &handle, handle.id, &code).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(
        rig.status(&a).await.1,
        json!({
            "session_id":a.session_id, "password_account":true, "verified_email":EMAIL,
        })
    );
    assert_eq!(owner(&pool, EMAIL).await, Some(a.account_id));
    assert_eq!(code_state(&pool, handle.id).await, (1, true));
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM accounts), (SELECT count(*) FROM sessions),
                (SELECT count(*) FROM account_passwords WHERE version = 1)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1, 1));

    let (handle, code) = rig.sign_in_proof(EMAIL).await;
    let SignInOutcome::Authenticated(after) =
        rig.store.complete_sign_in(&handle, &code).await.unwrap()
    else {
        panic!("sign-in after attachment")
    };
    assert_eq!(after.account_id, a.account_id);
    let pending_account = rig.register("pending_account").await;
    let pending = rig.start(&pending_account, "pending@example.test").await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations-rollback/20261120000000_resend_payload_fence.down.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations-rollback/20261119000000_email_attachment.down.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        owner(&pool, EMAIL).await,
        Some(a.account_id),
        "rollback keeps verified bindings"
    );
    let proof_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM email_challenges WHERE id = $1)")
            .bind(pending.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!proof_exists, "rollback drops pending attach proofs");
    assert_eq!(
        vink_identity::persistence::migrate(&pool)
            .await
            .unwrap()
            .applied,
        [20261119000000, 20261120000000]
    );
    assert_eq!(rig.status(&a).await.1["verified_email"], EMAIL);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn attachment_is_session_purpose_version_and_expiry_scoped(pool: PgPool) {
    let rig = Rig::new(&pool);
    let a = rig.register("account_a").await;
    let a2 = rig
        .store
        .sign_in_password(&Username::parse("account_a").unwrap(), PASSWORD)
        .await
        .unwrap()
        .unwrap();
    let stale = rig.rpc(&format!("{PREFIX}/challenges"), json!({
        "credential":a2.credential.encode(), "session_id":a.session_id, "email":EMAIL, "password":PASSWORD,
    })).await;
    assert_eq!(stale.1["error"]["code"], "attachment_context_changed");
    let (signin, signin_code) = rig.sign_in_proof("other@example.test").await;
    let (status, denied) = rig.complete(&a, &signin, signin.id, &signin_code).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(denied["error"]["code"], "challenge_closed");
    assert_eq!(code_state(&pool, signin.id).await, (0, false));

    let handle = rig.start(&a, EMAIL).await;
    let code = rig.code(&handle, "attach_email").await;
    let (status, denied) = rig.complete(&a2, &handle, handle.id, &code).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(denied["error"]["code"], "challenge_closed");
    assert_eq!(code_state(&pool, handle.id).await, (0, false));
    let second = rig.start(&a, "newer@example.test").await;
    let code2 = rig.code(&second, "attach_email").await;
    assert_eq!(code_state(&pool, handle.id).await, (0, true));
    sqlx::query(
        "UPDATE email_challenges SET created_at = now() - interval '10 minutes',
                expires_at = now() - interval '1 minute' WHERE id = $1",
    )
    .bind(second.id)
    .execute(&pool)
    .await
    .unwrap();
    let denied = rig.complete(&a, &second, second.id, &code2).await;
    assert_eq!(denied.1["error"]["code"], "challenge_closed");
    assert_eq!(code_state(&pool, second.id).await, (0, false));

    let newer = rig.start(&a2, "version@example.test").await;
    let code3 = rig.code(&newer, "attach_email").await;
    sqlx::query("UPDATE account_passwords SET version = version + 1 WHERE account_id = $1")
        .bind(a.account_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        rig.complete(&a2, &newer, newer.id, &code3).await.1["error"]["code"],
        "challenge_closed"
    );
    assert!(rig.store.revoke(&a2.credential.encode()).await.unwrap());
    assert_eq!(
        rig.complete(&a2, &newer, newer.id, &code3).await.0,
        StatusCode::FORBIDDEN
    );
    disable_account(&pool, a.account_id).await.unwrap();
    assert_eq!(rig.status(&a).await.0, StatusCode::FORBIDDEN);
    assert_eq!(owner(&pool, EMAIL).await, None);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn attachment_competition_never_merges_or_overwrites_accounts(pool: PgPool) {
    let rig = Rig::new(&pool);
    let a = rig.register("account_a").await;
    let b = rig.register("account_b").await;
    let first = rig.start(&a, EMAIL).await;
    let second = rig.start(&b, EMAIL).await;
    let code_a = rig.code(&first, "attach_email").await;
    let code_b = rig.code(&second, "attach_email").await;
    let (x, y) = tokio::join!(
        rig.complete(&a, &first, first.id, &code_a),
        rig.complete(&b, &second, second.id, &code_b)
    );
    let (winner, loser, rejected) = if x.0 == StatusCode::NO_CONTENT {
        (&a, &b, y)
    } else {
        assert_eq!(y.0, StatusCode::NO_CONTENT);
        (&b, &a, x)
    };
    assert_eq!(rejected.0, StatusCode::CONFLICT);
    assert_eq!(rejected.1["error"]["code"], "address_unavailable");
    assert_eq!(owner(&pool, EMAIL).await, Some(winner.account_id));
    assert!(rig.status(loser).await.1["verified_email"].is_null());
    assert_eq!(code_state(&pool, first.id).await, (0, true));
    assert_eq!(code_state(&pool, second.id).await, (0, true));
    let dummy = rig.start(loser, EMAIL).await;
    rig.worker.run_once().await.unwrap();
    assert!(
        !rig.outbox
            .join(format!("{}.txt", dummy.id.simple()))
            .exists()
    );
    let queued: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM mail_deliveries WHERE challenge_id = $1)")
            .bind(dummy.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!queued);
    let (_, body) = rig
        .rpc(
            &format!("{PREFIX}/challenges"),
            json!({
                "credential":winner.credential.encode(),"session_id":winner.session_id,
                "email":"different@example.test","password":PASSWORD,
            }),
        )
        .await;
    assert_eq!(body["error"]["code"], "already_attached");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn attachment_rolls_back_insert_and_proof_if_expiry_passes_during_unique_index_wait(
    pool: PgPool,
) {
    let rig = Rig::new(&pool);
    let a = rig.register("account_a").await;
    let b = rig.register("account_b").await;
    let handle = rig.start(&a, EMAIL).await;
    let code = rig.code(&handle, "attach_email").await;
    sqlx::query("UPDATE email_challenges SET expires_at = clock_timestamp() + interval '2 seconds' WHERE id = $1")
        .bind(handle.id).execute(&pool).await.unwrap();
    let mut competitor = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO account_emails (account_id, address, verified_at) VALUES ($1, $2, now())",
    )
    .bind(b.account_id)
    .bind(EMAIL)
    .execute(&mut *competitor)
    .await
    .unwrap();
    let store = rig.store.clone();
    let credential = a.credential.encode();
    let id = handle.id;
    let committing = tokio::spawn(async move {
        store
            .complete_email_attachment(&credential, &handle, &code)
            .await
            .unwrap()
    });
    let mut blocked = false;
    for _ in 0..100 {
        blocked = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database()
               AND cardinality(pg_blocking_pids(pid)) > 0)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if blocked {
            break;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    assert!(
        blocked,
        "completion must actually wait on the unique address"
    );
    sqlx::query(
        "SELECT pg_sleep(GREATEST(EXTRACT(EPOCH FROM (expires_at - clock_timestamp())), 0) + 0.05)
                 FROM email_challenges WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    competitor.rollback().await.unwrap();
    assert_eq!(
        committing.await.unwrap(),
        Err(AttachmentRejection::ChallengeClosed)
    );
    assert_eq!(owner(&pool, EMAIL).await, None);
    assert_eq!(code_state(&pool, id).await, (0, false));
}
