//! #273: production router + real loopback HTTP/1 + disposable PostgreSQL.
//! HeaderIdentity is the existing test-only principal seam, not production authentication.
use super::*;
use axum::body::Bytes;
use hyper::body::Frame;
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use unicode_normalization::UnicodeNormalization;
use vink_app::domain::document::{MAX_DOCUMENT_CHARS, MAX_SOURCE_BYTES};
use vink_app::domain::revision::ContentHash;
use vink_app::http::contract::MAX_SAVE_DRAFT_BODY_BYTES;

// No size hint: Hyper must send HTTP chunked framing, so the server cannot rely on a
// Content-Length precheck. Every frame is bounded, including for malicious test bodies.
struct Chunks(Bytes);
impl hyper::body::Body for Chunks {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        let length = self.0.len().min(16 * 1024);
        Poll::Ready((length != 0).then(|| Ok(Frame::data(self.0.split_to(length)))))
    }
}

async fn send(
    address: SocketAddr,
    method: &str,
    path: &str,
    actor: Option<Uuid>,
    origin: &str,
    raw: Vec<u8>,
    chunked: bool,
) -> (StatusCode, Value) {
    let bytes = raw.len();
    let start = Instant::now();
    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(socket))
        .await
        .unwrap();
    let connection = tokio::spawn(connection);
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, address.to_string())
        .header(header::ORIGIN, origin)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(actor) = actor {
        request = request.header("x-test-principal", actor.to_string());
    }
    let body = if chunked {
        Body::new(Chunks(Bytes::from(raw)))
    } else {
        Body::from(raw)
    };
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        sender.send_request(request.body(body).unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    let status = response.status();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    connection.abort();
    println!(
        "draft-http bytes={bytes} chunked={chunked} status={status} elapsed_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    (status, serde_json::from_slice(&body).unwrap())
}

async fn server(pool: &PgPool) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = app(pool).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (address, task)
}

