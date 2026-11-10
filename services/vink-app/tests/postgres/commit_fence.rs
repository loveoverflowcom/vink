//! D-05 generalized standing fence for drafts on real PostgreSQL: a suspension committed before
//! a draft write's commit refuses it; one still uncommitted makes the write wait and then refuse;
//! a write that already holds the fence makes the real suspension command wait. Discussion and
//! endorsement have the same checks in their own files.

use vink_app::domain::principal::SuspensionReason;
use vink_app::persistence::{reinstate_principal, suspend_principal};

use super::*;

fn reason() -> SuspensionReason {
    SuspensionReason::parse("Tạm khóa trong bộ test bản nháp").unwrap()
}

fn save(work: WorkId, expected_version: i64, title: &str) -> SaveDraft {
    SaveDraft {
        work_id: work,
        expected_version,
        title: title.into(),
        document: doc(&[&["Dòng thơ"]]),
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_writes_are_refused_at_commit_once_suspended(pool: PgPool) {
    let vink = seeded(&pool).await;
    let me = author().unwrap();
    let draft = vink.create_draft(author()).await.unwrap();
    suspend_principal(&pool, me, &reason()).await.unwrap();
    let works_before: i64 = sqlx::query_scalar("SELECT count(*) FROM works")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        vink.create_draft(author()).await.unwrap_err(),
        DraftError::Suspended
    );
    assert_eq!(
        vink.save_draft(author(), save(draft.work_id, 1, "Bị chặn"))
            .await
            .unwrap_err(),
        DraftError::Suspended
    );
    let works_after: i64 = sqlx::query_scalar("SELECT count(*) FROM works")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(works_after, works_before, "no work row was created");
    let (version, title): (i64, String) =
        sqlx::query_as("SELECT version, title FROM working_drafts WHERE work_id = $1")
            .bind(draft.work_id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((version, title.as_str()), (1, ""));
    // Over HTTP the test resolver ignores suspensions: the store's fence answers 403.
    let app = app(&pool).await;
    let (status, _, body) = call(
        &app,
        "PUT",
        &format!("/api/v1/drafts/{}", draft.work_id),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 1,
            "title": "Qua HTTP",
            "document": doc(&[&["Dòng"]]),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.contains("principal_suspended"), "{body}");
    reinstate_principal(&pool, me).await.unwrap();
    assert!(
        vink.save_draft(author(), save(draft.work_id, 1, "Sau khi mở"))
            .await
            .is_ok()
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_saves_and_suspensions_serialize_on_the_principal(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let me = author().unwrap();
    let draft = vink.create_draft(author()).await.unwrap();
    let saving = |expected: i64, title: &'static str| {
        let vink = Arc::clone(&vink);
        let work = draft.work_id;
        tokio::spawn(async move { vink.save_draft(author(), save(work, expected, title)).await })
    };

    // Order A: an uncommitted suspension holds the principal; the save waits, then refuses.
    let mut suspending = pool.begin().await.unwrap();
    let pid = backend_pid(&mut suspending).await;
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason)
         VALUES ($1, $2, 'Tạm khóa trong bộ test bản nháp')",
    )
    .bind(Uuid::now_v7())
    .bind(dev_seed::SAMPLE_AUTHOR)
    .execute(&mut *suspending)
    .await
    .unwrap();
    let blocked = saving(1, "Đua với lệnh khóa");
    wait_blocked_by(&pool, pid).await;
    assert!(!blocked.is_finished(), "the save waits for the suspension");
    suspending.commit().await.unwrap();
    assert_eq!(blocked.await.unwrap().unwrap_err(), DraftError::Suspended);
    reinstate_principal(&pool, me).await.unwrap();

    // Order B: a save that holds the fence (paused on its draft row) makes the real suspension
    // command wait; the save commits first, the next one is refused.
    let mut pauser = pool.begin().await.unwrap();
    let pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM working_drafts WHERE work_id = $1 FOR UPDATE")
        .bind(draft.work_id.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let in_flight = saving(1, "Đang lưu dở");
    wait_blocked_by(&pool, pid).await;
    let suspended = {
        let pool = pool.clone();
        tokio::spawn(async move { suspend_principal(&pool, me, &reason()).await })
    };
    wait_blocked(&pool, 2).await;
    assert!(
        !suspended.is_finished(),
        "the suspension waits for the save"
    );
    pauser.rollback().await.unwrap();
    assert!(in_flight.await.unwrap().is_ok());
    suspended.await.unwrap().unwrap();
    assert_eq!(
        vink.save_draft(author(), save(draft.work_id, 2, "Sau khi khóa"))
            .await
            .unwrap_err(),
        DraftError::Suspended
    );
}
