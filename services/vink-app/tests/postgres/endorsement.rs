//! Literary endorsement (LVE, #73) on real PostgreSQL. Use cases run through `PgStore`, so every
//! write is the production transaction that locks the facts, calls the pure decision and commits.
//! Integrity negatives bypass Rust with direct SQL. Races use a second connection that holds a
//! real lock (or an uncommitted row) and then commits or rolls back, so both commit orders are
//! exercised on purpose rather than by scheduler accident. Publication pointer changes are SQL
//! fixtures: no publish/withdraw command exists yet, so these prove the durable boundary, not a
//! product workflow.

use std::time::Duration;

use vink_app::application::discussion::{
    ModerateComment, ModerationAction, SetCommentsMode, SubmitComment,
};
use vink_app::application::endorsement::{
    Capability, EndorsementError, EndorsementOutcome, OwnEndorsement, SetCommentEndorsement,
    SetWorkEndorsement,
};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::discussion::settings::CommentsMode;
use vink_app::domain::endorsement::{EndorsementCount, EndorsementVersion, OwnView, Summary};
use vink_app::domain::ids::{CommentId, RevisionId};
use vink_app::identity::{IdentityResolver, Resolution, ResolveFuture};
use vink_app::persistence::grant_moderator;

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x6666_6666_6666_4666_8666_6666_6666_6666);
const PAUSE: Duration = Duration::from_millis(300);

fn reader_id(n: u128) -> Uuid {
    Uuid::from_u128(0x7777_7777_7777_4777_8777_0000_0000_0000 + n)
}

fn reader(n: u128) -> Option<PrincipalId> {
    principal(reader_id(n))
}

async fn setup(pool: &PgPool) -> Arc<Vink<PgStore>> {
    let vink = seeded(pool).await;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Kiểm duyệt viên LVE')")
        .bind(MODERATOR)
        .execute(pool)
        .await
        .unwrap();
    for n in 1..=8 {
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
            .bind(reader_id(n))
            .bind(format!("Bạn đọc {n} (dữ liệu kiểm thử)"))
            .execute(pool)
            .await
            .unwrap();
    }
    let note = GrantNote::parse("Kiểm duyệt viên của bộ test LVE").unwrap();
    grant_moderator(pool, PrincipalId::from_uuid(MODERATOR), &note)
        .await
        .unwrap();
    Arc::new(vink)
}

fn version(raw: i64) -> Option<EndorsementVersion> {
    EndorsementVersion::new(raw)
}

fn count(raw: u32) -> Summary {
    Summary::Count(EndorsementCount::restore(raw))
}

fn own(
    view: OwnView,
    raw_version: i64,
    revision: Option<Uuid>,
    capability: Capability,
) -> OwnEndorsement {
    OwnEndorsement {
        view,
        version: version(raw_version),
        revision: revision.map(RevisionId::from_uuid),
        capability,
    }
}

fn endorse(expected_version: i64, observed: Uuid) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: sample_work(),
        expected_version,
        endorsed: true,
        observed_revision: Some(observed.to_string()),
    }
}

fn clear(expected_version: i64) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: sample_work(),
        expected_version,
        endorsed: false,
        observed_revision: None,
    }
}

fn endorse_comment(comment_id: CommentId, expected_version: i64) -> SetCommentEndorsement {
    SetCommentEndorsement {
        work_id: sample_work(),
        comment_id,
        expected_version,
        endorsed: true,
    }
}

fn clear_comment(comment_id: CommentId, expected_version: i64) -> SetCommentEndorsement {
    SetCommentEndorsement {
        endorsed: false,
        ..endorse_comment(comment_id, expected_version)
    }
}

const R1: Uuid = dev_seed::SAMPLE_REVISION;

async fn submit(
    vink: &Vink<PgStore>,
    work_id: WorkId,
    writer: Option<PrincipalId>,
    target: Option<CommentId>,
    body: &str,
) -> CommentId {
    vink.submit_comment(
        writer,
        SubmitComment {
            work_id,
            target,
            body: body.into(),
            command_key: format!("k-{}", Uuid::now_v7().simple()),
        },
    )
    .await
    .unwrap()
    .comment_id
}

async fn decide(vink: &Vink<PgStore>, id: CommentId, expected: i64, action: ModerationAction) {
    let reason = (action != ModerationAction::Approve)
        .then(|| "Không phù hợp quy tắc (dữ liệu kiểm thử).".to_string());
    vink.moderate_comment(
        principal(MODERATOR),
        ModerateComment {
            comment_id: id,
            expected_version: expected,
            action,
            reason,
        },
    )
    .await
    .unwrap();
}

/// An approved comment on the sample work.
async fn public_comment(
    vink: &Vink<PgStore>,
    writer: Option<PrincipalId>,
    target: Option<CommentId>,
    body: &str,
) -> CommentId {
    let id = submit(vink, sample_work(), writer, target, body).await;
    decide(vink, id, 1, ModerationAction::Approve).await;
    id
}

/// A new immutable revision of the sample work (not yet published).
async fn new_revision(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         SELECT $1, work_id, title || ' (bản sửa)', document,
                md5(random()::text) || md5(random()::text), created_by
           FROM revisions WHERE id = $2",
    )
    .bind(id)
    .bind(R1)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// What a republish will do: move the pointer to another revision of the same work.
const POINT: &str = "UPDATE publications SET revision_id = $2, generation = generation + 1,
                            published_at = now() WHERE work_id = $1";
/// What a withdrawal will do: remove the pointer.
const WITHDRAW: &str = "DELETE FROM publications WHERE work_id = $1";

async fn point(pool: &PgPool, revision: Uuid) {
    sqlx::query(POINT)
        .bind(dev_seed::SAMPLE_WORK)
        .bind(revision)
        .execute(pool)
        .await
        .unwrap();
}

