//! #231: compare storage footprints in SQLx's disposable DB. No runtime schema or API is used.
use super::{Document, DocumentData, PgPool, Title, document_corpus, json};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use std::collections::HashSet;
use std::time::Instant;
use vink_app::domain::{document::BlockData, revision::ContentHash};

const FIXED_TARGET: usize = 60 * 1024;
const MIN: usize = 32 * 1024;
const MAX: usize = 64 * 1024;

#[derive(Clone, Copy)]
enum Layout {
    Inline,
    Whole,
    Fixed,
    ContentDefined,
}

impl Layout {
    fn name(self) -> &'static str {
        match self {
            Self::Inline => "jsonb",
            Self::Whole => "whole_payload",
            Self::Fixed => "fixed_control",
            Self::ContentDefined => "block_content_defined",
        }
    }

    fn chunked(self) -> bool {
        matches!(self, Self::Fixed | Self::ContentDefined)
    }

    fn chunk_blocks(self, blocks: &[BlockData]) -> Vec<&[BlockData]> {
        let limit = if matches!(self, Self::Fixed) {
            FIXED_TARGET
        } else {
            MAX
        };
        let mut groups = Vec::new();
        let mut first = 0;
        let mut bytes = 2; // JSON array brackets
        for (index, block) in blocks.iter().enumerate() {
            let encoded = serde_json::to_vec(block).unwrap();
            if index > first && bytes + 1 + encoded.len() > limit {
                groups.push(&blocks[first..index]);
                first = index;
                bytes = 2;
            }
            bytes += usize::from(index > first) + encoded.len();
            // Stable candidate cuts based on one complete block, not editor positions/IDs.
            // Never split UTF-8, a line or a stanza. Oversized blocks are isolated.
            if bytes >= limit
                || (matches!(self, Self::ContentDefined)
                    && bytes >= MIN
                    && Sha256::digest(&encoded)[0] & 15 == 0)
            {
                groups.push(&blocks[first..=index]);
                first = index + 1;
                bytes = 2;
            }
        }
        if first < blocks.len() {
            groups.push(&blocks[first..]);
        }
        groups
    }
}

fn hash_payload(encoded: &str) -> Vec<u8> {
    Sha256::new()
        .chain_update(b"vink-rfc-231-json-payload-v1\0")
        .chain_update(encoded)
        .finalize()
        .to_vec()
}

#[test]
fn content_storage_boundary_shift() {
    // Exactly 40 equal-size blocks per greedy group including JSON separators.
    let blocks: Vec<_> = (0..120).map(control_block).collect();
    let old = Layout::Fixed.chunk_blocks(&blocks);
    assert_eq!(
        old.iter().map(|group| group.len()).collect::<Vec<_>>(),
        [40, 40, 40]
    );
    // Reuse uses distinct content from the previous snapshot and each successor's chunk bytes.
    for (at, count, expected) in [
        (0, 3, 0),
        (5, 3, 0),
        (120, 3, 3),
        (0, 40, 3),
        (40, 3, 1),
        (40, 40, 3),
    ] {
        let mut edited = blocks.clone();
        edited.splice(at..at, (120..120 + count).map(control_block));
        let groups = Layout::Fixed.chunk_blocks(&edited);
        assert_eq!(
            groups.iter().filter(|group| old.contains(group)).count(),
            expected
        );
        assert_eq!(groups.concat(), edited);
    }
    let mut removed = blocks.clone();
    removed.remove(0);
    assert!(
        Layout::Fixed
            .chunk_blocks(&removed)
            .iter()
            .all(|group| !old.contains(group))
    );
    let mut reordered = blocks.clone();
    reordered.rotate_left(40);
    assert!(
        Layout::Fixed
            .chunk_blocks(&reordered)
            .iter()
            .all(|group| old.contains(group))
    );
    assert_eq!(Layout::Fixed.chunk_blocks(&reordered).concat(), reordered);

    let oversized = BlockData {
        lines: vec!["ế".repeat(MAX)],
    };
    let mut mixed = vec![blocks[0].clone(), oversized, blocks[1].clone()];
    for layout in [Layout::Fixed, Layout::ContentDefined] {
        let groups = layout.chunk_blocks(&mixed);
        assert_eq!(groups[1].len(), 1);
        assert_eq!(groups.concat(), mixed);
        assert!(layout.chunk_blocks(&[]).is_empty());
    }
    // Repeated or moved blocks need no synthetic IDs to round-trip the exact snapshot.
    mixed.push(mixed[0].clone());
    assert_eq!(Layout::ContentDefined.chunk_blocks(&mixed).concat(), mixed);
}

