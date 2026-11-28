//! #587: the author profile's work items on real PostgreSQL, through the production router.
//! Every expectation is written from the fixture definition, never copied from the SQL under
//! test.
//!
//! FIXTURES, labelled: most public works are SQL fixtures (an immutable revision, an optional
//! metadata snapshot sealed by the schema's digest function, the pointer); the pointer insert
//! fires the real first-publication trigger. One work is forced to `legacy` provenance with the
//! history guard disabled, as the #291 backfill classifies a pre-cutover pointer. One work goes
//! through the production seam (draft → capture → request-public → evaluator job → set-private)
//! to show that a private draft edit and a set-private leave no text on the profile.
use time::macros::datetime;
use time::{Duration, OffsetDateTime};
use vink_app::domain::document::{Document, DocumentFormat};
use vink_app::domain::ids::TagId;
use vink_app::domain::moderation::AppliedEvaluation;
use vink_app::domain::taxonomy::{CatalogChange, TagFacet};
use vink_app::persistence::PROFILE_WORKS_SQL;
use vink_app::persistence::evaluation::{JobOutcome, claim_job, finish_job};

use super::discovery::{curate, label};
use super::first_publication::exact_time;
use super::handle::create;
use super::*;

const TAN_VAN: TagId = TagId::from_uuid(Uuid::from_u128(0xa587_0000_0000_4000_8000_0000_0000_0001));
const KY_UC: TagId = TagId::from_uuid(Uuid::from_u128(0xa587_0000_0000_4000_8000_0000_0000_0011));
const BASE: OffsetDateTime = datetime!(2026-10-01 00:00 UTC);