async fn work_row(pool: &PgPool, actor: Uuid) -> Option<(Option<Uuid>, i64)> {
    sqlx::query_as(
        "SELECT revision_id, version FROM work_endorsements WHERE work_id = $1 AND actor_id = $2",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(actor)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn comment_row(pool: &PgPool, comment: CommentId, actor: Uuid) -> Option<(bool, i64)> {
    sqlx::query_as(
        "SELECT selected, version FROM comment_endorsements WHERE comment_id = $1 AND actor_id = $2",
    )
    .bind(comment.as_uuid())
    .bind(actor)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn work_summary(vink: &Vink<PgStore>) -> Result<Summary, EndorsementError> {
    vink.endorsement_summaries(sample_work(), None)
        .await
        .map(|s| s.work)
}

fn constraint_or_message(error: sqlx::Error) -> String {
    let db = error.as_database_error().expect("a database error");
    db.constraint()
        .map(str::to_owned)
        .unwrap_or_else(|| db.message().to_owned())
}

async fn raw(pool: &PgPool, sql: &str) -> Result<(), String> {
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_string()))
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(constraint_or_message)
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_database_alone_enforces_endorsement_structure(pool: PgPool) {
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc để ghi nhận").await;
    let foreign = vink.create_draft(author()).await.unwrap().work_id;
    let (work, rev, actor, ghost) = (
        dev_seed::SAMPLE_WORK,
        R1,
        reader_id(1),
        Uuid::from_u128(0xdead),
    );
    let c = root.as_uuid();
    let insert_work = |work: Uuid, actor: Uuid, rev: Option<Uuid>, version: i64| {
        let rev = rev.map_or("NULL".into(), |r| format!("'{r}'"));
        format!(
            "INSERT INTO work_endorsements (work_id, actor_id, revision_id, version)
             VALUES ('{work}', '{actor}', {rev}, {version})"
        )
    };
    let insert_comment = |comment: Uuid, actor: Uuid, version: i64| {
        format!(
            "INSERT INTO comment_endorsements (comment_id, actor_id, selected, version)
             VALUES ('{comment}', '{actor}', true, {version})"
        )
    };
    for (sql, expected) in [
        (
            insert_work(work, ghost, Some(rev), 2),
            "work_endorsements_actor_id_fkey",
        ),
        (
            insert_work(ghost, actor, None, 2),
            "work_endorsements_work_id_fkey",
        ),
        // The anchor must be a revision OF THIS work.
        (
            insert_work(foreign.as_uuid(), actor, Some(rev), 2),
            "work_endorsements_revision_of_work_fkey",
        ),
        (
            insert_work(work, actor, Some(ghost), 2),
            "work_endorsements_revision_of_work_fkey",
        ),
        // A stored row is an accepted intent: version 1 is the absent state, never stored.
        (
            insert_work(work, actor, Some(rev), 1),
            "work_endorsements_version_check",
        ),
        (
            insert_work(work, actor, None, 0),
            "work_endorsements_version_check",
        ),
        (
            insert_comment(ghost, actor, 2),
            "comment_endorsements_comment_id_fkey",
        ),
        (
            insert_comment(c, ghost, 2),
            "comment_endorsements_actor_id_fkey",
        ),
        (
            insert_comment(c, actor, 1),
            "comment_endorsements_version_check",
        ),
    ] {
        assert_eq!(raw(&pool, &sql).await.unwrap_err(), expected, "{sql}");
    }

    raw(&pool, &insert_work(work, actor, Some(rev), 2))
        .await
        .unwrap();
    raw(&pool, &insert_comment(c, actor, 2)).await.unwrap();
    assert_eq!(
        raw(&pool, &insert_work(work, actor, None, 5))
            .await
            .unwrap_err(),
        "work_endorsements_pkey",
        "one row per (actor, work)"
    );
    assert_eq!(
        raw(&pool, &insert_comment(c, actor, 5)).await.unwrap_err(),
        "comment_endorsements_pkey"
    );

    let other_reader = reader_id(2);
    for (sql, message) in [
        // Versions only advance, so a cleared row can never be reset under an old duplicate.
        (
            format!("UPDATE work_endorsements SET version = 2 WHERE actor_id = '{actor}'"),
            "version must advance",
        ),
        (
            format!(
                "UPDATE comment_endorsements SET selected = false, version = 1 WHERE actor_id = '{actor}'"
            ),
            "version must advance",
        ),
        (
            format!(
                "UPDATE work_endorsements SET actor_id = '{other_reader}', version = 3 WHERE actor_id = '{actor}'"
            ),
            "identity is immutable",
        ),
        (
            format!(
                "UPDATE comment_endorsements SET comment_id = comment_id, actor_id = '{other_reader}', version = 3"
            ),
            "identity is immutable",
        ),
        // Clearing keeps the row; nothing deletes or truncates endorsements, even matching none.
        ("DELETE FROM work_endorsements".into(), "rows are retained"),
        (
            "DELETE FROM comment_endorsements WHERE false".into(),
            "rows are retained",
        ),
        ("TRUNCATE work_endorsements".into(), "rows are retained"),
        ("TRUNCATE comment_endorsements".into(), "rows are retained"),
    ] {
        let error = raw(&pool, &sql).await.unwrap_err();
        assert!(error.contains(message), "{sql}: {error}");
    }
    // A clear through SQL keeps the row and advances its version; NULL anchor is allowed.
    raw(
        &pool,
        &format!("UPDATE work_endorsements SET revision_id = NULL, version = 3 WHERE actor_id = '{actor}'"),
    )
    .await
    .unwrap();
    assert_eq!(work_row(&pool, actor).await, Some((None, 3)));
    // Comments are reached through their thread; the table stores no second work column that
    // could disagree with it.
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
          WHERE table_name = 'comment_endorsements' ORDER BY ordinal_position",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        [
            "comment_id",
            "actor_id",
            "selected",
            "version",
            "updated_at"
        ]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_work_endorsement_follows_the_accepted_intent_version(pool: PgPool) {
    let vink = setup(&pool).await;
    let me = reader(1);
    let endorsed = |raw_version| {
        own(
            OwnView::Endorsed,
            raw_version,
            Some(R1),
            Capability::Allowed,
        )
    };
    let cleared = |raw_version| own(OwnView::NotEndorsed, raw_version, None, Capability::Allowed);

    let first = vink.set_work_endorsement(me, endorse(1, R1)).await.unwrap();
    assert_eq!(
        first,
        EndorsementOutcome {
            work_id: sample_work(),
            comment_id: None,
            accepted: true,
            own: endorsed(2),
            summary: count(1),
        }
    );
    // Lost ACK: the same request again is recognized by its effect, not applied twice.
    let retry = vink.set_work_endorsement(me, endorse(1, R1)).await.unwrap();
    assert_eq!(
        (retry.accepted, retry.own, retry.summary),
        (false, endorsed(2), count(1))
    );
    // Reaffirming at the current version is a new accepted intent: version +1, count unchanged.
    let reaffirmed = vink.set_work_endorsement(me, endorse(2, R1)).await.unwrap();
    assert_eq!(
        (reaffirmed.accepted, reaffirmed.own, reaffirmed.summary),
        (true, endorsed(3), count(1))
    );
    // A clear from the older view is stale: it would undo a newer accepted intent.
    assert_eq!(
        vink.set_work_endorsement(me, clear(2)).await,
        Err(EndorsementError::Conflict {
            current: endorsed(3)
        })
    );
    let off = vink.set_work_endorsement(me, clear(3)).await.unwrap();
    assert_eq!(
        (off.accepted, off.own, off.summary),
        (true, cleared(4), count(0))
    );
    assert_eq!(
        work_row(&pool, reader_id(1)).await,
        Some((None, 4)),
        "clearing keeps the row and its version"
    );
    // ABA: the delayed duplicate of the very first endorse cannot apply again.
    assert_eq!(
        vink.set_work_endorsement(me, endorse(1, R1)).await,
        Err(EndorsementError::Conflict {
            current: cleared(4)
        })
    );
    let clear_retry = vink.set_work_endorsement(me, clear(3)).await.unwrap();
    assert_eq!((clear_retry.accepted, clear_retry.own), (false, cleared(4)));
    assert_eq!(work_summary(&vink).await, Ok(count(0)));
    let readback = vink
        .own_endorsements(me, sample_work(), None)
        .await
        .unwrap();
    assert_eq!(readback.work, cleared(4));

    // The author can neither endorse nor clear on their own work, and gets no row.
    for command in [endorse(1, R1), clear(1)] {
        assert_eq!(
            vink.set_work_endorsement(author(), command).await,
            Err(EndorsementError::SelfEndorsement)
        );
    }
    assert_eq!(work_row(&pool, dev_seed::SAMPLE_AUTHOR).await, None);
    let mine = vink
        .own_endorsements(author(), sample_work(), None)
        .await
        .unwrap();
    assert_eq!(
        mine.work,
        own(OwnView::NotEndorsed, 1, None, Capability::OwnContent)
    );
    // A resolved principal without a Vink record is not an actor, before any decision.
    let ghost = Some(PrincipalId::from_uuid(Uuid::now_v7()));
    assert_eq!(
        vink.set_work_endorsement(ghost, endorse(1, R1)).await,
        Err(EndorsementError::Unauthenticated)
    );
    assert_eq!(
        vink.own_endorsements(ghost, sample_work(), None).await,
        Err(EndorsementError::Unauthenticated)
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_endorsements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_initial_clear_fences_an_endorse_created_earlier(pool: PgPool) {
    // LVE adversarial trace: OFF@1. A = endorse@1 was created first but is delayed; B = clear@1
    // arrives first. B is accepted (reaffirmed) and stored as a cleared row at version 2, so A
    // is stale instead of reviving the older intent.
    let vink = setup(&pool).await;
    let b = vink
        .set_work_endorsement(reader(1), clear(1))
        .await
        .unwrap();
    assert!(b.accepted);
    assert_eq!(work_row(&pool, reader_id(1)).await, Some((None, 2)));
    assert_eq!(
        vink.set_work_endorsement(reader(1), endorse(1, R1)).await,
        Err(EndorsementError::Conflict {
            current: own(OwnView::NotEndorsed, 2, None, Capability::Allowed)
        })
    );
    assert_eq!(work_summary(&vink).await, Ok(count(0)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn republication_needs_reconfirmation_and_withdrawal_hides_everything(pool: PgPool) {
    let vink = setup(&pool).await;
    vink.set_work_endorsement(reader(1), endorse(1, R1))
        .await
        .unwrap();
    vink.set_work_endorsement(reader(2), endorse(1, R1))
        .await
        .unwrap();
    assert_eq!(work_summary(&vink).await, Ok(count(2)));

    let r2 = new_revision(&pool).await;
    point(&pool, r2).await;
    // Earlier selections stay stored but are not counted for the new text.
    let summaries = vink
        .endorsement_summaries(sample_work(), None)
        .await
        .unwrap();
    assert_eq!(
        (summaries.work, summaries.revision),
        (count(0), RevisionId::from_uuid(r2))
    );
    let mine = vink
        .own_endorsements(reader(1), sample_work(), None)
        .await
        .unwrap();
    assert_eq!(
        mine.work,
        own(
            OwnView::EndorsedEarlierRevision,
            2,
            Some(R1),
            Capability::Allowed
        )
    );
    // Retrying the old page's request is not a reconfirmation; nothing is written.
    assert_eq!(
        vink.set_work_endorsement(reader(1), endorse(2, R1)).await,
        Err(EndorsementError::RevisionChanged {
            published: RevisionId::from_uuid(r2)
        })
    );
    assert_eq!(work_row(&pool, reader_id(1)).await, Some((Some(R1), 2)));
    let reconfirmed = vink
        .set_work_endorsement(reader(1), endorse(2, r2))
        .await
        .unwrap();
    assert_eq!(
        (reconfirmed.accepted, reconfirmed.own, reconfirmed.summary),
        (
            true,
            own(OwnView::Endorsed, 3, Some(r2), Capability::Allowed),
            count(1)
        )
    );

    // Republishing the earlier revision makes ITS selections effective again (same text).
    point(&pool, R1).await;
    assert_eq!(work_summary(&vink).await, Ok(count(1)), "reader 2 only");
    point(&pool, r2).await;

    // Withdrawal hides the work from every intent and every read; rows stay untouched.
    sqlx::query(WITHDRAW)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    for command in [endorse(3, r2), clear(3), endorse(1, R1)] {
        assert_eq!(
            vink.set_work_endorsement(reader(1), command).await,
            Err(EndorsementError::NotFound)
        );
    }
    assert_eq!(work_summary(&vink).await, Err(EndorsementError::NotFound));
    assert_eq!(
        vink.own_endorsements(reader(1), sample_work(), None).await,
        Err(EndorsementError::NotFound)
    );
    assert_eq!(work_row(&pool, reader_id(1)).await, Some((Some(r2), 3)));
    // A later republish restores the stored state without any write.
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, now())",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(r2)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(work_summary(&vink).await, Ok(count(1)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn comment_endorsements_follow_visibility_scope_and_comments_close(pool: PgPool) {
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc công khai").await;
    let reply = public_comment(&vink, author(), Some(root), "Phản hồi công khai").await;
    let pending = submit(&vink, sample_work(), other(), None, "Đang chờ duyệt").await;
    let rejected = submit(&vink, sample_work(), other(), None, "Bị từ chối").await;
    decide(&vink, rejected, 1, ModerationAction::Reject).await;
    let removed = public_comment(&vink, other(), None, "Sẽ bị gỡ").await;
    vink.remove_own_comment(other(), sample_work(), removed)
        .await
        .unwrap();
    let hidden_root = public_comment(&vink, other(), None, "Gốc sẽ bị ẩn").await;
    let under_hidden = public_comment(
        &vink,
        reader(3),
        Some(hidden_root),
        "Phản hồi còn công khai",
    )
    .await;
    decide(&vink, hidden_root, 2, ModerationAction::Remove).await;

    // Another work's public comment, addressed through the sample work's route.
    let foreign_work = vink.create_draft(author()).await.unwrap().work_id;
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         SELECT $1, $2, title, document, md5('x') || md5('y'), created_by FROM revisions WHERE id = $3",
    )
    .bind(Uuid::from_u128(0xf0))
    .bind(foreign_work.as_uuid())
    .bind(R1)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, now())",
    )
    .bind(foreign_work.as_uuid())
    .bind(Uuid::from_u128(0xf0))
    .execute(&pool)
    .await
    .unwrap();
    let foreign = submit(
        &vink,
        foreign_work,
        other(),
        None,
        "Bình luận tác phẩm khác",
    )
    .await;
    decide(&vink, foreign, 1, ModerationAction::Approve).await;

    let me = reader(1);
    let on = vink
        .set_comment_endorsement(me, endorse_comment(root, 1))
        .await
        .unwrap();
    assert_eq!(
        (on.accepted, on.comment_id, on.own, on.summary),
        (
            true,
            Some(root),
            own(OwnView::Endorsed, 2, None, Capability::Allowed),
            count(1)
        )
    );
    vink.set_comment_endorsement(me, endorse_comment(reply, 1))
        .await
        .unwrap();
    // A reply under a hidden root is still endorsable; the hidden root is not.
    vink.set_comment_endorsement(me, endorse_comment(under_hidden, 1))
        .await
        .unwrap();
    for target in [
        pending,
        rejected,
        removed,
        hidden_root,
        foreign,
        CommentId::generate(),
    ] {
        for command in [endorse_comment(target, 1), clear_comment(target, 1)] {
            assert_eq!(
                vink.set_comment_endorsement(me, command).await,
                Err(EndorsementError::NotFound),
                "{target}"
            );
        }
    }
    // The writer's own pending comment is not public, so it is unavailable before ownership.
    assert_eq!(
        vink.set_comment_endorsement(other(), endorse_comment(pending, 1))
            .await,
        Err(EndorsementError::NotFound)
    );
    assert_eq!(
        vink.set_comment_endorsement(other(), endorse_comment(root, 1))
            .await,
        Err(EndorsementError::SelfEndorsement)
    );
    // Nothing cascaded to the root, the work or the other tier.
    let raw_ids = [
        root,
        reply,
        under_hidden,
        hidden_root,
        pending,
        foreign,
        root,
    ]
    .map(|id| id.to_string())
    .join(",");
    let summaries = vink
        .endorsement_summaries(sample_work(), Some(&raw_ids))
        .await
        .unwrap();
    assert_eq!(summaries.work, count(0));
    assert_eq!(
        summaries
            .comments
            .iter()
            .map(|c| (c.comment_id, c.summary))
            .collect::<Vec<_>>(),
        [
            (root, count(1)),
            (reply, count(1)),
            (under_hidden, count(1)),
            (hidden_root, Summary::Unavailable),
            (pending, Summary::Unavailable),
            (foreign, Summary::Unavailable),
        ],
        "deduplicated, in request order; hidden is never zero"
    );

    // Closing comments stops NEW comment endorsements only.
    vink.set_comments_mode(
        author(),
        SetCommentsMode {
            work_id: sample_work(),
            expected_generation: 1,
            mode: CommentsMode::Closed,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        vink.set_comment_endorsement(reader(2), endorse_comment(root, 1))
            .await,
        Err(EndorsementError::CommentsClosed)
    );
    let reaffirmed = vink
        .set_comment_endorsement(me, endorse_comment(reply, 2))
        .await
        .unwrap();
    assert_eq!(
        (reaffirmed.accepted, reaffirmed.own.version),
        (true, version(3))
    );
    let cleared = vink
        .set_comment_endorsement(me, clear_comment(root, 2))
        .await
        .unwrap();
    assert_eq!(
        (cleared.accepted, cleared.own, cleared.summary),
        (
            true,
            own(OwnView::NotEndorsed, 3, None, Capability::CommentsClosed),
            count(0)
        )
    );
    assert_eq!(
        vink.set_comment_endorsement(me, endorse_comment(root, 3))
            .await,
        Err(EndorsementError::CommentsClosed)
    );
    // The work's own endorsement is not governed by the comments switch.
    vink.set_work_endorsement(me, endorse(1, R1)).await.unwrap();

    // Own state: actor-scoped, versions only where available, advisory capabilities.
    let ids = [root, reply, pending, hidden_root]
        .map(|id| id.to_string())
        .join(",");
    let mine = vink
        .own_endorsements(me, sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(
        mine.comments.iter().map(|c| c.own).collect::<Vec<_>>(),
        [
            own(OwnView::NotEndorsed, 3, None, Capability::CommentsClosed),
            own(OwnView::Endorsed, 3, None, Capability::Allowed),
            OwnEndorsement::UNAVAILABLE,
            OwnEndorsement::UNAVAILABLE,
        ]
    );
    let writer = vink
        .own_endorsements(other(), sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(
        writer.comments.iter().map(|c| c.own).collect::<Vec<_>>(),
        [
            own(OwnView::NotEndorsed, 1, None, Capability::OwnContent),
            own(OwnView::NotEndorsed, 1, None, Capability::CommentsClosed),
            // Their own pending comment is readable in the discussion, never endorsable.
            OwnEndorsement::UNAVAILABLE,
            OwnEndorsement::UNAVAILABLE,
        ],
        "another actor's selections never leak into this actor's state"
    );
    assert_eq!(
        vink.own_endorsements(None, sample_work(), Some(&ids)).await,
        Err(EndorsementError::Unauthenticated)
    );

    // Reopening restores endorsing without any data change.
    vink.set_comments_mode(
        author(),
        SetCommentsMode {
            work_id: sample_work(),
            expected_generation: 2,
            mode: CommentsMode::Open,
        },
    )
    .await
    .unwrap();
    let again = vink
        .set_comment_endorsement(me, endorse_comment(root, 3))
        .await
        .unwrap();
    assert_eq!((again.accepted, again.summary), (true, count(1)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_first_intents_have_exactly_one_acceptance(pool: PgPool) {
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc bị bấm nhiều lần").await;

    // Eight identical first endorsements of one actor: one accepted, seven already current.
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let vink = Arc::clone(&vink);
            tokio::spawn(async move { vink.set_work_endorsement(reader(1), endorse(1, R1)).await })
        })
        .collect();
    let mut accepted = 0;
    for task in tasks {
        let outcome = task.await.unwrap().unwrap();
        accepted += usize::from(outcome.accepted);
        assert_eq!(outcome.own.version, version(2));
    }
    assert_eq!(accepted, 1);
    assert_eq!(work_row(&pool, reader_id(1)).await, Some((Some(R1), 2)));

    // Mixed first intents at version 1 on a comment: exactly one is accepted; the rest are
    // already current (same desire) or conflicts (opposite desire); the count matches the row.
    let tasks: Vec<_> = (0..8)
        .map(|n| {
            let vink = Arc::clone(&vink);
            tokio::spawn(async move {
                let command = if n % 2 == 0 {
                    endorse_comment(root, 1)
                } else {
                    clear_comment(root, 1)
                };
                vink.set_comment_endorsement(reader(2), command).await
            })
        })
        .collect();
    let mut accepted = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(outcome) => accepted += usize::from(outcome.accepted),
            Err(EndorsementError::Conflict { current }) => {
                assert_eq!(current.version, version(2));
            }
            Err(other) => panic!("{other:?}"),
        }
    }
    assert_eq!(accepted, 1);
    let (selected, row_version) = comment_row(&pool, root, reader_id(2)).await.unwrap();
    assert_eq!(row_version, 2);
    let ids = root.to_string();
    let summary = vink
        .endorsement_summaries(sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(summary.comments[0].summary, count(u32::from(selected)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_uncommitted_first_intent_decides_a_waiting_insert_in_both_commit_orders(pool: PgPool) {
    let vink = setup(&pool).await;
    // Order 1: a concurrent first intent (a clear, accepted at version 1) is uncommitted when this
    // endorse decides from the absent row. Its INSERT waits; after the commit it reads that row
    // under lock and decides again: stale, nothing written.
    let mut first = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO work_endorsements (work_id, actor_id, revision_id, version)
         VALUES ($1, $2, NULL, 2)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(reader_id(1))
    .execute(&mut *first)
    .await
    .unwrap();
    let racing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_work_endorsement(reader(1), endorse(1, R1)).await })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !racing.is_finished(),
        "the INSERT waits for the uncommitted row"
    );
    first.commit().await.unwrap();
    assert_eq!(
        racing.await.unwrap(),
        Err(EndorsementError::Conflict {
            current: own(OwnView::NotEndorsed, 2, None, Capability::Allowed)
        })
    );
    assert_eq!(work_row(&pool, reader_id(1)).await, Some((None, 2)));

    // Order 2: the concurrent first intent rolls back; the waiting INSERT then wins.
    let mut first = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO work_endorsements (work_id, actor_id, revision_id, version)
         VALUES ($1, $2, $3, 2)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(reader_id(2))
    .bind(R1)
    .execute(&mut *first)
    .await
    .unwrap();
    let racing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_work_endorsement(reader(2), clear(1)).await })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(!racing.is_finished());
    first.rollback().await.unwrap();
    let won = racing.await.unwrap().unwrap();
    assert_eq!(
        (won.accepted, won.own),
        (
            true,
            own(OwnView::NotEndorsed, 2, None, Capability::Allowed)
        )
    );
    assert_eq!(work_row(&pool, reader_id(2)).await, Some((None, 2)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn endorsing_and_closing_comments_serialize_on_the_thread(pool: PgPool) {
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc trong lúc tắt bình luận").await;

    // An uncommitted close holds the thread row; a new endorsement waits, then sees Closed.
    let mut closer = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE discussion_threads SET comments_open = false,
                settings_generation = settings_generation + 1 WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .execute(&mut *closer)
    .await
    .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_comment_endorsement(reader(1), endorse_comment(root, 1))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !endorsing.is_finished(),
        "endorsement waits for the thread lock"
    );
    closer.commit().await.unwrap();
    assert_eq!(
        endorsing.await.unwrap(),
        Err(EndorsementError::CommentsClosed)
    );
    assert_eq!(comment_row(&pool, root, reader_id(1)).await, None);
    vink.set_comments_mode(
        author(),
        SetCommentsMode {
            work_id: sample_work(),
            expected_generation: 2,
            mode: CommentsMode::Open,
        },
    )
    .await
    .unwrap();

    // The other order: a REAL in-flight endorsement holds its thread share lock until commit.
    // Pause it on its own row (it already locked publication, thread and comment): a close must
    // wait, so no endorsement can commit after a committed close.
    vink.set_comment_endorsement(reader(2), clear_comment(root, 1))
        .await
        .unwrap();
    let mut pauser = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT 1 FROM comment_endorsements WHERE comment_id = $1 AND actor_id = $2 FOR UPDATE",
    )
    .bind(root.as_uuid())
    .bind(reader_id(2))
    .execute(&mut *pauser)
    .await
    .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_comment_endorsement(reader(2), endorse_comment(root, 2))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !endorsing.is_finished(),
        "endorsement is paused on its own row"
    );
    let closing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_comments_mode(
                author(),
                SetCommentsMode {
                    work_id: sample_work(),
                    expected_generation: 3,
                    mode: CommentsMode::Closed,
                },
            )
            .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !closing.is_finished(),
        "a close waits for the in-flight endorsement"
    );
    pauser.rollback().await.unwrap();
    let endorsed = endorsing.await.unwrap().unwrap();
    assert!(endorsed.accepted);
    let closed = closing.await.unwrap().unwrap();
    assert_eq!(closed.mode(), CommentsMode::Closed);
    let ordered: bool = sqlx::query_scalar(
        "SELECT e.updated_at <= t.updated_at FROM comment_endorsements e, discussion_threads t
          WHERE e.comment_id = $1 AND e.actor_id = $2 AND t.work_id = $3",
    )
    .bind(root.as_uuid())
    .bind(reader_id(2))
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(ordered, "the endorsement committed before the close");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn endorsing_and_moderation_serialize_on_the_comment(pool: PgPool) {
    let vink = setup(&pool).await;
    let first = public_comment(&vink, other(), None, "Gốc sẽ bị gỡ khi đang ghi nhận").await;
    let second = public_comment(&vink, other(), None, "Gốc thứ hai").await;

    // An uncommitted moderation removal holds the comment row; the endorsement waits and then
    // finds the comment unavailable.
    let mut moderating = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE comments SET removal = 'moderation', moderation_version = moderation_version + 1
          WHERE id = $1",
    )
    .bind(first.as_uuid())
    .execute(&mut *moderating)
    .await
    .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_comment_endorsement(reader(1), endorse_comment(first, 1))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !endorsing.is_finished(),
        "endorsement waits for the comment lock"
    );
    moderating.commit().await.unwrap();
    assert_eq!(endorsing.await.unwrap(), Err(EndorsementError::NotFound));
    assert_eq!(comment_row(&pool, first, reader_id(1)).await, None);

    // The other order: a paused in-flight endorsement holds the comment FOR SHARE, so a real
    // moderation decision waits for it; afterwards the stored endorsement is hidden, not lost.
    vink.set_comment_endorsement(reader(1), clear_comment(second, 1))
        .await
        .unwrap();
    let mut pauser = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT 1 FROM comment_endorsements WHERE comment_id = $1 AND actor_id = $2 FOR UPDATE",
    )
    .bind(second.as_uuid())
    .bind(reader_id(1))
    .execute(&mut *pauser)
    .await
    .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_comment_endorsement(reader(1), endorse_comment(second, 2))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    let removing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { decide(&vink, second, 2, ModerationAction::Remove).await })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !removing.is_finished(),
        "moderation waits for the in-flight endorsement"
    );
    pauser.rollback().await.unwrap();
    assert!(endorsing.await.unwrap().unwrap().accepted);
    removing.await.unwrap();
    assert_eq!(
        comment_row(&pool, second, reader_id(1)).await,
        Some((true, 3))
    );
    let ids = second.to_string();
    let summaries = vink
        .endorsement_summaries(sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(summaries.comments[0].summary, Summary::Unavailable);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn endorsing_and_publication_changes_serialize_on_the_pointer(pool: PgPool) {
    let vink = setup(&pool).await;
    let r2 = new_revision(&pool).await;

    // An uncommitted republish holds the pointer; an endorsement of the old page waits and then
    // learns that the text changed, writing nothing.
    let mut republishing = pool.begin().await.unwrap();
    sqlx::query(POINT)
        .bind(dev_seed::SAMPLE_WORK)
        .bind(r2)
        .execute(&mut *republishing)
        .await
        .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_work_endorsement(reader(1), endorse(1, R1)).await })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !endorsing.is_finished(),
        "endorsement waits for the publication pointer"
    );
    republishing.commit().await.unwrap();
    assert_eq!(
        endorsing.await.unwrap(),
        Err(EndorsementError::RevisionChanged {
            published: RevisionId::from_uuid(r2)
        })
    );
    assert_eq!(work_row(&pool, reader_id(1)).await, None);

    // An uncommitted withdrawal: the endorsement waits and then finds nothing.
    let mut withdrawing = pool.begin().await.unwrap();
    sqlx::query(WITHDRAW)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *withdrawing)
        .await
        .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_work_endorsement(reader(1), endorse(1, r2)).await })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(!endorsing.is_finished());
    withdrawing.rollback().await.unwrap();
    // Rolled back: the work stayed public and the endorsement applies.
    assert!(endorsing.await.unwrap().unwrap().accepted);

    // The other order: a paused in-flight endorsement holds the pointer FOR SHARE, so a
    // withdrawal waits for its commit and then hides it.
    let mut pauser = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM work_endorsements WHERE work_id = $1 AND actor_id = $2 FOR UPDATE")
        .bind(dev_seed::SAMPLE_WORK)
        .bind(reader_id(1))
        .execute(&mut *pauser)
        .await
        .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_work_endorsement(reader(1), clear(2)).await })
    };
    tokio::time::sleep(PAUSE).await;
    let withdrawing = {
        let pool = pool.clone();
        tokio::spawn(async move {
            sqlx::query(WITHDRAW)
                .bind(dev_seed::SAMPLE_WORK)
                .execute(&pool)
                .await
                .unwrap()
                .rows_affected()
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(
        !withdrawing.is_finished(),
        "withdrawal waits for the in-flight endorsement"
    );
    pauser.rollback().await.unwrap();
    assert!(endorsing.await.unwrap().unwrap().accepted);
    assert_eq!(withdrawing.await.unwrap(), 1);
    assert_eq!(work_summary(&vink).await, Err(EndorsementError::NotFound));
    assert_eq!(work_row(&pool, reader_id(1)).await, Some((None, 3)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_refused_or_failed_write_leaves_nothing_behind(pool: PgPool) {
    let vink = setup(&pool).await;
    vink.set_work_endorsement(reader(1), endorse(1, R1))
        .await
        .unwrap();
    // The last version: the core refuses to wrap and nothing is written or counted twice.
    sqlx::query("UPDATE work_endorsements SET version = $1 WHERE actor_id = $2")
        .bind(i64::MAX)
        .bind(reader_id(1))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        vink.set_work_endorsement(reader(1), clear(i64::MAX)).await,
        Err(EndorsementError::Internal)
    );
    assert_eq!(
        work_row(&pool, reader_id(1)).await,
        Some((Some(R1), i64::MAX))
    );
    assert_eq!(work_summary(&vink).await, Ok(count(1)));
    // A stale or refused request writes nothing either.
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work_endorsements WHERE updated_at IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    let _ = vink.set_work_endorsement(author(), endorse(1, R1)).await;
    let _ = vink
        .set_work_endorsement(reader(2), endorse(1, Uuid::now_v7()))
        .await;
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM work_endorsements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn summaries_equal_an_independent_recount_of_canonical_rows(pool: PgPool) {
    let vink = setup(&pool).await;
    let roots = [
        public_comment(&vink, other(), None, "Gốc một").await,
        public_comment(&vink, other(), None, "Gốc hai").await,
    ];
    let r2 = new_revision(&pool).await;
    // Readers 1–4 endorse revision 1; then revision 2 is published; readers 3–6 endorse it
    // (3 and 4 reconfirm), reader 5 clears again, reader 2 reaffirms its old selection request
    // in vain. Comments: readers 1–6 endorse root one, readers 1, 3, 5 also clear it.
    for n in 1..=4 {
        vink.set_work_endorsement(reader(n), endorse(1, R1))
            .await
            .unwrap();
    }
    point(&pool, r2).await;
    for n in 3..=4 {
        vink.set_work_endorsement(reader(n), endorse(2, r2))
            .await
            .unwrap();
    }
    for n in 5..=6 {
        vink.set_work_endorsement(reader(n), endorse(1, r2))
            .await
            .unwrap();
    }
    vink.set_work_endorsement(reader(5), clear(2))
        .await
        .unwrap();
    assert!(
        vink.set_work_endorsement(reader(2), endorse(2, R1))
            .await
            .is_err()
    );
    for n in 1..=6 {
        vink.set_comment_endorsement(reader(n), endorse_comment(roots[0], 1))
            .await
            .unwrap();
    }
    for n in [1, 3, 5] {
        vink.set_comment_endorsement(reader(n), clear_comment(roots[0], 2))
            .await
            .unwrap();
    }
    // A reaffirmation of a selection never adds a second vote.
    vink.set_comment_endorsement(reader(2), endorse_comment(roots[0], 2))
        .await
        .unwrap();

    // Independent recount: every effective row, grouped, joined to the CURRENT pointer.
    let (work_effective, all_selected): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE e.revision_id = p.revision_id),
                count(*) FILTER (WHERE e.revision_id IS NOT NULL)
           FROM work_endorsements e JOIN publications p USING (work_id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (work_effective, all_selected),
        (3, 5),
        "readers 3, 4, 6 at r2; 1, 2 at r1"
    );
    let per_comment: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT c.id, count(e.actor_id) FILTER (WHERE e.selected)
           FROM comments c LEFT JOIN comment_endorsements e ON e.comment_id = c.id
          GROUP BY c.id ORDER BY c.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let ids = roots.map(|id| id.to_string()).join(",");
    let summaries = vink
        .endorsement_summaries(sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(summaries.work, count(work_effective as u32));
    for entry in &summaries.comments {
        let (_, expected) = per_comment
            .iter()
            .find(|(id, _)| *id == entry.comment_id.as_uuid())
            .unwrap();
        assert_eq!(entry.summary, count(*expected as u32));
    }
    assert_eq!(summaries.comments[0].summary, count(3));
    assert_eq!(summaries.comments[1].summary, count(0));
}

/// Test-only resolver whose identity dependency can never decide.
struct UnavailableIdentity;

impl IdentityResolver for UnavailableIdentity {
    fn resolve<'a>(&'a self, _parts: &'a Parts, _peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(std::future::ready(Resolution::Unavailable))
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_http_api_keeps_every_endorsement_outcome_distinct(pool: PgPool) {
    let app = app(&pool).await;
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc qua HTTP").await;
    let work = format!("/api/v1/works/{}", dev_seed::SAMPLE_WORK);
    let mine = format!("{work}/endorsement/me");
    let on_root = format!("{work}/discussion/comments/{root}/endorsement/me");
    let (me, origin) = (Some(reader_id(1)), Some(ORIGIN));
    let code = |body: &str| json(body)["error"]["code"].as_str().unwrap().to_string();
    let put = |expected: i64, endorsed: bool, revision: Option<Uuid>| {
        Some(json!({
            "expected_version": expected,
            "endorsed": endorsed,
            "observed_revision_id": revision.map(|r| r.to_string()),
        }))
    };

    // The READ-01 publication representation is captured before any endorsement exists.
    let publication = format!("{work}/publication");
    let etag = |app: Router| {
        let path = publication.clone();
        async move {
            let response = app
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            response.headers()[header::ETAG]
                .to_str()
                .unwrap()
                .to_owned()
        }
    };
    let before = etag(app.clone()).await;

    // Public summary: no principal, no viewer state, never stored.
    let (status, cache, body) = call(
        &app,
        "GET",
        &format!("{work}/endorsements"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(
        json(&body),
        json!({
            "policy": "lve-core-v1",
            "work_id": dev_seed::SAMPLE_WORK.to_string(),
            "work": {"status": "available", "count": 0, "revision_id": R1.to_string()},
            "comments": []
        })
    );
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("{work}/endorsements/me"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::UNAUTHORIZED, "unauthenticated")
    );

    // Writes: authentication, then Origin, then the body.
    let (status, _, _) = call(&app, "PUT", &mine, None, origin, put(1, true, Some(R1))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, body) = call(&app, "PUT", &mine, me, None, put(1, true, Some(R1))).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "forbidden")
    );
    for (body, field) in [
        (
            Some(json!({"expected_version": 1, "endorsed": true,
                        "observed_revision_id": R1, "actor_id": dev_seed::SAMPLE_AUTHOR})),
            "body",
        ),
        (
            Some(
                json!({"expected_version": 1, "endorsed": true, "observed_revision_id": R1, "count": 99}),
            ),
            "body",
        ),
        (put(0, true, Some(R1)), "expected_version"),
        (put(1, true, None), "observed_revision_id"),
        (put(1, false, Some(R1)), "observed_revision_id"),
    ] {
        let (status, cache, response) = call(&app, "PUT", &mine, me, origin, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
        assert_eq!(json(&response)["error"]["field"], field, "{response}");
        assert_eq!(cache.as_deref(), Some("no-store"));
    }
    let (status, cache, body) = call(&app, "PUT", &mine, me, origin, put(1, true, Some(R1))).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    let written = json(&body);
    assert_eq!(written["accepted"], true);
    assert_eq!(
        written["endorsement"],
        json!({"state": "endorsed", "version": 2, "revision_id": R1.to_string(),
               "can_endorse": true, "blocked_reason": null})
    );
    assert_eq!(
        written["summary"],
        json!({"status": "available", "count": 1})
    );
    // Lost ACK retry: 200 with accepted false and the same state.
    let (status, _, body) = call(&app, "PUT", &mine, me, origin, put(1, true, Some(R1))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["accepted"], false);
    assert_eq!(json(&body)["endorsement"]["version"], 2);
    // Stale opposite intent: 409 with the stored state to reconcile from.
    let (status, _, body) = call(&app, "PUT", &mine, me, origin, put(1, false, None)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let error = &json(&body)["error"];
    assert_eq!(
        (error["code"].as_str(), error["current_version"].as_i64()),
        (Some("version_conflict"), Some(2))
    );
    assert_eq!(error["endorsement"]["state"], "endorsed");
    // Self-endorsement, unknown or malformed work.
    let (status, _, body) = call(
        &app,
        "PUT",
        &mine,
        Some(dev_seed::SAMPLE_AUTHOR),
        origin,
        put(1, true, Some(R1)),
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "self_endorsement")
    );
    for path in [
        format!("/api/v1/works/{}/endorsement/me", Uuid::now_v7()),
        "/api/v1/works/không-phải-id/endorsement/me".to_string(),
    ] {
        let (status, _, body) = call(&app, "PUT", &path, me, origin, put(1, true, Some(R1))).await;
        assert_eq!(
            (status, code(&body).as_str()),
            (StatusCode::NOT_FOUND, "not_found")
        );
    }
    // READ-01 is untouched: endorsement writes never change the public representation.
    assert_eq!(etag(app.clone()).await, before);
    let (_, _, body) = call(&app, "GET", &publication, None, None, None).await;
    assert!(!body.contains("endorse"), "{body}");

    // A republished work: the old page's endorsement is 409 revision_changed.
    let r2 = new_revision(&pool).await;
    point(&pool, r2).await;
    let (status, _, body) = call(
        &app,
        "PUT",
        &mine,
        Some(reader_id(2)),
        origin,
        put(1, true, Some(R1)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "revision_changed");
    assert_eq!(json(&body)["error"]["current_revision_id"], r2.to_string());

    // Comments: endorse, closed, cross-work and batch reads.
    let (status, _, body) = call(
        &app,
        "PUT",
        &on_root,
        me,
        origin,
        Some(json!({"expected_version": 1, "endorsed": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body)["comment_id"], root.to_string());
    assert_eq!(json(&body)["endorsement"]["revision_id"], Value::Null);
    vink.set_comments_mode(
        author(),
        SetCommentsMode {
            work_id: sample_work(),
            expected_generation: 1,
            mode: CommentsMode::Closed,
        },
    )
    .await
    .unwrap();
    let (status, _, body) = call(
        &app,
        "PUT",
        &on_root,
        Some(reader_id(2)),
        origin,
        Some(json!({"expected_version": 1, "endorsed": true})),
    )
    .await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::CONFLICT, "comments_closed")
    );
    let foreign_route = format!(
        "/api/v1/works/{}/discussion/comments/{root}/endorsement/me",
        Uuid::now_v7()
    );
    let (status, _, _) = call(
        &app,
        "PUT",
        &foreign_route,
        me,
        origin,
        Some(json!({"expected_version": 1, "endorsed": false})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let missing = Uuid::now_v7();
    let batch = format!("{work}/endorsements?comment_ids={root},{missing},{root}");
    let (status, _, body) = call(&app, "GET", &batch, None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json(&body)["comments"],
        json!([
            {"comment_id": root.to_string(), "status": "available", "count": 1},
            {"comment_id": missing.to_string(), "status": "unavailable", "count": null}
        ])
    );
    assert_eq!(json(&body)["work"]["revision_id"], r2.to_string());
    let own_batch = format!("{work}/endorsements/me?comment_ids={root}");
    let (status, cache, body) = call(&app, "GET", &own_batch, me, None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(json(&body)["work"]["state"], "endorsed_earlier_revision");
    assert_eq!(json(&body)["comments"][0]["state"], "endorsed");
    let too_many = (0..=100)
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>()
        .join(",");
    for query in [format!("comment_ids={too_many}"), "viewer=me".to_string()] {
        let (status, _, body) = call(
            &app,
            "GET",
            &format!("{work}/endorsements?{query}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }

    // An identity outage fails own state and writes as 503, never public summaries.
    let unavailable = router(AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        Arc::new(UnavailableIdentity),
        ORIGIN,
        std::path::Path::new("target/site"),
    ));
    let (status, _, _) = call(
        &unavailable,
        "GET",
        &format!("{work}/endorsements"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, body) = call(&unavailable, "GET", &own_batch, None, None, None).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, "dependency_unavailable")
    );
    let (status, _, _) = call(
        &unavailable,
        "PUT",
        &mine,
        None,
        origin,
        put(2, false, None),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
