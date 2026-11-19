use super::*;
use vink_app::application::handle::{HandleCommand, HandleError};
use vink_app::domain::handle::Intent;
use vink_app::domain::principal::SubjectKey;
use vink_app::persistence::handle::{backfill, retire};

pub(super) async fn create(pool: &PgPool, subject: &str, name: &str) -> PrincipalId {
    Vink::new(PgStore::new(pool.clone()))
        .provision_principal(&SubjectKey::new("vink-identity", subject).unwrap(), name)
        .await
        .unwrap()
        .bound
        .principal
}
fn command(actor: PrincipalId, intent: Intent, expected_version: i64, name: &str) -> HandleCommand {
    HandleCommand {
        principal_id: actor.as_uuid(),
        command_id: Uuid::new_v4(),
        intent,
        expected_version,
        handle: name.into(),
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn handle_backfill_is_bounded_resumable_and_only_provisions_bindings(pool: PgPool) {
    seed(&pool).await;
    let legacy = Uuid::new_v4();
    sqlx::query("INSERT INTO principals(id,display_name) VALUES ($1,'  Đặng  ')")
        .bind(legacy)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO principal_bindings(principal_id,authority,subject_id) VALUES($1,'vink-identity','legacy')").bind(legacy).execute(&pool).await.unwrap();
    let store = PgStore::new(pool.clone());
    assert!(store.resolve_handle("some-name", None).await.is_err());
    let first = backfill(&pool).await.unwrap();
    assert_eq!((first.created, first.existing, first.invalid), (1, 0, 0));
    assert!(first.unbound >= 2);
    let state = store
        .own_handle(PrincipalId::from_uuid(legacy))
        .await
        .unwrap();
    assert!(state.handle.starts_with("dang-"));
    assert_eq!(state.pen_name, "Đặng");
    let second = backfill(&pool).await.unwrap();
    assert_eq!(
        (second.created, second.existing, second.attempts),
        (0, 1, 0)
    );

    // Skip candidate insertion exactly as ON CONFLICT would. Sequence is external
    // to rollback, so its value measures the retry ceiling in the real allocator.
    sqlx::raw_sql("CREATE SEQUENCE collision_attempts; CREATE FUNCTION collide() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('collision_attempts'); RETURN NULL; END $$;
        CREATE TRIGGER collide BEFORE INSERT ON author_handle_names FOR EACH ROW EXECUTE FUNCTION collide()").execute(&pool).await.unwrap();
    let subject = SubjectKey::new("vink-identity", "collision").unwrap();
    assert!(
        Vink::new(store)
            .provision_principal(&subject, "May")
            .await
            .is_err()
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT last_value FROM collision_attempts),(SELECT count(*) FROM principal_bindings),(SELECT count(*) FROM author_profiles)")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (16, 1, 1));
    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION collide() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF nextval('collision_attempts') < 19 THEN RETURN NULL; END IF; RETURN NEW; END $$",
    )
    .execute(&pool)
    .await
    .unwrap();
    create(&pool, "recovered", "Mai").await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT last_value FROM collision_attempts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        19
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn handle_quota_uses_fence_receipts_and_rolls_back_every_effect(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let a = create(&pool, "a", "Cùng tên").await;
    let b = create(&pool, "b", "Cùng tên").await;
    let a0 = store.own_handle(a).await.unwrap().handle;
    let setup = command(a, Intent::Setup, 0, &a0);
    assert_eq!(
        store.set_handle(a, &setup).await.unwrap().remaining_changes,
        2
    );
    assert_eq!(
        store
            .set_handle(b, &command(b, Intent::Setup, 0, "but-muc"))
            .await
            .unwrap()
            .remaining_changes,
        2
    );
    assert!(matches!(
        store
            .set_handle(a, &command(a, Intent::Rename, 1, "but-muc"))
            .await,
        Err(HandleError::Taken)
    ));
    assert_eq!(
        store
            .set_handle(a, &command(a, Intent::Rename, 1, &a0))
            .await
            .unwrap()
            .accepted_version,
        1
    );
    sqlx::raw_sql("CREATE FUNCTION fail_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'simulated receipt failure'; END $$;
        CREATE TRIGGER fail_receipt BEFORE INSERT ON author_handle_receipts FOR EACH ROW EXECUTE FUNCTION fail_receipt()").execute(&pool).await.unwrap();
    assert!(
        store
            .set_handle(a, &command(a, Intent::Rename, 1, "rollback-name"))
            .await
            .is_err()
    );
    sqlx::query("DROP TRIGGER fail_receipt ON author_handle_receipts")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(store.own_handle(a).await.unwrap().remaining_changes, 2);
    let c1 = command(a, Intent::Rename, 1, "rollback-name");
    let c2 = command(a, Intent::Rename, 1, "but-chi");
    let (one, two) = tokio::join!(store.set_handle(a, &c1), store.set_handle(a, &c2));
    let winner = match (one, two) {
        (Ok(_), Err(HandleError::Stale(2))) => c1,
        (Err(HandleError::Stale(2)), Ok(_)) => c2,
        other => panic!("unexpected race: {other:?}"),
    };
    let last = command(a, Intent::Rename, 2, "muc-tim");
    assert_eq!(
        store.set_handle(a, &last).await.unwrap().remaining_changes,
        0
    );
    let receipt = store.set_handle(a, &winner).await.unwrap();
    assert_eq!(
        (receipt.accepted_version, receipt.remaining_changes),
        (2, 1)
    );
    assert_eq!(
        store.handle_receipt(a, winner.command_id).await.unwrap(),
        receipt
    );
    assert!(matches!(
        store.handle_receipt(b, winner.command_id).await,
        Err(HandleError::Missing)
    ));
    assert!(matches!(
        store.set_handle(b, &winner).await,
        Err(HandleError::WrongOwner)
    ));
    assert!(matches!(
        store
            .set_handle(
                a,
                &HandleCommand {
                    handle: "other".into(),
                    ..winner
                }
            )
            .await,
        Err(HandleError::KeyReused)
    ));
    assert!(matches!(
        store
            .set_handle(a, &command(a, Intent::Rename, 3, "lan-nua"))
            .await,
        Err(HandleError::Exhausted)
    ));
    assert!(matches!(
        store
            .set_handle(b, &command(b, Intent::Rename, 1, &a0))
            .await,
        Err(HandleError::Taken)
    ));
    assert!(matches!(
        store
            .set_handle(a, &command(a, Intent::Rename, 3, &a0))
            .await,
        Err(HandleError::OwnAlias)
    ));
    sqlx::query("UPDATE principals SET display_name='Bút danh mới' WHERE id=$1")
        .bind(a.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let state = store.own_handle(a).await.unwrap();
    assert_eq!(
        (
            state.pen_name.as_str(),
            state.handle.as_str(),
            state.remaining_changes,
            state.version
        ),
        ("Bút danh mới", "muc-tim", 0, 3)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn handle_public_http_direct_aliases_and_terminal_tombstones(pool: PgPool) {
    let app = app(&pool).await;
    let store = PgStore::new(pool.clone());
    let a = create(&pool, "a", "Nhật Ánh").await;
    let initial = store.own_handle(a).await.unwrap().handle;
    let (status, _, body) = call(&app, "GET", &format!("/@{initial}"), None, None, None).await;
    assert_eq!(status, 200);
    assert!(body.contains("Nhật Ánh") && body.contains("Chưa có tác phẩm công khai"));
    assert!(!body.contains("remaining_changes"));
    let (status, _, _) = call(&app, "GET", "/api/v1/me/handle", None, None, None).await;
    assert_eq!(status, 401);
    for (n, name) in [&initial, "ngay-mua", "ngay-nang"].into_iter().enumerate() {
        store
            .set_handle(
                a,
                &command(
                    a,
                    if n == 0 {
                        Intent::Setup
                    } else {
                        Intent::Rename
                    },
                    n as i64,
                    name,
                ),
            )
            .await
            .unwrap();
    }
    let expected: Value = serde_json::from_str(include_str!(
        "../../contracts/http/v1/fixtures/author.profile.current.json"
    ))
    .unwrap();
    let (code, cache, json_body) =
        call(&app, "GET", "/api/v1/authors/ngay-nang", None, None, None).await;
    assert_eq!(code, 200);
    assert_eq!(cache.as_deref(), Some("private, no-cache"));
    assert_eq!(json(&json_body), expected);
    for method in ["GET", "HEAD"] {
        for alias in [&initial, "ngay-mua"] {
            let r = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(format!("/@{alias}?next=elsewhere"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), 301);
            assert_eq!(r.headers()[header::LOCATION], "/@ngay-nang");
            assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
        }
    }
    let mut tx = pool.begin().await.unwrap();
    retire(&mut tx, a).await.unwrap();
    retire(&mut tx, a).await.unwrap();
    tx.commit().await.unwrap();
    for name in [&initial, "ngay-mua", "ngay-nang"] {
        for method in ["GET", "HEAD"] {
            let r = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(format!("/@{name}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), 410);
            assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
            assert!(r.headers().get(header::LOCATION).is_none());
            let body = r.into_body().collect().await.unwrap().to_bytes();
            assert!(!String::from_utf8_lossy(&body).contains("Nhật Ánh"));
        }
    }
    let b = create(&pool, "b", "Khác").await;
    assert!(matches!(
        store
            .set_handle(b, &command(b, Intent::Setup, 0, &initial))
            .await,
        Err(HandleError::Taken)
    ));
    for invalid in [initial.to_uppercase(), "api".into(), "a--b".into()] {
        assert!(
            sqlx::query("INSERT INTO author_handle_names(principal_id,handle) VALUES($1,$2)")
                .bind(b.as_uuid())
                .bind(invalid)
                .execute(&pool)
                .await
                .is_err()
        );
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn handle_two_accounts_compete_for_one_name(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let a = create(&pool, "left", "Cúc").await;
    let b = create(&pool, "right", "Cúc").await;
    let left = command(a, Intent::Setup, 0, "hoa-cuc");
    let right = command(b, Intent::Setup, 0, "hoa-cuc");
    let (left, right) = tokio::join!(store.set_handle(a, &left), store.set_handle(b, &right));
    let (winner, loser) = match (left, right) {
        (Ok(_), Err(HandleError::Taken)) => (a, b),
        (Err(HandleError::Taken), Ok(_)) => (b, a),
        other => panic!("unexpected claim race: {other:?}"),
    };
    let taken = store.own_handle(winner).await.unwrap();
    let other = store.own_handle(loser).await.unwrap();
    assert_eq!(taken.handle, "hoa-cuc");
    assert!(taken.setup_complete && !other.setup_complete);
    assert_eq!(
        (
            taken.remaining_changes,
            other.remaining_changes,
            other.version
        ),
        (2, 2, 0)
    );
    assert!(other.aliases.is_empty());
}

#[sqlx::test(migrations = false)]
async fn profile_fields_upgrade_existing_handle_without_resetting_receipts(pool: PgPool) {
    use vink_app::domain::profile::ProfilePolicies;
    use vink_app::persistence::MIGRATOR;

    MIGRATOR.run_to(HANDLE_MIGRATION, &pool).await.unwrap();
    let store = PgStore::new(pool.clone());
    let actor = create(&pool, "profile-upgrade", "Mực mới").await;
    let old = store.own_handle(actor).await.unwrap();
    let setup = command(actor, Intent::Setup, 0, "muc-moi");
    let receipt = store.set_handle(actor, &setup).await.unwrap();
    MIGRATOR.run(&pool).await.unwrap();

    let profile = store.own_profile(actor).await.unwrap();
    assert_eq!(profile.profile_version, 0);
    assert_eq!(profile.policies, ProfilePolicies::default());
    assert_eq!(profile.bio, None);
    assert_eq!(profile.handle, "muc-moi");
    assert!(profile.handle_setup_complete);
    assert_eq!(store.own_handle(actor).await.unwrap().aliases, [old.handle]);
    assert_eq!(
        store.handle_receipt(actor, setup.command_id).await.unwrap(),
        receipt
    );
}