/// FIXTURE publish: the revision of `content`, a snapshot of `tags` when given (`None`: published
/// before #96, without one) and the pointer binding both, in one transaction.
async fn publish(
    pool: &PgPool,
    work: Uuid,
    owner: Uuid,
    content: TitledDocument,
    at: OffsetDateTime,
    tags: Option<(Option<TagId>, &[TagId])>,
) {
    let content = RevisionContent::new(content).unwrap();
    let revision = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
        .bind(work)
        .bind(owner)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(revision)
    .bind(work)
    .bind(content.title().as_str())
    .bind(sqlx::types::Json(content.content().document().to_data()))
    .bind(content.hash().as_str())
    .bind(owner)
    .execute(&mut *tx)
    .await
    .unwrap();
    let snapshot = match tags {
        None => None,
        Some((form, themes)) => {
            let id = Uuid::now_v7();
            let themes: Vec<Uuid> = themes.iter().map(TagId::as_uuid).collect();
            sqlx::query(
                "INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
                 VALUES ($1, $2, 1, work_metadata_digest($3, $4::uuid[]))",
            )
            .bind(id)
            .bind(work)
            .bind(form.map(|f| f.as_uuid()))
            .bind(&themes)
            .execute(&mut *tx)
            .await
            .unwrap();
            let slots = form
                .map(|f| (0_i16, f.as_uuid(), "form"))
                .into_iter()
                .chain((1..).zip(&themes).map(|(slot, t)| (slot, *t, "theme")));
            for (slot, tag, facet) in slots {
                sqlx::query(
                    "INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(id)
                .bind(slot)
                .bind(tag)
                .bind(facet)
                .execute(&mut *tx)
                .await
                .unwrap();
            }
            Some(id)
        }
    };
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
         VALUES ($1, $2, 1, $3, $4)",
    )
    .bind(work)
    .bind(revision)
    .bind(at)
    .bind(snapshot)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

fn poem(title: &str, text: &str) -> TitledDocument {
    TitledDocument::author(
        Document::new(DocumentFormat::Default, text).unwrap(),
        Some(title),
    )
    .unwrap()
}

/// Work items of a `200` current profile page, and its cursor.
async fn page(app: &Router, path: &str) -> (Vec<Value>, Option<String>, String) {
    let (status, _, body) = call(app, "GET", path, None, None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let value = json(&body);
    assert_eq!(value["state"], "current", "{body}");
    assert_eq!(value["publications"]["state"], "page", "{body}");
    let works = value["publications"]["works"].as_array().unwrap().clone();
    let next = value["publications"]["next_cursor"]
        .as_str()
        .map(str::to_owned);
    (works, next, body)
}

fn ids(works: &[Value]) -> Vec<String> {
    works
        .iter()
        .map(|w| w["work_id"].as_str().unwrap().to_owned())
        .collect()
}

fn item(works: &[Value], id: Uuid) -> &Value {
    works
        .iter()
        .find(|w| w["work_id"] == id.to_string())
        .unwrap_or_else(|| panic!("{id} is on the page"))
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn profile_items_carry_current_public_metadata_and_never_private_text(pool: PgPool) {
    let app = app(&pool).await;
    let vink = Vink::new(PgStore::new(pool.clone()));
    let actor = create(&pool, "profile-works", "Lê Thu").await;
    let owner = actor.as_uuid();
    let handle = PgStore::new(pool.clone())
        .own_handle(actor)
        .await
        .unwrap()
        .handle;
    let path = format!("/api/v1/authors/{handle}");
    for (id, facet, text) in [
        (TAN_VAN, TagFacet::Form, "Tản văn"),
        (KY_UC, TagFacet::Theme, "Ký ức"),
    ] {
        curate(
            &pool,
            CatalogChange::Create {
                id,
                facet,
                label: label(text),
                description: None,
            },
        )
        .await;
    }

    // Newest pointer first: `rich`, `legacy`, `withdrawn`, then twelve plain works.
    let rich = Uuid::now_v7();
    publish(
        &pool,
        rich,
        owner,
        poem(
            "Một mùa chưa gọi tên",
            "Có những ngày trời không mưa\nMà lòng người vẫn ướt.",
        ),
        BASE,
        Some((Some(TAN_VAN), &[KY_UC])),
    )
    .await;
    let legacy = Uuid::now_v7();
    publish(
        &pool,
        legacy,
        owner,
        TitledDocument::author(
            Document::new(DocumentFormat::Markdown, "# Những lá thư không gửi\n\n   ").unwrap(),
            None,
        )
        .unwrap(),
        BASE - Duration::minutes(1),
        None,
    )
    .await;
    // What the #291 backfill records for a pointer that existed before exact history.
    sqlx::raw_sql("ALTER TABLE work_first_publications DISABLE TRIGGER guard_first_publication")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE work_first_publications
            SET provenance = 'legacy', source = 'existing_pointer', first_published_at = NULL,
                legacy_published_at = $2
          WHERE work_id = $1",
    )
    .bind(legacy)
    .bind(BASE - Duration::minutes(1))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("ALTER TABLE work_first_publications ENABLE TRIGGER guard_first_publication")
        .execute(&pool)
        .await
        .unwrap();
    let withdrawn = Uuid::now_v7();
    publish(
        &pool,
        withdrawn,
        owner,
        poem("Mới rút", "Câu này đã rút khỏi công chúng."),
        BASE - Duration::minutes(2),
        Some((Some(TAN_VAN), &[])),
    )
    .await;
    let mut plain = Vec::new();
    for n in 0..12 {
        let work = Uuid::now_v7();
        publish(
            &pool,
            work,
            owner,
            poem(&format!("Bài {n}"), "Một dòng công khai."),
            BASE - Duration::minutes(10 + n),
            None,
        )
        .await;
        plain.push(work.to_string());
    }
    // Another author's work never reaches this profile.
    let stranger = create(&pool, "profile-works-other", "Người khác").await;
    publish(
        &pool,
        Uuid::now_v7(),
        stranger.as_uuid(),
        poem("Bài của người khác", "Không thuộc hồ sơ này."),
        BASE + Duration::hours(1),
        Some((Some(TAN_VAN), &[])),
    )
    .await;

    // The production seam: published through the evaluator job, so its pointer time is now.
    let seam = vink.create_draft(Some(actor)).await.unwrap().work_id;
    vink.save_draft(
        Some(actor),
        save(
            seam,
            1,
            "Gió ngoài hiên",
            doc(&[&["Chiều hôm ấy, gió trở về qua tán cây."]]),
        ),
    )
    .await
    .unwrap();
    let revision = vink
        .capture_snapshot(Some(actor), seam, "capture-587", 2, 1)
        .await
        .unwrap()
        .revision_id;
    vink.request_public(Some(actor), seam, "public-587", revision, 1)
        .await
        .unwrap();
    let job = claim_job(&pool, "profile-587", 60, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        finish_job(&pool, "profile-587", &job, Some(evaluator_approval()), 0)
            .await
            .unwrap(),
        JobOutcome::Applied(AppliedEvaluation::Published)
    );
    // A private edit after publication changes the draft, never the public item.
    vink.save_draft(
        Some(actor),
        save(
            seam,
            2,
            "Bản nháp riêng",
            doc(&[&["Nội dung nháp chưa công khai."]]),
        ),
    )
    .await
    .unwrap();

    let (first, cursor, body) = page(&app, &path).await;
    let mut expected = vec![
        seam.to_string(),
        rich.to_string(),
        legacy.to_string(),
        withdrawn.to_string(),
    ];
    expected.extend(plain.iter().cloned());
    assert_eq!(ids(&first), expected[..12], "bounded at one page of 12");
    assert!(!body.contains("Nội dung nháp chưa công khai") && !body.contains("Bản nháp riêng"));
    assert!(!body.contains("Bài của người khác"));

    // Rich: excerpt with its line break, form then theme, the exact first-public instant.
    let rich_item = item(&first, rich);
    assert_eq!(rich_item["title"], "Một mùa chưa gọi tên");
    assert_eq!(
        rich_item["excerpt"],
        "Có những ngày trời không mưa\nMà lòng người vẫn ướt."
    );
    let tags: Vec<(&str, &str, &str)> = rich_item["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["facet"].as_str().unwrap(),
                t["label"].as_str().unwrap(),
                t["tag_id"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        tags,
        [
            ("form", "Tản văn", TAN_VAN.to_string().as_str()),
            ("theme", "Ký ức", KY_UC.to_string().as_str()),
        ]
    );
    let first_public = OffsetDateTime::parse(
        rich_item["first_published_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    assert_eq!(first_public, exact_time(&pool, rich).await);
    assert_ne!(first_public, BASE, "never the fixture pointer time");
    for key in ["author", "author_display_name", "published_at", "views"] {
        assert!(rich_item.get(key).is_none(), "{key} is not a profile field");
    }
    // No byline, counter or view number: an item holds exactly the public fields it was given.
    let mut keys: Vec<&str> = rich_item
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["excerpt", "first_published_at", "tags", "title", "work_id"]
    );

    // Legacy: no snapshot, no exact fact, no text after the title: facts omitted, tags empty.
    let legacy_item = item(&first, legacy);
    assert_eq!(legacy_item["title"], "Những lá thư không gửi");
    assert!(legacy_item.get("excerpt").is_none());
    assert!(legacy_item.get("first_published_at").is_none());
    assert_eq!(legacy_item["tags"], json!([]));

    // Seam: the published revision's text, never the later draft.
    let seam_item = item(&first, seam.as_uuid());
    assert_eq!(seam_item["title"], "Gió ngoài hiên");
    assert_eq!(
        seam_item["excerpt"],
        "Chiều hôm ấy, gió trở về qua tán cây."
    );
    assert!(seam_item["first_published_at"].is_string());
    assert_eq!(seam_item["tags"], json!([]));

    let cursor = cursor.expect("a further public work exists");
    let (second, end, _) = page(&app, &format!("{path}?cursor={cursor}")).await;
    assert_eq!(ids(&second), expected[12..]);
    assert_eq!(end, None);

    // Withdrawal (pointer removed) and set-private through the author's own command.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(withdrawn)
        .execute(&pool)
        .await
        .unwrap();
    vink.set_private(Some(actor), seam, "private-587", 2)
        .await
        .unwrap();
    let (first, cursor, body) = page(&app, &path).await;
    let mut kept = vec![rich.to_string(), legacy.to_string()];
    kept.extend(plain.iter().cloned());
    assert_eq!(ids(&first), kept[..12]);
    for gone in [
        "Gió ngoài hiên",
        "Chiều hôm ấy",
        "Mới rút",
        "đã rút khỏi công chúng",
    ] {
        assert!(!body.contains(gone), "{gone} left the profile");
    }
    let (second, end, body) = page(&app, &format!("{path}?cursor={}", cursor.unwrap())).await;
    assert_eq!(ids(&second), kept[12..]);
    assert_eq!(end, None);
    assert!(!body.contains(&seam.to_string()) && !body.contains(&withdrawn.to_string()));

    // The production page statement: one bounded read, no per-row subquery, no draft table.
    let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "EXPLAIN (ANALYZE, COSTS OFF) {PROFILE_WORKS_SQL}",
    )))
    .bind(owner)
    .bind(None::<OffsetDateTime>)
    .bind(None::<Uuid>)
    .bind(13_i64)
    .fetch_all(&pool)
    .await
    .unwrap();
    let plan = plan.join("\n");
    eprintln!("Author profile works PG plan:\n{plan}");
    assert!(
        plan.starts_with("Limit ") && plan.contains("rows=13"),
        "{plan}"
    );
    assert!(
        !plan.contains("SubPlan") && !plan.contains("working_drafts"),
        "{plan}"
    );
}
