//! #296: the production Feed query and HTTP router on migrated PostgreSQL. Fixture SQL
//! installs approved pointers; the Follow and Retirement writes use their real owners.
use std::collections::BTreeSet;

use super::{
    follow::{get_with_headers, seed_author},
    *,
};
use time::{OffsetDateTime, macros::datetime};
use vink_app::{
    application::{
        follow::SetFollowInput,
        social_retirement::{SocialRetirement, SocialRetirementProgress},
    },
    domain::revision::ContentHash,
    persistence::feed::{FEED_PAGE_SIZE, FOLLOWING_FEED_PAGE_SQL},
};

const PATH: &str = "/api/v1/me/following-feed";
const SESSION: (&str, &str) = ("cookie", "__Host-vink_session=feed-session-one");
const SESSION_B: (&str, &str) = ("cookie", "__Host-vink_session=feed-session-two");
const AT: OffsetDateTime = datetime!(2026-11-19 00:00 UTC);

async fn works(pool: &PgPool, owner: PrincipalId, count: i64, title: &str) -> Vec<Uuid> {
    let data = doc(&[&["Nguồn của revision công khai."]]);
    let hash = ContentHash::of(&titled(title, data.clone()))
        .as_str()
        .to_owned();
    let mut tx = pool.begin().await.unwrap();
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "INSERT INTO works(id, owner_id)
         SELECT gen_random_uuid(), $1 FROM generate_series(1, $2::bigint) RETURNING id",
    )
    .bind(owner.as_uuid())
    .bind(count)
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO revisions(id, work_id, title, document, content_hash, created_by)
         SELECT work, work, $2, $3, $4, $5 FROM unnest($1::uuid[]) AS work",
    )
    .bind(&ids)
    .bind(title)
    .bind(sqlx::types::Json(data))
    .bind(hash)
    .bind(owner.as_uuid())
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO publications(work_id, revision_id, generation, published_at)
         SELECT work, work, 1, $2 FROM unnest($1::uuid[]) AS work",
    )
    .bind(&ids)
    .bind(AT)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    ids
}

async fn set_follow(
    store: &PgStore,
    viewer: PrincipalId,
    handle: &str,
    order: i64,
    following: bool,
) {
    store
        .set_follow(
            viewer,
            handle,
            SetFollowInput {
                command_id: Uuid::new_v4(),
                expected_order: order,
                following,
            },
        )
        .await
        .unwrap();
}

async fn get(
    router: &Router,
    path: &str,
    who: Option<PrincipalId>,
    session: (&str, &str),
    status: u16,
) -> Value {
    let (code, cache, etag, body) = get_with_headers(
        router,
        path,
        who.map(|id| id.as_uuid()),
        &[session, ("if-none-match", "*")],
    )
    .await;
    assert_eq!(code.as_u16(), status, "{path}: {body}");
    assert_eq!(cache.as_deref(), Some("private, no-store"));
    assert_eq!(etag, None);
    json(&body)
}

