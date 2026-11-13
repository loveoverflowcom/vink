//! Vink-owned discussion on real PostgreSQL (D2T-02a). Use cases run through `PgStore`; integrity
//! negatives bypass Rust with direct SQL, so the schema's own guarantees are what is tested.
//! Constraints and triggers apply to every role; these run as the test-harness superuser.

use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, Row};
use vink_app::application::discussion::{
    DiscussionError, ModerateComment, ModerationAction, OwnStatus, RootEntry, RootPage,
    SetCommentsMode, SubmitComment,
};
use vink_app::domain::discussion::lifecycle::{CommentState, Review};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::discussion::page::PREVIEW_REPLIES;
use vink_app::domain::discussion::settings::CommentsMode;
use vink_app::domain::discussion::visibility::{
    CommentView, ReplyContext, RootListing, root_listing, view,
};
use vink_app::domain::ids::CommentId;
use vink_app::domain::principal::SuspensionReason;
use vink_app::persistence::{
    GrantOutcome, MIGRATOR, active_moderators, grant_moderator, migrate, reinstate_principal,
    revoke_moderator, suspend_principal,
};

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x5555_5555_5555_4555_8555_5555_5555_5555);

async fn discussion(pool: &PgPool) -> Vink<PgStore> {
    let vink = seeded(pool).await;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Kiểm duyệt viên')")
        .bind(MODERATOR)
        .execute(pool)
        .await
        .unwrap();
    let note = GrantNote::parse("Kiểm duyệt viên của bộ test").unwrap();
    let granted = grant_moderator(pool, PrincipalId::from_uuid(MODERATOR), &note)
        .await
        .unwrap();
    assert!(matches!(granted, GrantOutcome::Granted(_)));
    vink
}

fn moderator() -> Option<PrincipalId> {
    Some(PrincipalId::from_uuid(MODERATOR))
}

const REASON: &str = "Vi phạm quy tắc cộng đồng (dữ liệu kiểm thử).";

fn key() -> String {
    format!("k-{}", Uuid::now_v7().simple())
}

fn command(target: Option<CommentId>, body: &str, key: &str) -> SubmitComment {
    SubmitComment {
        work_id: sample_work(),
        target,
        body: body.into(),
        command_key: key.into(),
    }
}

async fn post(
    vink: &Vink<PgStore>,
    actor: Option<PrincipalId>,
    target: Option<CommentId>,
    body: &str,
) -> CommentId {
    let submitted = vink
        .submit_comment(actor, command(target, body, &key()))
        .await
        .unwrap();
    assert_eq!(submitted.status, OwnStatus::Pending);
    assert!(!submitted.replayed);
    submitted.comment_id
}

async fn moderate(
    vink: &Vink<PgStore>,
    comment_id: CommentId,
    expected_version: i64,
    action: ModerationAction,
) -> Result<CommentState, DiscussionError> {
    let reason = (action != ModerationAction::Approve).then(|| REASON.to_string());
    vink.moderate_comment(
        moderator(),
        ModerateComment {
            comment_id,
            expected_version,
            action,
            reason,
        },
    )
    .await
}

async fn published(
    vink: &Vink<PgStore>,
    actor: Option<PrincipalId>,
    target: Option<CommentId>,
    body: &str,
) -> CommentId {
    let id = post(vink, actor, target, body).await;
    moderate(vink, id, 1, ModerationAction::Approve)
        .await
        .unwrap();
    id
}

async fn roots(vink: &Vink<PgStore>, viewer: Option<PrincipalId>) -> RootPage {
    vink.discussion(viewer, sample_work(), None, Some(50))
        .await
        .unwrap()
}

fn listed_ids(page: &RootPage) -> Vec<CommentId> {
    page.roots
        .iter()
        .map(|root| match &root.entry {
            RootEntry::Comment(item) => item.id,
            RootEntry::Anchor { id, .. } => *id,
        })
        .collect()
}

async fn thread_of_sample(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM discussion_threads WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Columns and values that every direct INSERT must state: a premoderated comment is born pending
/// at version 1 (migration `20261111000000_comment_admission`).
const PREMODERATED: &str = "'pending', 1, 'premoderation.v1', 'policy_premoderation'";

/// Moves directly inserted rows to a reviewed state the way a decision would: one version later.
async fn review_raw(
    executor: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    review: &str,
    removal: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE comments SET review = $2, removal = $3,
                body = CASE WHEN $3 = 'author' THEN NULL ELSE body END,
                moderation_version = moderation_version + 1
          WHERE id = $1",
    )
    .bind(id)
    .bind(review)
    .bind(removal)
    .execute(executor)
    .await
    .map(drop)
}

/// Inserts a comment row with direct SQL, bypassing every Rust check, then approves it.
async fn raw_comment(
    pool: &PgPool,
    thread: Uuid,
    kind: &str,
    root: Option<Uuid>,
    reply_to: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query(AssertSqlSafe(format!(
        "INSERT INTO comments (id, thread_id, kind, root_id, root_kind, reply_to_id,
                               replied_to_reply, author_id, body, review, moderation_version,
                               admission_policy, admission_reason)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'Thử trực tiếp', {PREMODERATED})"
    )))
    .bind(id)
    .bind(thread)
    .bind(kind)
    .bind(root)
    .bind(root.map(|_| "root"))
    .bind(reply_to)
    .bind(reply_to.is_some())
    .bind(dev_seed::OTHER_PRINCIPAL)
    .execute(pool)
    .await?;
    review_raw(pool, id, "approved", None).await?;
    Ok(id)
}

