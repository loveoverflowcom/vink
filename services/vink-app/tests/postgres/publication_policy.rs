//! PUB-06 on PostgreSQL: repeat review of the same immutable revision.
use super::*;
use vink_app::application::publication::AuthorPublicationError;
use vink_app::domain::ids::RevisionId;
use vink_app::domain::moderation::{EvaluationRefusal, EvaluatorOutcome};
use vink_app::domain::publication::Moderation;
use vink_app::persistence::evaluation::{JobOutcome, claim_job, finish_job};

const POLICY_MIGRATIONS: [i64; 2] = [20261121120000, 20261121120100];
const BACKOUT: &str = concat!(
    include_str!("../../migrations-rollback/20261121120100_rereview_history.down.sql"),
    "\n",
    include_str!("../../migrations-rollback/20261121120000_policy_rereview.down.sql"),
);

async fn policy_ledger(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE version BETWEEN $1 AND $2 ORDER BY version",
    )
    .bind(POLICY_MIGRATIONS[0])
    .bind(POLICY_MIGRATIONS[1])
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn submit(vink: &Vink<PgStore>) -> (WorkId, RevisionId) {
    let work = vink.create_draft(author()).await.unwrap().work_id;
    vink.save_draft(
        author(),
        save(work, 1, "Theo policy", doc(&[&["Bản cần kiểm lại"]])),
    )
    .await
    .unwrap();
    let revision = vink
        .capture_snapshot(author(), work, "policy-snapshot", 2, 1)
        .await
        .unwrap()
        .revision_id;
    vink.request_public(author(), work, "policy-submit", revision, 1)
        .await
        .unwrap();
    (work, revision)
}

async fn bump(pool: &PgPool, work: WorkId) {
    assert_eq!(
        sqlx::query(
            "UPDATE work_publications SET policy_version = policy_version + 1 WHERE work_id=$1"
        )
        .bind(work.as_uuid())
        .execute(pool)
        .await
        .unwrap()
        .rows_affected(),
        1
    );
}

async fn history(pool: &PgPool, revision: RevisionId) -> Vec<(i64, i64, String, Option<String>)> {
    sqlx::query_as(
        "SELECT decision_version, policy_version, moderation, provenance_kind
        FROM work_revision_decision_history WHERE revision_id=$1 ORDER BY decision_version",
    )
    .bind(revision.as_uuid())
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn state_observes_policy_rereview_and_private_fences_late_approval(pool: PgPool) {
    let vink = seeded(&pool).await;
    let (work, revision) = submit(&vink).await;
    let worker = "state-policy-230";
    let claim = claim_job(&pool, worker, 60, None).await.unwrap().unwrap();
    assert!(matches!(
        finish_job(&pool, worker, &claim, Some(EvaluatorOutcome::Approve), 0)
            .await
            .unwrap(),
        JobOutcome::Applied(_)
    ));
    vink.set_private(author(), work, "policy-private-230", 2)
        .await
        .unwrap();
    bump(&pool, work).await;
    let prior_audit = || {
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(h) FROM work_revision_decision_history h WHERE revision_id=$1 AND decision_version=2",
        )
        .bind(revision.as_uuid())
        .fetch_one(&pool)
    };
    let approved_audit = prior_audit().await.unwrap();
    assert_eq!(approved_audit["evaluation_id"], json!(claim.id));
    vink.request_public(author(), work, "policy-rereview-230", revision, 3)
        .await
        .unwrap();
    assert_eq!(prior_audit().await.unwrap(), approved_audit);
    let replay = vink
        .request_public(author(), work, "policy-rereview-230", revision, 3)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.generation.get(), 4);
    assert!(matches!(
        vink.request_public(author(), work, "stale-generation", revision, 3)
            .await,
        Err(AuthorPublicationError::Stale {
            current_generation: 4
        })
    ));
    let expected_history = vec![
        (1, 1, "pending".into(), None),
        (2, 1, "approved".into(), Some("ai_approved".into())),
        (3, 2, "pending".into(), None),
    ];
    assert_eq!(history(&pool, revision).await, expected_history);
    let state = vink
        .load_author_publication_state(author(), work)
        .await
        .unwrap();
    assert!(state.publication.facts().pointer.is_none());
    assert_eq!(
        state.publication.facts().revisions[0].moderation,
        Moderation::Pending
    );
    let claim = claim_job(&pool, worker, 60, None).await.unwrap().unwrap();
    assert_eq!(
        (claim.work_id, claim.revision_id, claim.policy_version),
        (work.as_uuid(), revision.as_uuid(), 2)
    );
    assert_eq!(
        claim.content_digest,
        state.moderation.facts().revisions[0].digest.as_str()
    );
    // Policy changes again after the claim; an old policy result cannot approve.
    bump(&pool, work).await;
    assert_eq!(
        finish_job(&pool, worker, &claim, Some(EvaluatorOutcome::Approve), 0)
            .await
            .unwrap(),
        JobOutcome::Refused(EvaluationRefusal::StalePolicy)
    );
    // A fresh policy-3 job is then withdrawn by the author before its result returns.
    vink.request_public(author(), work, "policy-fresh-230", revision, 4)
        .await
        .unwrap();
    let late = claim_job(&pool, worker, 60, None).await.unwrap().unwrap();
    assert_eq!(late.policy_version, 3);
    vink.set_private(author(), work, "policy-final-private-230", 5)
        .await
        .unwrap();
    assert_eq!(
        finish_job(&pool, worker, &late, Some(EvaluatorOutcome::Approve), 0)
            .await
            .unwrap(),
        JobOutcome::Stale
    );
    let state = vink
        .load_author_publication_state(author(), work)
        .await
        .unwrap();
    assert_eq!(
        state.publication.facts().audience,
        vink_app::domain::publication::Audience::Private
    );
    assert!(state.publication.facts().pointer.is_none());
    assert!(
        state
            .moderation
            .facts()
            .jobs
            .iter()
            .all(|j| j.state != vink_app::domain::moderation::JobState::Open)
    );
    assert_eq!(history(&pool, revision).await, expected_history);
}

