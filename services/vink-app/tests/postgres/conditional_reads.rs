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

fn html_without_request_nonce(html: &str) -> String {
    // Leptos emits a fresh nonce per render, including on its resource preload tags.
    let (_, rest) = html
        .split_once("<script nonce=\"")
        .expect("hydration nonce");
    let (nonce, _) = rest.split_once('"').expect("end of hydration nonce");
    assert!(!nonce.is_empty());
    html.replace(&format!("nonce=\"{nonce}\""), "nonce=\"request-nonce\"")
        .replace(
            &format!("crossorigin=\"{nonce}\""),
            "crossorigin=\"request-nonce\"",
        )
}

/// Fixture publish: an immutable revision plus the public pointer, as one transaction.
pub(super) async fn publish(
    pool: &PgPool,
    work: Uuid,
    owner: Uuid,
    title: &str,
    line: &str,
    published_at: OffsetDateTime,
) -> Uuid {
    publish_revision(
        pool,
        work,
        owner,
        titled(title, doc(&[&[line]])),
        published_at,
    )
    .await
}

async fn publish_revision(
    pool: &PgPool,
    work: Uuid,
    owner: Uuid,
    content: TitledDocument,
    published_at: OffsetDateTime,
) -> Uuid {
    let revision = RevisionContent::new(content).unwrap();
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
    .bind(sqlx::types::Json(revision.content().document().to_data()))
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

/// An already-approved scratch fixture qualifies read boundaries independently of admission.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn markdown_visibility_precedes_validation_for_json_and_html(pool: PgPool) {
    let app = app(&pool).await;
    let work = Uuid::now_v7();
    let title = "Tên Markdown của fixture";
    let source = format!("# {title}\n\n**Dòng công khai fixture**\n");
    let owner = dev_seed::SAMPLE_AUTHOR;
    let at = datetime!(2026-11-01 08:00:00 UTC);
    let reader = format!("/api/v1/works/{work}/reader");
    let publication = publication_path(work);
    let html = format!("/tac-pham/{work}");
    let unpublished = get(&app, &reader, &["*"]).await;
    assert_eq!(unpublished.status, StatusCode::NOT_FOUND);
    assert!(unpublished.header(header::ETAG).is_none());
    assert_eq!(unpublished.header(header::CACHE_CONTROL), Some("no-store"));

    publish_revision(
        &pool,
        work,
        owner,
        titled(title, super::source("default", &source)),
        at,
    )
    .await;
    let default = get(&app, &reader, &[]).await;
    default.assert_full();
    let markdown = TitledDocument::author(
        Document::new(DocumentFormat::Markdown, &source).unwrap(),
        None,
    )
    .unwrap();
    let revision = publish_revision(&pool, work, owner, markdown, at).await;
    let published = get(&app, &reader, &[default.etag()]).await;
    published.assert_full();
    assert_eq!(published.json()["revision_id"], revision.to_string());
    assert_eq!(
        published.json()["document"],
        json!({"format": "markdown", "source": source})
    );
    get(&app, &reader, &[published.etag()])
        .await
        .assert_not_modified(published.etag());
    let direct = get(&app, &publication, &[published.etag()]).await;
    direct.assert_full();
    assert_ne!(
        direct.etag(),
        published.etag(),
        "two different JSON representations"
    );
    let ssr = get(&app, &html, &[published.etag(), direct.etag()]).await;
    assert_eq!(ssr.status, StatusCode::OK);
    assert!(
        ssr.header(header::ETAG).is_none(),
        "JSON cannot validate HTML"
    );
    let body = String::from_utf8(ssr.body).unwrap();
    assert!(body.contains("<strong>Dòng công khai fixture"), "{body}");
    assert!(body.contains("data-revision=\"") && body.contains(&revision.to_string()));

    // No delete command yet. Project durable controls with synthetic rows and the public
    // projection removed atomically, leaving the immutable revision untouched.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(work)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO work_publications (
             work_id, lifecycle, audience, generation, intent_revision_id, intent_generation,
             review_withdrawn_revision_id, review_withdrawn_intent_generation, ever_public
         ) VALUES ($1, 'active', 'public', 2, $2, 2, $2, 2, ARRAY[$2]::uuid[])",
    )
    .bind(work)
    .bind(revision)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    for control in ["review_withdrawn", "hidden", "tombstone"] {
        match control {
            "hidden" => {
                sqlx::query(
                    "UPDATE work_publications SET audience = 'private', generation = 3,
                            intent_revision_id = NULL, intent_generation = NULL,
                            review_withdrawn_revision_id = NULL, review_withdrawn_intent_generation = NULL,
                            hidden_revision_id = $2, hidden_intent_generation = 2 WHERE work_id = $1",
                )
                .bind(work)
                .bind(revision)
                .execute(&pool)
                .await
                .unwrap();
            }
            "tombstone" => {
                sqlx::query(
                    "UPDATE work_publications SET lifecycle = 'author_deleted', generation = 4,
                            hidden_revision_id = NULL, hidden_intent_generation = NULL,
                            deletion_provenance = 'tombstone', tombstone_revision_id = $2,
                            tombstone_intent_generation = 2 WHERE work_id = $1",
                )
                .bind(work)
                .bind(revision)
                .execute(&pool)
                .await
                .unwrap();
            }
            _ => {}
        }
        for field in [published.etag(), "*"] {
            let response = get(&app, &reader, &[field]).await;
            assert_eq!(response.header(header::CACHE_CONTROL), Some("no-store"));
            assert!(response.header(header::ETAG).is_none());
            if control == "tombstone" {
                assert_eq!(response.status, StatusCode::NOT_FOUND);
            } else {
                assert_eq!(response.status, StatusCode::OK);
                assert_eq!(response.json()["status"], control);
            }
            assert!(response.json().get("document").is_none());
            let previous = get(&app, &publication, &[field]).await;
            assert_eq!(previous.status, StatusCode::NOT_FOUND);
            assert!(previous.header(header::ETAG).is_none());
            let previous_html = get(&app, &html, &[field]).await;
            assert_eq!(previous_html.status, response.status);
            assert!(previous_html.header(header::ETAG).is_none());
            assert!(
                !String::from_utf8(previous_html.body)
                    .unwrap()
                    .contains(title)
            );
        }
    }
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
    assert_eq!(
        empty.json(),
        json!({ "works": [], "filters": { "q": null, "tags": [] }, "next_cursor": null })
    );
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

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn profile_owner_cas_revoke_and_public_page_share_the_current_projection(pool: PgPool) {
    use super::handle::create;

    let app = app(&pool).await;
    let a = create(&pool, "profile-a", "Hai bút danh trùng")
        .await
        .as_uuid();
    let b = create(&pool, "profile-b", "Hai bút danh trùng")
        .await
        .as_uuid();
    let own = "/api/v1/me/profile";
    let (status, cache, body) = call(&app, "GET", own, Some(a), None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    let initial = json(&body);
    let mut policies = initial["policies"].clone();
    assert_eq!(
        policies,
        json!({
            "pen_name": "everyone", "avatar": "everyone", "bio": "everyone",
            "date_of_birth": "self", "followers_list": "everyone", "followers_count": "everyone",
            "following_list": "self", "following_count": "self"
        })
    );
    let handle = initial["handle"].as_str().unwrap();
    let path = format!("/api/v1/authors/{handle}");
    let web = format!("/@{handle}");
    let absent = get(&app, &path, &[]).await;
    let (status, cache, absent_html) = call(&app, "GET", &web, None, None, None).await;
    let absent_html = html_without_request_nonce(&absent_html);
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    absent.assert_full();
    assert_eq!(absent.json()["publications"]["works"], json!([]));
    assert!(absent.json().get("bio").is_none());
    let command = |version, bio: Option<&str>, policies: &Value| {
        json!({
            "principal_id": a, "expected_version": version, "pen_name": "Hai bút danh trùng",
            "bio": bio, "policies": policies
        })
    };
    for (actor, status) in [(None, 401), (Some(b), 403)] {
        let (result, cache, _) = call(
            &app,
            "PUT",
            own,
            actor,
            Some(ORIGIN),
            Some(command(0, Some("Không ghi"), &policies)),
        )
        .await;
        assert_eq!(result.as_u16(), status);
        assert_eq!(cache.as_deref(), Some("no-store"));
    }
    let a_call = call(
        &app,
        "PUT",
        own,
        Some(a),
        Some(ORIGIN),
        Some(command(0, Some("Mưa qua thềm"), &policies)),
    );
    let b_call = call(
        &app,
        "PUT",
        own,
        Some(a),
        Some(ORIGIN),
        Some(command(0, Some("Sương qua thềm"), &policies)),
    );
    let (one, two) = tokio::join!(a_call, b_call);
    let winner = match (one.0.as_u16(), two.0.as_u16()) {
        (200, 409) => json(&one.2),
        (409, 200) => json(&two.2),
        other => panic!("expected exactly one commit, got {other:?}"),
    };
    assert_eq!(winner["profile_version"], 1);
    let bio = winner["bio"].as_str().unwrap();
    let public = get(&app, &path, &[absent.etag()]).await;
    public.assert_full();
    assert_eq!(public.json()["bio"], bio);
    assert!(
        call(&app, "GET", &web, None, None, None)
            .await
            .2
            .contains(bio)
    );

    policies["bio"] = json!("self");
    let (_, _, saved) = call(
        &app,
        "PUT",
        own,
        Some(a),
        Some(ORIGIN),
        Some(command(1, Some(bio), &policies)),
    )
    .await;
    assert_eq!(json(&saved)["policies"]["followers_count"], "everyone");
    assert_eq!(json(&saved)["bio"], bio);
    let revoked = get(&app, &path, &[public.etag()]).await;
    revoked.assert_full();
    assert_eq!(
        revoked.body, absent.body,
        "private equals absent byte-for-byte"
    );
    assert_eq!(revoked.etag(), absent.etag());
    get(&app, &path, &[absent.etag()])
        .await
        .assert_not_modified(absent.etag());
    let head = read(&app, "HEAD", &path, &[public.etag()]).await;
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(head.etag(), absent.etag());
    assert!(head.body.is_empty());
    for actor in [None, Some(b), Some(a)] {
        let (status, _, html) = call(&app, "GET", &web, actor, None, None).await;
        assert_eq!(status, 200);
        assert_eq!(
            html_without_request_nonce(&html),
            absent_html,
            "HTML, head and hydration match the absent Bio"
        );
        assert!(
            !html.contains(bio)
                && !html.contains("profile_version")
                && !html.contains(&a.to_string())
        );
        let (_, _, body) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(body.as_bytes(), revoked.body);
    }

    // Revoking followers_list hides only the list link while preserving public followers_count
    // and moving the public ETag so the previous validator misses cleanly.
    policies["followers_list"] = json!("self");
    let (status, _, saved_list) = call(
        &app,
        "PUT",
        own,
        Some(a),
        Some(ORIGIN),
        Some(command(2, Some(bio), &policies)),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json(&saved_list)["policies"]["followers_list"], "self");
    assert_eq!(json(&saved_list)["policies"]["followers_count"], "everyone");
    let revoked_list = get(&app, &path, &[absent.etag()]).await;
    revoked_list.assert_full();
    assert_ne!(revoked_list.etag(), absent.etag());
    let revoked_list_json = revoked_list.json();
    assert_eq!(revoked_list_json["relationships"]["followers_count"], 0);
    assert!(
        revoked_list_json["relationships"]
            .get("followers_href")
            .is_none()
    );

    // Revoking followers_count removes the last relationship summary field from public output.
    policies["followers_count"] = json!("self");
    let (status, _, _) = call(
        &app,
        "PUT",
        own,
        Some(a),
        Some(ORIGIN),
        Some(command(3, Some(bio), &policies)),
    )
    .await;
    assert_eq!(status, 200);
    let revoked_all = get(&app, &path, &[revoked_list.etag()]).await;
    revoked_all.assert_full();
    assert!(revoked_all.json().get("relationships").is_none());
    let (_, _, body) = call(&app, "GET", own, Some(b), None, None).await;
    assert_eq!(json(&body)["bio"], Value::Null);
    let (status, _, _) = call(
        &app,
        "PUT",
        own,
        Some(a),
        Some(ORIGIN),
        Some(command(4, Some(bio), &initial["policies"])),
    )
    .await;
    assert_eq!(status, 422, "widening stays gated");
    let mut birth = command(2, Some("bio"), &policies);
    birth["date_of_birth"] = json!("2000-01-01");
    let mut no_bio = command(2, Some("bio"), &policies);
    no_bio.as_object_mut().unwrap().remove("bio");
    for invalid in [birth.clone(), no_bio] {
        // Unknown fields and an omitted Bio key are contract errors, never a silent clear.
        let (status, cache, body) =
            call(&app, "PUT", own, Some(a), Some(ORIGIN), Some(invalid)).await;
        assert_eq!((status.as_u16(), cache.as_deref()), (422, Some("no-store")));
        assert_eq!(json(&body)["error"]["code"], "validation_failed");
        assert_eq!(json(&body)["error"]["field"], "body");
    }
    // The session is checked before the body: a guest learns nothing about validation.
    assert_eq!(
        call(&app, "PUT", own, None, Some(ORIGIN), Some(birth))
            .await
            .0,
        401
    );
    let (_, _, kept) = call(&app, "GET", own, Some(a), None, None).await;
    assert_eq!(
        json(&kept)["bio"],
        json!(bio),
        "rejected saves leave the Bio intact"
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            own,
            Some(a),
            Some(ORIGIN),
            Some(command(2, Some("Riêng mới"), &policies))
        )
        .await
        .0,
        200
    );
    // A new server context over the persisted schema still rejects the old public validator.
    let reopened = super::app(&pool).await;
    let verified = get(&reopened, &path, &[public.etag()]).await;
    verified.assert_full();
    assert_eq!(verified.body, absent.body);

    let recent = datetime!(2026-11-19 01:00 UTC);
    let mut expected = Vec::new();
    for offset in 0..13 {
        let work = Uuid::now_v7();
        publish(
            &pool,
            work,
            a,
            "Tác phẩm đúng tác giả",
            "Văn bản",
            recent - time::Duration::minutes(offset),
        )
        .await;
        expected.push(work.to_string());
    }
    publish(
        &pool,
        Uuid::now_v7(),
        b,
        "Tác phẩm của người trùng tên",
        "Văn bản",
        recent + time::Duration::hours(1),
    )
    .await;
    let withdrawn = Uuid::now_v7();
    publish(
        &pool,
        withdrawn,
        a,
        "Mới rút",
        "Không còn công khai",
        recent + time::Duration::hours(2),
    )
    .await;
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(withdrawn)
        .execute(&pool)
        .await
        .unwrap();
    let unpublished = Uuid::now_v7();
    sqlx::query("INSERT INTO works(id, owner_id) VALUES($1, $2)")
        .bind(unpublished)
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();

    let first = get(&app, &path, &[]).await.json();
    let rows = |page: &Value| {
        page["publications"]["works"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["work_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&first), expected[..12]);
    let cursor = first["publications"]["next_cursor"].as_str().unwrap();
    let second = get(&app, &format!("{path}?cursor={cursor}"), &[])
        .await
        .json();
    assert_eq!(rows(&second), expected[12..]);
    let other_handle = PgStore::new(pool.clone())
        .own_handle(PrincipalId::from_uuid(b))
        .await
        .unwrap()
        .handle;
    let other = get(
        &app,
        &format!("/api/v1/authors/{other_handle}?cursor={cursor}"),
        &[],
    )
    .await
    .json();
    assert_eq!(other["publications"], json!({"state":"restart"}));
    let handle_state = PgStore::new(pool.clone())
        .own_handle(PrincipalId::from_uuid(a))
        .await
        .unwrap();
    assert_eq!(
        (
            handle_state.handle.as_str(),
            handle_state.setup_complete,
            handle_state.version,
            handle_state.remaining_changes
        ),
        (handle, false, 0, 2)
    );
    assert!(handle_state.aliases.is_empty());
}
