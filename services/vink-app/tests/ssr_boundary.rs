//! #235: real Leptos/Axum response boundary with a labelled synthetic public-read port.
//! This isolates scheduling/serialization; PostgreSQL authority is covered by the Web review lane.
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use tower::ServiceExt;
use vink_web::api::LoadError;
use vink_web::contract::{Publication, ReaderResponse, TagList, WorkList};
use vink_web::server::{PortFuture, PublicReads, SharedPublicReads, SiteOrigin};
use vink_web::{App, shell};

const WORK: &str = "33333333-3333-4333-8333-333333333333";
const PRIVATE: &str = "PRIVATE-DRAFT-MUST-NOT-ENTER-HTML";

#[derive(Clone)]
struct Probe {
    reader: Result<ReaderResponse, LoadError>,
    delay: Option<Duration>,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Semaphore>,
}

impl PublicReads for Probe {
    fn works(
        &self,
        _: vink_web::discovery::ExploreQuery,
    ) -> PortFuture<Result<WorkList, LoadError>> {
        Box::pin(std::future::ready(Err(LoadError::Unavailable)))
    }
    fn catalog(&self) -> PortFuture<Result<TagList, LoadError>> {
        Box::pin(std::future::ready(Err(LoadError::Unavailable)))
    }
    fn publication(&self, _: String) -> PortFuture<Result<Publication, LoadError>> {
        // A private or obsolete alternate representation must never be used for Reader.
        let mut p = publication();
        p.title = PRIVATE.into();
        Box::pin(std::future::ready(Ok(p)))
    }
    fn reader(&self, _: String) -> PortFuture<Result<ReaderResponse, LoadError>> {
        Box::pin(std::future::ready(self.reader.clone()))
    }
    fn discussion(
        &self,
        _: String,
    ) -> PortFuture<Result<vink_web::contract::PublicDiscussionPage, LoadError>> {
        let delay = self.delay;
        let entered = self.entered.clone();
        let release = self.release.clone();
        Box::pin(async move {
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            } else {
                entered.notify_one();
                let _permit = release.acquire().await.unwrap();
            }
            // A failed secondary read must not replace the critical article/status.
            Err(LoadError::Unavailable)
        })
    }
}

fn publication() -> Publication {
    serde_json::from_str(include_str!(
        "../contracts/http/v1/fixtures/publication.get.json"
    ))
    .unwrap()
}

fn probe(reader: Result<ReaderResponse, LoadError>) -> Probe {
    Probe {
        reader,
        delay: None,
        entered: Arc::default(),
        release: Arc::new(tokio::sync::Semaphore::new(0)),
    }
}

fn router(probe: Probe) -> Router {
    router_with_routes(probe, generate_route_list(App))
}

fn router_with_routes(probe: Probe, routes: Vec<leptos_axum::AxumRouteListing>) -> Router {
    let reads: SharedPublicReads = Arc::new(probe);
    let options = LeptosOptions::builder().output_name("vink").build();
    let rendered = options.clone();
    Router::new()
        .leptos_routes_with_context(
            &options,
            routes,
            move || {
                provide_context(reads.clone());
                provide_context(SiteOrigin("http://vink.test".into()));
            },
            move || shell(rendered.clone()),
        )
        .with_state(options)
}

fn reader_route_retains_async_as_the_rollback_baseline() {
    let routes = generate_route_list(App);
    let reader = routes
        .iter()
        .find(|r| r.path() == "/tac-pham/{id}")
        .expect("Reader route");
    assert_eq!(format!("{:?}", reader.mode()), "Async");
}

async fn reader_async_waits_for_secondary_before_committing_headers() {
    for delay_ms in [0, 200, 1000] {
        let probe = probe(Ok(ReaderResponse::Published(publication())));
        let request = router(probe.clone()).oneshot(
            Request::get(format!("/tac-pham/{WORK}"))
                .body(Body::empty())
                .unwrap(),
        );
        tokio::pin!(request);
        tokio::select! {
            _ = probe.entered.notified() => {},
            result = &mut request => panic!("headers committed before secondary resolved: {:?}", result.unwrap().status()),
            _ = tokio::time::sleep(Duration::from_secs(10)) => panic!("secondary never started"),
        }
        // A bounded synthetic hold, not a latency threshold or a production benchmark.
        assert!(
            tokio::time::timeout(Duration::from_millis(delay_ms + 1), &mut request)
                .await
                .is_err()
        );
        probe.release.add_permits(1);
        let response = tokio::time::timeout(Duration::from_secs(10), request)
            .await
            .expect("response after release")
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains("Phố cũ mưa giăng sợi mỏng,"));
        assert!(body.contains("<title>[Mẫu] Mưa trên phố cũ — Vink</title>"));
        assert!(body.contains("http://vink.test/tac-pham/33333333-3333-4333-8333-333333333333"));
        assert!(body.contains("name=\"description\""));
        assert!(body.contains("Chưa tải được nhận xét. Bạn vẫn có thể đọc tác phẩm."));
        let retry = body
            .split("<button")
            .find(|button| {
                button
                    .split('>')
                    .next()
                    .is_some_and(|tag| tag.contains("id=\"discussion-load-retry\""))
            })
            .expect("secondary failure offers a retry control")
            .split('>')
            .next()
            .unwrap();
        assert!(
            retry.contains("aria-disabled=\"true\""),
            "client-only retry must be unavailable before hydration: {retry}"
        );
        assert!(!body.contains(PRIVATE));
        println!(
            "#235 synthetic secondary hold {delay_ms}ms: final article/meta/status before response PASS"
        );
    }
}

