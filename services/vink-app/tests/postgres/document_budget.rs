//! #229: opt-in measurements, never a production migration or a latency assertion.
//! Reuse SQLx's disposable database, the production application/store/router and CDP harness.
use super::*;
use sqlx::types::Json;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;
use vink_app::domain::document::MAX_DOCUMENT_CHARS;
use vink_app::domain::revision::ContentHash;
use vink_web::public_persistence::Envelope;
use vink_web::public_reads::{PublicKey, Snapshot};

#[path = "../support/document_budget.rs"]
mod corpus;

fn measured<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let value = black_box(f());
    (value, start.elapsed().as_secs_f64() * 1_000.0)
}

fn write_json(dir: &Path, name: &str, value: &Value) {
    std::fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn pure_measurements(data: &DocumentData, title: &str) -> Value {
    let normalized = Document::parse(data.clone()).unwrap();
    let canonical = normalized.to_data();
    let title = Title::parse(title).unwrap();
    let raw_json = serde_json::to_vec(data).unwrap();
    let encoded = serde_json::to_vec(&canonical).unwrap();
    let request = json!({"expected_version":1,"title":title.as_str(),"document":data});
    let wire = serde_json::to_vec(&request).unwrap();
    let _: vink_app::http::contract::SaveDraftRequest = serde_json::from_slice(&wire).unwrap();
    let mut times = Vec::new();
    for _ in 0..5 {
        let cloned = data.clone();
        let (_, nfc) = measured(|| {
            data.blocks
                .iter()
                .flat_map(|b| &b.lines)
                .map(|s| s.nfc().collect::<String>())
                .collect::<Vec<_>>()
        });
        let (_, validate) = measured(|| Document::parse(cloned).unwrap());
        let (_, hash) = measured(|| ContentHash::of(&title, &normalized));
        let (_, serialize) = measured(|| serde_json::to_vec(&canonical).unwrap());
        let web: vink_web::contract::DocumentDto = serde_json::from_slice(&encoded).unwrap();
        let (source, source_ms) = measured(|| vink_web::document::document_to_text(&web));
        let (roundtrip, snapshot_ms) = measured(|| vink_web::document::text_to_document(&source));
        assert_eq!(roundtrip, web);
        times.push(json!({"nfc_ms":nfc,"validate_including_nfc_ms":validate,"hash_ms":hash,
            "serialize_ms":serialize,"document_to_source_ms":source_ms,"source_to_dto_ms":snapshot_ms}));
    }
    json!({"raw_line_scalars":data.blocks.iter().flat_map(|b| &b.lines).map(|s|s.chars().count()).sum::<usize>(),
        "nfc_line_scalars":canonical.blocks.iter().flat_map(|b| &b.lines).map(|s|s.chars().count()).sum::<usize>(),
        "raw_line_utf8_bytes":data.blocks.iter().flat_map(|b| &b.lines).map(String::len).sum::<usize>(),
        "nfc_line_utf8_bytes":canonical.blocks.iter().flat_map(|b| &b.lines).map(String::len).sum::<usize>(),
        "blocks":canonical.blocks.len(),"lines":canonical.blocks.iter().map(|b|b.lines.len()).sum::<usize>(),
        "raw_document_json_bytes":raw_json.len(),"canonical_document_json_bytes":encoded.len(),
        "save_request_bytes":wire.len(),"content_hash_v1":ContentHash::of(&title,&normalized).as_str(),
        "samples_ms":times})
}

async fn wal_position(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT pg_current_wal_insert_lsn()::text")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn wal_since(pool: &PgPool, start: &str) -> i64 {
    sqlx::query_scalar("SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(), $1::pg_lsn)::bigint")
        .bind(start)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn assert_sample_transition(before: &DocumentData, submitted: &DocumentData, changed: bool) {
    assert_eq!(
        before != submitted,
        changed,
        "mislabeled content transition"
    );
}

// Restore outside every timed/WAL window. Repeating the same edited payload otherwise turns
// samples 2..5 into content no-ops, even though the optimistic version still advances.
async fn app_save_samples(
    pool: &PgPool,
    vink: &Vink<PgStore>,
    work: WorkId,
    mut version: i64,
    title: &str,
    baseline: &DocumentData,
) -> (Vec<f64>, Vec<Value>, i64) {
    let size: usize = baseline
        .blocks
        .iter()
        .flat_map(|b| &b.lines)
        .map(|s| s.chars().count())
        .sum();
    let title_value = Title::parse(title).unwrap();
    let before_hash = ContentHash::of(&title_value, &Document::parse(baseline.clone()).unwrap());
    let mut reads = Vec::new();
    let mut saves = Vec::new();
    for (kind, at) in [
        ("noop", None),
        ("begin", Some(0)),
        ("middle", Some(size / 2)),
        ("end", Some(size - 1)),
    ] {
        let mut edited = baseline.clone();
        if let Some(at) = at {
            corpus::edit(&mut edited, at);
        }
        let after_hash = ContentHash::of(&title_value, &Document::parse(edited.clone()).unwrap());
        for sample in 0..5 {
            let restored = vink
                .save_draft(author(), save(work, version, title, baseline.clone()))
                .await
                .unwrap();
            version = restored.version.get();
            let start = Instant::now();
            let loaded = vink.load_draft(author(), work).await.unwrap();
            reads.push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(loaded.version.get(), version);
            assert_eq!(loaded.document.to_data(), *baseline);
            assert_sample_transition(&loaded.document.to_data(), &edited, at.is_some());
            // Client-side input cloning, reset and verification are outside app/commit timing.
            let input = save(work, version, title, edited.clone());
            let wal = wal_position(pool).await;
            let start = Instant::now();
            let saved = vink.save_draft(author(), input).await.unwrap();
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            let wal_bytes = wal_since(pool, &wal).await;
            assert_eq!(saved.version.get(), version + 1);
            version = saved.version.get();
            let durable = vink.load_draft(author(), work).await.unwrap();
            assert_eq!(durable.version.get(), version);
            assert_eq!(durable.title, title_value);
            assert_eq!(durable.document.to_data(), edited);
            saves.push(
                json!({"kind":kind,"sample":sample,"ms":elapsed,"wal_bytes":wal_bytes,
                "before_hash_v1":before_hash.as_str(),"after_hash_v1":after_hash.as_str(),
                "durable_exact_match":true}),
            );
        }
    }
    (reads, saves, version)
}

#[test]
#[should_panic(expected = "mislabeled content transition")]
fn long_document_sample_rejects_repeated_edit_labeled_as_change() {
    let mut edited = corpus::corpus("poem_tabs", 30_000);
    corpus::edit(&mut edited, 15_000);
    assert_sample_transition(&edited, &edited, true);
}

#[test]
#[should_panic(expected = "mislabeled content transition")]
fn long_document_sample_rejects_change_labeled_as_noop() {
    let baseline = corpus::corpus("poem_tabs", 30_000);
    let mut edited = baseline.clone();
    corpus::edit(&mut edited, 15_000);
    assert_sample_transition(&baseline, &edited, false);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn long_document_samples_are_independent_durable_transitions(pool: PgPool) {
    let vink = seeded(&pool).await;
    for shape in ["poem_tabs", "tiny_blocks"] {
        let draft = vink.create_draft(author()).await.unwrap();
        let baseline = Document::parse(corpus::corpus(shape, 30_000))
            .unwrap()
            .to_data();
        let (reads, saves, version) =
            app_save_samples(&pool, &vink, draft.work_id, 1, "[Mẫu #229]", &baseline).await;
        assert_eq!(reads.len(), 20);
        assert_eq!(saves.len(), 20);
        assert_eq!(version, 41); // one unmeasured reset + one measured save per sample
        for sample in saves {
            assert_eq!(
                sample["before_hash_v1"] == sample["after_hash_v1"],
                sample["kind"] == "noop"
            );
            assert_eq!(sample["durable_exact_match"], true);
        }
    }
}

async fn relations(pool: &PgPool) -> Value {
    // Heap excludes TOAST and all indexes. Table includes TOAST; total includes every index.
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT c.relname, pg_relation_size(c.oid),
                CASE WHEN c.reltoastrelid=0 THEN 0 ELSE pg_total_relation_size(c.reltoastrelid) END,
                pg_indexes_size(c.oid), pg_total_relation_size(c.oid),
                COALESCE(s.n_dead_tup,0)
           FROM pg_class c LEFT JOIN pg_stat_user_tables s ON s.relid=c.oid
          WHERE c.oid IN ('working_drafts'::regclass,'revisions'::regclass) ORDER BY c.relname",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    json!(rows.into_iter().map(|(table,heap,toast,index,total,dead)|
        json!({"table":table,"heap_bytes":heap,"toast_total_bytes":toast,"main_index_bytes":index,
            "total_bytes":total,"estimated_dead_tuples":dead})).collect::<Vec<_>>())
}

async fn plan(pool: &PgPool, work: WorkId, document: &DocumentData) -> Value {
    let read: Json<Value> = sqlx::query_scalar(
        "EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)
         SELECT d.work_id,d.version,d.title,d.document,d.updated_at FROM working_drafts d
         JOIN works w ON w.id=d.work_id WHERE d.work_id=$1 AND w.owner_id=$2",
    )
    .bind(work.as_uuid())
    .bind(dev_seed::SAMPLE_AUTHOR)
    .fetch_one(pool)
    .await
    .unwrap();
    // Diagnostic statement matches save_draft's conditional UPDATE; rollback prevents a probe
    // from changing the version. App timings below include the standing fence and COMMIT too.
    let mut tx = pool.begin().await.unwrap();
    let update: Json<Value> = sqlx::query_scalar(
        "EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)
         UPDATE working_drafts d SET title=$4, document=$5, version=d.version+1,updated_at=now()
         FROM works w WHERE w.id=d.work_id AND d.work_id=$1 AND w.owner_id=$2 AND d.version=$3
         RETURNING d.version,d.updated_at",
    )
    .bind(work.as_uuid())
    .bind(dev_seed::SAMPLE_AUTHOR)
    .bind(2i64)
    .bind("[Mẫu #229]")
    .bind(Json(document))
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    json!({"read":read.0,"update_rolled_back":update.0})
}

async fn compression_probe(pool: &PgPool, fixtures: &[(String, DocumentData)]) -> Value {
    let mut results = Vec::new();
    for (method, alter) in [
        (
            "default",
            "ALTER TABLE budget_toast_probe ALTER COLUMN document SET COMPRESSION default",
        ),
        (
            "pglz",
            "ALTER TABLE budget_toast_probe ALTER COLUMN document SET COMPRESSION pglz",
        ),
        (
            "lz4",
            "ALTER TABLE budget_toast_probe ALTER COLUMN document SET COMPRESSION lz4",
        ),
    ] {
        // Fresh, identically ordered data for every method. This is an SQL compression probe,
        // separate from the production PgStore path, on a table in SQLx's scratch database.
        sqlx::query("DROP TABLE IF EXISTS budget_toast_probe")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE budget_toast_probe (id int PRIMARY KEY, document jsonb NOT NULL)",
        )
        .execute(pool)
        .await
        .unwrap();
        if let Err(e) = sqlx::query(alter).execute(pool).await {
            assert_eq!(method, "lz4", "unexpected compression failure: {e}");
            let unsupported = e.as_database_error().is_some_and(|e| {
                e.code().as_deref() == Some("0A000") && e.message().contains("lz4")
            });
            assert!(
                unsupported,
                "LZ4 failed for a reason other than unsupported build: {e}"
            );
            results.push(json!({"method":method,"status":"UNSUPPORTED"}));
            continue;
        }
        let mut samples = Vec::new();
        for (id, (name, data)) in fixtures.iter().enumerate() {
            let wal = wal_position(pool).await;
            let start = Instant::now();
            sqlx::query("INSERT INTO budget_toast_probe VALUES ($1,$2)")
                .bind(id as i32)
                .bind(Json(data))
                .execute(pool)
                .await
                .unwrap();
            let insert_ms = start.elapsed().as_secs_f64() * 1000.;
            let insert_wal = wal_since(pool, &wal).await;
            let mut reads = Vec::new();
            let mut updates = Vec::new();
            let mut update_wal = Vec::new();
            for iteration in 0..5 {
                let start = Instant::now();
                let read: Json<DocumentData> =
                    sqlx::query_scalar("SELECT document FROM budget_toast_probe WHERE id=$1")
                        .bind(id as i32)
                        .fetch_one(pool)
                        .await
                        .unwrap();
                reads.push(start.elapsed().as_secs_f64() * 1000.);
                assert_eq!(&read.0, data);
                let mut changed = data.clone();
                corpus::edit(&mut changed, iteration);
                let wal = wal_position(pool).await;
                let start = Instant::now();
                sqlx::query("UPDATE budget_toast_probe SET document=$2 WHERE id=$1")
                    .bind(id as i32)
                    .bind(Json(&changed))
                    .execute(pool)
                    .await
                    .unwrap();
                updates.push(start.elapsed().as_secs_f64() * 1000.);
                update_wal.push(wal_since(pool, &wal).await);
                // Restore using a client value, so each timed update starts from identical bytes.
                sqlx::query("UPDATE budget_toast_probe SET document=$2 WHERE id=$1")
                    .bind(id as i32)
                    .bind(Json(data))
                    .execute(pool)
                    .await
                    .unwrap();
            }
            let stored: (i32,Option<String>) = sqlx::query_as(
                "SELECT pg_column_size(document),pg_column_compression(document) FROM budget_toast_probe WHERE id=$1")
                .bind(id as i32).fetch_one(pool).await.unwrap();
            samples.push(
                json!({"case":name,"column_bytes":stored.0,"actual_compression":stored.1,
                "insert_ms":insert_ms,"insert_wal_bytes":insert_wal,"read_ms":reads,
                "update_ms":updates,"update_wal_bytes":update_wal}),
            );
        }
        let size: (i64,i64,i64,i64) = sqlx::query_as(
            "SELECT pg_relation_size(oid),pg_total_relation_size(reltoastrelid),pg_indexes_size(oid),pg_total_relation_size(oid)
             FROM pg_class WHERE oid='budget_toast_probe'::regclass").fetch_one(pool).await.unwrap();
        results.push(json!({"method":method,"status":"PASS","samples":samples,
            "heap_bytes":size.0,"toast_total_bytes":size.1,"main_index_bytes":size.2,"total_bytes":size.3}));
    }
    // ALTER only changes the policy; existing TOAST chunks retain their old compression.
    sqlx::query("DROP TABLE budget_toast_probe")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE budget_toast_probe (id int PRIMARY KEY,document jsonb COMPRESSION pglz)",
    )
    .execute(pool)
    .await
    .unwrap();
    let data = corpus::corpus("ascii_repeat", 200_000);
    sqlx::query("INSERT INTO budget_toast_probe VALUES (1,$1)")
        .bind(Json(&data))
        .execute(pool)
        .await
        .unwrap();
    if sqlx::query("ALTER TABLE budget_toast_probe ALTER COLUMN document SET COMPRESSION lz4")
        .execute(pool)
        .await
        .is_ok()
    {
        sqlx::query("INSERT INTO budget_toast_probe VALUES (2,$1)")
            .bind(Json(&data))
            .execute(pool)
            .await
            .unwrap();
        let rows: Vec<(i32, Option<String>)> = sqlx::query_as(
            "SELECT id,pg_column_compression(document) FROM budget_toast_probe ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(rows[0].1.as_deref(), Some("pglz"));
        assert_eq!(rows[1].1.as_deref(), Some("lz4"));
        results.push(json!({"existing_vs_new_after_alter":rows}));
    }
    json!(results)
}

/// Not ignored: durable boundaries, full valid content vs Axum's current 2 MiB transport cap.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn long_document_http_boundaries_preserve_durable_text_and_conflicts(pool: PgPool) {
    let app = app(&pool).await;
    let path = format!("/api/v1/drafts/{}", dev_seed::SAMPLE_WORK);
    let actor = Some(dev_seed::SAMPLE_AUTHOR);
    let mut version = 1;
    for size in [
        MAX_DOCUMENT_CHARS - 1,
        MAX_DOCUMENT_CHARS,
        MAX_DOCUMENT_CHARS + 1,
    ] {
        let input = json!({"expected_version":version,"title":"[Mẫu #229]","document":corpus::corpus("vi_nfd",size)});
        let (status, _, body) = call(&app, "PUT", &path, actor, Some(ORIGIN), Some(input)).await;
        if size > MAX_DOCUMENT_CHARS {
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(json(&body)["error"]["field"], "document");
        } else {
            assert_eq!(status, StatusCode::OK);
            version += 1;
        }
        let (_, _, loaded) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(json(&loaded)["version"], version);
        let doc: DocumentData = serde_json::from_value(json(&loaded)["document"].clone()).unwrap();
        let scalars = doc
            .blocks
            .iter()
            .flat_map(|b| &b.lines)
            .map(|s| s.chars().count())
            .sum::<usize>();
        assert_eq!(scalars, size.min(MAX_DOCUMENT_CHARS));
    }
    let (_, _, before) = call(&app, "GET", &path, actor, None, None).await;
    for (expected, document, status) in [
        (1, corpus::corpus("emoji", 200_000), StatusCode::CONFLICT),
        (
            version,
            corpus::corpus("tiny_blocks", 200_000),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let (actual, _, _) = call(
            &app,
            "PUT",
            &path,
            actor,
            Some(ORIGIN),
            Some(json!({
            "expected_version":expected,"title":"[Mẫu #229]","document":document})),
        )
        .await;
        assert_eq!(actual, status);
        let (_, _, after) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(json(&before), json(&after));
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
#[ignore = "opt-in benchmark: VINK_DOCUMENT_BUDGET_OUTPUT; see long-document-budget.md"]
async fn long_document_budget(pool: PgPool) {
    let out = PathBuf::from(
        std::env::var("VINK_DOCUMENT_BUDGET_OUTPUT").expect("output directory required"),
    );
    assert!(
        out.is_absolute(),
        "output must be an absolute path outside target"
    );
    std::fs::create_dir_all(&out).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let source = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .unwrap();
    let environment: (String,String,String)=sqlx::query_as(
        "SELECT version(),current_setting('default_toast_compression'),current_setting('shared_buffers')")
        .fetch_one(&pool).await.unwrap();
    let vink = seeded(&pool).await;
    let app = app(&pool).await;
    let mut cases = Vec::new();
    let mut fixtures = Vec::new();
    let mut browser = Vec::new();
    for size in [30_000, 100_000, 200_000] {
        for shape in corpus::SHAPES {
            let name = format!("{shape}-{size}");
            println!("budget {name}");
            let raw = corpus::corpus(shape, size);
            let doc = Document::parse(raw.clone()).unwrap().to_data();
            let title = format!("[Mẫu #229] {name}");
            let pure = pure_measurements(&raw, &title);
            let draft = vink.create_draft(author()).await.unwrap();
            let work = draft.work_id;
            let wal = wal_position(&pool).await;
            let start = Instant::now();
            vink.save_draft(author(), save(work, 1, &title, raw.clone()))
                .await
                .unwrap();
            let initial_save_ms = start.elapsed().as_secs_f64() * 1000.;
            let initial_wal = wal_since(&pool, &wal).await;
            let plans = plan(&pool, work, &doc).await;
            let (reads, saves, mut version) =
                app_save_samples(&pool, &vink, work, 2, &title, &doc).await;
            // Restore original corpus, then measure immutable snapshot history using the actual
            // capture use case (including metadata binding and receipts), not a fake INSERT loop.
            vink.save_draft(author(), save(work, version, &title, doc.clone()))
                .await
                .unwrap();
            version += 1;
            let mut histories = Vec::new();
            let history_limit = if size == 200_000 && ["vi_nfc", "tiny_blocks"].contains(&shape) {
                100
            } else {
                1
            };
            let mut first_revision = None;
            for n in 1..=history_limit {
                if n > 1 {
                    vink.save_draft(
                        author(),
                        save(work, version, &format!("{title} revision {n}"), doc.clone()),
                    )
                    .await
                    .unwrap();
                    version += 1;
                }
                let wal = wal_position(&pool).await;
                let start = Instant::now();
                let capture = vink
                    .capture_snapshot(author(), work, &format!("budget-{n:08}"), version, 1)
                    .await
                    .unwrap();
                let capture_ms = start.elapsed().as_secs_f64() * 1000.;
                let capture_wal = wal_since(&pool, &wal).await;
                first_revision.get_or_insert(capture.revision_id);
                if [1, 10, 100].contains(&n) {
                    let mut owner_reads = Vec::new();
                    for _ in 0..5 {
                        let start = Instant::now();
                        black_box(
                            vink.load_author_publication_state(author(), work)
                                .await
                                .unwrap(),
                        );
                        owner_reads.push(start.elapsed().as_secs_f64() * 1000.);
                    }
                    let stored: (i64,i64)=sqlx::query_as("SELECT count(*),sum(pg_column_size(document))::bigint FROM revisions WHERE work_id=$1")
                        .bind(work.as_uuid()).fetch_one(&pool).await.unwrap();
                    sqlx::query("SELECT pg_stat_force_next_flush()")
                        .execute(&pool)
                        .await
                        .unwrap();
                    sqlx::query("SELECT pg_stat_clear_snapshot()")
                        .execute(&pool)
                        .await
                        .unwrap();
                    histories.push(json!({"revision_count":stored.0,"revision_document_column_bytes":stored.1,
                        "capture_ms":capture_ms,"capture_wal_bytes":capture_wal,"owner_state_read_ms":owner_reads,
                        "cumulative_relations":relations(&pool).await}));
                }
            }
            // Test-only public fixture, like dev_seed. No evaluator or production publication
            // policy is enabled. The revision was captured by the production owner above.
            let revision = first_revision.unwrap();
            sqlx::query("INSERT INTO publications(work_id,revision_id,generation,published_at) VALUES($1,$2,1,now())")
                .bind(work.as_uuid()).bind(revision.as_uuid()).execute(&pool).await.unwrap();
            let path = format!("/api/v1/works/{work}/publication");
            let start = Instant::now();
            let (status, _, public) = call(&app, "GET", &path, None, None, None).await;
            assert_eq!(status, StatusCode::OK);
            let public_ms = start.elapsed().as_secs_f64() * 1000.;
            let publication: vink_web::contract::Publication =
                serde_json::from_str(&public).unwrap();
            let envelope = Envelope::from_origin(
                PublicKey::Reader(work.to_string()),
                &Snapshot::Publication(publication),
                ORIGIN.into(),
                10_000,
            )
            .unwrap();
            let envelope_bytes = serde_json::to_vec(&envelope).unwrap().len();
            assert_eq!(
                envelope.encode().is_some(),
                envelope_bytes <= vink_web::public_persistence::MAX_RECORD_BYTES
            );
            let start = Instant::now();
            let (status, _, html) =
                call(&app, "GET", &format!("/tac-pham/{work}"), None, None, None).await;
            assert_eq!(status, StatusCode::OK);
            let ssr_ms = start.elapsed().as_secs_f64() * 1000.;
            let (_, _, ack) = call(
                &app,
                "PUT",
                &format!("/api/v1/drafts/{work}"),
                Some(dev_seed::SAMPLE_AUTHOR),
                Some(ORIGIN),
                Some(json!({"expected_version":1,"title":title,"document":raw})),
            )
            .await;
            cases.push(json!({"case":name,"pure":pure,"initial_app_save_ms":initial_save_ms,"initial_wal_bytes":initial_wal,
                "app_load_ms":reads,"app_saves":saves,"explain":plans,"histories":histories,
                "publication_json_bytes":public.len(),"publication_router_ms":public_ms,"ssr_html_bytes":html.len(),
                "ssr_router_ms":ssr_ms,"cache_envelope_bytes":envelope_bytes,"cache_record_accepted":envelope.encode().is_some(),
                "stale_http_result":json(&ack)["error"]["code"]}));
            browser.push(json!({"case":name,"work_id":work.to_string(),"document":doc}));
            fixtures.push((name, Document::parse(raw).unwrap().to_data()));
            // Write partial evidence after every case so an interrupted browser does not erase PG evidence.
            write_json(
                &out,
                "postgres.json",
                &json!({"source_sha":String::from_utf8_lossy(&source.stdout).trim(),
                "seed":corpus::SEED,"build_profile":"dev","samples":5,"environment":environment,
                "measurement_version":2,
                "save_boundary":"App validation + PgStore commit; input clone, baseline reset and exact readback excluded",
                "cache_state":"warm process/PG; baseline reset before every sample, no OS cache eviction",
                "cases":cases}),
            );
        }
    }
    write_json(
        &out,
        "compression.json",
        &compression_probe(&pool, &fixtures).await,
    );
    write_json(&out, "fixtures.json", &json!(browser));
    if let Ok(chrome) = std::env::var("VINK_CHROME") {
        struct BudgetIdentity;
        impl IdentityResolver for BudgetIdentity {
            fn resolve<'a>(
                &'a self,
                _parts: &'a Parts,
                _peer: Option<SocketAddr>,
            ) -> ResolveFuture<'a> {
                Box::pin(std::future::ready(Resolution::Principal(author().unwrap())))
            }
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let origin = format!("http://{address}");
        let server_app = router(AppState::new(
            vink,
            Arc::new(BudgetIdentity),
            &origin,
            &root.join("target/site"),
        ));
        // SQLx tests use a current-thread runtime. Use a bounded multithread executor like the
        // App, so a large owner-state decode cannot block all unrelated HTTP requests in the
        // test harness merely because they share its one executor thread.
        let server_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let task = server_runtime.spawn(async move {
            axum::serve(
                listener,
                server_app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap()
        });
        let state = out.join("browser-state.json");
        let result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("node")
                .current_dir(root)
                .args([
                    "apps/vink-web/e2e/browser-smoke.mjs",
                    "--phase",
                    "document-budget",
                    "--base",
                    &origin,
                    "--chrome",
                    &chrome,
                    "--state",
                    state.to_str().unwrap(),
                ])
                .status()
                .unwrap()
        })
        .await
        .unwrap();
        task.abort();
        server_runtime.shutdown_background();
        assert!(
            result.success(),
            "browser benchmark failed; raw PG evidence retained"
        );
    } else {
        write_json(
            &out,
            "browser.json",
            &json!({"status":"NOT_RUN","reason":"set VINK_CHROME and build target/site first"}),
        );
    }
}
