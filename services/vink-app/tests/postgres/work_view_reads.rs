//! Work views read (#342) on real PostgreSQL: the `views` of the private author read-model
//! (`GET /api/v1/author/works`, CHOICE-05 A) over rows the #341 write path produced through
//! `Vink::record_work_view`. Publication changes are the real author, evaluator and admin case
//! paths. The decision clock is the wall clock here (not the virtual timeline of
//! `scenario.json`, which `work_view::shared_timeline_oracle_on_postgres` replays and reads
//! back), so a first publication written by the database clock falls before or after the
//! counting start on purpose: both "Tính từ" cases are deterministic.

use std::sync::Mutex;

use axum::http::{HeaderMap, HeaderName};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::layer::SubscriberExt;
use vink_app::application::author_works::AuthorWorkSummary;
use vink_app::application::drafts::SearchDrafts;
use vink_app::application::work_view::WorkViewResult;
use vink_app::domain::ids::RevisionId;
use vink_app::domain::work_view::ViewSkip;
use vink_app::domain::work_view::count::ViewCount;
use vink_app::evaluation_worker::EvaluationWorker;
use vink_app::persistence::moderation::add_admin_seat;

use super::work_view::{event_at, guest, new_member, view};
use super::*;

const AUTHOR_WORKS: &str = "/api/v1/author/works?limit=100";

