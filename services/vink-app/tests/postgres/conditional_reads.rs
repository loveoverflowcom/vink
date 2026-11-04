//! READ-01 conditional public reads on real PostgreSQL. The oracle for every validator is an
//! independent SHA-256 of the received body, so a `304` is only correct when the server would
//! send exactly the bytes the client already has. Publish/withdraw/rename have no product
//! command yet: these tests change the rows directly with SQL, labelled as fixtures.

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::macros::datetime;

use super::*;

const LIST: &str = "/api/v1/works";

fn publication_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/publication")
}

#[derive(Debug)]
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn etag(&self) -> &str {
        self.headers
            .get(header::ETAG)
            .unwrap_or_else(|| panic!("no ETag: {self:?}"))
            .to_str()
            .unwrap()
    }

    fn header(&self, name: header::HeaderName) -> Option<&str> {
        self.headers.get(name).map(|value| value.to_str().unwrap())
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    /// A full `200` whose validator is the digest of exactly these bytes.
    fn assert_full(&self) {
        assert_eq!(self.status, StatusCode::OK, "{self:?}");
        assert_eq!(self.etag(), oracle(&self.body), "validator of these bytes");
        assert_eq!(
            self.header(header::CACHE_CONTROL),
            Some("private, no-cache")
        );
        assert_eq!(self.header(header::CONTENT_TYPE), Some("application/json"));
        assert!(
            self.header(header::VARY).is_none(),
            "no header selects a variant"
        );
    }

    fn assert_not_modified(&self, etag: &str) {
        assert_eq!(self.status, StatusCode::NOT_MODIFIED, "{self:?}");
        assert!(self.body.is_empty());
        assert_eq!(self.etag(), etag);
        assert_eq!(
            self.header(header::CACHE_CONTROL),
            Some("private, no-cache")
        );
        assert!(self.header(header::CONTENT_TYPE).is_none());
    }
}

/// Independent of the implementation: quoted hex of the first 128 bits of SHA-256.
fn oracle(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

async fn read(app: &Router, method: &str, path: &str, if_none_match: &[&str]) -> Reply {
    let mut request = Request::builder().method(method).uri(path);
    for value in if_none_match {
        request = request.header(header::IF_NONE_MATCH, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        headers,
        body: body.to_vec(),
    }
}

async fn get(app: &Router, path: &str, if_none_match: &[&str]) -> Reply {
    read(app, "GET", path, if_none_match).await
}

/// Fixture publish: an immutable revision plus the public pointer, as one transaction.
async fn publish(
    pool: &PgPool,
    work: Uuid,
    owner: Uuid,
    title: &str,
    line: &str,
    published_at: OffsetDateTime,
) -> Uuid {
    let revision = RevisionContent::new(
        Title::parse(title).unwrap(),
        Document::parse(doc(&[&[line]])).unwrap(),
    )
    .unwrap();
    let revision_id = Uuid::now_v7();
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
    .bind(revision_id)
    .bind(work)
    .bind(revision.title().as_str())
    .bind(sqlx::types::Json(revision.document().to_data()))
    .bind(revision.hash().as_str())
    .bind(owner)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, $3)
         ON CONFLICT (work_id) DO UPDATE
            SET revision_id = EXCLUDED.revision_id,
                generation = publications.generation + 1,
                published_at = EXCLUDED.published_at",
    )
    .bind(work)
    .bind(revision_id)
    .bind(published_at)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    revision_id
}

async fn rename(pool: &PgPool, principal: Uuid, name: &str) {
    sqlx::query("UPDATE principals SET display_name = $2 WHERE id = $1")
        .bind(principal)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
}

