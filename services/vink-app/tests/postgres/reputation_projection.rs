//! Điểm Vink projection (#646) on real PostgreSQL: backfill from a consistent snapshot, the
//! consumer applying community events and markers through the pure decision, maturity, DEL-05,
//! correction and reconcile. Principal age is moved with SQL (`created_at`) instead of waiting
//! seven days; publication pointer changes are SQL fixtures, as in the LVE tests.

use vink_app::application::endorsement::{
    EndorsementOutcomeView, EndorsementResult, SetWorkEndorsement,
};
use vink_app::persistence::reputation;
use vink_app::persistence::reputation_projection::{
    self as projection, BackfillRefusal, CorrectionOutcome, CorrectionScope,
};
use vink_app::reputation_worker::ReputationWorker;

use super::*;

const R1: Uuid = dev_seed::SAMPLE_REVISION;

fn reader_id(n: u128) -> Uuid {
    Uuid::from_u128(0x7777_7777_7777_4777_8777_0000_0000_2000 + n)
}

async fn setup(pool: &PgPool, readers: u128) -> Vink<PgStore> {
    let vink = seeded(pool).await;
    for n in 1..=readers {
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
            .bind(reader_id(n))
            .bind(format!("Bạn đọc {n} (dữ liệu kiểm thử #646)"))
            .execute(pool)
            .await
            .unwrap();
    }
    vink
}

/// Ages a principal past the 7 × 24h threshold (Q30).
async fn age(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE principals SET created_at = now() - interval '8 days' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn activate(pool: &PgPool) {
    reputation::set_activation(pool, true, "kiểm thử #646: bật Điểm Vink")
        .await
        .unwrap();
}

async fn ready(pool: &PgPool) {
    activate(pool).await;
    let report = projection::backfill(pool, true).await.unwrap().unwrap();
    assert!(report.applied);
}

async fn drain(pool: &PgPool) {
    loop {
        let report = ReputationWorker::pass(pool, 50).await.unwrap();
        assert!(report.ready);
        if report.events == 0 && report.matured == 0 {
            break;
        }
    }
}

fn tham_command(key: &str, expected: i64, endorsed: bool) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: sample_work(),
        revision: R1.to_string(),
        command_key: key.into(),
        expected_order: expected,
        endorsed,
    }
}

async fn set(vink: &Vink<PgStore>, actor: Uuid, expected: i64, endorsed: bool) {
    match vink
        .set_work_endorsement(
            principal(actor),
            tham_command(&Uuid::now_v7().to_string(), expected, endorsed),
        )
        .await
    {
        Ok(EndorsementOutcomeView {
            result: EndorsementResult::Accepted(_),
            ..
        }) => {}
        other => panic!("expected an accepted command, got {other:?}"),
    }
}