fn constraint(error: sqlx::Error) -> String {
    let db = error.as_database_error().expect("a database error");
    db.constraint()
        .map(str::to_owned)
        .unwrap_or_else(|| db.message().to_owned())
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_database_alone_rejects_a_third_tier_and_cross_scope_references(pool: PgPool) {
    let vink = discussion(&pool).await;
    let root_a = published(&vink, other(), None, "Gốc A").await;
    let reply_b = published(&vink, author(), Some(root_a), "Trả lời A").await;
    let root_c = published(&vink, other(), None, "Gốc C").await;
    let reply_d = published(&vink, author(), Some(root_c), "Trả lời C").await;
    let thread = thread_of_sample(&pool).await;
    let (a, b, c, d) = (
        root_a.as_uuid(),
        reply_b.as_uuid(),
        root_c.as_uuid(),
        reply_d.as_uuid(),
    );

    // Another work's thread (no publication needed for raw SQL).
    let foreign_work = vink.create_draft(author()).await.unwrap().work_id;
    let foreign_thread = Uuid::now_v7();
    sqlx::query("INSERT INTO discussion_threads (id, work_id) VALUES ($1, $2)")
        .bind(foreign_thread)
        .bind(foreign_work.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let foreign_root = raw_comment(&pool, foreign_thread, "root", None, None)
        .await
        .unwrap();

    // Level 3: a reply whose structural parent is a reply.
    let error = raw_comment(&pool, thread, "reply", Some(b), None)
        .await
        .unwrap_err();
    assert_eq!(constraint(error), "comments_root_fkey");
    // Cross-thread root.
    let error = raw_comment(&pool, thread, "reply", Some(foreign_root), None)
        .await
        .unwrap_err();
    assert_eq!(constraint(error), "comments_root_fkey");
    // Context naming a root, a reply of another root, or itself.
    for (root, context) in [(a, a), (a, d), (c, b)] {
        let error = raw_comment(&pool, thread, "reply", Some(root), Some(context))
            .await
            .unwrap_err();
        assert_eq!(
            constraint(error),
            "comments_reply_to_fkey",
            "{root} {context}"
        );
    }
    // A root carrying a parent or context.
    let error = raw_comment(&pool, thread, "root", Some(a), None)
        .await
        .unwrap_err();
    assert_eq!(constraint(error), "comments_tier");
    // The valid shape passes the same path, so the negatives above are not vacuous.
    let peer = raw_comment(&pool, thread, "reply", Some(a), Some(b))
        .await
        .unwrap();

    // Placement is immutable; removal is terminal; versions never go back.
    for (statement, value) in [
        ("UPDATE comments SET root_id = $2 WHERE id = $1", c),
        ("UPDATE comments SET reply_to_id = $2 WHERE id = $1", d),
        (
            "UPDATE comments SET thread_id = $2 WHERE id = $1",
            foreign_thread,
        ),
        (
            "UPDATE comments SET author_id = $2 WHERE id = $1",
            dev_seed::SAMPLE_AUTHOR,
        ),
    ] {
        let error = sqlx::query(statement)
            .bind(peer)
            .bind(value)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(constraint(error).contains("immutable"), "{statement}");
    }
    vink.remove_own_comment(other(), sample_work(), root_c)
        .await
        .unwrap();
    let error = sqlx::query("UPDATE comments SET removal = NULL, body = 'hồi sinh' WHERE id = $1")
        .bind(c)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(constraint(error).contains("terminal"));
    let error = sqlx::query("UPDATE comments SET moderation_version = 1 WHERE id = $1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(constraint(error).contains("cannot decrease"));

    // No cascade: a root with replies cannot be deleted; deleting a context keeps its peers.
    let error = sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error
            .as_database_error()
            .and_then(|db| db.code())
            .as_deref(),
        Some("23503"),
        "deleting a root with replies is a foreign-key violation, never a cascade"
    );
    // Reviewed comments also keep audit rows, so delete a raw context to observe SET NULL.
    let context = raw_comment(&pool, thread, "reply", Some(a), None)
        .await
        .unwrap();
    let dependant = raw_comment(&pool, thread, "reply", Some(a), Some(context))
        .await
        .unwrap();
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(context)
        .execute(&pool)
        .await
        .unwrap();
    let row = sqlx::query(
        "SELECT root_id, thread_id, reply_to_id, replied_to_reply FROM comments WHERE id = $1",
    )
    .bind(dependant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<Option<Uuid>, _>("root_id"), Some(a));
    assert_eq!(row.get::<Uuid, _>("thread_id"), thread);
    assert_eq!(row.get::<Option<Uuid>, _>("reply_to_id"), None);
    assert!(row.get::<bool, _>("replied_to_reply"));
    let page = vink
        .replies(None, sample_work(), root_a, None, Some(50))
        .await
        .unwrap();
    let purged = page
        .replies
        .iter()
        .find(|r| r.id.as_uuid() == dependant)
        .unwrap();
    assert_eq!(purged.context, ReplyContext::Unavailable);

    // One thread per work.
    let error = sqlx::query("INSERT INTO discussion_threads (id, work_id) VALUES ($1, $2)")
        .bind(Uuid::now_v7())
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(constraint(error), "discussion_threads_work_id_key");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_new_comment_is_private_until_approved_and_retries_are_idempotent(pool: PgPool) {
    let vink = discussion(&pool).await;
    let key = key();
    let first = vink
        .submit_comment(other(), command(None, "Đọc xong muốn khóc 😢", &key))
        .await
        .unwrap();
    assert_eq!(first.status, OwnStatus::Pending);

    // Lost response: the same key and payload (NFD spelling) replays, nothing new is written.
    let retry = vink
        .submit_comment(
            other(),
            command(None, "Đo\u{0323}c xong muô\u{0301}n kho\u{0301}c 😢", &key),
        )
        .await
        .unwrap();
    assert_eq!(retry.comment_id, first.comment_id);
    assert!(retry.replayed);
    assert_eq!(
        vink.submit_comment(other(), command(None, "Nội dung khác", &key))
            .await,
        Err(DiscussionError::KeyReused)
    );
    // Keys are per actor: another principal may use the same key for its own comment.
    let theirs = vink
        .submit_comment(author(), command(None, "Cảm ơn bạn", &key))
        .await
        .unwrap();
    assert_ne!(theirs.comment_id, first.comment_id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM comments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);

    // Pending is invisible to anonymous readers and to the work's author; its writer sees status.
    assert!(listed_ids(&roots(&vink, None).await).is_empty());
    assert_eq!(
        listed_ids(&roots(&vink, author()).await),
        vec![theirs.comment_id]
    );
    let own = roots(&vink, other()).await;
    let RootEntry::Comment(item) = &own.roots[0].entry else {
        panic!("own pending comment expected")
    };
    assert_eq!(item.view, CommentView::OwnPending);
    assert_eq!(item.body, "Đọc xong muốn khóc 😢");

    moderate(&vink, first.comment_id, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let public = roots(&vink, None).await;
    assert_eq!(listed_ids(&public), vec![first.comment_id]);
    assert_eq!(public.roots[0].public_replies, 0);
    // A replay after approval reports the current status, without echoing text.
    let replay = vink
        .submit_comment(other(), command(None, "Đọc xong muốn khóc 😢", &key))
        .await
        .unwrap();
    assert_eq!(
        (replay.status, replay.replayed),
        (OwnStatus::Approved, true)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_retries_with_one_key_create_exactly_one_comment(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    let key = key();
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let vink = Arc::clone(&vink);
        let key = key.clone();
        tasks.push(tokio::spawn(async move {
            vink.submit_comment(other(), command(None, "Gửi một lần thôi", &key))
                .await
                .unwrap()
        }));
    }
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| !r.replayed).count(), 1);
    assert!(
        results
            .iter()
            .all(|r| r.comment_id == results[0].comment_id)
    );
    let (comments, receipts, threads): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM comments), (SELECT count(*) FROM discussion_command_receipts),
                (SELECT count(*) FROM discussion_threads)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((comments, receipts, threads), (1, 1, 1));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_work_author_toggles_comments_but_has_no_power_over_others(pool: PgPool) {
    let vink = discussion(&pool).await;
    let approved = published(&vink, other(), None, "Bình luận đã duyệt").await;
    let pending = post(&vink, other(), None, "Bình luận chờ duyệt").await;
    let close = |expected_generation| SetCommentsMode {
        work_id: sample_work(),
        expected_generation,
        mode: CommentsMode::Closed,
    };

    assert_eq!(
        vink.set_comments_mode(other(), close(1)).await,
        Err(DiscussionError::NotFound)
    );
    let closed = vink.set_comments_mode(author(), close(1)).await.unwrap();
    assert_eq!(
        (closed.mode(), closed.generation().get()),
        (CommentsMode::Closed, 2)
    );
    // A retry of the same choice writes nothing; a stale tab cannot reopen behind the author's back.
    assert_eq!(vink.set_comments_mode(author(), close(1)).await, Ok(closed));
    let reopen_at = |expected_generation| SetCommentsMode {
        mode: CommentsMode::Open,
        ..close(expected_generation)
    };
    assert_eq!(
        vink.set_comments_mode(author(), reopen_at(1)).await,
        Err(DiscussionError::Conflict { current: 2 })
    );
    // COM-07 trace: tab A's reopen@2 is delayed; tab B asks to keep it closed at 2 and arrives
    // first. B is accepted and its generation is stored, so A's late request is stale.
    let kept = vink.set_comments_mode(author(), close(2)).await.unwrap();
    assert_eq!(
        (kept.mode(), kept.generation().get()),
        (CommentsMode::Closed, 3)
    );
    assert_eq!(
        vink.set_comments_mode(author(), reopen_at(2)).await,
        Err(DiscussionError::Conflict { current: 3 })
    );
    let stored: (bool, i64) = sqlx::query_as(
        "SELECT comments_open, settings_generation FROM discussion_threads WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        (false, 3),
        "the reaffirmation is durable, the late reopen is not"
    );

    // Closed: nothing new, but approved comments stay readable and nothing is hidden or deleted.
    assert_eq!(
        vink.submit_comment(other(), command(None, "Muộn rồi", &key()))
            .await,
        Err(DiscussionError::CommentsClosed)
    );
    assert_eq!(
        vink.submit_comment(
            author(),
            command(Some(approved), "Tác giả cũng vậy", &key())
        )
        .await,
        Err(DiscussionError::CommentsClosed)
    );
    let page = roots(&vink, None).await;
    assert_eq!(page.settings.mode(), CommentsMode::Closed);
    assert_eq!(listed_ids(&page), vec![approved]);
    assert!(roots(&vink, author()).await.viewer_is_work_author);
    assert!(!page.viewer_is_work_author);

    // The work's author cannot remove other people's comments, visible or not.
    assert_eq!(
        vink.remove_own_comment(author(), sample_work(), approved)
            .await,
        Err(DiscussionError::NotCommentWriter)
    );
    assert_eq!(
        vink.remove_own_comment(author(), sample_work(), pending)
            .await,
        Err(DiscussionError::NotFound)
    );
    // ...nor moderate them: there is no grant through ownership.
    assert_eq!(
        vink.moderate_comment(
            author(),
            ModerateComment {
                comment_id: approved,
                expected_version: 2,
                action: ModerationAction::Remove,
                reason: Some(REASON.into()),
            },
        )
        .await,
        Err(DiscussionError::NotModerator)
    );
    // The writer removes their own comment, even while closed; the text is dropped.
    assert_eq!(
        vink.remove_own_comment(other(), sample_work(), approved)
            .await,
        Ok(OwnStatus::Removed)
    );
    assert_eq!(
        vink.remove_own_comment(other(), sample_work(), approved)
            .await,
        Ok(OwnStatus::Removed)
    );
    let body: Option<String> = sqlx::query_scalar("SELECT body FROM comments WHERE id = $1")
        .bind(approved.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(body, None);
    assert!(listed_ids(&roots(&vink, None).await).is_empty());
}

fn close_at(expected_generation: i64) -> SetCommentsMode {
    SetCommentsMode {
        work_id: sample_work(),
        expected_generation,
        mode: CommentsMode::Closed,
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn closing_waits_for_an_in_flight_submission_and_vice_versa(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    let opener = published(&vink, other(), None, "Mở thread").await;

    // An uncommitted close holds the thread row; a submission waits and then sees Closed.
    let mut closer = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE discussion_threads SET comments_open = false,
                settings_generation = settings_generation + 1 WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .execute(&mut *closer)
    .await
    .unwrap();
    let submitting = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.submit_comment(other(), command(None, "Đua với lệnh tắt", &key()))
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !submitting.is_finished(),
        "submission must wait for the thread lock"
    );
    closer.commit().await.unwrap();
    assert_eq!(
        submitting.await.unwrap(),
        Err(DiscussionError::CommentsClosed)
    );

    // A submission's share lock makes a mode change wait; once released the change applies.
    let mut submitter = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM discussion_threads WHERE work_id = $1 FOR SHARE")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *submitter)
        .await
        .unwrap();
    let reopening = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_comments_mode(
                author(),
                SetCommentsMode {
                    work_id: sample_work(),
                    expected_generation: 2,
                    mode: CommentsMode::Open,
                },
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !reopening.is_finished(),
        "mode change must wait for the share lock"
    );
    submitter.rollback().await.unwrap();
    let reopened = reopening.await.unwrap().unwrap();
    assert_eq!(reopened.mode(), CommentsMode::Open);
    let comments: i64 = sqlx::query_scalar("SELECT count(*) FROM comments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(comments, 1, "the refused submission left nothing behind");

    // A REAL in-flight submission holds its thread share lock until commit. Pause it on its
    // target row (locked here, after it already locked the thread): a close must wait, so no
    // comment can commit after a committed close.
    let mut pauser = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(opener.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let replying = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.submit_comment(author(), command(Some(opener), "Đang gửi dở", &key()))
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !replying.is_finished(),
        "submission is paused on its target"
    );
    let closing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_comments_mode(author(), close_at(3)).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !closing.is_finished(),
        "a close must wait for the in-flight submission's thread lock"
    );
    pauser.rollback().await.unwrap();
    let reply = replying.await.unwrap().unwrap();
    assert_eq!(reply.status, OwnStatus::Pending);
    let closed = closing.await.unwrap().unwrap();
    assert_eq!(
        (closed.mode(), closed.generation().get()),
        (CommentsMode::Closed, 4)
    );
    let reply_created_before_close: bool = sqlx::query_scalar(
        "SELECT c.created_at <= t.updated_at FROM comments c, discussion_threads t
          WHERE c.id = $1 AND t.work_id = $2",
    )
    .bind(reply.comment_id.as_uuid())
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(reply_created_before_close);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderation_is_version_bound_audited_and_cannot_revive_removed_comments(pool: PgPool) {
    let vink = discussion(&pool).await;
    let id = post(&vink, other(), None, "Chờ người duyệt").await;
    let approved = moderate(&vink, id, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(
        (approved.review(), approved.version().get()),
        (Review::Approved, 2)
    );
    // Old expected versions lose; a new reaffirmation advances the decision fence.
    assert_eq!(
        moderate(&vink, id, 1, ModerationAction::Reject).await,
        Err(DiscussionError::ModerationConflict { current: 2 })
    );
    let confirmed = moderate(&vink, id, 2, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(confirmed.version().get(), 3);
    let removed = moderate(&vink, id, 3, ModerationAction::Remove)
        .await
        .unwrap();
    assert_eq!(removed.version().get(), 4);
    for action in [ModerationAction::Approve, ModerationAction::Reject] {
        assert_eq!(
            moderate(&vink, id, 4, action).await,
            Err(DiscussionError::Removed)
        );
    }
    // Moderation keeps the text for audit, but no read path returns it.
    let body: Option<String> = sqlx::query_scalar("SELECT body FROM comments WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(body.as_deref(), Some("Chờ người duyệt"));
    for viewer in [None, other(), author()] {
        assert!(listed_ids(&roots(&vink, viewer).await).is_empty());
    }
    let audit: Vec<(i64, String, Uuid, Option<String>, bool)> = sqlx::query_as(
        "SELECT d.version, d.action, d.moderator_id, d.reason, g.principal_id = d.moderator_id
           FROM comment_moderation_decisions d
           JOIN discussion_moderator_grants g ON g.id = d.grant_id
          WHERE d.comment_id = $1 ORDER BY d.version",
    )
    .bind(id.as_uuid())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit,
        vec![
            (2, "approve".to_string(), MODERATOR, None, true),
            (3, "approve".to_string(), MODERATOR, None, true),
            (
                4,
                "remove".to_string(),
                MODERATOR,
                Some(REASON.to_string()),
                true
            )
        ]
    );
    let error = sqlx::query("UPDATE comment_moderation_decisions SET action = 'approve'")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(constraint(error).contains("append-only"));
    assert_eq!(
        moderate(
            &vink,
            CommentId::from_uuid(Uuid::now_v7()),
            1,
            ModerationAction::Approve
        )
        .await,
        Err(DiscussionError::NotFound)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn hidden_roots_anchor_public_replies_and_hidden_contexts_degrade(pool: PgPool) {
    let vink = discussion(&pool).await;
    let root = published(&vink, author(), None, "Gốc sẽ bị gỡ").await;
    let first = published(&vink, other(), Some(root), "Trả lời trực tiếp").await;
    let second = published(&vink, author(), Some(first), "Trả lời trả lời").await;
    let pending = post(&vink, other(), Some(second), "Còn chờ").await;

    // Replies to replies stay in one flat list under the root, with context.
    let page = vink
        .replies(None, sample_work(), root, None, None)
        .await
        .unwrap();
    let contexts: Vec<_> = page.replies.iter().map(|r| (r.id, r.context)).collect();
    assert_eq!(
        contexts,
        vec![
            (first, ReplyContext::Root),
            (second, ReplyContext::Reply(first))
        ]
    );
    // Replying to a pending or unknown comment is refused, even by its own writer.
    for target in [pending, CommentId::from_uuid(Uuid::now_v7())] {
        assert_eq!(
            vink.submit_comment(other(), command(Some(target), "Không được", &key()))
                .await,
            Err(DiscussionError::TargetUnavailable)
        );
    }

    vink.remove_own_comment(author(), sample_work(), root)
        .await
        .unwrap();
    moderate(&vink, first, 2, ModerationAction::Remove)
        .await
        .unwrap();
    let page = roots(&vink, None).await;
    let [anchor] = page.roots.as_slice() else {
        panic!("one anchored root expected: {page:?}")
    };
    assert!(matches!(anchor.entry, RootEntry::Anchor { id, .. } if id == root));
    assert_eq!(anchor.public_replies, 1);
    assert_eq!(anchor.preview.len(), 1);
    assert_eq!(anchor.preview[0].id, second);
    assert_eq!(anchor.preview[0].context, ReplyContext::Unavailable);
    // The writer of the pending reply still sees it, as their own pending reply.
    let own = roots(&vink, other()).await;
    let own_ids: Vec<_> = own.roots[0]
        .preview
        .iter()
        .map(|r| (r.id, r.view))
        .collect();
    assert_eq!(
        own_ids,
        vec![
            (second, CommentView::Public),
            (pending, CommentView::OwnPending)
        ]
    );
    assert_eq!(
        own.roots[0].public_replies, 1,
        "own pending replies are not counted"
    );
    let serialized = format!("{page:?}");
    for leaked in ["Gốc sẽ bị gỡ", "Trả lời trực tiếp", "Còn chờ"] {
        assert!(!serialized.contains(leaked), "{leaked} leaked");
    }

    // Once no public reply remains, the anchor disappears and its replies are unreadable.
    moderate(&vink, second, 2, ModerationAction::Reject)
        .await
        .unwrap();
    assert!(listed_ids(&roots(&vink, None).await).is_empty());
    assert_eq!(
        vink.replies(None, sample_work(), root, None, None).await,
        Err(DiscussionError::NotFound)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn pages_follow_the_keyset_order_and_previews_continue_on_the_reply_page(pool: PgPool) {
    let vink = discussion(&pool).await;
    let first_root = published(&vink, other(), None, "Gốc 0").await;
    let thread = thread_of_sample(&pool).await;
    // Five more roots in ONE transaction share created_at, so only the ID breaks ties.
    let mut tx = pool.begin().await.unwrap();
    let mut tied = Vec::new();
    for _ in 0..5 {
        let id = Uuid::now_v7();
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body,
                                   review, moderation_version, admission_policy, admission_reason)
             VALUES ($1, $2, 'root', false, $3, 'Gốc cùng thời điểm', {PREMODERATED})"
        )))
        .bind(id)
        .bind(thread)
        .bind(dev_seed::OTHER_PRINCIPAL)
        .execute(&mut *tx)
        .await
        .unwrap();
        review_raw(&mut *tx, id, "approved", None).await.unwrap();
        tied.push(CommentId::from_uuid(id));
    }
    tx.commit().await.unwrap();
    tied.sort();
    let mut replies = Vec::new();
    for n in 0..5 {
        replies.push(published(&vink, author(), Some(first_root), &format!("Trả lời {n}")).await);
    }

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = vink
            .discussion(None, sample_work(), cursor.as_deref(), Some(2))
            .await
            .unwrap();
        assert!(page.roots.len() <= 2);
        seen.extend(listed_ids(&page));
        match page.next {
            Some(next) => cursor = Some(next.encode()),
            None => break,
        }
    }
    let mut expected = vec![first_root];
    expected.extend(tied);
    assert_eq!(
        seen, expected,
        "every root exactly once, ties ordered by ID"
    );

    let page = roots(&vink, None).await;
    let head = &page.roots[0];
    assert_eq!(head.public_replies, 5);
    let preview: Vec<_> = head.preview.iter().map(|r| r.id).collect();
    assert_eq!(preview, replies[..usize::from(PREVIEW_REPLIES)]);
    let after = head
        .preview_next
        .expect("more replies after the preview")
        .encode();
    let rest = vink
        .replies(None, sample_work(), first_root, Some(&after), None)
        .await
        .unwrap();
    let rest: Vec<_> = rest.replies.iter().map(|r| r.id).collect();
    assert_eq!(rest, replies[usize::from(PREVIEW_REPLIES)..]);
    assert!(page.roots[1].preview_next.is_none());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn sql_visibility_filters_match_the_domain_projection(pool: PgPool) {
    let vink = discussion(&pool).await;
    published(&vink, other(), None, "Mở thread").await;
    let thread = thread_of_sample(&pool).await;
    // One root per (review, removal) state and writer; no replies, so anchors never apply.
    let mut rows = Vec::new();
    for writer in [dev_seed::OTHER_PRINCIPAL, dev_seed::SAMPLE_AUTHOR] {
        for review in ["pending", "approved", "rejected"] {
            for removal in [None, Some("author"), Some("moderation")] {
                let id = Uuid::now_v7();
                sqlx::query(AssertSqlSafe(format!(
                    "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id,
                                           body, review, moderation_version,
                                           admission_policy, admission_reason)
                     VALUES ($1, $2, 'root', false, $3, 'Trạng thái', {PREMODERATED})"
                )))
                .bind(id)
                .bind(thread)
                .bind(writer)
                .execute(&pool)
                .await
                .unwrap();
                if (review, removal) != ("pending", None) {
                    review_raw(&pool, id, review, removal).await.unwrap();
                }
                rows.push(id);
            }
        }
    }
    let states: Vec<(Uuid, Uuid, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, author_id, review, removal, moderation_version FROM comments ORDER BY created_at, id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(states.len(), rows.len() + 1);
    let moderator = Some(PrincipalId::from_uuid(MODERATOR));
    for viewer in [None, other(), author(), moderator] {
        let mut expected = Vec::new();
        for (id, writer, review, removal, version) in &states {
            let review = match review.as_str() {
                "pending" => Review::Pending,
                "approved" => Review::Approved,
                _ => Review::Rejected,
            };
            let removal = match removal.as_deref() {
                None => None,
                Some("author") => Some(vink_app::domain::discussion::lifecycle::Removal::ByAuthor),
                _ => Some(vink_app::domain::discussion::lifecycle::Removal::ByModeration),
            };
            let state = CommentState::restore(
                review,
                removal,
                vink_app::domain::discussion::lifecycle::ModerationVersion::new(*version).unwrap(),
            );
            let seen = view(PrincipalId::from_uuid(*writer), state, viewer);
            if let Some(RootListing::Shown(_)) = root_listing(seen, 0) {
                expected.push(CommentId::from_uuid(*id));
            }
        }
        assert_eq!(
            listed_ids(&roots(&vink, viewer).await),
            expected,
            "viewer {viewer:?}"
        );
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_withdrawn_work_closes_its_discussion_but_writers_keep_control_of_their_words(
    pool: PgPool,
) {
    let vink = discussion(&pool).await;
    let mine = published(&vink, other(), None, "Bình luận trước khi rút").await;
    // No withdraw command exists yet; removing the pointer is what withdrawal will do.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    for viewer in [None, other(), author()] {
        assert_eq!(
            vink.discussion(viewer, sample_work(), None, None).await,
            Err(DiscussionError::NotFound)
        );
    }
    assert_eq!(
        vink.submit_comment(other(), command(Some(mine), "Sau khi rút", &key()))
            .await,
        Err(DiscussionError::NotFound)
    );
    assert_eq!(
        vink.remove_own_comment(author(), sample_work(), mine).await,
        Err(DiscussionError::NotFound),
        "a withdrawn thread reveals nothing to non-writers"
    );
    assert_eq!(
        vink.remove_own_comment(other(), sample_work(), mine).await,
        Ok(OwnStatus::Removed)
    );
    let unknown = Some(PrincipalId::from_uuid(Uuid::now_v7()));
    assert_eq!(
        vink.submit_comment(unknown, command(None, "Ai đây?", &key()))
            .await,
        Err(DiscussionError::Unauthenticated)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_http_api_keeps_every_discussion_outcome_distinct(pool: PgPool) {
    let vink = discussion(&pool).await;
    let app = app(&pool).await;
    let base = format!("/api/v1/works/{}/discussion", dev_seed::SAMPLE_WORK);
    let comments = format!("{base}/comments");
    let writer = Some(dev_seed::OTHER_PRINCIPAL);
    let work_author = Some(dev_seed::SAMPLE_AUTHOR);
    let origin = Some(ORIGIN);
    let submit = |target: Option<String>, body: &str, key: &str| {
        Some(json!({"target_id": target, "body": body, "command_key": key}))
    };
    let code = |body: &str| json(body)["error"]["code"].as_str().unwrap().to_string();
    let field = |body: &str| json(body)["error"]["field"].as_str().map(str::to_string);

    // Anyone reads a public work's discussion; nothing here may be shared-cached.
    let (status, cache, body) = call(&app, "GET", &base, None, None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    let page = json(&body);
    assert_eq!(
        page["settings"],
        json!({"comments_open": true, "generation": 1})
    );
    assert_eq!(page["roots"], json!([]));

    // Authentication, then Origin, then the body.
    let first = submit(None, "Đọc xong muốn đọc lại 📖", "http-key-0001");
    let (status, _, body) = call(&app, "POST", &comments, None, origin, first.clone()).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::UNAUTHORIZED, "unauthenticated")
    );
    let (status, _, body) = call(&app, "POST", &comments, writer, None, first.clone()).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "forbidden")
    );

    let (status, cache, body) = call(&app, "POST", &comments, writer, origin, first.clone()).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::CREATED, Some("no-store"))
    );
    let created = json(&body);
    assert_eq!(
        (created["status"].as_str(), created["replayed"].as_bool()),
        (Some("pending"), Some(false))
    );
    let root_id = created["comment_id"].as_str().unwrap().to_string();
    // A retry after a lost response replays with 200; the same key with new text is refused.
    let (status, _, body) = call(&app, "POST", &comments, writer, origin, first).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["comment_id"], root_id.as_str());
    assert_eq!(json(&body)["replayed"], true);
    let changed = submit(None, "Nội dung khác", "http-key-0001");
    let (status, _, body) = call(&app, "POST", &comments, writer, origin, changed).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        (code(&body), field(&body)),
        ("idempotency_key_reused".into(), Some("command_key".into()))
    );
    for (request, expected_field) in [
        (
            submit(Some("không-phải-id".into()), "x", "http-key-0002"),
            "target_id",
        ),
        (submit(None, "  \n ", "http-key-0003"), "body"),
        (submit(None, "x", "ngắn"), "command_key"),
        (
            Some(
                json!({"body": "x", "command_key": "http-key-0004", "target_id": null, "author_id": dev_seed::SAMPLE_AUTHOR}),
            ),
            "body",
        ),
    ] {
        let (status, _, body) = call(&app, "POST", &comments, writer, origin, request).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(field(&body).as_deref(), Some(expected_field), "{body}");
    }

    // Pending: only its writer sees it; the work's author sees nothing and may not remove it.
    let (_, _, body) = call(&app, "GET", &base, writer, None, None).await;
    let own = &json(&body)["roots"][0]["root"]["comment"];
    assert_eq!(
        (own["visibility"].as_str(), own["can_remove"].as_bool()),
        (Some("own_pending"), Some(true))
    );
    let (_, _, body) = call(&app, "GET", &base, work_author, None, None).await;
    assert_eq!(json(&body)["roots"], json!([]));
    assert_eq!(json(&body)["viewer_is_work_author"], true);
    let root_path = format!("{comments}/{root_id}");
    let (status, _, _) = call(&app, "DELETE", &root_path, work_author, origin, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let root = CommentId::parse(&root_id).unwrap();
    moderate(&vink, root, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let (_, _, body) = call(&app, "GET", &base, work_author, None, None).await;
    let public = &json(&body)["roots"][0]["root"]["comment"];
    assert_eq!(
        (
            public["visibility"].as_str(),
            public["can_remove"].as_bool()
        ),
        (Some("public"), Some(false))
    );
    assert_eq!(public["reply_to"], Value::Null);
    let (status, _, body) = call(&app, "DELETE", &root_path, work_author, origin, None).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "not_comment_writer")
    );

    // Replies: flat under the root, with context; an unknown target is a conflict, not a 404.
    let reply = submit(Some(root_id.clone()), "Cảm ơn bạn đã đọc.", "http-key-0005");
    let (status, _, _) = call(&app, "POST", &comments, work_author, origin, reply).await;
    assert_eq!(status, StatusCode::CREATED);
    let ghost = submit(
        Some(Uuid::now_v7().to_string()),
        "Trả lời ai?",
        "http-key-0006",
    );
    let (status, _, body) = call(&app, "POST", &comments, writer, origin, ghost).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "target_unavailable")
    );
    let replies = format!("{base}/roots/{root_id}/replies");
    let (status, cache, body) = call(&app, "GET", &replies, work_author, None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    let mine = &json(&body)["replies"][0];
    assert_eq!(mine["reply_to"], json!({"kind": "root"}));
    assert_eq!(mine["visibility"], "own_pending");
    let (_, _, body) = call(&app, "GET", &replies, None, None, None).await;
    assert_eq!(json(&body)["replies"], json!([]));

    // Settings: only the work's author; a stale tab conflicts; closed blocks new comments.
    let settings = format!("{base}/settings");
    let close = Some(json!({"expected_generation": 1, "comments_open": false}));
    let (status, _, _) = call(&app, "PUT", &settings, writer, origin, close.clone()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, body) = call(&app, "PUT", &settings, work_author, origin, close).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json(&body),
        json!({"comments_open": false, "generation": 2})
    );
    let stale = Some(json!({"expected_generation": 1, "comments_open": true}));
    let (status, _, body) = call(&app, "PUT", &settings, work_author, origin, stale).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "version_conflict")
    );
    assert_eq!(json(&body)["error"]["current_version"], 2);
    // Asking for the current state: at an older generation it writes nothing; at the current one
    // it is accepted and advances the generation, which fences an older in-flight reopen.
    let retry = Some(json!({"expected_generation": 1, "comments_open": false}));
    let (status, _, body) = call(&app, "PUT", &settings, work_author, origin, retry).await;
    assert_eq!(
        (status, json(&body)),
        (
            StatusCode::OK,
            json!({"comments_open": false, "generation": 2})
        )
    );
    let keep = Some(json!({"expected_generation": 2, "comments_open": false}));
    let (status, _, body) = call(&app, "PUT", &settings, work_author, origin, keep).await;
    assert_eq!(
        (status, json(&body)),
        (
            StatusCode::OK,
            json!({"comments_open": false, "generation": 3})
        )
    );
    let delayed = Some(json!({"expected_generation": 2, "comments_open": true}));
    let (status, _, body) = call(&app, "PUT", &settings, work_author, origin, delayed).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "version_conflict")
    );
    assert_eq!(json(&body)["error"]["current_version"], 3);
    let late = submit(None, "Muộn rồi", "http-key-0007");
    let (status, _, body) = call(&app, "POST", &comments, writer, origin, late).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "comments_closed")
    );

    // The writer removes their own comment, repeatably; the text is gone from every reader.
    for _ in 0..2 {
        let (status, _, body) = call(&app, "DELETE", &root_path, writer, origin, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json(&body),
            json!({"comment_id": root_id, "status": "removed"})
        );
    }
    let (_, _, body) = call(&app, "GET", &base, None, None, None).await;
    assert!(!body.contains("Đọc xong muốn đọc lại"));

    // Paging input and identifiers.
    for (query, expected_field) in [
        ("?cursor=c9.x", "cursor"),
        ("?limit=0", "limit"),
        ("?limit=abc", "query"),
    ] {
        let (status, _, body) =
            call(&app, "GET", &format!("{base}{query}"), None, None, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(field(&body).as_deref(), Some(expected_field), "{query}");
    }
    for path in [
        "/api/v1/works/not-a-uuid/discussion".to_string(),
        format!("/api/v1/works/{}/discussion", Uuid::now_v7()),
        format!("{base}/roots/not-a-uuid/replies"),
    ] {
        let (status, _, _) = call(&app, "GET", &path, None, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

async fn decide(
    vink: &Vink<PgStore>,
    actor: Option<PrincipalId>,
    comment_id: CommentId,
    expected_version: i64,
    action: ModerationAction,
) -> Result<CommentState, DiscussionError> {
    let reason = (action != ModerationAction::Approve).then(|| REASON.to_string());
    vink.moderate_comment(
        actor,
        ModerateComment {
            comment_id,
            expected_version,
            action,
            reason,
        },
    )
    .await
}

async fn review_of(pool: &PgPool, id: CommentId) -> (String, i64) {
    sqlx::query_as("SELECT review, moderation_version FROM comments WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderation_authority_is_a_revocable_grant_rechecked_at_commit(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    let mod_id = PrincipalId::from_uuid(MODERATOR);
    let note = GrantNote::parse("lặp lại").unwrap();
    let active = active_moderators(&pool).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(
        grant_moderator(&pool, mod_id, &note).await.unwrap(),
        GrantOutcome::AlreadyActive(active[0].grant_id),
        "granting twice keeps one active grant"
    );
    assert_eq!(
        grant_moderator(&pool, PrincipalId::from_uuid(Uuid::now_v7()), &note).await,
        Err(vink_app::application::ports::StoreError::UnknownPrincipal)
    );

    // Without a grant nobody learns anything about comments, not even whether one exists.
    let first = post(&vink, other(), None, "Chờ duyệt một").await;
    for target in [first, CommentId::from_uuid(Uuid::now_v7())] {
        assert_eq!(
            decide(&vink, other(), target, 1, ModerationAction::Approve).await,
            Err(DiscussionError::NotModerator)
        );
    }
    assert_eq!(
        vink.moderation_queue(author(), None, None).await,
        Err(DiscussionError::NotModerator)
    );

    // A revocation in flight holds the grant row: the decision waits, then sees it revoked.
    let mut revoking = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE discussion_moderator_grants SET revoked_at = now()
          WHERE principal_id = $1 AND revoked_at IS NULL",
    )
    .bind(MODERATOR)
    .execute(&mut *revoking)
    .await
    .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            decide(&vink, moderator(), first, 1, ModerationAction::Approve).await
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !deciding.is_finished(),
        "decision must wait for the revocation"
    );
    revoking.rollback().await.unwrap();
    // Rolled back: the grant is still active, so the waiting decision now applies.
    assert_eq!(deciding.await.unwrap().unwrap().review(), Review::Approved);

    // A REAL decision in flight (paused on its comment row after taking the grant share lock)
    // makes the operator's revocation wait; the decision commits under the authority it had.
    let second = post(&vink, other(), None, "Chờ duyệt hai").await;
    let mut pauser = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(second.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            decide(&vink, moderator(), second, 1, ModerationAction::Reject).await
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!deciding.is_finished(), "decision is paused on its comment");
    let revoke = {
        let pool = pool.clone();
        tokio::spawn(
            async move { revoke_moderator(&pool, PrincipalId::from_uuid(MODERATOR)).await },
        )
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !revoke.is_finished(),
        "revocation must wait for the in-flight decision"
    );
    pauser.rollback().await.unwrap();
    assert_eq!(deciding.await.unwrap().unwrap().review(), Review::Rejected);
    assert!(revoke.await.unwrap().unwrap().is_some());

    // Revoked: no queue, no decisions; the revoked grant cannot be revived, only re-granted.
    let third = post(&vink, other(), None, "Chờ duyệt ba").await;
    assert_eq!(
        decide(&vink, moderator(), third, 1, ModerationAction::Approve).await,
        Err(DiscussionError::NotModerator)
    );
    assert_eq!(review_of(&pool, third).await, ("pending".into(), 1));
    assert_eq!(
        vink.moderation_queue(moderator(), None, None).await,
        Err(DiscussionError::NotModerator)
    );
    assert_eq!(revoke_moderator(&pool, mod_id).await.unwrap(), None);
    let error = sqlx::query("UPDATE discussion_moderator_grants SET revoked_at = NULL")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(constraint(error).contains("only be revoked once"));
    assert!(matches!(
        grant_moderator(&pool, mod_id, &note).await.unwrap(),
        GrantOutcome::Granted(_)
    ));
    assert_eq!(
        decide(&vink, moderator(), third, 1, ModerationAction::Approve)
            .await
            .unwrap()
            .review(),
        Review::Approved
    );
    // Each decision names the grant that authorized it.
    let grants: i64 =
        sqlx::query_scalar("SELECT count(DISTINCT grant_id) FROM comment_moderation_decisions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(grants, 2);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderators_recuse_from_their_own_comments_and_their_own_works(pool: PgPool) {
    let vink = discussion(&pool).await;
    // The work's author is ALSO a moderator: ownership still grants nothing over its discussion.
    let note = GrantNote::parse("Tác giả kiêm kiểm duyệt").unwrap();
    grant_moderator(
        &pool,
        PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR),
        &note,
    )
    .await
    .unwrap();
    let readers = post(&vink, other(), None, "Của bạn đọc").await;
    let moderators_own = post(&vink, moderator(), None, "Của kiểm duyệt viên").await;

    assert_eq!(
        decide(&vink, author(), readers, 1, ModerationAction::Remove).await,
        Err(DiscussionError::Recused)
    );
    assert_eq!(
        decide(
            &vink,
            moderator(),
            moderators_own,
            1,
            ModerationAction::Approve
        )
        .await,
        Err(DiscussionError::Recused)
    );
    for id in [readers, moderators_own] {
        assert_eq!(review_of(&pool, id).await, ("pending".into(), 1));
    }
    let queue = vink
        .moderation_queue(moderator(), None, None)
        .await
        .unwrap();
    let flags: Vec<_> = queue
        .items
        .iter()
        .map(|i| (i.comment_id, i.can_decide))
        .collect();
    assert_eq!(queue.pending_total, 1);
    assert_eq!(flags, vec![(readers, true)]);
    let authors_view = vink.moderation_queue(author(), None, None).await.unwrap();
    assert_eq!(authors_view.pending_total, 0);
    assert!(authors_view.items.is_empty());
    assert_eq!(
        decide(&vink, moderator(), readers, 1, ModerationAction::Approve)
            .await
            .unwrap()
            .review(),
        Review::Approved
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn rejection_reasons_reach_only_their_writer_and_are_required_in_sql(pool: PgPool) {
    let vink = discussion(&pool).await;
    let id = post(&vink, other(), None, "Tác giả thật là anh X ở phố Y.").await;
    assert_eq!(
        vink.moderate_comment(
            moderator(),
            ModerateComment {
                comment_id: id,
                expected_version: 1,
                action: ModerationAction::Reject,
                reason: None,
            },
        )
        .await
        .map(|_| ()),
        Err(DiscussionError::Invalid(
            vink_app::application::discussion::DiscussionInput::Reason(
                vink_app::domain::discussion::moderation::ReasonError::Required
            )
        ))
    );
    decide(&vink, moderator(), id, 1, ModerationAction::Reject)
        .await
        .unwrap();
    let own = roots(&vink, other()).await;
    let RootEntry::Comment(item) = &own.roots[0].entry else {
        panic!("own rejected comment expected")
    };
    assert_eq!(item.view, CommentView::OwnRejected);
    assert_eq!(item.rejection_reason.as_deref(), Some(REASON));
    for viewer in [None, author(), moderator()] {
        let page = roots(&vink, viewer).await;
        assert!(page.roots.is_empty());
        assert!(!format!("{page:?}").contains(REASON));
    }
    // Reversed on review: public again, and the old reason is no longer shown to anyone.
    decide(&vink, moderator(), id, 2, ModerationAction::Approve)
        .await
        .unwrap();
    let RootEntry::Comment(item) = &roots(&vink, other()).await.roots[0].entry else {
        panic!("public comment expected")
    };
    assert_eq!(
        (item.view, item.rejection_reason.as_deref()),
        (CommentView::Public, None)
    );

    // The schema itself refuses unexplained or unauthorized decisions.
    for (statement, expected) in [
        (
            "INSERT INTO comment_moderation_decisions (comment_id, version, action, moderator_id, grant_id)
             SELECT $1, 9, 'reject', $2, id FROM discussion_moderator_grants LIMIT 1",
            "comment_moderation_decisions_reason_required",
        ),
        (
            "INSERT INTO comment_moderation_decisions (comment_id, version, action, moderator_id)
             VALUES ($1, 9, 'approve', $2)",
            "comment_moderation_decisions_provenance_required",
        ),
    ] {
        let error = sqlx::query(statement)
            .bind(id.as_uuid())
            .bind(MODERATOR)
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(constraint(error), expected);
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_queue_lists_pending_present_comments_oldest_first(pool: PgPool) {
    let vink = discussion(&pool).await;
    let mut pending = Vec::new();
    for n in 0..5 {
        pending.push(post(&vink, other(), None, &format!("Chờ {n}")).await);
    }
    let approved = published(&vink, other(), None, "Đã duyệt").await;
    let withdrawn = post(&vink, other(), None, "Tự gỡ").await;
    vink.remove_own_comment(other(), sample_work(), withdrawn)
        .await
        .unwrap();

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let queue = vink
            .moderation_queue(moderator(), cursor.as_deref(), Some(2))
            .await
            .unwrap();
        assert_eq!(queue.pending_total, 5);
        for item in &queue.items {
            assert_eq!(item.work_title.as_deref(), Some(dev_seed::SAMPLE_TITLE));
            assert_eq!(item.version.get(), 1);
            assert!(item.is_root);
        }
        seen.extend(queue.items.iter().map(|item| item.comment_id));
        match queue.next {
            Some(next) => cursor = Some(next.encode()),
            None => break,
        }
    }
    assert_eq!(seen, pending);
    assert!(!seen.contains(&approved) && !seen.contains(&withdrawn));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_http_moderation_api_keeps_every_outcome_distinct(pool: PgPool) {
    let vink = discussion(&pool).await;
    let app = app(&pool).await;
    let queue = "/api/v1/moderation/comments";
    let origin = Some(ORIGIN);
    let code = |body: &str| json(body)["error"]["code"].as_str().unwrap().to_string();
    let target = post(&vink, other(), None, "Chờ người duyệt qua HTTP").await;
    let decisions = format!("{queue}/{target}/decisions");
    let reject = Some(json!({"expected_version": 1, "action": "reject", "reason": REASON}));

    let (status, _, _) = call(&app, "GET", queue, None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, body) = call(
        &app,
        "GET",
        queue,
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "not_moderator")
    );
    let (status, cache, body) = call(&app, "GET", queue, Some(MODERATOR), None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    let listed = json(&body);
    assert_eq!(listed["pending_total"], 1);
    assert_eq!(listed["items"][0]["comment_id"], target.to_string());
    assert_eq!(listed["items"][0]["can_decide"], true);

    let (status, _, _) = call(&app, "POST", &decisions, None, origin, reject.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, body) = call(
        &app,
        "POST",
        &decisions,
        Some(MODERATOR),
        None,
        reject.clone(),
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "forbidden")
    );
    let (status, _, body) = call(
        &app,
        "POST",
        &decisions,
        Some(dev_seed::SAMPLE_AUTHOR),
        origin,
        reject.clone(),
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "not_moderator")
    );
    for (request, field) in [
        (json!({"expected_version": 1, "action": "reject"}), "reason"),
        (
            json!({"expected_version": 0, "action": "approve"}),
            "expected_version",
        ),
        (json!({"expected_version": 1, "action": "restore"}), "body"),
        (
            json!({"expected_version": 1, "action": "approve", "moderator_id": MODERATOR}),
            "body",
        ),
    ] {
        let (status, _, body) = call(
            &app,
            "POST",
            &decisions,
            Some(MODERATOR),
            origin,
            Some(request),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(json(&body)["error"]["field"], field, "{body}");
    }
    let (status, cache, body) = call(
        &app,
        "POST",
        &decisions,
        Some(MODERATOR),
        origin,
        reject.clone(),
    )
    .await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(
        json(&body),
        json!({"comment_id": target.to_string(), "review": "rejected", "removed": false, "version": 2})
    );
    let (status, _, body) = call(&app, "POST", &decisions, Some(MODERATOR), origin, reject).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "version_conflict")
    );
    assert_eq!(json(&body)["error"]["current_version"], 2);
    // The writer reads the reason on their own rejected comment, through the public API.
    let page = format!("/api/v1/works/{}/discussion", dev_seed::SAMPLE_WORK);
    let (_, _, body) = call(
        &app,
        "GET",
        &page,
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(
        json(&body)["roots"][0]["root"]["comment"]["rejection_reason"],
        REASON
    );

    let remove = Some(json!({"expected_version": 2, "action": "remove", "reason": REASON}));
    let (status, _, body) = call(&app, "POST", &decisions, Some(MODERATOR), origin, remove).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["removed"], true);
    let approve = Some(json!({"expected_version": 3, "action": "approve"}));
    let (status, _, body) = call(&app, "POST", &decisions, Some(MODERATOR), origin, approve).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "comment_removed")
    );

    let own = post(&vink, moderator(), None, "Kiểm duyệt viên tự viết").await;
    let own_path = format!("{queue}/{own}/decisions");
    let approve = Some(json!({"expected_version": 1, "action": "approve"}));
    let (status, _, body) = call(
        &app,
        "POST",
        &own_path,
        Some(MODERATOR),
        origin,
        approve.clone(),
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "moderator_recused")
    );
    for path in [
        format!("{queue}/{}/decisions", Uuid::now_v7()),
        format!("{queue}/not-a-uuid/decisions"),
    ] {
        let (status, _, _) = call(
            &app,
            "POST",
            &path,
            Some(MODERATOR),
            origin,
            approve.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

async fn admission_of(pool: &PgPool, id: CommentId) -> (String, String, String, i64) {
    sqlx::query_as(
        "SELECT admission_policy, admission_reason, review, moderation_version
           FROM comments WHERE id = $1",
    )
    .bind(id.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

/// (comments, submission receipts, threads): what a refused submission must leave unchanged.
async fn written(pool: &PgPool) -> (i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM comments), (SELECT count(*) FROM discussion_command_receipts),
                (SELECT count(*) FROM discussion_threads)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A direct root INSERT with the given `review, removal, version, policy, reason` values.
async fn raw_admitted(pool: &PgPool, thread: Uuid, values: &str) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query(AssertSqlSafe(format!(
        "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                               removal, moderation_version, admission_policy, admission_reason)
         VALUES ($1, $2, 'root', false, $3, 'Nhập thẳng', {values})"
    )))
    .bind(id)
    .bind(thread)
    .bind(dev_seed::OTHER_PRINCIPAL)
    .execute(pool)
    .await?;
    Ok(id)
}

fn suspension_reason() -> SuspensionReason {
    SuspensionReason::parse("Tạm khóa trong bộ test thảo luận").unwrap()
}

/// #120: each comment records the admission that chose its initial state. The schema alone keeps
/// a row from being born public (or reviewed, removed, later) under premoderation, from naming an
/// unknown or not-yet-accepted policy, and from rewriting its provenance; review still moves on.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn admission_provenance_is_recorded_and_held_by_the_schema(pool: PgPool) {
    let vink = discussion(&pool).await;
    let root = post(&vink, other(), None, "Gốc chờ duyệt").await;
    let premoderated = (
        "premoderation.v1".to_string(),
        "policy_premoderation".to_string(),
    );
    assert_eq!(
        admission_of(&pool, root).await,
        (
            premoderated.0.clone(),
            premoderated.1.clone(),
            "pending".into(),
            1
        )
    );
    let thread = thread_of_sample(&pool).await;

    // The lawful shape passes the same path, so the refusals below are not vacuous.
    raw_admitted(
        &pool,
        thread,
        "'pending', NULL, 1, 'premoderation.v1', 'policy_premoderation'",
    )
    .await
    .unwrap();
    for values in [
        "'approved', NULL, 1, 'premoderation.v1', 'policy_premoderation'",
        "'rejected', NULL, 1, 'premoderation.v1', 'policy_premoderation'",
        "'pending', NULL, 2, 'premoderation.v1', 'policy_premoderation'",
        "'pending', 'moderation', 1, 'premoderation.v1', 'policy_premoderation'",
    ] {
        let error = raw_admitted(&pool, thread, values).await.unwrap_err();
        assert!(
            constraint(error).contains("disagrees with its admission"),
            "{values}"
        );
    }
    // The candidate cannot be effective in the pilot, even with a consistent approval.
    let error = raw_admitted(
        &pool,
        thread,
        "'approved', NULL, 1, 'trusted-cohort.v1', 'trusted_and_clear'",
    )
    .await
    .unwrap_err();
    assert_eq!(constraint(error), "comments_admission_premoderation_only");
    let error = raw_admitted(
        &pool,
        thread,
        "'pending', NULL, 1, 'premoderation.v1', 'trust_unknown'",
    )
    .await
    .unwrap_err();
    assert_eq!(constraint(error), "comments_admission_known");
    let error = raw_admitted(
        &pool,
        thread,
        "'pending', NULL, 1, 'auto.v0', 'policy_premoderation'",
    )
    .await
    .unwrap_err();
    assert!(constraint(error).starts_with("comments_admission_"));
    // No default: a writer that does not name its admission is refused.
    let error = sqlx::query(
        "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                               moderation_version)
         VALUES ($1, $2, 'root', false, $3, 'Không nguồn gốc', 'pending', 1)",
    )
    .bind(Uuid::now_v7())
    .bind(thread)
    .bind(dev_seed::OTHER_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error
            .as_database_error()
            .and_then(|db| db.code())
            .as_deref(),
        Some("23502"),
        "not_null_violation"
    );

    // Provenance never changes; a moderation decision still advances the review.
    let error = sqlx::query(
        "UPDATE comments SET admission_policy = 'trusted-cohort.v1',
                             admission_reason = 'trusted_and_clear' WHERE id = $1",
    )
    .bind(root.as_uuid())
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(constraint(error).contains("provenance is immutable"));
    moderate(&vink, root, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(
        admission_of(&pool, root).await,
        (premoderated.0, premoderated.1, "approved".into(), 2)
    );
}

/// #120 standing fence: a suspension committed before the submission's commit refuses it, exact
/// retries and replies included, and leaves no comment, receipt or thread binding behind. Over
/// HTTP the test resolver ignores suspensions, so the store's refusal itself answers `403`.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_suspended_writer_is_refused_at_commit_and_nothing_is_written(pool: PgPool) {
    let vink = discussion(&pool).await;
    let writer = PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL);
    suspend_principal(&pool, writer, &suspension_reason())
        .await
        .unwrap();
    // The work's first comment: the thread binding it would create rolls back with it.
    assert_eq!(
        vink.submit_comment(other(), command(None, "Bị chặn", &key()))
            .await,
        Err(DiscussionError::Suspended)
    );
    assert_eq!(written(&pool).await, (0, 0, 0));

    reinstate_principal(&pool, writer).await.unwrap();
    let retry_key = key();
    let first = vink
        .submit_comment(other(), command(None, "Gửi trước khi khóa", &retry_key))
        .await
        .unwrap();
    assert_eq!(first.status, OwnStatus::Pending);
    let before = written(&pool).await;
    suspend_principal(&pool, writer, &suspension_reason())
        .await
        .unwrap();
    for (target, body, command_key) in [
        (None, "Gửi trước khi khóa", retry_key.clone()),
        (Some(first.comment_id), "Trả lời khi đã bị khóa", key()),
    ] {
        assert_eq!(
            vink.submit_comment(other(), command(target, body, &command_key))
                .await,
            Err(DiscussionError::Suspended),
            "{body}"
        );
    }
    assert_eq!(written(&pool).await, before);

    let app = app(&pool).await;
    let comments = format!(
        "/api/v1/works/{}/discussion/comments",
        dev_seed::SAMPLE_WORK
    );
    let (status, cache, body) = call(
        &app,
        "POST",
        &comments,
        Some(dev_seed::OTHER_PRINCIPAL),
        Some(ORIGIN),
        Some(json!({"body": "Qua HTTP", "command_key": key()})),
    )
    .await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::FORBIDDEN, Some("no-store"))
    );
    assert_eq!(json(&body)["error"]["code"], "principal_suspended");
    assert_eq!(written(&pool).await, before);
}

