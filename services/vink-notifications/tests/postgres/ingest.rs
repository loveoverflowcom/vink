//! Receipt + effect atomicity, idempotency and the write-time rollup (AC-02, AC-03).

use super::*;

const OWNER: u8 = 0x0a;
const WORK: u8 = 0xaa;
const REVISION: u8 = 0xee;

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_lost_ack_replays_as_already_applied_and_another_payload_is_a_mismatch(pool: PgPool) {
    let event = event_id();
    let text = root(event, id(1), id(OWNER), id(WORK), id(0x31), 2, T0);
    assert_eq!(
        send(&pool, &text, T0).await,
        (IngestStatus::Applied, Some(Effect::Signal))
    );
    // The relay lost the answer and re-sends the same (event_id, digest): one effect only.
    assert_eq!(
        send(&pool, &text, T0).await,
        (IngestStatus::AlreadyApplied, None)
    );
    let groups = page(&pool, id(OWNER), T0).await.groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].actor_count, 1);
    // Same ID, another (validly signed) payload: never a replay, nothing written.
    let other = root(event, id(2), id(OWNER), id(WORK), id(0x32), 2, T0);
    assert_eq!(
        send(&pool, &other, T0).await,
        (IngestStatus::PayloadMismatch, None)
    );
    let groups = page(&pool, id(OWNER), T0).await.groups;
    assert_eq!(groups[0].actor_count, 1);
    // A wrong digest is refused before the receipt is read.
    let tampered = text.replace("\"source_order\":2", "\"source_order\":3");
    assert_eq!(
        send(&pool, &tampered, T0).await,
        (IngestStatus::RejectedInvalid, None)
    );
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn one_unread_group_counts_distinct_actors_and_previews_the_three_latest(pool: PgPool) {
    let owner = id(OWNER);
    for (n, actor) in [1u8, 2, 3, 4].into_iter().enumerate() {
        let comment = id(0x30 + actor);
        let when = T0 + time::Duration::minutes(n as i64);
        applied(
            &pool,
            &root(event_id(), id(actor), owner, id(WORK), comment, 2, when),
            when,
        )
        .await;
    }
    // Actor 1 again with another comment: still four people, actor 1 now the most recent.
    applied(
        &pool,
        &root(event_id(), id(1), owner, id(WORK), id(0x41), 2, T0),
        T0,
    )
    .await;
    let groups = page(&pool, owner, T0).await.groups;
    assert_eq!(groups.len(), 1, "one unread group per key");
    let group = &groups[0];
    assert_eq!(group.actor_count, 4);
    assert_eq!(group.actors_preview, vec![id(1), id(4), id(3)]);
    assert!(group.unread && group.unseen);
    assert_eq!(group.target["comment_id"], id(0x41).to_string());
    assert_eq!(badge(&pool, owner, T0).await, 1);
    assert_eq!(open_groups(&pool).await, 1);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_reply_reaches_each_recipient_in_one_envelope_and_an_event_after_read_starts_a_new_group(
    pool: PgPool,
) {
    let (root_author, owner) = (id(0x0b), id(OWNER));
    let text = reply(
        event_id(),
        id(1),
        vec![root_author, owner],
        id(WORK),
        id(0x21),
        id(0x22),
        T0,
    );
    assert_eq!(applied(&pool, &text, T0).await, Effect::Signal);
    for recipient in [root_author, owner] {
        assert_eq!(page(&pool, recipient, T0).await.groups.len(), 1);
    }
    let group = page(&pool, owner, T0).await.groups.remove(0);
    run(
        &pool,
        owner,
        "r1",
        Command::Read {
            group_id: group.group_id,
            observed_version: group.group_version,
        },
        T0,
    )
    .await;
    applied(
        &pool,
        &reply(
            event_id(),
            id(2),
            vec![owner],
            id(WORK),
            id(0x21),
            id(0x23),
            T0,
        ),
        T0,
    )
    .await;
    let groups = page(&pool, owner, T0).await.groups;
    assert_eq!(groups.len(), 2, "N5: the read group stays, a new one opens");
    assert!(groups[0].unread && groups[0].actor_count == 1);
    assert!(!groups[1].unread && groups[1].group_id == group.group_id);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn concurrent_writers_keep_one_open_group_and_every_contribution(pool: PgPool) {
    let owner = id(OWNER);
    let mut tasks = Vec::new();
    for actor in 1u8..=12 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let text = root(
                event_id(),
                id(0x10 + actor),
                owner,
                id(WORK),
                id(0x40 + actor),
                2,
                T0,
            );
            send(&pool, &text, T0).await
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().0, IngestStatus::Applied);
    }
    let groups = page(&pool, owner, T0).await.groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].actor_count, 12);
    assert_eq!(open_groups(&pool).await, 1);
    // The inbox sequence advanced once per signal: commit order is version order.
    let last: i64 = sqlx::query_scalar("SELECT last_seq FROM inboxes WHERE recipient = $1")
        .bind(owner)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(last, 12);
    assert_eq!(groups[0].group_version, 12);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn the_same_event_sent_by_two_relays_at_once_has_one_effect(pool: PgPool) {
    let text = root(event_id(), id(1), id(OWNER), id(WORK), id(0x31), 2, T0);
    let (a, b) = tokio::join!(send(&pool, &text, T0), send(&pool, &text, T0));
    let mut statuses = [a.0, b.0];
    statuses.sort_by_key(|status| status.code());
    assert_eq!(
        statuses,
        [IngestStatus::AlreadyApplied, IngestStatus::Applied]
    );
    assert_eq!(page(&pool, id(OWNER), T0).await.groups[0].group_version, 1);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn tham_signals_once_unlike_reconciles_and_zero_hides_the_unread_group(pool: PgPool) {
    let (owner, work, revision) = (id(OWNER), id(WORK), id(REVISION));
    // P-05 a: U1 likes then unlikes before the owner opens the bell.
    assert_eq!(
        applied(
            &pool,
            &tham(
                event_id(),
                id(1),
                owner,
                work,
                revision,
                Some(true),
                1,
                true,
                T0
            ),
            T0
        )
        .await,
        Effect::Signal
    );
    assert_eq!(badge(&pool, owner, T0).await, 1);
    assert_eq!(
        applied(
            &pool,
            &tham(event_id(), id(1), owner, work, revision, None, 2, false, T0),
            T0
        )
        .await,
        Effect::Reconcile
    );
    assert_eq!(
        badge(&pool, owner, T0).await,
        0,
        "the count comes back down"
    );
    assert!(
        page(&pool, owner, T0).await.groups.is_empty(),
        "the row disappears, not read-empty"
    );
    // Relike: silent, no new row, no badge (P-02B).
    assert_eq!(
        applied(
            &pool,
            &tham(
                event_id(),
                id(1),
                owner,
                work,
                revision,
                Some(false),
                3,
                true,
                T0
            ),
            T0
        )
        .await,
        Effect::Reconcile
    );
    assert!(page(&pool, owner, T0).await.groups.is_empty());
    assert_eq!(badge(&pool, owner, T0).await, 0);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn an_unlike_inside_or_outside_the_preview_updates_count_and_preview(pool: PgPool) {
    let (owner, work, revision) = (id(OWNER), id(WORK), id(REVISION));
    for actor in 1u8..=4 {
        applied(
            &pool,
            &tham(
                event_id(),
                id(actor),
                owner,
                work,
                revision,
                Some(true),
                1,
                true,
                T0,
            ),
            T0,
        )
        .await;
    }
    let before = page(&pool, owner, T0).await.groups.remove(0);
    assert_eq!(
        (before.actor_count, before.actors_preview.clone()),
        (4, vec![id(4), id(3), id(2)])
    );
    // U3 (inside the preview) leaves: U1 moves in.
    applied(
        &pool,
        &tham(event_id(), id(3), owner, work, revision, None, 2, true, T0),
        T0,
    )
    .await;
    let after = page(&pool, owner, T0).await.groups.remove(0);
    assert_eq!(
        (after.actor_count, after.actors_preview.clone()),
        (3, vec![id(4), id(2), id(1)])
    );
    assert!(after.group_version > before.group_version);
    assert_eq!(
        after.signal_version, before.signal_version,
        "P-03A: no new signal"
    );
    // U3 relikes: membership restored silently.
    applied(
        &pool,
        &tham(
            event_id(),
            id(3),
            owner,
            work,
            revision,
            Some(false),
            3,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let relike = page(&pool, owner, T0).await.groups.remove(0);
    assert_eq!(relike.actor_count, 4);
    assert_eq!(relike.signal_version, before.signal_version);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn after_read_a_relike_never_notifies_but_a_new_person_does(pool: PgPool) {
    let (owner, work, revision) = (id(OWNER), id(WORK), id(REVISION));
    applied(
        &pool,
        &tham(
            event_id(),
            id(1),
            owner,
            work,
            revision,
            Some(true),
            1,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let group = page(&pool, owner, T0).await.groups.remove(0);
    run(
        &pool,
        owner,
        "read-1",
        Command::Read {
            group_id: group.group_id,
            observed_version: group.group_version,
        },
        T0,
    )
    .await;
    // P-05 c: unlike then relike on a read group: history kept, no new group.
    applied(
        &pool,
        &tham(event_id(), id(1), owner, work, revision, None, 2, true, T0),
        T0,
    )
    .await;
    applied(
        &pool,
        &tham(
            event_id(),
            id(1),
            owner,
            work,
            revision,
            Some(false),
            3,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let groups = page(&pool, owner, T0).await.groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].actor_count, 1, "a read group keeps its history");
    assert!(!groups[0].unread);
    // U5's first Thấm opens a new unread group.
    applied(
        &pool,
        &tham(
            event_id(),
            id(5),
            owner,
            work,
            revision,
            Some(true),
            1,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let groups = page(&pool, owner, T0).await.groups;
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].actors_preview, vec![id(5)]);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn reordered_thams_events_never_wake_a_closed_selection(pool: PgPool) {
    let (owner, work, revision) = (id(OWNER), id(WORK), id(REVISION));
    // Remove(2) arrives before the first Add(1).
    assert_eq!(
        applied(
            &pool,
            &tham(event_id(), id(1), owner, work, revision, None, 2, true, T0),
            T0
        )
        .await,
        Effect::Reconcile
    );
    assert_eq!(
        applied(
            &pool,
            &tham(
                event_id(),
                id(1),
                owner,
                work,
                revision,
                Some(true),
                1,
                true,
                T0
            ),
            T0
        )
        .await,
        Effect::Stale
    );
    assert!(page(&pool, owner, T0).await.groups.is_empty());
    // An add that was not readable at the source never signals, even if flagged first.
    applied(
        &pool,
        &tham(
            event_id(),
            id(2),
            owner,
            work,
            revision,
            Some(true),
            1,
            false,
            T0,
        ),
        T0,
    )
    .await;
    assert!(page(&pool, owner, T0).await.groups.is_empty());
    // Same actor, same lifetime, a later first flag (a producer bug or replay): one signal only.
    applied(
        &pool,
        &tham(
            event_id(),
            id(4),
            owner,
            work,
            revision,
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
            id(3),
            owner,
            work,
            revision,
            Some(true),
            1,
            true,
            T0,
        ),
        T0,
    )
    .await;
    let before = page(&pool, owner, T0).await.groups.remove(0);
    applied(
        &pool,
        &tham(event_id(), id(3), owner, work, revision, None, 2, true, T0),
        T0,
    )
    .await;
    assert_eq!(
        applied(
            &pool,
            &tham(
                event_id(),
                id(3),
                owner,
                work,
                revision,
                Some(true),
                3,
                true,
                T0
            ),
            T0
        )
        .await,
        Effect::Reconcile
    );
    let after = page(&pool, owner, T0).await.groups.remove(0);
    assert_eq!(after.actor_count, 2);
    assert_eq!(
        after.signal_version, before.signal_version,
        "no second signal for actor 3"
    );
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_removed_comment_leaves_its_groups_and_a_late_older_approval_cannot_restore_it(
    pool: PgPool,
) {
    let (owner, work) = (id(OWNER), id(WORK));
    applied(
        &pool,
        &root(event_id(), id(1), owner, work, id(0x31), 2, T0),
        T0,
    )
    .await;
    applied(
        &pool,
        &root(event_id(), id(2), owner, work, id(0x32), 2, T0),
        T0,
    )
    .await;
    let group = page(&pool, owner, T0).await.groups.remove(0);
    assert_eq!(group.actor_count, 2);
    // Comment 0x32 left public at version 3.
    assert_eq!(
        applied(&pool, &comment_gone(event_id(), work, id(0x32), 3, T0), T0).await,
        Effect::Reconcile
    );
    let after = page(&pool, owner, T0).await.groups.remove(0);
    assert_eq!(
        (after.actor_count, after.actors_preview.clone()),
        (1, vec![id(1)])
    );
    // An approval at version 2 delivered late (another event ID) is older than the fence.
    assert_eq!(
        applied(
            &pool,
            &root(event_id(), id(2), owner, work, id(0x32), 2, T0),
            T0
        )
        .await,
        Effect::Stale
    );
    assert_eq!(page(&pool, owner, T0).await.groups[0].actor_count, 1);
    // A re-approval at version 4 is new.
    assert_eq!(
        applied(
            &pool,
            &root(event_id(), id(2), owner, work, id(0x32), 4, T0),
            T0
        )
        .await,
        Effect::Signal
    );
    assert_eq!(page(&pool, owner, T0).await.groups[0].actor_count, 2);
    // The invalidation before any approval: the fence alone stops the older approval.
    assert_eq!(
        applied(&pool, &comment_gone(event_id(), work, id(0x39), 5, T0), T0).await,
        Effect::Noted
    );
    assert_eq!(
        applied(
            &pool,
            &root(event_id(), id(3), owner, work, id(0x39), 2, T0),
            T0
        )
        .await,
        Effect::Stale
    );
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn decisions_count_items_per_outcome_and_late_events_never_signal(pool: PgPool) {
    let (owner, work, revision) = (id(OWNER), id(WORK), id(REVISION));
    applied(
        &pool,
        &decided(event_id(), owner, work, revision, 1, false, T0),
        T0,
    )
    .await;
    applied(
        &pool,
        &decided(event_id(), owner, work, revision, 2, true, T0),
        T0,
    )
    .await;
    applied(
        &pool,
        &decided(event_id(), owner, work, revision, 3, false, T0),
        T0,
    )
    .await;
    let groups = page(&pool, owner, T0).await.groups;
    assert_eq!(
        groups.len(),
        2,
        "approved and rejected are separate groups (P-07A)"
    );
    let approved = groups
        .iter()
        .find(|g| g.target["decision_version"] == 3)
        .unwrap();
    assert_eq!(approved.item_count, 2);
    assert!(!approved.action_required);
    let rejected = groups
        .iter()
        .find(|g| g.target["decision_version"] == 2)
        .unwrap();
    assert!(rejected.action_required);
    // P-13B: an event that occurred more than 30 days ago is receipted, never shown.
    let old = T0 - time::Duration::days(31);
    let late = root(event_id(), id(7), id(0x0c), work, id(0x37), 2, old);
    assert_eq!(
        send(&pool, &late, T0).await,
        (IngestStatus::Applied, Some(Effect::Late))
    );
    assert!(page(&pool, id(0x0c), T0).await.groups.is_empty());
    assert_eq!(send(&pool, &late, T0).await.0, IngestStatus::AlreadyApplied);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn the_ingest_route_answers_one_result_per_event_in_order(pool: PgPool) {
    let router = app(pool.clone(), Scope::Ingest);
    let first = event_id();
    let good = root(first, id(1), id(OWNER), id(WORK), id(0x31), 2, T0);
    let admin_id = event_id();
    let admin = sign(json!({
        "event_id": admin_id.to_string(),
        "event_type": "admin.notice.reserved",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": 1,
        "occurred_at": "2026-11-21T00:00:00Z",
        "actor_id": null,
        "recipients": [id(OWNER).to_string()],
        "target": {"kind": "work", "work_id": id(WORK).to_string()},
        "semantic_kind": "approved",
        "action_required": false
    }));
    let body = json!({"events": [
        {"event_id": first, "envelope": good},
        {"event_id": admin_id, "envelope": admin},
        {"event_id": first, "envelope": good},
    ]});
    let (status, answer) = call(&router, "POST", "/v1/ingest", body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        answer,
        json!({"results": [
            {"event_id": first, "status": "applied"},
            {"event_id": admin_id, "status": "rejected_invalid"},
            {"event_id": first, "status": "already_applied"},
        ]})
    );
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM ingestion_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        receipts, 1,
        "the reserved admin type is never stored (P-05)"
    );
}

/// The SQL refresh and the pure rule (`rollup::preview`) agree on count and preview, including
/// repeated actors, removed contributions and ties.
#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn the_stored_preview_matches_the_pure_rule(pool: PgPool) {
    use vink_notifications::rollup::{Member, preview};
    let (owner, work) = (id(OWNER), id(WORK));
    for n in 0u8..30 {
        // Seven actors, several comments each.
        let actor = id(0x60 + n % 7);
        applied(
            &pool,
            &root(event_id(), actor, owner, work, id(0x80 + n), 2, T0),
            T0,
        )
        .await;
        if n % 4 == 3 {
            let gone = comment_gone(event_id(), work, id(0x80 + n), 3, T0);
            applied(&pool, &gone, T0).await;
        }
        let group = page(&pool, owner, T0).await.groups.remove(0);
        let rows: Vec<(Uuid, Uuid, bool, i64)> = sqlx::query_as(
            "SELECT actor_id, contribution, active, last_seq FROM group_members WHERE group_id = $1",
        )
        .bind(group.group_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let members: Vec<Member> = rows
            .into_iter()
            .map(|(actor, contribution, active, last_seq)| Member {
                actor,
                contribution,
                active,
                last_seq,
            })
            .collect();
        assert_eq!(
            (group.actor_count, group.actors_preview),
            preview(&members),
            "after event {n}"
        );
    }
}
