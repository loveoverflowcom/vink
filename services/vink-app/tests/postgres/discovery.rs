//! Public discovery (TAG-PR-04, #98) on real PostgreSQL, through the production router. Every
//! expected list is written by hand from the fixture's definition (or sorted in Rust from it),
//! never copied from the SQL under test.
//!
//! FIXTURES, labelled: no publish/withdraw/moderation workflow exists (PUB-01..03), so public
//! works are made by SQL (an immutable revision, a metadata snapshot sealed by the schema's own
//! digest function, the pointer) and withdrawn by deleting the pointer. One work goes through the
//! production seam instead (author saves draft tags → `capture_metadata_snapshot` → pointer), to
//! show that private edits after a capture never reach a search. Catalog changes use the real
//! operator path (`change_catalog` with operator authority). None of this proves an author can
//! publish: it proves what reads do with whatever publications exist.

use std::collections::HashSet;

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};
use vink_app::application::catalog::CuratorAuthority;
use vink_app::application::taxonomy::SaveWorkTags;
use vink_app::domain::discovery::{DiscoveryCursor, SearchScope};
use vink_app::domain::ids::TagId;
use vink_app::domain::taxonomy::{
    CatalogChange, CatalogCommand, CatalogOutcome, CatalogVersion, MetadataVersion, SearchKey,
    TagFacet, TagLabel,
};
use vink_app::persistence::{SnapshotCapture, capture_metadata_snapshot, change_catalog};

use super::*;

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

const fn work(n: u128) -> Uuid {
    Uuid::from_u128(0x5555_0000_0000_4000_8000_0000_0000_0000 + n)
}

/// A third author whose display name is stored decomposed (NFD), as a client might send it.
const POET: Uuid = uuid::uuid!("66666666-6666-4666-8666-666666666666");

const BASE: OffsetDateTime = datetime!(2026-10-01 00:00 UTC);

async fn catalog_version(pool: &PgPool) -> CatalogVersion {
    let raw: i64 = sqlx::query_scalar("SELECT version FROM tag_catalog")
        .fetch_one(pool)
        .await
        .unwrap();
    CatalogVersion::new(raw).unwrap()
}

/// One accepted catalog change through the operator path.
async fn curate(pool: &PgPool, change: CatalogChange) {
    let expected = catalog_version(pool).await;
    let outcome = change_catalog(
        pool,
        CuratorAuthority::operator(),
        &CatalogCommand { expected, change },
    )
    .await
    .unwrap();
    assert!(
        matches!(outcome, CatalogOutcome::Accepted(_)),
        "{outcome:?}"
    );
}

fn label(text: &str) -> TagLabel {
    TagLabel::parse(text).unwrap()
}

/// The seed without its sample publication (withdrawn, FIXTURE), its two principals renamed to
/// short names that match no fixture title (the seed's "Tác giả mẫu …" would match `ma`, `gia`),
/// a third principal and the catalog: two forms, four themes, the alias `Hồi ức` → `Ký ức`.
async fn setup(pool: &PgPool) -> Router {
    let app = app(pool).await;
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(pool)
        .await
        .unwrap();
    for (id, name) in [
        (dev_seed::SAMPLE_AUTHOR, "Lê Thu"),
        (dev_seed::OTHER_PRINCIPAL, "Trần Bình"),
    ] {
        sqlx::query("UPDATE principals SET display_name = $2 WHERE id = $1")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
    }
    let nfd: String = unicode_normalization::UnicodeNormalization::nfd("Đặng Thùy").collect();
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
        .bind(POET)
        .bind(nfd)
        .execute(pool)
        .await
        .unwrap();
    for (id, facet, text) in [
        (TAN_VAN, TagFacet::Form, "Tản văn"),
        (THO, TagFacet::Form, "Thơ"),
        (KY_UC, TagFacet::Theme, "Ký ức"),
        (GIA_DINH, TagFacet::Theme, "Gia đình"),
        (DO_THI, TagFacet::Theme, "Đô thị"),
        (MUA, TagFacet::Theme, "Mùa"),
    ] {
        curate(
            pool,
            CatalogChange::Create {
                id,
                facet,
                label: label(text),
                description: None,
            },
        )
        .await;
    }
    curate(
        pool,
        CatalogChange::AddAlias {
            target: KY_UC,
            label: label("Hồi ức"),
        },
    )
    .await;
    app
}

#[derive(Clone, Copy)]
enum Meta<'a> {
    /// Published without a metadata snapshot (every publication before #96).
    Untagged,
    Tags(Option<TagId>, &'a [TagId]),
}