/// #120 standing fence under concurrency, both orders, through the real store: the principal
/// row's FOR UPDATE conflicts with the FOR KEY SHARE that a suspension's foreign key takes.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn suspension_and_submission_serialize_on_the_principal(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    let writer = PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL);
    let root = published(&vink, other(), None, "Gốc").await;
    let reply = published(&vink, author(), Some(root), "Trả lời gốc").await;
    let submit = |target: Option<CommentId>, body: &'static str| {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.submit_comment(other(), command(target, body, &key()))
                .await
        })
    };

    // Order A: an uncommitted suspension holds the principal; the submission waits at its
    // standing fence, then reads the committed suspension and writes nothing.
    let before = written(&pool).await;
    let mut suspending = pool.begin().await.unwrap();
    let pid = backend_pid(&mut suspending).await;
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason)
         VALUES ($1, $2, 'Tạm khóa trong bộ test thảo luận')",
    )
    .bind(Uuid::now_v7())
    .bind(dev_seed::OTHER_PRINCIPAL)
    .execute(&mut *suspending)
    .await
    .unwrap();
    let submitting = submit(None, "Đua với lệnh khóa");
    wait_blocked_by(&pool, pid).await;
    assert!(
        !submitting.is_finished(),
        "the submission waits for the suspension"
    );
    suspending.commit().await.unwrap();
    assert_eq!(submitting.await.unwrap(), Err(DiscussionError::Suspended));
    assert_eq!(written(&pool).await, before);
    reinstate_principal(&pool, writer).await.unwrap();

    // Order B: a submission that already holds the principal makes the real suspension command
    // wait. It is paused on the root row, which its reply-to-reply insert key-shares through the
    // root foreign key only after the standing fence; the comment commits first.
    let mut pauser = pool.begin().await.unwrap();
    let pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(root.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let replying = submit(Some(reply), "Trả lời đang gửi dở");
    wait_blocked_by(&pool, pid).await;
    let suspended = {
        let pool = pool.clone();
        tokio::spawn(async move { suspend_principal(&pool, writer, &suspension_reason()).await })
    };
    wait_blocked(&pool, 2).await;
    assert!(
        !suspended.is_finished(),
        "the suspension waits for the submission"
    );
    pauser.rollback().await.unwrap();
    let created = replying.await.unwrap().unwrap();
    assert_eq!(created.status, OwnStatus::Pending);
    suspended.await.unwrap().unwrap();
    assert_eq!(
        vink.submit_comment(other(), command(None, "Sau khi khóa", &key()))
            .await,
        Err(DiscussionError::Suspended)
    );
}

