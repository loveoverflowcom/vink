//! Điểm Vink outbox (#645) on real PostgreSQL: the activation cutover, community events written
//! by the production endorsement transaction (only on a real change, with the context read under
//! its locks), and the as-of markers written by triggers for publication, comment and principal
//! changes. Publication pointer changes are SQL fixtures, as in the LVE tests.

use vink_app::application::discussion::{ModerateComment, ModerationAction, SubmitComment};
use vink_app::application::endorsement::{
    EndorsementOutcomeView, EndorsementResult, SetCommentEndorsement, SetWorkEndorsement,
};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::ids::CommentId;
use vink_app::domain::principal::SuspensionReason;
use vink_app::domain::reputation::outbox::ActivationChange;
use vink_app::persistence::reputation;
use vink_app::persistence::{grant_moderator, reinstate_principal, suspend_principal};

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x6666_6666_6666_4666_8666_6666_6666_7777);
const R1: Uuid = dev_seed::SAMPLE_REVISION;

fn reader_id(n: u128) -> Uuid {
    Uuid::from_u128(0x7777_7777_7777_4777_8777_0000_0000_1000 + n)
}

async fn setup(pool: &PgPool) -> Vink<PgStore> {
    let vink = seeded(pool).await;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Kiểm duyệt viên #645')")
        .bind(MODERATOR)
        .execute(pool)
        .await
        .unwrap();
    for n in 1..=3 {
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
            .bind(reader_id(n))
            .bind(format!("Bạn đọc {n} (dữ liệu kiểm thử #645)"))
            .execute(pool)
            .await
            .unwrap();
    }
    let note = GrantNote::parse("Kiểm duyệt viên của bộ test #645").unwrap();
    grant_moderator(pool, PrincipalId::from_uuid(MODERATOR), &note)
        .await
        .unwrap();
    vink
}

async fn activate(pool: &PgPool) {
    let change = reputation::set_activation(pool, true, "kiểm thử #645: bật Điểm Vink")
        .await
        .unwrap();
    assert!(
        matches!(change, ActivationChange::Changed(next) if next.active() && next.epoch() == 1)
    );
}

fn key() -> String {
    Uuid::now_v7().to_string()
}

fn on_work(key: &str, expected: i64, endorsed: bool) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: sample_work(),
        revision: R1.to_string(),
        command_key: key.into(),
        expected_order: expected,
        endorsed,
    }
}

fn accepted(outcome: Result<EndorsementOutcomeView, impl std::fmt::Debug>) {
    match outcome {
        Ok(EndorsementOutcomeView {
            result: EndorsementResult::Accepted(_),
            ..
        }) => {}
        other => panic!("expected an accepted command, got {other:?}"),
    }
}

#[derive(Debug, sqlx::FromRow, PartialEq, Eq)]
struct Event {
    kind: String,
    actor_id: Option<Uuid>,
    recipient_id: Option<Uuid>,
    work_id: Option<Uuid>,
    revision_id: Option<Uuid>,
    comment_id: Option<Uuid>,
    principal_id: Option<Uuid>,
    selected: Option<bool>,
    version: Option<i64>,
    accepted_order: Option<i64>,
    work_public: Option<bool>,
    hidden_revision_id: Option<Uuid>,
    comment_public: Option<bool>,
    principal_suspended: Option<bool>,
    principal_deleted: Option<bool>,
}