fn ids(body: &Value, field: &str) -> Vec<Uuid> {
    body[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| Uuid::parse_str(card["work_id"].as_str().unwrap()).unwrap())
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn first_public_keyset_live_revocation_cursor_and_no_store(pool: PgPool) {
    let router = app(&pool).await;
    let store = PgStore::new(pool.clone());
    let viewer = seed_author(&pool, Uuid::new_v4(), "Người đọc", "feed-nguoi-doc").await;
    let other = seed_author(&pool, Uuid::new_v4(), "Thành viên", "feed-thanh-vien").await;
    let author = seed_author(&pool, Uuid::new_v4(), "Trăng non", "feed-trang-non").await;
    let poet = seed_author(&pool, Uuid::new_v4(), "Mây trắng", "feed-may-trang").await;
    seed_author(&pool, Uuid::new_v4(), "Bản thảo", "feed-ban-thao").await;
    let outside = seed_author(&pool, Uuid::new_v4(), "Ngoài dòng", "feed-ngoai-dong").await;

    let mut expected = works(&pool, author, 28, "Tác phẩm trước Follow").await;
    let by_poet = works(&pool, poet, 7, "Bài của Mây trắng").await;
    expected.extend_from_slice(&by_poet);
    works(&pool, outside, 700, "Không theo dõi").await;
    // Fixture-only updates create adversarial ties across pages and ensure the
    // recently replaced pointer sorts by the original first-public instant.
    sqlx::raw_sql("ALTER TABLE work_first_publications DISABLE TRIGGER guard_first_publication")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE work_first_publications SET first_published_at=$1 WHERE work_id=ANY($2::uuid[])",
    )
    .bind(AT)
    .bind(&expected)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(
        "ALTER TABLE work_first_publications ENABLE TRIGGER guard_first_publication; ANALYZE",
    )
    .execute(&pool)
    .await
    .unwrap();
    expected.sort_unstable_by(|a, b| b.cmp(a));

    let guest = get(&router, &format!("{PATH}?q=forged"), None, SESSION, 401).await;
    assert_eq!(guest["error"]["code"], "unauthenticated");
    let no_follows = get(&router, PATH, Some(other), SESSION_B, 200).await;
    assert_eq!(no_follows["has_following"], false);
    set_follow(&store, other, "feed-ban-thao", 0, true).await;
    let no_works = get(&router, PATH, Some(other), SESSION_B, 200).await;
    assert_eq!(no_works["has_following"], true);
    assert!(ids(&no_works, "works").is_empty());
    set_follow(&store, viewer, "feed-trang-non", 0, true).await;
    set_follow(&store, viewer, "feed-may-trang", 0, true).await;

    let head = get(&router, PATH, Some(viewer), SESSION, 200).await;
    assert_eq!(ids(&head, "works"), expected[..FEED_PAGE_SIZE]);
    assert!(head["works"].as_array().unwrap().iter().all(|card| {
        card.get("document").is_none()
            && card.get("body").is_none()
            && card["published_at"] == "2026-11-19T00:00:00Z"
    }));
    assert_eq!(head["viewer"], viewer.as_uuid().to_string());
    let cursor = head["next_cursor"].as_str().unwrap();
    let next_path = format!("{PATH}?cursor={cursor}");
    for (who, session) in [(other, SESSION), (viewer, SESSION_B)] {
        let restart = get(&router, &next_path, Some(who), session, 200).await;
        assert_eq!(restart, json!({"state":"restart"}));
    }
    for wrong_scope in ["q=sai", "mode=works", "tag=tho"] {
        get(
            &router,
            &format!("{next_path}&{wrong_scope}"),
            Some(viewer),
            SESSION,
            422,
        )
        .await;
    }
    let first_tail = get(&router, &next_path, Some(viewer), SESSION, 200).await;
    assert_eq!(ids(&first_tail, "works"), expected[12..24]);
    let deep = expected[15];
    let around = get(
        &router,
        &format!("{PATH}?around={deep}"),
        Some(viewer),
        SESSION,
        200,
    )
    .await;
    assert_eq!(ids(&around, "works")[0], deep);

    const RETURN_SCOPE: &str = "x-vink-feed-return-scope";
    let read_return = |session: (&str, &str), hint: Option<&str>| {
        let mut request = Request::get(format!("{PATH}?around={deep}"))
            .header("x-test-principal", viewer.as_uuid().to_string())
            .header(session.0, session.1);
        if let Some(hint) = hint {
            request = request.header(RETURN_SCOPE, hint);
        }
        router.clone().oneshot(request.body(Body::empty()).unwrap())
    };
    let original = read_return(SESSION, None).await.unwrap();
    assert_eq!(original.status(), StatusCode::OK);
    let original_scope = original.headers()[RETURN_SCOPE].to_str().unwrap();
    assert_eq!(original_scope.len(), 64);
    let same = read_return(SESSION, Some(original_scope)).await.unwrap();
    assert_eq!(same.headers()[RETURN_SCOPE], original_scope);
    let body = same.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        ids(&json(std::str::from_utf8(&body).unwrap()), "works")[0],
        deep
    );

    let new = read_return(SESSION_B, Some(original_scope)).await.unwrap();
    assert_eq!(new.status(), StatusCode::OK);
    assert_ne!(new.headers()[RETURN_SCOPE], original_scope);
    let body = new.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        ids(&json(std::str::from_utf8(&body).unwrap()), "works"),
        expected[..FEED_PAGE_SIZE]
    );

    let replacement = expected[0];
    let replaced = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO revisions(id, work_id, title, document, content_hash, created_by)
         SELECT $1, work_id, 'Revision mới', document, $3, created_by
           FROM revisions WHERE id=$2",
    )
    .bind(replaced)
    .bind(replacement)
    .bind(
        ContentHash::of(&titled(
            "Revision mới",
            doc(&[&["Nguồn của revision công khai."]]),
        ))
        .as_str(),
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE publications SET revision_id=$1, generation=2, published_at=now() WHERE work_id=$2",
    )
    .bind(replaced)
    .bind(replacement)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // Withdraw a card from the already displayed page, without invalidating the boundary.
    let withdrawn = expected[1];
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(withdrawn)
        .execute(&pool)
        .await
        .unwrap();
    let seen = expected[..12]
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let page = get(
        &router,
        &format!("{next_path}&seen={seen}"),
        Some(viewer),
        SESSION,
        200,
    )
    .await;
    assert_eq!(ids(&page, "works"), expected[12..24]);
    let mut expected_retained = expected[..12].to_vec();
    expected_retained.remove(1);
    assert_eq!(ids(&page, "retained"), expected_retained);
    assert_eq!(page["retained"][0]["title"], "Revision mới");
    assert_eq!(
        page["retained"][0]["published_at"],
        head["works"][0]["published_at"]
    );
    let fallback = get(
        &router,
        &format!("{PATH}?around={withdrawn}"),
        Some(viewer),
        SESSION,
        200,
    )
    .await;
    assert_eq!(ids(&fallback, "works")[0], replacement);

    // Reinsert a pointer: the persisted first timestamp and the rank are unchanged.
    sqlx::query("INSERT INTO publications(work_id, revision_id, generation, published_at) VALUES ($1,$1,3,now())")
        .bind(withdrawn).execute(&pool).await.unwrap();
    let restored = get(&router, PATH, Some(viewer), SESSION, 200).await;
    assert_eq!(ids(&restored, "works"), expected[..12]);
    assert_eq!(
        restored["works"][1]["published_at"],
        head["works"][1]["published_at"]
    );

    // Exercise the real graph writer. A stale cursor either restarts if its
    // boundary disappeared, or skips revoked works before filling the page.
    let boundary = expected[11];
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(boundary)
        .execute(&pool)
        .await
        .unwrap();
    let restart = get(&router, &next_path, Some(viewer), SESSION, 200).await;
    assert_eq!(restart, json!({"state": "restart"}));
    sqlx::query("INSERT INTO publications(work_id, revision_id, generation, published_at) VALUES ($1,$1,2,now())")
        .bind(boundary).execute(&pool).await.unwrap();
    let poet_owned: BTreeSet<_> = by_poet.into_iter().collect();
    set_follow(&store, viewer, "feed-may-trang", 1, false).await;
    let after = get(&router, &next_path, Some(viewer), SESSION, 200).await;
    if poet_owned.contains(&boundary) {
        assert_eq!(after["state"], "restart");
    } else {
        let allowed: Vec<_> = expected[12..]
            .iter()
            .filter(|id| !poet_owned.contains(id))
            .copied()
            .take(12)
            .collect();
        assert_eq!(ids(&after, "works"), allowed);
    }
    set_follow(&store, viewer, "feed-may-trang", 2, true).await;
    let lookup = "/api/v1/authors?q=%40feed-may-trang";
    let (status, _, before) = call(&router, "GET", lookup, None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&before)["authors"][0]["href"], "/@feed-may-trang");

    let retirement = SocialRetirement::new(poet, Uuid::new_v4()).unwrap();
    assert_eq!(
        store.fence_social_retirement(retirement).await.unwrap(),
        SocialRetirementProgress::Fenced
    );
    let edge_pending_cleanup: bool = sqlx::query_scalar(
        "SELECT active FROM author_follows WHERE follower_id=$1 AND followee_id=$2",
    )
    .bind(viewer.as_uuid())
    .bind(poet.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(edge_pending_cleanup);
    let (status, cache, after) = call(&router, "GET", lookup, None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(json(&after)["authors"], json!([]));
    let after_retire = get(
        &router,
        &format!("{PATH}?seen={seen}"),
        Some(viewer),
        SESSION,
        200,
    )
    .await;
    assert!(
        ids(&after_retire, "retained")
            .iter()
            .all(|id| !poet_owned.contains(id))
    );
    assert_eq!(
        ids(&after_retire, "retained").contains(&boundary),
        !poet_owned.contains(&boundary)
    );

    assert_eq!(
        store.step_social_retirement(retirement).await.unwrap(),
        SocialRetirementProgress::Complete
    );

    let disabled = router_with_feed_off(&pool);
    get(&disabled, PATH, Some(viewer), SESSION, 404).await;
    let (_, _, _, html) = get_with_headers(
        &disabled,
        "/?mode=following",
        Some(viewer.as_uuid()),
        &[SESSION],
    )
    .await;
    assert!(html.contains("data-feed-state=\"disabled\""));
    assert!(!html.contains("id=\"explore-mode-following\""));
    assert!(!html.contains("Tác phẩm trước Follow"));
    let response = get(&router, PATH, Some(viewer), SESSION, 200).await;
    assert_eq!(
        ids(&response, "works").len(),
        12,
        "rollback never rewrites graph or dates"
    );
    let (_, _, _, ssr) = get_with_headers(
        &router,
        "/?mode=following",
        Some(viewer.as_uuid()),
        &[SESSION],
    )
    .await;
    let (_, _, _, guest_ssr) = get_with_headers(&router, "/?mode=following", None, &[]).await;
    for html in [&ssr, &guest_ssr] {
        assert!(html.contains("data-feed-state=\"checking\""));
        assert!(html.contains("explore-mode-following"));
        assert!(!html.contains("Tác phẩm trước Follow"));
        assert!(!html.contains(&viewer.as_uuid().to_string()));
    }

    // Inspect the actual production keyset SQL, with a selective graph and larger
    // unrelated corpus. This statement never has a revision/document node.
    let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {FOLLOWING_FEED_PAGE_SQL}",
    )))
    .bind(viewer.as_uuid())
    .bind(AT)
    .bind(boundary)
    .bind(false)
    .bind(13_i64)
    .fetch_all(&pool)
    .await
    .unwrap();
    let plan = plan.join("\n");
    eprintln!("Following Feed PG plan:\n{plan}");
    assert!(
        plan.starts_with("Limit ") && plan.contains("rows=13"),
        "{plan}"
    );
    assert!(
        !plan.contains("revisions") && !plan.contains("working_drafts"),
        "{plan}"
    );
}

fn router_with_feed_off(pool: &PgPool) -> Router {
    router(
        AppState::new(
            Vink::new(PgStore::new(pool.clone())),
            Arc::new(HeaderIdentity),
            ORIGIN,
            std::path::Path::new("target/site"),
        )
        .with_following_feed_enabled(false),
    )
}
