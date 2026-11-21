use super::{app, call, follow::seed_author, json};
use axum::Router;
use axum::http::StatusCode;
use serde_json::Value;
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;
use vink_app::application::handle::HandleCommand;
use vink_app::domain::handle::Intent;
use vink_app::persistence::{AUTHOR_SEARCH_SQL, PgStore, handle};

async fn read(app: &Router, query: &str, actor: Option<Uuid>) -> String {
    let (status, cache, body) = call(
        app,
        "GET",
        &format!("/api/v1/authors{query}"),
        actor,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    body
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_lookup_is_actor_free_and_reads_current_profiles_before_paging(pool: PgPool) {
    let app = app(&pool).await;
    let first = seed_author(&pool, Uuid::new_v4(), "Đặng Mai", "dang-mai").await;
    let second = seed_author(&pool, Uuid::new_v4(), "Đặng Mai", "hoa-giay").await;
    let lookup = "?q=%C4%90%E1%BA%B7ng+Mai&limit=1";
    let baseline = read(&app, lookup, None).await;
    let page = json(&baseline);
    assert_eq!(
        page["authors"][0],
        serde_json::json!({"pen_name": "Đặng Mai", "handle": "dang-mai", "href": "/@dang-mai"})
    );
    let cursor = page["next_cursor"].as_str().unwrap();
    assert!(!cursor.contains(&first.as_uuid().to_string()));
    let next = format!("{lookup}&cursor={cursor}");
    let second_page = read(&app, &next, None).await;
    assert_eq!(json(&second_page)["authors"][0]["href"], "/@hoa-giay");
    assert!(json(&second_page).get("next_cursor").is_none());

    // Change only private fields and membership; both exact response bytes and keyset stay put.
    sqlx::query(
        "UPDATE author_profiles
            SET bio = 'Biệt danh bí mật',
                policies = jsonb_set(policies, '{bio}', '\"self\"')
          WHERE principal_id = $1",
    )
    .bind(first.as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO author_follows (follower_id, followee_id, active, version, accepted_order, active_since)
         VALUES ($1, $2, true, 1, 1, now()), ($2, $1, true, 1, 1, now())",
    )
    .bind(first.as_uuid())
    .bind(second.as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    for viewer in [None, Some(first.as_uuid()), Some(second.as_uuid())] {
        assert_eq!(read(&app, lookup, viewer).await, baseline);
        assert_eq!(read(&app, &next, viewer).await, second_page);
    }
    let folded = read(&app, "?q=dang+mai", None).await;
    let decomposed = read(&app, "?q=%C4%90a%CC%A3%CC%86ng+Mai", None).await;
    assert_eq!(folded, decomposed);
    assert_eq!(json(&folded)["authors"].as_array().unwrap().len(), 2);
    let bare = json(&read(&app, "?q=dang", None).await);
    assert_eq!(
        bare["authors"].as_array().unwrap().len(),
        2,
        "deduplicates both matches"
    );
    let exact = read(&app, "?q=%40DANG-MAI", None).await;
    assert_eq!(
        json(&exact)["authors"].as_array().unwrap(),
        &[page["authors"][0].clone()]
    );
    assert!(read(&app, "", None).await.contains("/@hoa-giay"));
    assert_eq!(
        json(&read(&app, "?q=Bi%E1%BB%87t+danh+b%C3%AD+m%E1%BA%ADt", None).await)["authors"],
        serde_json::json!([])
    );
    let (status, _, profile) =
        call(&app, "GET", "/api/v1/authors/hoa-giay", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&profile)["state"], "current");
    assert_eq!(
        json(&profile)["publications"]["works"],
        serde_json::json!([])
    );

    for query in [
        format!("?q=khac&cursor={cursor}"),
        "?cursor=malformed".into(),
        "?limit=25".into(),
        "?q=one&q=two".into(),
        "?scope=viewer".into(),
    ] {
        let (status, cache, body) = call(
            &app,
            "GET",
            &format!("/api/v1/authors{query}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
    }

    let store = PgStore::new(pool.clone());
    store
        .set_handle(
            first,
            &HandleCommand {
                principal_id: first.as_uuid(),
                command_id: Uuid::new_v4(),
                expected_version: 0,
                intent: Intent::Rename,
                handle: "dang-moi".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        json(&read(&app, "?q=%40dang-moi", None).await)["authors"][0]["handle"],
        "dang-moi"
    );
    assert_eq!(
        json(&read(&app, "?q=%40dang-mai", None).await)["authors"],
        serde_json::json!([])
    );
    let mut tx = pool.begin().await.unwrap();
    handle::retire(&mut tx, first).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(read(&app, lookup, None).await, second_page);
    assert!(!read(&app, "", None).await.contains("/@dang-moi"));
    let (status, _, _) = call(&app, "GET", "/@dang-mai", None, None, None).await;
    assert_eq!(status, StatusCode::GONE);

    sqlx::query("UPDATE author_handle_rollout SET ready = false")
        .execute(&pool)
        .await
        .unwrap();
    let (status, cache, body) = call(&app, "GET", "/api/v1/authors", None, None, None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_search_indexes_live_keysets_on_skewed_names(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(
        "CREATE TEMP TABLE author_search_fixture ON COMMIT DROP AS
         SELECT md5('social-295-' || i)::uuid AS id,
                'tac-gia-' || lpad(i::text, 5, '0') AS handle,
                CASE WHEN i IN (1, 14999, 15000) THEN 'Ánh Sáng Hiếm'
                     WHEN i % 4 <> 0 THEN 'Đặng Mai' ELSE 'Hà ' || i END AS pen,
                i <= 1000 AS retired
           FROM generate_series(1, 15000) i;
         INSERT INTO principals (id, display_name) SELECT id, pen FROM author_search_fixture;
         INSERT INTO principal_bindings(principal_id, authority, subject_id)
              SELECT id, 'vink-identity', id::text FROM author_search_fixture;
         INSERT INTO author_handle_names(principal_id, handle)
              SELECT id, handle FROM author_search_fixture;
         INSERT INTO author_profiles(principal_id, current_handle)
              SELECT id, CASE WHEN retired THEN NULL ELSE handle END FROM author_search_fixture;
         ANALYZE principals;
         ANALYZE author_profiles;",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    for (name, handle, after, expected_index) in [
        (
            None,
            Some("%"),
            "tac-gia-14950",
            "author_profiles_search_handle_idx",
        ),
        (
            Some("anh sang%"),
            None,
            "",
            "principals_search_pen_name_idx",
        ),
        (
            Some("dang mai%"),
            None,
            "",
            "author_profiles_search_handle_idx",
        ),
        (
            None,
            Some("tac-gia-15000"),
            "",
            "author_profiles_search_handle_idx",
        ),
    ] {
        let plan: Value = sqlx::query_scalar(AssertSqlSafe(format!(
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {AUTHOR_SEARCH_SQL}"
        )))
        .bind(name)
        .bind(handle)
        .bind(after)
        .bind(13_i64)
        .persistent(false)
        .fetch_one(&pool)
        .await
        .unwrap();
        let stats = &plan[0];
        let root = &stats["Plan"];
        let plan_string = root.to_string();
        assert!(plan_string.contains(expected_index), "{root}");
        assert!(root["Actual Rows"].as_u64().unwrap() <= 13, "{root}");
        println!(
            "author search {name:?}/{handle:?}: {expected_index}, {} rows, {} shared block hits, {} ms",
            root["Actual Rows"], root["Shared Hit Blocks"], stats["Execution Time"]
        );
    }
    let store = PgStore::new(pool);
    let page = store
        .discover_authors(vink_app::application::author_discovery::DiscoverAuthors {
            q: Some("Ánh Sáng".into()),
            limit: Some("1".into()),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(
        page.authors[0].handle, "tac-gia-14999",
        "retired match was filtered before LIMIT"
    );
    assert!(page.next_cursor.is_some());
}
