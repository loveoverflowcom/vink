//! #255/#263: the one canonical document on real PostgreSQL — save/CAS with format-aware title
//! authority, snapshot titles, the publication capability gate, the database CHECK and the
//! forward-only cutover (including its refusal while immutable revisions exist).
use super::*;
use sqlx::migrate::Migrator;
use time::OffsetDateTime;
use vink_app::application::publication::AuthorPublicationError;
use vink_app::persistence::{MIGRATOR, migrate};

const CANONICAL_MIGRATION: i64 = 20_261_122_000_000;

fn markdown(work_id: WorkId, expected_version: i64, text: &str) -> SaveDraft {
    SaveDraft {
        work_id,
        expected_version,
        title: None,
        document: source("markdown", text),
    }
}

async fn stored(pool: &PgPool, work: WorkId) -> (String, serde_json::Value, i64) {
    sqlx::query_as("SELECT title, document, version FROM working_drafts WHERE work_id = $1")
        .bind(work.as_uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn markdown_title_is_materialized_in_the_same_save_and_cas(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = vink.create_draft(author()).await.unwrap().work_id;
    let text = "Lời đề.\r\n\r\n# *Mưa* trên Vie\u{0323}\u{0302}t\r\n\r\n\r\n  thơ\t\n\n# Tên sau\n";
    let saved = vink
        .save_draft(author(), markdown(work, 1, text))
        .await
        .unwrap();
    assert_eq!(saved.version.get(), 2);
    let (title, document, version) = stored(&pool, work).await;
    assert_eq!(
        title, "Mưa trên Việt",
        "derived, NFC, in the same row and version"
    );
    assert_eq!(
        document,
        serde_json::json!({"format": "markdown", "source": text}),
        "exact source: CRLF, blank lines, tab and NFD survive"
    );
    assert_eq!(version, 2);
    let loaded = vink.load_draft(author(), work).await.unwrap();
    assert_eq!(loaded.content.title().as_str(), "Mưa trên Việt");
    assert_eq!(loaded.content.document().source(), text);

    // A lost answer retried with the old version conflicts and writes nothing.
    let retried = vink.save_draft(author(), markdown(work, 1, "# Khác")).await;
    assert_eq!(
        retried,
        Err(DraftError::VersionConflict {
            current: DraftVersion::new(2).unwrap()
        })
    );
    // Deleting the first H1 moves the title to the next one; no H1 leaves it empty.
    vink.save_draft(author(), markdown(work, 2, "Thân.\n\n# Tên sau\n"))
        .await
        .unwrap();
    assert_eq!(stored(&pool, work).await.0, "Tên sau");
    vink.save_draft(author(), markdown(work, 3, "Chỉ thân bài.\n"))
        .await
        .unwrap();
    let before = (
        String::new(),
        json!({"format": "markdown", "source": "Chỉ thân bài.\n"}),
        4,
    );
    assert_eq!(stored(&pool, work).await, before);
    let router = app(&pool).await;
    let (status, _, body) = call(&router, "POST", &format!("/api/v1/works/{work}/snapshots"),
        Some(dev_seed::SAMPLE_AUTHOR), Some(ORIGIN), Some(json!({
            "command_key": "untitled-snapshot", "expected_draft_version": 4, "expected_metadata_version": 1
        }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json(&body)["error"]["field"], "title");
    let revision_count: i64 = sqlx::query_scalar("SELECT count(*) FROM revisions WHERE work_id=$1")
        .bind(work.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(revision_count, 0);
    // Markdown only accepts a title derived from its first H1.
    assert!(matches!(
        vink.save_draft(
            author(),
            SaveDraft {
                title: Some("Tên client".into()),
                ..markdown(work, 4, "# Tên")
            }
        )
        .await,
        Err(DraftError::Invalid(_))
    ));
    for (invalid_heading, reason) in [
        ("ơ".repeat(201), "title exceeds 200 characters"),
        (
            "Mưa&#x2028;đêm".to_owned(),
            "title contains a line break or control character",
        ),
    ] {
        let source = format!("Lời đề.\n\n{invalid_heading}\n===\n\n# Tên sau\n");
        let (status, _, body) = call(
            &router,
            "PUT",
            &format!("/api/v1/drafts/{work}"),
            Some(dev_seed::SAMPLE_AUTHOR),
            Some(ORIGIN),
            Some(json!({
                "expected_version": 4,
                "document": {"format": "markdown", "source": source}
            })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            json(&body),
            json!({"error": {
                "code": "validation_failed",
                "field": "title",
                "message": format!("title: first H1 in document.source: {reason}")
            }})
        );
        assert_eq!(stored(&pool, work).await, before);
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn snapshot_title_comes_from_the_captured_source_and_markdown_stays_private(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = vink.create_draft(author()).await.unwrap().work_id;
    vink.save_draft(author(), markdown(work, 1, "# Mưa đầu mùa\n\nThân."))
        .await
        .unwrap();
    let capture = vink
        .capture_snapshot(author(), work, "canonical-capture", 2, 1)
        .await
        .unwrap();
    // Editing the H1 afterwards never renames the immutable snapshot.
    vink.save_draft(author(), markdown(work, 2, "# Tên mới\n\nThân."))
        .await
        .unwrap();
    let (title, document, hash): (String, serde_json::Value, String) =
        sqlx::query_as("SELECT title, document, content_hash FROM revisions WHERE id = $1")
            .bind(capture.revision_id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(title, "Mưa đầu mùa");
    assert_eq!(document["source"], "# Mưa đầu mùa\n\nThân.");
    let content = TitledDocument::author(
        Document::new(DocumentFormat::Markdown, "# Mưa đầu mùa\n\nThân.").unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        hash,
        vink_app::domain::revision::ContentHash::of(&content).as_str()
    );
    assert_eq!(capture.content_hash.as_str(), hash);
    // No safe Reader yet (#264): the request is refused before any state, receipt or ticket.
    assert_eq!(
        vink.request_public(author(), work, "canonical-request", capture.revision_id, 1)
            .await,
        Err(AuthorPublicationError::FormatNotPublishable)
    );
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM work_publication_command_receipts
          WHERE work_id = $1 AND operation = 'request_public'",
    )
    .bind(work.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, 0);
    assert_eq!(
        vink.publication(work).await,
        Err(vink_app::application::ReadError::NotFound)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn rejected_markdown_preserves_the_previous_public_revision(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    let before = vink.publication(work).await.unwrap();
    vink.save_draft(author(), markdown(work, 1, "# Bản sửa riêng"))
        .await
        .unwrap();
    let capture = vink
        .capture_snapshot(author(), work, "private-markdown", 2, 1)
        .await
        .unwrap();
    assert_eq!(
        vink.request_public(author(), work, "reject-markdown", capture.revision_id, 2)
            .await,
        Err(AuthorPublicationError::FormatNotPublishable)
    );
    assert_eq!(vink.publication(work).await.unwrap(), before);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn database_accepts_only_the_canonical_shape(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = vink.create_draft(author()).await.unwrap().work_id;
    for document in [
        r#"{"schema_version":1,"blocks":[]}"#,
        r#"{"schema_version":2,"format":"default","source":"","line_ending":"lf"}"#,
        r#"{"format":"default"}"#,
        r#"{"source":""}"#,
        r#"{"format":null,"source":""}"#,
        r#"{"format":"asciidoc","source":"= X"}"#,
        r#"{"format":"default","source":1}"#,
        r#"{"format":"default","source":"","html":"<b>x</b>"}"#,
        r#"["default",""]"#,
        r#""default""#,
    ] {
        let error =
            sqlx::query("UPDATE working_drafts SET document = $2::text::jsonb WHERE work_id = $1")
                .bind(work.as_uuid())
                .bind(document)
                .execute(&pool)
                .await
                .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514"),
            "{document}"
        );
    }
    let oversized = serde_json::json!({"format": "default", "source": "a".repeat(2_097_153)});
    let error = sqlx::query("UPDATE working_drafts SET document = $2 WHERE work_id = $1")
        .bind(work.as_uuid())
        .bind(sqlx::types::Json(oversized))
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    // A drifted Markdown title column is an integrity error on read, never a silent rename.
    sqlx::query(
        "UPDATE working_drafts SET title = 'Tên lệch',
                document = '{\"format\":\"markdown\",\"source\":\"# Tên thật\"}'
          WHERE work_id = $1",
    )
    .bind(work.as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        vink.load_draft(author(), work).await,
        Err(DraftError::Internal)
    );
}

async fn previous_schema(pool: &PgPool) {
    Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < CANONICAL_MIGRATION)
            .cloned()
            .collect(),
    )
    .run(pool)
    .await
    .unwrap();
}

const RETIRED_DRAFT: &str = r##"{"schema_version":1,"blocks":[{"lines":["Phố cũ mưa giăng,","    mái ngói rêu."]},{"lines":["# Không phải tên"]}]}"##;

async fn retired_work(pool: &PgPool, work: Uuid, version: i64) {
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO principals (id, display_name) VALUES ('{author}', '[Mẫu] Người viết')
             ON CONFLICT DO NOTHING;
         INSERT INTO works (id, owner_id) VALUES ('{work}', '{author}');
         INSERT INTO working_drafts (work_id, version, title, document, updated_at)
             VALUES ('{work}', {version}, 'Tên riêng', '{RETIRED_DRAFT}', '2026-11-01T00:00:00Z');",
        author = dev_seed::SAMPLE_AUTHOR,
    )))
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = false)]
async fn cutover_converts_retired_drafts_exactly_once(pool: PgPool) {
    previous_schema(&pool).await;
    let work = Uuid::now_v7();
    let empty = Uuid::now_v7();
    retired_work(&pool, work, 7).await;
    retired_work(&pool, empty, 1).await;
    sqlx::query(r#"UPDATE working_drafts SET document = '{"schema_version":1,"blocks":[]}' WHERE work_id = $1"#)
        .bind(empty)
        .execute(&pool)
        .await
        .unwrap();
    let before: (OffsetDateTime,) =
        sqlx::query_as("SELECT updated_at FROM working_drafts WHERE work_id = $1")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();

    let report = migrate(&pool).await.unwrap();
    assert_eq!(report.applied, vec![CANONICAL_MIGRATION]);
    let (title, document, version) = stored(&pool, WorkId::from_uuid(work)).await;
    assert_eq!((title.as_str(), version), ("Tên riêng", 7));
    assert_eq!(
        document,
        serde_json::json!({
            "format": "default",
            "source": "Phố cũ mưa giăng,\n    mái ngói rêu.\n\n# Không phải tên"
        })
    );
    let after: (OffsetDateTime,) =
        sqlx::query_as("SELECT updated_at FROM working_drafts WHERE work_id = $1")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after, "no author edit happened");
    assert_eq!(
        stored(&pool, WorkId::from_uuid(empty)).await.1,
        serde_json::json!({"format": "default", "source": ""})
    );
    // The runtime reads the converted draft as a default document: `#` stays literal text.
    let vink = Vink::new(PgStore::new(pool.clone()));
    let loaded = vink
        .load_draft(author(), WorkId::from_uuid(work))
        .await
        .unwrap();
    assert_eq!(loaded.content.title().as_str(), "Tên riêng");
    assert_eq!(loaded.content.format(), DocumentFormat::Default);
    // Repeating the migration is a no-op.
    assert!(migrate(&pool).await.unwrap().applied.is_empty());
    assert_eq!(stored(&pool, WorkId::from_uuid(work)).await.1, document);

    // Backout is exact while every draft is representable, and the forward conversion is
    // deterministic across a backout/forward cycle.
    run_backout(&pool).await.unwrap();
    let restored: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM working_drafts WHERE work_id = $1")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        restored,
        serde_json::from_str::<serde_json::Value>(RETIRED_DRAFT).unwrap()
    );
    assert_eq!(
        migrate(&pool).await.unwrap().applied,
        vec![CANONICAL_MIGRATION]
    );
    assert_eq!(stored(&pool, WorkId::from_uuid(work)).await.1, document);
}

/// The operator script as written (its own BEGIN/COMMIT) on one connection; a refusal leaves
/// that connection's transaction aborted, so it is rolled back before the pool reuses it.
pub(super) async fn run_backout(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await.unwrap();
    let result = sqlx::raw_sql(sqlx::AssertSqlSafe(include_str!(
        "../../migrations-rollback/20261122000000_canonical_document.down.sql"
    )))
    .execute(&mut *conn)
    .await
    .map(|_| ());
    if result.is_err() {
        sqlx::query("ROLLBACK").execute(&mut *conn).await.unwrap();
    }
    result
}

#[sqlx::test(migrations = false)]
async fn cutover_refuses_unrepresentable_retired_drafts_atomically(pool: PgPool) {
    previous_schema(&pool).await;
    let valid = Uuid::now_v7();
    let invalid = Uuid::now_v7();
    retired_work(&pool, valid, 7).await;
    retired_work(&pool, invalid, 3).await;
    let unchanged = stored(&pool, WorkId::from_uuid(valid)).await;
    for (title, lines) in [
        ("Tên", vec![]),
        ("Tên", vec!["\u{a0}"]),
        ("Tên", vec!["a\rb"]),
        ("Tên", vec!["e\u{301}"]),
        (" padded ", vec!["a"]),
        ("Tên", vec!["a\u{80}"]),
    ] {
        let doc = json!({"schema_version":1,"blocks":[{"lines":lines}]});
        sqlx::query("UPDATE working_drafts SET document=$2,title=$3 WHERE work_id=$1")
            .bind(invalid)
            .bind(&doc)
            .bind(title)
            .execute(&pool)
            .await
            .unwrap();
        let refused = migrate(&pool).await.unwrap_err();
        assert!(refused.to_string().contains("cutover blocked"), "{refused}");
        assert_eq!(stored(&pool, WorkId::from_uuid(valid)).await, unchanged);
        assert_eq!(
            stored(&pool, WorkId::from_uuid(invalid)).await,
            (title.into(), doc, 3)
        );
    }
}

#[sqlx::test(migrations = false)]
async fn cutover_refuses_while_immutable_revisions_exist(pool: PgPool) {
    previous_schema(&pool).await;
    let work = Uuid::now_v7();
    let revision = Uuid::now_v7();
    retired_work(&pool, work, 3).await;
    // The retired v1 content hash (literal encoding, as the #100 SQL fixtures compute it).
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         SELECT '{revision}', '{work}', 'Tên riêng', '{RETIRED_DRAFT}',
                encode(sha256(convert_to('vink-revision-content-v1','UTF8')
                    || int8send(octet_length('Tên riêng')::bigint) || convert_to('Tên riêng','UTF8')
                    || int8send(2)
                    || int8send(2)
                    || int8send(octet_length('Phố cũ mưa giăng,')::bigint) || convert_to('Phố cũ mưa giăng,','UTF8')
                    || int8send(octet_length('    mái ngói rêu.')::bigint) || convert_to('    mái ngói rêu.','UTF8')
                    || int8send(1)
                    || int8send(octet_length('# Không phải tên')::bigint) || convert_to('# Không phải tên','UTF8')),'hex'),
                '{author}';",
        author = dev_seed::SAMPLE_AUTHOR,
    )))
    .execute(&pool)
    .await
    .unwrap();

    let error = migrate(&pool).await.unwrap_err().to_string();
    assert!(error.contains("cutover blocked"), "{error}");
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(latest < CANONICAL_MIGRATION, "nothing was recorded");
    let draft: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM working_drafts WHERE work_id = $1")
            .bind(work)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        draft,
        serde_json::from_str::<serde_json::Value>(RETIRED_DRAFT).unwrap(),
        "no partial cutover"
    );

    // The exact conflict: the canonical hash of the same text differs from the stored digest,
    // and the stored snapshot cannot be rewritten in place.
    let stored_hash: String =
        sqlx::query_scalar("SELECT content_hash FROM revisions WHERE id = $1")
            .bind(revision)
            .fetch_one(&pool)
            .await
            .unwrap();
    let canonical = vink_app::domain::revision::ContentHash::of(&titled(
        "Tên riêng",
        source(
            "default",
            "Phố cũ mưa giăng,\n    mái ngói rêu.\n\n# Không phải tên",
        ),
    ));
    assert_ne!(canonical.as_str(), stored_hash);
    let error = sqlx::query(
        r#"UPDATE revisions SET document = '{"format":"default","source":""}' WHERE id = $1"#,
    )
    .bind(revision)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(error.to_string().contains("immutable snapshot"), "{error}");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn backout_refuses_when_it_would_lose_text_or_history(pool: PgPool) {
    let vink = Vink::new(PgStore::new(pool.clone()));
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, '[Mẫu] Người viết')")
        .bind(dev_seed::SAMPLE_AUTHOR)
        .execute(&pool)
        .await
        .unwrap();
    let work = vink.create_draft(author()).await.unwrap().work_id;
    for (text, format) in [
        ("Một\n\n\nHai", "default"),
        ("Một\n   \nHai", "default"),
        ("Một\n\u{00a0}\nHai", "default"),
        (
            "Một\n\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{202f}\u{205f}\u{3000}\nHai",
            "default",
        ),
        ("Một\r\nHai", "default"),
        ("Vie\u{0323}\u{0302}t", "default"),
        ("\nMột", "default"),
        ("# Mưa", "markdown"),
    ] {
        let version = stored(&pool, work).await.2;
        let command = if format == "markdown" {
            markdown(work, version, text)
        } else {
            save(work, version, "Tên", source("default", text))
        };
        vink.save_draft(author(), command).await.unwrap();
        let error = run_backout(&pool).await.unwrap_err().to_string();
        assert!(error.contains("roll forward"), "{text:?}: {error}");
        assert_eq!(stored(&pool, work).await.1["source"], text, "untouched");
    }
    let version = stored(&pool, work).await.2;
    vink.save_draft(
        author(),
        save(work, version, "Tên", doc(&[&["Một"], &["Hai"]])),
    )
    .await
    .unwrap();
    dev_seed::apply(&pool).await.unwrap();
    let error = run_backout(&pool).await.unwrap_err().to_string();
    assert!(error.contains("immutable canonical revisions"), "{error}");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn backout_refuses_drafts_outside_retired_reader_limits(pool: PgPool) {
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, '[Mẫu] Người viết')")
        .bind(dev_seed::SAMPLE_AUTHOR)
        .execute(&pool)
        .await
        .unwrap();
    let work = Vink::new(PgStore::new(pool.clone()))
        .create_draft(author())
        .await
        .unwrap()
        .work_id;
    for (title, text) in [
        ("Tên".into(), "a\u{1}".into()),
        ("Tên".into(), "a".repeat(200001)),
        ("t".repeat(201), "a".into()),
        ("a\nb".into(), "a".into()),
    ] {
        let doc = json!({"format":"default", "source":text});
        sqlx::query("UPDATE working_drafts SET document=$2,title=$3 WHERE work_id=$1")
            .bind(work.as_uuid())
            .bind(&doc)
            .bind(&title)
            .execute(&pool)
            .await
            .unwrap();
        let before = stored(&pool, work).await;
        let error = run_backout(&pool).await.unwrap_err();
        assert!(
            error.to_string().contains("not exactly representable"),
            "{error}"
        );
        assert_eq!(stored(&pool, work).await, before);
    }
}
