//! The email-code authority on real PostgreSQL (D-05/S-03 spike): one-time challenges, account
//! provisioning under concurrency, session verification, logout, expiry, disable and verified
//! email change. Codes are read back from the local outbox like a person reads their mail.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

use sqlx::PgPool;
use uuid::Uuid;
use vink_identity::accounts::{
    AccountChange, AccountStore, ChallengeOutcome, EmailChangeOutcome, EmailChangeStart,
    NewSession, SignInOutcome, disable_account, enable_account,
};
use vink_identity::contract::{ChallengeRejection, CredentialRejection};
use vink_identity::email_code::{ChallengeHandle, Code, EmailAddress, Policy};
use vink_identity::mail::LocalOutbox;

struct Harness {
    store: AccountStore,
    outbox: PathBuf,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.outbox);
    }
}

fn harness(pool: &PgPool, policy: Policy) -> Harness {
    let outbox = std::env::temp_dir().join(format!("vink-outbox-pg-{}", Uuid::now_v7()));
    std::fs::create_dir(&outbox).unwrap();
    std::fs::set_permissions(&outbox, std::fs::Permissions::from_mode(0o700)).unwrap();
    Harness {
        store: AccountStore::new(pool.clone(), LocalOutbox::open(&outbox).unwrap(), policy),
        outbox,
    }
}

fn address(raw: &str) -> EmailAddress {
    EmailAddress::parse(raw).unwrap()
}

impl Harness {
    /// The code the person received for `handle`, read from the outbox message.
    fn code(&self, handle: &ChallengeHandle) -> Code {
        let path = self.outbox.join(format!("{}.txt", handle.id.simple()));
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        let text = std::fs::read_to_string(&path).unwrap();
        let line = text
            .lines()
            .find_map(|line| line.strip_prefix("X-Vink-Code: "))
            .unwrap();
        Code::parse(line).unwrap()
    }

    async fn challenge(&self, raw: &str) -> ChallengeHandle {
        match self.store.start_sign_in(&address(raw)).await.unwrap() {
            ChallengeOutcome::Issued { handle, .. } => handle,
            ChallengeOutcome::RateLimited => panic!("rate limited"),
        }
    }

    async fn sign_in(&self, raw: &str) -> NewSession {
        let handle = self.challenge(raw).await;
        match self
            .store
            .complete_sign_in(&handle, &self.code(&handle))
            .await
            .unwrap()
        {
            SignInOutcome::Authenticated(session) => session,
            SignInOutcome::Rejected(reason) => panic!("rejected: {reason:?}"),
        }
    }

    async fn verify(&self, session: &NewSession) -> Result<Uuid, CredentialRejection> {
        self.store
            .verify(&session.credential.encode())
            .await
            .unwrap()
            .map(|active| active.account_id)
    }
}