/// Include timestamps and source IDs: a rewritten decision/history row must be visible here.
async fn stored_rows(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        "SELECT jsonb_build_array(
           (SELECT jsonb_agg(m ORDER BY work_id, revision_id) FROM work_revision_moderation m),
           (SELECT jsonb_agg(h ORDER BY id) FROM work_revision_decision_history h),
           (SELECT jsonb_agg(j ORDER BY id) FROM work_evaluation_jobs j),
           (SELECT jsonb_agg(c ORDER BY id) FROM moderation_cases c),
           (SELECT jsonb_agg(r ORDER BY work_id, operation, command_key) FROM work_publication_command_receipts r),
           (SELECT jsonb_agg(w ORDER BY work_id) FROM work_publications w),
           (SELECT jsonb_agg(p ORDER BY work_id) FROM publications p),
           (SELECT jsonb_agg(r ORDER BY id) FROM revisions r))",
    ).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn policy_rereview_conflicting_audit_rolls_back_the_command(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    let revision = RevisionId::from_uuid(dev_seed::SAMPLE_REVISION);
    vink.set_private(author(), work, "audit-private", 2)
        .await
        .unwrap();
    bump(&pool, work).await;
    assert!(history(&pool, revision).await.is_empty());

    // Simulate inconsistent imported evidence at the old or incoming version.
    // Neither version's existing evidence may be overwritten to finish a request.
    for version in [2_i64, 3] {
        let fixture = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO work_revision_decision_history
            (id,work_id,revision_id,decision_version,moderation,policy_version,provenance_kind)
            VALUES ($1,$2,$3,$4,'approved',1,'ai_approved')",
        )
        .bind(fixture)
        .bind(work.as_uuid())
        .bind(revision.as_uuid())
        .bind(version)
        .execute(&pool)
        .await
        .unwrap();
        let before = stored_rows(&pool).await;
        assert!(
            matches!(
                vink.request_public(author(), work, "audit-rereview", revision, 3)
                    .await,
                Err(AuthorPublicationError::Internal)
            ),
            "conflicting history v{version}"
        );
        assert_eq!(stored_rows(&pool).await, before);
        sqlx::query("DELETE FROM work_revision_decision_history WHERE id=$1")
            .bind(fixture)
            .execute(&pool)
            .await
            .unwrap();
    }
    // A failed command left its key available. Store the legacy approval exactly
    // once before committing a new ticket and pending version.
    let result = vink
        .request_public(author(), work, "audit-rereview", revision, 3)
        .await
        .unwrap();
    assert!(result.ticket.is_some());
    assert_eq!(
        history(&pool, revision).await,
        vec![
            (2, 1, "approved".into(), Some("legacy_bootstrap".into())),
            (3, 2, "pending".into(), None),
        ]
    );
}

