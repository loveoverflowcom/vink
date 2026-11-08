//! Work taxonomy (TAG-PR-02, #96) on real PostgreSQL. Draft saves run through `Vink` and
//! `PgStore`, so every write is the production transaction that locks the facts, calls
//! `decide_tag_change` and commits; catalog changes run through `change_catalog` with operator
//! authority, which calls `decide_catalog_change` and audits each accepted change (#97). Integrity negatives bypass Rust with direct SQL.
//!
//! Races use a second connection that holds a real lock (or an uncommitted row) and then commits
//! or rolls back, so both commit orders are exercised on purpose. Waiting is OBSERVED, not
//! assumed: `wait_blocked*` polls `pg_blocking_pids` until the expected backends are blocked on a
//! lock; the poll interval only paces that query and the deadline turns a hang into a failure.
//! No elapsed time is evidence.
//!
//! Publication pointer changes and the "publication transaction" are labelled FIXTURES: no
//! publish/withdraw command exists (PUB-01..03). They call the production capture seam
//! (`capture_metadata_snapshot`) and move the pointer with SQL, proving the storage boundary,
//! not a product workflow.

use std::time::{Duration, Instant};

use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnection;
use sqlx::{AssertSqlSafe, Connection};
use vink_app::application::ReadError;
use vink_app::application::catalog::CuratorAuthority;
use vink_app::application::ports::StoreError;
use vink_app::application::taxonomy::{SaveWorkTags, SavedWorkTags, TagError, TagRejection};
use vink_app::domain::ids::{RevisionId, TagId};
use vink_app::domain::principal::SuspensionReason;
use vink_app::domain::taxonomy::{
    CatalogChange, CatalogCommand, CatalogOutcome, CatalogRefusal, CatalogVersion,
    MAX_THEMES_PER_WORK, MetadataDigest, MetadataVersion, SuccessorError, TagDescription, TagFacet,
    TagLabel, WorkTags,
};
use vink_app::persistence::{
    MIGRATOR, SnapshotCapture, capture_metadata_snapshot, change_catalog, migrate,
    reinstate_principal, suspend_principal,
};

use super::*;

const TAXONOMY_MIGRATION: i64 = 20_261_108_000_000;

const fn tag(n: u128) -> TagId {
    TagId::from_uuid(Uuid::from_u128(
        0xa000_0000_0000_4000_8000_0000_0000_0000 + n,
    ))
}

const TAN_VAN: TagId = tag(1);
const THO: TagId = tag(2);
const KY_UC: TagId = tag(11);
const GIA_DINH: TagId = tag(12);
const DO_THI: TagId = tag(13);
const MUA: TagId = tag(14);

const CATALOG: [(TagId, TagFacet, &str); 6] = [
    (TAN_VAN, TagFacet::Form, "Tản văn"),
    (THO, TagFacet::Form, "Thơ"),
    (KY_UC, TagFacet::Theme, "Ký ức"),
    (GIA_DINH, TagFacet::Theme, "Gia đình"),
    (DO_THI, TagFacet::Theme, "Đô thị"),
    (MUA, TagFacet::Theme, "Mùa"),
];

fn label(text: &str) -> TagLabel {
    TagLabel::parse(text).unwrap()
}

async fn catalog_version(pool: &PgPool) -> CatalogVersion {
    let raw: i64 = sqlx::query_scalar("SELECT version FROM tag_catalog")
        .fetch_one(pool)
        .await
        .unwrap();
    CatalogVersion::new(raw).unwrap()
}

/// One catalog change at the current catalog version (operator authority).
async fn curate(pool: &PgPool, change: CatalogChange) -> CatalogOutcome {
    let expected = catalog_version(pool).await;
    change_catalog(
        pool,
        CuratorAuthority::operator(),
        &CatalogCommand { expected, change },
    )
    .await
    .unwrap()
}

async fn create(pool: &PgPool, id: TagId, facet: TagFacet, text: &str) {
    let outcome = curate(
        pool,
        CatalogChange::Create {
            id,
            facet,
            label: label(text),
            description: None,
        },
    )
    .await;
    assert!(
        matches!(outcome, CatalogOutcome::Accepted(_)),
        "{outcome:?}"
    );
}

async fn retire(pool: &PgPool, id: TagId, successor: Option<TagId>) -> CatalogOutcome {
    curate(pool, CatalogChange::Retire { id, successor }).await
}

/// The seed (author, sample work with draft text and publication R1) plus a six-tag catalog.
async fn setup(pool: &PgPool) -> Arc<Vink<PgStore>> {
    let vink = seeded(pool).await;
    for (id, facet, text) in CATALOG {
        create(pool, id, facet, text).await;
    }
    Arc::new(vink)
}

fn command(expected: i64, form: Option<TagId>, themes: &[TagId]) -> SaveWorkTags {
    SaveWorkTags {
        work_id: sample_work(),
        expected_version: expected,
        form: form.map(|id| id.to_string()),
        themes: themes.iter().map(ToString::to_string).collect(),
    }
}

async fn save_tags(
    vink: &Vink<PgStore>,
    actor: Option<PrincipalId>,
    expected: i64,
    form: Option<TagId>,
    themes: &[TagId],
) -> Result<SavedWorkTags, TagError> {
    vink.save_work_tags(actor, command(expected, form, themes))
        .await
}

fn tags(form: Option<TagId>, themes: &[TagId]) -> WorkTags {
    WorkTags::restore(form, themes.to_vec()).unwrap()
}

/// The durable draft metadata of the sample work: version (absent row = None) and slots.
async fn draft_state(pool: &PgPool) -> (Option<i64>, Vec<(i16, Uuid)>) {
    let version = sqlx::query_scalar("SELECT version FROM work_draft_metadata WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_optional(pool)
        .await
        .unwrap();
    let slots =
        sqlx::query_as("SELECT slot, tag_id FROM work_draft_tags WHERE work_id = $1 ORDER BY slot")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_all(pool)
            .await
            .unwrap();
    (version, slots)
}

fn slots(form: Option<TagId>, themes: &[TagId]) -> Vec<(i16, Uuid)> {
    let mut rows: Vec<(i16, Uuid)> = form.map(|id| (0, id.as_uuid())).into_iter().collect();
    for (i, id) in themes.iter().enumerate() {
        rows.push((i16::try_from(i + 1).unwrap(), id.as_uuid()));
    }
    rows
}

async fn rows(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn backend_pid(conn: &mut PgConnection) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(conn)
        .await
        .unwrap()
}

const WAIT_DEADLINE: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(10);

/// Returns once some backend of this database is blocked on a lock held by `blocker`.
async fn wait_blocked_by(pool: &PgPool, blocker: i32) {
    let deadline = Instant::now() + WAIT_DEADLINE;
    loop {
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                             WHERE datname = current_database()
                               AND $1 = ANY (pg_blocking_pids(pid)))",
        )
        .bind(blocker)
        .fetch_one(pool)
        .await
        .unwrap();
        if blocked {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "no backend became blocked by {blocker}"
        );
        tokio::time::sleep(POLL).await;
    }
}