async fn reader_control_and_errors_commit_safe_status_and_metadata() {
    for (reader, expected, notice, no_store) in [
        (
            Ok(ReaderResponse::Hidden {
                work_id: WORK.into(),
            }),
            StatusCode::OK,
            "Nội dung đã được ẩn",
            true,
        ),
        (
            Ok(ReaderResponse::ReviewWithdrawn {
                work_id: WORK.into(),
                author_name: "Tác giả mẫu".into(),
            }),
            StatusCode::OK,
            "Bản này đã bị gỡ sau khi kiểm duyệt.",
            true,
        ),
        (
            Err(LoadError::NotFound),
            StatusCode::NOT_FOUND,
            "Không tìm thấy trang này.",
            false,
        ),
        (
            Err(LoadError::Unavailable),
            StatusCode::SERVICE_UNAVAILABLE,
            "Tạm thời chưa đọc được",
            false,
        ),
    ] {
        println!("#235 rendering control {notice}");
        let probe = probe(reader);
        probe.release.add_permits(1);
        let response = router(probe)
            .oneshot(
                Request::get(format!("/tac-pham/{WORK}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if no_store {
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains(notice), "{notice}");
        assert!(!body.contains(PRIVATE));
        assert!(!body.contains("Phố cũ mưa giăng sợi mỏng,"));
        assert!(!body.contains("[Mẫu] Mưa trên phố cũ"));
        assert!(!body.contains("property=\"og:title\""));
        assert!(!body.contains("rel=\"canonical\""));
        if no_store {
            assert!(body.contains("content=\"noindex\""));
        }
    }
}

// Leptos 0.8.8 SuppressResourceLoad uses a process-global AtomicBool while generating
// routes. Keep this HTTP/scheduling matrix in its own integration-test process: concurrent
// library tests generate the real App route list and can otherwise suppress a pending read.
#[tokio::test]
async fn reader_ssr_boundary() {
    reader_route_retains_async_as_the_rollback_baseline();
    tokio::time::timeout(Duration::from_secs(30), async {
        reader_async_waits_for_secondary_before_committing_headers().await;
        reader_control_and_errors_commit_safe_status_and_metadata().await;
    })
    .await
    .expect("SSR matrix must complete within 30 seconds");
}

/// Loopback HTTP fixture for the opt-in direct/proxy comparison. The runtime route is unchanged.
#[tokio::test]
#[ignore = "opt-in #235 chunk probe; use the evidence recipe with VINK_SSR_PROBE_FILE"]
async fn reader_chunk_probe() {
    use leptos_router::SsrMode;
    let file = std::env::var("VINK_SSR_PROBE_FILE").expect("path for the chosen loopback port");
    let delay: u64 = std::env::var("VINK_SSR_PROBE_DELAY")
        .unwrap()
        .parse()
        .unwrap();
    let mode = std::env::var("VINK_SSR_PROBE_MODE").unwrap();
    let answer = match std::env::var("VINK_SSR_PROBE_CASE").unwrap().as_str() {
        "published" => Ok(ReaderResponse::Published(publication())),
        "hidden" => Ok(ReaderResponse::Hidden {
            work_id: WORK.into(),
        }),
        "withdrawn" => Ok(ReaderResponse::ReviewWithdrawn {
            work_id: WORK.into(),
            author_name: "Tác giả mẫu".into(),
        }),
        "missing" => Err(LoadError::NotFound),
        "error" => Err(LoadError::Unavailable),
        _ => panic!("unknown probe case"),
    };
    let mut port = probe(answer);
    port.delay = Some(Duration::from_millis(delay));
    let routes = generate_route_list(App)
        .into_iter()
        .map(|r| {
            if r.path() != "/tac-pham/{id}" || mode == "async" {
                return r;
            }
            assert_eq!(mode, "out-of-order");
            leptos_axum::AxumRouteListing::new(
                r.path().into(),
                SsrMode::OutOfOrder,
                r.methods(),
                vec![],
            )
        })
        .collect();
    let app = router_with_routes(port, routes);
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    std::fs::write(file, socket.local_addr().unwrap().to_string()).unwrap();
    axum::serve(socket, app).await.unwrap();
}
