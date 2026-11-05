//! Literary endorsement (LVE, #73) on real PostgreSQL. Use cases run through `PgStore`, so every
//! write is the production transaction that locks the facts, calls `decide_endorsement` and
//! commits. Integrity negatives bypass Rust with direct SQL. Races use a second connection that
//! holds a real lock (or an uncommitted row) and then commits or rolls back, so both commit
//! orders are exercised on purpose rather than by scheduler accident. Publication pointer changes
//! are SQL fixtures: no publish/withdraw command exists yet, so these prove the durable boundary,
//! not a product workflow.

use std::time::Duration;

use vink_app::application::discussion::{
    ModerateComment, ModerationAction, SetCommentsMode, SubmitComment,
};
use vink_app::application::endorsement::{
    Capability, EndorsementError, EndorsementOutcomeView, EndorsementResult, OwnEndorsement,
    SetCommentEndorsement, SetWorkEndorsement,
};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::discussion::settings::CommentsMode;
use vink_app::domain::endorsement::{EndorsementCount, EndorsementState, EndorsementTarget};
use vink_app::domain::ids::{CommentId, RevisionId};
use vink_app::identity::{IdentityResolver, Resolution, ResolveFuture};
use vink_app::persistence::grant_moderator;

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x6666_6666_6666_4666_8666_6666_6666_6666);
const PAUSE: Duration = Duration::from_millis(300);
const R1: Uuid = dev_seed::SAMPLE_REVISION;

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

fn state(selected: bool, version: i64, order: i64) -> EndorsementState {
    EndorsementState::restore(selected, version, order).unwrap()
}

fn count(raw: i64) -> Option<EndorsementCount> {
    EndorsementCount::new(raw)
}

fn key() -> String {
    Uuid::now_v7().to_string()
}

fn on_work(key: &str, expected: i64, endorsed: bool, revision: Uuid) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: sample_work(),
        revision: revision.to_string(),
        command_key: key.into(),
        expected_order: expected,
        endorsed,
    }
}

fn on_comment(
    key: &str,
    comment: CommentId,
    expected: i64,
    endorsed: bool,
) -> SetCommentEndorsement {
    SetCommentEndorsement {
        work_id: sample_work(),
        comment_id: comment,
        command_key: key.into(),
        expected_order: expected,
        endorsed,
    }
}

fn accepted(
    outcome: Result<EndorsementOutcomeView, EndorsementError>,
) -> (EndorsementState, Option<EndorsementCount>) {
    match outcome {
        Ok(EndorsementOutcomeView {
            result: EndorsementResult::Accepted(state),
            summary,
            ..
        }) => (state, summary),
        other => panic!("expected an accepted command, got {other:?}"),
    }
}

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

