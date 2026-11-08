//! Tag catalog, draft-tag API and catalog governance (TAG-PR-03, #97) on real PostgreSQL. HTTP
//! requests go through the production router, use cases and `PgStore`; only the identity
//! resolver is a labelled test double (`HeaderIdentity`: a header names the principal, which
//! production never trusts; `DownIdentity`: every resolution is unavailable). Catalog changes run
//! through `Vink::change_catalog` with operator authority, and the operator command is run as the
//! real `vink-app` binary against the test database. Durable state is asserted with SQL,
//! independently of the response that reported it.

use std::process::{Command, Output};

use vink_app::application::catalog::{
    CatalogChanged, CatalogError, CatalogOperation, CatalogRejection, CatalogRequest,
    ChangeCatalog, CuratorAuthority,
};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::principal::SuspensionReason;
use vink_app::persistence::{grant_moderator, reinstate_principal, suspend_principal};

use super::*;

/// The tags of the API tests, by label. Identities are server-chosen at creation.
struct Catalog {
    tan_van: Uuid,
    tho: Uuid,
    ky_uc: Uuid,
    gia_dinh: Uuid,
    do_thi: Uuid,
    mua: Uuid,
    ma: Uuid,
    ma_sac: Uuid,
}

async fn catalog_version(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT version FROM tag_catalog")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// One change at the version the "operator" just read.
async fn curate(
    vink: &Vink<PgStore>,
    request: CatalogRequest,
) -> Result<CatalogChanged, CatalogError> {
    let expected_version = catalog_version(vink.store().pool()).await;
    let command = ChangeCatalog {
        expected_version,
        request,
    }
    .validate()
    .unwrap();
    vink.change_catalog(CuratorAuthority::operator(), command)
        .await
}

async fn create(vink: &Vink<PgStore>, facet: &str, label: &str) -> Uuid {
    let created = curate(
        vink,
        CatalogRequest::Create {
            facet: facet.into(),
            label: label.into(),
        },
    )
    .await
    .unwrap();
    created.subject.as_uuid()
}

/// Seed + catalog: two forms, six themes; an alias (Hồi ức → Ký ức), a description, a rename
/// (Gia đình → Gia đình Việt, so `gia-đình` is a former name) and a retirement with a successor
/// hint (Đô thị → Mùa).
async fn catalog(pool: &PgPool) -> (Vink<PgStore>, Catalog) {
    let vink = seeded(pool).await;
    let tags = Catalog {
        tan_van: create(&vink, "form", "Tản văn").await,
        tho: create(&vink, "form", "Thơ").await,
        ky_uc: create(&vink, "theme", "Ký ức").await,
        gia_dinh: create(&vink, "theme", "Gia đình").await,
        do_thi: create(&vink, "theme", "Đô thị").await,
        mua: create(&vink, "theme", "Mùa").await,
        ma: create(&vink, "theme", "ma").await,
        ma_sac: create(&vink, "theme", "má").await,
    };
    let tag = |id: Uuid| id.to_string();
    for request in [
        CatalogRequest::AddAlias {
            tag: tag(tags.ky_uc),
            label: "Hồi ức".into(),
        },
        CatalogRequest::Describe {
            tag: tag(tags.ky_uc),
            description: Some("Những điều đã qua còn ở lại trong người viết.".into()),
        },
        CatalogRequest::Rename {
            tag: tag(tags.gia_dinh),
            label: "Gia đình Việt".into(),
        },
        CatalogRequest::Retire {
            tag: tag(tags.do_thi),
            successor: Some(tag(tags.mua)),
        },
    ] {
        curate(&vink, request).await.unwrap();
    }
    (vink, tags)
}

/// Test-only resolver: Identity is down for every request.
struct DownIdentity;

impl IdentityResolver for DownIdentity {
    fn resolve<'a>(&'a self, _parts: &'a Parts, _peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(std::future::ready(Resolution::Unavailable))
    }
}

fn router_with(pool: &PgPool, identity: Arc<dyn IdentityResolver>) -> Router {
    router(AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        identity,
        ORIGIN,
        std::path::Path::new("target/site"),
    ))
}