fn listed_ids(reply: &Reply) -> Vec<String> {
    reply.json()["works"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["work_id"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn unchanged_public_reads_revalidate_with_not_modified(pool: PgPool) {
    let app = app(&pool).await;
    for path in [LIST.to_string(), publication_path(dev_seed::SAMPLE_WORK)] {
        let full = get(&app, &path, &[]).await;
        full.assert_full();
        let etag = full.etag().to_string();
        assert_eq!(get(&app, &path, &[]).await.etag(), etag, "{path}: stable");

        let weak = format!("W/{etag}");
        let listed = format!(r#""stale", {etag}"#);
        for field in [etag.as_str(), weak.as_str(), listed.as_str(), "*"] {
            get(&app, &path, &[field]).await.assert_not_modified(&etag);
        }
        // Separate field lines combine into one list.
        get(&app, &path, &[r#""stale""#, &etag])
            .await
            .assert_not_modified(&etag);

        // A near-miss, a malformed field and an oversized one are a full answer, never a 304.
        let prefix = &etag[..etag.len() - 2];
        let near = format!("{prefix}\"");
        let unquoted = etag.trim_matches('"').to_string();
        let star_list = format!("*, {etag}");
        let oversized = vec![etag.as_str(); 200].join(", ");
        for field in [&near, &unquoted, &star_list, &oversized] {
            let reply = get(&app, &path, &[field]).await;
            reply.assert_full();
            assert_eq!(reply.body, full.body, "{path}");
        }

        // HEAD: same validator and policy, no content; conditional HEAD is a 304 as well.
        let head = read(&app, "HEAD", &path, &[]).await;
        assert_eq!(head.status, StatusCode::OK);
        assert!(head.body.is_empty());
        assert_eq!(head.etag(), etag);
        assert_eq!(
            head.header(header::CACHE_CONTROL),
            Some("private, no-cache")
        );
        read(&app, "HEAD", &path, &[&etag])
            .await
            .assert_not_modified(&etag);
    }

    // A private draft save is not part of the public representation: still 304.
    let list = get(&app, LIST, &[]).await;
    let publication = get(&app, &publication_path(dev_seed::SAMPLE_WORK), &[]).await;
    Vink::new(PgStore::new(pool.clone()))
        .save_draft(
            author(),
            save(
                sample_work(),
                1,
                "Tiêu đề riêng",
                doc(&[&[dev_seed::DRAFT_ONLY_LINE]]),
            ),
        )
        .await
        .unwrap();
    get(&app, LIST, &[list.etag()])
        .await
        .assert_not_modified(list.etag());
    get(
        &app,
        &publication_path(dev_seed::SAMPLE_WORK),
        &[publication.etag()],
    )
    .await
    .assert_not_modified(publication.etag());

    // The SSR reader is not an API representation and does not carry the API validator.
    let html = get(
        &app,
        &format!("/tac-pham/{}", dev_seed::SAMPLE_WORK),
        &["*"],
    )
    .await;
    assert_eq!(html.status, StatusCode::OK);
    assert!(html.header(header::ETAG).is_none());
}

/// Each step changes the rows; a stale tag must get the new bytes and their own tag, and an
/// unchanged representation must still answer its tag with 304.
async fn step(
    app: &Router,
    sample: &str,
    list: &mut Reply,
    detail: &mut Reply,
    list_changes: bool,
    detail_changes: bool,
) {
    for (path, before, changes) in [(LIST, list, list_changes), (sample, detail, detail_changes)] {
        let reply = get(app, path, &[before.etag()]).await;
        if changes {
            reply.assert_full();
            assert_ne!(reply.etag(), before.etag(), "{path}");
            assert_ne!(reply.body, before.body, "{path}");
            *before = reply;
        } else {
            reply.assert_not_modified(before.etag());
        }
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn every_displayed_change_produces_a_new_validator(pool: PgPool) {
    let app = app(&pool).await;
    let sample = publication_path(dev_seed::SAMPLE_WORK);
    let mut list = get(&app, LIST, &[]).await;
    let mut detail = get(&app, &sample, &[]).await;

    // Author rename: the revision is unchanged, the byline is not.
    rename(&pool, dev_seed::SAMPLE_AUTHOR, "Tác giả đã đổi tên").await;
    step(&app, &sample, &mut list, &mut detail, true, true).await;
    assert_eq!(
        detail.json()["author_display_name"],
        "Tác giả đã đổi tên",
        "the new body carries the new byline"
    );

    // Renaming someone with no publication changes nothing public.
    rename(&pool, dev_seed::OTHER_PRINCIPAL, "Người khác đổi tên").await;
    step(&app, &sample, &mut list, &mut detail, false, false).await;

    // Pointer moves to a new revision of the same work (title and body change).
    let at = datetime!(2026-11-01 08:00:00 UTC);
    let revision = publish(
        &pool,
        dev_seed::SAMPLE_WORK,
        dev_seed::SAMPLE_AUTHOR,
        "Mưa trên phố cũ (bản hai)",
        "Một dòng của bản hai.",
        at,
    )
    .await;
    step(&app, &sample, &mut list, &mut detail, true, true).await;
    assert_eq!(detail.json()["revision_id"], revision.to_string());

    // Same title, body and time: only the pointed revision differs. Still a new representation.
    publish(
        &pool,
        dev_seed::SAMPLE_WORK,
        dev_seed::SAMPLE_AUTHOR,
        "Mưa trên phố cũ (bản hai)",
        "Một dòng của bản hai.",
        at,
    )
    .await;
    step(&app, &sample, &mut list, &mut detail, false, true).await;

    // Membership: another author's work at the SAME instant (tie broken by work_id).
    let tied = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001);
    publish(
        &pool,
        tied,
        dev_seed::OTHER_PRINCIPAL,
        "Bài cùng thời điểm",
        "Một dòng.",
        at,
    )
    .await;
    step(&app, &sample, &mut list, &mut detail, true, false).await;
    let order = listed_ids(&list);
    assert_eq!(
        order,
        [tied.to_string(), dev_seed::SAMPLE_WORK.to_string()],
        "a tie is ordered by work_id"
    );
    for _ in 0..3 {
        get(&app, LIST, &[list.etag()])
            .await
            .assert_not_modified(list.etag());
    }

    // Reorder without membership change: the sample work becomes newer.
    sqlx::query("UPDATE publications SET published_at = $2 WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .bind(datetime!(2026-11-02 08:00:00 UTC))
        .execute(&pool)
        .await
        .unwrap();
    step(&app, &sample, &mut list, &mut detail, true, true).await;
    assert_eq!(
        listed_ids(&list),
        [dev_seed::SAMPLE_WORK.to_string(), tied.to_string()]
    );

    // Withdrawal fixture: the list shrinks, and an old tag for the work is 404, never 304.
    let tied_path = publication_path(tied);
    let tied_reply = get(&app, &tied_path, &[]).await;
    tied_reply.assert_full();
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(tied)
        .execute(&pool)
        .await
        .unwrap();
    step(&app, &sample, &mut list, &mut detail, true, false).await;
    assert_eq!(listed_ids(&list), [dev_seed::SAMPLE_WORK.to_string()]);
    for field in [tied_reply.etag(), "*"] {
        let gone = get(&app, &tied_path, &[field]).await;
        assert_eq!(gone.status, StatusCode::NOT_FOUND, "{field}");
        assert_eq!(gone.json()["error"]["code"], "not_found");
        assert_eq!(gone.header(header::CACHE_CONTROL), Some("no-store"));
        assert!(gone.header(header::ETAG).is_none());
    }
    // Missing and malformed identifiers: 404 with no-store even when the client sends `*`.
    for path in [
        publication_path(Uuid::now_v7()),
        "/api/v1/works/not-a-uuid/publication".to_string(),
    ] {
        let missing = get(&app, &path, &["*"]).await;
        assert_eq!(missing.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(missing.header(header::CACHE_CONTROL), Some("no-store"));
    }

    // Empty collection is a representation too: it has its own validator.
    sqlx::query("DELETE FROM publications")
        .execute(&pool)
        .await
        .unwrap();
    let empty = get(&app, LIST, &[list.etag()]).await;
    empty.assert_full();
    assert_eq!(empty.json(), json!({ "works": [] }));
    get(&app, LIST, &[empty.etag()])
        .await
        .assert_not_modified(empty.etag());
}

/// Each read is one SQL statement (one snapshot) serialized once, so under concurrent renames a
/// validator always belongs to the body it travels with, and a tag from one state never yields
/// a 304 in the other.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_renames_never_pair_a_validator_with_another_body(pool: PgPool) {
    let app = app(&pool).await;
    const NAMES: [&str; 2] = ["Tên thứ nhất", "Tên thứ hai"];
    let path = publication_path(dev_seed::SAMPLE_WORK);
    let mut tags = Vec::new();
    for name in NAMES {
        rename(&pool, dev_seed::SAMPLE_AUTHOR, name).await;
        let reply = get(&app, &path, &[]).await;
        reply.assert_full();
        tags.push(reply.etag().to_string());
    }
    assert_ne!(tags[0], tags[1]);

    let writer = {
        let pool = pool.clone();
        tokio::spawn(async move {
            for round in 0..60 {
                rename(&pool, dev_seed::SAMPLE_AUTHOR, NAMES[round % 2]).await;
                tokio::task::yield_now().await;
            }
        })
    };
    let readers: Vec<_> = (0..8)
        .map(|reader| {
            let app = app.clone();
            let path = path.clone();
            let tags = tags.clone();
            tokio::spawn(async move {
                let mut not_modified = 0;
                for _ in 0..25 {
                    // Ask with the tag of one known state; the answer is a 304 for that state
                    // or the full other state with its own tag.
                    let asked = &tags[reader % 2];
                    let reply = get(&app, &path, &[asked]).await;
                    match reply.status {
                        StatusCode::NOT_MODIFIED => {
                            reply.assert_not_modified(asked);
                            not_modified += 1;
                        }
                        _ => {
                            reply.assert_full();
                            let name = reply.json()["author_display_name"].clone();
                            let index = NAMES.iter().position(|n| name == *n).unwrap();
                            assert_eq!(reply.etag(), tags[index], "tag of the body's own state");
                            assert_ne!(reply.etag(), asked);
                        }
                    }
                }
                not_modified
            })
        })
        .collect();
    writer.await.unwrap();
    for reader in readers {
        reader.await.unwrap();
    }
    // After the writer stops, the settled state answers its own tag with 304.
    let settled = get(&app, &path, &[]).await;
    settled.assert_full();
    get(&app, &path, &[settled.etag()])
        .await
        .assert_not_modified(settled.etag());
}