/// D-05 generalized standing fence: own removal, the author's mode change and a moderator's
/// decision are refused at commit once the actor is suspended, and write nothing. The test
/// resolver ignores suspensions, so only the store's fence can refuse them.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn every_discussion_write_is_refused_at_commit_once_suspended(pool: PgPool) {
    let vink = discussion(&pool).await;
    let mine = published(&vink, other(), None, "Của người viết").await;
    let pending = post(&vink, author(), None, "Chờ duyệt").await;
    for id in [
        dev_seed::OTHER_PRINCIPAL,
        dev_seed::SAMPLE_AUTHOR,
        MODERATOR,
    ] {
        suspend_principal(&pool, PrincipalId::from_uuid(id), &suspension_reason())
            .await
            .unwrap();
    }
    assert_eq!(
        vink.remove_own_comment(other(), sample_work(), mine).await,
        Err(DiscussionError::Suspended)
    );
    assert_eq!(
        vink.set_comments_mode(author(), close_at(1)).await,
        Err(DiscussionError::Suspended)
    );
    assert_eq!(
        moderate(&vink, pending, 1, ModerationAction::Approve).await,
        Err(DiscussionError::Suspended)
    );
    assert_eq!(review_of(&pool, mine).await.0, "approved");
    assert_eq!(review_of(&pool, pending).await, ("pending".to_string(), 1));
    let (open, generation): (bool, i64) = sqlx::query_as(
        "SELECT comments_open, settings_generation FROM discussion_threads WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((open, generation), (true, 1));
    let decisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM comment_moderation_decisions WHERE comment_id = $1",
    )
    .bind(pending.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(decisions, 0);
    // Reinstated: the same commands succeed (the fence holds no state of its own).
    for id in [
        dev_seed::OTHER_PRINCIPAL,
        dev_seed::SAMPLE_AUTHOR,
        MODERATOR,
    ] {
        reinstate_principal(&pool, PrincipalId::from_uuid(id))
            .await
            .unwrap();
    }
    assert!(
        moderate(&vink, pending, 1, ModerationAction::Approve)
            .await
            .is_ok()
    );
    assert!(vink.set_comments_mode(author(), close_at(1)).await.is_ok());
}

