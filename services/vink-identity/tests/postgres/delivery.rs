//! The mail outbox and its worker on real PostgreSQL, against a real SMTP conversation on
//! loopback (`support/smtp_sink.rs`). This proves the outbox, the worker and the SMTP adapter
//! contract; it is not a deliverability check (no TLS, DNS, SPF/DKIM or remote mailbox).

use std::sync::Arc;
use std::time::Duration;

use sqlx::{PgPool, Row};
use tokio::sync::{Notify, watch};
use uuid::Uuid;
use vink_identity::accounts::{
    self, AccountStore, ChallengeOutcome, EmailChangeStart, Retention, SignInOutcome, Throttle,
};
use vink_identity::delivery::{self, DeliveryPolicy, Final, Worker};
use vink_identity::email_code::{ChallengeHandle, Code, EmailAddress, Policy};
use vink_identity::mail::{Smtp, SmtpSecurity, SmtpSettings, Transport};

use crate::smtp_sink::{Reply, Sink};

fn smtp(sink: &Sink) -> Transport {
    Arc::new(
        Smtp::new(SmtpSettings {
            host: "127.0.0.1".into(),
            port: sink.port(),
            security: SmtpSecurity::Plaintext,
            username: None,
            password_file: None,
            from: "Vink <no-reply@vink.test>".into(),
            timeout: Duration::from_secs(2),
        })
        .unwrap(),
    )
}

struct Rig {
    pool: PgPool,
    store: AccountStore,
    sink: Sink,
    wake: Arc<Notify>,
}

impl Rig {
    async fn new(pool: &PgPool, policy: Policy, reply: Reply) -> Self {
        let wake = Arc::new(Notify::new());
        Self {
            pool: pool.clone(),
            store: AccountStore::new(pool.clone(), policy, wake.clone()),
            sink: Sink::start(reply).await,
            wake,
        }
    }

    fn worker(&self) -> Worker {
        Worker::new(
            self.pool.clone(),
            smtp(&self.sink),
            DeliveryPolicy::default(),
            self.wake.clone(),
        )
    }

    async fn challenge(&self, raw: &str) -> ChallengeHandle {
        match self
            .store
            .start_sign_in(&EmailAddress::parse(raw).unwrap())
            .await
            .unwrap()
        {
            ChallengeOutcome::Issued { handle, .. } => handle,
            ChallengeOutcome::RateLimited(throttle) => panic!("rate limited: {throttle:?}"),
        }
    }

    async fn row(&self, handle: &ChallengeHandle) -> (String, Option<String>, i32, Option<String>) {
        let row = sqlx::query(
            "SELECT state, code, attempts, last_error FROM mail_deliveries WHERE challenge_id = $1",
        )
        .bind(handle.id)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        (
            row.get("state"),
            row.get("code"),
            row.get("attempts"),
            row.get("last_error"),
        )
    }

    /// Makes every pending row due now (a test clock for backoff and leases).
    async fn make_due(&self) {
        sqlx::query("UPDATE mail_deliveries SET next_attempt_at = now() WHERE state = 'pending'")
            .execute(&self.pool)
            .await
            .unwrap();
    }
}