fn labels(tags: &Value) -> Vec<String> {
    tags.as_array()
        .unwrap()
        .iter()
        .map(|tag| tag["label"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn catalog_reads_are_public_ranked_and_survive_an_identity_outage(pool: PgPool) {
    let (_, tags) = catalog(&pool).await;
    let app = router_with(&pool, Arc::new(DownIdentity));

    let (status, cache, body) = call(&app, "GET", "/api/v1/tags", None, None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    let listed = json(&body);
    // Active tags only, forms first, then by folded label; the retired Đô thị is not offered.
    assert_eq!(
        labels(&listed["tags"]),
        [
            "Tản văn",
            "Thơ",
            "Gia đình Việt",
            "Ký ức",
            "ma",
            "má",
            "Mùa"
        ]
    );
    assert_eq!(listed["truncated"], false);
    assert_eq!(
        listed["tags"][3]["description"],
        "Những điều đã qua còn ở lại trong người viết."
    );
    assert!(!body.contains("Đô thị") && !body.contains("operator"));
    let (_, _, body) = call(&app, "GET", "/api/v1/tags?facet=form", None, None, None).await;
    assert_eq!(labels(&json(&body)["tags"]), ["Tản văn", "Thơ"]);

    // `ma`: the exact name first, `má` only as a folded candidate; never one merged tag.
    let (status, _, body) = call(
        &app,
        "GET",
        "/api/v1/tags/suggestions?q=ma",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let found = json(&body);
    assert_eq!(found["query"], "ma");
    let suggestions = found["suggestions"].as_array().unwrap();
    assert_eq!(suggestions[0]["tag"]["tag_id"], tags.ma.to_string());
    assert_eq!(suggestions[0]["match"], "exact");
    assert_eq!(suggestions[1]["tag"]["tag_id"], tags.ma_sac.to_string());
    assert_eq!(suggestions[1]["match"], "folded");
    // An alias prefix suggests the canonical tag; a retired tag is never suggested.
    let (_, _, body) = call(
        &app,
        "GET",
        "/api/v1/tags/suggestions?q=h%E1%BB%93i&facet=theme&limit=3",
        None,
        None,
        None,
    )
    .await;
    let found = json(&body);
    assert_eq!(found["suggestions"][0]["tag"]["slug"], "ký-ức");
    assert_eq!(found["suggestions"][0]["via_alias"], "Hồi ức");
    let (_, _, body) = call(
        &app,
        "GET",
        "/api/v1/tags/suggestions?q=%C4%91%C3%B4",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(json(&body)["suggestions"], json!([]), "{body}");

    // Slugs: canonical, alias and former names redirect, a retired tag carries its hint.
    for (path, resolution, slug) in [
        ("k%C3%BD-%E1%BB%A9c", "canonical", "ký-ức"),
        // `KÝ-ỨC` decomposed (NFD) and uppercase is the same slug.
        ("KY%CC%81-%C6%AF%CC%81C", "canonical", "ký-ức"),
        ("h%E1%BB%93i-%E1%BB%A9c", "redirect", "ký-ức"),
        ("gia-%C4%91%C3%ACnh", "redirect", "gia-đình-việt"),
        ("%C4%91%C3%B4-th%E1%BB%8B", "retired", "đô-thị"),
    ] {
        let (status, cache, body) = call(
            &app,
            "GET",
            &format!("/api/v1/tags/slugs/{path}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
        let resolved = json(&body);
        assert_eq!(resolved["resolution"], resolution, "{path}");
        assert_eq!(resolved["tag"]["slug"], slug, "{path}");
    }
    let (_, _, body) = call(
        &app,
        "GET",
        "/api/v1/tags/slugs/%C4%91%C3%B4-th%E1%BB%8B",
        None,
        None,
        None,
    )
    .await;
    let retired = json(&body);
    assert_eq!(retired["tag"]["status"], "retired");
    assert_eq!(retired["tag"]["successor"]["tag_id"], tags.mua.to_string());
    assert_eq!(retired["tag"]["successor"]["label"], "Mùa");
    let (status, cache, _) = call(
        &app,
        "GET",
        "/api/v1/tags/slugs/bi%E1%BB%83n",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(cache.as_deref(), Some("no-store"));

    // The private routes need Identity and say so; the catalog above did not.
    let (status, cache, body) = call(
        &app,
        "GET",
        &format!("/api/v1/drafts/{}/tags", dev_seed::SAMPLE_WORK),
        Some(dev_seed::SAMPLE_AUTHOR),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(json(&body)["error"]["code"], "dependency_unavailable");
    assert_eq!(cache.as_deref(), Some("no-store"));
}

/// The sample work's durable draft metadata: version (absent row = None) and slot → tag.
async fn durable(pool: &PgPool) -> (Option<i64>, Vec<(i16, Uuid)>) {
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

fn tags_path() -> String {
    format!("/api/v1/drafts/{}/tags", dev_seed::SAMPLE_WORK)
}

fn desired(expected: i64, form: Option<Uuid>, themes: &[Uuid]) -> Value {
    json!({
        "expected_version": expected,
        "form": form.map(|id| id.to_string()),
        "themes": themes.iter().map(ToString::to_string).collect::<Vec<_>>(),
    })
}

async fn put_tags(app: &Router, actor: Option<Uuid>, body: Value) -> (StatusCode, Value) {
    let (status, cache, body) =
        call(app, "PUT", &tags_path(), actor, Some(ORIGIN), Some(body)).await;
    assert_eq!(cache.as_deref(), Some("no-store"), "{body}");
    (status, json(&body))
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_tags_round_trip_privately_with_presence_and_catalog_rules(pool: PgPool) {
    let (_, tags) = catalog(&pool).await;
    let app = router_with(&pool, Arc::new(HeaderIdentity));
    let author = Some(dev_seed::SAMPLE_AUTHOR);
    let other = Some(dev_seed::OTHER_PRINCIPAL);

    let (status, _, body) = call(&app, "GET", &tags_path(), None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json(&body)["error"]["code"], "unauthenticated");
    let (status, cache, _) = call(&app, "GET", &tags_path(), other, None, None).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another owner's draft is not disclosed"
    );
    assert_eq!(cache.as_deref(), Some("no-store"));
    let (status, cache, body) = call(&app, "GET", &tags_path(), author, None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(
        json(&body),
        json!({
            "work_id": dev_seed::SAMPLE_WORK.to_string(),
            "version": 1,
            "form": null,
            "themes": [],
        })
    );

    // Nothing that fails authority, presence or input validation writes anything.
    let valid = desired(1, Some(tags.tan_van), &[tags.ky_uc]);
    let (status, body) = put_tags(&app, other, valid.clone()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let mut omitted = valid.clone();
    omitted.as_object_mut().unwrap().remove("form");
    let mut forged = valid.clone();
    forged["owner_id"] = json!(dev_seed::OTHER_PRINCIPAL.to_string());
    let mut bad_form = valid.clone();
    bad_form["form"] = json!("Tản văn");
    let seven: Vec<Uuid> = std::iter::repeat_n(tags.ky_uc, 7).collect();
    for (body, field) in [
        (omitted, "body"),
        (forged, "body"),
        (bad_form, "form"),
        (desired(1, None, &seven), "themes"),
        (desired(0, None, &[]), "expected_version"),
    ] {
        let (status, response) = put_tags(&app, author, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
        assert_eq!(response["error"]["code"], "validation_failed");
        assert_eq!(response["error"]["field"], field, "{response}");
    }
    let (status, _, _) = call(&app, "PUT", &tags_path(), author, None, Some(valid.clone())).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a write needs the same Origin"
    );
    assert_eq!(
        durable(&pool).await,
        (None, vec![]),
        "refusals wrote nothing"
    );

    // Accepted: duplicates collapse to the first position; the response names identities.
    let (status, saved) = put_tags(
        &app,
        author,
        desired(
            1,
            Some(tags.tan_van),
            &[tags.ky_uc, tags.gia_dinh, tags.ky_uc],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["version"], 2);
    assert_eq!(saved["changed"], true);
    assert_eq!(saved["form"], tags.tan_van.to_string());
    assert_eq!(
        saved["themes"],
        json!([tags.ky_uc.to_string(), tags.gia_dinh.to_string()])
    );
    assert_eq!(
        durable(&pool).await,
        (
            Some(2),
            vec![(0, tags.tan_van), (1, tags.ky_uc), (2, tags.gia_dinh)]
        )
    );
    let (_, _, body) = call(&app, "GET", &tags_path(), author, None, None).await;
    let view = json(&body);
    assert_eq!(view["version"], 2);
    assert_eq!(view["form"]["label"], "Tản văn");
    assert_eq!(labels(&view["themes"]), ["Ký ức", "Gia đình Việt"]);

    // Catalog refusals: precise code, field and tag; nothing written.
    let unknown = Uuid::now_v7();
    for (body, status, code, field, tag) in [
        (
            desired(2, None, &[unknown]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "unknown_tag",
            "themes",
            unknown,
        ),
        (
            desired(2, Some(tags.ky_uc), &[]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "wrong_facet",
            "form",
            tags.ky_uc,
        ),
        (
            desired(2, None, &[tags.tho]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "wrong_facet",
            "themes",
            tags.tho,
        ),
        (
            desired(2, None, &[tags.do_thi]),
            StatusCode::CONFLICT,
            "tag_retired",
            "themes",
            tags.do_thi,
        ),
    ] {
        let (got, response) = put_tags(&app, author, body).await;
        assert_eq!(got, status, "{response}");
        assert_eq!(response["error"]["code"], code);
        assert_eq!(response["error"]["field"], field);
        assert_eq!(response["error"]["tag_id"], tag.to_string());
        if code == "tag_retired" {
            assert_eq!(response["error"]["successor_id"], tags.mua.to_string());
        }
    }
    let (status, response) = put_tags(
        &app,
        author,
        desired(2, None, &[tags.ky_uc, tags.gia_dinh, tags.mua, tags.ma]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response["error"]["code"], "too_many_themes");
    assert_eq!(durable(&pool).await.0, Some(2), "refusals wrote nothing");

    // A stale tab learns the stored tags instead of overwriting them.
    let (status, response) = put_tags(&app, author, desired(1, None, &[tags.mua])).await;
    assert_eq!(status, StatusCode::CONFLICT, "{response}");
    assert_eq!(response["error"]["code"], "version_conflict");
    assert_eq!(response["error"]["current_version"], 2);
    assert_eq!(
        response["error"]["tags"],
        json!({
            "form": tags.tan_van.to_string(),
            "themes": [tags.ky_uc.to_string(), tags.gia_dinh.to_string()],
        })
    );

    // Explicit `null`/`[]` clear; a same-state save is accepted and still advances.
    let (status, cleared) = put_tags(&app, author, desired(2, None, &[])).await;
    assert_eq!(
        (status, cleared["version"].clone()),
        (StatusCode::OK, json!(3))
    );
    assert_eq!(cleared["changed"], true);
    assert_eq!(durable(&pool).await, (Some(3), vec![]));
    let (status, same) = put_tags(&app, author, desired(3, None, &[])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (same["version"].clone(), same["changed"].clone()),
        (json!(4), json!(false))
    );

    // Text and tags have separate versions: saving text neither needs nor touches tags.
    let text = json!({
        "expected_version": 1,
        "title": "Sửa chữ, không sửa thẻ",
        "document": { "schema_version": 1, "blocks": [{ "lines": ["Một dòng mới."] }] }
    });
    let (status, _, body) = call(
        &app,
        "PUT",
        &format!("/api/v1/drafts/{}", dev_seed::SAMPLE_WORK),
        author,
        Some(ORIGIN),
        Some(text),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body)["version"], 2);
    assert_eq!(durable(&pool).await.0, Some(4));

    // Saving tags publishes nothing: the public read is the same bytes as before.
    let publication = format!("/api/v1/works/{}/publication", dev_seed::SAMPLE_WORK);
    let (status, _, public) = call(&app, "GET", &publication, None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json(&public).get("tags").is_none() && !public.contains("Tản văn"));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn lost_acknowledgements_and_parallel_tabs_never_overwrite_a_newer_choice(pool: PgPool) {
    let (_, tags) = catalog(&pool).await;
    let app = router_with(&pool, Arc::new(HeaderIdentity));
    let author = Some(dev_seed::SAMPLE_AUTHOR);

    // Tab A saves and loses the answer; tab B then saves a different choice.
    let tab_a = desired(1, None, &[tags.ky_uc]);
    let (status, _) = put_tags(&app, author, tab_a.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = put_tags(&app, author, desired(2, None, &[tags.mua])).await;
    assert_eq!(status, StatusCode::OK);
    // A's retry conflicts and shows B's tags: not what A wanted, so A must not resend.
    let (status, retry) = put_tags(&app, author, tab_a).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(retry["error"]["current_version"], 3);
    assert_eq!(
        retry["error"]["tags"]["themes"],
        json!([tags.mua.to_string()])
    );
    assert_eq!(durable(&pool).await, (Some(3), vec![(1, tags.mua)]));

    // Alone, a lost answer is reconciled by state: the retry conflicts with exactly the tags it
    // asked for, so the client adopts `current_version` without writing again.
    let intent = desired(3, Some(tags.tho), &[tags.mua, tags.ky_uc]);
    let (status, _) = put_tags(&app, author, intent.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (status, retry) = put_tags(&app, author, intent).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(retry["error"]["current_version"], 4);
    assert_eq!(
        retry["error"]["tags"],
        json!({
            "form": tags.tho.to_string(),
            "themes": [tags.mua.to_string(), tags.ky_uc.to_string()],
        })
    );
    assert_eq!(durable(&pool).await.0, Some(4), "the retry wrote nothing");

    // Six tabs at one version through the router: exactly one is accepted.
    let barrier = Arc::new(tokio::sync::Barrier::new(6));
    let choices = [
        tags.ky_uc,
        tags.mua,
        tags.ma,
        tags.ma_sac,
        tags.gia_dinh,
        tags.ky_uc,
    ];
    let tasks: Vec<_> = choices
        .into_iter()
        .map(|theme| {
            let (app, barrier) = (app.clone(), Arc::clone(&barrier));
            tokio::spawn(async move {
                barrier.wait().await;
                put_tags(&app, author, desired(4, None, &[theme])).await
            })
        })
        .collect();
    let mut accepted = 0;
    for task in tasks {
        let (status, body) = task.await.unwrap();
        match status {
            StatusCode::OK => accepted += 1,
            StatusCode::CONFLICT => assert_eq!(body["error"]["current_version"], 5),
            other => panic!("{other}: {body}"),
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(durable(&pool).await.0, Some(5));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn standing_is_rechecked_when_the_tag_save_commits(pool: PgPool) {
    let (_, tags) = catalog(&pool).await;
    // The test resolver ignores suspensions, as a resolution made just before one would; the
    // store's standing fence still refuses at commit.
    let app = router_with(&pool, Arc::new(HeaderIdentity));
    let author = Some(dev_seed::SAMPLE_AUTHOR);
    let principal = PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR);
    let reason = SuspensionReason::parse("Tạm khóa trong bộ test API thẻ").unwrap();
    suspend_principal(&pool, principal, &reason).await.unwrap();
    let (status, body) = put_tags(&app, author, desired(1, None, &[tags.ky_uc])).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "principal_suspended");
    assert_eq!(durable(&pool).await, (None, vec![]));
    reinstate_principal(&pool, principal).await.unwrap();
    let (status, body) = put_tags(&app, author, desired(1, None, &[tags.ky_uc])).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
struct AuditRow {
    catalog_version: i64,
    operation: String,
    tag_id: Uuid,
    slug: Option<String>,
    successor_id: Option<Uuid>,
    changed: bool,
    authority: String,
    database_role: String,
}

async fn audit(pool: &PgPool) -> Vec<AuditRow> {
    sqlx::query_as(
        "SELECT catalog_version, operation, tag_id, slug, successor_id, changed, authority,
                database_role
           FROM tag_catalog_changes ORDER BY catalog_version",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn sql_error(pool: &PgPool, statements: &[&str]) -> String {
    let mut tx = pool.begin().await.unwrap();
    for statement in statements {
        if let Err(error) = sqlx::raw_sql(sqlx::AssertSqlSafe(statement.to_string()))
            .execute(&mut *tx)
            .await
        {
            return error.to_string();
        }
    }
    match tx.commit().await {
        Ok(()) => panic!("committed although it should not: {statements:?}"),
        Err(error) => error.to_string(),
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn catalog_governance_runs_through_the_use_case_and_audits_every_step(pool: PgPool) {
    let (vink, tags) = catalog(&pool).await;
    let role: String = sqlx::query_scalar("SELECT session_user::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Eight creates, alias, describe, rename, retire: versions 2..=13, one audit row each.
    let rows = audit(&pool).await;
    assert_eq!(catalog_version(&pool).await, 13);
    assert_eq!(
        rows.iter().map(|r| r.catalog_version).collect::<Vec<_>>(),
        (2..=13).collect::<Vec<_>>()
    );
    assert!(
        rows.iter()
            .all(|r| r.authority == "operator" && r.database_role == role)
    );
    assert_eq!(
        rows[0],
        AuditRow {
            catalog_version: 2,
            operation: "create".into(),
            tag_id: tags.tan_van,
            slug: Some("tản-văn".into()),
            successor_id: None,
            changed: true,
            authority: "operator".into(),
            database_role: role.clone(),
        }
    );
    assert_eq!(
        (
            rows[8].operation.as_str(),
            rows[8].tag_id,
            rows[8].slug.as_deref()
        ),
        ("alias", tags.ky_uc, Some("hồi-ức"))
    );
    assert_eq!(
        (rows[9].operation.as_str(), rows[9].slug.as_deref()),
        ("describe", None)
    );
    assert_eq!(
        (rows[10].operation.as_str(), rows[10].slug.as_deref()),
        ("rename", Some("gia-đình-việt"))
    );
    assert_eq!(
        (rows[11].operation.as_str(), rows[11].successor_id),
        ("retire", Some(tags.mua))
    );

    // A same-state change is accepted, advances the version and is audited as unchanged.
    let same = curate(
        &vink,
        CatalogRequest::Describe {
            tag: tags.mua.to_string(),
            description: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        (same.operation, same.write.changed()),
        (CatalogOperation::Describe, false)
    );
    assert!(!audit(&pool).await.last().unwrap().changed);
    assert_eq!(catalog_version(&pool).await, 14);

    // Refusals and conflicts write nothing, audit included.
    let refused = curate(
        &vink,
        CatalogRequest::Create {
            facet: "theme".into(),
            label: "KÝ ỨC".into(),
        },
    )
    .await;
    assert_eq!(
        refused.unwrap_err(),
        CatalogError::Rejected(CatalogRejection::SlugTaken {
            holder: vink_app::domain::ids::TagId::from_uuid(tags.ky_uc)
        })
    );
    let retired = curate(
        &vink,
        CatalogRequest::Rename {
            tag: tags.do_thi.to_string(),
            label: "Phố thị".into(),
        },
    )
    .await;
    assert_eq!(
        retired.unwrap_err(),
        CatalogError::Rejected(CatalogRejection::Retired)
    );
    let stale = ChangeCatalog {
        expected_version: 13,
        request: CatalogRequest::AddAlias {
            tag: tags.mua.to_string(),
            label: "Bốn mùa".into(),
        },
    }
    .validate()
    .unwrap();
    assert_eq!(
        vink.change_catalog(CuratorAuthority::operator(), stale)
            .await,
        Err(CatalogError::Conflict {
            current: vink_app::domain::taxonomy::CatalogVersion::new(14).unwrap()
        })
    );
    assert_eq!(catalog_version(&pool).await, 14);
    assert_eq!(audit(&pool).await.len(), 13);

    // The operator's views read the same rows.
    let listing = vink.catalog_listing().await.unwrap();
    assert_eq!(listing.version.get(), 14);
    assert_eq!(listing.entries.len(), 8);
    let gia_dinh = listing
        .entries
        .iter()
        .find(|e| e.tag.id().as_uuid() == tags.gia_dinh)
        .unwrap();
    let names: Vec<(&str, vink_app::domain::taxonomy::SlugKind)> = gia_dinh
        .names
        .iter()
        .map(|n| (n.slug.as_str(), n.kind))
        .collect();
    assert!(names.contains(&("gia-đình", vink_app::domain::taxonomy::SlugKind::Former)));
    assert!(names.contains(&(
        "gia-đình-việt",
        vink_app::domain::taxonomy::SlugKind::Canonical
    )));
    let history = vink.catalog_history(2).await.unwrap();
    assert_eq!(
        history.iter().map(|r| r.version.get()).collect::<Vec<_>>(),
        [14, 13]
    );

    // The schema pairs every version step with its audit row, also for direct SQL.
    assert!(
        sql_error(&pool, &["UPDATE tag_catalog SET version = version + 1"])
            .await
            .contains("has no audit row")
    );
    let ahead = format!(
        "INSERT INTO tag_catalog_changes (catalog_version, operation, tag_id, slug, changed, authority)
         VALUES (99, 'create', '{}', 'xa', true, 'operator')",
        tags.tho
    );
    assert!(
        sql_error(&pool, &[&ahead])
            .await
            .contains("ahead of the catalog version")
    );
    for statement in [
        "UPDATE tag_catalog_changes SET changed = true",
        "DELETE FROM tag_catalog_changes WHERE false",
        "TRUNCATE tag_catalog_changes",
    ] {
        assert!(
            sql_error(&pool, &[statement])
                .await
                .contains("rows are immutable"),
            "{statement}"
        );
    }
    let forged = format!(
        "INSERT INTO tag_catalog_changes (catalog_version, operation, tag_id, slug, changed, authority)
         VALUES (15, 'create', '{}', 'xa', true, 'moderator')",
        tags.tho
    );
    assert!(
        sql_error(&pool, &["UPDATE tag_catalog SET version = 15", &forged])
            .await
            .contains("tag_catalog_changes_authority_check")
    );

    // A failure after the change's rows are written rolls the change and its audit back together.
    sqlx::raw_sql(
        "CREATE FUNCTION fail_audit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'injected audit failure'; END; $$;
         CREATE TRIGGER fail_audit BEFORE INSERT ON tag_catalog_changes
             FOR EACH ROW EXECUTE FUNCTION fail_audit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let failed = curate(
        &vink,
        CatalogRequest::Create {
            facet: "theme".into(),
            label: "Biển".into(),
        },
    )
    .await;
    assert_eq!(failed.unwrap_err(), CatalogError::Internal);
    let biển: i64 = sqlx::query_scalar("SELECT count(*) FROM tag_names WHERE slug = 'biển'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((biển, catalog_version(&pool).await), (0, 14));
    sqlx::raw_sql("DROP TRIGGER fail_audit ON tag_catalog_changes")
        .execute(&pool)
        .await
        .unwrap();
    create(&vink, "theme", "Biển").await;
    assert_eq!(catalog_version(&pool).await, 15);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn comment_moderators_get_no_catalog_authority(pool: PgPool) {
    let (_, tags) = catalog(&pool).await;
    let moderator = PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL);
    let note = GrantNote::parse("Kiểm duyệt bình luận trong test").unwrap();
    grant_moderator(&pool, moderator, &note).await.unwrap();
    let app = router_with(&pool, Arc::new(HeaderIdentity));
    let before = (catalog_version(&pool).await, audit(&pool).await.len());
    for (method, path) in [
        ("POST", "/api/v1/tags".to_string()),
        (
            "PUT",
            format!("/api/v1/tags/slugs/{}", "k%C3%BD-%E1%BB%A9c"),
        ),
        (
            "DELETE",
            format!("/api/v1/tags/slugs/{}", "k%C3%BD-%E1%BB%A9c"),
        ),
    ] {
        let (status, _, body) = call(
            &app,
            method,
            &path,
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(json!({ "label": "Ký ức 2", "tag_id": tags.ky_uc.to_string() })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path}: {body}"
        );
    }
    assert_eq!(
        (catalog_version(&pool).await, audit(&pool).await.len()),
        before
    );
}

const AUDIT_MIGRATION: i64 = 20_261_109_000_000;

/// A catalog written before the audit existed keeps its history; changes from then on are audited.
#[sqlx::test(migrations = false)]
async fn upgrading_an_unaudited_catalog_keeps_it_and_audits_from_then_on(pool: PgPool) {
    use sqlx::migrate::Migrator;
    use vink_app::persistence::{MIGRATOR, migrate};
    let previous = Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < AUDIT_MIGRATION)
            .cloned()
            .collect(),
    );
    previous.run(&pool).await.unwrap();
    // What the #96 adapter wrote: a tag, its canonical name and one version step, no audit.
    let tag = Uuid::now_v7();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "BEGIN;
         INSERT INTO tags (id, facet, label, slug, status) VALUES ('{tag}', 'theme', 'Ký ức', 'ký-ức', 'active');
         INSERT INTO tag_names (slug, tag_id, kind) VALUES ('ký-ức', '{tag}', 'canonical');
         UPDATE tag_catalog SET version = 2;
         COMMIT;"
    )))
    .execute(&pool)
    .await
    .unwrap();

    let report = migrate(&pool).await.unwrap();
    assert_eq!(report.applied, [AUDIT_MIGRATION]);
    assert_eq!(catalog_version(&pool).await, 2);
    assert!(
        audit(&pool).await.is_empty(),
        "no audit is invented for the past"
    );

    let vink = Vink::new(PgStore::new(pool.clone()));
    let page = vink.catalog_tags(None).await.unwrap();
    assert_eq!(page.tags.len(), 1);
    curate(
        &vink,
        CatalogRequest::AddAlias {
            tag: tag.to_string(),
            label: "Hồi ức".into(),
        },
    )
    .await
    .unwrap();
    let rows = audit(&pool).await;
    assert_eq!(
        rows.iter()
            .map(|r| (r.catalog_version, r.operation.as_str()))
            .collect::<Vec<_>>(),
        [(3, "alias")]
    );
}

/// The test database's URL for a child process: the harness URL with this test's database.
fn database_url(pool: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").expect("the postgres lane sets DATABASE_URL");
    let database = pool.connect_options().get_database().unwrap().to_owned();
    let (prefix, _) = base.rsplit_once('/').unwrap();
    format!("{prefix}/{database}")
}

fn vink_tag(database_url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vink-app"))
        .env_clear()
        .env("DATABASE_URL", database_url)
        .env("VINK_PROFILE", "local")
        .env("RUST_LOG", "warn")
        .arg("tag")
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_operator_command_changes_and_reports_the_catalog(pool: PgPool) {
    let url = database_url(&pool);
    let role: String = sqlx::query_scalar("SELECT session_user::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    let created = vink_tag(&url, &["create", "1", "theme", "Ký", "ức"]);
    assert_eq!(created.status.code(), Some(0), "{}", text(&created.stderr));
    let stdout = text(&created.stdout);
    assert!(stdout.starts_with("catalog version 2: create "), "{stdout}");
    assert!(
        stdout.contains("\ttheme\tactive\tký-ức\tKý ức\t-\t-"),
        "{stdout}"
    );
    let id: Uuid = sqlx::query_scalar("SELECT id FROM tags WHERE slug = 'ký-ức'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stdout.contains(&id.to_string()));

    let aliased = vink_tag(&url, &["alias", "2", &id.to_string(), "Hồi", "ức"]);
    assert_eq!(aliased.status.code(), Some(0), "{}", text(&aliased.stderr));
    let listed = vink_tag(&url, &["list"]);
    let stdout = text(&listed.stdout);
    assert!(stdout.starts_with("catalog version\t3\n"), "{stdout}");
    assert!(
        stdout.contains(&format!("name\t{id}\talias\thồi-ức\tHồi ức")),
        "{stdout}"
    );

    // Refused, conflicting and invalid changes exit 2 and write nothing.
    for (args, needle) in [
        (vec!["create", "3", "theme", "KÝ", "ỨC"], "refused"),
        (
            vec!["describe", "1", &id.to_string(), "Mô tả"],
            "catalog changed",
        ),
    ] {
        let output = vink_tag(&url, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains(needle) && stderr.contains("nothing was written"),
            "{stderr}"
        );
    }
    assert_eq!(catalog_version(&pool).await, 3);
    // Invalid input is refused before any connection: the database URL is not even reachable.
    let unreachable = "postgres://vink:unused@127.0.0.1:1/vink";
    let invalid = vink_tag(unreachable, &["create", "3", "theme", "海"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(
        text(&invalid.stderr).contains("label"),
        "{}",
        text(&invalid.stderr)
    );
    let down = vink_tag(unreachable, &["create", "3", "theme", "Biển"]);
    assert_eq!(
        down.status.code(),
        Some(1),
        "an outage is not a usage error"
    );

    let history = vink_tag(&url, &["history"]);
    let stdout = text(&history.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(lines[0].starts_with("3\t"), "newest first: {stdout}");
    assert!(
        lines[1].ends_with(&format!(
            "\tcreate\t{id}\tký-ức\t-\tchanged\toperator\t{role}"
        )),
        "{stdout}"
    );
    assert_eq!(audit(&pool).await.len(), 2);
}