fn control_block(i: usize) -> BlockData {
    BlockData {
        lines: vec![format!("{i:04}{}", "x".repeat(1516))],
    }
}

fn corpus(shape: &str, size: usize) -> DocumentData {
    let mut data = document_corpus::corpus(
        if shape == "vi_paragraphs" {
            "vi_nfd"
        } else {
            shape
        },
        size,
    );
    if shape == "vi_paragraphs" {
        // Split after NFC so the fixture never separates a combining mark from its base.
        let canonical = Document::parse(data.clone()).unwrap().to_data();
        let chars: Vec<_> = canonical.blocks[0].lines[0].chars().collect();
        data.blocks = chars
            .chunks(600)
            .map(|block| BlockData {
                lines: vec![block.iter().collect()],
            })
            .collect();
    }
    data
}

fn history(base: &DocumentData) -> Vec<(&'static str, DocumentData)> {
    let mut next = base.clone();
    let mut versions = vec![("initial", next.clone()), ("title_only", next.clone())];
    for (kind, at) in [
        ("prepend", 0),
        ("middle_insert", next.blocks.len() / 2),
        ("append", next.blocks.len() + 2),
    ] {
        let block = BlockData {
            lines: vec![format!("\tĐoạn thử {kind}")],
        };
        next.blocks.insert(at, block);
        versions.push((kind, next.clone()));
    }
    next.blocks.remove(0);
    versions.push(("delete", next.clone()));
    next.blocks.rotate_left(1);
    versions.push(("reorder", next.clone()));
    versions.push(("undo", base.clone()));
    next = base.clone();
    for edit in 0..16 {
        let at = (edit * 137 + 7) % next.blocks.len();
        next.blocks[at].lines[0].push('ế');
        versions.push(("small_edit", next.clone()));
    }
    versions
}

async fn prepare_tables(pool: &PgPool, layout: Layout) {
    sqlx::raw_sql("DROP TABLE IF EXISTS rfc_parts, rfc_versions, rfc_payloads")
        .execute(pool)
        .await
        .unwrap();
    let schema = match layout {
        Layout::Inline => {
            "CREATE TABLE rfc_versions(id int PRIMARY KEY, title text NOT NULL, body jsonb NOT NULL)"
        }
        Layout::Whole => {
            "CREATE TABLE rfc_payloads(hash bytea PRIMARY KEY, body jsonb NOT NULL);
            CREATE TABLE rfc_versions(id int PRIMARY KEY, title text NOT NULL,
                hash bytea NOT NULL REFERENCES rfc_payloads(hash))"
        }
        _ => {
            "CREATE TABLE rfc_payloads(hash bytea PRIMARY KEY, body jsonb NOT NULL);
            CREATE TABLE rfc_versions(id int PRIMARY KEY, title text NOT NULL);
            CREATE TABLE rfc_parts(version int REFERENCES rfc_versions(id), ordinal bigint,
                hash bytea NOT NULL REFERENCES rfc_payloads(hash), PRIMARY KEY (version, ordinal))"
        }
    };
    sqlx::raw_sql(schema).execute(pool).await.unwrap();
}

async fn read_version(pool: &PgPool, layout: Layout, id: i32) -> (String, Json<DocumentData>) {
    let sql = match layout {
        Layout::Inline => "SELECT title, body FROM rfc_versions WHERE id = $1",
        Layout::Whole => {
            "SELECT v.title, p.body FROM rfc_versions v
            JOIN rfc_payloads p ON p.hash = v.hash WHERE v.id = $1"
        }
        _ => {
            "SELECT v.title, jsonb_build_object('schema_version', 1, 'blocks',
                COALESCE((SELECT jsonb_agg(b.block ORDER BY p.ordinal, b.ordinal)
                 FROM rfc_parts p JOIN rfc_payloads c ON c.hash = p.hash
                 CROSS JOIN LATERAL jsonb_array_elements(c.body) WITH ORDINALITY b(block, ordinal)
                 WHERE p.version = v.id), '[]'::jsonb))
            FROM rfc_versions v WHERE v.id = $1"
        }
    };
    sqlx::query_as(sql).bind(id).fetch_one(pool).await.unwrap()
}

