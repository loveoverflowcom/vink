//! PostgreSQL integration tests. Each test gets a fresh database with all migrations applied
//! (`#[sqlx::test]`), so `DATABASE_URL` must point at a server where the role may create
//! databases. Run with `cargo xtask verify postgres`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;
use vink_app::application::{DraftError, SaveDraft, Vink};
use vink_app::dev_seed;
use vink_app::domain::document::{BlockData, Document, DocumentData, SCHEMA_VERSION, Title};
use vink_app::domain::draft::DraftVersion;
use vink_app::domain::ids::{PrincipalId, WorkId};
use vink_app::domain::revision::RevisionContent;
use vink_app::http::{AppState, router};
use vink_app::identity::IdentityResolver;
use vink_app::persistence::{PgStore, SchemaState, schema_state};

const ORIGIN: &str = "http://127.0.0.1:3000";

fn principal(id: Uuid) -> Option<PrincipalId> {
    Some(PrincipalId::from_uuid(id))
}

fn author() -> Option<PrincipalId> {
    principal(dev_seed::SAMPLE_AUTHOR)
}

fn other() -> Option<PrincipalId> {
    principal(dev_seed::OTHER_PRINCIPAL)
}

fn doc(blocks: &[&[&str]]) -> DocumentData {
    DocumentData {
        schema_version: SCHEMA_VERSION,
        blocks: blocks
            .iter()
            .map(|lines| BlockData {
                lines: lines.iter().map(|l| l.to_string()).collect(),
            })
            .collect(),
    }
}

fn save(work_id: WorkId, expected_version: i64, title: &str, document: DocumentData) -> SaveDraft {
    SaveDraft {
        work_id,
        expected_version,
        title: title.into(),
        document,
    }
}

async fn seeded(pool: &PgPool) -> Vink<PgStore> {
    dev_seed::apply(pool).await.unwrap();
    Vink::new(PgStore::new(pool.clone()))
}