fn wrong(code: &Code) -> Code {
    let digit = (code.expose().as_bytes()[0] - b'0' + 1) % 10;
    Code::parse(&format!("{digit}{}", &code.expose()[1..])).unwrap()
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn first_sign_in_creates_one_account_and_a_verifiable_session(pool: PgPool) {
    let h = harness(&pool, Policy::default());
    let first = h.sign_in("Tac.Gia@Vi-Du.vn").await;
    assert_eq!(h.verify(&first).await, Ok(first.account_id));
    // Same mailbox, other spelling: the same account, a new session.
    let second = h.sign_in("  tac.gia@vi-du.VN").await;
    assert_eq!(second.account_id, first.account_id);
    assert_ne!(second.session_id, first.session_id);
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM sessions").await, 2);
    // Nothing secret is stored in clear: verifiers only.
    let credential = first.credential.encode();
    let stored: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT token_hash FROM sessions UNION ALL SELECT code_hash FROM email_challenges",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for bytes in stored {
        assert_eq!(bytes.len(), 32);
        assert!(!String::from_utf8_lossy(&bytes).contains(&credential[8..20]));
    }
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn codes_are_one_time_bound_to_their_handle_and_attempt_limited(pool: PgPool) {
    let h = harness(
        &pool,
        Policy {
            max_challenges_per_window: 10,
            ..Policy::default()
        },
    );
    // Four wrong codes are reported as such; the fifth closes the challenge for good.
    let handle = h.challenge("tho@vi-du.vn").await;
    let right = h.code(&handle);
    for _ in 0..4 {
        assert!(matches!(
            h.store
                .complete_sign_in(&handle, &wrong(&right))
                .await
                .unwrap(),
            SignInOutcome::Rejected(ChallengeRejection::InvalidCode)
        ));
    }
    for code in [wrong(&right), right.clone()] {
        assert!(matches!(
            h.store.complete_sign_in(&handle, &code).await.unwrap(),
            SignInOutcome::Rejected(ChallengeRejection::ChallengeClosed)
        ));
    }

    // The right code with the right id but another secret (a code typed into a client that did
    // not request it) is a wrong code.
    let handle = h.challenge("tho@vi-du.vn").await;
    let code = h.code(&handle);
    let foreign = ChallengeHandle::generate(handle.id).unwrap();
    assert!(matches!(
        h.store.complete_sign_in(&foreign, &code).await.unwrap(),
        SignInOutcome::Rejected(ChallengeRejection::InvalidCode)
    ));
    // Accepted once, never replayed.
    assert!(matches!(
        h.store.complete_sign_in(&handle, &code).await.unwrap(),
        SignInOutcome::Authenticated(_)
    ));
    assert!(matches!(
        h.store.complete_sign_in(&handle, &code).await.unwrap(),
        SignInOutcome::Rejected(ChallengeRejection::ChallengeClosed)
    ));

    // Expired: the right code no longer works.
    let handle = h.challenge("tho@vi-du.vn").await;
    let code = h.code(&handle);
    sqlx::query(
        "UPDATE email_challenges SET created_at = now() - interval '1 hour',
                expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(handle.id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        h.store.complete_sign_in(&handle, &code).await.unwrap(),
        SignInOutcome::Rejected(ChallengeRejection::ChallengeClosed)
    ));
    // An unknown challenge is closed, not an error.
    let unknown = ChallengeHandle::generate(Uuid::now_v7()).unwrap();
    assert!(matches!(
        h.store.complete_sign_in(&unknown, &code).await.unwrap(),
        SignInOutcome::Rejected(ChallengeRejection::ChallengeClosed)
    ));
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 1);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn concurrent_completions_issue_one_session_per_challenge_and_one_account_per_address(
    pool: PgPool,
) {
    let h = harness(
        &pool,
        Policy {
            max_challenges_per_window: 20,
            ..Policy::default()
        },
    );
    // One challenge answered eight times at once: exactly one session.
    let handle = h.challenge("song.song@vi-du.vn").await;
    let code = h.code(&handle);
    let attempts = (0..8).map(|_| {
        let store = h.store.clone();
        let (handle, code) = (handle.clone(), code.clone());
        tokio::spawn(async move { store.complete_sign_in(&handle, &code).await.unwrap() })
    });
    let mut authenticated = 0;
    for attempt in attempts {
        match attempt.await.unwrap() {
            SignInOutcome::Authenticated(_) => authenticated += 1,
            SignInOutcome::Rejected(reason) => {
                assert_eq!(reason, ChallengeRejection::ChallengeClosed);
            }
        }
    }
    assert_eq!(authenticated, 1);

    // Six first sign-ins of a new address racing on different challenges: one account.
    let mut pending = Vec::new();
    for _ in 0..6 {
        let handle = h.challenge("moi.toanh@vi-du.vn").await;
        let code = h.code(&handle);
        pending.push((handle, code));
    }
    let tasks = pending.into_iter().map(|(handle, code)| {
        let store = h.store.clone();
        tokio::spawn(async move { store.complete_sign_in(&handle, &code).await.unwrap() })
    });
    let mut accounts = Vec::new();
    for task in tasks {
        match task.await.unwrap() {
            SignInOutcome::Authenticated(session) => accounts.push(session.account_id),
            SignInOutcome::Rejected(reason) => panic!("{reason:?}"),
        }
    }
    accounts.dedup();
    assert_eq!(accounts.len(), 1, "{accounts:?}");
    assert_eq!(count(&pool, "SELECT count(*) FROM accounts").await, 2);
    assert_eq!(count(&pool, "SELECT count(*) FROM account_emails").await, 2);
}

/// Deterministic race: a test transaction holds the challenge row while four completions arrive,
/// so all of them are in flight together; only then is the row released.
#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn completions_queued_behind_a_row_lock_consume_the_challenge_once(pool: PgPool) {
    // Own pool with room for the gate, the probe and four blocked completions.
    let wide = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let h = harness(&wide, Policy::default());
    let handle = h.challenge("xep.hang@vi-du.vn").await;
    let code = h.code(&handle);
    let mut probe = wide.acquire().await.unwrap();
    let mut gate = wide.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM email_challenges WHERE id = $1 FOR UPDATE")
        .bind(handle.id)
        .execute(&mut *gate)
        .await
        .unwrap();
    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let store = h.store.clone();
            let (handle, code) = (handle.clone(), code.clone());
            tokio::spawn(async move { store.complete_sign_in(&handle, &code).await.unwrap() })
        })
        .collect();
    // Wait until all four are blocked on a row lock in this database (not a timing guess).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(&mut *probe)
        .await
        .unwrap();
        if waiting >= 4 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "only {waiting} completions waited"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    gate.commit().await.unwrap();
    let mut authenticated = 0;
    for task in tasks {
        match task.await.unwrap() {
            SignInOutcome::Authenticated(_) => authenticated += 1,
            SignInOutcome::Rejected(reason) => {
                assert_eq!(reason, ChallengeRejection::ChallengeClosed);
            }
        }
    }
    assert_eq!(authenticated, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM sessions").await, 1);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_per_address_throttle_holds_under_concurrency_and_ignores_account_existence(
    pool: PgPool,
) {
    let h = harness(&pool, Policy::default());
    h.sign_in("da.co@vi-du.vn").await; // one challenge used by an existing account
    for raw in ["da.co@vi-du.vn", "chua.co@vi-du.vn"] {
        let tasks = (0..8).map(|_| {
            let store = h.store.clone();
            let address = address(raw);
            tokio::spawn(async move { store.start_sign_in(&address).await.unwrap() })
        });
        let mut issued = 0;
        for task in tasks {
            if matches!(task.await.unwrap(), ChallengeOutcome::Issued { .. }) {
                issued += 1;
            }
        }
        // Three per window per address; the existing account's earlier code counts too.
        let expected = if raw.starts_with("da") { 2 } else { 3 };
        assert_eq!(issued, expected, "{raw}");
    }
    // Another address is unaffected.
    h.challenge("khac@vi-du.vn").await;
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn logout_revokes_only_that_session_and_is_idempotent(pool: PgPool) {
    let h = harness(&pool, Policy::default());
    let laptop = h.sign_in("hai.may@vi-du.vn").await;
    let phone = h.sign_in("hai.may@vi-du.vn").await;
    let credential = laptop.credential.encode();
    assert!(h.store.revoke(&credential).await.unwrap());
    assert_eq!(h.verify(&laptop).await, Err(CredentialRejection::Revoked));
    assert_eq!(h.verify(&phone).await, Ok(phone.account_id));
    assert!(!h.store.revoke(&credential).await.unwrap());
    assert!(!h.store.revoke("vink_s1_not-a-credential").await.unwrap());
    // Revocation is final: nothing in the contract restores a revoked session.
    assert_eq!(h.verify(&laptop).await, Err(CredentialRejection::Revoked));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn expired_sessions_are_rejected_and_cannot_be_revoked_back_to_life(pool: PgPool) {
    let h = harness(&pool, Policy::default());
    let session = h.sign_in("het.han@vi-du.vn").await;
    sqlx::query(
        "UPDATE sessions SET created_at = now() - interval '31 days',
                expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(session.session_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(h.verify(&session).await, Err(CredentialRejection::Expired));
    assert!(!h.store.revoke(&session.credential.encode()).await.unwrap());
    // A forged credential of the right shape is unknown.
    let forged = vink_identity::email_code::SessionCredential::generate().unwrap();
    assert_eq!(
        h.store.verify(&forged.encode()).await.unwrap(),
        Err(CredentialRejection::Unknown)
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn disabling_an_account_revokes_every_session_and_blocks_new_ones(pool: PgPool) {
    let h = harness(
        &pool,
        Policy {
            max_challenges_per_window: 10,
            ..Policy::default()
        },
    );
    let one = h.sign_in("bi.khoa@vi-du.vn").await;
    let two = h.sign_in("bi.khoa@vi-du.vn").await;
    let bystander = h.sign_in("ben.canh@vi-du.vn").await;
    assert_eq!(
        disable_account(&pool, one.account_id).await.unwrap(),
        AccountChange::Changed {
            sessions_revoked: 2
        }
    );
    assert_eq!(
        disable_account(&pool, one.account_id).await.unwrap(),
        AccountChange::Unchanged
    );
    for session in [&one, &two] {
        assert_eq!(
            h.verify(session).await,
            Err(CredentialRejection::AccountDisabled)
        );
    }
    assert_eq!(h.verify(&bystander).await, Ok(bystander.account_id));
    // Proving the mailbox again yields no session while disabled.
    let sessions = count(&pool, "SELECT count(*) FROM sessions").await;
    let handle = h.challenge("bi.khoa@vi-du.vn").await;
    assert!(matches!(
        h.store
            .complete_sign_in(&handle, &h.code(&handle))
            .await
            .unwrap(),
        SignInOutcome::Rejected(ChallengeRejection::AccountDisabled)
    ));
    assert_eq!(
        count(&pool, "SELECT count(*) FROM sessions").await,
        sessions
    );
    // Re-enabling restores sign-in, never the revoked sessions.
    assert_eq!(
        enable_account(&pool, one.account_id).await.unwrap(),
        AccountChange::Changed {
            sessions_revoked: 0
        }
    );
    assert_eq!(h.verify(&one).await, Err(CredentialRejection::Revoked));
    assert_eq!(
        h.sign_in("bi.khoa@vi-du.vn").await.account_id,
        one.account_id
    );
    assert_eq!(
        disable_account(&pool, Uuid::now_v7()).await.unwrap(),
        AccountChange::UnknownAccount
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_verified_email_change_keeps_the_subject_and_a_freed_address_never_inherits_it(
    pool: PgPool,
) {
    let h = harness(
        &pool,
        Policy {
            max_challenges_per_window: 10,
            ..Policy::default()
        },
    );
    let current = h.sign_in("cu@vi-du.vn").await;
    let other_device = h.sign_in("cu@vi-du.vn").await;
    let credential = current.credential.encode();

    // An address already verified by someone else cannot even start a change.
    let neighbour = h.sign_in("hang.xom@vi-du.vn").await;
    assert!(matches!(
        h.store
            .start_email_change(&credential, &address("hang.xom@vi-du.vn"))
            .await
            .unwrap(),
        EmailChangeStart::AddressUnavailable
    ));
    // A revoked or forged session cannot start one.
    assert!(matches!(
        h.store
            .start_email_change("vink_s1_forged", &address("moi@vi-du.vn"))
            .await
            .unwrap(),
        EmailChangeStart::CredentialRejected
    ));

    let EmailChangeStart::Issued(ChallengeOutcome::Issued { handle, .. }) = h
        .store
        .start_email_change(&credential, &address("moi@vi-du.vn"))
        .await
        .unwrap()
    else {
        panic!("change not started");
    };
    let code = h.code(&handle);
    // Only the account's own session can complete its change.
    assert_eq!(
        h.store
            .complete_email_change(&neighbour.credential.encode(), &handle, &code)
            .await
            .unwrap(),
        EmailChangeOutcome::Rejected(ChallengeRejection::ChallengeClosed)
    );
    assert_eq!(
        h.store
            .complete_email_change(&credential, &handle, &code)
            .await
            .unwrap(),
        EmailChangeOutcome::Changed
    );
    // Same subject; the presenting session stays, the other device is signed out.
    assert_eq!(h.verify(&current).await, Ok(current.account_id));
    assert_eq!(
        h.verify(&other_device).await,
        Err(CredentialRejection::Revoked)
    );
    assert_eq!(
        h.sign_in("moi@vi-du.vn").await.account_id,
        current.account_id
    );
    // The freed address proves a *new* account: no inheritance of the old subject.
    let recycled = h.sign_in("cu@vi-du.vn").await;
    assert_ne!(recycled.account_id, current.account_id);

    // Race: another account verifies the target address before the change completes.
    let EmailChangeStart::Issued(ChallengeOutcome::Issued { handle, .. }) = h
        .store
        .start_email_change(&credential, &address("tranh.chap@vi-du.vn"))
        .await
        .unwrap()
    else {
        panic!("change not started");
    };
    let code = h.code(&handle);
    let winner = h.sign_in("tranh.chap@vi-du.vn").await;
    assert_ne!(winner.account_id, current.account_id);
    assert_eq!(
        h.store
            .complete_email_change(&credential, &handle, &code)
            .await
            .unwrap(),
        EmailChangeOutcome::Rejected(ChallengeRejection::AddressUnavailable)
    );
    assert_eq!(
        h.sign_in("moi@vi-du.vn").await.account_id,
        current.account_id
    );

    // After logout the session can no longer complete anything.
    assert!(h.store.revoke(&credential).await.unwrap());
    assert_eq!(
        h.store
            .complete_email_change(&credential, &handle, &code)
            .await
            .unwrap(),
        EmailChangeOutcome::CredentialRejected
    );
}