fn wire(version: i64, title: Option<&str>, document: &DocumentData) -> Vec<u8> {
    // gloo-net RequestBuilder::json uses this exact serde_json serializer on Web's DTO.
    let request = vink_web::contract::SaveDraftRequest {
        expected_version: version,
        title: title.map(str::to_owned),
        document: serde_json::from_value(serde_json::to_value(document).unwrap()).unwrap(),
    };
    serde_json::to_vec(&request).unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn anonymous_draft_http_budget_returns_401_before_reading_body(pool: PgPool) {
    let app = app(&pool).await;
    let body = Body::from("{}").map_frame(|_| -> Frame<Bytes> {
        panic!("an anonymous draft save must return before reading the body")
    });
    let request = Request::put(format!("/api/v1/drafts/{}", dev_seed::SAMPLE_WORK))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::new(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_http_budget_accepts_client_corpus_and_preserves_authority(pool: PgPool) {
    let (address, server) = server(&pool).await;
    let path = format!("/api/v1/drafts/{}", dev_seed::SAMPLE_WORK);
    let owner = Some(dev_seed::SAMPLE_AUTHOR);
    let mut version = 1;
    let title = "\u{1f82}".repeat(200).nfd().collect::<String>();
    let mut cases: Vec<_> = super::document_corpus::SHAPES
        .iter()
        .map(|shape| {
            (
                shape.to_string(),
                super::document_corpus::corpus(shape, MAX_DOCUMENT_CHARS),
            )
        })
        .collect();
    for (name, character) in [
        ("four-scalar-nfd", "\u{1f82}".nfd().collect::<String>()),
        ("escaped-quote", "\"".into()),
        ("escaped-backslash", "\\".into()),
        ("supplementary", "🧑".into()),
    ] {
        cases.push((
            name.into(),
            source("default", &character.repeat(MAX_DOCUMENT_CHARS)),
        ));
    }
    cases.push((
        "crlf-byte-limit".into(),
        source("default", &"\r\n".repeat(MAX_SOURCE_BYTES / 2)),
    ));
    cases.push((
        "markdown-derived-title".into(),
        source("markdown", "# Tiếng Việt\r\n\t  \r\n"),
    ));
    for (index, (name, input)) in cases.into_iter().enumerate() {
        let document = Document::parse(input.clone()).unwrap();
        let title = (input.format == "default").then_some(title.as_str());
        let content = TitledDocument::author(document.clone(), title).unwrap();
        let raw = wire(version, title, &input);
        assert!(raw.len() <= MAX_SAVE_DRAFT_BODY_BYTES);
        println!("corpus={name}");
        let (status, saved) = send(address, "PUT", &path, owner, ORIGIN, raw, index % 2 == 0).await;
        assert_eq!(status, StatusCode::OK, "{name}: {saved}");
        version += 1;
        assert_eq!(saved["version"], version);
        let (status, loaded) = send(address, "GET", &path, owner, ORIGIN, vec![], false).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(loaded["version"], version);
        assert_eq!(loaded["title"], content.title().as_str());
        assert_eq!(
            loaded["document"],
            serde_json::to_value(document.to_data()).unwrap()
        );
        let stored = Vink::new(PgStore::new(pool.clone()))
            .load_draft(author(), sample_work())
            .await
            .unwrap();
        assert_eq!(ContentHash::of(&stored.content), ContentHash::of(&content));
        assert_eq!(stored.version.get(), version);
        // Every refusal is followed by an exact readback including updated_at.
        for (actor, origin, expected, code) in [
            (owner, ORIGIN, version - 1, StatusCode::CONFLICT),
            (
                Some(dev_seed::OTHER_PRINCIPAL),
                ORIGIN,
                version,
                StatusCode::NOT_FOUND,
            ),
            (None, ORIGIN, version, StatusCode::UNAUTHORIZED),
            (
                owner,
                "https://foreign.invalid",
                version,
                StatusCode::FORBIDDEN,
            ),
        ] {
            let (status, _) = send(
                address,
                "PUT",
                &path,
                actor,
                origin,
                wire(expected, title, &input),
                true,
            )
            .await;
            assert_eq!(status, code);
            let (_, after) = send(address, "GET", &path, owner, ORIGIN, vec![], false).await;
            assert_eq!(loaded, after);
        }
    }
    server.abort();
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn draft_http_budget_limits_streamed_and_declared_bodies_without_writes(pool: PgPool) {
    let (address, server) = server(&pool).await;
    let path = format!("/api/v1/drafts/{}", dev_seed::SAMPLE_WORK);
    let owner = Some(dev_seed::SAMPLE_AUTHOR);
    let document = doc(&[&["Nội dung chính xác \\\" 🧑\t"]]);
    let mut version = 1;
    for chunked in [false, true] {
        for size in [
            MAX_SAVE_DRAFT_BODY_BYTES - 1,
            MAX_SAVE_DRAFT_BODY_BYTES,
            MAX_SAVE_DRAFT_BODY_BYTES + 1,
        ] {
            let (_, before) = send(address, "GET", &path, owner, ORIGIN, vec![], false).await;
            let mut raw = wire(version, Some("Tên"), &document);
            raw.resize(size, b' '); // legal JSON whitespace pins the actual byte boundary
            let (status, body) = send(address, "PUT", &path, owner, ORIGIN, raw, chunked).await;
            let (_, after) = send(address, "GET", &path, owner, ORIGIN, vec![], false).await;
            if size <= MAX_SAVE_DRAFT_BODY_BYTES {
                assert_eq!(status, StatusCode::OK);
                version += 1;
                assert_eq!(body["version"], version);
                assert_eq!(after["document"], serde_json::to_value(&document).unwrap());
                assert_eq!(after["version"], version);
            } else {
                assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
                assert_eq!(body["error"]["code"], "validation_failed");
                assert_eq!(body["error"]["field"], "body");
                assert_eq!(before, after);
            }
        }
    }
    let (_, before) = send(address, "GET", &path, owner, ORIGIN, vec![], false).await;
    let over_source = wire(
        version,
        Some("T"),
        &source("default", &"\n".repeat(MAX_SOURCE_BYTES + 1)),
    );
    assert!(over_source.len() < MAX_SAVE_DRAFT_BODY_BYTES);
    let mut malformed = wire(version, Some("T"), &document);
    malformed.pop();
    for (raw, field) in [
        (malformed, "body"),
        (b"{\"unexpected\":true}".to_vec(), "body"),
        (over_source, "document"),
        (
            wire(
                version,
                Some("T"),
                &super::document_corpus::corpus("tiny_blocks", MAX_DOCUMENT_CHARS + 1),
            ),
            "document",
        ),
        (wire(version, Some(&"T".repeat(201)), &document), "title"),
    ] {
        let (status, body) = send(address, "PUT", &path, owner, ORIGIN, raw, true).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["field"], field, "{body}");
        let (_, after) = send(address, "GET", &path, owner, ORIGIN, vec![], false).await;
        assert_eq!(before, after);
    }
    // A different JSON route keeps Axum's 2 MiB default. Without the guard the padded
    // object is valid and would update tag version, so this cannot pass on shape alone.
    let tags_path = format!("{path}/tags");
    let (_, tags_before) = send(address, "GET", &tags_path, owner, ORIGIN, vec![], false).await;
    let mut tags = serde_json::to_vec(
        &json!({"expected_version":tags_before["version"], "form":null,"themes":[]}),
    )
    .unwrap();
    tags.resize(2 * 1024 * 1024 + 1, b' ');
    let (status, body) = send(address, "PUT", &tags_path, owner, ORIGIN, tags, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["field"], "body");
    let (_, tags_after) = send(address, "GET", &tags_path, owner, ORIGIN, vec![], false).await;
    assert_eq!(tags_before, tags_after);
    server.abort();
}
