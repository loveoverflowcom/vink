//! #205: real PostgreSQL, HTTP, domain-oracle parity and measured sparse queues.
use super::*;
use vink_app::domain::moderation::authority::{ReviewCapability, ReviewTarget};
use vink_app::persistence::{appeals, moderation};

const WORKS: &str = "/api/v1/moderation/works/queue/page";
const APPEALS: &str = "/api/v1/moderation/appeals/page";
fn id(n: i32) -> Uuid {
    Uuid::from_u128(10_000 + n as u128)
}
async fn corpus(pool: &PgPool, n: i32) {
    dev_seed::apply(pool).await.unwrap();
    // IDs and identical timestamps intentionally put authorized rows behind many denied rows.
    for sql in [
        "INSERT INTO works(id,owner_id) SELECT ('00000000-0000-0000-0000-' || lpad(to_hex(10000+i),12,'0'))::uuid,$2 FROM generate_series(1,$1) i",
        "INSERT INTO revisions(id,work_id,title,document,content_hash,created_by) SELECT id,id,'[Mẫu] Queue ỹ ỷ', '{\"format\":\"default\",\"source\":\"Hàng chờ mẫu\"}',repeat('a',64),$1 FROM works WHERE id<> '33333333-3333-4333-8333-333333333333'::uuid",
        "INSERT INTO work_publications(work_id,audience,lifecycle,generation,intent_revision_id,intent_generation,ever_public,public_epoch,policy_version) SELECT id,'public','active',2,id,2,'{}',0,1 FROM works WHERE id<> '33333333-3333-4333-8333-333333333333'::uuid",
        "INSERT INTO work_revision_moderation(work_id,revision_id,moderation,decision_version,policy_version) SELECT id,id,'pending',1,1 FROM works WHERE id<> '33333333-3333-4333-8333-333333333333'::uuid",
        "INSERT INTO moderation_cases(id,subject_kind,work_id,revision_id,content_digest,decision_version,target_content_owner,target_work_owner,origin,version,state,created_at) SELECT id,'work_revision',id,id,repeat('a',64),1,$1,$1,'automation_paused',1,'queued','2026-11-16T00:00:00Z' FROM works WHERE id<> '33333333-3333-4333-8333-333333333333'::uuid",
        "INSERT INTO work_appeals(id,work_id,revision_id,content_digest,decision_version,appellant_id,decided_by,created_at) SELECT id,id,id,repeat('a',64),1,$1,'evaluator','2026-11-16T00:00:00Z' FROM works WHERE id<> '33333333-3333-4333-8333-333333333333'::uuid",
    ] {
        // First statement has a count parameter; the others deliberately use only author.
        if sql.contains("generate_series") {
            sqlx::query(sql)
                .bind(n)
                .bind(dev_seed::SAMPLE_AUTHOR)
                .execute(pool)
                .await
                .unwrap();
        } else if sql.contains("$1") {
            sqlx::query(sql)
                .bind(dev_seed::SAMPLE_AUTHOR)
                .execute(pool)
                .await
                .unwrap();
        } else {
            sqlx::query(sql).execute(pool).await.unwrap();
        }
    }
}
async fn grant(
    pool: &PgPool,
    actor: Uuid,
    scope: Option<Uuid>,
    group: bool,
    expired: bool,
) -> Uuid {
    let gid = Uuid::now_v7();
    sqlx::query("INSERT INTO moderation_grants(id,grantee_kind,grantee_id,capability,scope_kind,scope_work_id,expires_at,origin) VALUES ($1,$2,$3,'works',$4,$5,CASE WHEN $6 THEN now()-interval '1 second' ELSE NULL END,'admin')")
    .bind(gid).bind(if group {"group"} else {"principal"}).bind(actor).bind(if scope.is_some(){"work"}else{"global"}).bind(scope).bind(expired).execute(pool).await.unwrap();
    gid
}
async fn page(app: &Router, path: &str, actor: Uuid) -> Value {
    let (status, cache, body) = call(app, "GET", path, Some(actor), None, None).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    json(&body)
}
async fn refused(app: &Router, path: &str, actor: Uuid, status: StatusCode) {
    let (s, cache, body) = call(app, "GET", path, Some(actor), None, None).await;
    assert_eq!(s, status, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert!(json(&body).get("items").is_none());
}
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderation_queue_pages_filter_before_limit_and_recheck_live_authority(pool: PgPool) {
    let app = app(&pool).await;
    corpus(&pool, 120).await;
    let actor = dev_seed::OTHER_PRINCIPAL;
    let g = Uuid::now_v7();
    sqlx::query("INSERT INTO moderation_groups(id,name,created_by) VALUES ($1,'[Mẫu] group',$2)")
        .bind(g)
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO moderation_group_members(group_id,principal_id,added_by) VALUES ($1,$2,$2)",
    )
    .bind(g)
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();
    for n in 111..=120 {
        grant(&pool, g, Some(id(n)), true, false).await;
        grant(&pool, actor, Some(id(n)), false, true).await;
    }
    grant(&pool, g, Some(id(111)), true, false).await; // duplicate grants must never duplicate rows
    grant(&pool, actor, Some(id(1)), false, true).await;
    // Same domain oracle as commands, evaluated independently against every target.
    let mut conn = pool.acquire().await.unwrap();
    let now = time::OffsetDateTime::now_utc();
    let d = moderation::load_moderation_directory(&mut conn, now)
        .await
        .unwrap();
    drop(conn);
    let expected: Vec<_> = (1..=120)
        .filter(|n| {
            d.resolve(
                PrincipalId::from_uuid(actor),
                &ReviewTarget {
                    capability: ReviewCapability::Works,
                    work: WorkId::from_uuid(id(*n)),
                    content_owner: PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR),
                    work_owner: PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR),
                },
                now,
            )
            .is_ok()
        })
        .map(id)
        .collect();
    let first = page(&app, &format!("{WORKS}?limit=3"), actor).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 3);
    assert_eq!(first["items"][0]["id"], id(111).to_string());
    assert!(first.get("total").is_none());
    let cursor = first["next_cursor"].as_str().unwrap();
    // Complete a static corpus with stable same-timestamp order, no duplicates or gaps.
    let mut seen = Vec::new();
    let mut p = first.clone();
    loop {
        seen.extend(
            p["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| Uuid::parse_str(r["id"].as_str().unwrap()).unwrap()),
        );
        let Some(c) = p["next_cursor"].as_str() else {
            break;
        };
        p = page(&app, &format!("{WORKS}?limit=3&cursor={c}"), actor).await;
    }
    assert_eq!(seen, expected);
    for query in [
        "limit=0",
        "limit=101",
        "limit=a",
        "q=a",
        "limit=1&limit=1",
        "cursor=%",
        "cursor=zz",
        "limit=%ff",
    ] {
        refused(
            &app,
            &format!("{WORKS}?{query}"),
            actor,
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    refused(
        &app,
        &format!("{APPEALS}?limit=3&cursor={cursor}"),
        actor,
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    refused(
        &app,
        &format!("{WORKS}?limit=4&cursor={cursor}"),
        actor,
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    refused(
        &app,
        &format!("{WORKS}?limit=3&cursor={cursor}"),
        dev_seed::SAMPLE_AUTHOR,
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    // Anchor disappears and another case is claimed/private: position is independent of row existence.
    sqlx::query("UPDATE moderation_cases SET state='closed',close_reason='superseded' WHERE id=$1")
        .bind(id(113))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE moderation_cases SET state='claimed',claimed_by=$1 WHERE id=$2")
        .bind(actor)
        .bind(id(114))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_publications SET audience='private',intent_revision_id=NULL,intent_generation=NULL WHERE work_id=$1")
        .bind(id(115))
        .execute(&pool)
        .await
        .unwrap();
    let next = page(&app, &format!("{WORKS}?limit=3&cursor={cursor}"), actor).await;
    assert_eq!(next["items"][0]["id"], id(114).to_string());
    assert_eq!(next["items"][1]["id"], id(116).to_string());
    let remaining = grant(&pool, actor, Some(id(120)), false, false).await;
    sqlx::query("DELETE FROM moderation_group_members WHERE group_id=$1")
        .bind(g)
        .execute(&pool)
        .await
        .unwrap();
    let narrowed = page(&app, &format!("{WORKS}?limit=3&cursor={cursor}"), actor).await;
    assert_eq!(narrowed["items"][0]["id"], id(120).to_string());
    assert_ne!(narrowed["authority_key"], first["authority_key"]);
    sqlx::query("UPDATE moderation_grants SET expires_at=now() WHERE id=$1")
        .bind(remaining)
        .execute(&pool)
        .await
        .unwrap();
    refused(
        &app,
        &format!("{WORKS}?limit=3&cursor={cursor}"),
        actor,
        StatusCode::FORBIDDEN,
    )
    .await;
    let live = grant(&pool, actor, None, false, false).await;
    assert_eq!(
        page(&app, WORKS, actor).await["items"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    assert_eq!(
        page(&app, &format!("{WORKS}?limit=100"), actor).await["items"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/moderation/works/queue?limit=1")
                .header("x-test-principal", actor.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(
        response.headers()[header::LINK]
            .to_str()
            .unwrap()
            .contains("rel=\"next\"")
    );
    let legacy: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(legacy.as_array().unwrap().len(), 1);

    assert_eq!(
        page(&app, &format!("{WORKS}?limit=3&cursor={cursor}"), actor).await["items"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    sqlx::query("UPDATE moderation_grants SET revoked=true WHERE id=$1")
        .bind(live)
        .execute(&pool)
        .await
        .unwrap();
    refused(&app, WORKS, actor, StatusCode::FORBIDDEN).await;
    grant(&pool, actor, None, false, false).await;
    sqlx::query("INSERT INTO principal_suspensions(id,principal_id,reason) VALUES ($1,$2,'test')")
        .bind(Uuid::now_v7())
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    // Resolver may refuse suspended standing before queue authorization; either is still no-store.
    let (s, cache, _) = call(&app, "GET", WORKS, Some(actor), None, None).await;
    assert!(s == StatusCode::FORBIDDEN);
    assert_eq!(cache.as_deref(), Some("no-store"));
}
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderation_queue_admin_appeal_paging_recusal_and_seat_revocation(pool: PgPool) {
    let app = app(&pool).await;
    corpus(&pool, 7).await;
    let actor = dev_seed::OTHER_PRINCIPAL;
    let mut conn = pool.acquire().await.unwrap();
    moderation::add_admin_seat(&mut conn, "test", PrincipalId::from_uuid(actor), None)
        .await
        .unwrap();
    drop(conn);
    // Admins also recuse, and lifecycle excludes private/tombstoned targets before lookahead.
    sqlx::query("UPDATE moderation_cases SET target_content_owner=$1 WHERE id=$2")
        .bind(actor)
        .bind(id(1))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_appeals SET appellant_id=$1 WHERE id=$2")
        .bind(actor)
        .bind(id(1))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_publications SET audience='private',intent_revision_id=NULL,intent_generation=NULL WHERE work_id=$1")
        .bind(id(2))
        .execute(&pool)
        .await
        .unwrap();
    for path in [WORKS, APPEALS] {
        // Legacy array clients must explicitly follow Link; ignoring it sees only page one.
        let mut uri = format!("{}?limit=2", path.strip_suffix("/page").unwrap());
        let mut legacy_ids = Vec::new();
        loop {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&uri)
                        .header("x-test-principal", actor.to_string())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let next = response.headers().get(header::LINK).map(|h| {
                h.to_str()
                    .unwrap()
                    .strip_prefix('<')
                    .unwrap()
                    .split_once('>')
                    .unwrap()
                    .0
                    .to_owned()
            });
            let body: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            let items = body.as_array().unwrap();
            assert!(items.len() <= 2);
            legacy_ids.extend(items.iter().map(|r| r["id"].as_str().unwrap().to_owned()));
            assert!(
                legacy_ids.len() <= 5,
                "legacy continuation must make progress"
            );
            if let Some(next) = next {
                assert!(
                    !items.is_empty(),
                    "an empty page cannot advertise continuation"
                );
                uri = next;
            } else {
                break;
            }
        }
        assert_eq!(
            legacy_ids,
            (3..=7).map(|n| id(n).to_string()).collect::<Vec<_>>()
        );
        let first = page(&app, &format!("{path}?limit=2"), actor).await;
        assert_eq!(first["items"][0]["id"], id(3).to_string());
        let c = first["next_cursor"].as_str().unwrap();
        let next = page(&app, &format!("{path}?limit=2&cursor={c}"), actor).await;
        assert_eq!(next["items"][0]["id"], id(5).to_string());
        let last = page(
            &app,
            &format!(
                "{path}?limit=2&cursor={}",
                next["next_cursor"].as_str().unwrap()
            ),
            actor,
        )
        .await;
        assert_eq!(last["items"].as_array().unwrap().len(), 1);
        assert!(last["next_cursor"].is_null());
    }
    let first = page(&app, &format!("{APPEALS}?limit=2"), actor).await;
    let c = first["next_cursor"].as_str().unwrap();
    sqlx::query("UPDATE work_appeals SET state='closed',close_reason='superseded' WHERE id=$1")
        .bind(id(4))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        page(&app, &format!("{APPEALS}?limit=2&cursor={c}"), actor).await["items"][0]["id"],
        id(5).to_string()
    );
    sqlx::query("DELETE FROM moderation_admin_seats WHERE principal_id=$1")
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    refused(
        &app,
        &format!("{APPEALS}?limit=2&cursor={c}"),
        actor,
        StatusCode::FORBIDDEN,
    )
    .await;
    refused(
        &app,
        &format!("/api/v1/moderation/appeals/{}", id(5)),
        actor,
        StatusCode::FORBIDDEN,
    )
    .await;
}
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderation_queue_large_sparse_query_plans(pool: PgPool) {
    let app = app(&pool).await;
    corpus(&pool, 6000).await;
    let actor = dev_seed::OTHER_PRINCIPAL;
    grant(&pool, actor, Some(id(5999)), false, false).await;
    sqlx::raw_sql("ANALYZE moderation_cases; ANALYZE work_publications; ANALYZE revisions; ANALYZE work_appeals;").execute(&pool).await.unwrap();
    assert_eq!(
        page(&app, &format!("{WORKS}?limit=1"), actor).await["items"][0]["id"],
        id(5999).to_string()
    );
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        moderation::WORK_QUEUE_SQL
    );
    let plan: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(actor)
        .bind(false)
        .bind(vec![id(5999)])
        .bind(None::<time::OffsetDateTime>)
        .bind(None::<Uuid>)
        .bind(2i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("WORK_SPARSE_PLAN={plan}");
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        moderation::WORK_QUEUE_SQL
    );
    let global_plan: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(actor)
        .bind(true)
        .bind(Vec::<Uuid>::new())
        .bind(None::<time::OffsetDateTime>)
        .bind(None::<Uuid>)
        .bind(51i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("WORK_GLOBAL_PLAN={global_plan}");
    assert_eq!(global_plan[0]["Plan"]["Actual Rows"], 51);
    assert!(
        global_plan
            .to_string()
            .contains("moderation_cases_work_queue_order")
    );
    let at = time::OffsetDateTime::from_unix_timestamp(1794787200).unwrap();
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        moderation::WORK_QUEUE_SQL
    );
    let deep: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(actor)
        .bind(true)
        .bind(Vec::<Uuid>::new())
        .bind(Some(at))
        .bind(Some(id(5900)))
        .bind(51i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("WORK_DEEP_PLAN={deep}");
    assert_eq!(deep[0]["Plan"]["Actual Rows"], 51);

    assert_eq!(plan[0]["Plan"]["Actual Rows"], 1);
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        appeals::APPEAL_QUEUE_SQL
    );
    let plan: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(actor)
        .bind(None::<time::OffsetDateTime>)
        .bind(None::<Uuid>)
        .bind(51i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("APPEAL_PLAN={plan}");
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}",
        appeals::APPEAL_QUEUE_SQL
    );
    let deep: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(actor)
        .bind(Some(at))
        .bind(Some(id(5900)))
        .bind(51i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("APPEAL_DEEP_PLAN={deep}");
    assert_eq!(deep[0]["Plan"]["Actual Rows"], 51);

    assert_eq!(plan[0]["Plan"]["Actual Rows"], 51);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn moderation_queue_custom_plans_are_scoped_to_each_read_transaction(pool: PgPool) {
    use vink_app::domain::moderation_queue::{QueueKind, QueueQuery};
    corpus(&pool, 12).await;
    let actor = PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL);
    moderation::add_admin_seat(&mut pool.acquire().await.unwrap(), "test", actor, None)
        .await
        .unwrap();
    // Exercise the production persistence entrypoints on one repeatedly reused connection.
    let pinned = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    sqlx::query("SET plan_cache_mode = force_generic_plan")
        .execute(&pinned)
        .await
        .unwrap();
    for _ in 0..8 {
        let works = QueueQuery::parse(actor, QueueKind::Works, Some("2"), None).unwrap();
        let appeals = QueueQuery::parse(actor, QueueKind::Appeals, Some("2"), None).unwrap();
        assert_eq!(
            moderation::list_open_work_cases(&pinned, actor, &works)
                .await
                .unwrap()
                .0
                .len(),
            3
        );
        assert_eq!(
            appeals::list_open(&pinned, actor, &appeals)
                .await
                .unwrap()
                .len(),
            3
        );
    }
    for statement in [moderation::WORK_QUEUE_SQL, appeals::APPEAL_QUEUE_SQL] {
        let (custom, generic): (i64, i64) = sqlx::query_as(
            "SELECT custom_plans, generic_plans FROM pg_prepared_statements WHERE statement = $1",
        )
        .bind(statement)
        .fetch_one(&pinned)
        .await
        .unwrap();
        assert!(
            custom >= 8,
            "queue must plan with the actual actor and cursor: {custom}"
        );
        assert_eq!(
            generic, 0,
            "queue must not reuse an unselective generic plan"
        );
    }
    let mode: String = sqlx::query_scalar("SHOW plan_cache_mode")
        .fetch_one(&pinned)
        .await
        .unwrap();
    assert_eq!(
        mode, "force_generic_plan",
        "SET LOCAL must not leak into the pool"
    );
    sqlx::query("DELETE FROM moderation_admin_seats WHERE principal_id = $1")
        .bind(actor.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let works = QueueQuery::parse(actor, QueueKind::Works, None, None).unwrap();
    assert!(
        moderation::list_open_work_cases(&pinned, actor, &works)
            .await
            .is_err()
    );
    let appeals = QueueQuery::parse(actor, QueueKind::Appeals, None, None).unwrap();
    assert!(appeals::list_open(&pinned, actor, &appeals).await.is_err());
    let mode: String = sqlx::query_scalar("SHOW plan_cache_mode")
        .fetch_one(&pinned)
        .await
        .unwrap();
    assert_eq!(
        mode, "force_generic_plan",
        "refusal must restore session planning too"
    );
    pinned.close().await;
}
