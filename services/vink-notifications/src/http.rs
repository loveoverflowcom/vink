//! The private contract v1 (`contracts/rpc/v1`), served as two routers on two Unix sockets
//! (T-07 CHOICE-21 A):
//!
//! - **ingest** (App relay, `vink-worker`): `POST /v1/ingest`.
//! - **inbox** (App Gateway, `vink-app`): `/v1/inboxes/{recipient}/…` count, groups, seen, read,
//!   mark-all-read.
//!
//! Each socket answers only connections whose OS peer user is its configured caller; a header,
//! body field or address is never evidence of a caller, and a route of one scope does not exist
//! on the other socket. Both expose `/health/live` and `/health/ready`. The recipient in an inbox
//! path is the principal the Gateway resolved from the session; the service cannot check it and
//! trusts only the authenticated Gateway to choose it (K5).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::postgres::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::persistence::{self, SchemaState};
use crate::rollup::MAX_STALENESS_SECONDS;
use crate::server::Peer;
use crate::store::{self, Command, InboxError};

/// Bound on one call, below the relay's send timeout (10 s) and the Gateway's call budget.
pub const DEADLINE: Duration = Duration::from_secs(5);
/// Events per ingest call; equals the relay's `MAX_BATCH`.
pub const MAX_BATCH: usize = 100;
/// Request body limit: a full batch of maximal envelopes plus framing.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_COMMAND_KEY: usize = 128;

/// Which socket a router serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Ingest,
    Inbox,
}

impl Scope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Inbox => "inbox",
        }
    }
}

pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

#[derive(Clone)]
pub struct NotificationState {
    pool: PgPool,
    caller_uid: Option<u32>,
    scope: Scope,
    clock: Clock,
    deadline: Duration,
}

impl NotificationState {
    pub fn new(pool: PgPool, scope: Scope, caller_uid: Option<u32>) -> Self {
        Self {
            pool,
            caller_uid,
            scope,
            clock: Arc::new(OffsetDateTime::now_utc),
            deadline: DEADLINE,
        }
    }

    /// Replaces the wall clock (tests of retention and late delivery).
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    fn now(&self) -> OffsetDateTime {
        (self.clock)()
    }
}