async fn work_row(pool: &PgPool, revision: Uuid, actor: Uuid) -> Option<(bool, i64, i64)> {
    sqlx::query_as(
        "SELECT selected, version, accepted_order FROM work_endorsements
          WHERE work_id = $1 AND revision_id = $2 AND actor_id = $3",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(revision)
    .bind(actor)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn comment_row(pool: &PgPool, comment: CommentId, actor: Uuid) -> Option<(bool, i64, i64)> {
    sqlx::query_as(
        "SELECT selected, version, accepted_order FROM comment_endorsements
          WHERE comment_id = $1 AND actor_id = $2",
    )
    .bind(comment.as_uuid())
    .bind(actor)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn rows(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn work_summary(vink: &Vink<PgStore>) -> Result<EndorsementCount, EndorsementError> {
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
    let foreign = vink.create_draft(author()).await.unwrap().work_id.as_uuid();
    let (work, rev, actor, ghost, c) = (
        dev_seed::SAMPLE_WORK,
        R1,
        reader_id(1),
        Uuid::from_u128(0xdead),
        root.as_uuid(),
    );
    let w = |work: Uuid, rev: Uuid, actor: Uuid, version: i64, order: i64| {
        format!(
            "INSERT INTO work_endorsements (work_id, revision_id, actor_id, selected, version, accepted_order)
             VALUES ('{work}', '{rev}', '{actor}', true, {version}, {order})"
        )
    };
    let cm = |comment: Uuid, actor: Uuid, version: i64, order: i64| {
        format!(
            "INSERT INTO comment_endorsements (comment_id, actor_id, selected, version, accepted_order)
             VALUES ('{comment}', '{actor}', true, {version}, {order})"
        )
    };
    let receipt = |targets: &str, expected: i64, version: i64, order: i64| {
        format!(
            "INSERT INTO endorsement_receipts (actor_id, command_key, work_id, revision_id, comment_id,
                 expected_order, selected, committed_version, committed_order)
             VALUES ('{actor}', gen_random_uuid(), {targets}, {expected}, true, {version}, {order})"
        )
    };
    for (sql, expected) in [
        (w(work, rev, ghost, 2, 2), "work_endorsements_actor_id_fkey"),
        (w(ghost, rev, actor, 2, 2), "work_endorsements_work_id_fkey"),
        // The anchor must be a revision OF THIS work.
        (
            w(foreign, rev, actor, 2, 2),
            "work_endorsements_revision_of_work_fkey",
        ),
        (
            w(work, ghost, actor, 2, 2),
            "work_endorsements_revision_of_work_fkey",
        ),
        // A stored row is an accepted command: order 1 is the absent state; version <= order.
        (w(work, rev, actor, 1, 1), "work_endorsements_tokens_check"),
        (w(work, rev, actor, 3, 2), "work_endorsements_tokens_check"),
        (w(work, rev, actor, 0, 2), "work_endorsements_tokens_check"),
        (
            cm(ghost, actor, 2, 2),
            "comment_endorsements_comment_id_fkey",
        ),
        (cm(c, ghost, 2, 2), "comment_endorsements_actor_id_fkey"),
        (cm(c, actor, 1, 1), "comment_endorsements_tokens_check"),
        // Receipts: exactly one typed target, the revision of its work, the next order.
        (
            receipt(&format!("'{work}', '{rev}', '{c}'"), 1, 2, 2),
            "endorsement_receipts_one_target",
        ),
        (
            receipt(&format!("'{work}', NULL, NULL"), 1, 2, 2),
            "endorsement_receipts_one_target",
        ),
        (
            receipt(&format!("'{foreign}', '{rev}', NULL"), 1, 2, 2),
            "endorsement_receipts_revision_of_work_fkey",
        ),
        (
            receipt(&format!("NULL, NULL, '{ghost}'"), 1, 2, 2),
            "endorsement_receipts_comment_id_fkey",
        ),
        (
            receipt(&format!("'{work}', '{rev}', NULL"), 1, 2, 3),
            "endorsement_receipts_committed_check",
        ),
        // Selection follows the version's parity (OFF/v1 start, every step flips): selected at
        // an odd version, or OFF at an even one, is a state no accepted command reaches.
        (w(work, rev, actor, 1, 2), "work_endorsements_version_parity_check"),
        (w(work, rev, actor, 3, 4), "work_endorsements_version_parity_check"),
        (cm(c, actor, 1, 2), "comment_endorsements_version_parity_check"),
        (
            format!(
                "INSERT INTO comment_endorsements (comment_id, actor_id, selected, version, accepted_order)
                 VALUES ('{c}', '{actor}', false, 2, 3)"
            ),
            "comment_endorsements_version_parity_check",
        ),
        (
            receipt(&format!("NULL, NULL, '{c}'"), 1, 1, 2),
            "endorsement_receipts_version_parity_check",
        ),
    ] {
        assert_eq!(raw(&pool, &sql).await.unwrap_err(), expected, "{sql}");
    }

    raw(&pool, &w(work, rev, actor, 2, 2)).await.unwrap();
    raw(&pool, &cm(c, actor, 2, 2)).await.unwrap();
    raw(&pool, &receipt(&format!("NULL, NULL, '{c}'"), 1, 2, 2))
        .await
        .unwrap();
    assert_eq!(
        raw(&pool, &w(work, rev, actor, 2, 5)).await.unwrap_err(),
        "work_endorsements_pkey",
        "one row per (actor, revision)"
    );
    assert_eq!(
        raw(&pool, &cm(c, actor, 2, 5)).await.unwrap_err(),
        "comment_endorsements_pkey"
    );

    let other_reader = reader_id(2);
    for (sql, message) in [
        // The order only advances, so a removed row can never be reset under an old command.
        (
            format!("UPDATE work_endorsements SET accepted_order = 2 WHERE actor_id = '{actor}'"),
            "accepted order must advance",
        ),
        (
            format!(
                "UPDATE comment_endorsements SET accepted_order = 3, version = 1 WHERE actor_id = '{actor}'"
            ),
            "version cannot decrease",
        ),
        // A flip without a version step would change the public count with no mutation.
        (
            format!(
                "UPDATE work_endorsements SET selected = false, accepted_order = 3 WHERE actor_id = '{actor}'"
            ),
            "work_endorsements_version_parity_check",
        ),
        (
            format!(
                "UPDATE work_endorsements SET actor_id = '{other_reader}', accepted_order = 3 WHERE actor_id = '{actor}'"
            ),
            "identity is immutable",
        ),
        (
            format!(
                "UPDATE work_endorsements SET revision_id = revision_id, work_id = '{foreign}', accepted_order = 3"
            ),
            "identity is immutable",
        ),
        // Records and receipts are retained, even by a statement matching no row.
        ("DELETE FROM work_endorsements".into(), "rows are retained"),
        (
            "DELETE FROM comment_endorsements WHERE false".into(),
            "rows are retained",
        ),
        ("TRUNCATE work_endorsements".into(), "rows are retained"),
        ("TRUNCATE comment_endorsements".into(), "rows are retained"),
        (
            "UPDATE endorsement_receipts SET selected = false".into(),
            "rows are retained",
        ),
        (
            "DELETE FROM endorsement_receipts".into(),
            "rows are retained",
        ),
        ("TRUNCATE endorsement_receipts".into(), "rows are retained"),
    ] {
        let error = raw(&pool, &sql).await.unwrap_err();
        assert!(error.contains(message), "{sql}: {error}");
    }
    // A removal through SQL keeps the row and advances its order.
    raw(
        &pool,
        &format!(
            "UPDATE work_endorsements SET selected = false, version = 3, accepted_order = 3 WHERE actor_id = '{actor}'"
        ),
    )
    .await
    .unwrap();
    assert_eq!(work_row(&pool, R1, actor).await, Some((false, 3, 3)));
    // Comments are reached through their thread; no second work column could disagree.
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
            "accepted_order",
            "updated_at"
        ]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_work_endorsement_follows_accepted_order_and_receipts(pool: PgPool) {
    let vink = setup(&pool).await;
    let me = reader(1);
    let a = key();

    let first = vink
        .set_work_endorsement(me, on_work(&a, 1, true, R1))
        .await;
    assert_eq!(accepted(first), (state(true, 2, 2), count(1)));
    // Lost ACK: the same key and body is a historical replay, never applied twice.
    let retry = vink
        .set_work_endorsement(me, on_work(&a, 1, true, R1))
        .await
        .unwrap();
    assert_eq!(
        retry.result,
        EndorsementResult::Replayed {
            committed: state(true, 2, 2),
            observed: state(true, 2, 2),
        }
    );
    assert_eq!(retry.summary, count(1));
    // The same key with another body is refused.
    assert_eq!(
        vink.set_work_endorsement(me, on_work(&a, 2, false, R1))
            .await,
        Err(EndorsementError::KeyReused)
    );
    // Same-state at the current order is a new accepted command: order + 1, version kept.
    let again = vink
        .set_work_endorsement(me, on_work(&key(), 2, true, R1))
        .await;
    assert_eq!(accepted(again), (state(true, 2, 3), count(1)));
    // A stale order conflicts, whatever the desire.
    for endorsed in [true, false] {
        assert_eq!(
            vink.set_work_endorsement(me, on_work(&key(), 2, endorsed, R1))
                .await,
            Err(EndorsementError::Conflict {
                observed: state(true, 2, 3)
            })
        );
    }
    let off = vink
        .set_work_endorsement(me, on_work(&key(), 3, false, R1))
        .await;
    assert_eq!(accepted(off), (state(false, 3, 4), count(0)));
    assert_eq!(
        work_row(&pool, R1, reader_id(1)).await,
        Some((false, 3, 4)),
        "removing keeps the row and its order"
    );
    // ABA: replaying the very first command reports its history and the current OFF state; it
    // never resurrects the selection.
    let replay = vink
        .set_work_endorsement(me, on_work(&a, 1, true, R1))
        .await
        .unwrap();
    assert_eq!(
        replay.result,
        EndorsementResult::Replayed {
            committed: state(true, 2, 2),
            observed: state(false, 3, 4),
        }
    );
    assert_eq!(work_summary(&vink).await, Ok(EndorsementCount::ZERO));
    let readback = vink
        .own_endorsements(me, sample_work(), None)
        .await
        .unwrap();
    assert_eq!(
        readback.work,
        OwnEndorsement {
            state: Some(state(false, 3, 4)),
            capability: Capability::Allowed,
        }
    );
    assert_eq!(rows(&pool, "endorsement_receipts").await, 3);

    // The author cannot select their own work and gets no row.
    assert_eq!(
        vink.set_work_endorsement(author(), on_work(&key(), 1, true, R1))
            .await,
        Err(EndorsementError::SelfEndorsement)
    );
    assert_eq!(work_row(&pool, R1, dev_seed::SAMPLE_AUTHOR).await, None);
    let mine = vink
        .own_endorsements(author(), sample_work(), None)
        .await
        .unwrap();
    assert_eq!(mine.work.capability, Capability::OwnContent);
    // A resolved principal without a Vink record is not an actor, before any decision.
    let ghost = Some(PrincipalId::from_uuid(Uuid::now_v7()));
    assert_eq!(
        vink.set_work_endorsement(ghost, on_work(&key(), 1, true, R1))
            .await,
        Err(EndorsementError::Unauthenticated)
    );
    assert_eq!(
        vink.own_endorsements(ghost, sample_work(), None).await,
        Err(EndorsementError::Unauthenticated)
    );
    assert_eq!(rows(&pool, "work_endorsements").await, 1);
    assert_eq!(rows(&pool, "endorsement_receipts").await, 3);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_initial_remove_fences_a_select_created_earlier(pool: PgPool) {
    // OFF@1. A = select@1 was created first but is delayed; B = remove@1 arrives first. B is a
    // new accepted command (order 2, version 1), so A conflicts instead of reviving it.
    let vink = setup(&pool).await;
    let b = vink
        .set_work_endorsement(reader(1), on_work(&key(), 1, false, R1))
        .await;
    assert_eq!(accepted(b).0, state(false, 1, 2));
    assert_eq!(
        vink.set_work_endorsement(reader(1), on_work(&key(), 1, true, R1))
            .await,
        Err(EndorsementError::Conflict {
            observed: state(false, 1, 2)
        })
    );
    assert_eq!(work_summary(&vink).await, Ok(EndorsementCount::ZERO));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn each_revision_is_its_own_target_and_own_removal_survives_hiding(pool: PgPool) {
    let vink = setup(&pool).await;
    for n in [1, 2] {
        vink.set_work_endorsement(reader(n), on_work(&key(), 1, true, R1))
            .await
            .unwrap();
    }
    assert_eq!(
        work_summary(&vink).await,
        EndorsementCount::new(2).ok_or(EndorsementError::Internal)
    );

    let r2 = new_revision(&pool).await;
    point(&pool, r2).await;
    // The new text starts its own count; earlier selections stay stored and are not transferred.
    let summaries = vink
        .endorsement_summaries(sample_work(), None)
        .await
        .unwrap();
    assert_eq!(
        (summaries.work, summaries.revision),
        (EndorsementCount::ZERO, RevisionId::from_uuid(r2))
    );
    let mine = vink
        .own_endorsements(reader(1), sample_work(), None)
        .await
        .unwrap();
    assert_eq!(
        (mine.revision, mine.work.state),
        (RevisionId::from_uuid(r2), Some(EndorsementState::INITIAL))
    );
    // Selecting the old page's revision is refused with the current one named; nothing written.
    assert_eq!(
        vink.set_work_endorsement(reader(3), on_work(&key(), 1, true, R1))
            .await,
        Err(EndorsementError::RevisionChanged {
            published: RevisionId::from_uuid(r2)
        })
    );
    assert_eq!(work_row(&pool, R1, reader_id(3)).await, None);
    // The owner of a retained record of the old revision may still remove it.
    let removed = vink
        .set_work_endorsement(reader(2), on_work(&key(), 2, false, R1))
        .await;
    assert_eq!(
        accepted(removed),
        (state(false, 3, 3), None),
        "no count for a hidden target"
    );
    let selected = vink
        .set_work_endorsement(reader(1), on_work(&key(), 1, true, r2))
        .await;
    assert_eq!(accepted(selected), (state(true, 2, 2), count(1)));

    // Republishing the earlier revision makes ITS selections effective again (same text).
    point(&pool, R1).await;
    assert_eq!(
        work_summary(&vink).await,
        Ok(EndorsementCount::new(1).unwrap()),
        "reader 1 only"
    );
    point(&pool, r2).await;

    // Withdrawal hides the work from selection and from every read; own removal still works.
    sqlx::query(WITHDRAW)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        vink.set_work_endorsement(reader(4), on_work(&key(), 1, true, r2))
            .await,
        Err(EndorsementError::NotFound)
    );
    assert_eq!(
        vink.set_work_endorsement(reader(4), on_work(&key(), 1, false, r2))
            .await,
        Err(EndorsementError::NotFound),
        "no retained record: nothing to remove, nothing revealed"
    );
    assert_eq!(work_summary(&vink).await, Err(EndorsementError::NotFound));
    assert_eq!(
        vink.own_endorsements(reader(1), sample_work(), None).await,
        Err(EndorsementError::NotFound)
    );
    let own_removal = vink
        .set_work_endorsement(reader(1), on_work(&key(), 2, false, r2))
        .await;
    assert_eq!(accepted(own_removal), (state(false, 3, 3), None));
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, now())",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(r2)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(work_summary(&vink).await, Ok(EndorsementCount::ZERO));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn comment_endorsements_follow_visibility_scope_and_ignore_comments_close(pool: PgPool) {
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
    // Reader 1 endorses the foreign comment through ITS work's route.
    vink.set_comment_endorsement(
        reader(1),
        SetCommentEndorsement {
            work_id: foreign_work,
            ..on_comment(&key(), foreign, 1, true)
        },
    )
    .await
    .unwrap();

    let me = reader(1);
    let on = vink
        .set_comment_endorsement(me, on_comment(&key(), root, 1, true))
        .await;
    assert_eq!(accepted(on), (state(true, 2, 2), count(1)));
    vink.set_comment_endorsement(me, on_comment(&key(), reply, 1, true))
        .await
        .unwrap();
    vink.set_comment_endorsement(me, on_comment(&key(), hidden_root, 1, true))
        .await
        .unwrap();
    decide(&vink, hidden_root, 2, ModerationAction::Remove).await;
    // A reply under a hidden root stays endorsable; the hidden root does not.
    vink.set_comment_endorsement(me, on_comment(&key(), under_hidden, 1, true))
        .await
        .unwrap();
    for target in [
        pending,
        rejected,
        removed,
        hidden_root,
        CommentId::generate(),
    ] {
        let expected = if target == hidden_root { 2 } else { 1 };
        assert_eq!(
            vink.set_comment_endorsement(me, on_comment(&key(), target, expected, true))
                .await,
            Err(EndorsementError::NotFound),
            "{target}"
        );
    }
    // A comment of another work is unreachable through this work, even to remove the actor's
    // own retained record of it.
    assert_eq!(
        vink.set_comment_endorsement(me, on_comment(&key(), foreign, 2, false))
            .await,
        Err(EndorsementError::NotFound)
    );
    assert_eq!(
        comment_row(&pool, foreign, reader_id(1)).await,
        Some((true, 2, 2))
    );
    // Own removal of a retained record survives the comment being hidden.
    let unhidden = vink
        .set_comment_endorsement(me, on_comment(&key(), hidden_root, 2, false))
        .await;
    assert_eq!(accepted(unhidden), (state(false, 3, 3), None));
    // The writer's own pending comment is not public; their public root is own content.
    assert_eq!(
        vink.set_comment_endorsement(other(), on_comment(&key(), pending, 1, true))
            .await,
        Err(EndorsementError::NotFound)
    );
    assert_eq!(
        vink.set_comment_endorsement(other(), on_comment(&key(), root, 1, true))
            .await,
        Err(EndorsementError::SelfEndorsement)
    );
    // Nothing cascaded to the root, the work or the other tier; hidden is never zero.
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
    assert_eq!(summaries.work, EndorsementCount::ZERO);
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
            (hidden_root, None),
            (pending, None),
            (foreign, None),
        ]
    );

    // Closing comments governs text, not endorsement (CHOICE-09 A): select and remove proceed.
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
    let closed_select = vink
        .set_comment_endorsement(reader(2), on_comment(&key(), root, 1, true))
        .await;
    assert_eq!(accepted(closed_select), (state(true, 2, 2), count(2)));
    let closed_remove = vink
        .set_comment_endorsement(me, on_comment(&key(), root, 2, false))
        .await;
    assert_eq!(accepted(closed_remove), (state(false, 3, 3), count(1)));

    // Own state: actor-scoped, tokens only where visible, advisory capabilities.
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
            OwnEndorsement {
                state: Some(state(false, 3, 3)),
                capability: Capability::Allowed,
            },
            OwnEndorsement {
                state: Some(state(true, 2, 2)),
                capability: Capability::Allowed,
            },
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
            OwnEndorsement {
                state: Some(EndorsementState::INITIAL),
                capability: Capability::OwnContent,
            },
            OwnEndorsement {
                state: Some(EndorsementState::INITIAL),
                capability: Capability::Allowed,
            },
            OwnEndorsement::UNAVAILABLE,
            OwnEndorsement::UNAVAILABLE,
        ],
        "another actor's records never leak into this actor's state"
    );
    assert_eq!(
        vink.own_endorsements(None, sample_work(), Some(&ids)).await,
        Err(EndorsementError::Unauthenticated)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_commands_have_exactly_one_winner(pool: PgPool) {
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc bị bấm nhiều lần").await;

    // Eight copies of ONE command (same key and body): one accepted, seven replays.
    let a = key();
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let (vink, a) = (Arc::clone(&vink), a.clone());
            tokio::spawn(async move {
                vink.set_work_endorsement(reader(1), on_work(&a, 1, true, R1))
                    .await
            })
        })
        .collect();
    let mut accepted_count = 0;
    for task in tasks {
        match task.await.unwrap().unwrap().result {
            EndorsementResult::Accepted(state) => {
                accepted_count += 1;
                assert_eq!(state, self::state(true, 2, 2));
            }
            EndorsementResult::Replayed { committed, .. } => {
                assert_eq!(committed, state(true, 2, 2));
            }
        }
    }
    assert_eq!(accepted_count, 1);
    assert_eq!(work_row(&pool, R1, reader_id(1)).await, Some((true, 2, 2)));

    // Eight first commands with different keys, mixed desires, all at order 1: one accepted,
    // the rest conflict; the count matches the row.
    let tasks: Vec<_> = (0..8)
        .map(|n| {
            let vink = Arc::clone(&vink);
            tokio::spawn(async move {
                vink.set_comment_endorsement(reader(2), on_comment(&key(), root, 1, n % 2 == 0))
                    .await
            })
        })
        .collect();
    let mut accepted_count = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(outcome) => {
                assert!(matches!(outcome.result, EndorsementResult::Accepted(_)));
                accepted_count += 1;
            }
            Err(EndorsementError::Conflict { observed }) => assert_eq!(observed.order().get(), 2),
            Err(other) => panic!("{other:?}"),
        }
    }
    assert_eq!(accepted_count, 1);
    let (selected, _, order) = comment_row(&pool, root, reader_id(2)).await.unwrap();
    assert_eq!(order, 2);
    let summary = vink
        .endorsement_summaries(sample_work(), Some(&root.to_string()))
        .await
        .unwrap();
    assert_eq!(summary.comments[0].summary, count(i64::from(selected)));

    // One key, two targets at once: the key namespace is the actor's, across targets.
    let shared = key();
    let (left, right) = {
        let (v1, v2, k1, k2) = (
            Arc::clone(&vink),
            Arc::clone(&vink),
            shared.clone(),
            shared.clone(),
        );
        (
            tokio::spawn(async move {
                v1.set_work_endorsement(reader(3), on_work(&k1, 1, true, R1))
                    .await
            }),
            tokio::spawn(async move {
                v2.set_comment_endorsement(reader(3), on_comment(&k2, root, 1, true))
                    .await
            }),
        )
    };
    let outcomes = [
        left.await.unwrap().map(|_| ()),
        right.await.unwrap().map(|_| ()),
    ];
    assert_eq!(
        outcomes.iter().filter(|o| o.is_ok()).count(),
        1,
        "{outcomes:?}"
    );
    assert!(
        outcomes.contains(&Err(EndorsementError::KeyReused)),
        "{outcomes:?}"
    );
    let receipts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM endorsement_receipts WHERE command_key = $1")
            .bind(Uuid::parse_str(&shared).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(receipts, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_uncommitted_first_command_decides_a_waiting_insert_in_both_commit_orders(pool: PgPool) {
    let vink = setup(&pool).await;
    // Order 1: a concurrent first command (a remove, accepted at order 1) is uncommitted when
    // this select decides from the absent row. Its INSERT waits; after the commit it reads that
    // row under lock and decides again: a conflict, nothing written, no receipt.
    let mut first = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO work_endorsements (work_id, revision_id, actor_id, selected, version, accepted_order)
         VALUES ($1, $2, $3, false, 1, 2)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(R1)
    .bind(reader_id(1))
    .execute(&mut *first)
    .await
    .unwrap();
    let racing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_work_endorsement(reader(1), on_work(&key(), 1, true, R1))
                .await
        })
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
            observed: state(false, 1, 2)
        })
    );
    assert_eq!(work_row(&pool, R1, reader_id(1)).await, Some((false, 1, 2)));
    assert_eq!(rows(&pool, "endorsement_receipts").await, 0);

    // Order 2: the concurrent first command rolls back; the waiting INSERT then wins.
    let mut first = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO work_endorsements (work_id, revision_id, actor_id, selected, version, accepted_order)
         VALUES ($1, $2, $3, true, 2, 2)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(R1)
    .bind(reader_id(2))
    .execute(&mut *first)
    .await
    .unwrap();
    let racing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_work_endorsement(reader(2), on_work(&key(), 1, false, R1))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(!racing.is_finished());
    first.rollback().await.unwrap();
    assert_eq!(accepted(racing.await.unwrap()).0, state(false, 1, 2));
    assert_eq!(work_row(&pool, R1, reader_id(2)).await, Some((false, 1, 2)));
    assert_eq!(rows(&pool, "endorsement_receipts").await, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn endorsing_and_moderation_serialize_on_the_comment(pool: PgPool) {
    let vink = setup(&pool).await;
    let first = public_comment(&vink, other(), None, "Gốc sẽ bị gỡ khi đang ghi nhận").await;
    let second = public_comment(&vink, other(), None, "Gốc thứ hai").await;

    // An uncommitted moderation removal holds the comment row; the select waits and then finds
    // the comment unavailable.
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
            vink.set_comment_endorsement(reader(1), on_comment(&key(), first, 1, true))
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
    // moderation decision waits for it; afterwards the stored record is hidden, not lost.
    vink.set_comment_endorsement(reader(1), on_comment(&key(), second, 1, false))
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
            vink.set_comment_endorsement(reader(1), on_comment(&key(), second, 2, true))
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
    assert_eq!(accepted(endorsing.await.unwrap()).0, state(true, 2, 3));
    removing.await.unwrap();
    assert_eq!(
        comment_row(&pool, second, reader_id(1)).await,
        Some((true, 2, 3))
    );
    let summaries = vink
        .endorsement_summaries(sample_work(), Some(&second.to_string()))
        .await
        .unwrap();
    assert_eq!(summaries.comments[0].summary, None);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn endorsing_and_publication_changes_serialize_on_the_pointer(pool: PgPool) {
    let vink = setup(&pool).await;
    let r2 = new_revision(&pool).await;

    // An uncommitted republish holds the pointer; a select of the old page waits and then learns
    // that the text changed, writing nothing.
    let mut republishing = pool.begin().await.unwrap();
    sqlx::query(POINT)
        .bind(dev_seed::SAMPLE_WORK)
        .bind(r2)
        .execute(&mut *republishing)
        .await
        .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_work_endorsement(reader(1), on_work(&key(), 1, true, R1))
                .await
        })
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
    assert_eq!(work_row(&pool, R1, reader_id(1)).await, None);

    // An uncommitted withdrawal that rolls back: the select waits, then applies.
    let mut withdrawing = pool.begin().await.unwrap();
    sqlx::query(WITHDRAW)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *withdrawing)
        .await
        .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_work_endorsement(reader(1), on_work(&key(), 1, true, r2))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    assert!(!endorsing.is_finished());
    withdrawing.rollback().await.unwrap();
    assert_eq!(accepted(endorsing.await.unwrap()).0, state(true, 2, 2));

    // The other order: a paused in-flight endorsement holds the pointer FOR SHARE, so a
    // withdrawal waits for its commit and then hides it.
    let mut pauser = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT 1 FROM work_endorsements WHERE work_id = $1 AND revision_id = $2 AND actor_id = $3
            FOR UPDATE",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(r2)
    .bind(reader_id(1))
    .execute(&mut *pauser)
    .await
    .unwrap();
    let endorsing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.set_work_endorsement(reader(1), on_work(&key(), 2, false, r2))
                .await
        })
    };
    tokio::time::sleep(PAUSE).await;
    let withdrawn = {
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
        !withdrawn.is_finished(),
        "withdrawal waits for the in-flight endorsement"
    );
    pauser.rollback().await.unwrap();
    assert_eq!(accepted(endorsing.await.unwrap()).0, state(false, 3, 3));
    assert_eq!(withdrawn.await.unwrap(), 1);
    assert_eq!(work_summary(&vink).await, Err(EndorsementError::NotFound));
    assert_eq!(work_row(&pool, r2, reader_id(1)).await, Some((false, 3, 3)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_refused_or_failed_write_leaves_nothing_behind(pool: PgPool) {
    let vink = setup(&pool).await;
    vink.set_work_endorsement(reader(1), on_work(&key(), 1, true, R1))
        .await
        .unwrap();
    // The last order: the core refuses to wrap; nothing is written, no receipt is added.
    sqlx::query("UPDATE work_endorsements SET accepted_order = $1 WHERE actor_id = $2")
        .bind(i64::MAX)
        .bind(reader_id(1))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        vink.set_work_endorsement(reader(1), on_work(&key(), i64::MAX, false, R1))
            .await,
        Err(EndorsementError::Internal)
    );
    assert_eq!(
        work_row(&pool, R1, reader_id(1)).await,
        Some((true, 2, i64::MAX))
    );
    assert_eq!(
        work_summary(&vink).await,
        Ok(EndorsementCount::new(1).unwrap())
    );
    let _ = vink
        .set_work_endorsement(author(), on_work(&key(), 1, true, R1))
        .await;
    let _ = vink
        .set_work_endorsement(reader(2), on_work(&key(), 1, true, Uuid::now_v7()))
        .await;
    let _ = vink
        .set_work_endorsement(reader(2), on_work(&key(), 5, true, R1))
        .await;
    assert_eq!(rows(&pool, "work_endorsements").await, 1);
    assert_eq!(rows(&pool, "endorsement_receipts").await, 1);
}

/// A failure AFTER the state row was written (here: the receipt INSERT, made to fail by a
/// test-only trigger) rolls the whole command back: no state change, no orphan row, no count
/// change, and the command key is not consumed, so the same key and body later commit once.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_failure_after_the_state_write_rolls_back_state_count_and_key(pool: PgPool) {
    let vink = setup(&pool).await;
    let root = public_comment(&vink, other(), None, "Gốc cho lỗi giữa giao dịch").await;
    vink.set_work_endorsement(reader(1), on_work(&key(), 1, true, R1))
        .await
        .unwrap();
    raw(
        &pool,
        "CREATE FUNCTION test_fail_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'injected receipt failure'; END; $$;
         CREATE TRIGGER test_fail_receipt BEFORE INSERT ON endorsement_receipts
             FOR EACH ROW EXECUTE FUNCTION test_fail_receipt();",
    )
    .await
    .unwrap();

    // UPDATE path (reader 1 has a row) and INSERT path (reader 2's first command, a comment).
    let (update_key, insert_key) = (key(), key());
    assert_eq!(
        vink.set_work_endorsement(reader(1), on_work(&update_key, 2, false, R1))
            .await,
        Err(EndorsementError::Internal)
    );
    assert_eq!(
        vink.set_comment_endorsement(reader(2), on_comment(&insert_key, root, 1, true))
            .await,
        Err(EndorsementError::Internal)
    );
    assert_eq!(work_row(&pool, R1, reader_id(1)).await, Some((true, 2, 2)));
    assert_eq!(comment_row(&pool, root, reader_id(2)).await, None);
    assert_eq!(rows(&pool, "endorsement_receipts").await, 1);
    assert_eq!(
        work_summary(&vink).await,
        Ok(EndorsementCount::new(1).unwrap())
    );

    raw(
        &pool,
        "DROP TRIGGER test_fail_receipt ON endorsement_receipts",
    )
    .await
    .unwrap();
    // Nothing of the failed attempts survived: the same keys are new commands, not replays.
    assert_eq!(
        accepted(
            vink.set_work_endorsement(reader(1), on_work(&update_key, 2, false, R1))
                .await
        ),
        (state(false, 3, 3), count(0))
    );
    assert_eq!(
        accepted(
            vink.set_comment_endorsement(reader(2), on_comment(&insert_key, root, 1, true))
                .await
        ),
        (state(true, 2, 2), count(1))
    );
    assert_eq!(rows(&pool, "endorsement_receipts").await, 3);
}

