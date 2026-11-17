use super::*;
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;
use vink_app::application::drafts::SearchDrafts;
use vink_app::domain::draft_query::{DraftQueryError, search_key, title_key};

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_unicode_tables_match_pinned_rust_and_postgres(pool: PgPool) {
    // Every scalar affected by NFD, mark stripping, per-character lowercase or whitespace.
    // This catches a Unicode-table/toolchain/DB upgrade changing comparison semantics.
    let inputs: Vec<String> = (1..=0x10ffff)
        .filter_map(char::from_u32)
        .filter_map(|c| {
            let raw = c.to_string();
            (search_key(&raw) != raw).then_some(raw)
        })
        .collect();
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT value, vink_desk_key(value) FROM unnest($1::text[]) value")
            .bind(&inputs)
            .fetch_all(&pool)
            .await
            .unwrap();
    let differences: Vec<_> = rows
        .into_iter()
        .filter(|(raw, key)| search_key(raw) != *key)
        .collect();
    assert!(
        differences.is_empty(),
        "Unicode normalization drift (first 12): {:?}",
        &differences[..differences.len().min(12)]
    );
}

async fn corpus(pool: &PgPool, count: i64, owner: Uuid, prefix: &str) {
    sqlx::query("INSERT INTO works(id, owner_id) SELECT md5($1 || n)::uuid, $2 FROM generate_series(1,$3::bigint) n")
        .bind(prefix).bind(owner).bind(count).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO working_drafts(work_id, version, title, document, updated_at)
        SELECT md5($1 || n)::uuid, 1,
          CASE WHEN n = 1 THEN 'Bản cũ duy nhất' WHEN n % 11 = 0 THEN ''
               WHEN n % 7 = 0 THEN 'ĐƯỜNG   VỀ' WHEN n % 3 = 0 THEN 'đường về'
               ELSE 'Trang ' || lpad(n::text, 5, '0') END,
          '{\"format\":\"default\",\"source\":\"PRIVATE-MANUSCRIPT-SENTINEL\"}'::jsonb,
          '2026-01-01'::timestamptz + (n / 3) * interval '1 second'
        FROM generate_series(1,$2::bigint) n",
    )
    .bind(prefix)
    .bind(count)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_paging_searches_all_owned_titles_with_exact_order_and_ties(pool: PgPool) {
    let vink = seeded(&pool).await;
    corpus(&pool, 175, dev_seed::SAMPLE_AUTHOR, "draft-owner-").await;
    corpus(&pool, 30, dev_seed::OTHER_PRINCIPAL, "draft-other-").await;
    let legacy = vink.list_drafts(author()).await.unwrap();
    assert_eq!(legacy.len(), 100);
    assert!(!legacy.iter().any(|d| d.title.as_str() == "Bản cũ duy nhất"));
    let found = vink
        .search_drafts(
            author(),
            SearchDrafts {
                query: Some("ban cu duy nhat".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(found.drafts.len(), 1);
    assert_eq!(found.drafts[0].title.as_str(), "Bản cũ duy nhất");
    assert!(found.next_cursor.is_none());
    let rows: Vec<(Uuid, String, time::OffsetDateTime)> = sqlx::query_as("SELECT d.work_id, d.title, d.updated_at FROM working_drafts d JOIN works w ON w.id=d.work_id WHERE w.owner_id=$1")
        .bind(dev_seed::SAMPLE_AUTHOR).fetch_all(&pool).await.unwrap();
    for sort in ["recent", "title"] {
        for query in ["", "  ĐƯỜNG\u{a0}về ", "chua dat ten", "KHÔNG CÓ", "%_"] {
            let mut expected: Vec<_> = rows
                .iter()
                .filter(|(_, title, _)| {
                    let display = if title.is_empty() {
                        "Chưa đặt tên"
                    } else {
                        title.as_str()
                    };
                    vink_web::search::search_key(display)
                        .contains(&vink_web::search::search_key(query))
                })
                .collect();
            expected.sort_by(|a, b| {
                if sort == "recent" {
                    b.2.cmp(&a.2).then(a.0.cmp(&b.0))
                } else {
                    vink_web::search::search_key(if a.1.is_empty() {
                        "Chưa đặt tên"
                    } else {
                        &a.1
                    })
                    .cmp(&vink_web::search::search_key(if b.1.is_empty() {
                        "Chưa đặt tên"
                    } else {
                        &b.1
                    }))
                    .then(a.0.cmp(&b.0))
                }
            });
            let mut seen = Vec::new();
            let mut cursor = None;
            loop {
                let page = vink
                    .search_drafts(
                        author(),
                        SearchDrafts {
                            query: Some(query.into()),
                            sort: Some(sort.into()),
                            limit: Some("7".into()),
                            cursor,
                        },
                    )
                    .await
                    .unwrap();
                assert!(page.drafts.len() <= 7);
                assert!(!page.drafts.is_empty() || page.next_cursor.is_none());
                seen.extend(page.drafts.into_iter().map(|d| d.work_id.as_uuid()));
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
                assert!(seen.len() <= rows.len());
            }
            assert_eq!(
                seen,
                expected.iter().map(|r| r.0).collect::<Vec<_>>(),
                "{sort}/{query}"
            );
            assert_eq!(seen.iter().collect::<HashSet<_>>().len(), seen.len());
        }
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn live_draft_moves_require_a_fresh_head_in_both_orders(pool: PgPool) {
    let vink = seeded(&pool).await;
    corpus(&pool, 175, dev_seed::SAMPLE_AUTHOR, "moving-draft-").await;
    for sort in ["recent", "title"] {
        let request = SearchDrafts {
            sort: Some(sort.into()),
            limit: Some("7".into()),
            ..Default::default()
        };
        let head = vink.search_drafts(author(), request.clone()).await.unwrap();
        let second = vink
            .search_drafts(
                author(),
                SearchDrafts {
                    cursor: head.next_cursor.clone(),
                    ..request.clone()
                },
            )
            .await
            .unwrap();
        let moved = second.drafts.last().unwrap().work_id;
        // A writer on another connection moves a not-yet-seen item before the cursor.
        sqlx::query("UPDATE working_drafts SET title='000 moved before cursor', updated_at=clock_timestamp(), version=version+1 WHERE work_id=$1")
            .bind(moved.as_uuid()).execute(&pool).await.unwrap();
        let continuation = vink
            .search_drafts(
                author(),
                SearchDrafts {
                    cursor: head.next_cursor,
                    ..request.clone()
                },
            )
            .await
            .unwrap();
        assert!(!continuation.drafts.iter().any(|d| d.work_id == moved));
        let restarted = vink.search_drafts(author(), request).await.unwrap();
        assert!(restarted.drafts.iter().any(|d| d.work_id == moved));
        assert_ne!(restarted.next_cursor, continuation.next_cursor);
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_cursor_never_supplies_authority_and_http_preserves_legacy_privacy(pool: PgPool) {
    let app = app(&pool).await;
    corpus(&pool, 175, dev_seed::SAMPLE_AUTHOR, "http-draft-").await;
    let author = Some(dev_seed::SAMPLE_AUTHOR);
    let (status, cache, body) = call(&app, "GET", "/api/v1/drafts", author, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let old: vink_web::contract::DraftList = serde_json::from_str(&body).unwrap();
    assert_eq!(old.drafts.len(), 100);
    assert!(!body.contains("document") && !body.contains("PRIVATE-MANUSCRIPT"));
    let cursor = json(&body)["next_cursor"].as_str().unwrap().to_string();
    for (path, actor, field, code) in [
        (
            format!("/api/v1/drafts?cursor={cursor}"),
            Some(dev_seed::OTHER_PRINCIPAL),
            "cursor",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            format!("/api/v1/drafts?cursor={cursor}"),
            None,
            "",
            StatusCode::UNAUTHORIZED,
        ),
        (
            format!("/api/v1/drafts?sort=title&cursor={cursor}"),
            author,
            "cursor",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            format!("/api/v1/drafts?q=missing&cursor={cursor}"),
            author,
            "cursor",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            format!("/api/v1/drafts?limit=10&cursor={cursor}"),
            author,
            "cursor",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "/api/v1/drafts?owner_id=someone".into(),
            author,
            "query",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "/api/v1/drafts?q=%ff".into(),
            author,
            "query",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "/api/v1/drafts?q=%00".into(),
            author,
            "q",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "/api/v1/drafts?q=a&q=b".into(),
            author,
            "query",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "/api/v1/drafts?limit=101".into(),
            author,
            "limit",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "/api/v1/drafts?cursor=01".into(),
            author,
            "cursor",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let (status, cache, body) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(status, code, "{path}: {body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
        if !field.is_empty() {
            assert_eq!(json(&body)["error"]["field"], field);
        }
        assert!(!body.contains(&cursor));
    }
    let (_, cache, body) = call(
        &app,
        "GET",
        "/api/v1/drafts",
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(json(&body)["drafts"], json!([]));
    assert!(json(&body).get("next_cursor").is_none());
    let (_, _, body) = call(&app, "GET", "/ban-viet", author, None, None).await;
    assert!(!body.contains("Bản cũ duy nhất") && !body.contains("PRIVATE-MANUSCRIPT"));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_keys_match_web_oracle_and_backfill_or_change_atomically(pool: PgPool) {
    let vink = seeded(&pool).await;
    let alphabet =
        "aăâbcdđeêghiklmnoôơpqrstuưvxyáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ";
    let mut texts = vec![
        String::new(),
        "A_%—B".into(),
        "ΟΣ İ ß ẞ".into(),
        "a\u{345}\u{1ab0}z".into(),
        "\t \n\u{a0}\u{2003}đường\u{202f}về  ".into(),
    ];
    for c in alphabet.chars() {
        for s in [c.to_string(), c.to_uppercase().collect::<String>()] {
            texts.push(s.nfd().collect());
            texts.push(s);
        }
    }
    for text in texts {
        let key: String = sqlx::query_scalar("SELECT vink_desk_key($1)")
            .bind(&text)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(key, vink_web::search::search_key(&text), "{text:?}");
        assert_eq!(key, search_key(&text));
    }
    // Apply the new migration to an existing corpus, rather than only an empty schema.
    sqlx::raw_sql(
        "ALTER TABLE working_drafts DROP COLUMN desk_title_key; DROP FUNCTION vink_desk_key(text)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20261120000000_draft_search.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let loaded = vink.load_draft(author(), sample_work()).await.unwrap();
    let backfilled: String =
        sqlx::query_scalar("SELECT desk_title_key FROM working_drafts WHERE work_id=$1")
            .bind(sample_work().as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(backfilled, title_key(loaded.content.title().as_str()));
    let saved = vink
        .save_draft(
            author(),
            save(
                sample_work(),
                loaded.version.get(),
                "ĐƯỜNG  mới",
                doc(&[&["riêng"]]),
            ),
        )
        .await
        .unwrap();
    let key: String =
        sqlx::query_scalar("SELECT desk_title_key FROM working_drafts WHERE work_id=$1")
            .bind(sample_work().as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(key, title_key("ĐƯỜNG  mới"));
    assert!(
        vink.save_draft(
            author(),
            save(sample_work(), loaded.version.get(), "STALE", doc(&[]))
        )
        .await
        .is_err()
    );
    let newest = vink
        .search_drafts(
            author(),
            SearchDrafts {
                limit: Some("1".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(newest.drafts[0].version, saved.version);
    assert_eq!(newest.drafts[0].title.as_str(), "ĐƯỜNG  mới");
    sqlx::query("UPDATE working_drafts SET title=$2 WHERE work_id=$1")
        .bind(sample_work().as_uuid())
        .bind(" \u{a0}\u{2003} ")
        .execute(&pool)
        .await
        .unwrap();
    let blank = vink
        .search_drafts(
            author(),
            SearchDrafts {
                query: Some("chua dat ten".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(
        blank
            .drafts
            .iter()
            .any(|draft| draft.work_id == sample_work() && draft.title.is_empty())
    );
    let bad = vink
        .search_drafts(
            None,
            SearchDrafts {
                limit: Some("0".into()),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(bad, Err(DraftError::Unauthenticated));
    assert_eq!(
        vink.search_drafts(
            author(),
            SearchDrafts {
                limit: Some("0".into()),
                ..Default::default()
            }
        )
        .await,
        Err(DraftError::Query(DraftQueryError::Limit))
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_query_and_cursor_do_not_enter_debug_request_traces(pool: PgPool) {
    use std::io::Write;
    use std::sync::Mutex;
    use tracing::instrument::WithSubscriber;
    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let app = app(&pool).await;
    let buffer = Buffer(Arc::new(Mutex::new(Vec::new())));
    let writer = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _ = call(
        &app,
        "GET",
        "/api/v1/drafts?q=PRIVATE-QUERY-SENTINEL&cursor=PRIVATE-CURSOR-SENTINEL",
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .with_subscriber(subscriber)
    .await;
    let logs = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("/api/v1/drafts"),
        "trace was actually captured"
    );
    assert!(!logs.contains("PRIVATE-QUERY") && !logs.contains("PRIVATE-CURSOR"));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
#[ignore = "measurement; run explicitly with --ignored --nocapture"]
async fn draft_query_plans(pool: PgPool) {
    seeded(&pool).await;
    corpus(&pool, 20_000, dev_seed::SAMPLE_AUTHOR, "large-owner-").await;
    corpus(&pool, 150, dev_seed::OTHER_PRINCIPAL, "small-owner-").await;
    // One large neighbouring owner makes isolation/skew observable in the actual plans.
    let neighbour = Uuid::from_u128(123);
    sqlx::query("INSERT INTO principals(id,display_name) VALUES ($1,'Neighbour')")
        .bind(neighbour)
        .execute(&pool)
        .await
        .unwrap();
    corpus(&pool, 80_000, neighbour, "neighbour-").await;
    sqlx::raw_sql("ANALYZE works; ANALYZE working_drafts;")
        .execute(&pool)
        .await
        .unwrap();
    let version: String = sqlx::query_scalar("SELECT version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    println!(
        "{version}\nCORPUS: 150 / 20,001 / 80,000 drafts, 3 owners, equal timestamps and folded titles; page=24 (+1)."
    );
    for (label, owner) in [
        ("small", dev_seed::OTHER_PRINCIPAL),
        ("large", dev_seed::SAMPLE_AUTHOR),
    ] {
        for (sort, sql) in [
            ("recent", vink_app::persistence::DRAFT_RECENT_SQL),
            ("title", vink_app::persistence::DRAFT_TITLE_SQL),
        ] {
            for q in ["", "ban cu duy nhat", "duong ve", "missing"] {
                let statement = format!("EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {sql}");
                let query =
                    sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(statement.as_str()))
                        .bind(owner)
                        .bind(q);
                let query = if sort == "recent" {
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
                println!("\nPLAN {label} {sort} q={q:?}\n{}", plan.join("\n"));
            }
        }
    }
    // Bounded experiment for CHOICE-02: compute the SAME key at read time versus the
    // generated key. Only static SQL fragments are substituted; all input is still bound.
    let computed = vink_app::persistence::DRAFT_TITLE_SQL.replace(
        "d.desk_title_key",
        "vink_desk_key(CASE WHEN d.title = '' THEN 'Chưa đặt tên' ELSE d.title END)",
    );
    let statement = format!("EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {computed}");
    let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
        .bind(dev_seed::SAMPLE_AUTHOR)
        .bind("")
        .bind(None::<String>)
        .bind(None::<Uuid>)
        .bind(25_i64)
        .fetch_all(&pool)
        .await
        .unwrap();
    println!(
        "\nPLAN large title computed-key candidate\n{}",
        plan.join("\n")
    );
    for (sort, sql) in [
        ("recent", vink_app::persistence::DRAFT_RECENT_SQL),
        ("title", vink_app::persistence::DRAFT_TITLE_SQL),
    ] {
        let statement = format!("EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {sql}");
        let query = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(statement))
            .bind(dev_seed::SAMPLE_AUTHOR)
            .bind("");
        let query = if sort == "recent" {
            query.bind(time::macros::datetime!(2026-01-01 00:55:33 UTC))
        } else {
            query.bind("trang 10000")
        };
        let plan = query
            .bind(Uuid::nil())
            .bind(25_i64)
            .fetch_all(&pool)
            .await
            .unwrap();
        println!("\nPLAN large {sort} middle cursor\n{}", plan.join("\n"));
    }
}
