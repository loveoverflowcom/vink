//! Vink-owned discussion on real PostgreSQL (D2T-02a). Use cases run through `PgStore`; integrity
//! negatives bypass Rust with direct SQL, so the schema's own guarantees are what is tested.
//! Constraints and triggers apply to every role; these run as the test-harness superuser.

use std::time::Duration;

use sqlx::Row;
use vink_app::application::discussion::{
    DiscussionError, ModerateComment, ModerationAction, ModeratorGrant, OwnStatus, RootEntry,
    RootPage, SetCommentsMode, SubmitComment,
};
use vink_app::domain::discussion::lifecycle::{CommentState, Review};
use vink_app::domain::discussion::page::PREVIEW_REPLIES;
use vink_app::domain::discussion::settings::CommentsMode;
use vink_app::domain::discussion::visibility::{
    CommentView, ReplyContext, RootListing, root_listing, view,
};
use vink_app::domain::ids::CommentId;

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x5555_5555_5555_4555_8555_5555_5555_5555);

async fn discussion(pool: &PgPool) -> Vink<PgStore> {
    let vink = seeded(pool).await;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Kiểm duyệt viên')")
        .bind(MODERATOR)
        .execute(pool)
        .await
        .unwrap();
    vink
}

fn grant() -> Option<ModeratorGrant> {
    Some(ModeratorGrant::from_authority(PrincipalId::from_uuid(
        MODERATOR,
    )))
}

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
    vink.moderate_comment(
        grant(),
        ModerateComment {
            comment_id,
            expected_version,
            action,
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

/// Inserts a comment row with direct SQL, bypassing every Rust check.
async fn raw_comment(
    pool: &PgPool,
    thread: Uuid,
    kind: &str,
    root: Option<Uuid>,
    reply_to: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, thread_id, kind, root_id, root_kind, reply_to_id,
                               replied_to_reply, author_id, body, review, moderation_version)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'Thử trực tiếp', 'approved', 1)",
    )
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
    // A retry of the same choice is a no-op; a stale tab cannot reopen behind the author's back.
    assert_eq!(vink.set_comments_mode(author(), close(1)).await, Ok(closed));
    let reopen_stale = SetCommentsMode {
        mode: CommentsMode::Open,
        ..close(1)
    };
    assert_eq!(
        vink.set_comments_mode(author(), reopen_stale).await,
        Err(DiscussionError::Conflict { current: 2 })
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
            None,
            ModerateComment {
                comment_id: approved,
                expected_version: 2,
                action: ModerationAction::Remove,
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
    // A second moderator acting on version 1 loses; a repeated decision is a no-op.
    assert_eq!(
        moderate(&vink, id, 1, ModerationAction::Reject).await,
        Err(DiscussionError::Conflict { current: 2 })
    );
    assert_eq!(
        moderate(&vink, id, 2, ModerationAction::Approve).await,
        Ok(approved)
    );
    let removed = moderate(&vink, id, 2, ModerationAction::Remove)
        .await
        .unwrap();
    assert_eq!(removed.version().get(), 3);
    for action in [ModerationAction::Approve, ModerationAction::Reject] {
        assert_eq!(
            moderate(&vink, id, 3, action).await,
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
    let audit: Vec<(i64, String, Uuid)> = sqlx::query_as(
        "SELECT version, action, moderator_id FROM comment_moderation_decisions
          WHERE comment_id = $1 ORDER BY version",
    )
    .bind(id.as_uuid())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit,
        vec![
            (2, "approve".to_string(), MODERATOR),
            (3, "remove".to_string(), MODERATOR)
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
        sqlx::query(
            "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body,
                                   review, moderation_version)
             VALUES ($1, $2, 'root', false, $3, 'Gốc cùng thời điểm', 'approved', 1)",
        )
        .bind(id)
        .bind(thread)
        .bind(dev_seed::OTHER_PRINCIPAL)
        .execute(&mut *tx)
        .await
        .unwrap();
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
                sqlx::query(
                    "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id,
                                           body, review, removal, moderation_version)
                     VALUES ($1, $2, 'root', false, $3, $4, $5, $6, 1)",
                )
                .bind(id)
                .bind(thread)
                .bind(writer)
                .bind((removal != Some("author")).then_some("Trạng thái"))
                .bind(review)
                .bind(removal)
                .execute(&pool)
                .await
                .unwrap();
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
