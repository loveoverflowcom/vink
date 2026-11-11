//! Credential enrollment (ACC-03) on real PostgreSQL: a signed-in email-code account adds a
//! username/password to the SAME account, proven by a fresh code mailed to its own verified
//! address. Codes reach the local outbox through the real delivery worker and are read back like
//! a person reads their mail. Races are made deterministic with row locks held by the test and
//! `pg_blocking_pids`, not with sleeps.

use std::os::unix::fs::PermissionsExt;
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
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;
use vink_identity::accounts::{
    AccountStore, ChallengeOutcome, EmailChangeOutcome, EmailChangeStart, EnrollmentOutcome,
    EnrollmentRejection, EnrollmentStart, NewSession, PasswordRegistrationError,
    PasswordSignInRejection, SignInOutcome, StoreError, disable_account,
};
use vink_identity::authority::{Authority, MethodCapabilities};
use vink_identity::contract::CredentialRejection;
use vink_identity::delivery::{DeliveryPolicy, Worker};
use vink_identity::email_code::{ChallengeHandle, Code, EmailAddress, Policy};
use vink_identity::http::{IdentityState, Peer, router};
use vink_identity::mail::{LocalOutbox, Transport};
use vink_identity::password::{Password, Username, hash_password};

const CALLER: u32 = 501;
const MAIL: &str = "tac.gia@vi-du.vn";

struct Harness {
    store: AccountStore,
    worker: Worker,
    outbox: PathBuf,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.outbox);
    }
}

fn policy() -> Policy {
    Policy {
        max_challenges_per_window: 50,
        max_email_changes_per_window: 50,
        max_challenges_globally: 500,
        ..Policy::default()
    }
}

fn harness(pool: &PgPool) -> Harness {
    let outbox = std::env::temp_dir().join(format!("vink-enroll-outbox-{}", Uuid::now_v7()));
    std::fs::create_dir(&outbox).unwrap();
    std::fs::set_permissions(&outbox, std::fs::Permissions::from_mode(0o700)).unwrap();
    let wake = Arc::new(Notify::new());
    Harness {
        store: AccountStore::new(pool.clone(), policy(), wake.clone()),
        worker: Worker::new(
            pool.clone(),
            Arc::new(LocalOutbox::open(&outbox).unwrap()) as Transport,
            DeliveryPolicy::default(),
            wake,
        ),
        outbox,
    }
}

fn user(raw: &str) -> Username {
    Username::parse(raw).unwrap()
}

fn pass(raw: &str) -> Password {
    Password::parse(raw).unwrap()
}

