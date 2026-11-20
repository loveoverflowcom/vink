//! SOC-FEED-01 / PUBTIME-01..03. The witness is the public pointer transaction.
use super::*;
use time::OffsetDateTime;
use vink_app::domain::ids::RevisionId;
use vink_app::domain::moderation::{AppliedEvaluation, EvaluatorOutcome, PolicyReasonCode};
use vink_app::persistence::evaluation::{JobOutcome, claim_job, finish_job};
use vink_app::persistence::{MIGRATOR, migrate};

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct Fact {
    provenance: String,
    source: String,
    first_published_at: Option<OffsetDateTime>,
    legacy_published_at: Option<OffsetDateTime>,
}

async fn fact(pool: &PgPool, work: Uuid) -> Fact {
    sqlx::query_as(
        "SELECT provenance, source, first_published_at, legacy_published_at
           FROM work_first_publications WHERE work_id = $1",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub(super) async fn exact_time(pool: &PgPool, work: Uuid) -> OffsetDateTime {
    let row = fact(pool, work).await;
    assert_eq!(row.provenance, "exact");
    assert_eq!(row.source, "pointer_insert");
    assert_eq!(row.legacy_published_at, None);
    row.first_published_at.unwrap()
}

pub(super) async fn dated_revision(pool: &PgPool, work: Uuid) -> Option<Uuid> {
    sqlx::query_scalar(
        "SELECT revision_id FROM publications_with_exact_first_time WHERE work_id=$1",
    )
    .bind(work)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn capture(vink: &Vink<PgStore>, work: WorkId, version: i64) -> RevisionId {
    vink.save_draft(
        author(),
        save(work, version, "Bản thử", doc(&[&["Một câu."]])),
    )
    .await
    .unwrap();
    vink.capture_snapshot(
        author(),
        work,
        &format!("capture-{version}"),
        version + 1,
        1,
    )
    .await
    .unwrap()
    .revision_id
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn only_a_committed_first_pointer_dates_the_work(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = vink.create_draft(author()).await.unwrap().work_id;
    let never = fact(&pool, work.as_uuid()).await;
    assert_eq!(never.provenance, "never_public");
    assert_eq!(never.first_published_at, None);
    let rejected = capture(&vink, work, 1).await;
    assert_eq!(fact(&pool, work.as_uuid()).await, never);
    vink.request_public(author(), work, "reject-291", rejected, 1)
        .await
        .unwrap();
    let worker = "first-public";
    let job = claim_job(&pool, worker, 60, None).await.unwrap().unwrap();
    let reason = PolicyReasonCode::SafetyPolicy;
    assert_eq!(
        finish_job(
            &pool,
            worker,
            &job,
            Some(EvaluatorOutcome::Reject(reason)),
            0
        )
        .await
        .unwrap(),
        JobOutcome::Applied(AppliedEvaluation::Rejected { reason })
    );
    assert_eq!(fact(&pool, work.as_uuid()).await, never);

    let revision = capture(&vink, work, 2).await;
    vink.request_public(author(), work, "approve-291", revision, 2)
        .await
        .unwrap();
    let job = claim_job(&pool, worker, 60, None).await.unwrap().unwrap();
    let approve = || finish_job(&pool, worker, &job, Some(EvaluatorOutcome::Approve), 0);
    // Force the last job write to fail AFTER the pointer and its trigger have run.
    sqlx::query("ALTER TABLE work_evaluation_jobs ADD CONSTRAINT fail_applied_291 CHECK (state <> 'applied') NOT VALID")
        .execute(&pool).await.unwrap();
    assert!(approve().await.is_err());
    assert_eq!(fact(&pool, work.as_uuid()).await, never);
    assert_eq!(
        vink.publication(work).await,
        Err(vink_app::application::ReadError::NotFound)
    );
    sqlx::query("ALTER TABLE work_evaluation_jobs DROP CONSTRAINT fail_applied_291")
        .execute(&pool)
        .await
        .unwrap();

    // In-flight pre-#291 pointer SQL competes with both worker deliveries. Its
    // provisional time is invisible and must roll back before a worker wins.
    let mut old = pool.begin().await.unwrap();
    let blocker = backend_pid(&mut old).await;
    sqlx::query(
        "INSERT INTO publications(work_id,revision_id,generation,published_at)
                 VALUES ($1,$2,3,now())",
    )
    .bind(work.as_uuid())
    .bind(revision.as_uuid())
    .execute(&mut *old)
    .await
    .unwrap();
    assert_eq!(fact(&pool, work.as_uuid()).await, never);
    let (before, (a, b)) = tokio::join!(
        async {
            wait_blocked_by(&pool, blocker).await;
            let waited: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
                .fetch_one(&pool)
                .await
                .unwrap();
            old.rollback().await.unwrap();
            waited
        },
        async { tokio::join!(approve(), approve()) }
    );
    let results = [a.unwrap(), b.unwrap()];
    assert!(results.contains(&JobOutcome::Applied(AppliedEvaluation::Published)));
    assert!(results.contains(&JobOutcome::Stale));
    let at = exact_time(&pool, work.as_uuid()).await;
    let after: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!((before..=after).contains(&at));
    assert_eq!(vink.publication(work).await.unwrap().revision_id, revision);

    vink.set_private(author(), work, "private-291", 3)
        .await
        .unwrap();
    assert_eq!(
        vink.publication(work).await,
        Err(vink_app::application::ReadError::NotFound)
    );
    assert_eq!(exact_time(&pool, work.as_uuid()).await, at);
    assert_eq!(dated_revision(&pool, work.as_uuid()).await, None);
    vink.request_public(author(), work, "republish-291", revision, 4)
        .await
        .unwrap();
    assert!(
        vink.request_public(author(), work, "republish-291", revision, 4)
            .await
            .unwrap()
            .replayed
    );
    assert_eq!(vink.publication(work).await.unwrap().revision_id, revision);
    assert_eq!(exact_time(&pool, work.as_uuid()).await, at);
    // The metadata binding exposed to #296 follows the current immutable revision.
    let metadata: Uuid =
        sqlx::query_scalar("SELECT metadata_snapshot_id FROM revisions WHERE id=$1")
            .bind(revision.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    let projection: (Uuid, Uuid, OffsetDateTime) = sqlx::query_as(
        "SELECT revision_id, metadata_snapshot_id, first_published_at
           FROM publications_with_exact_first_time WHERE work_id=$1",
    )
    .bind(work.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(projection, (revision.as_uuid(), metadata, at));
}

const MIGRATION: &str = include_str!("../../migrations/20261123000000_first_publication.sql");

async fn counts(pool: &PgPool) -> (i64, i64, i64, i64) {
    sqlx::query_as("SELECT count(*) FILTER (WHERE provenance='exact'),
                           count(*) FILTER (WHERE provenance='legacy'),
                           count(*) FILTER (WHERE provenance='unknown'),
                           count(*) FILTER (WHERE provenance='never_public') FROM work_first_publications")
        .fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = false)]
async fn backfill_and_old_writer_preserve_the_source_across_republication(pool: PgPool) {
    MIGRATOR
        .run_to(CANONICAL_DOCUMENT_MIGRATION, &pool)
        .await
        .unwrap();
    let vink = seeded(&pool).await;
    let legacy = sample_work();
    // This stage has the old schema. Use its public pointer as the migration oracle.
    let pointer_time: OffsetDateTime =
        sqlx::query_scalar("SELECT published_at FROM publications WHERE work_id = $1")
            .bind(legacy.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    let withdrawn = vink.create_draft(author()).await.unwrap().work_id;
    let withdrawn_revision = capture(&vink, withdrawn, 1).await;
    vink.request_public(author(), withdrawn, "submit-old", withdrawn_revision, 1)
        .await
        .unwrap();
    let job = claim_job(&pool, "old", 60, None).await.unwrap().unwrap();
    finish_job(&pool, "old", &job, Some(EvaluatorOutcome::Approve), 0)
        .await
        .unwrap();
    vink.set_private(author(), withdrawn, "withdraw-old", 2)
        .await
        .unwrap();
    let uncertain = vink.create_draft(author()).await.unwrap().work_id;
    capture(&vink, uncertain, 1).await;
    let draft = vink.create_draft(author()).await.unwrap().work_id;

    let mut writer = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE publications IN ROW EXCLUSIVE MODE")
        .execute(&mut *writer)
        .await
        .unwrap();
    assert!(
        migrate(&pool)
            .await
            .unwrap_err()
            .to_string()
            .contains("could not obtain lock")
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT to_regclass('work_first_publications') IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    writer.commit().await.unwrap();
    assert_eq!(
        migrate(&pool).await.unwrap().applied,
        [
            FIRST_PUBLICATION_MIGRATION,
            HANDLE_MIGRATION,
            PROFILE_FIELDS_MIGRATION,
            PUBLIC_ATTRIBUTION_MIGRATION,
            AUTHOR_FOLLOW_MIGRATION,
            NOTIFICATION_OUTBOX_MIGRATION
        ]
    );
    // A binary embedding only the prior ledger is not ready after an additive migration.
    let prior = sqlx::migrate::Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|m| m.version <= CANONICAL_DOCUMENT_MIGRATION)
            .cloned()
            .collect(),
    );
    assert_eq!(
        vink_store_postgres::schema_state(&pool, &prior)
            .await
            .unwrap(),
        SchemaState::Diverged(vink_store_postgres::Divergence::Unknown {
            version: FIRST_PUBLICATION_MIGRATION
        })
    );
    assert_eq!(counts(&pool).await, (0, 1, 2, 1));
    let old = fact(&pool, legacy.as_uuid()).await;
    assert_eq!(old.provenance, "legacy");
    assert_eq!(old.source, "existing_pointer");
    assert_eq!(old.legacy_published_at, Some(pointer_time));
    assert_eq!(old.first_published_at, None);
    let hidden = fact(&pool, withdrawn.as_uuid()).await;
    assert_eq!(hidden.provenance, "unknown");
    assert_eq!(hidden.source, "withdrawn_without_time");
    assert_eq!(
        (hidden.first_published_at, hidden.legacy_published_at),
        (None, None)
    );
    assert_eq!(
        fact(&pool, uncertain.as_uuid()).await.source,
        "incomplete_history"
    );
    assert_eq!(fact(&pool, draft.as_uuid()).await.source, "no_revision");

    vink.set_private(author(), legacy, "hide-seed", 2)
        .await
        .unwrap();
    dev_seed::apply(&pool).await.unwrap();
    assert!(vink.publication(legacy).await.is_err());
    vink.request_public(
        author(),
        legacy,
        "restore-seed",
        RevisionId::from_uuid(dev_seed::SAMPLE_REVISION),
        3,
    )
    .await
    .unwrap();
    vink.request_public(author(), withdrawn, "restore-old", withdrawn_revision, 3)
        .await
        .unwrap();
    assert_eq!(
        vink.publication(withdrawn).await.unwrap().revision_id,
        withdrawn_revision
    );
    assert_eq!(fact(&pool, legacy.as_uuid()).await, old);
    assert_eq!(fact(&pool, withdrawn.as_uuid()).await, hidden);

    // A pre-#291 writer names just the old pointer columns and even supplies a stale timestamp.
    let revision = capture(&vink, draft, 1).await;
    sqlx::query(
        "INSERT INTO publications(work_id,revision_id,generation,published_at)
                 VALUES ($1,$2,2,'2001-01-01 00:00:00+00')",
    )
    .bind(draft.as_uuid())
    .bind(revision.as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    let at = exact_time(&pool, draft.as_uuid()).await;
    assert!(at > vink.publication(draft).await.unwrap().published_at);
    for work in [legacy, withdrawn, draft] {
        for mutation in [
            "UPDATE work_first_publications SET provenance='exact', source='pointer_insert',
             first_published_at=now(), legacy_published_at=NULL WHERE work_id=$1",
            "DELETE FROM work_first_publications WHERE work_id=$1",
        ] {
            let err = sqlx::query(mutation)
                .bind(work.as_uuid())
                .execute(&pool)
                .await
                .unwrap_err();
            assert_eq!(
                err.as_database_error().unwrap().code().as_deref(),
                Some("23000")
            );
        }
    }
    // Replay the actual backfill after pointers/known history changed, then the migration runner.
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    assert!(migrate(&pool).await.unwrap().applied.is_empty());
    assert_eq!(counts(&pool).await, (1, 1, 2, 0));
    let candidates: Vec<(Uuid, Uuid)> =
        sqlx::query_as("SELECT work_id, revision_id FROM publications_with_exact_first_time")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(candidates, [(draft.as_uuid(), revision.as_uuid())]);
    assert_eq!(exact_time(&pool, draft.as_uuid()).await, at);
    assert_eq!(fact(&pool, legacy.as_uuid()).await, old);
    assert_eq!(fact(&pool, withdrawn.as_uuid()).await, hidden);
}