/// FIXTURE publish: an immutable revision, a snapshot whose digest the schema computes and seals,
/// and the pointer binding both, in one transaction.
async fn publish(
    pool: &PgPool,
    work: Uuid,
    owner: Uuid,
    title: &str,
    at: OffsetDateTime,
    meta: Meta<'_>,
) {
    let content = RevisionContent::new(
        Title::parse(title).unwrap(),
        Document::parse(doc(&[&["Một dòng công khai."]])).unwrap(),
    )
    .unwrap();
    let revision = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
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
    .bind(sqlx::types::Json(content.document().to_data()))
    .bind(content.hash().as_str())
    .bind(owner)
    .execute(&mut *tx)
    .await
    .unwrap();
    let snapshot = match meta {
        Meta::Untagged => None,
        Meta::Tags(form, themes) => {
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
            let mut slots: Vec<(i16, Uuid, &str)> = Vec::new();
            if let Some(form) = form {
                slots.push((0, form.as_uuid(), "form"));
            }
            for (slot, theme) in (1..).zip(&themes) {
                slots.push((slot, *theme, "theme"));
            }
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
         VALUES ($1, $2, 1, $3, $4)
         ON CONFLICT (work_id) DO UPDATE
            SET revision_id = EXCLUDED.revision_id, generation = publications.generation + 1,
                published_at = EXCLUDED.published_at,
                metadata_snapshot_id = EXCLUDED.metadata_snapshot_id",
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

/// FIXTURE: what a withdrawal will do (remove the pointer; history stays).
async fn withdraw(pool: &PgPool, work: Uuid) {
    let removed = sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(work)
        .execute(pool)
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(removed, 1);
}

async fn rename(pool: &PgPool, id: TagId, text: &str) {
    curate(
        pool,
        CatalogChange::Rename {
            id,
            label: label(text),
        },
    )
    .await;
}

#[derive(Debug)]
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&self.body)))
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn etag(&self) -> String {
        self.headers[header::ETAG].to_str().unwrap().to_owned()
    }

    /// Work ids of a `200`, in order.
    fn ids(&self) -> Vec<Uuid> {
        assert_eq!(self.status, StatusCode::OK, "{}", self.text());
        self.json()["works"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| Uuid::parse_str(w["work_id"].as_str().unwrap()).unwrap())
            .collect()
    }

    fn next(&self) -> Option<String> {
        self.json()["next_cursor"].as_str().map(str::to_owned)
    }
}