/// Returns once at least `n` backends of this database are blocked on locks.
async fn wait_blocked(pool: &PgPool, n: i64) {
    let deadline = Instant::now() + WAIT_DEADLINE;
    loop {
        let blocked: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database() AND cardinality(pg_blocking_pids(pid)) > 0",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if blocked >= n {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "expected {n} blocked backend(s), saw {blocked}"
        );
        tokio::time::sleep(POLL).await;
    }
}

/// FIXTURE: a new immutable revision of the sample work (what a future submission creates).
async fn new_revision(conn: &mut PgConnection) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         SELECT $1, work_id, title || ' (bản sửa)', document,
                md5(random()::text) || md5(random()::text), created_by
           FROM revisions WHERE id = $2",
    )
    .bind(id)
    .bind(dev_seed::SAMPLE_REVISION)
    .execute(conn)
    .await
    .unwrap();
    id
}

/// FIXTURE: what a future publication transaction does after its own authority/intent checks:
/// move the pointer to the pair (revision, metadata snapshot) in ONE row.
const BIND: &str = "UPDATE publications
                       SET revision_id = $2, metadata_snapshot_id = $3,
                           generation = generation + 1, published_at = now()
                     WHERE work_id = $1";
/// FIXTURE: what a withdrawal will do: remove the pointer (snapshots stay as history).
const WITHDRAW: &str = "DELETE FROM publications WHERE work_id = $1";

async fn bind(conn: &mut PgConnection, revision: Uuid, snapshot: Uuid) {
    let moved = sqlx::query(BIND)
        .bind(dev_seed::SAMPLE_WORK)
        .bind(revision)
        .bind(snapshot)
        .execute(conn)
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(moved, 1);
}

fn captured(capture: SnapshotCapture) -> Uuid {
    match capture {
        SnapshotCapture::Captured { id, .. } => id,
        other => panic!("expected a capture, got {other:?}"),
    }
}

fn version(raw: i64) -> MetadataVersion {
    MetadataVersion::new(raw).unwrap()
}

fn shown(tags: &[vink_app::domain::taxonomy::CatalogTag]) -> Vec<TagId> {
    tags.iter().map(|t| t.id()).collect()
}

/// A database refusal: SQLSTATE, the named constraint (if any) and the message.
#[derive(Debug)]
struct Refusal {
    code: String,
    constraint: Option<String>,
    message: String,
}

impl Refusal {
    fn names(&self, needle: &str) -> bool {
        self.constraint.as_deref() == Some(needle) || self.message.contains(needle)
    }
}

/// Runs `statements` in one transaction and returns the error that stops it, at a statement or
/// at commit (deferred constraints). Panics if everything commits.
async fn refused(pool: &PgPool, statements: &[String]) -> Refusal {
    let describe = |error: sqlx::Error| {
        let db = error.as_database_error().expect("a database error");
        Refusal {
            code: db.code().unwrap_or_default().into_owned(),
            constraint: db.constraint().map(str::to_owned),
            message: db.message().to_owned(),
        }
    };
    let mut tx = pool.begin().await.unwrap();
    for statement in statements {
        if let Err(error) = sqlx::raw_sql(AssertSqlSafe(statement.clone()))
            .execute(&mut *tx)
            .await
        {
            return describe(error);
        }
    }
    match tx.commit().await {
        Ok(()) => panic!("committed although it should not: {statements:?}"),
        Err(error) => describe(error),
    }
}

