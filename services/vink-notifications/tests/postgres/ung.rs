//! Ưng on comments (#365 AC-05): one group per (writer, comment), P-02B once per actor + comment
//! for its whole lifetime, P-03A removal before Read, zero hides the group, and one comment's Ưng
//! never joins another comment's group or a Thấm group.

use super::*;

const WRITER: u8 = 0x0c;
const WORK: u8 = 0xaa;
const COMMENT: u8 = 0xc1;
const OTHER_COMMENT: u8 = 0xc2;

async fn ung_at(
    pool: &PgPool,
    actor: u8,
    comment: u8,
    add: Option<bool>,
    order: i64,
    now: OffsetDateTime,
) -> Effect {
    let text = ung(
        event_id(),
        id(actor),
        id(WRITER),
        id(WORK),
        id(comment),
        add,
        order,
        now,
    );
    applied(pool, &text, now).await
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn ung_signals_once_per_actor_and_comment_and_zero_hides_the_unread_group(pool: PgPool) {
    let writer = id(WRITER);
    assert_eq!(
        ung_at(&pool, 1, COMMENT, Some(true), 2, T0).await,
        Effect::Signal
    );
    assert_eq!(
        ung_at(&pool, 2, COMMENT, Some(true), 2, T0).await,
        Effect::Signal
    );
    let groups = page(&pool, writer, T0).await.groups;
    assert_eq!(groups.len(), 1, "one group per comment");
    let group = &groups[0];
    assert_eq!(group.event_type, "comment.endorsement.ung.changed");
    assert_eq!(group.actor_count, 2);
    assert_eq!(group.actors_preview, vec![id(2), id(1)]);
    assert_eq!(group.target["kind"], "work_comment_endorsement");
    assert_eq!(group.target["comment_id"], id(COMMENT).to_string());
    assert!(
        group.contributions.is_none(),
        "only comment approval groups carry contributions"
    );
    assert_eq!(badge(&pool, writer, T0).await, 1);

    // P-03A: both remove before the writer reads; the count goes down, then the row disappears.
    assert_eq!(
        ung_at(&pool, 2, COMMENT, None, 3, T0).await,
        Effect::Reconcile
    );
    let groups = page(&pool, writer, T0).await.groups;
    assert_eq!(
        (groups[0].actor_count, groups[0].actors_preview.clone()),
        (1, vec![id(1)])
    );
    assert_eq!(
        ung_at(&pool, 1, COMMENT, None, 3, T0).await,
        Effect::Reconcile
    );
    assert!(page(&pool, writer, T0).await.groups.is_empty());
    assert_eq!(badge(&pool, writer, T0).await, 0);

    // P-02B: an Ưng again by the same actor on the same comment stays silent, even when the App
    // flags it as first (it never does after a removal; the mark is the second line of defense).
    assert_eq!(
        ung_at(&pool, 1, COMMENT, Some(true), 4, T0).await,
        Effect::Reconcile
    );
    assert!(page(&pool, writer, T0).await.groups.is_empty());
    // The same actor on another comment is a first Ưng there.
    assert_eq!(
        ung_at(&pool, 1, OTHER_COMMENT, Some(true), 2, T0).await,
        Effect::Signal
    );
    let groups = page(&pool, writer, T0).await.groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].target["comment_id"],
        id(OTHER_COMMENT).to_string()
    );
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn after_read_a_reung_is_silent_and_a_new_actor_opens_a_new_group(pool: PgPool) {
    let writer = id(WRITER);
    ung_at(&pool, 1, COMMENT, Some(true), 2, T0).await;
    let group = page(&pool, writer, T0).await.groups.remove(0);
    run(
        &pool,
        writer,
        "read-1",
        Command::Read {
            group_id: group.group_id,
            observed_version: group.group_version,
        },
        T0,
    )
    .await;
    // Remove and Ưng again after Read: the read group keeps its history, nothing wakes up.
    ung_at(&pool, 1, COMMENT, None, 3, T0).await;
    ung_at(&pool, 1, COMMENT, Some(false), 4, T0).await;
    let groups = page(&pool, writer, T0).await.groups;
    assert_eq!(groups.len(), 1);
    assert!(!groups[0].unread);
    assert_eq!(badge(&pool, writer, T0).await, 0);
    // A new person's first Ưng opens the next unread group for the same comment (N5).
    assert_eq!(
        ung_at(&pool, 2, COMMENT, Some(true), 2, T0).await,
        Effect::Signal
    );
    let groups = page(&pool, writer, T0).await.groups;
    assert_eq!(groups.len(), 2);
    assert!(groups[0].unread && groups[0].actor_count == 1);
    assert_eq!(badge(&pool, writer, T0).await, 1);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn an_older_ung_event_never_overrides_a_newer_one_and_late_events_never_signal(pool: PgPool) {
    let writer = id(WRITER);
    // The removal (order 3) arrives before the add it follows (order 2): the add is stale.
    assert_eq!(
        ung_at(&pool, 1, COMMENT, None, 3, T0).await,
        Effect::Reconcile
    );
    assert_eq!(
        ung_at(&pool, 1, COMMENT, Some(true), 2, T0).await,
        Effect::Stale
    );
    assert!(page(&pool, writer, T0).await.groups.is_empty());
    // An event older than the late cutoff updates the mark but never signals (P-13B).
    let late = T0 - time::Duration::days(31);
    let text = ung(
        event_id(),
        id(2),
        id(WRITER),
        id(WORK),
        id(COMMENT),
        Some(true),
        2,
        late,
    );
    assert_eq!(applied(&pool, &text, T0).await, Effect::Late);
    assert!(page(&pool, writer, T0).await.groups.is_empty());
    let marks: i64 = sqlx::query_scalar("SELECT count(*) FROM ung_marks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(marks, 2);
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn ung_marks_are_purged_with_their_horizon(pool: PgPool) {
    ung_at(&pool, 1, COMMENT, Some(true), 2, T0).await;
    let later = T0 + time::Duration::days(401);
    let purged = store::purge(&pool, later, 1_000, 10).await.unwrap();
    assert_eq!((purged.groups, purged.ung_marks), (1, 1));
    assert_eq!(store::status(&pool, later).await.unwrap().ung_marks, 0);
}