async fn observations(pool: &PgPool, layout: Layout, history: &[(&str, DocumentData)]) -> Value {
    prepare_tables(pool, layout).await;
    let mut known = HashSet::new();
    let mut rows = Vec::new();
    for (i, (kind, data)) in history.iter().enumerate() {
        let id = i as i32 + 1;
        let title = format!("Mẫu #231, revision {id}");
        let canonical = Document::parse(data.clone()).unwrap();
        let expected = canonical.to_data();
        let revision_hash = ContentHash::of(&Title::parse(&title).unwrap(), &canonical);
        let full_json = serde_json::to_string(&expected).unwrap();
        let start = Instant::now();
        let mut hashes = Vec::new();
        let mut new_hashes = Vec::new();
        let mut new_bodies = Vec::new();
        let mut payload_bytes = 0;
        let mut new_bytes = 0;
        let mut reused_bytes = 0;

        if matches!(layout, Layout::Inline) {
            payload_bytes = full_json.len();
            new_bytes = payload_bytes;
        } else {
            let bodies = if layout.chunked() {
                layout
                    .chunk_blocks(&expected.blocks)
                    .iter()
                    .map(|group| serde_json::to_string(group).unwrap())
                    .collect()
            } else {
                vec![full_json.clone()]
            };
            for body in bodies {
                let hash = hash_payload(&body);
                hashes.push(hash.clone());
                payload_bytes += body.len();
                if known.insert(hash.clone()) {
                    new_bytes += body.len();
                    new_hashes.push(hash);
                    new_bodies.push(body);
                } else {
                    reused_bytes += body.len();
                }
            }
        }
        let prepare_ms = start.elapsed().as_secs_f64() * 1000.;
        let lsn: String = sqlx::query_scalar("SELECT pg_current_wal_insert_lsn()::text")
            .fetch_one(pool)
            .await
            .unwrap();
        let start = Instant::now();
        let mut tx = pool.begin().await.unwrap();
        if !new_hashes.is_empty() {
            sqlx::query(
                "INSERT INTO rfc_payloads(hash, body)
                SELECT hash, body::jsonb FROM unnest($1::bytea[], $2::text[]) AS t(hash, body)",
            )
            .bind(new_hashes)
            .bind(new_bodies)
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        match layout {
            Layout::Inline => {
                sqlx::query("INSERT INTO rfc_versions VALUES ($1, $2, $3::text::jsonb)")
                    .bind(id)
                    .bind(&title)
                    .bind(&full_json)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            Layout::Whole => {
                sqlx::query("INSERT INTO rfc_versions VALUES ($1, $2, $3)")
                    .bind(id)
                    .bind(&title)
                    .bind(&hashes[0])
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("INSERT INTO rfc_versions VALUES ($1, $2)")
                    .bind(id)
                    .bind(&title)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO rfc_parts
                    SELECT $1, ordinal, hash FROM unnest($2::bytea[]) WITH ORDINALITY t(hash, ordinal)")
                    .bind(id).bind(&hashes).execute(&mut *tx).await.unwrap();
            }
        }
        tx.commit().await.unwrap();
        let commit_ms = start.elapsed().as_secs_f64() * 1000.;
        let wal_bytes: i64 = sqlx::query_scalar(
            "SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(), $1::pg_lsn)::bigint",
        )
        .bind(lsn)
        .fetch_one(pool)
        .await
        .unwrap();
        let start = Instant::now();
        let (loaded_title, Json(loaded)) = read_version(pool, layout, id).await;
        let read_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(loaded_title, title);
        assert_eq!(loaded, expected, "{kind}, {}", layout.name());
        let restored_hash = ContentHash::of(
            &Title::parse(&loaded_title).unwrap(),
            &Document::parse(loaded).unwrap(),
        );
        assert_eq!(restored_hash, revision_hash);
        rows.push(json!({
            "revision":id, "operation":kind, "full_json_bytes":full_json.len(),
            "payload_bytes":payload_bytes, "new_payload_bytes":new_bytes, "reused_payload_bytes":reused_bytes,
            "payload_refs":hashes.len(), "wal_bytes":wal_bytes,
            "prepare_ms":prepare_ms, "commit_ms":commit_ms, "read_ms":read_ms,
            "hash_v1":revision_hash.as_str(),
        }));
    }

    let relations: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT relname::text, pg_relation_size(oid), pg_indexes_size(oid),
            CASE WHEN reltoastrelid = 0 THEN 0 ELSE pg_total_relation_size(reltoastrelid) END,
            pg_total_relation_size(oid)
         FROM pg_class WHERE relnamespace = 'public'::regnamespace
            AND relname IN ('rfc_versions','rfc_payloads','rfc_parts') ORDER BY relname",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let payload_stats = if matches!(layout, Layout::Inline) {
        "SELECT count(*), sum(pg_column_size(body))::bigint,
            sum(octet_length(body::text))::bigint FROM rfc_versions"
    } else {
        "SELECT count(*), sum(pg_column_size(body))::bigint,
            sum(octet_length(body::text))::bigint FROM rfc_payloads"
    };
    let (payload_rows, column_bytes, logical_jsonb_text_bytes): (i64, i64, i64) =
        sqlx::query_as(payload_stats).fetch_one(pool).await.unwrap();
    json!({
        "layout":layout.name(), "payload_rows":payload_rows,
        "payload_column_bytes":column_bytes, "payload_jsonb_text_bytes":logical_jsonb_text_bytes,
        "relations":relations.into_iter().map(|(name, heap, index, toast, total)|
            json!({"name":name,"heap_bytes":heap,"index_bytes":index,"toast_total_bytes":toast,"total_bytes":total})
        ).collect::<Vec<_>>(),
        "samples":rows,
    })
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
#[ignore = "opt-in scratch PostgreSQL storage comparison; set VINK_CONTENT_STORAGE_OUTPUT"]
async fn content_storage_probe(pool: PgPool) {
    let output = std::env::var("VINK_CONTENT_STORAGE_OUTPUT").unwrap();
    let postgres: String = sqlx::query_scalar("SELECT version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let settings: Value = sqlx::query_scalar(
        "SELECT jsonb_object_agg(name, setting || COALESCE(' ' || unit, '')) FROM pg_settings
         WHERE name IN ('default_toast_compression','fsync','full_page_writes','wal_compression',
                         'shared_buffers','synchronous_commit','server_encoding','block_size')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut cases = Vec::new();
    for (shape, size) in [
        ("vi_paragraphs", 30_000),
        ("vi_paragraphs", 190_000),
        ("poem_tabs", 190_000),
        ("vi_nfc", 190_000),
    ] {
        let data = corpus(shape, size);
        let history = history(&data);
        let mut layouts = Vec::new();
        for layout in [
            Layout::Inline,
            Layout::Whole,
            Layout::Fixed,
            Layout::ContentDefined,
        ] {
            layouts.push(observations(&pool, layout, &history).await);
        }
        cases.push(json!({"shape":shape,"initial_scalars":size,"layouts":layouts}));
    }
    let result =
        json!({"postgres":postgres,"settings":settings,"seed":document_corpus::SEED,"cases":cases});
    std::fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn content_storage_probe_constraints(pool: PgPool) {
    let layout = Layout::ContentDefined;
    prepare_tables(&pool, layout).await;
    let expected = Document::parse(document_corpus::corpus("poem_tabs", 256))
        .unwrap()
        .to_data();
    let body = serde_json::to_string(&expected.blocks).unwrap();
    let hash = hash_payload(&body);
    let mut writer = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO rfc_versions VALUES (1, 'Thơ')")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("INSERT INTO rfc_payloads VALUES ($1, $2::text::jsonb)")
        .bind(&hash)
        .bind(body)
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("INSERT INTO rfc_parts VALUES (1, 1, $1)")
        .bind(&hash)
        .execute(&mut *writer)
        .await
        .unwrap();
    // Use a distinct pool connection while the writer still holds its transaction.
    let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rfc_versions WHERE id=1)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!visible);
    writer.commit().await.unwrap();
    assert_eq!(
        read_version(&pool, layout, 1).await,
        ("Thơ".to_owned(), Json(expected))
    );

    for (sql, code) in [
        ("INSERT INTO rfc_parts VALUES (1, 2, '\\x00')", "23503"),
        ("INSERT INTO rfc_parts SELECT * FROM rfc_parts", "23505"),
        ("DELETE FROM rfc_payloads", "23503"),
    ] {
        let error = sqlx::query(sql).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some(code)
        );
    }
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM rfc_parts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 1);
}
