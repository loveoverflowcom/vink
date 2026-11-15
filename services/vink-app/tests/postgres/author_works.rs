//! #214: production reads against real PostgreSQL, with independent public projections.
use super::*;
use vink_app::application::author_works::AuthorWorkSummary;
use vink_app::application::drafts::SearchDrafts;
use vink_app::domain::draft_query::DraftSort;

async fn page(vink: &Vink<PgStore>) -> AuthorWorkSummary {
    vink.search_author_works(author(), SearchDrafts::default())
        .await
        .unwrap()
        .works
        .into_iter()
        .find(|w| w.work_id == sample_work())
        .unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_work_corpus_cursor_privacy_and_legacy_contract(pool: PgPool) {
    let app = app(&pool).await;
    let vink = Vink::new(PgStore::new(pool.clone()));
    for (n, owner) in [
        (175_i64, dev_seed::SAMPLE_AUTHOR),
        (30, dev_seed::OTHER_PRINCIPAL),
    ] {
        sqlx::query("INSERT INTO works(id,owner_id,created_at) SELECT md5($1::uuid::text || n)::uuid,$1,'2026-01-01'::timestamptz + (n/3)*interval '1 second' FROM generate_series(1,$2) n")
            .bind(owner).bind(n).execute(&pool).await.unwrap();
        // Half the corpus has no working draft (legal schema); it must remain discoverable.
        sqlx::query("INSERT INTO working_drafts(work_id,version,title,document,updated_at) SELECT md5($1::uuid::text || n)::uuid,1,CASE WHEN n%7=0 THEN 'ĐƯỜNG VỀ' ELSE 'Trang '||n END,'{\"schema_version\":1,\"blocks\":[]}', '2026-01-01'::timestamptz + (n/3)*interval '1 second' FROM generate_series(1,$2) n WHERE n%2=0")
            .bind(owner).bind(n).execute(&pool).await.unwrap();
    }
    for sort in ["recent", "title"] {
        let mut ids = std::collections::HashSet::new();
        let mut ordered = Vec::new();
        let mut cursor = None;
        loop {
            let result = vink
                .search_author_works(
                    author(),
                    SearchDrafts {
                        sort: Some(sort.into()),
                        limit: Some("24".into()),
                        cursor,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            assert!(result.works.len() <= 24);
            for w in result.works {
                ordered.push(w.work_id.as_uuid());
                assert!(ids.insert(w.work_id));
            }
            cursor = result.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(ids.len(), 176);
        let mut expected:Vec<(Uuid,String,time::OffsetDateTime)>=sqlx::query_as("SELECT w.id,coalesce(d.title,''),coalesce(d.updated_at,w.created_at) FROM works w LEFT JOIN working_drafts d ON d.work_id=w.id WHERE w.owner_id=$1").bind(dev_seed::SAMPLE_AUTHOR).fetch_all(&pool).await.unwrap();
        expected.sort_by(|a, b| {
            if sort == "recent" {
                b.2.cmp(&a.2).then(a.0.cmp(&b.0))
            } else {
                vink_app::domain::draft_query::title_key(&a.1)
                    .cmp(&vink_app::domain::draft_query::title_key(&b.1))
                    .then(a.0.cmp(&b.0))
            }
        });
        assert_eq!(
            ordered,
            expected.into_iter().map(|r| r.0).collect::<Vec<_>>()
        );
    }
    let found = vink
        .search_author_works(
            author(),
            SearchDrafts {
                query: Some("  đường  về ".into()),
                sort: Some("title".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(!found.works.is_empty());
    assert!(found.works.iter().all(|w| w.title.as_str() == "ĐƯỜNG VỀ"));
    let (status, cache, body) = call(
        &app,
        "GET",
        "/api/v1/author/works?limit=24",
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    let decoded: vink_web::contract::author_works::AuthorWorkList =
        serde_json::from_str(&body).unwrap();
    assert_eq!(decoded.works.len(), 24);
    assert!(!body.contains("document") && !body.contains("actor_id"));
    let cursor = decoded.next_cursor.unwrap();
    let old_route = format!("/api/v1/drafts?limit=24&cursor={cursor}");
    assert_eq!(
        call(
            &app,
            "GET",
            &old_route,
            Some(dev_seed::SAMPLE_AUTHOR),
            None,
            None
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    for (actor, path, expected) in [
        (
            None,
            "/api/v1/author/works".into(),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Some(dev_seed::OTHER_PRINCIPAL),
            format!("/api/v1/author/works?limit=24&cursor={cursor}"),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Some(dev_seed::SAMPLE_AUTHOR),
            "/api/v1/author/works?owner_id=forged".into(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let (s, c, b) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(s, expected, "{b}");
        assert_eq!(c.as_deref(), Some("no-store"));
        assert!(!b.contains("Trang"));
    }
    let (_, _, body) = call(
        &app,
        "GET",
        "/api/v1/drafts",
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .await;
    let legacy: vink_web::contract::DraftList = serde_json::from_str(&body).unwrap();
    assert_eq!(legacy.drafts.len(), 88);
    // Public SSR emits shell only, including when an owner session is present.
    let (_, _, html) = call(
        &app,
        "GET",
        "/ban-viet",
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .await;
    assert!(!html.contains("ĐƯỜNG VỀ") && !html.contains("current_public"));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_work_revision_counts_public_projection_and_content_metadata_changes(pool: PgPool) {
    let vink = seeded(&pool).await;
    let capture = vink
        .capture_snapshot(author(), sample_work(), "author-work-capture", 1, 1)
        .await
        .unwrap();
    let r2 = capture.revision_id.as_uuid();
    // Fixture switch is atomic, like publication writes; not evidence for write command semantics.
    sqlx::query("UPDATE work_publications SET intent_revision_id=$2,intent_generation=3,generation=3 WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK).bind(r2).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_revision_moderation(work_id,revision_id,decision_version,moderation) VALUES($1,$2,1,'pending')")
        .bind(dev_seed::SAMPLE_WORK).bind(r2).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_endorsements(work_id,revision_id,actor_id,selected,version,accepted_order) VALUES($1,$2,$3,true,2,2)")
        .bind(dev_seed::SAMPLE_WORK).bind(dev_seed::SAMPLE_REVISION).bind(dev_seed::OTHER_PRINCIPAL).execute(&pool).await.unwrap();
    // Every review/presence combination for root and reply. Public replies under invisible
    // roots still count once; root placeholders and private overlays never count.
    let thread = Uuid::now_v7();
    sqlx::query("INSERT INTO discussion_threads(id,work_id,comments_open) VALUES($1,$2,false)")
        .bind(thread)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    for review in ["approved", "pending", "rejected"] {
        for removal in [None, Some("author"), Some("moderation")] {
            let root = Uuid::now_v7();
            let body = (removal != Some("author")).then_some("comment fixture");
            let reply = Uuid::now_v7();
            sqlx::query("INSERT INTO comments(id,thread_id,kind,replied_to_reply,author_id,body,review,moderation_version,admission_policy,admission_reason) VALUES($1,$2,'root',false,$3,'comment fixture','pending',1,'premoderation.v1','policy_premoderation')")
                .bind(root).bind(thread).bind(dev_seed::OTHER_PRINCIPAL).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO comments(id,thread_id,kind,root_id,root_kind,replied_to_reply,author_id,body,review,moderation_version,admission_policy,admission_reason) VALUES($1,$2,'reply',$3,'root',false,$4,'comment fixture','pending',1,'premoderation.v1','policy_premoderation')")
                .bind(reply).bind(thread).bind(root).bind(dev_seed::OTHER_PRINCIPAL).execute(&pool).await.unwrap();
            sqlx::query("UPDATE comments SET body=$2,review=$3,removal=$4,moderation_version=2 WHERE id=ANY($1)")
                .bind(vec![root,reply]).bind(body).bind(review).bind(removal).execute(&pool).await.unwrap();
            if review == "pending" && removal.is_none() {
                sqlx::query(
                    "UPDATE comments SET review='approved',moderation_version=3 WHERE id=$1",
                )
                .bind(reply)
                .execute(&pool)
                .await
                .unwrap();
            }
        }
    }
    let w = page(&vink).await;
    let p = w.publication.unwrap();
    assert_eq!(p.intent_moderation.as_deref(), Some("pending"));
    let public = p.current_public.unwrap();
    assert_eq!(public.revision_id.as_uuid(), dev_seed::SAMPLE_REVISION);
    assert_eq!(public.tham, Some(1));
    assert_eq!(public.comments, Some(3));
    let reader = vink.publication(sample_work()).await.unwrap();
    assert_eq!(reader.revision_id, public.revision_id);
    let app = app(&pool).await;
    let (s, _, body) = call(
        &app,
        "GET",
        &format!("/api/v1/works/{}/discussion/public", dev_seed::SAMPLE_WORK),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let projection = json(&body);
    let independent: usize = projection["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|root| {
            usize::from(root["root"]["kind"] == "comment")
                + root["preview"].as_array().unwrap().len()
        })
        .sum();
    assert_eq!(independent, 3, "{projection}");
    // Hold the metrics relation until a pointer switch commits between page and counters.
    // Repeatable read must return the R1 anchor AND R1 count, never R1/R2 mixed facts.
    let mut blocker = pool.acquire().await.unwrap();
    let pid = backend_pid(&mut blocker).await;
    sqlx::query("BEGIN").execute(&mut *blocker).await.unwrap();
    sqlx::query("LOCK TABLE work_endorsements IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let read_pool = pool.clone();
    let in_flight = tokio::spawn(async move { page(&Vink::new(PgStore::new(read_pool))).await });
    wait_blocked_by(&pool, pid).await;
    // A new revision gets its own count, and identical draft content+tags is not "Có bản sửa".
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE publications SET revision_id=$2,generation=3,metadata_snapshot_id=(SELECT metadata_snapshot_id FROM revisions WHERE id=$2) WHERE work_id=$1").bind(dev_seed::SAMPLE_WORK).bind(r2).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE work_publications SET pointer_revision_id=$2,pointer_intent_generation=3 WHERE work_id=$1").bind(dev_seed::SAMPLE_WORK).bind(r2).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    sqlx::query("ROLLBACK")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let old = in_flight
        .await
        .unwrap()
        .publication
        .unwrap()
        .current_public
        .unwrap();
    assert_eq!(old.revision_id.as_uuid(), dev_seed::SAMPLE_REVISION);
    assert_eq!(old.tham, Some(1));
    let p = page(&vink).await.publication.unwrap();
    assert_eq!(p.has_draft_changes, Some(false));
    assert_eq!(p.current_public.unwrap().tham, Some(0));
    // Metadata-only change is independent of content version and public metadata.
    use vink_app::application::catalog::{CatalogRequest, ChangeCatalog, CuratorAuthority};
    use vink_app::application::taxonomy::SaveWorkTags;
    let command = ChangeCatalog {
        expected_version: 1,
        request: CatalogRequest::Create {
            facet: "theme".into(),
            label: "Ký ức".into(),
        },
    }
    .validate()
    .unwrap();
    let tag = vink
        .change_catalog(CuratorAuthority::operator(), command)
        .await
        .unwrap()
        .subject;
    vink.save_work_tags(
        author(),
        SaveWorkTags {
            work_id: sample_work(),
            expected_version: 1,
            form: None,
            themes: vec![tag.to_string()],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        page(&vink).await.publication.unwrap().has_draft_changes,
        Some(true)
    );
    vink.save_work_tags(
        author(),
        SaveWorkTags {
            work_id: sample_work(),
            expected_version: 2,
            form: None,
            themes: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        page(&vink).await.publication.unwrap().has_draft_changes,
        Some(false)
    );
    // A no-op save increments version but does not create content changes.
    sqlx::query("UPDATE working_drafts SET version=version+1 WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        page(&vink).await.publication.unwrap().has_draft_changes,
        Some(false)
    );
    sqlx::query("UPDATE working_drafts SET title=title||' sửa',version=version+1 WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        page(&vink).await.publication.unwrap().has_draft_changes,
        Some(true)
    );
    // Same revision republished retains its count, while withdrawal hides metrics.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE publications SET revision_id=$2,generation=4,metadata_snapshot_id=NULL WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK)
        .bind(dev_seed::SAMPLE_REVISION)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE work_publications SET generation=4,intent_revision_id=$2,intent_generation=4,pointer_revision_id=$2,pointer_intent_generation=4 WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK)
        .bind(dev_seed::SAMPLE_REVISION)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        page(&vink)
            .await
            .publication
            .unwrap()
            .current_public
            .unwrap()
            .tham,
        Some(1)
    );
    // Production on/off and exact replay cannot add a second count.
    use vink_app::application::endorsement::SetWorkEndorsement;
    let off = SetWorkEndorsement {
        work_id: sample_work(),
        revision: dev_seed::SAMPLE_REVISION.to_string(),
        command_key: Uuid::now_v7().to_string(),
        expected_order: 2,
        endorsed: false,
    };
    vink.set_work_endorsement(other(), off.clone())
        .await
        .unwrap();
    vink.set_work_endorsement(other(), off).await.unwrap();
    assert_eq!(
        page(&vink)
            .await
            .publication
            .unwrap()
            .current_public
            .unwrap()
            .tham,
        Some(0)
    );
    vink.set_work_endorsement(
        other(),
        SetWorkEndorsement {
            work_id: sample_work(),
            revision: dev_seed::SAMPLE_REVISION.to_string(),
            command_key: Uuid::now_v7().to_string(),
            expected_order: 3,
            endorsed: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        page(&vink)
            .await
            .publication
            .unwrap()
            .current_public
            .unwrap()
            .tham,
        Some(1)
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE work_publications SET audience='private',intent_revision_id=NULL,intent_generation=NULL,pointer_revision_id=NULL,pointer_intent_generation=NULL WHERE work_id=$1").bind(dev_seed::SAMPLE_WORK).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let p = page(&vink).await.publication.unwrap();
    assert_eq!(p.audience, "private");
    assert!(p.current_public.is_none());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_work_query_plans_and_fixed_statement_count(pool: PgPool) {
    seeded(&pool).await;
    for (owner, n) in [
        (dev_seed::SAMPLE_AUTHOR, 10000_i64),
        (dev_seed::OTHER_PRINCIPAL, 100),
    ] {
        sqlx::query("INSERT INTO works(id,owner_id) SELECT md5($1::uuid::text||n)::uuid,$1 FROM generate_series(1,$2) n").bind(owner).bind(n).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO working_drafts(work_id,version,title,document) SELECT id,1,'Đường về '||id,'{\"schema_version\":1,\"blocks\":[]}' FROM works WHERE owner_id=$1 ON CONFLICT DO NOTHING").bind(owner).execute(&pool).await.unwrap();
    }
    let thread = Uuid::now_v7();
    sqlx::query("INSERT INTO discussion_threads(id,work_id,comments_open) VALUES($1,$2,false)")
        .bind(thread)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO comments(id,thread_id,kind,replied_to_reply,author_id,body,review,moderation_version,admission_policy,admission_reason) SELECT md5('skew-'||n)::uuid,$1,'root',false,$2,'plan fixture','pending',1,'premoderation.v1','policy_premoderation' FROM generate_series(1,10000) n").bind(thread).bind(dev_seed::OTHER_PRINCIPAL).execute(&pool).await.unwrap();
    sqlx::query("UPDATE comments SET review='approved',moderation_version=2 WHERE thread_id=$1")
        .bind(thread)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("ANALYZE works; ANALYZE working_drafts; ANALYZE comments;")
        .execute(&pool)
        .await
        .unwrap();
    println!(
        "#214 corpus: 100 / 10001 owned works; same timestamps, 10000 comments concentrated on one public work, selected page=24+1; PostgreSQL {}",
        sqlx::query_scalar::<_, String>("SELECT version()")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    for owner in [dev_seed::SAMPLE_AUTHOR, dev_seed::OTHER_PRINCIPAL] {
        for sort in [DraftSort::Recent, DraftSort::Title] {
            let sql = format!(
                "EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {}",
                vink_app::persistence::author_page_sql(sort)
            );
            let query = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql))
                .bind(owner)
                .bind("");
            let query = if sort == DraftSort::Recent {
                query.bind(None::<time::OffsetDateTime>)
            } else {
                query.bind(None::<String>)
            };
            let plan = query
                .bind(None::<Uuid>)
                .bind(25_i64)
                .fetch_all(&pool)
                .await
                .unwrap();
            println!("{owner} {sort:?}\n{}", plan.join("\n"));
        }
    }
    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {}",
        vink_app::persistence::AUTHOR_METRICS_SQL
    );
    let plan = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql))
        .bind(vec![dev_seed::SAMPLE_WORK])
        .bind(vec![dev_seed::SAMPLE_REVISION])
        .fetch_all(&pool)
        .await
        .unwrap();
    println!("METRICS\n{}", plan.join("\n"));
    let app = app(&pool).await;
    let (status, _, body) = call(
        &app,
        "GET",
        "/api/v1/author/works?q=mua+tren+pho+cu",
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        json(&body)["works"][0]["publication"]["current_public"]["comments"],
        10000
    );
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/api/v1/works/{}/discussion/public", dev_seed::SAMPLE_WORK),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(json(&body)["roots"].as_array().unwrap().len() < 10000);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_work_metrics_failure_preserves_list(pool: PgPool) {
    let vink = seeded(&pool).await;
    // The page query still succeeds when the independent metric relation is unavailable.
    sqlx::query("ALTER TABLE work_endorsements RENAME TO unavailable_work_endorsements")
        .execute(&pool)
        .await
        .unwrap();
    let result = page(&vink).await;
    let public = result.publication.unwrap().current_public.unwrap();
    assert_eq!(public.revision_id.as_uuid(), dev_seed::SAMPLE_REVISION);
    assert!(public.tham.is_none() && public.comments.is_none());
    assert!(result.draft_version.is_some());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
#[ignore = "measurement: requires VINK_PG_QUERY_LOG and superuser SET log_statement"]
async fn author_work_statement_count_measurement(pool: PgPool) {
    seeded(&pool).await;
    sqlx::query("INSERT INTO works(id,owner_id,created_at) SELECT md5('batch-'||n)::uuid,$1,'2020-01-01' FROM generate_series(1,120) n")
        .bind(dev_seed::SAMPLE_AUTHOR).execute(&pool).await.unwrap();
    // Vary the number of PUBLIC anchors with page size, so a per-public-card query loop
    // would be observable (not just one public work surrounded by private rows).
    sqlx::query("INSERT INTO revisions(id,work_id,title,document,content_hash,created_by) SELECT md5('batch-revision-'||n)::uuid,md5('batch-'||n)::uuid,'Batch public '||n,'{\"schema_version\":1,\"blocks\":[]}',repeat('a',64),$1 FROM generate_series(1,120) n")
        .bind(dev_seed::SAMPLE_AUTHOR).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_publications(work_id,audience,lifecycle,generation,intent_revision_id,intent_generation,pointer_revision_id,pointer_intent_generation) SELECT md5('batch-'||n)::uuid,'public','active',2,md5('batch-revision-'||n)::uuid,2,md5('batch-revision-'||n)::uuid,2 FROM generate_series(1,120) n")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO publications(work_id,revision_id,generation,published_at) SELECT md5('batch-'||n)::uuid,md5('batch-revision-'||n)::uuid,2,'2020-01-01' FROM generate_series(1,120) n")
        .execute(&pool).await.unwrap();
    let logged = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    sqlx::query("SET log_statement = 'all'")
        .execute(&logged)
        .await
        .unwrap();
    let log_path = std::env::var("VINK_PG_QUERY_LOG").expect("PostgreSQL server log path required");
    let vink = Vink::new(PgStore::new(logged.clone()));
    for size in [1, 24, 100] {
        let start = format!("SELECT 'VINK214_START_{size}'");
        sqlx::query(sqlx::AssertSqlSafe(start))
            .execute(&logged)
            .await
            .unwrap();
        let result = vink
            .search_author_works(
                author(),
                SearchDrafts {
                    limit: Some(size.to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result.works.len(), size);
        assert!(
            result
                .works
                .iter()
                .all(|w| w.publication.as_ref().unwrap().current_public.is_some())
        );
        let end = format!("SELECT 'VINK214_END_{size}'");
        sqlx::query(sqlx::AssertSqlSafe(end))
            .execute(&logged)
            .await
            .unwrap();
        let raw = std::fs::read_to_string(&log_path).unwrap();
        let segment = raw
            .rsplit_once(&format!("VINK214_START_{size}"))
            .unwrap()
            .1
            .split(&format!("VINK214_END_{size}"))
            .next()
            .unwrap();
        let pages = segment.matches("WITH candidates AS").count();
        let metrics = segment.matches("WITH anchors AS").count();
        println!(
            "PostgreSQL server log: page_size={size}, page SELECTs={pages}, metrics SELECTs={metrics}"
        );
        assert_eq!((pages, metrics), (1, 1), "{segment}");
    }
}
