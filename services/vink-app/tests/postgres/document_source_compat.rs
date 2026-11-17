//! PostgreSQL compatibility and schema-boundary tests for `VDR-DOCUMENT-SOURCE-FORMAT` (#255).
//!
//! `AC-03` & `AC-05`: Verifies that production `working_drafts` and `revisions` continue writing
//! and serving schema v1 without mutating historical rows or `content_hash`, while proving on a
//! scratch table that the proposed additive `IS TRUE`-guarded `CHECK` constraint enforces strict
//! v1+v2 structure and preserves raw NFD/whitespace/emoji verbatim through PostgreSQL JSONB.

use serde_json::{Value, json};
use vink_app::domain::revision::ContentHash;
use vink_app::domain::source::{
    DocumentSource, EffectiveFormat, LineEnding, SourceFormat, StoredDocument,
};

use super::*;

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn production_v1_rows_remain_unmodified_and_readable_via_stored_document(pool: PgPool) {
    let vink = seeded(&pool).await;
    let draft = vink.create_draft(author()).await.unwrap();
    vink.save_draft(
        author(),
        save(
            draft.work_id,
            1,
            "Nháp v1 có dấu thăng",
            doc(&[&["# Không phải tiêu đề Markdown"], &["Dòng hai."]]),
        ),
    )
    .await
    .unwrap();

    // Production writers still emit schema v1 without format/source (AC-05).
    let raw_draft: Value =
        sqlx::query_scalar("SELECT document FROM working_drafts WHERE work_id = $1")
            .bind(draft.work_id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(raw_draft.get("schema_version"), Some(&json!(1)));
    assert!(raw_draft.get("format").is_none());
    assert!(raw_draft.get("source").is_none());

    let parsed_draft = StoredDocument::parse_json_value(raw_draft.clone()).unwrap();
    assert_eq!(parsed_draft.schema_version(), 1);
    assert_eq!(
        parsed_draft.effective_format(),
        EffectiveFormat::LegacyDefault
    );
    assert!(!parsed_draft.effective_format().renders_markdown());

    // Idempotent re-read never rewrites the stored v1 draft JSONB.
    let reread_raw: Value =
        sqlx::query_scalar("SELECT document FROM working_drafts WHERE work_id = $1")
            .bind(draft.work_id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reread_raw, raw_draft);

    // Seeded revision + publication pointer stay untouched in schema v1 and v1 ContentHash.
    let public_rev_before: uuid::Uuid =
        sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (rev_title, rev_doc, rev_hash): (String, Value, String) =
        sqlx::query_as("SELECT title, document, content_hash FROM revisions WHERE id = $1")
            .bind(dev_seed::SAMPLE_REVISION)
            .fetch_one(&pool)
            .await
            .unwrap();
    let parsed_rev = StoredDocument::parse_json_value(rev_doc).unwrap();
    let StoredDocument::V1(v1_doc) = &parsed_rev else {
        panic!("seeded revision must stay schema v1");
    };
    assert_eq!(
        rev_hash,
        ContentHash::of(&Title::parse(&rev_title).unwrap(), v1_doc).as_str()
    );
    let public_rev_after: uuid::Uuid =
        sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(public_rev_after, public_rev_before);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn proposed_v2_check_constraint_supports_lossless_nfd_and_dual_read_in_scratch_table(
    pool: PgPool,
) {
    // `(cond) IS TRUE` prevents PostgreSQL 3-valued NULL from bypassing missing JSONB keys.
    sqlx::raw_sql(
        "CREATE TABLE scratch_compat_documents (
            id uuid PRIMARY KEY,
            document jsonb NOT NULL CHECK (
                jsonb_typeof(document) = 'object'
                AND (
                    (
                        (document ->> 'schema_version') = '1'
                        AND jsonb_typeof(document -> 'blocks') = 'array'
                    ) IS TRUE
                    OR
                    (
                        (document ->> 'schema_version') = '2'
                        AND (document ->> 'format') IN ('default', 'markdown')
                        AND jsonb_typeof(document -> 'source') = 'string'
                        AND (document ->> 'line_ending') IN ('lf', 'crlf')
                    ) IS TRUE
                )
            )
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    let v1_id = Uuid::now_v7();
    let v2_default_id = Uuid::now_v7();
    let v2_markdown_id = Uuid::now_v7();

    let v1_json = serde_json::to_value(doc(&[&["# Vẫn là v1"]])).unwrap();
    let authored_nfd_crlf = concat!(
        "\r\n",
        "  \tTie\u{0302}\u{0301}ng Vie\u{0323}\u{0302}t NFD\r\n",
        "   \r\n",
        "\r\n",
        "Thơ \u{1f9d1}\u{200d}\u{1f33e} \u{2764}\u{fe0f}\r\n"
    );
    let src_default =
        DocumentSource::from_authored(SourceFormat::Default, authored_nfd_crlf).unwrap();
    assert_eq!(src_default.line_ending(), LineEnding::Crlf);
    let expected_title = Title::parse("Tiếng Việt").unwrap();
    let expected_hash_default = ContentHash::of_source_v2(&expected_title, &src_default);

    let src_markdown = DocumentSource::from_authored(
        SourceFormat::Markdown,
        "# Tie\u{0302}\u{0301}ng mưa\n\n\nNội dung **Markdown**\n",
    )
    .unwrap();

    for (id, payload) in [
        (v1_id, v1_json.clone()),
        (
            v2_default_id,
            serde_json::to_value(src_default.to_data()).unwrap(),
        ),
        (
            v2_markdown_id,
            serde_json::to_value(src_markdown.to_data()).unwrap(),
        ),
    ] {
        sqlx::query("INSERT INTO scratch_compat_documents(id, document) VALUES ($1, $2)")
            .bind(id)
            .bind(payload)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Proposed CHECK constraint must reject deferred asciidoc, unknown format, bad version, and
    // missing keys (which would pass as SQL UNKNOWN without `IS TRUE`).
    for invalid in [
        json!({"schema_version": 2, "format": "asciidoc", "source": "= H1", "line_ending": "lf"}),
        json!({"schema_version": 2, "format": "org", "source": "* H1", "line_ending": "lf"}),
        json!({"schema_version": 3, "format": "default", "source": "x", "line_ending": "lf"}),
        json!({"schema_version": 2, "format": "markdown", "line_ending": "lf"}),
        json!({"schema_version": 2, "source": "x", "line_ending": "lf"}),
        json!({"schema_version": 2, "format": "markdown", "source": 123, "line_ending": "lf"}),
        json!({"schema_version": 1}),
    ] {
        let rejected =
            sqlx::query("INSERT INTO scratch_compat_documents(id, document) VALUES ($1, $2)")
                .bind(Uuid::now_v7())
                .bind(invalid)
                .execute(&pool)
                .await;
        assert!(rejected.is_err());
    }

    let read_v1 = StoredDocument::parse_json_value(
        sqlx::query_scalar::<_, Value>(
            "SELECT document FROM scratch_compat_documents WHERE id = $1",
        )
        .bind(v1_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(read_v1.effective_format(), EffectiveFormat::LegacyDefault);

    let read_v2_default = StoredDocument::parse_json_value(
        sqlx::query_scalar::<_, Value>(
            "SELECT document FROM scratch_compat_documents WHERE id = $1",
        )
        .bind(v2_default_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .unwrap();
    let StoredDocument::V2(roundtripped_default) = read_v2_default else {
        panic!("expected schema v2 source");
    };
    assert_eq!(roundtripped_default, src_default);
    assert_eq!(roundtripped_default.to_authored_text(), authored_nfd_crlf);
    assert_eq!(
        ContentHash::of_source_v2(&expected_title, &roundtripped_default),
        expected_hash_default
    );

    let read_v2_md = StoredDocument::parse_json_value(
        sqlx::query_scalar::<_, Value>(
            "SELECT document FROM scratch_compat_documents WHERE id = $1",
        )
        .bind(v2_markdown_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        read_v2_md.effective_format(),
        EffectiveFormat::Explicit(SourceFormat::Markdown)
    );
}
