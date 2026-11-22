//! Badge, Seen, Read and MarkAllRead by snapshot (AC-04), through the store and the inbox route.

use super::*;

const OWNER: u8 = 0x0a;
const WORK: u8 = 0xaa;

async fn signal(pool: &PgPool, actor: u8, comment: u8) {
    applied(
        pool,
        &root(
            event_id(),
            id(actor),
            id(OWNER),
            id(WORK),
            id(comment),
            2,
            T0,
        ),
        T0,
    )
    .await;
}

async fn signal_on(pool: &PgPool, actor: u8, work: u8, comment: u8) {
    applied(
        pool,
        &root(
            event_id(),
            id(actor),
            id(OWNER),
            id(work),
            id(comment),
            2,
            T0,
        ),
        T0,
    )
    .await;
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn seen_covers_only_its_snapshot_and_an_old_ack_changes_nothing(pool: PgPool) {
    let owner = id(OWNER);
    signal_on(&pool, 1, 0xa1, 0x31).await;
    let s1 = page(&pool, owner, T0).await.snapshot;
    // A new group commits after the bell opened at S1.
    signal_on(&pool, 2, 0xa2, 0x32).await;
    assert_eq!(badge(&pool, owner, T0).await, 2);
    let answer = run(
        &pool,
        owner,
        "seen-1",
        Command::Seen { up_to: s1.token() },
        T0,
    )
    .await;
    assert_eq!(answer["seen_snapshot"], s1.token());
    assert_eq!(
        badge(&pool, owner, T0).await,
        1,
        "the S2 group still counts"
    );
    // Seen never marks anything read.
    assert!(page(&pool, owner, T0).await.groups.iter().all(|g| g.unread));
    let s2 = page(&pool, owner, T0).await.snapshot;
    run(
        &pool,
        owner,
        "seen-2",
        Command::Seen { up_to: s2.token() },
        T0,
    )
    .await;
    assert_eq!(badge(&pool, owner, T0).await, 0);
    // Tab A's late Seen(S1) is a successful no-op.
    let late = run(
        &pool,
        owner,
        "seen-1b",
        Command::Seen { up_to: s1.token() },
        T0,
    )
    .await;
    assert_eq!(late["seen_snapshot"], s2.token());
    // A future watermark is refused.
    let future = store::command(
        &pool,
        owner,
        "seen-3",
        &Command::Seen {
            up_to: "s999".into(),
        },
        T0,
    )
    .await;
    assert_eq!(future, Err(store::InboxError::InvalidSnapshot));
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn reading_an_observed_version_keeps_newer_signals_unread_across_two_tabs(pool: PgPool) {
    let owner = id(OWNER);
    signal(&pool, 1, 0x31).await;
    let v1 = page(&pool, owner, T0).await.groups.remove(0);
    // Tab B: a new signal on the same group (v2) before tab A's Read(v1) arrives.
    signal(&pool, 2, 0x32).await;
    let read = run(
        &pool,
        owner,
        "a-read",
        Command::Read {
            group_id: v1.group_id,
            observed_version: v1.group_version,
        },
        T0,
    )
    .await;
    assert_eq!(read["unread"], true);
    let now = page(&pool, owner, T0).await.groups.remove(0);
    assert!(now.unread, "the v2 signal stays unread");
    assert_eq!(
        now.group_id, v1.group_id,
        "partly read: still the open group"
    );
    // Tab B reads the latest; tab A's ACK retried late keeps that result.
    run(
        &pool,
        owner,
        "b-read",
        Command::Read {
            group_id: now.group_id,
            observed_version: now.group_version,
        },
        T0,
    )
    .await;
    let retried = run(
        &pool,
        owner,
        "a-read",
        Command::Read {
            group_id: v1.group_id,
            observed_version: v1.group_version,
        },
        T0,
    )
    .await;
    assert_eq!(
        retried, read,
        "the same key and request replays the stored answer"
    );
    assert!(!page(&pool, owner, T0).await.groups[0].unread);
    // A silent change after the observed version does not keep the group unread.
    signal_on(&pool, 3, 0xab, 0x33).await;
    let tham_owner = id(OWNER);
    applied(
        &pool,
        &tham(
            event_id(),
            id(4),
            tham_owner,
            id(0xa9),
            id(0xe9),
            Some(true),
            1,
            true,
            T0,
        ),
        T0,
    )
    .await;
    applied(
        &pool,
        &tham(
            event_id(),
            id(5),
            tham_owner,
            id(0xa9),
            id(0xe9),
            Some(true),
            1,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let seen = page(&pool, owner, T0)
        .await
        .groups
        .into_iter()
        .find(|g| g.event_type == "work.endorsement.tham.changed")
        .unwrap();
    applied(
        &pool,
        &tham(
            event_id(),
            id(5),
            tham_owner,
            id(0xa9),
            id(0xe9),
            None,
            2,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let answer = run(
        &pool,
        owner,
        "silent",
        Command::Read {
            group_id: seen.group_id,
            observed_version: seen.group_version,
        },
        T0,
    )
    .await;
    assert_eq!(answer["unread"], false);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn mark_all_read_stops_at_its_snapshot(pool: PgPool) {
    let owner = id(OWNER);
    signal_on(&pool, 1, 0xa1, 0x31).await;
    signal_on(&pool, 2, 0xa2, 0x32).await;
    let s1 = page(&pool, owner, T0).await.snapshot;
    // After S1: a new signal on an existing group and a brand new group.
    signal_on(&pool, 3, 0xa1, 0x33).await;
    signal_on(&pool, 4, 0xa3, 0x34).await;
    let answer = run(
        &pool,
        owner,
        "all",
        Command::MarkAllRead { up_to: s1.token() },
        T0,
    )
    .await;
    assert_eq!(answer["groups_marked"], 2);
    let groups = page(&pool, owner, T0).await.groups;
    let unread: Vec<_> = groups
        .iter()
        .filter(|g| g.unread)
        .map(|g| g.target["work_id"].clone())
        .collect();
    assert_eq!(unread.len(), 2, "{groups:?}");
    assert!(unread.contains(&json!(id(0xa1).to_string())));
    assert!(unread.contains(&json!(id(0xa3).to_string())));
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_command_key_is_bound_to_its_request_and_versions_cannot_be_invented(pool: PgPool) {
    let owner = id(OWNER);
    signal(&pool, 1, 0x31).await;
    let group = page(&pool, owner, T0).await.groups.remove(0);
    let snapshot = page(&pool, owner, T0).await.snapshot;
    run(
        &pool,
        owner,
        "k",
        Command::Seen {
            up_to: snapshot.token(),
        },
        T0,
    )
    .await;
    let reused = store::command(
        &pool,
        owner,
        "k",
        &Command::MarkAllRead {
            up_to: snapshot.token(),
        },
        T0,
    )
    .await;
    assert_eq!(reused, Err(store::InboxError::IdempotencyKeyReused));
    let future = store::command(
        &pool,
        owner,
        "v",
        &Command::Read {
            group_id: group.group_id,
            observed_version: group.group_version + 1,
        },
        T0,
    )
    .await;
    assert_eq!(future, Err(store::InboxError::InvalidVersion));
    // Another recipient cannot read this group: it does not exist in their inbox.
    let foreign = store::command(
        &pool,
        id(0x0b),
        "x",
        &Command::Read {
            group_id: group.group_id,
            observed_version: 1,
        },
        T0,
    )
    .await;
    assert_eq!(foreign, Err(store::InboxError::NotFound));
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn the_inbox_route_pages_by_cursor_and_runs_commands(pool: PgPool) {
    let router = app(pool.clone(), Scope::Inbox);
    for n in 0u8..5 {
        signal_on(&pool, 1, 0xa0 + n, 0x30 + n).await;
    }
    let base = format!("/v1/inboxes/{}", id(OWNER));
    let (status, count) = call(&router, "GET", &format!("{base}/count"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(count["status"], "fresh");
    assert_eq!(count["unseen_group_count"], 5);
    assert_eq!(count["as_of_snapshot"], "s5");
    assert_eq!(count["max_staleness_seconds"], 60);

    let (status, first) = call(
        &router,
        "GET",
        &format!("{base}/groups?limit=2"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["inbox_snapshot"], "s5");
    assert_eq!(first["groups"].as_array().unwrap().len(), 2);
    let cursor = first["next_cursor"].as_str().unwrap().to_string();
    let (_, second) = call(
        &router,
        "GET",
        &format!("{base}/groups?limit=2&cursor={cursor}"),
        Value::Null,
    )
    .await;
    let (_, third) = call(
        &router,
        "GET",
        &format!(
            "{base}/groups?limit=2&cursor={}",
            second["next_cursor"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(third["groups"].as_array().unwrap().len(), 1);
    assert!(third["next_cursor"].is_null());
    let mut ids: Vec<String> = [&first, &second, &third]
        .iter()
        .flat_map(|page| {
            page["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g["group_id"].as_str().unwrap().to_string())
        })
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 5);

    let group = &first["groups"][0];
    let (status, answer) = call(
        &router,
        "POST",
        &format!("{base}/read"),
        json!({"command_key": "click-1", "group_id": group["group_id"], "observed_group_version": group["group_version"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["unread"], false);
    let (status, answer) = call(
        &router,
        "POST",
        &format!("{base}/seen"),
        json!({"command_key": "bell-1", "up_to_snapshot": "s99"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer["error"]["code"], "invalid_snapshot");
    let (status, answer) = call(
        &router,
        "POST",
        &format!("{base}/mark-all-read"),
        json!({"command_key": "click-1", "up_to_snapshot": "s5"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer["error"]["code"], "idempotency_key_reused");
    let (status, answer) = call(
        &router,
        "GET",
        &format!("{base}/groups?cursor=bogus"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer["error"]["code"], "invalid_cursor");
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn read_racing_new_signals_never_leaves_two_open_groups(pool: PgPool) {
    let owner = id(OWNER);
    signal(&pool, 1, 0x31).await;
    for round in 0u8..10 {
        let group = page(&pool, owner, T0)
            .await
            .groups
            .into_iter()
            .find(|g| g.unread);
        let reader = {
            let pool = pool.clone();
            async move {
                if let Some(group) = group {
                    let key = format!("race-{round}");
                    let _ = store::command(
                        &pool,
                        owner,
                        &key,
                        &Command::Read {
                            group_id: group.group_id,
                            observed_version: group.group_version,
                        },
                        T0,
                    )
                    .await;
                }
            }
        };
        let writer = signal(&pool, 0x60 + round, 0x50 + round);
        tokio::join!(reader, writer);
        let open: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notification_groups WHERE recipient = $1 AND open",
        )
        .bind(owner)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(open <= 1, "round {round}: {open} open groups");
    }
    // Every unread group has a signal newer than what was read.
    let bad: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_groups WHERE open AND signal_seq <= read_through",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bad, 0);
}

/// #348 seam: a comment group carries its newest active contributions (bounded, newest first)
/// and how many are active, so the Gateway can fall back to an older valid comment; an
/// invalidated contribution leaves the list; other group types carry none.
#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn comment_groups_carry_their_newest_active_contributions_within_a_bound(pool: PgPool) {
    // 20 comments by 4 actors on one work: one open group.
    for n in 0..20u8 {
        signal(&pool, 0x40 + n % 4, 0x80 + n).await;
    }
    let page = store::page(&pool, id(OWNER), 10, None, T0).await.unwrap();
    assert_eq!(page.groups.len(), 1);
    let contributions = page.groups[0].contributions.clone().unwrap();
    assert_eq!(contributions.total, 20);
    assert_eq!(
        contributions.newest.len(),
        usize::try_from(store::MAX_CONTRIBUTIONS).unwrap()
    );
    assert_eq!(contributions.newest[0], (id(0x80 + 19), id(0x40 + 19 % 4)));
    assert_eq!(contributions.newest[1], (id(0x80 + 18), id(0x40 + 18 % 4)));
    // The newest comment is invalidated: it leaves the list, the next one leads.
    applied(
        &pool,
        &comment_gone(event_id(), id(WORK), id(0x80 + 19), 3, T0),
        T0,
    )
    .await;
    let page = store::page(&pool, id(OWNER), 10, None, T0).await.unwrap();
    let contributions = page.groups[0].contributions.clone().unwrap();
    assert_eq!(contributions.total, 19);
    assert_eq!(contributions.newest[0], (id(0x80 + 18), id(0x40 + 18 % 4)));
    assert!(
        !contributions
            .newest
            .contains(&(id(0x80 + 19), id(0x40 + 19 % 4)))
    );
    // The route serializes the seam; a decision group has none.
    applied(
        &pool,
        &decided(event_id(), id(OWNER), id(WORK), id(0x77), 2, true, T0),
        T0,
    )
    .await;
    let page = store::page(&pool, id(OWNER), 10, None, T0).await.unwrap();
    let json: Vec<Value> = page.groups.iter().map(store::GroupView::json).collect();
    let decision = json
        .iter()
        .find(|g| g["event_type"] == "work.moderation.decided")
        .unwrap();
    assert!(decision["contributions"].is_null());
    let root = json
        .iter()
        .find(|g| g["event_type"] == "comment.root.approved")
        .unwrap();
    assert_eq!(root["contributions"]["total"], 19);
    assert_eq!(
        root["contributions"]["newest"][0]["comment_id"],
        id(0x80 + 18).to_string()
    );
}