/// A contract error: fixed code and message, never echoing input.
fn error(status: StatusCode, code: &'static str) -> Response {
    let message = match code {
        "caller_unauthenticated" => "this socket answers only its configured caller",
        "invalid_request" => "the request does not match the contract",
        "batch_too_large" => "too many events in one call",
        "invalid_snapshot" => "unknown or future inbox snapshot",
        "invalid_cursor" => "unknown cursor",
        "invalid_group_version" => "the group never had that version",
        "idempotency_key_reused" => "this command key was used for another command",
        "not_found" => "no such group in this inbox",
        "deadline_exceeded" => "the call did not finish in time",
        "unknown_route" => "this socket has no such route",
        "recovering" => "the inbox is being reconciled after a restore",
        _ => "the notification store is unavailable",
    };
    let mut response = (
        status,
        axum::Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn ok(body: Value) -> Response {
    let mut response = (StatusCode::OK, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// The router for one socket.
pub fn router(state: NotificationState) -> Router {
    let contract = match state.scope {
        Scope::Ingest => Router::new().route("/v1/ingest", post(ingest)),
        Scope::Inbox => Router::new()
            .route("/v1/inboxes/{recipient}/count", get(count))
            .route("/v1/inboxes/{recipient}/groups", get(groups))
            .route("/v1/inboxes/{recipient}/seen", post(seen))
            .route("/v1/inboxes/{recipient}/read", post(read))
            .route("/v1/inboxes/{recipient}/mark-all-read", post(mark_all_read)),
    }
    .layer(middleware::from_fn_with_state(state.clone(), deadline))
    .layer(middleware::from_fn_with_state(
        state.clone(),
        authenticate_caller,
    ))
    .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));
    Router::new()
        .route(
            "/health/live",
            get(|| async { ok(json!({ "status": "live" })) }),
        )
        .route("/health/ready", get(ready))
        .merge(contract)
        .fallback(|| async { error(StatusCode::NOT_FOUND, "unknown_route") })
        .with_state(state)
}

/// Fails closed: no configured caller, an unknown peer or another user is refused before the
/// body is read.
async fn authenticate_caller(
    State(state): State<NotificationState>,
    request: Request,
    next: Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<Peer>>()
        .map(|info| info.0);
    let peer_uid = peer.and_then(|peer| peer.uid);
    match state.caller_uid {
        Some(allowed) if peer_uid == Some(allowed) => next.run(request).await,
        Some(_) => {
            tracing::warn!(
                scope = state.scope.label(),
                ?peer_uid,
                "call refused: peer is not the configured caller"
            );
            error(StatusCode::UNAUTHORIZED, "caller_unauthenticated")
        }
        None => {
            tracing::warn!(
                scope = state.scope.label(),
                "call refused: no caller is configured"
            );
            error(StatusCode::UNAUTHORIZED, "caller_unauthenticated")
        }
    }
}

/// Bounds the whole call; a dropped transaction rolls back, so a timed-out ingest applied nothing
/// unless it already committed (the relay then gets `already_applied` on retry).
async fn deadline(
    State(state): State<NotificationState>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(state.deadline, next.run(request)).await {
        Ok(response) => response,
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "deadline_exceeded"),
    }
}

/// While an operator reconciles after a restore, inbox reads and commands are refused with
/// `503 recovering`: a projection that may be behind or ahead of the App is never served as
/// current (contract §1.3 step 5). Ingest is not gated, so the App relay's replay can run.
/// Called by each inbox handler after its own request checks, so a malformed request is still
/// `400` before any database call.
async fn recovery_gate(state: &NotificationState) -> Option<Response> {
    match store::recovery(&state.pool).await {
        Ok(recovery) if !recovery.recovering => None,
        Ok(_) => Some(error(StatusCode::SERVICE_UNAVAILABLE, "recovering")),
        Err(failure) => {
            tracing::warn!(
                error = store::error_class(&failure),
                "recovery gate unreadable"
            );
            Some(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
            ))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngestRequest {
    events: Vec<IngestItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngestItem {
    event_id: Uuid,
    envelope: String,
}

/// Applies each event in its own transaction, in order, and answers one result per event.
async fn ingest(State(state): State<NotificationState>, body: Bytes) -> Response {
    let Ok(request) = serde_json::from_slice::<IngestRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if request.events.len() > MAX_BATCH {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "batch_too_large");
    }
    let mut results = Vec::with_capacity(request.events.len());
    let mut applied = 0usize;
    let mut signals = 0usize;
    for item in &request.events {
        let ingested = store::ingest(&state.pool, item.event_id, &item.envelope, state.now()).await;
        if ingested.status == store::IngestStatus::Applied {
            applied += 1;
        }
        if ingested.effect == Some(store::Effect::Signal) {
            signals += 1;
        }
        results.push(json!({ "event_id": item.event_id, "status": ingested.status.code() }));
    }
    tracing::debug!(
        events = results.len(),
        applied,
        signals,
        "ingest call answered"
    );
    ok(json!({ "results": results }))
}

fn recipient(raw: &str) -> Option<Uuid> {
    let id = Uuid::try_parse(raw).ok()?;
    (id.hyphenated().to_string() == raw).then_some(id)
}

fn inbox_error(error_kind: InboxError) -> Response {
    match error_kind {
        InboxError::InvalidSnapshot => error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_snapshot"),
        InboxError::InvalidCursor => error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_cursor"),
        InboxError::InvalidVersion => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_group_version")
        }
        InboxError::IdempotencyKeyReused => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused")
        }
        InboxError::NotFound => error(StatusCode::NOT_FOUND, "not_found"),
        InboxError::Unavailable => error(StatusCode::SERVICE_UNAVAILABLE, "dependency_unavailable"),
    }
}

async fn count(State(state): State<NotificationState>, Path(raw): Path<String>) -> Response {
    let Some(recipient) = recipient(&raw) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if let Some(refused) = recovery_gate(&state).await {
        return refused;
    }
    match store::count(&state.pool, recipient, state.now()).await {
        Ok(count) => ok(json!({
            "status": "fresh",
            "unseen_group_count": count.unseen_group_count,
            "as_of_snapshot": count.as_of.token(),
            "max_staleness_seconds": MAX_STALENESS_SECONDS,
        })),
        Err(kind) => inbox_error(kind),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<i64>,
    cursor: Option<String>,
}

async fn groups(
    State(state): State<NotificationState>,
    Path(raw): Path<String>,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (Some(recipient), Ok(Query(query))) = (recipient(&raw), query) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let limit = query.limit.unwrap_or(20);
    if !(1..=store::MAX_PAGE).contains(&limit) {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    if let Some(refused) = recovery_gate(&state).await {
        return refused;
    }
    match store::page(
        &state.pool,
        recipient,
        limit,
        query.cursor.as_deref(),
        state.now(),
    )
    .await
    {
        Ok(page) => ok(json!({
            "inbox_snapshot": page.snapshot.token(),
            "seen_snapshot": page.seen.token(),
            "groups": page.groups.iter().map(store::GroupView::json).collect::<Vec<_>>(),
            "next_cursor": page.next_cursor,
        })),
        Err(kind) => inbox_error(kind),
    }
}

fn command_key(raw: &str) -> bool {
    (1..=MAX_COMMAND_KEY).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_graphic())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotCommand {
    command_key: String,
    up_to_snapshot: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadCommand {
    command_key: String,
    group_id: Uuid,
    observed_group_version: i64,
}

async fn run(state: &NotificationState, raw: &str, key: &str, command: Command) -> Response {
    let Some(recipient) = recipient(raw) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if !command_key(key) {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    if let Some(refused) = recovery_gate(state).await {
        return refused;
    }
    match store::command(&state.pool, recipient, key, &command, state.now()).await {
        Ok(answer) => ok(answer),
        Err(kind) => inbox_error(kind),
    }
}

async fn seen(
    State(state): State<NotificationState>,
    Path(raw): Path<String>,
    body: Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<SnapshotCommand>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let command = Command::Seen {
        up_to: request.up_to_snapshot,
    };
    run(&state, &raw, &request.command_key, command).await
}

async fn read(
    State(state): State<NotificationState>,
    Path(raw): Path<String>,
    body: Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<ReadCommand>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let command = Command::Read {
        group_id: request.group_id,
        observed_version: request.observed_group_version,
    };
    run(&state, &raw, &request.command_key, command).await
}

async fn mark_all_read(
    State(state): State<NotificationState>,
    Path(raw): Path<String>,
    body: Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<SnapshotCommand>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let command = Command::MarkAllRead {
        up_to: request.up_to_snapshot,
    };
    run(&state, &raw, &request.command_key, command).await
}

/// Ready only when the database answers with the current schema, the role cannot change schema
/// or roles, and this socket has a caller.
async fn ready(State(state): State<NotificationState>) -> Response {
    let probe = tokio::time::timeout(Duration::from_secs(2), async {
        let schema = persistence::schema_state(&state.pool).await?;
        let privileged = persistence::role_is_privileged(&state.pool).await?;
        Ok::<_, sqlx::Error>((schema, privileged))
    })
    .await;
    let (database, schema, role) = match probe {
        Ok(Ok((schema, privileged))) => (
            "ok",
            Some(schema),
            if privileged {
                "privileged"
            } else {
                "least_privileged"
            },
        ),
        Ok(Err(_)) | Err(_) => ("unavailable", None, "unknown"),
    };
    let callers = state.caller_uid.is_some();
    let ready = schema == Some(SchemaState::Current) && role == "least_privileged" && callers;
    // Reported, not a readiness condition: ingest must stay up for the replay, and a gated inbox
    // already answers `503 recovering` per call.
    let recovery = if schema == Some(SchemaState::Current) {
        match store::recovery(&state.pool).await {
            Ok(recovery) if recovery.recovering => "recovering",
            Ok(_) => "serving",
            Err(_) => "unknown",
        }
    } else {
        "unknown"
    };
    let body = json!({
        "status": if ready { "ready" } else { "not_ready" },
        "scope": state.scope.label(),
        "checks": {
            "database": database,
            "schema": schema.map_or("unknown", SchemaState::label),
            "role": role,
            "callers": if callers { "configured" } else { "not_configured" },
            "recovery": recovery,
        },
    });
    let mut response = (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        axum::Json(body),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app(scope: Scope, caller: Option<u32>, peer: Option<u32>) -> Router {
        // Nothing listens on port 1: only routes that never reach the database are exercised.
        let pool = persistence::lazy_pool("postgres://nobody:unused@127.0.0.1:1/none").unwrap();
        router(NotificationState::new(pool, scope, caller))
            .layer(axum::Extension(ConnectInfo(Peer { uid: peer })))
    }

    async fn call(router: Router, method: &str, uri: &str, body: &str) -> (StatusCode, Value) {
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    const INBOX: &str = "/v1/inboxes/22222222-2222-4222-8222-22222222222b/count";

    #[tokio::test]
    async fn only_the_configured_peer_user_reaches_the_contract() {
        for (caller, peer) in [(None, Some(501)), (Some(501), None), (Some(501), Some(502))] {
            let (status, body) =
                call(app(Scope::Ingest, caller, peer), "POST", "/v1/ingest", "{}").await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{caller:?}/{peer:?}");
            assert_eq!(body["error"]["code"], "caller_unauthenticated");
        }
        let (status, _) = call(app(Scope::Inbox, Some(501), Some(502)), "GET", INBOX, "").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // The right caller with a malformed body is a contract error, not an auth error.
        let (status, body) = call(
            app(Scope::Ingest, Some(501), Some(501)),
            "POST",
            "/v1/ingest",
            "{\"events\":1}",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request");
    }

    #[tokio::test]
    async fn a_route_of_one_scope_does_not_exist_on_the_other_socket() {
        let (status, body) = call(
            app(Scope::Inbox, Some(501), Some(501)),
            "POST",
            "/v1/ingest",
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "unknown_route");
        let (status, _) = call(app(Scope::Ingest, Some(501), Some(501)), "GET", INBOX, "").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn batches_and_paths_are_bounded_before_any_database_call() {
        let item = r#"{"event_id":"019a9b10-0001-7000-8000-000000000101","envelope":"{}"}"#;
        let body = format!("{{\"events\":[{}]}}", vec![item; MAX_BATCH + 1].join(","));
        let (status, body) = call(
            app(Scope::Ingest, Some(501), Some(501)),
            "POST",
            "/v1/ingest",
            &body,
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"]["code"], "batch_too_large");
        let upper = "/v1/inboxes/22222222-2222-4222-8222-22222222222B/count";
        let (status, _) = call(app(Scope::Inbox, Some(501), Some(501)), "GET", upper, "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = call(
            app(Scope::Inbox, Some(501), Some(501)),
            "POST",
            "/v1/inboxes/22222222-2222-4222-8222-22222222222b/seen",
            r#"{"command_key":"has space","up_to_snapshot":"s1"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn an_unreachable_database_is_an_explicit_outage_never_zero() {
        let (status, body) = call(app(Scope::Inbox, Some(501), Some(501)), "GET", INBOX, "").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "dependency_unavailable");
        let (status, body) = call(
            app(Scope::Inbox, Some(501), Some(501)),
            "GET",
            "/health/ready",
            "",
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["checks"]["database"], "unavailable");
        assert_eq!(body["scope"], "inbox");
    }
}
