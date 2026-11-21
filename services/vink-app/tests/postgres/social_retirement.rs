use super::{app, backend_pid, call, follow::seed_author, json, wait_blocked_by};
use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;
use vink_app::application::follow::{FollowError, SetFollowInput};
use vink_app::application::handle::{HandleCommand, HandleError};
use vink_app::application::social_retirement::{
    SocialRetirement, SocialRetirementError,
    SocialRetirementProgress::{Complete, Fenced},
};
use vink_app::domain::handle::Intent;
use vink_app::domain::ids::PrincipalId;
use vink_app::persistence::{PgStore, handle};

fn operation(principal: PrincipalId) -> SocialRetirement {
    SocialRetirement::new(principal, Uuid::new_v4()).unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn social_retirement_restarts_between_commits_without_reviving_the_public_graph(
    pool: PgPool,
) {
    let app = app(&pool).await;
    let owner = seed_author(&pool, Uuid::new_v4(), "Bút danh cần thu hồi", "ten-moi").await;
    let peer = seed_author(&pool, Uuid::new_v4(), "Người đọc", "nguoi-doc").await;
    sqlx::query(
        "INSERT INTO author_handle_names (principal_id, handle)
         VALUES ($1, 'ten-cu'), ($1, 'ten-cu-hon')",
    )
    .bind(owner.as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(
        "CREATE TEMP TABLE retirement_fixture ON COMMIT DROP AS
         SELECT md5('retirement-test-' || i)::uuid AS id, 'ban-doc-' || i AS handle
           FROM generate_series(1, 51) i;
         INSERT INTO principals (id, display_name) SELECT id, handle FROM retirement_fixture;
         INSERT INTO principal_bindings (principal_id, authority, subject_id)
             SELECT id, 'vink-identity', id::text FROM retirement_fixture;
         INSERT INTO author_handle_names (principal_id, handle)
             SELECT id, handle FROM retirement_fixture;
         INSERT INTO author_profiles (principal_id, current_handle)
             SELECT id, handle FROM retirement_fixture;",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO author_follows (follower_id, followee_id, active, version, accepted_order, active_since)
         SELECT id, $1, true, 1, 1, now() FROM retirement_fixture
         UNION ALL SELECT $1, id, true, 1, 1, now() FROM retirement_fixture
         UNION ALL VALUES ($2, $1, true, 1, 1, now()), ($1, $2, true, 1, 1, now())",
    )
    .bind(owner.as_uuid())
    .bind(peer.as_uuid())
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let live_path = "/api/v1/authors/nguoi-doc";
    let (_, _, live_before) = call(&app, "GET", live_path, None, None, None).await;
    assert_eq!(json(&live_before)["relationships"]["followers_count"], 1);
    let command = operation(owner);
    let store = PgStore::new(pool.clone());
    assert!(matches!(
        store.step_social_retirement(command).await,
        Err(SocialRetirementError::NotFenced)
    ));
    sqlx::raw_sql(
        "CREATE FUNCTION fail_test_profile_fence() RETURNS trigger LANGUAGE plpgsql AS
         $$BEGIN RAISE EXCEPTION 'test fence failure'; END;$$;
         CREATE TRIGGER fail_test_profile_fence BEFORE UPDATE ON author_profiles
           FOR EACH ROW EXECUTE FUNCTION fail_test_profile_fence();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(store.fence_social_retirement(command).await.is_err());
    assert!(matches!(
        store.step_social_retirement(command).await,
        Err(SocialRetirementError::NotFenced)
    ));
    sqlx::raw_sql(
        "DROP TRIGGER fail_test_profile_fence ON author_profiles; DROP FUNCTION fail_test_profile_fence();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        store.fence_social_retirement(command).await.unwrap(),
        Fenced
    );
    drop(store);

    // Begin committed but no batch ran yet. Both directions are already invisible to reads.
    for path in ["/@ten-moi", "/@ten-cu", "/@ten-cu-hon"] {
        let (status, cache, body) = call(&app, "GET", path, None, None, None).await;
        assert_eq!(status, StatusCode::GONE, "{path}");
        assert_eq!(cache.as_deref(), Some("no-store"));
        assert!(!body.contains("Bút danh cần thu hồi"));
    }
    let (_, _, live_after) = call(&app, "GET", live_path, None, None, None).await;
    assert_eq!(json(&live_after)["relationships"]["followers_count"], 0);
    let (_, _, list_after) = call(
        &app,
        "GET",
        "/api/v1/authors/nguoi-doc/followers",
        None,
        None,
        None,
    )
    .await;
    assert!(!list_after.contains("ten-moi"), "{list_after}");
    let id = owner.as_uuid();
    assert_eq!(active_edges(&pool, id).await, 104);

    let store = PgStore::new(pool.clone());
    assert_eq!(
        store.fence_social_retirement(command).await.unwrap(),
        Fenced
    );
    assert!(matches!(
        store.fence_social_retirement(operation(owner)).await,
        Err(SocialRetirementError::OperationConflict)
    ));

    // A PostgreSQL failure inside the batch rolls back every edge write and the checkpoint.
    sqlx::raw_sql(
        "CREATE SEQUENCE test_edge_attempts;
         CREATE FUNCTION fail_test_edge_cleanup() RETURNS trigger LANGUAGE plpgsql AS
         $$BEGIN
             IF nextval('test_edge_attempts') = 37 THEN
                 RAISE EXCEPTION 'test batch failure';
             END IF;
             RETURN NEW;
         END;$$;
         CREATE TRIGGER fail_test_edge_cleanup BEFORE UPDATE ON author_follows
           FOR EACH ROW EXECUTE FUNCTION fail_test_edge_cleanup();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(store.step_social_retirement(command).await.is_err());
    let attempted: i64 = sqlx::query_scalar("SELECT last_value FROM test_edge_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempted, 37);
    assert_eq!(active_edges(&pool, id).await, 104);
    sqlx::raw_sql("DROP TRIGGER fail_test_edge_cleanup ON author_follows; DROP FUNCTION fail_test_edge_cleanup(); DROP SEQUENCE test_edge_attempts;")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(store.step_social_retirement(command).await.unwrap(), Fenced);
    assert_eq!(active_edges(&pool, id).await, 4);
    let restarted = PgStore::new(pool.clone());
    // Treat the completion ACK as lost, then replay the original operation.
    let _lost_ack = restarted.step_social_retirement(command).await.unwrap();
    assert_eq!(
        restarted.fence_social_retirement(command).await.unwrap(),
        Complete
    );
    assert_eq!(
        restarted.step_social_retirement(command).await.unwrap(),
        Complete
    );
    assert_eq!(active_edges(&pool, id).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM author_follows
              WHERE (follower_id=$1 OR followee_id=$1) AND version=2 AND accepted_order=2",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        104
    );
    assert!(
        sqlx::query("DELETE FROM social_retirements WHERE principal_id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE social_retirements SET edges_complete=false WHERE principal_id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .is_err()
    );
    let reuse = restarted
        .set_handle(
            peer,
            &HandleCommand {
                principal_id: peer.as_uuid(),
                command_id: Uuid::new_v4(),
                intent: Intent::Rename,
                expected_version: 0,
                handle: "ten-cu".into(),
            },
        )
        .await;
    assert!(matches!(reuse, Err(HandleError::Taken)));
}

async fn active_edges(pool: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM author_follows WHERE active AND (follower_id=$1 OR followee_id=$1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn social_retirement_wins_against_queued_follow_unfollow_and_rename(pool: PgPool) {
    let owner = seed_author(&pool, Uuid::from_u128(1), "Tác giả", "tac-gia").await;
    let peer = seed_author(&pool, Uuid::from_u128(2), "Người đọc", "nguoi-doc").await;
    let store = PgStore::new(pool.clone());
    let follow = SetFollowInput {
        command_id: Uuid::new_v4(),
        expected_order: 0,
        following: true,
    };
    store.set_follow(peer, "tac-gia", follow).await.unwrap();

    let command = operation(owner);
    let mut blocker = pool.begin().await.unwrap();
    let blocker_pid = backend_pid(&mut blocker).await;
    sqlx::query("SELECT id FROM principals WHERE id=$1 FOR UPDATE")
        .bind(owner.as_uuid())
        .execute(&mut *blocker)
        .await
        .unwrap();
    let fence = tokio::spawn({
        let store = store.clone();
        async move { store.fence_social_retirement(command).await }
    });
    wait_blocked_by(&pool, blocker_pid).await;
    let unfollow = tokio::spawn({
        let store = store.clone();
        async move {
            store
                .set_follow(
                    peer,
                    "tac-gia",
                    SetFollowInput {
                        command_id: Uuid::new_v4(),
                        expected_order: 1,
                        following: false,
                    },
                )
                .await
        }
    });
    let rename = tokio::spawn({
        let store = store.clone();
        async move {
            store
                .set_handle(
                    owner,
                    &HandleCommand {
                        principal_id: owner.as_uuid(),
                        command_id: Uuid::new_v4(),
                        expected_version: 0,
                        intent: Intent::Rename,
                        handle: "ten-sau-fence".into(),
                    },
                )
                .await
        }
    });
    let reverse = tokio::spawn({
        let store = store.clone();
        async move {
            store
                .set_follow(
                    owner,
                    "nguoi-doc",
                    SetFollowInput {
                        command_id: Uuid::new_v4(),
                        expected_order: 0,
                        following: true,
                    },
                )
                .await
        }
    });
    blocker.commit().await.unwrap();
    assert_eq!(fence.await.unwrap().unwrap(), Fenced);
    assert!(matches!(unfollow.await.unwrap(), Err(FollowError::Gone)));
    assert!(matches!(rename.await.unwrap(), Err(HandleError::Missing)));
    assert!(reverse.await.unwrap().is_err());
    assert_eq!(
        store.step_social_retirement(command).await.unwrap(),
        Complete
    );
    assert_eq!(active_edges(&pool, owner.as_uuid()).await, 0);
    assert!(
        store.set_follow(peer, "tac-gia", follow).await.is_err(),
        "old receipt is no authority"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn social_retirement_redacts_bound_legacy_author_before_backfill(pool: PgPool) {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1,'Tên cũ chưa cấp Handle')")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO principal_bindings(principal_id, authority, subject_id) VALUES ($1,'vink-identity',$2)")
        .bind(id).bind(id.to_string()).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let store = PgStore::new(pool.clone());
    let command = operation(PrincipalId::from_uuid(id));
    assert_eq!(
        store.fence_social_retirement(command).await.unwrap(),
        Fenced
    );
    assert_eq!(
        store.step_social_retirement(command).await.unwrap(),
        Complete
    );
    let public: (String, Option<String>, bool, Option<String>) = sqlx::query_as(
        "SELECT display_name, handle, retired, display_name_search_key
           FROM public_author_attribution WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(public, ("Tài khoản đã xóa".into(), None, true, None));

    assert_eq!(handle::backfill(&pool).await.unwrap().created, 0);
    assert!(
        sqlx::query("INSERT INTO author_profiles(principal_id, current_handle) VALUES ($1, NULL)")
            .bind(id)
            .execute(&pool)
            .await
            .is_err()
    );
    let other = seed_author(&pool, Uuid::new_v4(), "Người khác", "tac-gia-khac").await;
    let conflict = SocialRetirement::new(other, command_id(&pool, id).await).unwrap();
    assert!(matches!(
        store.fence_social_retirement(conflict).await,
        Err(SocialRetirementError::OperationConflict)
    ));
    let unbound = Uuid::new_v4();
    sqlx::query("INSERT INTO principals(id,display_name) VALUES ($1,'Principal kỹ thuật')")
        .bind(unbound)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        store
            .fence_social_retirement(operation(PrincipalId::from_uuid(unbound)))
            .await,
        Err(SocialRetirementError::Unbound)
    ));
}

async fn command_id(pool: &PgPool, id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT operation_id FROM social_retirements WHERE principal_id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}
