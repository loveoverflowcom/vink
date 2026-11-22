//! Workload measurements for #347 AC-06 (ignored by default; run by hand):
//!
//! `cargo test -p vink-notifications --test postgres budget:: -- --ignored --nocapture --test-threads=1`
//!
//! Each test prints its numbers and asserts the published budget (docs/development/notifications.md
//! §7.7) with the measured value, so a regression fails loudly. Volumes are synthetic: IDs only.

use std::time::{Duration, Instant};

use sqlx::postgres::PgPoolOptions;

use super::*;

const WRITERS: usize = 8;

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

async fn wal_lsn(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT pg_current_wal_lsn()::text")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn wal_bytes(pool: &PgPool, from: &str) -> i64 {
    sqlx::query_scalar("SELECT pg_wal_lsn_diff(pg_current_wal_lsn(), $1::pg_lsn)::bigint")
        .bind(from)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Ingests `events` with [`WRITERS`] concurrent writers; returns (elapsed, sorted latencies).
async fn drive(pool: &PgPool, events: Vec<(Uuid, String)>) -> (Duration, Vec<Duration>) {
    let wide = PgPoolOptions::new()
        .max_connections(WRITERS as u32 + 2)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let queue = std::sync::Arc::new(std::sync::Mutex::new(events));
    let started = Instant::now();
    let mut tasks = Vec::new();
    for _ in 0..WRITERS {
        let (pool, queue) = (wide.clone(), queue.clone());
        tasks.push(tokio::spawn(async move {
            let mut latencies = Vec::new();
            loop {
                let next = queue.lock().unwrap().pop();
                let Some((event_id, text)) = next else { break };
                let at = Instant::now();
                let ingested = store::ingest(&pool, event_id, &text, T0).await;
                assert_eq!(ingested.status, IngestStatus::Applied);
                latencies.push(at.elapsed());
            }
            latencies
        }));
    }
    let mut latencies = Vec::new();
    for task in tasks {
        latencies.extend(task.await.unwrap());
    }
    let elapsed = started.elapsed();
    latencies.sort_unstable();
    wide.close().await;
    (elapsed, latencies)
}

fn report(label: &str, elapsed: Duration, latencies: &[Duration], wal: i64) -> (f64, Duration) {
    let rate = latencies.len() as f64 / elapsed.as_secs_f64();
    let p99 = percentile(latencies, 0.99);
    println!(
        "{label}: {} events in {:.2}s = {rate:.0}/s; p50 {:.1} ms, p95 {:.1} ms, p99 {:.1} ms; WAL {:.0} B/event",
        latencies.len(),
        elapsed.as_secs_f64(),
        percentile(latencies, 0.50).as_secs_f64() * 1e3,
        percentile(latencies, 0.95).as_secs_f64() * 1e3,
        p99.as_secs_f64() * 1e3,
        wal as f64 / latencies.len() as f64
    );
    (rate, p99)
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
#[ignore = "measurement: run by hand with --ignored --nocapture"]
async fn ingest_throughput_spread_and_hot_key(pool: PgPool) {
    const EVENTS: u32 = 2_000;
    // Spread: 200 recipients, ten works each, one comment per event.
    let spread: Vec<(Uuid, String)> = (0..EVENTS)
        .map(|n| {
            let recipient = Uuid::from_u128(0x1000 + u128::from(n % 200));
            let work = Uuid::from_u128(0x2000 + u128::from(n % 2000 / 200));
            let actor = Uuid::from_u128(0x3000 + u128::from(n));
            let comment = Uuid::from_u128(0x4000 + u128::from(n));
            let event = event_id();
            (event, root(event, actor, recipient, work, comment, 2, T0))
        })
        .collect();
    let before = wal_lsn(&pool).await;
    let (elapsed, latencies) = drive(&pool, spread).await;
    let wal = wal_bytes(&pool, &before).await;
    let (spread_rate, spread_p99) = report("spread (200 inboxes)", elapsed, &latencies, wal);

    // Hot key: every event is a first Thấm on the same revision of one author.
    let author = Uuid::from_u128(0x9000);
    let (work, revision) = (Uuid::from_u128(0x9001), Uuid::from_u128(0x9002));
    let hot: Vec<(Uuid, String)> = (0..EVENTS)
        .map(|n| {
            let actor = Uuid::from_u128(0x5000 + u128::from(n));
            let event = event_id();
            (
                event,
                tham(
                    event,
                    actor,
                    author,
                    work,
                    revision,
                    Some(true),
                    1,
                    true,
                    T0,
                ),
            )
        })
        .collect();
    let before = wal_lsn(&pool).await;
    let (elapsed, latencies) = drive(&pool, hot).await;
    let wal = wal_bytes(&pool, &before).await;
    let (hot_rate, hot_p99) = report("hot key (1 inbox, 1 group)", elapsed, &latencies, wal);
    let group = page(&pool, author, T0).await.groups.remove(0);
    assert_eq!(group.actor_count, EVENTS as i32);

    // Budgets (§7.7), with headroom for a shared development machine.
    assert!(spread_rate >= 200.0, "spread throughput");
    assert!(spread_p99 <= Duration::from_millis(250), "spread p99");
    assert!(hot_rate >= 100.0, "hot-key throughput");
    assert!(hot_p99 <= Duration::from_millis(500), "hot-key p99");
    assert!(wal / i64::from(EVENTS) <= 8 * 1024, "WAL per event");
}

/// Bulk rows for storage and plan measurements: `inboxes` inboxes of `groups` groups each, three
/// members per group, plus one inbox of `hot` groups. Mostly read groups, one unread in ten.
async fn seed(pool: &PgPool, inboxes: i64, groups: i64, hot: i64) {
    sqlx::query(
        "INSERT INTO inboxes (recipient, last_seq, seen_seq, created_at, updated_at)
         SELECT ('00000000-0000-4000-8000-' || lpad(to_hex(r), 12, '0'))::uuid,
                CASE WHEN r = 0 THEN $3 ELSE $2 END * 3, 0, $4, $4
         FROM generate_series(0, $1) AS r",
    )
    .bind(inboxes)
    .bind(groups)
    .bind(hot)
    .bind(T0)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO notification_groups
             (group_id, recipient, event_type, target_key, target, target_order, work_id,
              publication_epoch, open, created_at, expires_at, last_signal_at, group_version,
              signal_seq, first_seq, read_through, actor_count, item_count, actors_preview)
         SELECT gen_random_uuid(), i.recipient, 'comment.root.approved', 'work/' || g,
                jsonb_build_object('kind', 'work_comment_thread', 'work_id', gen_random_uuid(),
                                   'comment_id', gen_random_uuid(), 'moderation_version', 2),
                2, gen_random_uuid(), 1, false, $3, $3 + interval '180 days', $3,
                g * 3, g * 3, g * 3 - 2, CASE WHEN g % 10 = 0 THEN 0 ELSE g * 3 END,
                3, 0, ARRAY[gen_random_uuid(), gen_random_uuid(), gen_random_uuid()]
         FROM inboxes i
         CROSS JOIN LATERAL generate_series(1, CASE WHEN i.recipient =
             '00000000-0000-4000-8000-000000000000'::uuid THEN $2 ELSE $1 END) AS g",
    )
    .bind(groups)
    .bind(hot)
    .bind(T0)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO group_members (group_id, contribution, actor_id, active, source_order, last_seq)
         SELECT g.group_id, gen_random_uuid(), a, true, 2, g.signal_seq
         FROM notification_groups g CROSS JOIN LATERAL unnest(g.actors_preview) AS a",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("ANALYZE").execute(pool).await.unwrap();
}

async fn explain(pool: &PgPool, label: &str, sql: &str, recipient: Uuid) -> f64 {
    let lines: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "EXPLAIN (ANALYZE, BUFFERS) {sql}"
    )))
    .bind(recipient)
    .bind(T0)
    .fetch_all(pool)
    .await
    .unwrap();
    println!("--- {label}");
    for line in &lines {
        println!("    {line}");
    }
    lines
        .iter()
        .find_map(|line| line.trim().strip_prefix("Execution Time: "))
        .and_then(|rest| rest.trim_end_matches(" ms").parse().ok())
        .unwrap()
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
#[ignore = "measurement: run by hand with --ignored --nocapture"]
async fn storage_plans_and_purge_at_100k_groups(pool: PgPool) {
    const INBOXES: i64 = 1_000;
    const GROUPS: i64 = 100;
    const HOT: i64 = 10_000;
    seed(&pool, INBOXES, GROUPS, HOT).await;
    let sizes: Vec<(String, i64)> = sqlx::query_as(
        "SELECT relname::text, pg_total_relation_size(oid) FROM pg_class
         WHERE relname IN ('notification_groups', 'group_members', 'inboxes') ORDER BY relname",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let total_groups = INBOXES * GROUPS + HOT;
    let mut per_group = 0;
    for (table, bytes) in &sizes {
        println!("{table}: {:.1} MiB", *bytes as f64 / 1048576.0);
        if table != "inboxes" {
            per_group += bytes / total_groups;
        }
    }
    println!("per group with 3 members (tables + indexes): {per_group} B");

    let hot = Uuid::parse_str("00000000-0000-4000-8000-000000000000").unwrap();
    let normal = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let count_sql = "SELECT i.last_seq,
            (SELECT count(*) FROM notification_groups g
              WHERE g.recipient = i.recipient AND g.expires_at > $2
                AND g.signal_seq > g.read_through AND g.signal_seq > i.seen_seq
                AND g.actor_count + g.item_count > 0) AS unseen
         FROM inboxes i WHERE i.recipient = $1";
    let page_sql = "SELECT group_id, signal_seq FROM notification_groups
         WHERE recipient = $1 AND expires_at > $2 AND actor_count + item_count > 0
           AND (signal_seq, group_id) < (9223372036854775807, 'ffffffff-ffff-ffff-ffff-ffffffffffff'::uuid)
         ORDER BY signal_seq DESC, group_id DESC LIMIT 21";
    let count_hot = explain(&pool, "badge count, 10k-group inbox", count_sql, hot).await;
    let count_normal = explain(&pool, "badge count, 100-group inbox", count_sql, normal).await;
    let page_hot = explain(&pool, "first page, 10k-group inbox", page_sql, hot).await;

    // Purge: every group of 500 inboxes ages out (50k groups, 150k members).
    sqlx::query(
        "UPDATE notification_groups SET created_at = $1 - interval '200 days',
             expires_at = $1 - interval '20 days', last_signal_at = $1 - interval '200 days'
         WHERE recipient IN (SELECT recipient FROM inboxes ORDER BY recipient LIMIT 500 OFFSET 1)",
    )
    .bind(T0)
    .execute(&pool)
    .await
    .unwrap();
    let before = wal_lsn(&pool).await;
    let started = Instant::now();
    let purged = store::purge(&pool, T0, 1_000, 1_000).await.unwrap();
    let elapsed = started.elapsed();
    let wal = wal_bytes(&pool, &before).await;
    let rate = purged.groups as f64 / elapsed.as_secs_f64();
    println!(
        "purge: {} groups (with members) in {:.2}s = {rate:.0} groups/s, batches of 1000; WAL {:.1} MiB",
        purged.groups,
        elapsed.as_secs_f64(),
        wal as f64 / 1048576.0
    );
    assert_eq!(purged.groups, 50_000);

    // Budgets (§7.7).
    assert!(count_hot <= 50.0 && count_normal <= 5.0 && page_hot <= 20.0);
    assert!(per_group <= 4 * 1024);
    assert!(rate >= 2_000.0);
}