#[sqlx::test(migrations = false)]
async fn policy_upgrade_and_backout_keep_legacy_history_before_canonical_cutover(pool: PgPool) {
    use vink_app::persistence::{MIGRATOR, migrate};

    MIGRATOR.run_to(20261121000000, &pool).await.unwrap();
    seed(&pool).await;
    sqlx::raw_sql(include_str!("../support/pre_cutover_policy.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let before = stored_rows(&pool).await;
    for (index, migration) in POLICY_MIGRATIONS.iter().enumerate() {
        MIGRATOR.run_to(*migration, &pool).await.unwrap();
        assert_eq!(policy_ledger(&pool).await, POLICY_MIGRATIONS[..=index]);
        assert_eq!(stored_rows(&pool).await, before);
    }
    assert!(
        migrate(&pool)
            .await
            .unwrap_err()
            .to_string()
            .contains("cutover blocked")
    );
    assert!(before_cutover(&pool).await);
    assert_eq!(stored_rows(&pool).await, before);
    sqlx::raw_sql(BACKOUT).execute(&pool).await.unwrap();
    assert_eq!(stored_rows(&pool).await, before);
    assert!(policy_ledger(&pool).await.is_empty());
    MIGRATOR.run_to(POLICY_MIGRATIONS[1], &pool).await.unwrap();

    // Represent an old writer's next decision without opening its retired document in the
    // canonical application. Both traces are appended; the revision and its digest stay fixed.
    let revision: Uuid = sqlx::query_scalar(
        "SELECT revision_id FROM work_revision_moderation WHERE provenance_kind='ai_approved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE work_publications SET policy_version=2, generation=3, intent_generation=3, intent_revision_id=$1 WHERE work_id=$2")
        .bind(revision).bind(dev_seed::SAMPLE_WORK).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE work_revision_moderation SET decision_version=3, policy_version=2, moderation='pending', provenance_kind=NULL WHERE revision_id=$1")
        .bind(revision).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO work_revision_decision_history(id,work_id,revision_id,decision_version,moderation,policy_version)
                 VALUES(gen_random_uuid(),$1,$2,3,'pending',2)")
        .bind(dev_seed::SAMPLE_WORK).bind(revision).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let pending = stored_rows(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let refusal = sqlx::raw_sql(BACKOUT)
        .execute(&mut *conn)
        .await
        .unwrap_err();
    assert_eq!(
        refusal.as_database_error().unwrap().constraint(),
        Some("work_revision_moderation_pending_check")
    );
    sqlx::query("ROLLBACK").execute(&mut *conn).await.unwrap();
    drop(conn);
    assert_eq!(stored_rows(&pool).await, pending);
    assert_eq!(policy_ledger(&pool).await, POLICY_MIGRATIONS);

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE work_revision_moderation SET decision_version=4, moderation='approved', provenance_kind='ai_approved' WHERE revision_id=$1")
        .bind(revision).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO work_revision_decision_history(id,work_id,revision_id,decision_version,moderation,policy_version,provenance_kind)
                 VALUES(gen_random_uuid(),$1,$2,4,'approved',2,'ai_approved')")
        .bind(dev_seed::SAMPLE_WORK).bind(revision).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let resolved = stored_rows(&pool).await;
    assert_eq!(
        history(&pool, RevisionId::from_uuid(revision)).await.len(),
        3
    );
    assert_eq!(
        resolved[7], before[7],
        "immutable revision rows and digests"
    );
    sqlx::raw_sql(BACKOUT).execute(&pool).await.unwrap();
    assert!(policy_ledger(&pool).await.is_empty());
    assert_eq!(stored_rows(&pool).await, resolved);
    MIGRATOR.run_to(POLICY_MIGRATIONS[1], &pool).await.unwrap();
    assert_eq!(stored_rows(&pool).await, resolved);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn empty_schema_rolls_back_canonical_before_the_two_policy_migrations(pool: PgPool) {
    super::canonical_document::run_backout(&pool).await.unwrap();
    assert!(before_cutover(&pool).await);
    assert_eq!(policy_ledger(&pool).await, POLICY_MIGRATIONS);
    sqlx::raw_sql(BACKOUT).execute(&pool).await.unwrap();
    assert!(policy_ledger(&pool).await.is_empty());
    assert_eq!(
        vink_app::persistence::migrate(&pool).await.unwrap().applied,
        [
            POLICY_MIGRATIONS[0],
            POLICY_MIGRATIONS[1],
            CANONICAL_DOCUMENT_MIGRATION
        ]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn canonical_policy_rereview_is_atomic_and_publishes_the_captured_digest(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    let revision = RevisionId::from_uuid(dev_seed::SAMPLE_REVISION);
    assert!(history(&pool, revision).await.is_empty());
    vink.set_private(author(), work, "legacy-private", 2)
        .await
        .unwrap();
    bump(&pool, work).await;
    // Fail at the final write, after tentative intent, prior approval history, decision and job.
    sqlx::query("ALTER TABLE work_publication_command_receipts ADD CONSTRAINT test_stop_receipt CHECK (command_key <> 'legacy-rereview')")
        .execute(&pool).await.unwrap();
    let before_request = stored_rows(&pool).await;
    assert!(matches!(
        vink.request_public(author(), work, "legacy-rereview", revision, 3)
            .await,
        Err(AuthorPublicationError::Internal)
    ));
    assert_eq!(stored_rows(&pool).await, before_request);
    sqlx::query("ALTER TABLE work_publication_command_receipts DROP CONSTRAINT test_stop_receipt")
        .execute(&pool)
        .await
        .unwrap();
    let accepted = vink
        .request_public(author(), work, "legacy-rereview", revision, 3)
        .await
        .unwrap();
    let ticket = accepted.ticket.unwrap();
    assert_eq!(ticket.target.revision(), revision);
    assert_eq!(ticket.policy.get(), 2);
    assert_eq!(
        history(&pool, revision).await,
        vec![
            (2, 1, "approved".into(), Some("legacy_bootstrap".into())),
            (3, 2, "pending".into(), None),
        ]
    );
    let after_request = stored_rows(&pool).await;
    assert!(
        vink.request_public(author(), work, "legacy-rereview", revision, 3)
            .await
            .unwrap()
            .replayed
    );
    assert_eq!(stored_rows(&pool).await, after_request);

    // Conclude the queued canonical revision through the real worker.
    let claim = claim_job(&pool, "current", 60, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.id, ticket.id.as_uuid());
    assert_eq!(
        finish_job(&pool, "current", &claim, Some(EvaluatorOutcome::Approve), 0)
            .await
            .unwrap(),
        JobOutcome::Applied(vink_app::domain::moderation::AppliedEvaluation::Published)
    );
    let public = vink.publication(work).await.unwrap();
    assert_eq!(public.revision_id, revision);
    assert_eq!(
        RevisionContent::new(public.content)
            .unwrap()
            .hash()
            .as_str(),
        claim.content_digest
    );
    assert_eq!(
        history(&pool, revision).await,
        vec![
            (2, 1, "approved".into(), Some("legacy_bootstrap".into())),
            (3, 2, "pending".into(), None),
            (4, 2, "approved".into(), Some("ai_approved".into())),
        ]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn direct_sql_rejects_impossible_pending_rows_and_predecessors(pool: PgPool) {
    let vink = seeded(&pool).await;
    let (work, target) = submit(&vink).await;
    let legacy = dev_seed::SAMPLE_REVISION;
    let check = "work_revision_moderation_pending_check";
    let transition = "work_revision_moderation_pending_transition";
    for (revision, assignment, constraint) in [
        (
            target.as_uuid(),
            "decision_version=0",
            "work_revision_moderation_decision_version_check",
        ),
        (
            target.as_uuid(),
            "decision_version=2, policy_version=2",
            check,
        ),
        (target.as_uuid(), "decision_version=3", check),
        (
            target.as_uuid(),
            "decision_version=3, policy_version=2",
            transition,
        ),
        (target.as_uuid(), "provenance_kind='ai_approved'", check),
        (target.as_uuid(), "admin_verdict='approve'", check),
        (
            target.as_uuid(),
            "admin_actor=(SELECT owner_id FROM works LIMIT 1)",
            check,
        ),
        (target.as_uuid(), "admin_reason='approved'", check),
        (target.as_uuid(), "policy_reason_code='harassment'", check),
        (target.as_uuid(), "needs_admin_cause='uncertain'", check),
        (
            target.as_uuid(),
            "moderator_grant_id=gen_random_uuid()",
            check,
        ),
        (legacy, "provenance_kind=NULL", check),
        (
            legacy,
            "provenance_kind='admin',admin_actor=(SELECT owner_id FROM works LIMIT 1),admin_reason='ok',admin_verdict=NULL",
            check,
        ),
        (
            legacy,
            "moderation='pending', provenance_kind=NULL, decision_version=1",
            transition,
        ),
        (
            legacy,
            "moderation='pending', provenance_kind=NULL, decision_version=4, policy_version=2",
            transition,
        ),
        (
            legacy,
            "moderation='pending', provenance_kind=NULL, decision_version=3, policy_version=2",
            transition,
        ),
    ] {
        let sql = sqlx::AssertSqlSafe(format!(
            "UPDATE work_revision_moderation SET {assignment} WHERE revision_id=$1"
        ));
        let error = sqlx::query(sql)
            .bind(revision)
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().constraint(),
            Some(constraint),
            "{assignment}: {error}"
        );
    }

    // Each binder exists, but this revision belongs to another work.
    let error = sqlx::query(
        "INSERT INTO work_revision_moderation (work_id,revision_id,decision_version,moderation)
        VALUES ($1,$2,1,'pending')",
    )
    .bind(work.as_uuid())
    .bind(legacy)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some("work_revision_moderation_revision_fkey")
    );

    // A real snapshot with no decision cannot begin life at an invented review version.
    let new_revision = vink
        .capture_snapshot(author(), work, "another-snapshot", 2, 1)
        .await
        .unwrap()
        .revision_id;
    let error = sqlx::query("INSERT INTO work_revision_moderation (work_id,revision_id,decision_version,policy_version,moderation)
        VALUES ($1,$2,3,2,'pending')").bind(work.as_uuid()).bind(new_revision.as_uuid()).execute(&pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some(transition)
    );

    // The exact work, immutable revision, next version and new policy all match.
    // A human approval still cannot be turned into an evaluator candidate.
    sqlx::query(
        "UPDATE work_revision_moderation SET moderation='approved',decision_version=2,
        provenance_kind='admin',admin_actor=$2,admin_verdict='approve',admin_reason='Đã duyệt'
        WHERE revision_id=$1",
    )
    .bind(target.as_uuid())
    .bind(dev_seed::OTHER_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    bump(&pool, work).await;
    let error = sqlx::query(
        "UPDATE work_revision_moderation SET moderation='pending',decision_version=3,
        policy_version=2,provenance_kind=NULL,admin_actor=NULL,admin_verdict=NULL,admin_reason=NULL
        WHERE revision_id=$1",
    )
    .bind(target.as_uuid())
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some(transition)
    );

    // Leaving a legitimate v3 re-review also consumes exactly one version.
    vink.set_private(author(), sample_work(), "scope-private", 2)
        .await
        .unwrap();
    bump(&pool, sample_work()).await;
    vink.request_public(
        author(),
        sample_work(),
        "scope-rereview",
        RevisionId::from_uuid(legacy),
        3,
    )
    .await
    .unwrap();
    for version in [2_i64, 3, 5] {
        let error = sqlx::query(
            "UPDATE work_revision_moderation SET moderation='approved',
            decision_version=$2,provenance_kind='ai_approved' WHERE revision_id=$1",
        )
        .bind(legacy)
        .bind(version)
        .execute(&pool)
        .await
        .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().constraint(),
            Some(transition)
        );
    }

    // Neither a bootstrap import nor a request for human re-review is a new verdict.
    for (state, provenance) in [
        ("approved", "legacy_bootstrap"),
        ("needs_admin", "policy_rereview"),
    ] {
        let error = sqlx::query(
            "UPDATE work_revision_moderation SET moderation=$2,
            decision_version=4, provenance_kind=$3 WHERE revision_id=$1",
        )
        .bind(legacy)
        .bind(state)
        .bind(provenance)
        .execute(&pool)
        .await
        .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().constraint(),
            Some(transition)
        );
    }

    bump(&pool, sample_work()).await;
    let stale = sqlx::query(
        "UPDATE work_revision_moderation SET moderation='approved',
        decision_version=4,provenance_kind='ai_approved' WHERE revision_id=$1",
    )
    .bind(legacy)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        stale.as_database_error().unwrap().constraint(),
        Some(transition)
    );
}
