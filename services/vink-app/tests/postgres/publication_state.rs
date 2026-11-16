//! #230: runtime body exclusion and integrity at the boundaries that consume content.
use super::*;
use vink_app::application::ports::{AuthorPublicationStore, StoreError};
use vink_app::application::publication::AuthorPublicationError;

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn state_reads_all_history_without_permission_to_read_document(pool: PgPool) {
    let _ = seeded(&pool).await;
    // Legacy NULL metadata, >100 revisions, tied timestamps. IDs are the deterministic tie-break.
    sqlx::query(
        "INSERT INTO revisions(id,work_id,title,document,content_hash,created_by,created_at)
        SELECT md5(n::text)::uuid,work_id,title,document,content_hash,created_by,created_at
        FROM revisions CROSS JOIN generate_series(1,105) n WHERE id=$1",
    )
    .bind(dev_seed::SAMPLE_REVISION)
    .execute(&pool)
    .await
    .unwrap();
    let role = format!("state_230_{}", Uuid::now_v7().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE {role} NOLOGIN;
         GRANT USAGE ON SCHEMA public TO {role};
         GRANT SELECT ON ALL TABLES IN SCHEMA public TO {role};
         REVOKE SELECT ON revisions FROM {role};
         GRANT SELECT (id,work_id,title,content_hash,metadata_snapshot_id,created_at) ON revisions TO {role};"
    ))).execute(&pool).await.unwrap();
    let selected_role = role.clone();
    let restricted = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let sql = format!("SET ROLE {selected_role}");
            Box::pin(async move {
                sqlx::query(sqlx::AssertSqlSafe(sql)).execute(conn).await?;
                Ok(())
            })
        })
        .connect_with(pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    assert!(
        sqlx::query("SELECT document FROM revisions LIMIT 1")
            .fetch_one(&restricted)
            .await
            .is_err()
    );
    let store = PgStore::new(restricted.clone());
    let state = store
        .load_author_publication_state(author().unwrap(), sample_work())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.publication.facts().revisions.len(), 106);
    let expected: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM revisions WHERE work_id=$1 ORDER BY created_at,id")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        state
            .revision_metadata
            .iter()
            .map(|r| r.0.as_uuid())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        state
            .revision_metadata
            .iter()
            .filter(|r| r.2.is_none())
            .count(),
        106
    );
    for (actor, work) in [
        (other().unwrap(), sample_work()),
        (author().unwrap(), WorkId::from_uuid(Uuid::now_v7())),
    ] {
        assert!(
            store
                .load_author_publication_state(actor, work)
                .await
                .unwrap()
                .is_none()
        );
    }
    restricted.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP OWNED BY {role}; DROP ROLE {role};"
    )))
    .execute(&pool)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn state_is_not_a_body_integrity_or_publication_capability(pool: PgPool) {
    let app = app(&pool).await;
    let vink = Vink::new(PgStore::new(pool.clone()));
    let path = format!("/api/v1/works/{}/publication", dev_seed::SAMPLE_WORK);
    let (_, _, original) = call(&app, "GET", &path, None, None, None).await;
    assert!(json(&original).get("document").is_some());
    // Simulate storage corruption beyond the immutable-row guard in this disposable database.
    sqlx::query("ALTER TABLE revisions DISABLE TRIGGER revisions_are_immutable")
        .execute(&pool)
        .await
        .unwrap();
    for corrupt in [
        json!({"schema_version":1,"blocks":[{"lines":["corrupt body"]}]}),
        json!({"schema_version":999,"blocks":[]}),
    ] {
        sqlx::query("UPDATE revisions SET document=$1 WHERE id=$2")
            .bind(sqlx::types::Json(corrupt))
            .bind(dev_seed::SAMPLE_REVISION)
            .execute(&pool)
            .await
            .unwrap();
        // Summary reports stored bindings only; it never deserializes even an invalid document.
        let summary = vink
            .load_author_publication_state(author(), sample_work())
            .await
            .unwrap();
        assert_eq!(
            summary
                .publication
                .facts()
                .pointer
                .unwrap()
                .revision
                .as_uuid(),
            dev_seed::SAMPLE_REVISION
        );
        let outcome = vink
            .request_public(
                author(),
                sample_work(),
                "corrupt-request-230",
                summary.publication.facts().pointer.unwrap().revision,
                2,
            )
            .await;
        assert!(matches!(outcome, Err(AuthorPublicationError::Internal)));
        assert_eq!(
            call(&app, "GET", &path, None, None, None).await.0,
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            call(
                &app,
                "GET",
                &format!("/api/v1/works/{}/reader", dev_seed::SAMPLE_WORK),
                None,
                None,
                None
            )
            .await
            .0,
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let conditional = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header(header::IF_NONE_MATCH, "*")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            conditional.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "corruption must be checked before even wildcard 304"
        );
        let generation: i64 =
            sqlx::query_scalar("SELECT generation FROM work_publications WHERE work_id=$1")
                .bind(dev_seed::SAMPLE_WORK)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(generation, 2, "failed command rolls back");
        let receipts:i64=sqlx::query_scalar("SELECT count(*) FROM work_publication_command_receipts WHERE command_key='corrupt-request-230'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(receipts, 0);
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn state_rejects_job_digest_and_database_rejects_cross_work_bindings(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = vink.create_draft(author()).await.unwrap().work_id;
    vink.save_draft(
        author(),
        save(work, 1, "Bản mới", doc(&[&["Giữ đúng bản"]])),
    )
    .await
    .unwrap();
    let captured = vink
        .capture_snapshot(author(), work, "state-snapshot-230", 2, 1)
        .await
        .unwrap();
    vink.request_public(author(), work, "state-public-230", captured.revision_id, 1)
        .await
        .unwrap();
    sqlx::query("UPDATE work_evaluation_jobs SET content_digest=repeat('0',64) WHERE work_id=$1")
        .bind(work.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        PgStore::new(pool.clone())
            .load_author_publication_state(author().unwrap(), work)
            .await,
        Err(StoreError::Integrity(_))
    ));
    // Each statement rolls back independently on the existing composite foreign key.
    for sql in [
        "UPDATE work_publications SET intent_revision_id=$1,pointer_revision_id=NULL,pointer_intent_generation=NULL WHERE work_id=$2",
        "UPDATE publications SET revision_id=$1 WHERE work_id=$2",
    ] {
        let result = sqlx::query(sql)
            .bind(captured.revision_id.as_uuid())
            .bind(dev_seed::SAMPLE_WORK)
            .execute(&pool)
            .await;
        assert_eq!(
            result
                .unwrap_err()
                .as_database_error()
                .unwrap()
                .code()
                .as_deref(),
            Some("23503")
        );
    }
    sqlx::query("ALTER TABLE revisions DISABLE TRIGGER revisions_are_immutable")
        .execute(&pool)
        .await
        .unwrap();
    let other_metadata: Uuid =
        sqlx::query_scalar("SELECT metadata_snapshot_id FROM revisions WHERE id=$1")
            .bind(captured.revision_id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    let result = sqlx::query("UPDATE revisions SET metadata_snapshot_id=$1 WHERE id=$2")
        .bind(other_metadata)
        .bind(dev_seed::SAMPLE_REVISION)
        .execute(&pool)
        .await;
    assert_eq!(
        result
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23503")
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn state_reads_one_snapshot_across_a_concurrent_moderation_commit(pool: PgPool) {
    let vink = seeded(&pool).await;
    let mut writer = pool.begin().await.unwrap();
    let pid = backend_pid(&mut writer).await;
    sqlx::query("LOCK TABLE revisions IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let reader_pool = pool.clone();
    let reading = tokio::spawn(async move {
        Vink::new(PgStore::new(reader_pool))
            .load_author_publication_state(author(), sample_work())
            .await
    });
    // PubRow has been read; the revision query is blocked at a known boundary.
    wait_blocked_by(&pool, pid).await;
    sqlx::query(
        "UPDATE work_publications SET pointer_revision_id=NULL,pointer_intent_generation=NULL,
        review_withdrawn_revision_id=$1,review_withdrawn_intent_generation=2 WHERE work_id=$2",
    )
    .bind(dev_seed::SAMPLE_REVISION)
    .bind(dev_seed::SAMPLE_WORK)
    .execute(&mut *writer)
    .await
    .unwrap();
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE work_revision_moderation SET moderation='rejected',decision_version=3,
        provenance_kind='ai_rejected',policy_reason_code='safety_policy' WHERE work_id=$1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .execute(&mut *writer)
    .await
    .unwrap();
    writer.commit().await.unwrap();
    let before = reading.await.unwrap().unwrap();
    assert!(before.publication.facts().pointer.is_some());
    assert_eq!(
        before.publication.facts().revisions[0].moderation,
        vink_app::domain::publication::Moderation::Approved
    );
    let after = vink
        .load_author_publication_state(author(), sample_work())
        .await
        .unwrap();
    assert!(after.publication.facts().pointer.is_none());
    assert!(after.publication.facts().review_withdrawn.is_some());
    assert_eq!(
        after.publication.facts().revisions[0].moderation,
        vink_app::domain::publication::Moderation::Rejected
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn state_observes_policy_rereview_and_private_fences_late_approval(pool: PgPool) {
    use vink_app::domain::moderation::{EvaluationRefusal, EvaluatorOutcome};
    use vink_app::domain::publication::Moderation;
    use vink_app::persistence::evaluation::{JobOutcome, claim_job, finish_job};
    let vink = seeded(&pool).await;
    let work = vink.create_draft(author()).await.unwrap().work_id;
    vink.save_draft(
        author(),
        save(work, 1, "Theo policy", doc(&[&["Bản cần kiểm lại"]])),
    )
    .await
    .unwrap();
    let revision = vink
        .capture_snapshot(author(), work, "policy-snapshot-230", 2, 1)
        .await
        .unwrap()
        .revision_id;
    vink.request_public(author(), work, "policy-public-230", revision, 1)
        .await
        .unwrap();
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
    sqlx::query("UPDATE work_publications SET policy_version=2 WHERE work_id=$1")
        .bind(work.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    vink.request_public(author(), work, "policy-rereview-230", revision, 3)
        .await
        .unwrap();
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
    assert_eq!(claim.policy_version, 2);
    // Policy changes again after the claim; an old policy result cannot approve.
    sqlx::query("UPDATE work_publications SET policy_version=3 WHERE work_id=$1")
        .bind(work.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
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
}