async fn events(pool: &PgPool) -> Vec<Event> {
    sqlx::query_as(
        "SELECT kind, actor_id, recipient_id, work_id, revision_id, comment_id, principal_id,
                selected, version, accepted_order, work_public, hidden_revision_id, comment_public,
                principal_suspended, principal_deleted
           FROM reputation_events ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn owner(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT owner_id FROM works WHERE id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn inactive_reputation_writes_nothing_and_activation_is_audited(pool: PgPool) {
    let vink = setup(&pool).await;
    let me = principal(reader_id(1));
    accepted(
        vink.set_work_endorsement(me, on_work(&key(), 1, true))
            .await,
    );
    assert!(events(&pool).await.is_empty(), "inactive: no event");

    activate(&pool).await;
    let again = reputation::set_activation(&pool, true, "kiểm thử #645: bật lại")
        .await
        .unwrap();
    assert!(matches!(again, ActivationChange::Unchanged(_)));
    let off = reputation::set_activation(&pool, false, "kiểm thử #645: tắt")
        .await
        .unwrap();
    assert!(matches!(off, ActivationChange::Changed(next) if !next.active() && next.epoch() == 1));
    let audit: Vec<(bool, i64, String)> =
        sqlx::query_as("SELECT active, epoch, note FROM reputation_activation_changes ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        audit,
        [
            (true, 1, "kiểm thử #645: bật Điểm Vink".to_string()),
            (false, 1, "kiểm thử #645: tắt".to_string()),
        ]
    );
    accepted(
        vink.set_work_endorsement(me, on_work(&key(), 2, false))
            .await,
    );
    assert!(events(&pool).await.is_empty(), "deactivated: no event");
}

#[sqlx::test(migrations = "./migrations")]
async fn only_real_tham_changes_owe_one_event_with_their_context(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let me = principal(reader_id(1));
    let first = key();
    accepted(
        vink.set_work_endorsement(me, on_work(&first, 1, true))
            .await,
    );
    // Exact replay of the same key: historical answer, no new event.
    accepted(
        vink.set_work_endorsement(me, on_work(&first, 1, true))
            .await,
    );
    // Same-state intent with a new key: the order advances, the selection does not.
    accepted(
        vink.set_work_endorsement(me, on_work(&key(), 2, true))
            .await,
    );
    accepted(
        vink.set_work_endorsement(me, on_work(&key(), 3, false))
            .await,
    );

    let owner = owner(&pool).await;
    let got = events(&pool).await;
    assert_eq!(got.len(), 2, "{got:?}");
    let expect = |selected: bool, version: i64, order: i64| Event {
        kind: "tham".into(),
        actor_id: Some(reader_id(1)),
        recipient_id: Some(owner),
        work_id: Some(dev_seed::SAMPLE_WORK),
        revision_id: Some(R1),
        comment_id: None,
        principal_id: None,
        selected: Some(selected),
        version: Some(version),
        accepted_order: Some(order),
        work_public: Some(true),
        hidden_revision_id: None,
        comment_public: None,
        principal_suspended: None,
        principal_deleted: None,
    };
    assert_eq!(got[0], expect(true, 2, 2));
    assert_eq!(got[1], expect(false, 3, 4));
    // The accepted time is the source transaction's clock, shared with its receipt.
    let same: bool = sqlx::query_scalar(
        "SELECT bool_and(e.accepted_at = r.created_at)
           FROM reputation_events e
           JOIN endorsement_receipts r ON r.actor_id = e.actor_id
            AND r.committed_version = e.version AND r.committed_order = e.accepted_order",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(same);
}

#[sqlx::test(migrations = "./migrations")]
async fn own_removal_after_withdrawal_carries_the_private_context(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let me = principal(reader_id(2));
    accepted(
        vink.set_work_endorsement(me, on_work(&key(), 1, true))
            .await,
    );
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    accepted(
        vink.set_work_endorsement(me, on_work(&key(), 2, false))
            .await,
    );
    let got = events(&pool).await;
    let kinds: Vec<&str> = got.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["tham", "work", "tham"]);
    assert_eq!(
        got[1].work_public,
        Some(false),
        "the withdrawal marker is as of its commit"
    );
    assert_eq!(
        got[2].work_public,
        Some(false),
        "OFF while parked: private context"
    );
    assert_eq!(got[2].selected, Some(false));
}

#[sqlx::test(migrations = "./migrations")]
async fn rolled_back_source_changes_owe_nothing(pool: PgPool) {
    let _vink = setup(&pool).await;
    activate(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert!(events(&pool).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn ung_events_and_comment_markers_carry_comment_facts(pool: PgPool) {
    let vink = setup(&pool).await;
    let writer = principal(reader_id(1));
    let id = vink
        .submit_comment(
            writer,
            SubmitComment {
                work_id: sample_work(),
                target: None,
                body_format: Default::default(),
                body: "Nhận xét kiểm thử #645".into(),
                command_key: format!("k-{}", Uuid::now_v7().simple()),
            },
        )
        .await
        .unwrap()
        .comment_id;
    activate(&pool).await;
    let moderate = |action: ModerationAction, expected: i64| ModerateComment {
        comment_id: id,
        expected_version: expected,
        action,
        reason: (action != ModerationAction::Approve)
            .then(|| "Không phù hợp quy tắc (dữ liệu kiểm thử).".to_string()),
    };
    vink.moderate_comment(principal(MODERATOR), moderate(ModerationAction::Approve, 1))
        .await
        .unwrap();
    let ung =
        |key: &str, expected: i64, endorsed: bool, comment: CommentId| SetCommentEndorsement {
            work_id: sample_work(),
            comment_id: comment,
            command_key: key.into(),
            expected_order: expected,
            endorsed,
        };
    accepted(
        vink.set_comment_endorsement(principal(reader_id(2)), ung(&key(), 1, true, id))
            .await,
    );
    let got = events(&pool).await;
    let kinds: Vec<&str> = got.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["comment", "ung"]);
    assert_eq!(got[0].comment_public, Some(true), "approval marker");
    assert_eq!(got[1].recipient_id, Some(reader_id(1)));
    assert_eq!(
        (got[1].work_public, got[1].comment_public, got[1].selected),
        (Some(true), Some(true), Some(true))
    );
    assert_eq!(got[1].comment_id, Some(id.as_uuid()));
}

#[sqlx::test(migrations = "./migrations")]
async fn principal_markers_follow_standing_and_deletion(pool: PgPool) {
    let _vink = setup(&pool).await;
    activate(&pool).await;
    let who = PrincipalId::from_uuid(reader_id(3));
    let reason = SuspensionReason::parse("Kiểm thử #645").unwrap();
    suspend_principal(&pool, who, &reason).await.unwrap();
    reinstate_principal(&pool, who).await.unwrap();
    let got = events(&pool).await;
    let standing: Vec<(Option<bool>, Option<bool>)> = got
        .iter()
        .filter(|e| e.kind == "principal")
        .map(|e| (e.principal_suspended, e.principal_deleted))
        .collect();
    assert_eq!(
        standing,
        [(Some(true), Some(false)), (Some(false), Some(false))]
    );
    assert!(got.iter().all(|e| e.principal_id == Some(reader_id(3))));
}

#[sqlx::test(migrations = "./migrations")]
async fn events_are_immutable(pool: PgPool) {
    let _vink = setup(&pool).await;
    activate(&pool).await;
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    let update = sqlx::query("UPDATE reputation_events SET work_public = true")
        .execute(&pool)
        .await;
    assert!(update.is_err(), "events cannot be rewritten");
    let bad = sqlx::query(
        "INSERT INTO reputation_events (kind, activation_epoch, accepted_at, work_id)
         VALUES ('work', 1, now(), $1)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .execute(&pool)
    .await;
    assert!(bad.is_err(), "a marker without its facts is refused");
}