async fn assert_refused(pool: &PgPool, statements: &[String], code: &str, needle: &str) {
    let refusal = refused(pool, statements).await;
    assert!(
        refusal.code == code && refusal.names(needle),
        "expected {code} naming {needle:?} for {statements:?}, got {refusal:?}"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn sql_digest_matches_rust_and_the_independent_vectors(pool: PgPool) {
    // Golden vectors of #95 (Python hashlib), recomputed by the SQL function of the seal.
    let form = Uuid::parse_str("a1111111-1111-4111-8111-111111111111").unwrap();
    let t1 = Uuid::parse_str("b2222222-2222-4222-8222-222222222222").unwrap();
    let t2 = Uuid::parse_str("b3333333-3333-4333-8333-333333333333").unwrap();
    let digest = |form: Option<Uuid>, themes: Vec<Uuid>| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT work_metadata_digest($1, $2::uuid[])")
                .bind(form)
                .bind(themes)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let cases = [
        (
            Some(form),
            vec![t1, t2],
            "3caaa54dcf51e0f2ad79f574b6d1ddfe6890a3f55b9604ddffde8882f4798f8a",
        ),
        (
            Some(form),
            vec![t2, t1],
            "373e9f343ff0395bb03624b6720cd6f1f49176b327eee45ac756e05760feaf37",
        ),
        (
            None,
            vec![],
            "9be68828b1f3703c6923321d34c21a29a7f6913220699f9fa9468f1684094c57",
        ),
    ];
    for (form, themes, expected) in cases {
        let rust = MetadataDigest::of(
            &WorkTags::restore(
                form.map(TagId::from_uuid),
                themes.iter().copied().map(TagId::from_uuid).collect(),
            )
            .unwrap(),
        );
        assert_eq!(rust.as_str(), expected);
        assert_eq!(digest(form, themes).await, expected);
    }
    // Shapes without a golden vector: SQL and Rust still agree byte for byte.
    for (form, themes) in [
        (None, vec![t1]),
        (Some(form), vec![]),
        (None, vec![t2, t1, form]),
    ] {
        let rust = MetadataDigest::of(
            &WorkTags::restore(
                form.map(TagId::from_uuid),
                themes.iter().copied().map(TagId::from_uuid).collect(),
            )
            .unwrap(),
        );
        assert_eq!(digest(form, themes).await, rust.as_str());
    }
}

#[sqlx::test(migrations = false)]
async fn upgrade_keeps_existing_untagged_works_readable_without_backfill(pool: PgPool) {
    // The schema before this slice, with data written by the old code.
    let previous = Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < TAXONOMY_MIGRATION)
            .cloned()
            .collect(),
    );
    assert!(previous.migrations.len() < MIGRATOR.migrations.len());
    previous.run(&pool).await.unwrap();
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Pending);
    let vink = seeded(&pool).await;
    let untitled = vink.create_draft(author()).await.unwrap().work_id;
    // Read the old public state with SQL: today's public read needs today's schema.
    let before: (Uuid, String) = sqlx::query_as(
        "SELECT r.id, r.title FROM publications p
           JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
          WHERE p.work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    let hash: String = sqlx::query_scalar("SELECT content_hash FROM revisions WHERE id = $1")
        .bind(dev_seed::SAMPLE_REVISION)
        .fetch_one(&pool)
        .await
        .unwrap();

    // Upgrade through the shared migration lifecycle.
    let report = migrate(&pool).await.unwrap();
    // This slice's migration first; later additive slices (the #97 audit) may follow.
    assert_eq!(report.applied.first(), Some(&TAXONOMY_MIGRATION));
    assert!(report.applied.iter().all(|v| *v >= TAXONOMY_MIGRATION));
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);

    // Nothing was inferred or backfilled; old rows read as untagged.
    assert_eq!(rows(&pool, "work_draft_metadata").await, 0);
    assert_eq!(rows(&pool, "tags").await, 0);
    assert_eq!(catalog_version(&pool).await, CatalogVersion::FIRST);
    for work in [sample_work(), untitled] {
        let view = vink.work_tags(author(), work).await.unwrap();
        assert_eq!(view.version, MetadataVersion::FIRST);
        assert_eq!(view.tags, WorkTags::EMPTY);
    }
    let public = vink.public_work_tags(sample_work()).await.unwrap();
    assert_eq!(public.revision_id.as_uuid(), before.0);
    assert!(public.tags.is_empty());
    let read = vink.publication(sample_work()).await.unwrap();
    assert_eq!(
        (read.revision_id.as_uuid(), read.title.as_str()),
        (before.0, before.1.as_str())
    );
    assert!(read.tags.is_empty());
    let after: String = sqlx::query_scalar("SELECT content_hash FROM revisions WHERE id = $1")
        .bind(dev_seed::SAMPLE_REVISION)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, hash, "content_hash v1 is untouched");

    // The upgraded database works: tag an old work.
    create(&pool, KY_UC, TagFacet::Theme, "Ký ức").await;
    let saved = save_tags(&vink, author(), 1, None, &[KY_UC]).await.unwrap();
    assert_eq!(saved.version.get(), 2);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn catalog_rules_hold_in_sql_without_rust(pool: PgPool) {
    setup(&pool).await;
    let new = Uuid::now_v7();
    // One row per slug, forever; one canonical name per tag; alias spelling only on aliases.
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO tag_names (slug, tag_id, kind) VALUES ('ký-ức', '{GIA_DINH}', 'former')"
        )],
        "23505",
        "tag_names_pkey",
    )
    .await;
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO tag_names (slug, tag_id, kind) VALUES ('ký-ức-khác', '{KY_UC}', 'canonical')"
        )],
        "23505",
        "tag_names_one_canonical_key",
    )
    .await;
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO tag_names (slug, tag_id, kind) VALUES ('hồi-ức', '{KY_UC}', 'alias')"
        )],
        "23514",
        "tag_names_alias_label",
    )
    .await;
    // A tag's slug must be its canonical name row (checked at commit).
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO tags (id, facet, label, slug, status)
             VALUES ('{new}', 'theme', 'Biển', 'biển', 'active')"
        )],
        "23503",
        "tags_canonical_name_fkey",
    )
    .await;
    assert_refused(
        &pool,
        &[
            format!(
                "INSERT INTO tags (id, facet, label, slug, status)
                 VALUES ('{new}', 'theme', 'Biển', 'biển', 'active')"
            ),
            format!(
                "INSERT INTO tag_names (slug, tag_id, kind, alias_label)
                 VALUES ('biển', '{new}', 'alias', 'Biển')"
            ),
        ],
        "23503",
        "tags_canonical_name_fkey",
    )
    .await;
    // Successors: retired only, never itself, same facet.
    assert_refused(
        &pool,
        &[format!(
            "UPDATE tags SET status = 'retired', successor_id = id WHERE id = '{MUA}'"
        )],
        "23514",
        "tags_successor_not_self",
    )
    .await;
    assert_refused(
        &pool,
        &[format!(
            "UPDATE tags SET status = 'retired', successor_id = '{TAN_VAN}' WHERE id = '{MUA}'"
        )],
        "23503",
        "tags_successor_same_facet_fkey",
    )
    .await;
    assert_refused(
        &pool,
        &[format!(
            "UPDATE tags SET successor_id = '{KY_UC}' WHERE id = '{MUA}'"
        )],
        "23514",
        "tags_successor_only_when_retired",
    )
    .await;
    // Identity and facet are immutable; a slug never moves to another tag.
    assert_refused(
        &pool,
        &[format!("UPDATE tags SET facet = 'form' WHERE id = '{MUA}'")],
        "23000",
        "identity and facet are immutable",
    )
    .await;
    assert_refused(
        &pool,
        &[format!(
            "UPDATE tag_names SET tag_id = '{GIA_DINH}' WHERE slug = 'ký-ức'"
        )],
        "23000",
        "belongs to its tag forever",
    )
    .await;
    // Retirement is terminal and freezes the tag's names.
    assert!(matches!(
        retire(&pool, DO_THI, Some(MUA)).await,
        CatalogOutcome::Accepted(_)
    ));
    assert_refused(
        &pool,
        &[format!(
            "UPDATE tags SET status = 'active', successor_id = NULL WHERE id = '{DO_THI}'"
        )],
        "23000",
        "retirement is terminal",
    )
    .await;
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO tag_names (slug, tag_id, kind, alias_label)
             VALUES ('phố', '{DO_THI}', 'alias', 'Phố')"
        )],
        "23000",
        "are frozen",
    )
    .await;
    // Nothing is deleted or truncated; the catalog version only advances.
    for statement in [
        "DELETE FROM tags WHERE false",
        "DELETE FROM tag_names WHERE false",
        "DELETE FROM tag_catalog",
        "TRUNCATE tag_catalog",
    ] {
        assert_refused(
            &pool,
            &[statement.to_string()],
            "23000",
            "rows are retained",
        )
        .await;
    }
    assert_refused(
        &pool,
        &["UPDATE tag_catalog SET version = version".to_string()],
        "23000",
        "catalog version must advance",
    )
    .await;
    assert_eq!(rows(&pool, "tags").await, 6);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn assignment_and_snapshot_rules_hold_in_sql_without_rust(pool: PgPool) {
    let vink = setup(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let metadata = format!("INSERT INTO work_draft_metadata (work_id) VALUES ('{work}')");
    let slot = |slot: i16, id: TagId, facet: &str| {
        format!(
            "INSERT INTO work_draft_tags (work_id, slot, tag_id, facet)
             VALUES ('{work}', {slot}, '{id}', '{facet}')"
        )
    };
    // The schema's slot bound is the core's: one form slot and MAX_THEMES_PER_WORK themes.
    assert_eq!(
        MAX_THEMES_PER_WORK, 3,
        "widen the slot CHECKs in a new migration"
    );
    assert_refused(
        &pool,
        &[metadata.clone(), slot(0, KY_UC, "theme")],
        "23514",
        "work_draft_tags_slot_facet",
    )
    .await;
    assert_refused(
        &pool,
        &[metadata.clone(), slot(1, TAN_VAN, "theme")],
        "23503",
        "work_draft_tags_tag_fkey",
    )
    .await;
    assert_refused(
        &pool,
        &[metadata.clone(), slot(4, MUA, "theme")],
        "23514",
        "work_draft_tags_slot_check",
    )
    .await;
    assert_refused(
        &pool,
        &[
            metadata.clone(),
            slot(1, KY_UC, "theme"),
            slot(2, KY_UC, "theme"),
        ],
        "23505",
        "work_draft_tags_once_key",
    )
    .await;
    assert_refused(
        &pool,
        &[
            metadata.clone(),
            slot(1, TagId::from_uuid(Uuid::now_v7()), "theme"),
        ],
        "23503",
        "work_draft_tags_tag_fkey",
    )
    .await;
    assert_refused(
        &pool,
        &[slot(1, KY_UC, "theme")],
        "23503",
        "work_draft_tags_work_id_fkey",
    )
    .await;
    // Positive control: a full assignment is representable.
    let mut full = vec![metadata.clone(), slot(0, TAN_VAN, "form")];
    for (i, id) in [KY_UC, GIA_DINH, MUA].into_iter().enumerate() {
        full.push(slot(i16::try_from(i + 1).unwrap(), id, "theme"));
    }
    let mut tx = pool.begin().await.unwrap();
    for statement in &full {
        sqlx::raw_sql(AssertSqlSafe(statement.clone()))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.rollback().await.unwrap();

    // Draft metadata versions only advance and rows are kept.
    save_tags(&vink, author(), 1, Some(TAN_VAN), &[KY_UC])
        .await
        .unwrap();
    assert_refused(
        &pool,
        &[format!(
            "UPDATE work_draft_metadata SET version = 1 WHERE work_id = '{work}'"
        )],
        "23000",
        "draft metadata version must advance",
    )
    .await;
    assert_refused(
        &pool,
        &["DELETE FROM work_draft_metadata".to_string()],
        "23000",
        "rows are retained",
    )
    .await;

    // Snapshots are sealed by their digest at commit and immutable afterwards.
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
             VALUES ('{}', '{work}', 1, repeat('0', 64))",
            Uuid::now_v7()
        )],
        "23000",
        "does not match its digest",
    )
    .await;
    let mut publishing = pool.begin().await.unwrap();
    let snapshot = captured(
        capture_metadata_snapshot(&mut publishing, sample_work(), version(2))
            .await
            .unwrap(),
    );
    publishing.commit().await.unwrap();
    assert_refused(
        &pool,
        &[format!(
            "INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
             VALUES ('{snapshot}', 2, '{MUA}', 'theme')"
        )],
        "23000",
        "does not match its digest",
    )
    .await;
    for statement in [
        "UPDATE work_metadata_snapshots SET digest = digest".to_string(),
        "DELETE FROM work_metadata_snapshot_tags".to_string(),
        "TRUNCATE work_metadata_snapshot_tags".to_string(),
        "DELETE FROM work_metadata_snapshots WHERE false".to_string(),
    ] {
        assert_refused(&pool, &[statement], "23000", "rows are immutable").await;
    }
    // A snapshot written by SQL alone passes the seal when its digest is right.
    let manual = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(format!(
        "INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
         VALUES ('{manual}', '{work}', 2, work_metadata_digest(NULL, ARRAY['{MUA}'::uuid]));
         INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
         VALUES ('{manual}', 1, '{MUA}', 'theme');"
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // A publication binds only a snapshot of its own work.
    let foreign_work = Uuid::now_v7();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
        .bind(foreign_work)
        .bind(dev_seed::OTHER_PRINCIPAL)
        .execute(&pool)
        .await
        .unwrap();
    let mut publishing = pool.begin().await.unwrap();
    let foreign = captured(
        capture_metadata_snapshot(
            &mut publishing,
            WorkId::from_uuid(foreign_work),
            MetadataVersion::FIRST,
        )
        .await
        .unwrap(),
    );
    publishing.commit().await.unwrap();
    assert_refused(
        &pool,
        &[format!(
            "UPDATE publications SET metadata_snapshot_id = '{foreign}' WHERE work_id = '{work}'"
        )],
        "23503",
        "publications_metadata_of_work_fkey",
    )
    .await;
    assert_eq!(rows(&pool, "work_metadata_snapshots").await, 3);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_tags_round_trip_and_stay_private_to_their_owner(pool: PgPool) {
    let vink = setup(&pool).await;
    let work = sample_work();
    let saved = save_tags(&vink, author(), 1, Some(TAN_VAN), &[KY_UC, GIA_DINH])
        .await
        .unwrap();
    assert_eq!(saved.version.get(), 2);
    assert!(saved.changed);
    assert_eq!(saved.tags, tags(Some(TAN_VAN), &[KY_UC, GIA_DINH]));
    // A new pool: the answer was durable before it was given.
    let options = pool.connect_options();
    let reopened = Vink::new(PgStore::new(
        PgPoolOptions::new()
            .connect_with((*options).clone())
            .await
            .unwrap(),
    ));
    let view = reopened.work_tags(author(), work).await.unwrap();
    assert_eq!(view.version.get(), 2);
    assert_eq!(shown(&view.shown), [TAN_VAN, KY_UC, GIA_DINH]);
    assert_eq!(view.shown[1].label().as_str(), "Ký ức");

    // Another owner and a missing work are the same answer; neither reads nor writes anything.
    let missing = WorkId::from_uuid(Uuid::now_v7());
    for (actor, target) in [(other(), work), (author(), missing), (other(), missing)] {
        assert_eq!(vink.work_tags(actor, target).await, Err(TagError::NotFound));
        let attempt = SaveWorkTags {
            work_id: target,
            ..command(2, Some(THO), &[MUA])
        };
        assert_eq!(
            vink.save_work_tags(actor, attempt).await,
            Err(TagError::NotFound)
        );
    }
    assert_eq!(rows(&pool, "work_draft_metadata").await, 1);
    assert_eq!(
        draft_state(&pool).await,
        (Some(2), slots(Some(TAN_VAN), &[KY_UC, GIA_DINH]))
    );
    assert_eq!(
        vink.work_tags(None, work).await,
        Err(TagError::Unauthenticated)
    );
    assert_eq!(
        save_tags(&vink, principal(Uuid::now_v7()), 2, None, &[]).await,
        Err(TagError::Unauthenticated)
    );

    // Tags are not text: the draft text version, the revision and its hash are untouched, and
    // the public side shows nothing until a publication binds a snapshot.
    let draft = vink.load_draft(author(), work).await.unwrap();
    assert_eq!(draft.version, DraftVersion::FIRST);
    assert_eq!(rows(&pool, "revisions").await, 1);
    assert_eq!(rows(&pool, "work_metadata_snapshots").await, 0);
    assert!(vink.public_work_tags(work).await.unwrap().tags.is_empty());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn stale_tabs_conflict_and_every_accepted_save_advances(pool: PgPool) {
    let vink = setup(&pool).await;
    save_tags(&vink, author(), 1, None, &[KY_UC]).await.unwrap();
    // Tab A saves; tab B, which also saw version 2, arrives late.
    let a = save_tags(&vink, author(), 2, None, &[KY_UC, GIA_DINH])
        .await
        .unwrap();
    assert_eq!(a.version.get(), 3);
    assert_eq!(
        save_tags(&vink, author(), 2, Some(THO), &[]).await,
        Err(TagError::VersionConflict {
            current: version(3),
            observed: tags(None, &[KY_UC, GIA_DINH]),
        })
    );
    // Tab A lost its acknowledgement and resends: a conflict whose observed state matches.
    match save_tags(&vink, author(), 2, None, &[KY_UC, GIA_DINH]).await {
        Err(TagError::VersionConflict { current, observed }) => {
            assert_eq!(current.get(), 3);
            assert_eq!(observed, a.tags);
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
    // A same-state save is accepted, writes no slot and still advances the version.
    let same = save_tags(&vink, author(), 3, None, &[KY_UC, GIA_DINH])
        .await
        .unwrap();
    assert_eq!((same.version.get(), same.changed), (4, false));
    assert_eq!(
        draft_state(&pool).await,
        (Some(4), slots(None, &[KY_UC, GIA_DINH]))
    );
    // Clearing is a save like any other.
    let cleared = save_tags(&vink, author(), 4, None, &[]).await.unwrap();
    assert_eq!(cleared.tags, WorkTags::EMPTY);
    assert_eq!(draft_state(&pool).await, (Some(5), vec![]));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn catalog_refusals_and_suspension_write_nothing(pool: PgPool) {
    let vink = setup(&pool).await;
    save_tags(&vink, author(), 1, Some(TAN_VAN), &[DO_THI])
        .await
        .unwrap();
    assert!(matches!(
        retire(&pool, DO_THI, Some(MUA)).await,
        CatalogOutcome::Accepted(_)
    ));
    let unknown = TagId::from_uuid(Uuid::now_v7());
    let refusals = [
        (
            save_tags(&vink, author(), 2, None, &[unknown]).await,
            TagRejection::UnknownTag {
                tag: unknown,
                slot: TagFacet::Theme,
            },
        ),
        (
            save_tags(&vink, author(), 2, Some(KY_UC), &[]).await,
            TagRejection::WrongFacet {
                tag: KY_UC,
                slot: TagFacet::Form,
            },
        ),
        (
            save_tags(&vink, author(), 2, None, &[KY_UC, GIA_DINH, MUA, DO_THI]).await,
            TagRejection::TooManyThemes,
        ),
    ];
    for (result, rejection) in refusals {
        assert_eq!(result, Err(TagError::Rejected(rejection)));
    }
    // The retired tag stays where it already is; it cannot be added elsewhere.
    let kept = save_tags(&vink, author(), 2, Some(THO), &[DO_THI, KY_UC])
        .await
        .unwrap();
    assert_eq!(kept.version.get(), 3);
    let view = vink.work_tags(author(), sample_work()).await.unwrap();
    assert!(!view.shown[1].status().is_active());
    let other_work = vink.create_draft(author()).await.unwrap().work_id;
    assert_eq!(
        vink.save_work_tags(
            author(),
            SaveWorkTags {
                work_id: other_work,
                ..command(1, None, &[DO_THI])
            }
        )
        .await,
        Err(TagError::Rejected(TagRejection::RetiredTag {
            tag: DO_THI,
            slot: TagFacet::Theme,
            successor: Some(MUA),
        }))
    );
    // A suspended author is refused before any catalog rule.
    let reason = SuspensionReason::parse("Tạm khóa trong bộ test thẻ").unwrap();
    suspend_principal(
        &pool,
        PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR),
        &reason,
    )
    .await
    .unwrap();
    assert_eq!(
        save_tags(&vink, author(), 3, None, &[unknown]).await,
        Err(TagError::Suspended)
    );
    assert_eq!(
        draft_state(&pool).await,
        (Some(3), slots(Some(THO), &[DO_THI, KY_UC]))
    );
    assert_eq!(rows(&pool, "work_draft_metadata").await, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn text_and_tag_saves_of_one_work_do_not_conflict(pool: PgPool) {
    // CHOICE-02 B evidence: independent versions, so a tag save never fails a text autosave.
    let vink = setup(&pool).await;
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let text = {
        let (vink, barrier) = (Arc::clone(&vink), Arc::clone(&barrier));
        tokio::spawn(async move {
            barrier.wait().await;
            vink.save_draft(
                author(),
                save(sample_work(), 1, "Nháp mới", doc(&[&["Câu đầu."]])),
            )
            .await
        })
    };
    let metadata = {
        let (vink, barrier) = (Arc::clone(&vink), Arc::clone(&barrier));
        tokio::spawn(async move {
            barrier.wait().await;
            save_tags(&vink, author(), 1, Some(TAN_VAN), &[KY_UC]).await
        })
    };
    assert_eq!(text.await.unwrap().unwrap().version.get(), 2);
    assert_eq!(metadata.await.unwrap().unwrap().version.get(), 2);
    let draft = vink.load_draft(author(), sample_work()).await.unwrap();
    assert_eq!(draft.title.as_str(), "Nháp mới");
    assert_eq!(
        draft_state(&pool).await,
        (Some(2), slots(Some(TAN_VAN), &[KY_UC]))
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_saves_of_one_metadata_version_have_one_winner(pool: PgPool) {
    let vink = setup(&pool).await;
    // First saves race on an absent row: the lazy insert lets exactly one through.
    let choices: [&[TagId]; 4] = [&[KY_UC], &[GIA_DINH], &[MUA], &[KY_UC, MUA]];
    let barrier = Arc::new(tokio::sync::Barrier::new(choices.len()));
    let tasks: Vec<_> = choices
        .iter()
        .map(|themes| {
            let (vink, barrier) = (Arc::clone(&vink), Arc::clone(&barrier));
            let themes = themes.to_vec();
            tokio::spawn(async move {
                barrier.wait().await;
                save_tags(&vink, author(), 1, None, &themes).await
            })
        })
        .collect();
    let mut winners = Vec::new();
    let mut observed = Vec::new();
    for task in tasks {
        match task.await.unwrap() {
            Ok(saved) => winners.push(saved.tags),
            Err(TagError::VersionConflict {
                current,
                observed: seen,
            }) => {
                assert_eq!(current.get(), 2);
                observed.push(seen);
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(winners.len(), 1, "exactly one save of version 1 wins");
    assert!(observed.iter().all(|seen| *seen == winners[0]));
    let (stored, _) = draft_state(&pool).await;
    assert_eq!(stored, Some(2));
    assert_eq!(
        vink.work_tags(author(), sample_work()).await.unwrap().tags,
        winners[0]
    );

    // Both commit orders against a writer that holds the metadata row.
    let mut writer = pool.begin().await.unwrap();
    let pid = backend_pid(&mut writer).await;
    sqlx::query("UPDATE work_draft_metadata SET version = version + 1 WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *writer)
        .await
        .unwrap();
    let racing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 2, Some(THO), &[]).await })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(!racing.is_finished());
    writer.commit().await.unwrap();
    assert!(matches!(
        racing.await.unwrap(),
        Err(TagError::VersionConflict { current, .. }) if current.get() == 3
    ));

    let mut writer = pool.begin().await.unwrap();
    let pid = backend_pid(&mut writer).await;
    sqlx::query("UPDATE work_draft_metadata SET version = version + 1 WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *writer)
        .await
        .unwrap();
    let racing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 3, Some(THO), &[]).await })
    };
    wait_blocked_by(&pool, pid).await;
    writer.rollback().await.unwrap();
    assert_eq!(racing.await.unwrap().unwrap().version.get(), 4);
    assert_eq!(draft_state(&pool).await, (Some(4), slots(Some(THO), &[])));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn retirement_and_assignment_serialize_on_the_catalog_row(pool: PgPool) {
    let vink = setup(&pool).await;
    save_tags(&vink, author(), 1, None, &[KY_UC]).await.unwrap();

    // Order A: a retirement in flight holds the tag row (paused on its successor's row); a save
    // that adds the tag waits, then sees it retired and writes nothing.
    let mut pauser = pool.begin().await.unwrap();
    let pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM tags WHERE id = $1 FOR UPDATE")
        .bind(MUA.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let retiring = {
        let pool = pool.clone();
        tokio::spawn(async move { retire(&pool, DO_THI, Some(MUA)).await })
    };
    wait_blocked_by(&pool, pid).await;
    let adding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 2, None, &[KY_UC, DO_THI]).await })
    };
    wait_blocked(&pool, 2).await;
    assert!(!adding.is_finished(), "the save waits for the retirement");
    pauser.rollback().await.unwrap();
    assert!(matches!(
        retiring.await.unwrap(),
        CatalogOutcome::Accepted(_)
    ));
    assert_eq!(
        adding.await.unwrap(),
        Err(TagError::Rejected(TagRejection::RetiredTag {
            tag: DO_THI,
            slot: TagFacet::Theme,
            successor: Some(MUA),
        }))
    );
    assert_eq!(draft_state(&pool).await, (Some(2), slots(None, &[KY_UC])));

    // Order B: a save in flight holds GIA_DINH FOR SHARE (paused on the metadata row); its
    // retirement waits, so the save commits with the tag, which then stays (kept, not added).
    let mut pauser = pool.begin().await.unwrap();
    let pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM work_draft_metadata WHERE work_id = $1 FOR UPDATE")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *pauser)
        .await
        .unwrap();
    let adding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 2, None, &[KY_UC, GIA_DINH]).await })
    };
    wait_blocked_by(&pool, pid).await;
    let retiring = {
        let pool = pool.clone();
        tokio::spawn(async move { retire(&pool, GIA_DINH, None).await })
    };
    wait_blocked(&pool, 2).await;
    assert!(!retiring.is_finished(), "the retirement waits for the save");
    pauser.rollback().await.unwrap();
    assert_eq!(adding.await.unwrap().unwrap().version.get(), 3);
    assert!(matches!(
        retiring.await.unwrap(),
        CatalogOutcome::Accepted(_)
    ));
    let view = vink.work_tags(author(), sample_work()).await.unwrap();
    assert_eq!(shown(&view.shown), [KY_UC, GIA_DINH]);
    assert!(!view.shown[1].status().is_active());
    // Keeping it is still allowed; the history is not rewritten.
    assert_eq!(
        save_tags(&vink, author(), 3, Some(TAN_VAN), &[KY_UC, GIA_DINH])
            .await
            .unwrap()
            .version
            .get(),
        4
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn suspension_and_tag_saves_serialize_on_the_principal(pool: PgPool) {
    let vink = setup(&pool).await;
    let author_id = PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR);

    // Order A: an uncommitted suspension (its foreign key holds the principal FOR KEY SHARE);
    // the save waits for it, then reads the committed suspension and writes nothing.
    let mut suspending = pool.begin().await.unwrap();
    let pid = backend_pid(&mut suspending).await;
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason)
         VALUES ($1, $2, 'Tạm khóa trong bộ test thẻ')",
    )
    .bind(Uuid::now_v7())
    .bind(dev_seed::SAMPLE_AUTHOR)
    .execute(&mut *suspending)
    .await
    .unwrap();
    let saving = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 1, None, &[KY_UC]).await })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(!saving.is_finished(), "the save waits for the suspension");
    suspending.commit().await.unwrap();
    assert_eq!(saving.await.unwrap(), Err(TagError::Suspended));
    assert_eq!(draft_state(&pool).await, (None, vec![]));
    reinstate_principal(&pool, author_id).await.unwrap();

    // Order B: a save in flight holds the principal (paused on the metadata row); the real
    // suspension command waits, so the save commits first and the next save is refused.
    save_tags(&vink, author(), 1, None, &[KY_UC]).await.unwrap();
    let mut pauser = pool.begin().await.unwrap();
    let pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM work_draft_metadata WHERE work_id = $1 FOR UPDATE")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *pauser)
        .await
        .unwrap();
    let saving = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 2, None, &[KY_UC, MUA]).await })
    };
    wait_blocked_by(&pool, pid).await;
    let suspended = {
        let pool = pool.clone();
        tokio::spawn(async move {
            let reason = SuspensionReason::parse("Tạm khóa lần hai trong bộ test thẻ").unwrap();
            suspend_principal(&pool, author_id, &reason).await
        })
    };
    wait_blocked(&pool, 2).await;
    assert!(
        !suspended.is_finished(),
        "the suspension waits for the save"
    );
    pauser.rollback().await.unwrap();
    assert_eq!(saving.await.unwrap().unwrap().version.get(), 3);
    suspended.await.unwrap().unwrap();
    assert_eq!(
        save_tags(&vink, author(), 3, None, &[]).await,
        Err(TagError::Suspended)
    );
    assert_eq!(
        draft_state(&pool).await,
        (Some(3), slots(None, &[KY_UC, MUA]))
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn public_tags_follow_the_binding_through_edits_replacement_and_withdrawal(pool: PgPool) {
    let vink = setup(&pool).await;
    let work = sample_work();
    let r1 = RevisionId::from_uuid(dev_seed::SAMPLE_REVISION);
    // A publication without metadata shows no tags, and a private edit changes nothing public.
    save_tags(&vink, author(), 1, Some(TAN_VAN), &[KY_UC, GIA_DINH])
        .await
        .unwrap();
    let public = vink.public_work_tags(work).await.unwrap();
    assert_eq!((public.revision_id, public.tags.len()), (r1, 0));

    // FIXTURE publication: capture the confirmed draft metadata and bind it beside R1.
    let mut publishing = pool.begin().await.unwrap();
    let s1 = captured(
        capture_metadata_snapshot(&mut publishing, work, version(2))
            .await
            .unwrap(),
    );
    bind(&mut publishing, dev_seed::SAMPLE_REVISION, s1).await;
    publishing.commit().await.unwrap();
    let public = vink.public_work_tags(work).await.unwrap();
    assert_eq!(public.revision_id, r1);
    assert_eq!(shown(&public.tags), [TAN_VAN, KY_UC, GIA_DINH]);

    // The author keeps editing privately: the public snapshot does not move.
    save_tags(&vink, author(), 2, Some(THO), &[MUA])
        .await
        .unwrap();
    assert_eq!(
        shown(&vink.public_work_tags(work).await.unwrap().tags),
        [TAN_VAN, KY_UC, GIA_DINH]
    );
    // A rename shows everywhere (snapshots hold identities); a retired tag stays in the snapshot
    // but is no longer shown.
    assert!(matches!(
        curate(
            &pool,
            CatalogChange::Rename {
                id: GIA_DINH,
                label: label("Gia đình Việt"),
            },
        )
        .await,
        CatalogOutcome::Accepted(_)
    ));
    assert!(matches!(
        retire(&pool, KY_UC, None).await,
        CatalogOutcome::Accepted(_)
    ));
    let public = vink.public_work_tags(work).await.unwrap();
    assert_eq!(shown(&public.tags), [TAN_VAN, GIA_DINH]);
    assert_eq!(public.tags[1].label().as_str(), "Gia đình Việt");

    // Replacement: an uncommitted pointer move to (R2, S2) is invisible and blocks no reader;
    // after commit the pair moves together, and the text read agrees on the revision.
    let mut publishing = pool.begin().await.unwrap();
    let r2 = new_revision(&mut publishing).await;
    let s2 = captured(
        capture_metadata_snapshot(&mut publishing, work, version(3))
            .await
            .unwrap(),
    );
    bind(&mut publishing, r2, s2).await;
    let during = vink.public_work_tags(work).await.unwrap();
    assert_eq!(during.revision_id, r1);
    assert_eq!(shown(&during.tags), [TAN_VAN, GIA_DINH]);
    publishing.commit().await.unwrap();
    let after = vink.public_work_tags(work).await.unwrap();
    assert_eq!(after.revision_id, RevisionId::from_uuid(r2));
    assert_eq!(shown(&after.tags), [THO, MUA]);
    assert_eq!(
        vink.publication(work).await.unwrap().revision_id,
        after.revision_id
    );

    // Withdrawal: invisible until commit; a read starting after the commit finds neither the
    // text nor the metadata. The snapshots stay as immutable history.
    let mut withdrawing = pool.begin().await.unwrap();
    sqlx::query(WITHDRAW)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *withdrawing)
        .await
        .unwrap();
    assert_eq!(
        shown(&vink.public_work_tags(work).await.unwrap().tags),
        [THO, MUA]
    );
    withdrawing.commit().await.unwrap();
    assert_eq!(vink.public_work_tags(work).await, Err(ReadError::NotFound));
    assert_eq!(vink.publication(work).await, Err(ReadError::NotFound));
    assert_eq!(rows(&pool, "work_metadata_snapshots").await, 2);
    // The draft is still the author's, untouched by publication changes.
    let view = vink.work_tags(author(), work).await.unwrap();
    assert_eq!(
        (view.version.get(), shown(&view.shown)),
        (3, vec![THO, MUA])
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn capture_and_tag_saves_cannot_publish_a_mismatched_snapshot(pool: PgPool) {
    let vink = setup(&pool).await;
    let work = sample_work();
    save_tags(&vink, author(), 1, Some(TAN_VAN), &[KY_UC])
        .await
        .unwrap();

    // Capture first: the publication transaction holds the metadata row FOR SHARE, so the
    // author's next save waits and cannot slip in between capture and binding.
    let mut publishing = pool.begin().await.unwrap();
    let pid = backend_pid(&mut publishing).await;
    let s1 = captured(
        capture_metadata_snapshot(&mut publishing, work, version(2))
            .await
            .unwrap(),
    );
    let saving = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 2, Some(THO), &[MUA]).await })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(!saving.is_finished(), "the save waits for the publication");
    bind(&mut publishing, dev_seed::SAMPLE_REVISION, s1).await;
    publishing.commit().await.unwrap();
    assert_eq!(saving.await.unwrap().unwrap().version.get(), 3);
    assert_eq!(
        shown(&vink.public_work_tags(work).await.unwrap().tags),
        [TAN_VAN, KY_UC]
    );

    // Save first: a save in flight holds the row FOR UPDATE (paused on its slot rows); the
    // capture of the version the author confirmed waits, then finds a newer version and writes
    // nothing, so the publication cannot bind tags the author did not confirm.
    let mut pauser = pool.begin().await.unwrap();
    let pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM work_draft_tags WHERE work_id = $1 FOR UPDATE")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *pauser)
        .await
        .unwrap();
    let saving = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { save_tags(&vink, author(), 3, None, &[GIA_DINH]).await })
    };
    wait_blocked_by(&pool, pid).await;
    let capturing = {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut publishing = pool.begin().await.unwrap();
            let capture = capture_metadata_snapshot(&mut publishing, work, version(3)).await;
            publishing.rollback().await.unwrap();
            capture
        })
    };
    wait_blocked(&pool, 2).await;
    assert!(!capturing.is_finished(), "the capture waits for the save");
    pauser.rollback().await.unwrap();
    assert_eq!(saving.await.unwrap().unwrap().version.get(), 4);
    assert_eq!(
        capturing.await.unwrap().unwrap(),
        SnapshotCapture::Conflict {
            current: version(4)
        }
    );
    assert_eq!(rows(&pool, "work_metadata_snapshots").await, 1);
    assert_eq!(
        shown(&vink.public_work_tags(work).await.unwrap().tags),
        [TAN_VAN, KY_UC]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_failure_after_partial_work_rolls_the_whole_transaction_back(pool: PgPool) {
    let vink = setup(&pool).await;
    save_tags(&vink, author(), 1, Some(TAN_VAN), &[KY_UC])
        .await
        .unwrap();
    // Test-only fault injection: the slot INSERT fails after the version step and the DELETE.
    sqlx::raw_sql(
        "CREATE FUNCTION inject_test_failure() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'injected failure (test only)'; END; $$;
         CREATE TRIGGER inject_test_failure BEFORE INSERT ON work_draft_tags
             FOR EACH ROW EXECUTE FUNCTION inject_test_failure();
         CREATE TRIGGER inject_test_failure BEFORE INSERT ON tag_names
             FOR EACH ROW EXECUTE FUNCTION inject_test_failure();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        save_tags(&vink, author(), 2, Some(THO), &[MUA]).await,
        Err(TagError::Internal)
    );
    assert_eq!(
        draft_state(&pool).await,
        (Some(2), slots(Some(TAN_VAN), &[KY_UC]))
    );
    let before = catalog_version(&pool).await;
    let created = change_catalog(
        &pool,
        CuratorAuthority::operator(),
        &CatalogCommand {
            expected: before,
            change: CatalogChange::Create {
                id: TagId::generate(),
                facet: TagFacet::Theme,
                label: label("Biển"),
                description: Some(TagDescription::parse("Biển, sóng và bờ.").unwrap()),
            },
        },
    )
    .await;
    assert!(
        matches!(created, Err(StoreError::Integrity(_))),
        "{created:?}"
    );
    assert_eq!(catalog_version(&pool).await, before);
    assert_eq!(rows(&pool, "tags").await, 6);

    sqlx::raw_sql(
        "DROP TRIGGER inject_test_failure ON work_draft_tags;
         DROP TRIGGER inject_test_failure ON tag_names;",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        save_tags(&vink, author(), 2, Some(THO), &[MUA])
            .await
            .unwrap()
            .version
            .get(),
        3
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn catalog_changes_keep_every_slug_with_its_tag(pool: PgPool) {
    setup(&pool).await;
    let names = |pool: PgPool| async move {
        // Sorted in Rust (code points), independent of the database collation.
        let mut names = sqlx::query_as::<_, (String, Uuid, String, Option<String>)>(
            "SELECT slug, tag_id, kind, alias_label FROM tag_names WHERE tag_id = $1",
        )
        .bind(KY_UC.as_uuid())
        .fetch_all(&pool)
        .await
        .unwrap();
        names.sort();
        names
    };
    let row = |slug: &str, kind: &str, alias: Option<&str>| {
        (
            slug.to_string(),
            KY_UC.as_uuid(),
            kind.to_string(),
            alias.map(str::to_string),
        )
    };
    let accepted = |outcome: CatalogOutcome| {
        assert!(
            matches!(outcome, CatalogOutcome::Accepted(_)),
            "{outcome:?}"
        );
    };
    accepted(
        curate(
            &pool,
            CatalogChange::AddAlias {
                target: KY_UC,
                label: label("Hồi ức"),
            },
        )
        .await,
    );
    // Rename: the old slug becomes a former name (redirect); the identity stays.
    accepted(
        curate(
            &pool,
            CatalogChange::Rename {
                id: KY_UC,
                label: label("Kỷ niệm"),
            },
        )
        .await,
    );
    assert_eq!(
        names(pool.clone()).await,
        [
            row("hồi-ức", "alias", Some("Hồi ức")),
            row("ký-ức", "former", None),
            row("kỷ-niệm", "canonical", None),
        ]
    );
    // Taking back a former name and promoting an alias both keep the slug with its tag.
    accepted(
        curate(
            &pool,
            CatalogChange::Rename {
                id: KY_UC,
                label: label("Ký ức"),
            },
        )
        .await,
    );
    accepted(
        curate(
            &pool,
            CatalogChange::Rename {
                id: KY_UC,
                label: label("Hồi ức"),
            },
        )
        .await,
    );
    assert_eq!(
        names(pool.clone()).await,
        [
            row("hồi-ức", "canonical", None),
            row("ký-ức", "former", None),
            row("kỷ-niệm", "former", None),
        ]
    );
    // No other tag can take any of them, whatever the spelling.
    assert_eq!(
        curate(
            &pool,
            CatalogChange::Create {
                id: TagId::generate(),
                facet: TagFacet::Theme,
                label: label("KÝ  ỨC"),
                description: None,
            },
        )
        .await,
        CatalogOutcome::Refused(CatalogRefusal::SlugTaken { holder: KY_UC })
    );
    // Marks distinguish names: má, mà and ma are three tags with three slugs.
    for text in ["má", "mà", "ma"] {
        create(&pool, TagId::generate(), TagFacet::Theme, text).await;
    }
    // Successor rules and stale versions are refused without writing.
    let before = catalog_version(&pool).await;
    assert_eq!(
        retire(&pool, MUA, Some(TAN_VAN)).await,
        CatalogOutcome::Refused(CatalogRefusal::InvalidSuccessor(SuccessorError::OtherFacet))
    );
    let stale = change_catalog(
        &pool,
        CuratorAuthority::operator(),
        &CatalogCommand {
            expected: CatalogVersion::FIRST,
            change: CatalogChange::Describe {
                id: MUA,
                description: None,
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(stale, CatalogOutcome::Conflict { current: before });
    assert_eq!(catalog_version(&pool).await, before);
    // A same-state change is accepted and still advances the version.
    accepted(
        curate(
            &pool,
            CatalogChange::Describe {
                id: MUA,
                description: None,
            },
        )
        .await,
    );
    assert_eq!(catalog_version(&pool).await, before.next().unwrap());

    // Concurrent creates at one catalog version: one winner, the rest conflict.
    let expected = catalog_version(&pool).await;
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let tasks: Vec<_> = ["Biển", "Sông", "Núi"]
        .into_iter()
        .map(|text| {
            let (pool, barrier) = (pool.clone(), Arc::clone(&barrier));
            tokio::spawn(async move {
                barrier.wait().await;
                change_catalog(
                    &pool,
                    CuratorAuthority::operator(),
                    &CatalogCommand {
                        expected,
                        change: CatalogChange::Create {
                            id: TagId::generate(),
                            facet: TagFacet::Theme,
                            label: label(text),
                            description: None,
                        },
                    },
                )
                .await
                .unwrap()
            })
        })
        .collect();
    let mut accepted_count = 0;
    for task in tasks {
        match task.await.unwrap() {
            CatalogOutcome::Accepted(_) => accepted_count += 1,
            CatalogOutcome::Conflict { current } => assert_eq!(current, expected.next().unwrap()),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(accepted_count, 1);
    assert_eq!(rows(&pool, "tags").await, 6 + 3 + 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_fresh_connection_sees_only_committed_catalog_and_draft_state(pool: PgPool) {
    // Read paths take one read-only snapshot; an uncommitted save is not visible to the owner's
    // own read and does not block it.
    let vink = setup(&pool).await;
    save_tags(&vink, author(), 1, None, &[KY_UC]).await.unwrap();
    let mut conn = PgConnection::connect_with(&pool.connect_options())
        .await
        .unwrap();
    let mut writer = conn.begin().await.unwrap();
    sqlx::query("UPDATE work_draft_metadata SET version = 9 WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&mut *writer)
        .await
        .unwrap();
    let view = vink.work_tags(author(), sample_work()).await.unwrap();
    assert_eq!(view.version.get(), 2);
    writer.rollback().await.unwrap();
}
