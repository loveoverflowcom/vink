//! 180-day ceiling, no revival, purge and what survives it (AC-05), with a controlled clock.

use super::*;
use time::Duration;

const OWNER: u8 = 0x0a;
const WORK: u8 = 0xaa;
const REVISION: u8 = 0xee;

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_group_expires_180_days_after_creation_and_updates_never_extend_it(pool: PgPool) {
    let owner = id(OWNER);
    applied(
        &pool,
        &root(event_id(), id(1), owner, id(WORK), id(0x31), 2, T0),
        T0,
    )
    .await;
    let day170 = T0 + Duration::days(170);
    applied(
        &pool,
        &root(event_id(), id(2), owner, id(WORK), id(0x32), 2, day170),
        day170,
    )
    .await;
    let group = page(&pool, owner, day170).await.groups.remove(0);
    assert_eq!(group.actor_count, 2, "day 170 joins the same group");
    assert_eq!(group.expires_at, T0 + Duration::days(180));
    // Day 180: gone from the inbox and the badge, before any purge.
    let day180 = T0 + Duration::days(180);
    assert!(page(&pool, owner, day180).await.groups.is_empty());
    assert_eq!(badge(&pool, owner, day180).await, 0);
    // Day 181: a new interaction starts a new group; the expired one is never revived.
    let day181 = T0 + Duration::days(181);
    applied(
        &pool,
        &root(event_id(), id(3), owner, id(WORK), id(0x33), 2, day181),
        day181,
    )
    .await;
    let groups = page(&pool, owner, day181).await.groups;
    assert_eq!(groups.len(), 1);
    assert_ne!(groups[0].group_id, group.group_id);
    assert_eq!(groups[0].actor_count, 1);
    assert_eq!(open_groups(&pool).await, 1);
    // Reading an expired group is not found.
    let expired = store::command(
        &pool,
        owner,
        "late",
        &Command::Read {
            group_id: group.group_id,
            observed_version: group.group_version,
        },
        day181,
    )
    .await;
    assert_eq!(expired, Err(store::InboxError::NotFound));
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn purge_deletes_expired_inbox_data_but_keeps_what_protects_p02b(pool: PgPool) {
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
    applied(
        &pool,
        &root(event_id(), id(2), owner, work, id(0x32), 2, T0),
        T0,
    )
    .await;
    let day181 = T0 + Duration::days(181);
    let purged = store::purge(&pool, day181, 1, 10).await.unwrap();
    assert_eq!(purged.groups, 2);
    assert_eq!(
        purged.receipts, 2,
        "receipts outlive the replay horizon, not the inbox"
    );
    assert_eq!(purged.marks, 0, "the Thấm mark outlives the inbox");
    let members: i64 = sqlx::query_scalar("SELECT count(*) FROM group_members")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(members, 0, "membership goes with its group");
    // Day 181: U1 unlikes and relikes; the inbox stays silent (P-02B after purge).
    applied(
        &pool,
        &tham(
            event_id(),
            id(1),
            owner,
            work,
            revision,
            None,
            2,
            true,
            day181,
        ),
        day181,
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
            day181,
        ),
        day181,
    )
    .await;
    assert!(page(&pool, owner, day181).await.groups.is_empty());
    // The inbox row and its sequence remain, so old snapshot tokens never cover new signals.
    let last: i64 = sqlx::query_scalar("SELECT last_seq FROM inboxes WHERE recipient = $1")
        .bind(owner)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(last >= 2);
    let status = store::status(&pool, day181).await.unwrap();
    assert_eq!((status.groups, status.receipts, status.marks), (0, 2, 1));
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_restore_without_receipts_replays_within_the_horizon_without_reviving_expired_groups(
    pool: PgPool,
) {
    let owner = id(OWNER);
    // An event applied on day 0, and another on day 175.
    let old = root(event_id(), id(1), owner, id(WORK), id(0x31), 2, T0);
    applied(&pool, &old, T0).await;
    let day175 = T0 + Duration::days(175);
    let recent = root(event_id(), id(2), owner, id(0xab), id(0x32), 2, day175);
    applied(&pool, &recent, day175).await;
    // Restore to a backup taken before both: the database lost every row of the inbox.
    sqlx::query("TRUNCATE inboxes, ingestion_receipts, notification_groups, group_members, tham_marks, comment_fences, inbox_commands")
        .execute(&pool)
        .await
        .unwrap();
    // The App replays its window on day 181. The day-0 event is past the late cutoff: receipted,
    // never shown again (its group would have expired). The day-175 one is restored.
    let day181 = T0 + Duration::days(181);
    assert_eq!(applied(&pool, &old, day181).await, Effect::Late);
    assert_eq!(applied(&pool, &recent, day181).await, Effect::Signal);
    let groups = page(&pool, owner, day181).await.groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].target["work_id"], id(0xab).to_string());
    assert_eq!(
        groups[0].expires_at,
        day175 + Duration::days(180),
        "aged from the original event"
    );
    // Replaying twice changes nothing more.
    assert_eq!(
        send(&pool, &recent, day181).await.0,
        IngestStatus::AlreadyApplied
    );
}
