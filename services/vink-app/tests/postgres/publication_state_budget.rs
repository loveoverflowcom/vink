//! #230 opt-in: same seeded corpus and real save/capture/owner read as #229.
use super::*;
use sqlx::types::Json;
use std::path::PathBuf;

use super::document_corpus as corpus;

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
#[ignore = "opt-in PostgreSQL measurement; VINK_PUBLICATION_STATE_OUTPUT required"]
async fn publication_state_budget(pool: PgPool) {
    let out = PathBuf::from(std::env::var("VINK_PUBLICATION_STATE_OUTPUT").unwrap());
    assert!(out.is_absolute());
    std::fs::create_dir_all(&out).unwrap();
    let vink = seeded(&pool).await;
    let app = app(&pool).await;
    let mut cases = Vec::new();
    for (shape, size, limit) in [
        ("vi_nfc", 30_000, 100),
        ("vi_nfc", 200_000, 100),
        ("tiny_blocks", 200_000, 100),
        ("vi_nfc", 300, 300),
    ] {
        let document = Document::parse(corpus::corpus(shape, size))
            .unwrap()
            .to_data();
        let work = vink.create_draft(author()).await.unwrap().work_id;
        let mut version = 1;
        for n in 1..=limit {
            let title = format!("[Mẫu #230] {shape} {size} {n:04}");
            vink.save_draft(author(), save(work, version, &title, document.clone()))
                .await
                .unwrap();
            version += 1;
            vink.capture_snapshot(author(), work, &format!("state-budget-{n:08}"), version, 1)
                .await
                .unwrap();
            if ![1, 10, 100, 300].contains(&n) {
                continue;
            }
            let path = format!("/api/v1/works/{work}/publication/state");
            let mut samples = Vec::new();
            // First sample is recorded separately; no claim of OS/PG cold cache.
            for _ in 0..4 {
                let start = Instant::now();
                let facts = vink
                    .load_author_publication_state(author(), work)
                    .await
                    .unwrap();
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(facts.revision_metadata.len(), n);
                samples.push(elapsed);
            }
            let (status, cache, body) = call(
                &app,
                "GET",
                &path,
                Some(dev_seed::SAMPLE_AUTHOR),
                None,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(cache.as_deref(), Some("no-store"));
            assert_eq!(json(&body)["revisions"].as_array().unwrap().len(), n);
            let bytes: (i64,i64,i64) = sqlx::query_as("SELECT count(*),sum(pg_column_size(document))::bigint,sum(octet_length(document::text))::bigint FROM revisions WHERE work_id=$1")
                .bind(work.as_uuid()).fetch_one(&pool).await.unwrap();
            let mut plans = Vec::new();
            for sql in [
                "EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) SELECT id,title,document,content_hash,metadata_snapshot_id,created_at FROM revisions WHERE work_id=$1 ORDER BY created_at,id",
                "EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) SELECT id,title,content_hash,metadata_snapshot_id,created_at FROM revisions WHERE work_id=$1 ORDER BY created_at,id",
            ] {
                let plan: Json<Value> = sqlx::query_scalar(sql)
                    .bind(work.as_uuid())
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                plans.push(plan.0);
            }
            cases.push(json!({"shape":shape,"scalars":size,"seed":corpus::SEED,"revisions":n,
                "document_json_bytes":serde_json::to_vec(&document).unwrap().len(),
                "stored_document_column_bytes":bytes.1,"document_text_bytes":bytes.2,
                "response_bytes":body.len(),"first_owner_state_ms":samples[0],"warm_owner_state_ms":&samples[1..],
                "explain_full_then_metadata":plans}));
            println!("state budget {shape}/{size}/{n}: {samples:?}");
            std::fs::write(
                out.join("measurements.json"),
                serde_json::to_vec_pretty(&json!({"cases":cases})).unwrap(),
            )
            .unwrap();
        }
    }
    // Explicitly exercise shared corpus edit utility too; the benchmark itself changes only title.
    let mut sanity = corpus::corpus(corpus::SHAPES[0], 10);
    corpus::edit(&mut sanity, 5);
    assert_eq!(sanity.source.chars().count(), 10);
}