/// The moderator's decision holds the comment row, then its standing fence: a suspension that
/// is still uncommitted makes it wait and then refuse; nothing is decided or audited.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_moderation_decision_waits_for_a_pending_suspension_and_refuses(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    let pending = post(&vink, other(), None, "Chờ duyệt").await;
    let mut suspending = pool.begin().await.unwrap();
    let pid = backend_pid(&mut suspending).await;
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason)
         VALUES ($1, $2, 'Tạm khóa trong bộ test thảo luận')",
    )
    .bind(Uuid::now_v7())
    .bind(MODERATOR)
    .execute(&mut *suspending)
    .await
    .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { moderate(&vink, pending, 1, ModerationAction::Approve).await })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(!deciding.is_finished(), "the decision waits at its fence");
    suspending.commit().await.unwrap();
    assert_eq!(deciding.await.unwrap(), Err(DiscussionError::Suspended));
    assert_eq!(review_of(&pool, pending).await, ("pending".to_string(), 1));
}

const ADMISSION_MIGRATION: i64 = 20_261_111_000_000;

/// Upgrade: comments written before #120 were all admitted under premoderation, and the migration
/// records exactly that without touching their review, removal, text or version.
#[sqlx::test(migrations = false)]
async fn upgrade_records_premoderation_for_existing_comments(pool: PgPool) {
    let previous = Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < ADMISSION_MIGRATION)
            .cloned()
            .collect(),
    );
    previous.run(&pool).await.unwrap();
    let vink = seeded(&pool).await;
    let thread = Uuid::now_v7();
    sqlx::query("INSERT INTO discussion_threads (id, work_id) VALUES ($1, $2)")
        .bind(thread)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    // Rows as the old code could leave them: pending, approved, removed by their writer.
    let mut old = Vec::new();
    for (review, removal, version, body) in [
        ("pending", None, 1_i64, Some("Chờ duyệt")),
        ("approved", None, 2, Some("Đã duyệt")),
        ("approved", Some("author"), 3, None),
    ] {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                                   removal, moderation_version)
             VALUES ($1, $2, 'root', false, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(thread)
        .bind(dev_seed::OTHER_PRINCIPAL)
        .bind(body)
        .bind(review)
        .bind(removal)
        .bind(version)
        .execute(&pool)
        .await
        .unwrap();
        old.push((id, review, removal, version, body));
    }

    let report = migrate(&pool).await.unwrap();
    assert_eq!(report.applied.first(), Some(&ADMISSION_MIGRATION));
    for (id, review, removal, version, body) in old {
        let row: (String, String, String, Option<String>, i64, Option<String>) = sqlx::query_as(
            "SELECT admission_policy, admission_reason, review, removal, moderation_version, body
               FROM comments WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            row,
            (
                "premoderation.v1".into(),
                "policy_premoderation".into(),
                review.into(),
                removal.map(str::to_owned),
                version,
                body.map(str::to_owned),
            )
        );
    }
    // The upgraded database admits new comments through the store as before.
    let id = post(&vink, other(), None, "Sau nâng cấp").await;
    assert_eq!(
        admission_of(&pool, id).await,
        (
            "premoderation.v1".into(),
            "policy_premoderation".into(),
            "pending".into(),
            1
        )
    );
}

#[path = "discussion_reads.rs"]
mod reads;

#[path = "comment_moderation_governance.rs"]
mod governance;