fn hashed(raw: &str) -> String {
    hash_password(&pass(raw)).unwrap()
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

impl Harness {
    async fn code(&self, handle: &ChallengeHandle) -> Code {
        self.worker.run_once().await.unwrap();
        let text = std::fs::read_to_string(self.outbox.join(format!("{}.txt", handle.id.simple())))
            .unwrap();
        let line = text
            .lines()
            .find_map(|line| line.strip_prefix("X-Vink-Code: "))
            .unwrap();
        Code::parse(line).unwrap()
    }

    /// First sign-in of `raw`: creates the account (or finds it) and a session.
    async fn email_sign_in(&self, raw: &str) -> NewSession {
        let address = EmailAddress::parse(raw).unwrap();
        let ChallengeOutcome::Issued { handle, .. } =
            self.store.start_sign_in(&address).await.unwrap()
        else {
            panic!("rate limited");
        };
        match self
            .store
            .complete_sign_in(&handle, &self.code(&handle).await)
            .await
            .unwrap()
        {
            SignInOutcome::Authenticated(session) => session,
            SignInOutcome::Rejected(reason) => panic!("rejected: {reason:?}"),
        }
    }

    async fn start(&self, session: &NewSession) -> ChallengeHandle {
        match self
            .store
            .start_password_enrollment(&session.credential.encode())
            .await
            .unwrap()
        {
            EnrollmentStart::Issued(ChallengeOutcome::Issued { handle, .. }) => handle,
            other => panic!("not issued: {other:?}"),
        }
    }

    async fn enroll(
        &self,
        session: &NewSession,
        handle: &ChallengeHandle,
        code: &Code,
        username: &str,
        password: &str,
    ) -> EnrollmentOutcome {
        self.store
            .enroll_password(
                &session.credential.encode(),
                handle,
                code,
                &user(username),
                &hashed(password),
            )
            .await
            .unwrap()
    }

    /// Start, read the code, enroll.
    async fn enrolled(&self, session: &NewSession, username: &str, password: &str) {
        let handle = self.start(session).await;
        let code = self.code(&handle).await;
        assert_eq!(
            self.enroll(session, &handle, &code, username, password)
                .await,
            EnrollmentOutcome::Enrolled
        );
    }
}

fn wrong(code: &Code) -> Code {
    let digit = (code.expose().as_bytes()[0] - b'0' + 1) % 10;
    Code::parse(&format!("{digit}{}", &code.expose()[1..])).unwrap()
}

/// Waits until a backend other than the test's is blocked on a lock held by `holder`.
async fn wait_until_blocked(pool: &PgPool) {
    for _ in 0..400 {
        let blocked: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database() AND cardinality(pg_blocking_pids(pid)) > 0",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if blocked > 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no backend became blocked");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn enrolling_attaches_the_credential_to_the_same_account_and_both_methods_sign_in_as_it(
    pool: PgPool,
) {
    let h = harness(&pool);
    let first = h.email_sign_in(MAIL).await;
    let account = first.account_id;
    let sessions_before = count(&pool, "SELECT count(*) FROM sessions").await;
    let email_before: (String, time::OffsetDateTime) =
        sqlx::query_as("SELECT address, verified_at FROM account_emails WHERE account_id = $1")
            .bind(account)
            .fetch_one(&pool)
            .await
            .unwrap();

    h.enrolled(&first, "Tac_Gia.01", "mat-khau-moi-123").await;

    // INV-IDENTITY-02..04: no account, no session, the same account id, the email untouched.
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 1);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM sessions").await,
        sessions_before,
        "enrollment issues no session"
    );
    let stored: (Uuid, String, String, i32) = sqlx::query_as(
        "SELECT account_id, username, password_hash, version FROM account_passwords",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.0, account);
    assert_eq!(stored.1, "tac_gia.01", "canonical lowercase");
    assert!(stored.2.starts_with("$argon2id$") && !stored.2.contains("mat-khau-moi-123"));
    assert_eq!(stored.3, 1);
    let email_after: (String, time::OffsetDateTime) =
        sqlx::query_as("SELECT address, verified_at FROM account_emails WHERE account_id = $1")
            .bind(account)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(email_after, email_before, "the verified email is untouched");
    // The presenting session is neither rotated nor revoked.
    assert_eq!(
        h.store
            .verify(&first.credential.encode())
            .await
            .unwrap()
            .unwrap()
            .account_id,
        account
    );

    // Password sign-in (any spelling of the username) and email-code sign-in: one subject.
    let by_password = h
        .store
        .sign_in_password(&user("TAC_GIA.01"), "mat-khau-moi-123")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_password.account_id, account);
    let by_email = h.email_sign_in("  TAC.GIA@vi-du.vn").await;
    assert_eq!(by_email.account_id, account);
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 1);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        1
    );
    // Same kind of opaque session either way.
    assert_eq!(
        by_password.credential.encode().len(),
        by_email.credential.encode().len()
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_proof_is_the_accounts_own_fresh_single_use_code(pool: PgPool) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;
    let b = h.email_sign_in("nguoi.khac@vi-du.vn").await;

    // A wrong code is a counted attempt; nothing is written.
    let handle = h.start(&a).await;
    let code = h.code(&handle).await;
    assert_eq!(
        h.enroll(&a, &handle, &wrong(&code), "tac_gia", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::InvalidCode)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );

    // Another account's session cannot use A's challenge, even with the right code.
    assert_eq!(
        h.enroll(&b, &handle, &code, "nguoi_khac", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );
    let second_session = h.email_sign_in(MAIL).await;
    assert_eq!(second_session.account_id, a.account_id);
    assert_eq!(
        h.enroll(
            &second_session,
            &handle,
            &code,
            "tac_gia",
            "mat-khau-moi-123"
        )
        .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );
    // A sign-in challenge is not an enrollment proof.
    let ChallengeOutcome::Issued {
        handle: sign_in, ..
    } = h
        .store
        .start_sign_in(&EmailAddress::parse(MAIL).unwrap())
        .await
        .unwrap()
    else {
        panic!()
    };
    let sign_in_code = h.code(&sign_in).await;
    assert_eq!(
        h.enroll(&a, &sign_in, &sign_in_code, "tac_gia", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );

    // The right code for the right account works once; a replay finds nothing to enroll.
    assert_eq!(
        h.enroll(&a, &handle, &code, "tac_gia", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Enrolled
    );
    assert_eq!(
        h.enroll(&a, &handle, &code, "tac_gia_2", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::AlreadyEnrolled)
    );
    // A new proof for an enrolled account is not even issued.
    assert!(matches!(
        h.store
            .start_password_enrollment(&a.credential.encode())
            .await
            .unwrap(),
        EnrollmentStart::AlreadyEnrolled
    ));
    // And B's replay of the spent code cannot enroll B either.
    assert_eq!(
        h.enroll(&b, &handle, &code, "nguoi_khac", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        1
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn an_expired_proof_or_a_changed_address_enrolls_nothing(pool: PgPool) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;

    let handle = h.start(&a).await;
    let code = h.code(&handle).await;
    sqlx::query(
        "UPDATE email_challenges SET created_at = now() - interval '2 hours',
                expires_at = now() - interval '1 hour' WHERE id = $1",
    )
    .bind(handle.id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        h.enroll(&a, &handle, &code, "tac_gia", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );

    // A verified address change closes a proof, even if the account later returns to the old
    // address without switching sessions.
    let handle = h.start(&a).await;
    let code = h.code(&handle).await;
    let credential = a.credential.encode();
    for email in ["moi@vi-du.vn", MAIL] {
        let EmailChangeStart::Issued(ChallengeOutcome::Issued { handle, .. }) = h
            .store
            .start_email_change(&credential, &EmailAddress::parse(email).unwrap())
            .await
            .unwrap()
        else {
            panic!("email change challenge was not issued")
        };
        assert_eq!(
            h.store
                .complete_email_change(&credential, &handle, &h.code(&handle).await)
                .await
                .unwrap(),
            EmailChangeOutcome::Changed
        );
    }
    assert_eq!(
        h.enroll(&a, &handle, &code, "tac_gia", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_proof_that_expires_while_waiting_on_a_username_cannot_commit(pool: PgPool) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;
    let handle = h.start(&a).await;
    let code = h.code(&handle).await;
    let username = user("tac_gia");
    let hash = hashed("mat-khau-moi-123");
    let mut other = pool.begin().await.unwrap();
    let other_id = Uuid::now_v7();
    sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
        .bind(other_id)
        .execute(&mut *other)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO account_passwords (account_id, username, password_hash) VALUES ($1, $2, $3)",
    )
    .bind(other_id)
    .bind(username.as_str())
    .bind(&hash)
    .execute(&mut *other)
    .await
    .unwrap();
    sqlx::query("UPDATE email_challenges SET expires_at = clock_timestamp() + interval '2 seconds' WHERE id = $1")
        .bind(handle.id)
        .execute(&pool)
        .await
        .unwrap();
    let id = handle.id;
    let enrollment = tokio::spawn(async move {
        h.store
            .enroll_password(&a.credential.encode(), &handle, &code, &username, &hash)
            .await
            .unwrap()
    });
    wait_until_blocked(&pool).await;
    sqlx::query(
        "SELECT pg_sleep(GREATEST(0, EXTRACT(EPOCH FROM expires_at - clock_timestamp())))
           FROM email_challenges WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    other.rollback().await.unwrap();
    assert_eq!(
        enrollment.await.unwrap(),
        EnrollmentOutcome::Rejected(EnrollmentRejection::ChallengeClosed)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_taken_username_does_not_spend_the_code_and_is_case_insensitive(pool: PgPool) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;
    let b = h.email_sign_in("nguoi.khac@vi-du.vn").await;
    h.enrolled(&b, "But_Danh", "mat-khau-cua-b-123").await;

    let handle = h.start(&a).await;
    let code = h.code(&handle).await;
    assert_eq!(
        h.enroll(&a, &handle, &code, "BUT_DANH", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Rejected(EnrollmentRejection::UsernameTaken)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        1,
        "B's credential only"
    );
    let consumed: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT consumed_at FROM email_challenges WHERE id = $1")
            .bind(handle.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(consumed.is_none(), "the code is still usable");
    // B's credential is intact: its hash and version did not change.
    assert!(
        h.store
            .sign_in_password(&user("but_danh"), "mat-khau-cua-b-123")
            .await
            .unwrap()
            .is_ok()
    );
    // Another username with the same code succeeds.
    assert_eq!(
        h.enroll(&a, &handle, &code, "but_danh_a", "mat-khau-moi-123")
            .await,
        EnrollmentOutcome::Enrolled
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn concurrent_enrollments_of_one_account_leave_exactly_one_credential(pool: PgPool) {
    let h = Arc::new(harness(&pool));
    let a = Arc::new(h.email_sign_in(MAIL).await);
    // Two proofs, two usernames, one account: both are valid on their own.
    let mut proofs = Vec::new();
    for username in ["ten_mot", "ten_hai", "ten_ba", "ten_bon"] {
        let handle = h.start(&a).await;
        proofs.push((username, h.code(&handle).await, handle));
    }
    let mut attempts = Vec::new();
    for (username, code, handle) in proofs {
        let (h, a) = (Arc::clone(&h), Arc::clone(&a));
        attempts.push(tokio::spawn(async move {
            h.enroll(&a, &handle, &code, username, "mat-khau-moi-123")
                .await
        }));
    }
    let mut enrolled = 0;
    for attempt in attempts {
        match attempt.await.unwrap() {
            EnrollmentOutcome::Enrolled => enrolled += 1,
            EnrollmentOutcome::Rejected(EnrollmentRejection::AlreadyEnrolled) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(enrolled, 1);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 1);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn concurrent_enrollments_of_two_accounts_with_one_username_give_it_to_one(pool: PgPool) {
    let h = Arc::new(harness(&pool));
    let mut tasks = Vec::new();
    let mut sessions = Vec::new();
    for n in 0..4 {
        sessions.push(Arc::new(
            h.email_sign_in(&format!("tac.gia.{n}@vi-du.vn")).await,
        ));
    }
    for session in &sessions {
        let handle = h.start(session).await;
        let code = h.code(&handle).await;
        let (h, session) = (Arc::clone(&h), Arc::clone(session));
        tasks.push(tokio::spawn(async move {
            h.enroll(&session, &handle, &code, "chung_ten", "mat-khau-moi-123")
                .await
        }));
    }
    let mut outcomes = Vec::new();
    for task in tasks {
        outcomes.push(task.await.unwrap());
    }
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o == EnrollmentOutcome::Enrolled)
            .count(),
        1
    );
    assert!(outcomes.iter().all(|o| matches!(
        o,
        EnrollmentOutcome::Enrolled
            | EnrollmentOutcome::Rejected(EnrollmentRejection::UsernameTaken)
    )));
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM account_passwords WHERE username = 'chung_ten'"
        )
        .await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 4);
    // The loser can still enroll with another username using the unspent code.
    let unspent: i64 = count(
        &pool,
        "SELECT count(*) FROM email_challenges
          WHERE purpose = 'enroll_password' AND consumed_at IS NULL",
    )
    .await;
    assert_eq!(unspent, 3);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_logout_that_wins_the_race_blocks_the_enrollment(pool: PgPool) {
    let h = Arc::new(harness(&pool));
    let a = Arc::new(h.email_sign_in(MAIL).await);
    let handle = h.start(&a).await;
    let code = h.code(&handle).await;

    // Hold the session row like a logout in flight: the enrollment passes its first check, then
    // waits at the fence.
    let mut logout = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM sessions WHERE id = $1 FOR UPDATE")
        .bind(a.session_id)
        .execute(&mut *logout)
        .await
        .unwrap();
    let racing = {
        let (h, a) = (Arc::clone(&h), Arc::clone(&a));
        tokio::spawn(async move {
            h.enroll(&a, &handle, &code, "tac_gia", "mat-khau-moi-123")
                .await
        })
    };
    wait_until_blocked(&pool).await;
    sqlx::query("UPDATE sessions SET revoked_at = now(), revoked_reason = 'logout' WHERE id = $1")
        .bind(a.session_id)
        .execute(&mut *logout)
        .await
        .unwrap();
    logout.commit().await.unwrap();

    assert_eq!(
        racing.await.unwrap(),
        EnrollmentOutcome::CredentialRejected,
        "a revoked session does not pass the commit fence"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );
    // Afterwards a revoked session cannot start or finish anything.
    assert!(matches!(
        h.store
            .start_password_enrollment(&a.credential.encode())
            .await
            .unwrap(),
        EnrollmentStart::CredentialRejected
    ));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_disable_that_wins_the_race_blocks_the_enrollment_and_a_credential_never_revives(
    pool: PgPool,
) {
    let h = Arc::new(harness(&pool));
    let a = Arc::new(h.email_sign_in(MAIL).await);
    let handle = h.start(&a).await;
    let code = h.code(&handle).await;

    let mut operator = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE")
        .bind(a.account_id)
        .execute(&mut *operator)
        .await
        .unwrap();
    let racing = {
        let (h, a) = (Arc::clone(&h), Arc::clone(&a));
        tokio::spawn(async move {
            h.enroll(&a, &handle, &code, "tac_gia", "mat-khau-moi-123")
                .await
        })
    };
    wait_until_blocked(&pool).await;
    sqlx::query("UPDATE accounts SET disabled_at = now() WHERE id = $1")
        .bind(a.account_id)
        .execute(&mut *operator)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET revoked_at = now(), revoked_reason = 'account_disabled'")
        .execute(&mut *operator)
        .await
        .unwrap();
    operator.commit().await.unwrap();

    assert_eq!(
        racing.await.unwrap(),
        EnrollmentOutcome::Rejected(EnrollmentRejection::AccountDisabled)
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );
    assert!(matches!(
        h.store
            .start_password_enrollment(&a.credential.encode())
            .await
            .unwrap(),
        EnrollmentStart::AccountDisabled
    ));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn enrolling_then_disabling_or_revoking_never_restores_a_session(pool: PgPool) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;
    let other = h.email_sign_in(MAIL).await;
    assert!(h.store.revoke(&other.credential.encode()).await.unwrap());
    h.enrolled(&a, "tac_gia", "mat-khau-moi-123").await;
    // The enrollment did not revive the revoked session of the same account.
    assert_eq!(
        h.store.verify(&other.credential.encode()).await.unwrap(),
        Err(CredentialRejection::Revoked)
    );
    disable_account(&pool, a.account_id).await.unwrap();
    assert_eq!(
        h.store.verify(&a.credential.encode()).await.unwrap(),
        Err(CredentialRejection::AccountDisabled)
    );
    assert!(matches!(
        h.store
            .sign_in_password(&user("tac_gia"), "mat-khau-moi-123")
            .await
            .unwrap(),
        Err(PasswordSignInRejection::AccountDisabled)
    ));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_failure_after_the_hash_leaves_no_half_credential_and_the_proof_stays_usable(
    pool: PgPool,
) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;
    let handle = h.start(&a).await;
    let code = h.code(&handle).await;
    let hash = hashed("mat-khau-moi-123");

    // The database fails at the write, after the code was accepted inside the transaction.
    sqlx::raw_sql(
        "CREATE FUNCTION fail_write() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'injected failure'; END $$;
         CREATE TRIGGER fail_write BEFORE INSERT ON account_passwords
             FOR EACH ROW EXECUTE FUNCTION fail_write();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let failed = h
        .store
        .enroll_password(
            &a.credential.encode(),
            &handle,
            &code,
            &user("tac_gia"),
            &hash,
        )
        .await;
    assert_eq!(failed, Err(StoreError::Unavailable));
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        0
    );
    let consumed: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT consumed_at FROM email_challenges WHERE id = $1")
            .bind(handle.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(consumed.is_none(), "an unavailable store spends no code");

    sqlx::raw_sql("DROP TRIGGER fail_write ON account_passwords; DROP FUNCTION fail_write();")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        h.store
            .enroll_password(
                &a.credential.encode(),
                &handle,
                &code,
                &user("tac_gia"),
                &hash
            )
            .await
            .unwrap(),
        EnrollmentOutcome::Enrolled
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_password_only_account_is_not_enrolled_again_and_registration_still_makes_new_accounts(
    pool: PgPool,
) {
    let h = harness(&pool);
    let registered = h
        .store
        .register_password(&user("chi_mat_khau"), &pass("mat-khau-chinh-123"))
        .await
        .unwrap()
        .unwrap();
    // INV-IDENTITY-08/09: it has a password and no email, so nothing is added or linked.
    assert!(matches!(
        h.store
            .start_password_enrollment(&registered.credential.encode())
            .await
            .unwrap(),
        EnrollmentStart::AlreadyEnrolled
    ));
    // Registration keeps its semantics: a new username is a new account.
    let second = h
        .store
        .register_password(&user("tai_khoan_hai"), &pass("mat-khau-chinh-123"))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(second.account_id, registered.account_id);
    assert_eq!(
        h.store
            .register_password(&user("CHI_MAT_KHAU"), &pass("mat-khau-chinh-123"))
            .await
            .unwrap()
            .unwrap_err(),
        PasswordRegistrationError::UsernameTaken
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 2);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn an_unknown_or_expired_session_starts_nothing_and_sends_no_mail(pool: PgPool) {
    let h = harness(&pool);
    assert!(matches!(
        h.store
            .start_password_enrollment("vink_s1_not-a-session")
            .await
            .unwrap(),
        EnrollmentStart::CredentialRejected
    ));
    let a = h.email_sign_in(MAIL).await;
    sqlx::query("UPDATE sessions SET created_at = now() - interval '2 days', expires_at = now() - interval '1 day' WHERE id = $1")
        .bind(a.session_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .start_password_enrollment(&a.credential.encode())
            .await
            .unwrap(),
        EnrollmentStart::CredentialRejected
    ));
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM email_challenges WHERE purpose = 'enroll_password'"
        )
        .await,
        0
    );
}

fn native(pool: &PgPool, h: &Harness) -> Router {
    router(IdentityState::new(
        pool.clone(),
        Authority::new_native(
            h.store.clone(),
            MethodCapabilities {
                email_code: true,
                password: true,
            },
        ),
        Some(CALLER),
    ))
    .layer(axum::Extension(ConnectInfo(Peer { uid: Some(CALLER) })))
}

async fn post(router: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_rpc_operations_answer_with_typed_results_and_leak_no_secret(pool: PgPool) {
    let h = harness(&pool);
    let router = native(&pool, &h);
    let a = h.email_sign_in(MAIL).await;
    let credential = a.credential.encode();

    // Capabilities advertise it only with both methods.
    let response = router
        .clone()
        .oneshot(
            Request::get("/identity/v1/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let caps: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(caps["operations"]["enroll_password"], "available");
    assert_eq!(
        post(
            &router,
            "/identity/v1/password/enroll/status",
            json!({"credential": credential})
        )
        .await,
        (StatusCode::OK, json!({"enrolled": false}))
    );

    // Unknown session: 403 credential_rejected, no mail.
    let (status, body) = post(
        &router,
        "/identity/v1/password/enroll/challenges",
        json!({"credential": "vink_s1_nope"}),
    )
    .await;
    assert_eq!(
        (status, &body["error"]["code"]),
        (StatusCode::FORBIDDEN, &json!("credential_rejected"))
    );

    let (status, body) = post(
        &router,
        "/identity/v1/password/enroll/challenges",
        json!({"credential": credential}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let handle = ChallengeHandle::parse(body["challenge"].as_str().unwrap()).unwrap();
    let code = h.code(&handle).await;
    let challenge = body["challenge"].as_str().unwrap().to_string();

    let request = |code: &str, username: &str, password: &str| {
        json!({
            "credential": credential, "challenge": challenge, "code": code,
            "username": username, "password": password,
        })
    };
    // The caller cannot name the account.
    let mut forged = request(code.expose(), "tac_gia", "mat-khau-moi-123");
    forged["account_id"] = json!(Uuid::now_v7().to_string());
    let (status, _) = post(&router, "/identity/v1/password/enroll", forged).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Policy errors are 422 and spend nothing.
    for (username, password, expected) in [
        ("ab", "mat-khau-moi-123", "invalid_username"),
        ("tac_gia", "ngan", "invalid_password"),
        ("tac_gia", &"a".repeat(129), "invalid_password"),
        ("tac_gia", &"a".repeat(4097), "malformed_request"),
    ] {
        let (status, body) = post(
            &router,
            "/identity/v1/password/enroll",
            request(code.expose(), username, password),
        )
        .await;
        assert_eq!(
            body["error"]["code"],
            expected,
            "{username} {}",
            password.len()
        );
        assert!(status.is_client_error());
    }
    let (status, body) = post(
        &router,
        "/identity/v1/password/enroll",
        request(wrong(&code).expose(), "tac_gia", "mat-khau-moi-123"),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::OK,
            json!({"outcome": "rejected", "reason": "invalid_code"})
        )
    );
    let (status, body) = post(
        &router,
        "/identity/v1/password/enroll",
        request(code.expose(), "tac_gia", "mat-khau-moi-123"),
    )
    .await;
    assert_eq!(
        (status, &body),
        (StatusCode::OK, &json!({"outcome": "enrolled"}))
    );
    // Nothing secret or internal is returned.
    let text = body.to_string();
    for forbidden in [
        "mat-khau",
        "argon2",
        "credential",
        "subject",
        "session",
        "account",
    ] {
        assert!(!text.contains(forbidden), "{text}");
    }
    // Readback needs only the still-valid session and sends no further mail.
    assert_eq!(
        post(
            &router,
            "/identity/v1/password/enroll/status",
            json!({"credential": credential})
        )
        .await,
        (StatusCode::OK, json!({"enrolled": true}))
    );
    let (status, body) = post(
        &router,
        "/identity/v1/password/enroll/challenges",
        json!({"credential": credential}),
    )
    .await;
    assert_eq!(
        (status, &body["error"]["code"]),
        (StatusCode::CONFLICT, &json!("already_enrolled"))
    );
    let (_, body) = post(
        &router,
        "/identity/v1/password/enroll",
        request(code.expose(), "tac_gia", "mat-khau-moi-123"),
    )
    .await;
    assert_eq!(
        body,
        json!({"outcome": "rejected", "reason": "already_enrolled"})
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn o12_unicode_passwords_work_end_to_end_through_enrollment_and_sign_in(pool: PgPool) {
    let h = harness(&pool);
    let router = native(&pool, &h);
    let a = h.email_sign_in(MAIL).await;
    let credential = a.credential.encode();

    // Typed decomposed (NFD): 10 letters, far more code points. Stored and verified as NFKC.
    let nfc = "mậtkhẩuđẹp";
    let nfd: String = nfc.nfd().collect();
    assert!(nfd.chars().count() > 10 && nfd.len() < 4096);
    let (_, body) = post(
        &router,
        "/identity/v1/password/enroll/challenges",
        json!({"credential": credential}),
    )
    .await;
    let challenge = body["challenge"].as_str().unwrap().to_string();
    let code = h.code(&ChallengeHandle::parse(&challenge).unwrap()).await;
    let (status, body) = post(
        &router,
        "/identity/v1/password/enroll",
        json!({"credential": credential, "challenge": challenge, "code": code.expose(),
               "username": "viet_nam", "password": nfd}),
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({"outcome": "enrolled"}))
    );

    // Sign in typing the precomposed form, a full-width (NFKC) form of an ASCII-only variant is
    // covered by the policy unit test; here the Vietnamese spelling must match both ways.
    for typed in [nfc.to_string(), nfd.clone()] {
        let (status, body) = post(
            &router,
            "/identity/v1/password/sign-in",
            json!({"username": "Viet_Nam", "password": typed}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["outcome"], "authenticated");
        assert_eq!(
            body["subject"]["id"].as_str().unwrap(),
            a.account_id.to_string(),
            "the password signs in as the email account"
        );
    }
    // A wrong password and an unknown username answer the same.
    let wrong_password = post(
        &router,
        "/identity/v1/password/sign-in",
        json!({"username": "viet_nam", "password": "mat-khau-sai-hoan-toan"}),
    )
    .await;
    let unknown = post(
        &router,
        "/identity/v1/password/sign-in",
        json!({"username": "khong_co_ai", "password": "mat-khau-sai-hoan-toan"}),
    )
    .await;
    assert_eq!(wrong_password, unknown);
    assert_eq!(wrong_password.1["reason"], "invalid_credentials");

    // Length bounds count code points after NFKC; the raw bound is 4096 bytes.
    let b = h.email_sign_in("nguoi.khac@vi-du.vn").await;
    let b_credential = b.credential.encode();
    let long = "ệ".repeat(128);
    let (_, body) = post(
        &router,
        "/identity/v1/password/enroll/challenges",
        json!({"credential": b_credential}),
    )
    .await;
    let challenge = body["challenge"].as_str().unwrap().to_string();
    let code = h.code(&ChallengeHandle::parse(&challenge).unwrap()).await;
    for (password, expected) in [
        ("ệ".repeat(9), "invalid_password"),
        ("ệ".repeat(129), "invalid_password"),
        ("a".repeat(4097), "malformed_request"),
    ] {
        let (_, body) = post(
            &router,
            "/identity/v1/password/enroll",
            json!({"credential": b_credential, "challenge": challenge, "code": code.expose(),
                   "username": "nguoi_khac", "password": password}),
        )
        .await;
        assert_eq!(body["error"]["code"], expected);
    }
    let (_, body) = post(
        &router,
        "/identity/v1/password/enroll",
        json!({"credential": b_credential, "challenge": challenge, "code": code.expose(),
               "username": "dai_nhat", "password": long}),
    )
    .await;
    assert_eq!(body, json!({"outcome": "enrolled"}));
    // Sign-in accepts the maximum too (transport bound is no longer 256 bytes).
    let (_, body) = post(
        &router,
        "/identity/v1/password/sign-in",
        json!({"username": "dai_nhat", "password": long}),
    )
    .await;
    assert_eq!(body["outcome"], "authenticated");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn enrollment_is_not_configured_without_both_methods(pool: PgPool) {
    let h = harness(&pool);
    for (email_code, password) in [(true, false), (false, true)] {
        let router = router(IdentityState::new(
            pool.clone(),
            Authority::new_native(
                h.store.clone(),
                MethodCapabilities {
                    email_code,
                    password,
                },
            ),
            Some(CALLER),
        ))
        .layer(axum::Extension(ConnectInfo(Peer { uid: Some(CALLER) })));
        for (path, body) in [
            (
                "/identity/v1/password/enroll/status",
                json!({"credential": "x"}),
            ),
            (
                "/identity/v1/password/enroll/challenges",
                json!({"credential": "x"}),
            ),
            (
                "/identity/v1/password/enroll",
                json!({"credential": "x", "challenge": "x", "code": "123456",
                       "username": "tac_gia", "password": "mat-khau-moi-123"}),
            ),
        ] {
            let (status, body) = post(&router, path, body).await;
            assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{path}");
            assert_eq!(body["error"]["code"], "authority_not_configured");
        }
    }
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_rollback_script_removes_only_enrollment_challenges_and_keeps_credentials(
    pool: PgPool,
) {
    let h = harness(&pool);
    let a = h.email_sign_in(MAIL).await;
    h.enrolled(&a, "tac_gia", "mat-khau-moi-123").await;
    let b = h.email_sign_in("nguoi.khac@vi-du.vn").await;
    let _pending = h.start(&b).await;
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM email_challenges WHERE purpose = 'enroll_password'"
        )
        .await,
        2
    );
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261114000000_password_enrollment.down.sql"),
    )
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(script))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM email_challenges WHERE purpose = 'enroll_password'"
        )
        .await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        1,
        "an enrolled credential survives the rollback"
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 2);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM _sqlx_migrations WHERE version = 20261114000000"
        )
        .await,
        0
    );
    // The older schema refuses the new purpose again, and password sign-in still works.
    let refused = sqlx::query(
        "INSERT INTO email_challenges (id, purpose, address, account_id, code_hash, expires_at)
         VALUES ($1, 'enroll_password', 'x@vi-du.vn', $2, decode(repeat('ab', 32), 'hex'),
                 now() + interval '10 minutes')",
    )
    .bind(Uuid::now_v7())
    .bind(a.account_id)
    .execute(&pool)
    .await;
    assert!(refused.is_err());
    assert!(
        h.store
            .sign_in_password(&user("tac_gia"), "mat-khau-moi-123")
            .await
            .unwrap()
            .is_ok()
    );
    assert_eq!(
        vink_identity::persistence::migrate(&pool)
            .await
            .unwrap()
            .applied,
        [20261114000000]
    );
    assert_eq!(h.email_sign_in(MAIL).await.account_id, a.account_id);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM account_passwords").await,
        1
    );
}