fn sample_work() -> WorkId {
    WorkId::from_uuid(dev_seed::SAMPLE_WORK)
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn fresh_database_migrates_to_current_schema(pool: PgPool) {
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);
    // Seeding twice is a no-op, never a failure or a duplicate.
    seeded(&pool).await;
    seeded(&pool).await;
    let works: i64 = sqlx::query_scalar("SELECT count(*) FROM works")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(works, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn confirmed_save_survives_a_new_connection_pool(pool: PgPool) {
    let vink = seeded(&pool).await;
    let draft = vink.create_draft(author()).await.unwrap();
    assert_eq!(draft.version, DraftVersion::FIRST);
    let saved = vink
        .save_draft(
            author(),
            save(
                draft.work_id,
                1,
                "Nháp",
                doc(&[&["Dòng một,", "dòng hai."], &["Đoạn sau."]]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(saved.version.get(), 2);

    // Simulate a process restart: close every connection and read through a new pool.
    let options = pool.connect_options();
    pool.close().await;
    let reopened = PgPoolOptions::new()
        .connect_with((*options).clone())
        .await
        .unwrap();
    let loaded = Vink::new(PgStore::new(reopened))
        .load_draft(author(), draft.work_id)
        .await
        .unwrap();
    assert_eq!(loaded.version.get(), 2);
    assert_eq!(loaded.title.as_str(), "Nháp");
    assert_eq!(
        loaded.document.to_data(),
        doc(&[&["Dòng một,", "dòng hai."], &["Đoạn sau."]])
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn another_actor_cannot_read_list_or_write_a_private_draft(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    assert_eq!(
        vink.load_draft(other(), work).await,
        Err(DraftError::NotFound)
    );
    assert!(vink.list_drafts(other()).await.unwrap().is_empty());
    assert_eq!(
        vink.save_draft(other(), save(work, 1, "Chiếm", doc(&[&["ghi đè"]])))
            .await,
        Err(DraftError::NotFound)
    );
    assert_eq!(
        vink.load_draft(None, work).await,
        Err(DraftError::Unauthenticated)
    );

    let untouched = vink.load_draft(author(), work).await.unwrap();
    assert_eq!(untouched.version, DraftVersion::FIRST);
    assert_eq!(untouched.title.as_str(), dev_seed::SAMPLE_TITLE);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn unknown_principal_is_not_an_authenticated_actor(pool: PgPool) {
    let vink = seeded(&pool).await;
    assert_eq!(
        vink.create_draft(principal(Uuid::now_v7())).await.err(),
        Some(DraftError::Unauthenticated)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn stale_expected_version_conflicts_without_writing(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    vink.save_draft(author(), save(work, 1, "Tab A", doc(&[&["bản của tab A"]])))
        .await
        .unwrap();
    let stale = vink
        .save_draft(author(), save(work, 1, "Tab B", doc(&[&["bản của tab B"]])))
        .await;
    assert_eq!(
        stale,
        Err(DraftError::VersionConflict {
            current: DraftVersion::new(2).unwrap()
        })
    );
    let stored = vink.load_draft(author(), work).await.unwrap();
    assert_eq!(stored.title.as_str(), "Tab A");
    assert_eq!(stored.document.to_data(), doc(&[&["bản của tab A"]]));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_saves_of_one_version_have_exactly_one_winner(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let work = sample_work();
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let tasks: Vec<_> = (0..8)
        .map(|i| {
            let vink = vink.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                let text = format!("người viết {i}");
                barrier.wait().await;
                let result = vink
                    .save_draft(author(), save(work, 1, &text, doc(&[&[text.as_str()]])))
                    .await;
                (text, result)
            })
        })
        .collect();
    let mut winners = Vec::new();
    for task in tasks {
        let (text, result) = task.await.unwrap();
        match result {
            Ok(saved) => {
                assert_eq!(saved.version.get(), 2);
                winners.push(text);
            }
            Err(error) => assert_eq!(
                error,
                DraftError::VersionConflict {
                    current: DraftVersion::new(2).unwrap()
                }
            ),
        }
    }
    assert_eq!(winners.len(), 1, "exactly one save may apply");
    let stored = vink.load_draft(author(), work).await.unwrap();
    assert_eq!(stored.title.as_str(), winners[0]);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn vietnamese_text_lines_and_stanzas_round_trip(pool: PgPool) {
    let vink = seeded(&pool).await;
    let draft = vink.create_draft(author()).await.unwrap();
    // "Thuở" with a combining hook above, and indentation inside a stanza.
    let input = doc(&[
        &[
            "Thuo\u{031B}\u{0309} ấy, trăng còn non,",
            "    gió đầu ngõ ỹ ỷ ữ ợ Đ.",
        ],
        &["Khổ hai — một dòng."],
    ]);
    vink.save_draft(author(), save(draft.work_id, 1, "Thử dấu", input))
        .await
        .unwrap();
    let stored = vink.load_draft(author(), draft.work_id).await.unwrap();
    assert_eq!(
        stored.document.to_data(),
        doc(&[
            &["Thuở ấy, trăng còn non,", "    gió đầu ngõ ỹ ỷ ữ ợ Đ."],
            &["Khổ hai — một dòng."],
        ])
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn public_read_serves_the_published_snapshot_not_the_draft(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    vink.save_draft(
        author(),
        save(work, 1, "Tiêu đề đang sửa", doc(&[&["Bản sửa riêng tư."]])),
    )
    .await
    .unwrap();

    let public = vink.publication(work).await.unwrap();
    assert_eq!(public.title.as_str(), dev_seed::SAMPLE_TITLE);
    let lines: Vec<String> = public
        .document
        .to_data()
        .blocks
        .into_iter()
        .flat_map(|b| b.lines)
        .collect();
    assert!(lines.iter().any(|l| l == dev_seed::PUBLISHED_STANZAS[0][0]));
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("riêng tư") || l == dev_seed::DRAFT_ONLY_LINE)
    );
    let listed = vink.public_works().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title.as_str(), dev_seed::SAMPLE_TITLE);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn snapshots_are_immutable_and_pointers_stay_within_their_work(pool: PgPool) {
    seeded(&pool).await;
    let update = sqlx::query("UPDATE revisions SET title = 'đổi' WHERE id = $1")
        .bind(dev_seed::SAMPLE_REVISION)
        .execute(&pool)
        .await;
    assert!(update.is_err(), "revision UPDATE must be rejected");

    let foreign_work = Uuid::now_v7();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
        .bind(foreign_work)
        .bind(dev_seed::OTHER_PRINCIPAL)
        .execute(&pool)
        .await
        .unwrap();
    let pointer = sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, now())",
    )
    .bind(foreign_work)
    .bind(dev_seed::SAMPLE_REVISION)
    .execute(&pool)
    .await;
    assert!(
        pointer.is_err(),
        "a pointer cannot reference another work's revision"
    );
}

/// Test-only resolver: takes the principal from a header. Production never trusts headers.
struct HeaderIdentity;

impl IdentityResolver for HeaderIdentity {
    fn resolve(&self, parts: &Parts, _peer: Option<SocketAddr>) -> Option<PrincipalId> {
        parts
            .headers
            .get("x-test-principal")
            .and_then(|value| value.to_str().ok())
            .and_then(PrincipalId::parse)
    }
}

async fn app(pool: &PgPool) -> Router {
    dev_seed::apply(pool).await.unwrap();
    router(AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        Arc::new(HeaderIdentity),
        ORIGIN,
        std::path::Path::new("target/site"),
    ))
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    actor: Option<Uuid>,
    origin: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Option<String>, String) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(actor) = actor {
        request = request.header("x-test-principal", actor.to_string());
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, cache, String::from_utf8(bytes.to_vec()).unwrap())
}

fn json(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("{e}: {body}"))
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn api_distinguishes_outcomes_and_keeps_drafts_private(pool: PgPool) {
    let app = app(&pool).await;
    let path = format!("/api/v1/drafts/{}", dev_seed::SAMPLE_WORK);
    let author = Some(dev_seed::SAMPLE_AUTHOR);

    let (status, _, body) = call(&app, "GET", &path, None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json(&body)["error"]["code"], "unauthenticated");

    let (status, _, _) = call(
        &app,
        "GET",
        &path,
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, cache, body) = call(&app, "GET", &path, author, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(json(&body)["version"], 1);

    let request = json!({
        "expected_version": 1,
        "title": "Sửa qua API",
        "document": { "schema_version": 1, "blocks": [{ "lines": ["Một dòng."] }] }
    });
    let (status, _, body) = call(
        &app,
        "PUT",
        &path,
        author,
        Some("http://evil.example"),
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _, _) = call(&app, "PUT", &path, author, None, Some(request.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "missing Origin is refused");

    let (status, _, body) = call(
        &app,
        "PUT",
        &path,
        author,
        Some(ORIGIN),
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body)["version"], 2);

    let (status, _, body) = call(&app, "PUT", &path, author, Some(ORIGIN), Some(request)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "version_conflict");
    assert_eq!(json(&body)["error"]["current_version"], 2);

    let invalid = json!({
        "expected_version": 2,
        "title": "x",
        "document": { "schema_version": 1, "blocks": [{ "lines": ["a\nb"] }] }
    });
    let (status, _, body) = call(&app, "PUT", &path, author, Some(ORIGIN), Some(invalid)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json(&body)["error"]["field"], "document");

    let (status, _, body) = call(&app, "POST", "/api/v1/drafts", author, Some(ORIGIN), None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(json(&body)["version"], 1);

    let (status, _, _) = call(&app, "GET", "/api/v1/drafts/not-a-uuid", author, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn server_html_has_public_content_and_never_private_drafts(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let vink = Vink::new(PgStore::new(pool.clone()));
    let private_title = "Tiêu đề mới chỉ ở nháp riêng";
    vink.save_draft(
        author(),
        save(
            sample_work(),
            1,
            private_title,
            doc(&[&[dev_seed::DRAFT_ONLY_LINE]]),
        ),
    )
    .await
    .unwrap();

    // An immutable revision is still private until the publication pointer selects it.
    let unpublished = Uuid::now_v7();
    let revision = RevisionContent::new(
        Title::parse("Snapshot chưa công khai").unwrap(),
        Document::parse(doc(&[&["Văn bản chỉ có trong snapshot chưa công khai."]])).unwrap(),
    )
    .unwrap();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(unpublished)
    .bind(work)
    .bind(revision.title().as_str())
    .bind(sqlx::types::Json(revision.document().to_data()))
    .bind(revision.hash().as_str())
    .bind(dev_seed::SAMPLE_AUTHOR)
    .execute(&pool)
    .await
    .unwrap();

    let (status, _, html) = call(&app, "GET", &format!("/tac-pham/{work}"), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("<title>[Mẫu] Mưa trên phố cũ — Vink</title>"),
        "{html}"
    );
    assert!(html.contains(r#"rel="canonical""#));
    assert!(html.contains(&format!("{ORIGIN}/tac-pham/{work}")));
    for line in dev_seed::PUBLISHED_STANZAS.iter().flat_map(|s| s.iter()) {
        assert!(html.contains(line), "missing published line {line}");
    }
    assert!(
        !html.contains(dev_seed::DRAFT_ONLY_LINE),
        "draft text leaked into reader HTML"
    );

    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/api/v1/works/{work}/publication"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains(dev_seed::DRAFT_ONLY_LINE));
    assert!(!body.contains(private_title));
    assert!(!body.contains("Snapshot chưa công khai"));
    assert_eq!(
        json(&body)["revision_id"],
        dev_seed::SAMPLE_REVISION.to_string()
    );
    assert!(!html.contains(private_title) && !html.contains("Snapshot chưa công khai"));
    assert!(html.contains(&format!(r#"data-revision="{}""#, dev_seed::SAMPLE_REVISION)));

    // The desk page is server-rendered as a shell; even the owner's draft is fetched later.
    let (status, _, html) = call(
        &app,
        "GET",
        &format!("/ban-viet/{work}"),
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains(dev_seed::DRAFT_ONLY_LINE));

    let (status, _, html) = call(&app, "GET", "/", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(dev_seed::SAMPLE_TITLE));
    assert!(html.contains("Khám phá") && html.contains("Bạn đã xem hết danh sách này."));
    assert!(!html.contains(private_title) && !html.contains("Snapshot chưa công khai"));

    let (status, _, body) = call(&app, "GET", "/api/v1/works", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["works"].as_array().unwrap().len(), 1);
    assert!(body.contains(dev_seed::SAMPLE_TITLE) && !body.contains(private_title));

    let private_work = vink.create_draft(author()).await.unwrap().work_id;
    for path in [
        format!("/tac-pham/{private_work}"),
        format!("/api/v1/works/{private_work}/publication"),
    ] {
        let (status, _, _) = call(&app, "GET", &path, None, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "private work: {path}");
    }

    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/tac-pham/{}", Uuid::now_v7()),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn no_publications_is_an_empty_collection_and_not_an_outage(pool: PgPool) {
    let app = app(&pool).await;
    sqlx::query("DELETE FROM publications")
        .execute(&pool)
        .await
        .unwrap();
    let (status, _, html) = call(&app, "GET", "/", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Chưa có tác phẩm nào được công khai."));
    assert!(!html.contains("Tạm thời chưa đọc được"));
    assert!(!html.contains(dev_seed::SAMPLE_TITLE) && !html.contains(dev_seed::DRAFT_ONLY_LINE));
    let (status, _, body) = call(&app, "GET", "/api/v1/works", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body), json!({ "works": [] }));
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/tac-pham/{}", dev_seed::SAMPLE_WORK),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