/// The code inside a received message body.
fn code_in(data: &str) -> String {
    let marker = "của bạn là ";
    let start = data.find(marker).expect("code sentence") + marker.len();
    data[start..start + 6].to_string()
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_challenge_commits_with_its_delivery_and_the_worker_sends_it_once(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    let handle = rig.challenge("Nguoi.Nhan@Vi-Du.vn").await;
    let (state, code, attempts, _) = rig.row(&handle).await;
    assert_eq!((state.as_str(), attempts), ("pending", 0));
    let pending = code.expect("the code waits for the worker");

    let worker = rig.worker();
    assert_eq!(worker.run_once().await.unwrap(), 1);
    let mails = rig.sink.messages();
    assert_eq!(mails.len(), 1);
    assert_eq!(mails[0].rcpt, vec!["<nguoi.nhan@vi-du.vn>".to_string()]);
    assert_eq!(code_in(&mails[0].text()), pending);
    // The mailed code answers the challenge (with the client-held handle).
    let code = Code::parse(&pending).unwrap();
    assert!(matches!(
        rig.store.complete_sign_in(&handle, &code).await.unwrap(),
        SignInOutcome::Authenticated(_)
    ));
    // Terminal state clears the code; nothing is sent twice.
    let (state, code, attempts, _) = rig.row(&handle).await;
    assert_eq!((state.as_str(), code, attempts), ("sent", None, 1));
    assert_eq!(worker.run_once().await.unwrap(), 0);
    assert_eq!(rig.sink.messages().len(), 1);
    // A refused challenge (throttle) leaves neither a challenge nor a delivery behind.
    let throttled = Rig::new(
        &pool,
        Policy {
            max_challenges_per_window: 1,
            ..Policy::default()
        },
        Reply::Accept,
    )
    .await;
    assert!(matches!(
        throttled
            .store
            .start_sign_in(&EmailAddress::parse("nguoi.nhan@vi-du.vn").unwrap())
            .await
            .unwrap(),
        ChallengeOutcome::RateLimited(Throttle::Address)
    ));
    let orphans: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM email_challenges) - (SELECT count(*) FROM mail_deliveries)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        orphans, 0,
        "every committed sign-in challenge has its delivery"
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn transient_failures_back_off_and_recover_permanent_ones_end_at_once(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Unavailable).await;
    let worker = rig.worker();
    let handle = rig.challenge("tam.thoi@vi-du.vn").await;
    worker.run_once().await.unwrap();
    let (state, code, attempts, error) = rig.row(&handle).await;
    assert_eq!(
        (state.as_str(), attempts, error.as_deref()),
        ("pending", 1, Some("transient"))
    );
    assert!(code.is_some());
    // Backoff: not due again yet.
    assert_eq!(worker.run_once().await.unwrap(), 0);
    let wait: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM next_attempt_at - now())::float8 FROM mail_deliveries",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!((4.0..=11.0).contains(&wait), "{wait}");
    // The relay recovers: the same code is delivered.
    rig.sink.set(Reply::Accept);
    rig.make_due().await;
    worker.run_once().await.unwrap();
    let (state, code, attempts, _) = rig.row(&handle).await;
    assert_eq!((state.as_str(), code, attempts), ("sent", None, 2));
    assert_eq!(rig.sink.messages().len(), 1);

    // 5xx: no retry, code cleared, status kept for operators (never the reply text).
    rig.sink.set(Reply::RejectRcpt(550));
    let refused = rig.challenge("khong.ton.tai@vi-du.vn").await;
    worker.run_once().await.unwrap();
    let (state, code, attempts, error) = rig.row(&refused).await;
    assert_eq!(
        (state.as_str(), code, attempts, error.as_deref()),
        ("failed", None, 1, Some("permanent"))
    );
    let status: Option<i16> =
        sqlx::query_scalar("SELECT last_status FROM mail_deliveries WHERE challenge_id = $1")
            .bind(refused.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, Some(550));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn attempts_are_bounded_and_used_or_expired_codes_are_never_sent(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Hang).await;
    let worker = Worker::new(
        pool.clone(),
        Arc::new(
            Smtp::new(SmtpSettings {
                host: "127.0.0.1".into(),
                port: rig.sink.port(),
                security: SmtpSecurity::Plaintext,
                username: None,
                password_file: None,
                from: "Vink <no-reply@vink.test>".into(),
                timeout: Duration::from_millis(200),
            })
            .unwrap(),
        ),
        DeliveryPolicy::default(),
        rig.wake.clone(),
    );
    let handle = rig.challenge("im.lang@vi-du.vn").await;
    for attempt in 1..=DeliveryPolicy::default().max_attempts {
        rig.make_due().await;
        assert_eq!(worker.run_once().await.unwrap(), 1, "attempt {attempt}");
    }
    let (state, code, attempts, error) = rig.row(&handle).await;
    assert_eq!(
        (state.as_str(), code, attempts, error.as_deref()),
        ("failed", None, 5, Some("timeout"))
    );
    rig.make_due().await;
    assert_eq!(worker.run_once().await.unwrap(), 0, "a failed row is final");

    // Expired before the worker ran (worker down for 10 minutes): cancelled, not sent.
    rig.sink.set(Reply::Accept);
    let late = rig.challenge("tre@vi-du.vn").await;
    sqlx::query(
        "UPDATE email_challenges SET created_at = now() - interval '20 minutes',
                expires_at = now() - interval '10 minutes' WHERE id = $1",
    )
    .bind(late.id)
    .execute(&pool)
    .await
    .unwrap();
    rig.worker().run_once().await.unwrap();
    let (state, code, _, error) = rig.row(&late).await;
    assert_eq!(
        (state.as_str(), code, error.as_deref()),
        ("cancelled", None, Some("expired"))
    );
    assert!(rig.sink.messages().is_empty());
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn a_crashed_worker_is_recovered_after_its_lease_at_least_once(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    let handle = rig.challenge("phuc.hoi@vi-du.vn").await;
    // Worker A claims, then "crashes" without sending or recording anything.
    let crashed = delivery::claim(&pool, &DeliveryPolicy::default())
        .await
        .unwrap();
    assert_eq!(crashed.len(), 1);
    // While A's lease holds, nobody else sends that code.
    let b = rig.worker();
    assert_eq!(b.run_once().await.unwrap(), 0);
    // After the lease, B delivers it.
    rig.make_due().await;
    assert_eq!(b.run_once().await.unwrap(), 1);
    assert_eq!(rig.sink.messages().len(), 1);
    // A comes back with its stale claim: its outcome is not recorded.
    assert!(
        !delivery::finish(&pool, &crashed[0], Final::Failed(None))
            .await
            .unwrap()
    );
    assert_eq!(rig.row(&handle).await.0, "sent");

    // A stale claim racing a live one on a row that is still pending: only the holder of the
    // current lease token may record an outcome.
    let raced = rig.challenge("dua.lease@vi-du.vn").await;
    let policy = DeliveryPolicy::default();
    let stale = delivery::claim(&pool, &policy).await.unwrap();
    rig.make_due().await;
    let live = delivery::claim(&pool, &policy).await.unwrap();
    assert_eq!((stale.len(), live.len()), (1, 1));
    assert!(
        !delivery::finish(&pool, &stale[0], Final::Failed(None))
            .await
            .unwrap(),
        "an expired lease records nothing"
    );
    assert_eq!(rig.row(&raced).await.0, "pending");
    assert!(
        delivery::finish(&pool, &live[0], Final::Sent)
            .await
            .unwrap()
    );
    assert_eq!(rig.row(&raced).await.0, "sent");

    // Crash AFTER the relay accepted but before the outcome committed: the next claim sends the
    // same code again under the same Message-ID (at-least-once, documented).
    let again = rig.challenge("trung.lap@vi-du.vn").await;
    let claimed = delivery::claim(&pool, &DeliveryPolicy::default())
        .await
        .unwrap();
    let transport = smtp(&rig.sink);
    transport
        .send(claimed[0].message.as_ref().unwrap())
        .await
        .unwrap();
    rig.make_due().await;
    b.run_once().await.unwrap();
    let mails: Vec<_> = rig
        .sink
        .messages()
        .into_iter()
        .filter(|m| m.data.contains(&again.id.simple().to_string()))
        .collect();
    assert_eq!(mails.len(), 2, "one duplicate, never more per crash");
    assert_eq!(code_in(&mails[0].text()), code_in(&mails[1].text()));
    assert_eq!(rig.row(&again).await.0, "sent");
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn concurrent_workers_never_send_one_claim_twice(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    let mut ids = Vec::new();
    for n in 0..40 {
        ids.push(rig.challenge(&format!("nguoi{n}@vi-du.vn")).await.id);
    }
    let started = std::time::Instant::now();
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let worker = rig.worker();
            tokio::spawn(async move {
                let mut claimed = 0;
                loop {
                    match worker.run_once().await.unwrap() {
                        0 => break claimed,
                        n => claimed += n,
                    }
                }
            })
        })
        .collect();
    let mut total = 0;
    for worker in workers {
        total += worker.await.unwrap();
    }
    let elapsed = started.elapsed();
    assert_eq!(total, 40);
    let mails = rig.sink.messages();
    assert_eq!(mails.len(), 40);
    for id in ids {
        let copies = mails
            .iter()
            .filter(|m| m.data.contains(&id.simple().to_string()))
            .count();
        assert_eq!(copies, 1, "{id}");
    }
    let sent: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_deliveries WHERE state = 'sent'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sent, 40);
    println!(
        "measure: 40 deliveries, 4 workers, loopback SMTP: {:?} ({} SMTP connections)",
        elapsed,
        rig.sink.connections()
    );
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_running_worker_wakes_on_commit_and_stops_on_shutdown(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    let (stop, stopped) = watch::channel(false);
    let task = tokio::spawn(rig.worker().run(stopped));
    let handle = rig.challenge("ngay.lap.tuc@vi-du.vn").await;
    let started = std::time::Instant::now();
    while rig.row(&handle).await.0 != "sent" {
        assert!(started.elapsed() < Duration::from_secs(5), "not delivered");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    println!("measure: commit → relay accepted: {:?}", started.elapsed());
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("worker stops on shutdown")
        .unwrap();
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn global_and_per_account_ceilings_hold(pool: PgPool) {
    let rig = Rig::new(
        &pool,
        Policy {
            max_challenges_globally: 5,
            ..Policy::default()
        },
        Reply::Accept,
    )
    .await;
    let tasks: Vec<_> = (0..8)
        .map(|n| {
            let store = rig.store.clone();
            tokio::spawn(async move {
                store
                    .start_sign_in(&EmailAddress::parse(&format!("xoay{n}@vi-du.vn")).unwrap())
                    .await
                    .unwrap()
            })
        })
        .collect();
    let mut issued = 0;
    for task in tasks {
        match task.await.unwrap() {
            ChallengeOutcome::Issued { .. } => issued += 1,
            ChallengeOutcome::RateLimited(throttle) => assert_eq!(throttle, Throttle::Global),
        }
    }
    // Soft ceiling: concurrent requests may pass it by at most their number (documented).
    assert!((5..=8).contains(&issued), "{issued}");
    assert!(matches!(
        rig.store
            .start_sign_in(&EmailAddress::parse("them@vi-du.vn").unwrap())
            .await
            .unwrap(),
        ChallengeOutcome::RateLimited(Throttle::Global)
    ));

    // Per account: three changes an hour, whatever the target addresses.
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    sqlx::raw_sql("DELETE FROM mail_deliveries; DELETE FROM email_challenges")
        .execute(&pool)
        .await
        .unwrap();
    let handle = rig.challenge("chu@vi-du.vn").await;
    rig.worker().run_once().await.unwrap();
    let code = Code::parse(&code_in(&rig.sink.messages()[0].text())).unwrap();
    let SignInOutcome::Authenticated(session) =
        rig.store.complete_sign_in(&handle, &code).await.unwrap()
    else {
        panic!("sign-in")
    };
    let credential = session.credential.encode();
    let mut outcomes = Vec::new();
    for n in 0..4 {
        let target = EmailAddress::parse(&format!("moi{n}@vi-du.vn")).unwrap();
        outcomes.push(
            match rig
                .store
                .start_email_change(&credential, &target)
                .await
                .unwrap()
            {
                EmailChangeStart::Issued(ChallengeOutcome::Issued { .. }) => "issued",
                EmailChangeStart::Issued(ChallengeOutcome::RateLimited(Throttle::Account)) => {
                    "account"
                }
                other => panic!("{other:?}"),
            },
        );
    }
    assert_eq!(outcomes, ["issued", "issued", "issued", "account"]);
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn prune_deletes_only_closed_records(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    let old = rig.challenge("cu.lam@vi-du.vn").await;
    let fresh = rig.challenge("con.moi@vi-du.vn").await;
    rig.worker().run_once().await.unwrap();
    let mails = rig.sink.messages();
    let mut sessions = Vec::new();
    for handle in [&old, &fresh] {
        // Sends run concurrently: find each code by its Message-ID, not by arrival order.
        let mail = mails
            .iter()
            .find(|m| m.data.contains(&handle.id.simple().to_string()))
            .unwrap();
        match rig
            .store
            .complete_sign_in(handle, &Code::parse(&code_in(&mail.text())).unwrap())
            .await
            .unwrap()
        {
            SignInOutcome::Authenticated(session) => sessions.push(session),
            SignInOutcome::Rejected(reason) => panic!("{reason:?}"),
        }
    }
    let open = rig.challenge("dang.mo@vi-du.vn").await;
    // Age the first challenge and session past their retention.
    sqlx::query(
        "UPDATE email_challenges SET created_at = now() - interval '9 days',
                expires_at = now() - interval '8 days' WHERE id = $1",
    )
    .bind(old.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE sessions SET created_at = now() - interval '70 days',
                expires_at = now() - interval '40 days', revoked_at = now() - interval '45 days',
                revoked_reason = 'logout' WHERE id = $1",
    )
    .bind(sessions[0].session_id)
    .execute(&pool)
    .await
    .unwrap();
    let pruned = accounts::prune(&pool, Retention::default()).await.unwrap();
    assert_eq!((pruned.challenges, pruned.sessions), (1, 1));
    let remaining: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM email_challenges ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, {
        let mut ids = vec![fresh.id, open.id];
        ids.sort();
        ids
    });
    let deliveries: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deliveries, 2, "the old delivery went with its challenge");
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(accounts, 2, "accounts are never pruned");
    assert!(
        rig.store
            .verify(&sessions[1].credential.encode())
            .await
            .unwrap()
            .is_ok()
    );
    // Idempotent.
    let again = accounts::prune(&pool, Retention::default()).await.unwrap();
    assert_eq!((again.challenges, again.sessions), (0, 0));
}

#[sqlx::test(migrator = "vink_identity::persistence::MIGRATOR")]
async fn the_rollback_script_drops_only_the_outbox(pool: PgPool) {
    let rig = Rig::new(&pool, Policy::default(), Reply::Accept).await;
    let handle = rig.challenge("giu.lai@vi-du.vn").await;
    rig.worker().run_once().await.unwrap();
    let code = Code::parse(&code_in(&rig.sink.messages()[0].text())).unwrap();
    let SignInOutcome::Authenticated(session) =
        rig.store.complete_sign_in(&handle, &code).await.unwrap()
    else {
        panic!("sign-in")
    };
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261112000000_mail_outbox.down.sql"),
    )
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(script))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(versions, [20261102000000, 20261107000000]);
    let outbox: Option<String> = sqlx::query_scalar("SELECT to_regclass('mail_deliveries')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(outbox, None);
    // The previous binary's queries still work on what is left: the session verifies.
    let still: bool = sqlx::query_scalar(
        "SELECT revoked_at IS NULL AND expires_at > now() FROM sessions WHERE id = $1",
    )
    .bind(session.session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(still, "rollback signs nobody out");
}