async fn summary(vink: &Vink<PgStore>, work: WorkId) -> AuthorWorkSummary {
    vink.search_author_works(
        author(),
        SearchDrafts {
            limit: Some("100".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .works
    .into_iter()
    .find(|w| w.work_id == work)
    .unwrap()
}

/// The author's number for one work, as the application reports it.
pub(super) async fn author_views(vink: &Vink<PgStore>, work: WorkId) -> Option<ViewCount> {
    summary(vink, work).await.views
}

pub(super) async fn started_at(pool: &PgPool) -> Option<OffsetDateTime> {
    sqlx::query_scalar("SELECT started_at FROM work_view_counting")
        .fetch_optional(pool)
        .await
        .unwrap()
}

pub(super) async fn first_published_at(pool: &PgPool, work: WorkId) -> OffsetDateTime {
    sqlx::query_scalar(
        "SELECT first_published_at FROM work_first_publications
          WHERE work_id = $1 AND provenance = 'exact'",
    )
    .bind(work.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn generation(pool: &PgPool, work: WorkId) -> i64 {
    sqlx::query_scalar("SELECT generation FROM work_publications WHERE work_id = $1")
        .bind(work.as_uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

/// One counted view by a new guest at the wall clock.
async fn count_one(vink: &Vink<PgStore>, work: WorkId) {
    let now = OffsetDateTime::now_utc();
    let record = view(vink, guest(), work, &event_at(now), now)
        .await
        .unwrap();
    assert_eq!(record.result, WorkViewResult::Counted);
}

/// The evaluator approves the open job (the local scripted scenario, as #341's tests do).
async fn approve(pool: &PgPool, work: WorkId) {
    sqlx::query(
        "UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1 AND state = 'open'",
    )
    .bind(work.as_uuid())
    .execute(pool)
    .await
    .unwrap();
    assert!(
        EvaluationWorker::run_once(pool, "views-read-worker")
            .await
            .unwrap()
    );
}

/// Save, snapshot and publish a fresh draft through the real author and evaluator paths.
async fn publish_draft(vink: &Vink<PgStore>, pool: &PgPool, work: WorkId) -> RevisionId {
    vink.save_draft(
        author(),
        save(work, 1, "Gió mùa", doc(&[&["Gió về trên phố."]])),
    )
    .await
    .unwrap();
    let revision = vink
        .capture_snapshot(author(), work, "views-read-capture", 2, 1)
        .await
        .unwrap()
        .revision_id;
    vink.request_public(author(), work, "views-read-publish", revision, 1)
        .await
        .unwrap();
    approve(pool, work).await;
    revision
}

async fn get(
    app: &Router,
    path: &str,
    actor: Option<Uuid>,
    headers: &[(HeaderName, &str)],
) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::builder().method("GET").uri(path);
    if let Some(actor) = actor {
        request = request.header("x-test-principal", actor.to_string());
    }
    for (name, value) in headers {
        request = request.header(name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
}

fn cache(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap())
}

/// The `views` object of one work in an author list body.
fn card_views(body: &str, work: WorkId) -> Value {
    let id = work.to_string();
    json(body)["works"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work_id"] == id.as_str())
        .unwrap_or_else(|| panic!("work {id} not in {body}"))["views"]
        .clone()
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        _ => "other",
    }
}

/// A response produced from real rows has exactly the shape of the contract example of the same
/// state (and same presence of "Tính từ"): same keys, same JSON type per key.
fn assert_matches_contract(real: &Value) {
    let examples: Vec<Value> = [
        include_str!("../../contracts/http/v1/fixtures/author.works.views.json"),
        include_str!("../../contracts/http/v1/fixtures/author.works.views-not-started.json"),
    ]
    .iter()
    .flat_map(|raw| json(raw)["works"].as_array().unwrap().clone())
    .map(|w| w["views"].clone())
    .collect();
    let example = examples
        .iter()
        .find(|e| {
            e["state"] == real["state"]
                && e["counted_since"].is_null() == real["counted_since"].is_null()
        })
        .unwrap_or_else(|| panic!("no contract example for {real}"));
    let (real, example) = (real.as_object().unwrap(), example.as_object().unwrap());
    assert_eq!(
        real.keys().collect::<Vec<_>>(),
        example.keys().collect::<Vec<_>>()
    );
    for (key, value) in real {
        assert_eq!(
            kind(value),
            kind(&example[key]),
            "{key}: {real:?} vs {example:?}"
        );
    }
}

/// AC-02/AC-04: "0" only when measured; before the start, never public and "Tính từ" are told
/// apart, and the HTTP representation of each matches the contract examples.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn author_reads_the_states_the_write_path_produces(pool: PgPool) {
    let vink = seeded(&pool).await;
    let app = app(&pool).await;
    let w = sample_work();
    let n = vink.create_draft(author()).await.unwrap().work_id;

    // Nothing decided yet on this installation: no number at all (VIEW-09), and a draft never
    // public says so (L-01).
    assert_eq!(author_views(&vink, w).await, Some(ViewCount::NotStarted));
    assert_eq!(author_views(&vink, n).await, Some(ViewCount::NeverPublic));
    let (status, _, body) = get(&app, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for (work, state) in [(w, "not_started"), (n, "never_public")] {
        let real = card_views(&body, work);
        assert_eq!(
            real,
            json!({"state": state, "count": null, "counted_since": null})
        );
        assert_matches_contract(&real);
    }

    // The first decided event starts counting. W was public before it: "Tính từ" (L-12).
    count_one(&vink, w).await;
    let start = started_at(&pool).await.unwrap();
    assert!(first_published_at(&pool, w).await < start, "precondition");
    assert_eq!(
        author_views(&vink, w).await,
        Some(ViewCount::Counted {
            views: 1,
            since: Some(start)
        })
    );
    assert_eq!(author_views(&vink, n).await, Some(ViewCount::NeverPublic));

    // N becomes public after the start: a real zero without a label (L-02, L-13), then 1.
    publish_draft(&vink, &pool, n).await;
    assert!(first_published_at(&pool, n).await > start, "precondition");
    assert_eq!(
        author_views(&vink, n).await,
        Some(ViewCount::Counted {
            views: 0,
            since: None
        })
    );
    let (_, _, body) = get(&app, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    let zero = card_views(&body, n);
    assert_eq!(
        zero,
        json!({"state": "counted", "count": 0, "counted_since": null})
    );
    assert_matches_contract(&zero);
    count_one(&vink, n).await;

    let (status, headers, body) = get(&app, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache(&headers), Some("no-store"));
    let since = card_views(&body, w);
    assert_eq!(
        since,
        json!({"state": "counted", "count": 1, "counted_since": start.format(&Rfc3339).unwrap()})
    );
    assert_matches_contract(&since);
    let plain = card_views(&body, n);
    assert_eq!(
        plain,
        json!({"state": "counted", "count": 1, "counted_since": null})
    );
    assert_matches_contract(&plain);
}

/// VIEW-06/VIEW-08, L-08/L-09: a review withdrawal keeps the number readable and frozen; the
/// next publication of a new revision counts on from it.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn views_survive_review_withdrawal_and_count_on_after_republication(pool: PgPool) {
    let vink = seeded(&pool).await;
    let app = app(&pool).await;
    let w = sample_work();
    count_one(&vink, w).await;
    count_one(&vink, w).await;
    let start = started_at(&pool).await.unwrap();
    let counted = |views| {
        Some(ViewCount::Counted {
            views,
            since: Some(start),
        })
    };
    assert_eq!(author_views(&vink, w).await, counted(2));

    // An admin withdraws the public revision after review (O-17@r2), through the case routes.
    let admin = new_member(&pool, "Quản trị (kiểm thử)").await;
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        add_admin_seat(
            &mut conn,
            "operator-test",
            PrincipalId::from_uuid(admin),
            None
        )
        .await
        .unwrap()
    );
    drop(conn);
    let post = |path: String, body: Value| {
        let app = app.clone();
        async move { call(&app, "POST", &path, Some(admin), Some(ORIGIN), Some(body)).await }
    };
    let (status, _, body) = post(
        format!(
            "/api/v1/moderation/works/{w}/revisions/{}/cases",
            dev_seed::SAMPLE_REVISION
        ),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let case = json(&body)["case_id"].as_str().unwrap().to_owned();
    let cases = format!("/api/v1/moderation/works/cases/{case}");
    let (status, _, body) = post(format!("{cases}/claim"), json!({"expected_version": 1})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, body) = post(
        format!("{cases}/conclude"),
        json!({"expected_version": 2, "verdict": "rejected", "reason": "Vi phạm chính sách"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let withdrawn = summary(&vink, w).await;
    let publication = withdrawn.publication.unwrap();
    assert!(publication.review_withdrawn && publication.current_public.is_none());
    assert_eq!(withdrawn.views, counted(2), "kept while withdrawn");
    let now = OffsetDateTime::now_utc();
    let record = view(&vink, guest(), w, &event_at(now), now).await.unwrap();
    assert_eq!(record.result, WorkViewResult::Skipped(ViewSkip::NotPublic));
    assert_eq!(author_views(&vink, w).await, counted(2), "not increased");

    // A new revision is published: the same WorkId total continues (no reset).
    let r2 = vink
        .capture_snapshot(author(), w, "views-read-r2", 1, 1)
        .await
        .unwrap()
        .revision_id;
    vink.request_public(
        author(),
        w,
        "views-read-r2-pub",
        r2,
        generation(&pool, w).await,
    )
    .await
    .unwrap();
    approve(&pool, w).await;
    let republished = summary(&vink, w).await;
    assert_eq!(
        republished
            .publication
            .unwrap()
            .current_public
            .unwrap()
            .revision_id,
        r2
    );
    assert_eq!(republished.views, counted(2));
    count_one(&vink, w).await;
    assert_eq!(author_views(&vink, w).await, counted(3));
}

struct OutageIdentity;

impl IdentityResolver for OutageIdentity {
    fn resolve<'a>(&'a self, _: &'a Parts, _: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(std::future::ready(Resolution::Unavailable))
    }
}

/// AC-01/AC-03 (VIEW-07): only the owner's session reads the number, privately and without a
/// validator; no public JSON, HTML or validator carries or depends on it.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn only_the_owner_reads_views_and_no_public_surface_depends_on_them(pool: PgPool) {
    let vink = seeded(&pool).await;
    let app = app(&pool).await;
    let w = sample_work();
    let reader = format!("/api/v1/works/{w}/reader");
    let publication = format!("/api/v1/works/{w}/publication");
    let (status, headers, before) = get(&app, &reader, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    let (_, list_headers, _) = get(&app, "/api/v1/works", None, &[]).await;
    let list_etag = list_headers[header::ETAG].to_str().unwrap().to_string();
    count_one(&vink, w).await;

    // The owner reads it: private, never stored, no validator, even when one is offered.
    for offered in [None, Some("*"), Some(etag.as_str())] {
        let headers: Vec<_> = offered
            .map(|v| (header::IF_NONE_MATCH, v))
            .into_iter()
            .collect();
        let (status, headers, body) =
            get(&app, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &headers).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(cache(&headers), Some("no-store"));
        assert!(headers.get(header::ETAG).is_none());
        assert_eq!(card_views(&body, w)["count"], 1);
    }

    // A counted view changes no public byte: the full text is not reloaded to learn a number.
    let (status, _, _) = get(
        &app,
        &reader,
        None,
        &[(header::IF_NONE_MATCH, etag.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    let (status, _, list) = get(
        &app,
        "/api/v1/works",
        None,
        &[(header::IF_NONE_MATCH, list_etag.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_MODIFIED, "{list}");
    for path in [reader.as_str(), publication.as_str(), "/api/v1/works"] {
        for actor in [None, Some(dev_seed::SAMPLE_AUTHOR)] {
            let (status, _, body) = get(&app, path, actor, &[]).await;
            assert_eq!(status, StatusCode::OK, "{path}: {body}");
            assert!(
                !body.contains("\"views\"") && !body.contains("counted_since"),
                "{path} carries no view number: {body}"
            );
        }
    }
    let (_, _, again) = get(&app, &reader, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    assert_eq!(
        again, before,
        "the reader bytes are the same for the owner, after a view"
    );
    for actor in [None, Some(dev_seed::SAMPLE_AUTHOR)] {
        let (status, _, html) = get(&app, &format!("/tac-pham/{w}"), actor, &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!html.contains("counted_since") && !html.contains("Lượt xem"));
    }

    // Anyone else: another member sees only their own works; a guest and an identity outage
    // get no list at all, and nothing is stored.
    let (status, headers, body) =
        get(&app, AUTHOR_WORKS, Some(dev_seed::OTHER_PRINCIPAL), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache(&headers), Some("no-store"));
    assert!(!body.contains(&w.to_string()), "{body}");
    let (status, headers, body) = get(&app, AUTHOR_WORKS, None, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(cache(&headers), Some("no-store"));
    assert!(!body.contains("views"));
    let outage = router(AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        Arc::new(OutageIdentity),
        ORIGIN,
        std::path::Path::new("target/site"),
    ));
    let (status, headers, body) =
        get(&outage, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(cache(&headers), Some("no-store"));
    assert!(!body.contains("views"));

    // Hidden by its author: the old validator never revives the public body, and the author
    // still reads the kept number (VIEW-08).
    vink.set_private(author(), w, "views-read-hide", generation(&pool, w).await)
        .await
        .unwrap();
    let (status, headers, body) = get(
        &app,
        &reader,
        None,
        &[(header::IF_NONE_MATCH, etag.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body)["status"], "hidden");
    assert_eq!(cache(&headers), Some("no-store"));
    let (_, _, body) = get(&app, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    assert_eq!(card_views(&body, w)["count"], 1);
}

/// L-05: an unreadable statistics source leaves titles, state and the other counters intact,
/// and a failure of the other counters leaves the views intact; an inconsistent row loses only
/// its own number.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_views_failure_is_independent_of_the_rest_of_the_row(pool: PgPool) {
    let vink = seeded(&pool).await;
    let app = app(&pool).await;
    let w = sample_work();
    let n = vink.create_draft(author()).await.unwrap().work_id;
    count_one(&vink, w).await;
    let rename = |from: &'static str, to: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {from} RENAME TO {to}"
            )))
            .execute(&pool)
            .await
            .unwrap();
        }
    };

    rename("work_view_totals", "unavailable_work_view_totals").await;
    let row = summary(&vink, w).await;
    assert_eq!(row.views, None, "unavailable, not zero");
    assert!(row.draft_version.is_some());
    let public = row.publication.unwrap().current_public.unwrap();
    assert_eq!(public.revision_id.as_uuid(), dev_seed::SAMPLE_REVISION);
    assert!(public.tham.is_some() && public.comments.is_some());
    let (status, _, body) = get(&app, AUTHOR_WORKS, Some(dev_seed::SAMPLE_AUTHOR), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let unavailable = card_views(&body, w);
    assert_eq!(
        unavailable,
        json!({"state": "unavailable", "count": null, "counted_since": null})
    );
    assert_matches_contract(&unavailable);
    rename("unavailable_work_view_totals", "work_view_totals").await;

    rename("work_endorsements", "unavailable_work_endorsements").await;
    let row = summary(&vink, w).await;
    assert!(matches!(
        row.views,
        Some(ViewCount::Counted { views: 1, .. })
    ));
    assert!(
        row.publication
            .unwrap()
            .current_public
            .unwrap()
            .tham
            .is_none()
    );
    rename("unavailable_work_endorsements", "work_endorsements").await;

    // A total without a counting start is a shape no write produces: that work only.
    sqlx::query("DELETE FROM work_view_counting")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(author_views(&vink, w).await, None);
    assert_eq!(author_views(&vink, n).await, Some(ViewCount::NeverPublic));
}

/// Every statement sqlx sends while the future runs, summary and full text.
#[derive(Clone, Default)]
struct Statements(Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Statements {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().target() != "sqlx::query" {
            return;
        }
        struct Sql(String);
        impl tracing::field::Visit for Sql {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if matches!(field.name(), "summary" | "db.statement") {
                    self.0.push_str(value);
                }
            }
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if matches!(field.name(), "summary" | "db.statement") {
                    self.0.push_str(&format!("{value:?}"));
                }
            }
        }
        let mut sql = Sql(String::new());
        event.record(&mut sql);
        self.0.lock().unwrap().push(sql.0);
    }
}

/// AC-03: the views are one batch statement per page, whatever its size (no N+1), next to the
/// page and Thấm/Nhận xét statements; counted client-side from sqlx's own statement events.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn views_read_is_one_batch_statement_for_any_page_size(pool: PgPool) {
    let vink = seeded(&pool).await;
    sqlx::query("INSERT INTO works(id,owner_id,created_at) SELECT md5('views-batch-'||n)::uuid,$1,'2020-01-01' FROM generate_series(1,120) n")
        .bind(dev_seed::SAMPLE_AUTHOR).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO revisions(id,work_id,title,document,content_hash,created_by) SELECT md5('views-batch-revision-'||n)::uuid,md5('views-batch-'||n)::uuid,'Batch public '||n,'{\"format\":\"default\",\"source\":\"\"}',repeat('a',64),$1 FROM generate_series(1,120) n")
        .bind(dev_seed::SAMPLE_AUTHOR).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_publications(work_id,audience,lifecycle,generation,intent_revision_id,intent_generation,pointer_revision_id,pointer_intent_generation) SELECT md5('views-batch-'||n)::uuid,'public','active',2,md5('views-batch-revision-'||n)::uuid,2,md5('views-batch-revision-'||n)::uuid,2 FROM generate_series(1,120) n")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO publications(work_id,revision_id,generation,published_at) SELECT md5('views-batch-'||n)::uuid,md5('views-batch-revision-'||n)::uuid,2,'2020-01-01' FROM generate_series(1,120) n")
        .execute(&pool).await.unwrap();
    // Half of the works get a counted view through the write path.
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT md5('views-batch-'||n)::uuid FROM generate_series(1,120,2) n")
            .fetch_all(&pool)
            .await
            .unwrap();
    for id in &ids {
        count_one(&vink, WorkId::from_uuid(*id)).await;
    }
    let mut totals = Vec::new();
    for size in [1_usize, 24, 100] {
        let search = || SearchDrafts {
            limit: Some(size.to_string()),
            ..Default::default()
        };
        vink.search_author_works(author(), search()).await.unwrap();
        let statements = Statements::default();
        let subscriber = tracing_subscriber::registry().with(statements.clone());
        let page = vink
            .search_author_works(author(), search())
            .with_subscriber(subscriber)
            .await
            .unwrap();
        assert_eq!(page.works.len(), size);
        let counted = page
            .works
            .iter()
            .filter(|w| matches!(w.views, Some(ViewCount::Counted { views: 1, .. })))
            .count();
        assert!(page.works.iter().all(|w| w.views.is_some()));
        assert!(size == 1 || counted > 0, "the page reads real totals");
        let sent = statements.0.lock().unwrap().clone();
        let with = |marker: &str| sent.iter().filter(|s| s.contains(marker)).count();
        println!(
            "#342 page_size={size}: {} statements, views totals on page={counted}",
            sent.len()
        );
        assert_eq!(
            (
                with("WITH candidates AS"),
                with("WITH anchors AS"),
                with("work_view_totals")
            ),
            (1, 1, 1),
            "{sent:#?}"
        );
        totals.push(sent.len());
    }
    assert!(
        totals.windows(2).all(|pair| pair[0] == pair[1]),
        "statement count independent of page size: {totals:?}"
    );
}
