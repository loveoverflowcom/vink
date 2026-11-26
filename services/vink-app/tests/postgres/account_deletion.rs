use super::{app, author, call, dev_seed, discussion, follow, json, other, work_view};
use axum::http::StatusCode;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;
use vink_app::application::{
    Vink,
    ports::{PrincipalStore, StoreError},
    work_view::WorkViewResult,
};
use vink_app::domain::discussion::moderation::ModerationAction;
use vink_app::domain::ids::PrincipalId;
use vink_app::domain::principal::{PenName, SubjectKey};
use vink_app::domain::work_view::ViewRefusal;
use vink_app::identity::client::AccountDeletion;
use vink_app::persistence::PgStore;

fn operation() -> AccountDeletion {
    AccountDeletion {
        account_id: Uuid::new_v4(),
        operation_id: Uuid::new_v4(),
        accepted_at: OffsetDateTime::now_utc().replace_nanosecond(0).unwrap(),
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn account_deletion_retries_atomically_and_keeps_only_neutral_public_projection(
    pool: PgPool,
) {
    let http = app(&pool).await;
    let vink = discussion::discussion(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let actor = dev_seed::SAMPLE_AUTHOR;
    let peer = dev_seed::OTHER_PRINCIPAL;
    let operation = operation();
    let mut profile = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO principal_bindings (principal_id, authority, subject_id)
         VALUES ($1, 'vink-identity', $2)",
    )
    .bind(actor)
    .bind(operation.account_id.to_string())
    .execute(&mut *profile)
    .await
    .unwrap();
    sqlx::query("INSERT INTO author_handle_names (principal_id, handle) VALUES ($1, 'ho-so-cu')")
        .bind(actor)
        .execute(&mut *profile)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO author_profiles (principal_id, current_handle, bio)
                 VALUES ($1, 'ho-so-cu', 'Tiểu sử cần dọn')",
    )
    .bind(actor)
    .execute(&mut *profile)
    .await
    .unwrap();
    profile.commit().await.unwrap();
    let moderated = Uuid::new_v4();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
        .bind(moderated)
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO work_publications (work_id, audience, lifecycle, generation, policy_version)
                 VALUES ($1, 'private', 'removed', 2, 1)",
    )
    .bind(moderated)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO saved_works (principal_id, work_id, generation, saved_at)
                 VALUES ($1, $2, 2, now())",
    )
    .bind(peer)
    .bind(moderated)
    .execute(&pool)
    .await
    .unwrap();
    let peer_work = Uuid::new_v4();
    let peer_revision = Uuid::new_v4();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
        .bind(peer_work)
        .bind(peer)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
                 SELECT $1, $2, title, document, content_hash, $3 FROM revisions WHERE id = $4",
    )
    .bind(peer_revision)
    .bind(peer_work)
    .bind(peer)
    .bind(dev_seed::SAMPLE_REVISION)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_endorsements (work_id, revision_id, actor_id, selected, version, accepted_order)
                 VALUES ($1, $2, $3, true, 2, 2)")
        .bind(peer_work).bind(peer_revision).bind(actor).execute(&pool).await.unwrap();
    sqlx::query("UPDATE notification_activation SET active = true, epoch = 1")
        .execute(&pool)
        .await
        .unwrap();

    let kept = discussion::published(&vink, author(), None, "Nhận xét được giữ lại").await;
    let pending = discussion::post(&vink, author(), None, "Nhận xét cần dọn").await;
    let peer_comment = discussion::published(&vink, other(), None, "Nhận xét của người đọc").await;
    sqlx::query(
        "INSERT INTO comment_endorsements (comment_id, actor_id, selected, version, accepted_order)
                 VALUES ($1, $2, true, 2, 2)",
    )
    .bind(peer_comment.as_uuid())
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO saved_works (principal_id, work_id, generation, saved_at)
                 VALUES ($1, $3, 2, now()), ($2, $3, 2, now())",
    )
    .bind(actor)
    .bind(peer)
    .bind(work)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_view_totals (work_id, views, updated_at) VALUES ($1, 3, now())")
        .bind(work)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO work_view_days (work_id, day, views) VALUES ($1, current_date, 3)")
        .bind(work)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO work_view_windows (work_id, reader_digest, counted_at)
                 VALUES ($1, decode(repeat('21', 32), 'hex'), now())",
    )
    .bind(work)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_view_receipts (event_digest, work_id, verdict, basis_revision_id, decided_at)
                 VALUES (decode(repeat('32', 32), 'hex'), $1, 'counted', $2, now())")
        .bind(work).bind(dev_seed::SAMPLE_REVISION).execute(&pool).await.unwrap();

    let path = format!("/api/v1/works/{work}/reader");
    let (status, _, etag, _) = follow::get_with_headers(&http, &path, None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    let etag = etag.unwrap();

    sqlx::raw_sql(
        "CREATE FUNCTION reject_test_erasure() RETURNS trigger LANGUAGE plpgsql AS
         $$BEGIN RAISE EXCEPTION 'injected erasure failure'; END;$$;
         CREATE TRIGGER reject_test_erasure BEFORE UPDATE ON revisions
             FOR EACH ROW EXECUTE FUNCTION reject_test_erasure()",
    )
    .execute(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    store.apply_account_deletion(operation).await.unwrap();
    assert!(store.purge_account_deletion(operation).await.is_err());
    assert!(
        store
            .account_deletion_applied(operation.operation_id)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM social_retirements")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::raw_sql(
        "DROP TRIGGER reject_test_erasure ON revisions; DROP FUNCTION reject_test_erasure()",
    )
    .execute(&pool)
    .await
    .unwrap();

    let restarted = PgStore::new(pool.clone());
    let (first, replayed) = tokio::join!(
        store.apply_account_deletion(operation),
        restarted.apply_account_deletion(operation)
    );
    assert_eq!(first.unwrap(), replayed.unwrap());
    restarted.purge_account_deletion(operation).await.unwrap();
    store.purge_account_deletion(operation).await.unwrap();
    assert!(
        store
            .account_deletion_applied(operation.operation_id)
            .await
            .unwrap()
    );

    let (status, cache, _, reader) =
        follow::get_with_headers(&http, &path, None, &[("if-none-match", &etag)]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(
        json(&reader),
        serde_json::json!({"status":"author_deleted","work_id":work})
    );
    let (status, _, thread) = call(
        &http,
        "GET",
        &format!("/api/v1/works/{work}/discussion/public"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{thread}");
    assert!(thread.contains("Nhận xét được giữ lại") && thread.contains("Tài khoản đã xóa"));
    assert!(!thread.contains("Nhận xét cần dọn") && !thread.contains("Tác giả mẫu"));
    assert!(
        discussion::moderate(&vink, pending, 1, ModerationAction::Approve)
            .await
            .is_err()
    );

    let row: (Option<String>, Option<serde_json::Value>) =
        sqlx::query_as("SELECT title, document FROM revisions WHERE work_id = $1")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, (None, None));
    let profile: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT current_handle, bio FROM author_profiles WHERE principal_id = $1")
            .bind(actor)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(profile, (None, None));
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM author_handle_names WHERE handle = 'ho-so-cu' AND principal_id = $1)",
    ).bind(actor).fetch_one(&pool).await.unwrap());
    let publication = sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 2, now())",
    )
    .bind(work)
    .bind(dev_seed::SAMPLE_REVISION)
    .execute(&pool)
    .await;
    assert_eq!(
        publication
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23000")
    );
    let remaining: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM working_drafts WHERE work_id = $1)
              + (SELECT count(*) FROM work_view_totals WHERE work_id = $1)
              + (SELECT count(*) FROM work_view_days WHERE work_id = $1)
              + (SELECT count(*) FROM work_view_windows WHERE work_id = $1)
              + (SELECT count(*) FROM work_view_receipts WHERE work_id = $1)
              + (SELECT count(*) FROM saved_works WHERE principal_id = $2)
              + (SELECT count(*) FROM comment_endorsements WHERE actor_id = $2 AND selected)
              + (SELECT count(*) FROM work_endorsements WHERE actor_id = $2 AND selected)",
    )
    .bind(work)
    .bind(actor)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    let bookmark: (String, Option<Uuid>) = sqlx::query_as(
        "SELECT t.kind, t.revision_id FROM saved_works s JOIN saved_work_targets t USING (work_id)
          WHERE s.principal_id = $1",
    )
    .bind(peer)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bookmark, ("author_deleted".into(), None));
    let saved_but_hidden: (bool, bool) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM saved_works WHERE principal_id = $1 AND work_id = $2),
                EXISTS (SELECT 1 FROM saved_work_targets WHERE work_id = $2)",
    )
    .bind(peer)
    .bind(moderated)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(saved_but_hidden, (true, false));
    let (status, _, _) = call(
        &http,
        "GET",
        &format!("/api/v1/works/{moderated}/reader"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let notices: Vec<String> = sqlx::query_scalar(
        "SELECT event_type FROM notification_events
          WHERE envelope::jsonb->>'semantic_kind' IN ('invalidate', 'remove') ORDER BY event_type",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        notices,
        [
            "comment.endorsement.ung.changed",
            "notification.invalidation",
            "notification.invalidation",
            "work.endorsement.tham.changed",
        ]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM comments WHERE id = $1 AND body IS NOT NULL"
        )
        .bind(kept.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let (status, _, body) = call(
        &http,
        "GET",
        &format!("/api/v1/works/{work}/publication"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json(&body)["error"]["code"], "not_found");
    assert!(Vink::new(store).create_draft(author()).await.is_err());

    let now = operation.accepted_at + time::Duration::minutes(1);
    let id = work_view::event_at(now);
    let record = work_view::view(
        &vink,
        work_view::guest(),
        vink_app::domain::ids::WorkId::from_uuid(work),
        &id,
        now,
    )
    .await
    .unwrap();
    assert_eq!(
        record.result,
        WorkViewResult::Ignored(ViewRefusal::WorkNotFound)
    );
    assert_eq!(record.basis, None);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM work_view_receipts WHERE work_id = $1")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn account_deletion_fences_a_subject_with_no_existing_principal(pool: PgPool) {
    let store = PgStore::new(pool);
    let receipt = operation();
    assert_eq!(store.apply_account_deletion(receipt).await.unwrap(), None);
    assert_eq!(store.apply_account_deletion(receipt).await.unwrap(), None);
    let subject = SubjectKey::new("vink-identity", &receipt.account_id.to_string()).unwrap();
    let result = store
        .provision_principal(
            &subject,
            PrincipalId::from_uuid(Uuid::new_v4()),
            &PenName::parse("Bút danh").unwrap(),
        )
        .await;
    assert!(matches!(result, Err(StoreError::PrincipalSuspended)));
}