/// Rust hydration fails closed on its own: if the schema's parity CHECK were missing (here
/// dropped in the test database), an unreachable stored state is an integrity error on the
/// actor's reads and writes, never shown as a selection or used as the base of a new command.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_unreachable_stored_state_fails_closed_without_the_schema_check(pool: PgPool) {
    let vink = setup(&pool).await;
    raw(
        &pool,
        "ALTER TABLE work_endorsements DROP CONSTRAINT work_endorsements_version_parity_check",
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_endorsements (work_id, revision_id, actor_id, selected, version, accepted_order)
         VALUES ($1, $2, $3, true, 1, 2)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(R1)
    .bind(reader_id(1))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        vink.own_endorsements(reader(1), sample_work(), None).await,
        Err(EndorsementError::Internal)
    );
    for endorsed in [false, true] {
        assert_eq!(
            vink.set_work_endorsement(reader(1), on_work(&key(), 2, endorsed, R1))
                .await,
            Err(EndorsementError::Internal)
        );
    }
    assert_eq!(work_row(&pool, R1, reader_id(1)).await, Some((true, 1, 2)));
    assert_eq!(rows(&pool, "endorsement_receipts").await, 0);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn summaries_equal_an_independent_recount_of_canonical_rows(pool: PgPool) {
    let vink = setup(&pool).await;
    let roots = [
        public_comment(&vink, other(), None, "Gốc một").await,
        public_comment(&vink, other(), None, "Gốc hai").await,
    ];
    let r2 = new_revision(&pool).await;
    // Readers 1–4 select revision 1; revision 2 is published; readers 3–6 select it; reader 5
    // removes it again. Comments: readers 1–6 select root one, readers 1, 3, 5 remove it,
    // reader 2 reaffirms (same state, new order, no second vote).
    for n in 1..=4 {
        vink.set_work_endorsement(reader(n), on_work(&key(), 1, true, R1))
            .await
            .unwrap();
    }
    point(&pool, r2).await;
    for n in 3..=6 {
        vink.set_work_endorsement(reader(n), on_work(&key(), 1, true, r2))
            .await
            .unwrap();
    }
    vink.set_work_endorsement(reader(5), on_work(&key(), 2, false, r2))
        .await
        .unwrap();
    for n in 1..=6 {
        vink.set_comment_endorsement(reader(n), on_comment(&key(), roots[0], 1, true))
            .await
            .unwrap();
    }
    for n in [1, 3, 5] {
        vink.set_comment_endorsement(reader(n), on_comment(&key(), roots[0], 2, false))
            .await
            .unwrap();
    }
    vink.set_comment_endorsement(reader(2), on_comment(&key(), roots[0], 2, true))
        .await
        .unwrap();

    // Independent recount: every selected row joined to the CURRENT pointer.
    let (effective, all_selected): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE e.selected AND e.revision_id = p.revision_id),
                count(*) FILTER (WHERE e.selected)
           FROM work_endorsements e JOIN publications p USING (work_id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (effective, all_selected),
        (3, 7),
        "r2: 3, 4, 6; r1: 1, 2, 3, 4"
    );
    let per_comment: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT c.id, count(e.actor_id) FILTER (WHERE e.selected)
           FROM comments c LEFT JOIN comment_endorsements e ON e.comment_id = c.id
          GROUP BY c.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let ids = roots.map(|id| id.to_string()).join(",");
    let summaries = vink
        .endorsement_summaries(sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(summaries.work.get(), effective);
    for entry in &summaries.comments {
        let (_, expected) = per_comment
            .iter()
            .find(|(id, _)| *id == entry.comment_id.as_uuid())
            .unwrap();
        assert_eq!(entry.summary, count(*expected));
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
    let put = |key: &str, expected: i64, endorsed: bool, revision: Uuid| {
        Some(json!({
            "command_key": key,
            "expected_order": expected,
            "endorsed": endorsed,
            "revision_id": revision.to_string(),
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
    let a = key();
    let (status, _, _) = call(&app, "PUT", &mine, None, origin, put(&a, 1, true, R1)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, body) = call(&app, "PUT", &mine, me, None, put(&a, 1, true, R1)).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::FORBIDDEN, "forbidden")
    );
    for (body, field) in [
        (
            Some(
                json!({"command_key": a, "expected_order": 1, "endorsed": true,
                        "revision_id": R1, "actor_id": dev_seed::SAMPLE_AUTHOR}),
            ),
            "body",
        ),
        (
            Some(
                json!({"command_key": a, "expected_order": 1, "endorsed": true,
                        "revision_id": R1, "count": 99}),
            ),
            "body",
        ),
        (
            Some(json!({"command_key": a, "expected_order": 1, "endorsed": true})),
            "body",
        ),
        (put(&a, 0, true, R1), "expected_order"),
        (put("not-a-uuid", 1, true, R1), "command_key"),
    ] {
        let (status, cache, response) = call(&app, "PUT", &mine, me, origin, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
        assert_eq!(json(&response)["error"]["field"], field, "{response}");
        assert_eq!(cache.as_deref(), Some("no-store"));
    }
    let (status, cache, body) = call(&app, "PUT", &mine, me, origin, put(&a, 1, true, R1)).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(
        json(&body),
        json!({
            "work_id": dev_seed::SAMPLE_WORK.to_string(),
            "revision_id": R1.to_string(),
            "comment_id": null,
            "outcome": "accepted",
            "endorsement": {"selected": true, "version": 2, "order": 2},
            "committed": null,
            "summary": {"status": "available", "count": 1}
        })
    );
    // Lost ACK retry with the same key and body: 200 replayed, history + current.
    let (status, _, body) = call(&app, "PUT", &mine, me, origin, put(&a, 1, true, R1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["outcome"], "replayed");
    assert_eq!(json(&body)["committed"]["order"], 2);
    // Same key, other body: 422 idempotency_key_reused.
    let (status, _, body) = call(&app, "PUT", &mine, me, origin, put(&a, 2, false, R1)).await;
    assert_eq!(
        (status, code(&body).as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused")
    );
    // Stale order with a fresh key: 409 with the observed state.
    let (status, _, body) = call(&app, "PUT", &mine, me, origin, put(&key(), 1, false, R1)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let error = &json(&body)["error"];
    assert_eq!(
        (error["code"].as_str(), error["current_version"].as_i64()),
        (Some("version_conflict"), Some(2))
    );
    assert_eq!(
        error["endorsement"],
        json!({"selected": true, "version": 2, "order": 2})
    );
    // Self-endorsement, unknown or malformed work.
    let (status, _, body) = call(
        &app,
        "PUT",
        &mine,
        Some(dev_seed::SAMPLE_AUTHOR),
        origin,
        put(&key(), 1, true, R1),
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
        let (status, _, body) =
            call(&app, "PUT", &path, me, origin, put(&key(), 1, true, R1)).await;
        assert_eq!(
            (status, code(&body).as_str()),
            (StatusCode::NOT_FOUND, "not_found")
        );
    }

    // READ-01 is untouched: endorsement writes never change the public representation.
    assert_eq!(etag(app.clone()).await, before);
    let (_, _, body) = call(&app, "GET", &publication, None, None, None).await;
    assert!(!body.contains("endorse"), "{body}");

    // A republished work: selecting the old page's revision is 409 revision_changed.
    let r2 = new_revision(&pool).await;
    point(&pool, r2).await;
    let (status, _, body) = call(
        &app,
        "PUT",
        &mine,
        Some(reader_id(2)),
        origin,
        put(&key(), 1, true, R1),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "revision_changed");
    assert_eq!(json(&body)["error"]["current_revision_id"], r2.to_string());

    // Comments: select, cross-work route, batch reads.
    let comment_body = |key: &str, expected: i64, endorsed: bool| {
        Some(json!({"command_key": key, "expected_order": expected, "endorsed": endorsed}))
    };
    let (status, _, body) = call(
        &app,
        "PUT",
        &on_root,
        me,
        origin,
        comment_body(&key(), 1, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body)["comment_id"], root.to_string());
    assert_eq!(json(&body)["revision_id"], Value::Null);
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
        comment_body(&key(), 2, false),
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
    assert_eq!(json(&body)["revision_id"], r2.to_string());
    assert_eq!(json(&body)["work"]["state"], "not_selected");
    assert_eq!(json(&body)["comments"][0]["state"], "selected");
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
        put(&key(), 2, false, R1),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let _ = EndorsementTarget::Work {
        work: sample_work(),
        revision: RevisionId::from_uuid(R1),
    };
}
