//! D-05 sign-in rate limits on real PostgreSQL: the count is exact under concurrency and shared
//! by separate App processes (separate pools); windows are independent per bucket and network;
//! the HTTP edge answers 429 with Retry-After from the shared counter; prune and the rollback
//! script touch nothing else.

use std::net::IpAddr;

use vink_app::application::ports::RateLimitStore;
use vink_app::domain::abuse::{Admission, Network, SIGN_IN_CHALLENGE};

use super::*;

fn network(raw: &str) -> Network {
    Network::of(raw.parse::<IpAddr>().unwrap())
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_requests_from_two_processes_are_counted_exactly(pool: PgPool) {
    // Two App processes: two pools on the same database.
    let options = pool.connect_options();
    let second = PgPoolOptions::new()
        .max_connections(10)
        .connect_with((*options).clone())
        .await
        .unwrap();
    let apps = [
        Arc::new(Vink::new(PgStore::new(pool.clone()))),
        Arc::new(Vink::new(PgStore::new(second))),
    ];
    let from = network("198.51.100.20");
    let started = Instant::now();
    let tasks: Vec<_> = (0..40)
        .map(|n| {
            let vink = Arc::clone(&apps[n % 2]);
            tokio::spawn(async move { vink.admit(&SIGN_IN_CHALLENGE, &from).await.unwrap() })
        })
        .collect();
    let mut admitted = 0;
    let mut refused = Vec::new();
    for task in tasks {
        match task.await.unwrap() {
            Admission::Admitted => admitted += 1,
            Admission::Refused {
                retry_after_seconds,
            } => refused.push(retry_after_seconds),
        }
    }
    let elapsed = started.elapsed();
    assert_eq!(
        admitted, SIGN_IN_CHALLENGE.limit,
        "exactly the limit passes"
    );
    assert_eq!(refused.len(), 40 - SIGN_IN_CHALLENGE.limit as usize);
    assert!(
        refused
            .iter()
            .all(|s| (1..=SIGN_IN_CHALLENGE.window_seconds as u32).contains(s))
    );
    let hits: i32 = sqlx::query_scalar("SELECT hits FROM auth_rate_limits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hits, 40, "every request is counted, refused ones too");
    println!("measure: 40 concurrent admissions over 2 pools: {elapsed:?}");

    // Other networks, the same /64 and other buckets.
    let vink = &apps[0];
    assert_eq!(
        vink.admit(&SIGN_IN_CHALLENGE, &network("198.51.100.21"))
            .await
            .unwrap(),
        Admission::Admitted
    );
    for n in 0..SIGN_IN_CHALLENGE.limit {
        let address = format!("2001:db8:7:7::{:x}", n + 1);
        vink.admit(&SIGN_IN_CHALLENGE, &network(&address))
            .await
            .unwrap();
    }
    assert!(
        matches!(
            vink.admit(&SIGN_IN_CHALLENGE, &network("2001:db8:7:7:ffff::1"))
                .await
                .unwrap(),
            Admission::Refused { .. }
        ),
        "rotating addresses inside one /64 does not escape the limit"
    );
    let complete = vink_app::domain::abuse::SIGN_IN_COMPLETE;
    assert_eq!(
        vink.admit(&complete, &from).await.unwrap(),
        Admission::Admitted,
        "buckets are independent"
    );
    // Keys are opaque: no address text is stored.
    let stored: Vec<Vec<u8>> = sqlx::query_scalar("SELECT network_key FROM auth_rate_limits")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(stored.iter().all(|key| key.len() == 32));
    // Single-request latency of the shared counter (local PostgreSQL).
    let mut samples = Vec::new();
    for n in 0..200 {
        let at = Instant::now();
        vink.admit(&complete, &network(&format!("192.0.2.{}", n % 250)))
            .await
            .unwrap();
        samples.push(at.elapsed());
    }
    samples.sort();
    println!(
        "measure: admit latency p50 {:?} p95 {:?} max {:?} (200 sequential)",
        samples[100], samples[190], samples[199]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_edge_answers_429_with_retry_after_from_the_shared_counter(pool: PgPool) {
    dev_seed::apply(&pool).await.unwrap();
    // Identity is unreachable: an admitted request answers 503, a refused one 429 without
    // trying Identity at all. A forged X-Forwarded-For from this direct peer is ignored.
    let unreachable = vink_app::identity::IdentityClient::new(
        std::path::PathBuf::from("/nonexistent/vink-identity.sock"),
        0,
    );
    let router = router(
        AppState::new(
            Vink::new(PgStore::new(pool.clone())),
            Arc::new(vink_app::identity::NoAuthority),
            ORIGIN,
            std::path::Path::new("target/site"),
        )
        .with_sign_in(Some(unreachable)),
    )
    .layer(axum::Extension(axum::extract::ConnectInfo(
        SocketAddr::from(([198, 51, 100, 30], 4000)),
    )));
    let mut answers = Vec::new();
    for n in 0..=SIGN_IN_CHALLENGE.limit {
        let response = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/sign-in/challenges")
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("x-forwarded-for", format!("192.0.2.{n}"))
                    .body(Body::from(r#"{"email":"a@vi-du.vn"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        answers.push((
            response.status(),
            response.headers().get(header::RETRY_AFTER).cloned(),
        ));
    }
    let (last, rest) = answers.split_last().unwrap();
    assert!(
        rest.iter()
            .all(|(status, retry)| *status == StatusCode::SERVICE_UNAVAILABLE && retry.is_none()),
        "{rest:?}"
    );
    assert_eq!(last.0, StatusCode::TOO_MANY_REQUESTS);
    let seconds: u32 = last.1.as_ref().unwrap().to_str().unwrap().parse().unwrap();
    assert!((1..=900).contains(&seconds));
    let rows: Vec<i32> = sqlx::query_scalar("SELECT hits FROM auth_rate_limits")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows, [11], "one bucket: the peer, not the forged header");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn password_routes_enforce_rate_limits_at_the_edge(pool: PgPool) {
    dev_seed::apply(&pool).await.unwrap();
    let unreachable = vink_app::identity::IdentityClient::new(
        std::path::PathBuf::from("/nonexistent/vink-identity.sock"),
        0,
    );
    let router = router(
        AppState::new(
            Vink::new(PgStore::new(pool.clone())),
            Arc::new(vink_app::identity::NoAuthority),
            ORIGIN,
            std::path::Path::new("target/site"),
        )
        .with_sign_in(Some(unreachable)),
    )
    .layer(axum::Extension(axum::extract::ConnectInfo(
        SocketAddr::from(([198, 51, 100, 31], 4000)),
    )));

    // Test PASSWORD_REGISTER rate limit (limit = 5).
    let mut answers = Vec::new();
    for _ in 0..=vink_app::domain::abuse::PASSWORD_REGISTER.limit {
        let response = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/register")
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"test_user","password":"test_password_123"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        answers.push((
            response.status(),
            response.headers().get(header::RETRY_AFTER).cloned(),
        ));
    }
    let (last, rest) = answers.split_last().unwrap();
    assert!(
        rest.iter()
            .all(|(status, retry)| *status == StatusCode::SERVICE_UNAVAILABLE && retry.is_none()),
        "admitted requests forward to unreachable identity (503)"
    );
    assert_eq!(last.0, StatusCode::TOO_MANY_REQUESTS);
    assert!(last.1.is_some(), "429 answers carry Retry-After");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn prune_and_rollback_touch_only_rate_limit_windows(pool: PgPool) {
    let vink = seeded(&pool).await;
    vink.admit(&SIGN_IN_CHALLENGE, &network("198.51.100.40"))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO auth_rate_limits (bucket, network_key, window_start, window_seconds, hits)
         VALUES ('sign_in.challenge', decode(repeat('ab', 32), 'hex'),
                 now() - interval '3 days', 900, 7)",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        PgStore::new(pool.clone())
            .prune_rate_limits()
            .await
            .unwrap(),
        1
    );
    assert_eq!(vink.prune_rate_limits().await.unwrap(), 0, "idempotent");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_rate_limits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 1, "the current window stays");

    let principals_before: i64 = sqlx::query_scalar("SELECT count(*) FROM principals")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Rollbacks run newest first. Operator post-review is unused here; its down script must
    // preserve debt/audit whenever present, covered by post_review's dedicated tests.
    for down in [
        "migrations-rollback/20261117000000_work_moderation_runtime.down.sql",
        "migrations-rollback/20261116000000_work_publication_persistence.down.sql",
        "migrations-rollback/20261115000000_comment_post_review.down.sql",
        "migrations-rollback/20261114000000_discussion_outbox.down.sql",
        "migrations-rollback/20261113000000_own_comment_overlay.down.sql",
        "migrations-rollback/20261112000000_auth_rate_limits.down.sql",
    ] {
        let script =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(down))
                .unwrap();
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(script))
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    let table: Option<String> = sqlx::query_scalar("SELECT to_regclass('auth_rate_limits')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(table, None);
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(latest, 20261111000000);
    let principals_after: i64 = sqlx::query_scalar("SELECT count(*) FROM principals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(principals_after, principals_before);
}