async fn owner(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT owner_id FROM works WHERE id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn profile(pool: &PgPool, id: Uuid) -> Option<(i64, i64, i64)> {
    sqlx::query_as(
        "SELECT impact, contribution, score_micros FROM reputation_profiles WHERE principal_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn ledger(pool: &PgPool) -> Vec<(String, i64, i16)> {
    sqlx::query_as(
        "SELECT calendar, period, net FROM reputation_ledger WHERE net <> 0
          ORDER BY calendar, period",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn backfill_needs_active_producers_and_reflects_current_facts(pool: PgPool) {
    let vink = setup(&pool, 2).await;
    age(&pool, reader_id(1)).await;
    // Endorsements made before activation: reflected by the backfill, never as growth.
    set(&vink, reader_id(1), 1, true).await;
    set(&vink, reader_id(2), 1, true).await;
    assert_eq!(
        projection::backfill(&pool, true).await.unwrap(),
        Err(BackfillRefusal::NotActive)
    );
    activate(&pool).await;
    let dry = projection::backfill(&pool, false).await.unwrap().unwrap();
    assert!(!dry.applied);
    assert!(
        projection::current_readiness(&pool)
            .await
            .unwrap()
            .is_none(),
        "dry-run rolled back"
    );
    let report = projection::backfill(&pool, true).await.unwrap().unwrap();
    assert_eq!(
        (report.entitlements, report.effective, report.profiles),
        (2, 1, 1)
    );
    let owner = owner(&pool).await;
    // One mature actor counts; the immature one is held.
    assert_eq!(profile(&pool, owner).await, Some((1, 0, 1_009_539_383)));
    assert!(
        ledger(&pool).await.is_empty(),
        "no history before activation"
    );
    assert_eq!(
        projection::backfill(&pool, true).await.unwrap(),
        Err(BackfillRefusal::AlreadyReady)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn consumer_applies_community_changes_and_neutral_lifecycle(pool: PgPool) {
    let vink = setup(&pool, 1).await;
    age(&pool, reader_id(1)).await;
    ready(&pool).await;
    let owner = owner(&pool).await;
    set(&vink, reader_id(1), 1, true).await;
    drain(&pool).await;
    assert_eq!(profile(&pool, owner).await.map(|p| p.0), Some(1));
    // Withdrawal: neutral, current only.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    drain(&pool).await;
    assert_eq!(profile(&pool, owner).await.map(|p| p.0), Some(0));
    let parked = ledger(&pool).await;
    // OFF while parked: retire without a debit.
    set(&vink, reader_id(1), 2, false).await;
    drain(&pool).await;
    assert_eq!(
        ledger(&pool).await,
        parked,
        "neutral events never move the ledger"
    );
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM reputation_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        remaining, 0,
        "applied events are deleted in the same transaction"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn maturity_makes_held_interactions_current_only(pool: PgPool) {
    let vink = setup(&pool, 1).await;
    ready(&pool).await;
    let owner = owner(&pool).await;
    set(&vink, reader_id(1), 1, true).await;
    drain(&pool).await;
    assert_eq!(
        profile(&pool, owner).await.map(|p| p.0).unwrap_or(0),
        0,
        "held"
    );
    age(&pool, reader_id(1)).await;
    drain(&pool).await;
    assert_eq!(profile(&pool, owner).await.map(|p| p.0), Some(1), "matured");
    let states: (String, String) = sqlx::query_as(
        "SELECT week_state, month_state FROM reputation_entitlements WHERE actor_id = $1",
    )
    .bind(reader_id(1))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        ["unstarted", "burned"].contains(&states.0.as_str()),
        "held maturity never becomes growth: {states:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn deleted_actor_retires_without_debit_and_deleted_recipient_is_purged(pool: PgPool) {
    let vink = setup(&pool, 1).await;
    age(&pool, reader_id(1)).await;
    ready(&pool).await;
    let owner = owner(&pool).await;
    set(&vink, reader_id(1), 1, true).await;
    drain(&pool).await;
    let before = ledger(&pool).await;
    // The marker the account_deletions trigger writes (labelled fixture).
    let marker = "INSERT INTO reputation_events
                         (kind, activation_epoch, accepted_at, principal_id, principal_suspended,
                          principal_deleted)
                  VALUES ('principal', 1, now(), $1, false, true)";
    sqlx::query(marker)
        .bind(reader_id(1))
        .execute(&pool)
        .await
        .unwrap();
    drain(&pool).await;
    assert_eq!(profile(&pool, owner).await.map(|p| p.0), Some(0));
    assert_eq!(ledger(&pool).await, before, "DEL-05 never debits growth");
    sqlx::query(marker)
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    drain(&pool).await;
    assert_eq!(profile(&pool, owner).await, None, "recipient purge");
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM reputation_ledger WHERE recipient_id = $1")
            .bind(owner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn correction_is_audited_idempotent_and_leaves_source_counts(pool: PgPool) {
    let vink = setup(&pool, 1).await;
    age(&pool, reader_id(1)).await;
    ready(&pool).await;
    let owner = owner(&pool).await;
    set(&vink, reader_id(1), 1, true).await;
    drain(&pool).await;
    let scope = CorrectionScope::Pair {
        actor: reader_id(1),
        recipient: owner,
    };
    let note = "ops: bằng chứng vòng Thấm qua lại đã xác minh";
    let planned = projection::correct(&pool, "case-646", scope, note, false)
        .await
        .unwrap();
    assert!(matches!(
        planned,
        CorrectionOutcome::Planned {
            entitlements: 1,
            ..
        }
    ));
    assert_eq!(
        profile(&pool, owner).await.map(|p| p.0),
        Some(1),
        "dry-run writes nothing"
    );
    let applied = projection::correct(&pool, "case-646", scope, note, true)
        .await
        .unwrap();
    assert!(matches!(
        applied,
        CorrectionOutcome::Applied {
            entitlements: 1,
            ..
        }
    ));
    assert_eq!(profile(&pool, owner).await.map(|p| p.0), Some(0));
    assert_eq!(
        projection::correct(&pool, "case-646", scope, note, true)
            .await
            .unwrap(),
        CorrectionOutcome::AlreadyApplied
    );
    let other = CorrectionScope::Entitlement {
        component: vink_app::domain::reputation::entitlement::Component::Impact,
        actor: reader_id(1),
        target: dev_seed::SAMPLE_WORK,
    };
    assert_eq!(
        projection::correct(&pool, "case-646", other, note, true)
            .await
            .unwrap(),
        CorrectionOutcome::KeyReused
    );
    let source: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM work_endorsements WHERE work_id = $1 AND selected",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source, 1, "the public Thấm count is unchanged");
    assert!(ledger(&pool).await.is_empty(), "history restated");
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM reputation_corrections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn reconcile_reports_and_fixes_drift(pool: PgPool) {
    let vink = setup(&pool, 1).await;
    age(&pool, reader_id(1)).await;
    ready(&pool).await;
    let owner = owner(&pool).await;
    set(&vink, reader_id(1), 1, true).await;
    drain(&pool).await;
    assert!(
        projection::reconcile(&pool, false)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("UPDATE reputation_profiles SET impact = 5 WHERE principal_id = $1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    let drift = projection::reconcile(&pool, false).await.unwrap();
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].expected.0, 1);
    projection::reconcile(&pool, true).await.unwrap();
    assert!(
        projection::reconcile(&pool, false)
            .await
            .unwrap()
            .is_empty()
    );
}