/// Independent of the implementation: quoted hex of the first 128 bits of SHA-256.
fn oracle(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

async fn get(app: &Router, path: &str, if_none_match: Option<&str>) -> Reply {
    let mut request = Request::get(path);
    if let Some(tag) = if_none_match {
        request = request.header(header::IF_NONE_MATCH, tag);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Reply {
        status,
        headers,
        body,
    }
}

/// `/api/v1/works?…` with each value percent-encoded once.
fn search(q: Option<&str>, tags: &[&str], limit: Option<usize>, cursor: Option<&str>) -> String {
    let mut query = form_urlencoded::Serializer::new(String::new());
    if let Some(q) = q {
        query.append_pair("q", q);
    }
    for tag in tags {
        query.append_pair("tag", tag);
    }
    if let Some(limit) = limit {
        query.append_pair("limit", &limit.to_string());
    }
    if let Some(cursor) = cursor {
        query.append_pair("cursor", cursor);
    }
    format!("/api/v1/works?{}", query.finish())
}

async fn found(app: &Router, q: Option<&str>, tags: &[&str]) -> Vec<Uuid> {
    get(app, &search(q, tags, None, None), None).await.ids()
}

/// Follows `next_cursor` from the first page; every page is bounded by `limit`.
async fn walk(app: &Router, q: Option<&str>, tags: &[&str], limit: usize) -> Vec<Uuid> {
    let mut all = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..100 {
        let reply = get(app, &search(q, tags, Some(limit), cursor.as_deref()), None).await;
        let ids = reply.ids();
        assert!(ids.len() <= limit, "a page is bounded");
        all.extend(ids);
        match reply.next() {
            None => return all,
            Some(next) => cursor = Some(next),
        }
    }
    panic!("pagination did not end");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn search_finds_works_beyond_the_first_twelve_in_bounded_ordered_pages(pool: PgPool) {
    let app = setup(&pool).await;
    // 30 public works, one hour apart (n = 1 newest); 10, 11 and 12 share one timestamp.
    let mut expected: Vec<(OffsetDateTime, Uuid)> = Vec::new();
    for n in 1..=30u128 {
        let hours = if (10..=12).contains(&n) {
            20
        } else {
            30 - n as i64
        };
        let at = BASE + Duration::hours(hours);
        let title = if n == 27 {
            "Hồi ức tuổi thơ".to_string()
        } else {
            format!("Bài số {n}")
        };
        let owner = if n % 2 == 0 {
            dev_seed::OTHER_PRINCIPAL
        } else {
            dev_seed::SAMPLE_AUTHOR
        };
        publish(&pool, work(n), owner, &title, at, Meta::Untagged).await;
        expected.push((at, work(n)));
    }
    // The order, decided here: newest first, then work id ascending.
    expected.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let order: Vec<Uuid> = expected.iter().map(|(_, id)| *id).collect();
    let ties: Vec<Uuid> = order
        .iter()
        .copied()
        .filter(|id| [work(10), work(11), work(12)].contains(id))
        .collect();
    assert_eq!(ties, [work(10), work(11), work(12)]);

    // The home page is the first 12 and says that more exist.
    let home = get(&app, "/api/v1/works", None).await;
    assert_eq!(home.ids(), order[..12]);
    assert!(home.next().is_some());
    assert!(
        !home.ids().contains(&work(27)),
        "the match is outside the first 12"
    );

    // The whole library is searched: unaccented, any case.
    for q in ["hoi uc", "HỒI ỨC", "tuoi tho", "Hồi"] {
        let reply = get(&app, &search(Some(q), &[], None, None), None).await;
        assert_eq!(reply.ids(), [work(27)], "{q}");
        assert_eq!(reply.next(), None);
        assert_eq!(reply.json()["filters"]["q"], q, "echoed as typed");
    }

    // Walking every page: no duplicate, no gap, deterministic across the tie (a 5-row page
    // boundary falls between works 10 and 11).
    for limit in [1, 5, 7, 24] {
        assert_eq!(walk(&app, None, &[], limit).await, order, "limit {limit}");
    }
    let page = get(&app, &search(None, &[], Some(24), None), None).await;
    assert_eq!(page.ids().len(), 24);
    // The same request twice gives the same bytes and validator.
    let again = get(&app, &search(None, &[], Some(24), None), None).await;
    assert_eq!((&page.body, page.etag()), (&again.body, again.etag()));
}

/// The truth-table fixture: overlapping tags, an untagged and an empty snapshot, a retired tag,
/// an unpublished work, a withdrawn work and a work whose draft tags changed after publication.
struct Library {
    /// An unpublished work whose private title and draft tags would match.
    private_draft: Uuid,
    /// Published with [Gia đình] through the capture seam; its draft later says [Ký ức].
    gia_pha: Uuid,
}

async fn library(pool: &PgPool) -> Library {
    let at = |hours: i64| BASE + Duration::hours(hours);
    let author = dev_seed::SAMPLE_AUTHOR;
    let other = dev_seed::OTHER_PRINCIPAL;
    publish(
        pool,
        work(1),
        author,
        "Mưa tháng Bảy",
        at(10),
        Meta::Tags(Some(TAN_VAN), &[KY_UC, GIA_DINH]),
    )
    .await;
    publish(
        pool,
        work(2),
        POET,
        "Má tôi",
        at(9),
        Meta::Tags(Some(THO), &[KY_UC]),
    )
    .await;
    publish(
        pool,
        work(3),
        other,
        "Mà thôi",
        at(8),
        Meta::Tags(Some(THO), &[GIA_DINH, MUA]),
    )
    .await;
    publish(pool, work(4), author, "Đường về", at(7), Meta::Untagged).await;
    publish(
        pool,
        work(5),
        other,
        "Giá_trị 50%",
        at(6),
        Meta::Tags(None, &[]),
    )
    .await;
    publish(
        pool,
        work(6),
        author,
        "Phố thị",
        at(5),
        Meta::Tags(Some(TAN_VAN), &[DO_THI, MUA]),
    )
    .await;
    curate(
        pool,
        CatalogChange::Retire {
            id: DO_THI,
            successor: Some(MUA),
        },
    )
    .await;

    // Unpublished: a private draft titled and tagged like a match.
    let vink = Vink::new(PgStore::new(pool.clone()));
    let draft = vink
        .create_draft(Some(PrincipalId::from_uuid(author)))
        .await
        .unwrap();
    vink.save_draft(
        Some(PrincipalId::from_uuid(author)),
        save(
            draft.work_id,
            1,
            "Ký ức riêng tư",
            doc(&[&["Chưa ai đọc."]]),
        ),
    )
    .await
    .unwrap();
    vink.save_work_tags(
        Some(PrincipalId::from_uuid(author)),
        SaveWorkTags {
            work_id: draft.work_id,
            expected_version: 1,
            form: Some(TAN_VAN.to_string()),
            themes: vec![KY_UC.to_string()],
        },
    )
    .await
    .unwrap();

    // Withdrawn after publication.
    publish(
        pool,
        work(8),
        author,
        "Mưa đã rút",
        at(4),
        Meta::Tags(None, &[KY_UC]),
    )
    .await;
    withdraw(pool, work(8)).await;

    // Through the production seam: draft tags [Gia đình] → capture → pointer; then the author
    // changes the draft to [Ký ức] privately.
    let other_actor = Some(PrincipalId::from_uuid(other));
    let gia_pha = vink.create_draft(other_actor).await.unwrap();
    vink.save_work_tags(
        other_actor,
        SaveWorkTags {
            work_id: gia_pha.work_id,
            expected_version: 1,
            form: None,
            themes: vec![GIA_DINH.to_string()],
        },
    )
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let mut tx = sqlx::Connection::begin(&mut *conn).await.unwrap();
    let snapshot =
        match capture_metadata_snapshot(&mut tx, gia_pha.work_id, MetadataVersion::new(2).unwrap())
            .await
            .unwrap()
        {
            SnapshotCapture::Captured { id, .. } => id,
            other => panic!("{other:?}"),
        };
    let content = RevisionContent::new(
        Title::parse("Gia phả").unwrap(),
        Document::parse(doc(&[&["Một dòng công khai."]])).unwrap(),
    )
    .unwrap();
    let revision = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(revision)
    .bind(gia_pha.work_id.as_uuid())
    .bind(content.title().as_str())
    .bind(sqlx::types::Json(content.document().to_data()))
    .bind(content.hash().as_str())
    .bind(other)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
         VALUES ($1, $2, 1, $3, $4)",
    )
    .bind(gia_pha.work_id.as_uuid())
    .bind(revision)
    .bind(at(3))
    .bind(snapshot)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    vink.save_work_tags(
        other_actor,
        SaveWorkTags {
            work_id: gia_pha.work_id,
            expected_version: 2,
            form: None,
            themes: vec![KY_UC.to_string()],
        },
    )
    .await
    .unwrap();
    Library {
        private_draft: draft.work_id.as_uuid(),
        gia_pha: gia_pha.work_id.as_uuid(),
    }
}

/// Labels of the tags shown on the card of `id` in a list reply.
fn card_tags(reply: &Reply, id: Uuid) -> Vec<String> {
    reply.json()["works"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work_id"] == id.to_string())
        .unwrap_or_else(|| panic!("{id} not listed"))["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["label"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn vietnamese_text_aliases_and_tag_filters_follow_an_explicit_truth_table(pool: PgPool) {
    let app = setup(&pool).await;
    let lib = library(&pool).await;
    let (w1, w2, w3, w4, w5, w6, gia_pha) = (
        work(1),
        work(2),
        work(3),
        work(4),
        work(5),
        work(6),
        lib.gia_pha,
    );
    let nfd =
        |text: &str| unicode_normalization::UnicodeNormalization::nfd(text).collect::<String>();

    // Text: folded word prefixes of the public title or the author's current name.
    let duong_nfd = nfd("Đường");
    let text_cases: Vec<(&str, Vec<Uuid>)> = vec![
        ("ma", vec![w2, w3]),
        ("Má", vec![w2, w3]),
        ("MƯA", vec![w1]),
        ("mua", vec![w1]),
        ("đường", vec![w4]),
        ("DUONG VE", vec![w4]),
        (&duong_nfd, vec![w4]),
        ("dang thuy", vec![w2]),
        ("ĐẶNG", vec![w2]),
        ("oi", vec![]),
        ("tri 5", vec![w5]),
        ("gia", vec![w5, gia_pha]),
        ("riêng tư", vec![]),
        ("rút", vec![]),
        ("le thu", vec![w1, w4, w6]),
        ("TRẦN", vec![w3, w5, gia_pha]),
        ("binh", vec![w3, w5, gia_pha]),
    ];
    for (q, expected) in text_cases {
        assert_eq!(found(&app, Some(q), &[]).await, expected, "q={q}");
    }

    // Tags: every selected tag must be on the bound public snapshot (AND).
    let tag_cases: Vec<(Vec<&str>, Vec<Uuid>)> = vec![
        (vec!["ký-ức"], vec![w1, w2]),
        (vec!["hồi-ức"], vec![w1, w2]),
        (vec!["ký-ức", "gia-đình"], vec![w1]),
        (vec!["gia-đình", "ký-ức"], vec![w1]),
        (vec!["thơ", "gia-đình"], vec![w3]),
        (vec!["thơ", "tản-văn"], vec![]),
        (vec!["mùa"], vec![w3, w6]),
        (vec!["gia-đình"], vec![w1, w3, gia_pha]),
        (vec!["tản-văn"], vec![w1, w6]),
        (vec!["đô-thị"], vec![]),
        (vec!["không-có"], vec![]),
        (vec!["ky-uc"], vec![]),
    ];
    for (tags, expected) in tag_cases {
        assert_eq!(found(&app, None, &tags).await, expected, "tag={tags:?}");
    }
    assert_eq!(found(&app, Some("ma"), &["thơ"]).await, [w2, w3]);
    assert_eq!(found(&app, Some("ma"), &["ký-ức"]).await, [w2]);
    assert_eq!(found(&app, Some("mua"), &["mùa"]).await, Vec::<Uuid>::new());

    // The answer says what each filter named.
    let reply = get(
        &app,
        &search(None, &[&nfd("KÝ-ỨC"), "ký-ức", "hồi-ức"], None, None),
        None,
    )
    .await;
    assert_eq!(reply.ids(), [w1, w2]);
    let filters = reply.json()["filters"]["tags"].clone();
    assert_eq!(
        filters.as_array().unwrap().len(),
        2,
        "spellings of one slug collapse"
    );
    assert_eq!(filters[0]["requested"], "ký-ức");
    assert_eq!(filters[0]["resolution"], "canonical");
    assert_eq!(filters[1]["requested"], "hồi-ức");
    assert_eq!(filters[1]["resolution"], "redirect");
    assert_eq!(
        filters[1]["tag"]["slug"], "ký-ức",
        "the client canonicalizes to this slug"
    );
    let retired = get(&app, &search(None, &["đô-thị"], None, None), None).await;
    assert_eq!(
        retired.json()["filters"]["tags"][0]["resolution"],
        "retired"
    );
    assert_eq!(
        retired.json()["filters"]["tags"][0]["tag"]["successor"]["slug"],
        "mùa"
    );
    assert_eq!(retired.next(), None);
    let unknown = get(&app, &search(None, &["không-có"], None, None), None).await;
    assert_eq!(
        unknown.json()["filters"]["tags"][0]["resolution"],
        "not_found"
    );
    assert_eq!(unknown.json()["filters"]["tags"][0]["tag"], Value::Null);

    // Cards show active public tags in snapshot order; a retired tag stays hidden.
    let home = get(&app, "/api/v1/works", None).await;
    assert_eq!(home.ids(), [w1, w2, w3, w4, w5, w6, gia_pha]);
    assert_eq!(card_tags(&home, w1), ["Tản văn", "Ký ức", "Gia đình"]);
    assert_eq!(card_tags(&home, w6), ["Tản văn", "Mùa"]);
    assert!(card_tags(&home, w4).is_empty() && card_tags(&home, w5).is_empty());
    assert_eq!(
        card_tags(&home, gia_pha),
        ["Gia đình"],
        "the public snapshot, not the draft"
    );
    let reader = get(&app, &format!("/api/v1/works/{w6}/publication"), None).await;
    let labels: Vec<String> = reader.json()["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["label"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(labels, ["Tản văn", "Mùa"]);

    // A rename shows everywhere and the old slug keeps filtering the same identity.
    rename(&pool, KY_UC, "Hoài niệm").await;
    let renamed = get(&app, &search(None, &["ký-ức"], None, None), None).await;
    assert_eq!(renamed.ids(), [w1, w2]);
    assert_eq!(
        renamed.json()["filters"]["tags"][0]["resolution"],
        "redirect"
    );
    assert_eq!(
        renamed.json()["filters"]["tags"][0]["tag"]["slug"],
        "hoài-niệm"
    );
    assert_eq!(card_tags(&renamed, w2), ["Thơ", "Hoài niệm"]);
    assert_eq!(found(&app, None, &["hoài-niệm"]).await, [w1, w2]);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn private_unpublished_and_withdrawn_works_never_reach_results_or_validators(pool: PgPool) {
    let app = setup(&pool).await;
    let lib = library(&pool).await;
    // The allowed public set, written from the fixture: works 1..6 and Gia phả.
    let public: HashSet<Uuid> = [
        work(1),
        work(2),
        work(3),
        work(4),
        work(5),
        work(6),
        lib.gia_pha,
    ]
    .into_iter()
    .collect();
    let hidden = [lib.private_draft, work(8)];
    let mut bodies = Vec::new();
    for path in [
        "/api/v1/works".to_string(),
        search(None, &[], Some(24), None),
        search(Some("ký ức riêng"), &[], None, None),
        search(Some("mưa"), &[], None, None),
        search(None, &["ký-ức"], None, None),
        search(None, &["tản-văn"], None, None),
        search(None, &["gia-đình"], None, None),
        search(Some("gia"), &["ký-ức"], None, None),
    ] {
        let reply = get(&app, &path, None).await;
        for id in reply.ids() {
            assert!(public.contains(&id), "{path}: {id} is not public");
        }
        bodies.push(reply.text());
    }
    let html = get(&app, "/", None).await;
    assert_eq!(html.status, StatusCode::OK);
    bodies.push(html.text());
    for body in &bodies {
        for id in hidden {
            assert!(!body.contains(&id.to_string()), "{body}");
        }
        for private in ["riêng tư", "Chưa ai đọc", "Mưa đã rút", "working_drafts"] {
            assert!(!body.contains(private), "{private}: {body}");
        }
    }
    for id in hidden {
        let reply = get(&app, &format!("/api/v1/works/{id}/publication"), None).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }

    // Private draft tag edits never change a public representation or its validator.
    let path = search(None, &["gia-đình"], None, None);
    let before = get(&app, &path, None).await;
    let home = get(&app, "/api/v1/works", None).await;
    let vink = Vink::new(PgStore::new(pool.clone()));
    vink.save_work_tags(
        Some(PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL)),
        SaveWorkTags {
            work_id: WorkId::from_uuid(lib.gia_pha),
            expected_version: 3,
            form: Some(THO.to_string()),
            themes: vec![MUA.to_string()],
        },
    )
    .await
    .unwrap();
    let after = get(&app, &path, Some(&before.etag())).await;
    assert_eq!(after.status, StatusCode::NOT_MODIFIED);
    assert_eq!(
        get(&app, "/api/v1/works", Some(&home.etag())).await.status,
        StatusCode::NOT_MODIFIED
    );
    assert_eq!(
        found(&app, None, &["thơ", "mùa"]).await,
        [work(3)],
        "draft-only tags never match"
    );

    // Withdrawal removes the work from results and changes the validator.
    withdraw(&pool, work(1)).await;
    let gone = get(&app, &path, Some(&before.etag())).await;
    assert_eq!(gone.status, StatusCode::OK);
    assert_eq!(gone.ids(), [work(3), lib.gia_pha]);
    assert_eq!(gone.etag(), oracle(&gone.body));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn search_validators_revalidate_and_stay_coherent_with_their_body(pool: PgPool) {
    let app = setup(&pool).await;
    library(&pool).await;
    let path = search(Some("ma"), &["thơ"], None, None);
    let first = get(&app, &path, None).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.etag(), oracle(&first.body));
    assert_eq!(first.headers[header::CACHE_CONTROL], "private, no-cache");
    let same = get(&app, &path, Some(&first.etag())).await;
    assert_eq!(same.status, StatusCode::NOT_MODIFIED);
    assert!(same.body.is_empty());
    // Another search is another representation, even with the same works.
    let alias = get(
        &app,
        &search(Some("MA"), &["thơ"], None, None),
        Some(&first.etag()),
    )
    .await;
    assert_eq!(alias.status, StatusCode::OK, "the echoed query differs");

    // A public label change is a new representation.
    rename(&pool, THO, "Thơ ca").await;
    let renamed = get(&app, &path, Some(&first.etag())).await;
    assert_eq!(renamed.status, StatusCode::OK);
    assert_ne!(renamed.etag(), first.etag());
    assert_eq!(card_tags(&renamed, work(2)), ["Thơ ca", "Ký ức"]);

    // Under concurrent renames, every body is one database state and its validator is its own.
    let app = std::sync::Arc::new(app);
    let mut readers = Vec::new();
    for _ in 0..24 {
        let app = app.clone();
        readers.push(tokio::spawn(async move {
            get(&app, &search(None, &["gia-đình"], None, None), None).await
        }));
    }
    for n in 0..8 {
        rename(&pool, GIA_DINH, if n % 2 == 0 { "Nhà" } else { "Gia đình" }).await;
    }
    for reader in readers {
        let reply = reader.await.unwrap();
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.etag(), oracle(&reply.body));
        let json = reply.json();
        let filter_label = json["filters"]["tags"][0]["tag"]["label"]
            .as_str()
            .unwrap()
            .to_owned();
        for card in json["works"].as_array().unwrap() {
            for tag in card["tags"].as_array().unwrap() {
                if tag["tag_id"] == GIA_DINH.to_string() {
                    assert_eq!(tag["label"].as_str().unwrap(), filter_label, "one snapshot");
                }
            }
        }
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn cursors_are_bound_to_their_search_and_dataset_changes_are_honest(pool: PgPool) {
    let app = setup(&pool).await;
    library(&pool).await;

    // A cursor continues only its own normalized search.
    let page = get(&app, &search(Some("ma"), &["ký-ức"], Some(1), None), None).await;
    assert_eq!(page.ids(), [work(2)]);
    assert_eq!(page.next(), None);
    let page = get(&app, &search(None, &["hồi-ức"], Some(1), None), None).await;
    assert_eq!(page.ids(), [work(1)]);
    let cursor = page.next().unwrap();
    let canonical = get(
        &app,
        &search(None, &["ký-ức"], Some(1), Some(&cursor)),
        None,
    )
    .await;
    assert_eq!(
        canonical.ids(),
        [work(2)],
        "alias and canonical are one search"
    );
    assert_eq!(canonical.next(), None);
    let mut tampered = cursor.clone().into_bytes();
    tampered[20] = if tampered[20] == b'0' { b'1' } else { b'0' };
    let tampered = String::from_utf8(tampered).unwrap();
    for path in [
        search(Some("mua"), &["ký-ức"], Some(1), Some(&cursor)),
        search(None, &["ký-ức", "thơ"], Some(1), Some(&cursor)),
        search(None, &[], Some(1), Some(&cursor)),
        search(None, &["ký-ức"], Some(1), Some(&cursor.to_uppercase())),
        search(None, &["ký-ức"], Some(1), Some(&cursor[..64])),
        search(None, &["ký-ức"], Some(1), Some(&"a".repeat(4000))),
    ] {
        let reply = get(&app, &path, None).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{path}");
        assert_eq!(reply.json()["error"]["field"], "cursor");
        assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    }
    // A forged position inside the same search only moves within public results.
    let forged = get(
        &app,
        &search(None, &["ký-ức"], Some(1), Some(&tampered)),
        None,
    )
    .await;
    for id in if forged.status == StatusCode::OK {
        forged.ids()
    } else {
        vec![]
    } {
        assert!([work(1), work(2)].contains(&id));
    }

    // Between pages: newer works stay before the cursor, withdrawn ones disappear, a republished
    // one moves to the top. Each page is one snapshot; the list is not one snapshot.
    let first = get(&app, &search(None, &[], Some(2), None), None).await;
    assert_eq!(first.ids(), [work(1), work(2)]);
    let cursor = first.next().unwrap();
    publish(
        &pool,
        work(40),
        dev_seed::SAMPLE_AUTHOR,
        "Bài mới nhất",
        BASE + Duration::hours(100),
        Meta::Untagged,
    )
    .await;
    withdraw(&pool, work(3)).await;
    publish(
        &pool,
        work(4),
        dev_seed::SAMPLE_AUTHOR,
        "Đường về",
        BASE + Duration::hours(101),
        Meta::Untagged,
    )
    .await;
    let second = get(&app, &search(None, &[], Some(2), Some(&cursor)), None).await;
    assert_eq!(second.ids(), [work(5), work(6)]);
    let top = get(&app, "/api/v1/works", None).await;
    assert_eq!(top.ids()[..3], [work(4), work(40), work(1)]);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn cursor_scope_uses_the_same_query_key_as_postgres_matching(pool: PgPool) {
    let app = setup(&pool).await;
    for (n, title) in [(1, "ΟΣ mới"), (2, "ΟΣ cũ"), (3, "οσ khác")] {
        publish(
            &pool,
            work(n),
            dev_seed::SAMPLE_AUTHOR,
            title,
            BASE - Duration::minutes(n as i64),
            Meta::Untagged,
        )
        .await;
    }
    // SQL lowercases word-final Σ to ς. Rust SearchKey::of lowercases it to σ.
    let first = get(&app, &search(Some("ΟΣ"), &[], Some(1), None), None).await;
    assert_eq!(first.ids(), [work(1)]);
    let cursor = first.next().unwrap();
    let wrong = get(&app, &search(Some("οσ"), &[], Some(1), Some(&cursor)), None).await;
    assert_eq!(wrong.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(wrong.json()["error"]["field"], "cursor");

    // A v1 cursor for ΟΣ used the Rust key οσ. It must not continue the SQL search for οσ.
    let position = DiscoveryCursor::decode(&cursor).unwrap().after();
    let old_scope = DiscoveryCursor::new(position, SearchScope::of(Some("οσ"), &[])).encode();
    let legacy = format!("01{}", &old_scope[2..]);
    let legacy = get(&app, &search(Some("οσ"), &[], Some(1), Some(&legacy)), None).await;
    assert_eq!(legacy.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(legacy.json()["error"]["field"], "cursor");

    let same = get(&app, &search(Some("ος"), &[], Some(1), Some(&cursor)), None).await;
    assert_eq!(same.status, StatusCode::OK);
    assert_eq!(same.ids(), [work(2)]);
    assert_eq!(same.next(), None);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn sql_search_key_matches_rust_for_vietnamese_and_latin(pool: PgPool) {
    let letters = "aàáảãạăằắẳẵặâầấẩẫậbcdđeèéẻẽẹêềếểễệghiìíỉĩịklmnoòóỏõọôồốổỗộơờớởỡợpqrstuùúủũụưừứửữựvxyỳýỷỹỵfjwz";
    let mut inputs: Vec<String> = Vec::new();
    for c in letters.chars() {
        let lower = c.to_string();
        let upper: String = c.to_uppercase().collect();
        for text in [lower, upper] {
            let nfd: String =
                unicode_normalization::UnicodeNormalization::nfd(text.as_str()).collect();
            inputs.push(text);
            inputs.push(nfd);
        }
    }
    for text in [
        "Mưa trên phố cũ",
        "  ĐƯỜNG   về—nhà!! ",
        "Giá_trị 50% (bản 2)",
        "Hồi ức: tuổi thơ / Ký-ức",
        "Lê Thu’s “Mùa” 2026",
        "",
        "!!!",
        "Phở 🍜 Hà Nội",
        "Ngõ 12A\tphố\nHuế",
    ] {
        inputs.push(text.to_string());
        inputs.push(unicode_normalization::UnicodeNormalization::nfd(text).collect());
    }
    let sql: Vec<String> = sqlx::query_scalar(
        "SELECT vink_search_key(t) FROM unnest($1::text[]) WITH ORDINALITY AS x (t, n) ORDER BY n",
    )
    .bind(&inputs)
    .fetch_all(&pool)
    .await
    .unwrap();
    for (input, sql) in inputs.iter().zip(&sql) {
        assert_eq!(sql, SearchKey::of(input).as_str(), "{input:?}");
    }
    // Recorded divergences outside Vietnamese/Latin (both sides of a match use SQL).
    let outside: Vec<(String, String)> =
        sqlx::query_as("SELECT t, vink_search_key(t) FROM unnest(ARRAY['x²', 'नमस्ते', 'Ⅻ']) AS t")
            .fetch_all(&pool)
            .await
            .unwrap();
    for (input, sql) in outside {
        assert_ne!(
            sql,
            SearchKey::of(&input).as_str(),
            "{input:?} now agrees; update the docs"
        );
    }
}

/// MEASUREMENT, not a gate (`#[ignore]`): run with
/// `cargo test -p vink-app --test postgres discovery_measurement -- --ignored --nocapture`.
/// Loads `VINK_DISCOVERY_ROWS` (default 20 000) public works by SQL (FIXTURE: 200 authors, every
/// work with a form and one to three themes, sealed snapshots), then prints `EXPLAIN (ANALYZE,
/// BUFFERS)` of the adapter's own statement for five shapes and the median of 15 warm HTTP
/// requests through the router. Results belong in docs/development/discovery-api.md with the
/// machine they ran on; no target is asserted.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
#[ignore = "measurement; run explicitly"]
async fn discovery_measurement(pool: PgPool) {
    let rows: i64 = std::env::var("VINK_DISCOVERY_ROWS")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(20_000);
    let app = setup(&pool).await;
    let started = std::time::Instant::now();
    let mut tx = pool.begin().await.unwrap();
    for statement in [
        "INSERT INTO principals (id, display_name)
         SELECT md5('author' || a)::uuid,
                (ARRAY['Lê','Trần','Nguyễn','Phạm','Đặng'])[1 + a % 5] || ' ' ||
                (ARRAY['Thu','Bình','An','Hà','Thùy','Minh'])[1 + a % 6] || ' ' || a
           FROM generate_series(1, 200) AS a",
        "INSERT INTO works (id, owner_id)
         SELECT md5('work' || n)::uuid, md5('author' || (1 + n % 200))::uuid
           FROM generate_series(1, $1::bigint) AS n",
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         SELECT md5('rev' || n)::uuid, md5('work' || n)::uuid,
                (ARRAY['Mưa','Nắng','Phố','Đường về','Hồi ức','Mùa thu','Gió','Biển'])[1 + n % 8]
                    || ' số ' || n,
                '{\"schema_version\":1,\"blocks\":[{\"lines\":[\"Một dòng.\"]}]}'::jsonb,
                md5('h' || n) || md5('i' || n), md5('author' || (1 + n % 200))::uuid
           FROM generate_series(1, $1::bigint) AS n",
        "CREATE TEMP TABLE picked ON COMMIT DROP AS
         SELECT n, md5('snap' || n)::uuid AS snapshot,
                (ARRAY['a0000000-0000-4000-8000-000000000001',
                       'a0000000-0000-4000-8000-000000000002'])[1 + n % 2]::uuid AS form,
                (ARRAY(SELECT DISTINCT t FROM unnest(ARRAY[
                    (ARRAY['a0000000-0000-4000-8000-00000000000b',
                           'a0000000-0000-4000-8000-00000000000c',
                           'a0000000-0000-4000-8000-00000000000e'])[1 + n % 3],
                    (ARRAY['a0000000-0000-4000-8000-00000000000c',
                           'a0000000-0000-4000-8000-00000000000e',
                           'a0000000-0000-4000-8000-00000000000b'])[1 + (n / 3) % 3]
                ]::uuid[]) AS t ORDER BY t)) AS themes
           FROM generate_series(1, $1::bigint) AS n",
        "INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
         SELECT snapshot, md5('work' || n)::uuid, 1, work_metadata_digest(form, themes) FROM picked",
        "INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
         SELECT snapshot, 0, form, 'form' FROM picked
         UNION ALL
         SELECT snapshot, t.slot::smallint, t.id, 'theme'
           FROM picked, unnest(themes) WITH ORDINALITY AS t (id, slot)",
        "INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
         SELECT md5('work' || n)::uuid, md5('rev' || n)::uuid, 1,
                timestamptz '2026-01-01' + n * interval '7 minutes', md5('snap' || n)::uuid
           FROM generate_series(1, $1::bigint) AS n",
    ] {
        let query = sqlx::query(sqlx::AssertSqlSafe(statement));
        let query = if statement.contains("$1") { query.bind(rows) } else { query };
        query.execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    sqlx::query("ANALYZE").execute(&pool).await.unwrap();
    let version: String = sqlx::query_scalar("SELECT version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("rows={rows} load={:?} {version}", started.elapsed());

    let deep: (OffsetDateTime, Uuid) = sqlx::query_as(
        "SELECT published_at, work_id FROM publications ORDER BY published_at DESC, work_id OFFSET $1 LIMIT 1",
    )
    .bind(rows / 2)
    .fetch_one(&pool)
    .await
    .unwrap();
    type Shape<'a> = (
        &'a str,
        Option<&'a str>,
        Vec<Uuid>,
        Option<(OffsetDateTime, Uuid)>,
    );
    let shapes: [Shape<'_>; 7] = [
        ("first page", None, vec![], None),
        ("q=mua", Some("mua"), vec![], None),
        (
            "q=khong co (no match: every row read)",
            Some("khong co"),
            vec![],
            None,
        ),
        (
            "tag=đô-thị (no work carries it: every index row filtered)",
            None,
            vec![DO_THI.as_uuid()],
            None,
        ),
        (
            "q=hoi uc tag=ký-ức",
            Some("hoi uc"),
            vec![KY_UC.as_uuid()],
            None,
        ),
        (
            "tag=thơ+mùa",
            None,
            vec![THO.as_uuid(), MUA.as_uuid()],
            None,
        ),
        ("middle cursor", None, vec![], Some(deep)),
    ];
    for (name, q, tags, after) in &shapes {
        let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "EXPLAIN (ANALYZE, BUFFERS, COSTS OFF) {}",
            vink_app::persistence::DISCOVERY_SEARCH_SQL
        )))
        .bind(q.map(|q| SearchKey::of(q).as_str().to_owned()))
        .bind(tags)
        .bind(after.map(|a| a.0))
        .bind(after.map(|a| a.1))
        .bind(13_i64)
        .fetch_all(&pool)
        .await
        .unwrap();
        println!("--- {name}\n{}", plan.join("\n"));
    }
    for path in [
        "/api/v1/works".to_string(),
        search(Some("mua"), &[], None, None),
        search(Some("hoi uc"), &["hồi-ức"], None, None),
        search(None, &["thơ", "mùa"], None, None),
        search(Some("khong co"), &[], None, None),
    ] {
        let mut times = Vec::new();
        for _ in 0..15 {
            let started = std::time::Instant::now();
            let reply = get(&app, &path, None).await;
            times.push(started.elapsed());
            assert_eq!(reply.status, StatusCode::OK);
        }
        times.sort();
        println!(
            "HTTP {path}: min={:?} median={:?} max={:?}",
            times[0], times[7], times[14]
        );
    }
}
