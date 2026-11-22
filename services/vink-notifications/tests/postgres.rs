//! The Notification Service on real PostgreSQL. Each test gets a fresh database (`#[sqlx::test]`)
//! from the test harness credential that `cargo xtask verify postgres` passes as `DATABASE_URL`.
//! Role separation and the real Unix sockets are exercised by the lane's process checks.

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::datetime;
use tower::ServiceExt;
use uuid::Uuid;
use vink_notifications::envelope::{canonical, sha256_hex};
use vink_notifications::http::{NotificationState, Scope, router};
use vink_notifications::persistence::{SchemaState, migrate, schema_state};
use vink_notifications::server::Peer;
use vink_notifications::store::{self, Command, Effect, IngestStatus, Page};

#[path = "postgres/budget.rs"]
mod budget;
#[path = "postgres/inbox.rs"]
mod inbox;
#[path = "postgres/ingest.rs"]
mod ingest;
#[path = "postgres/recovery.rs"]
mod recovery;
#[path = "postgres/retention.rs"]
mod retention;
#[path = "postgres/ung.rs"]
mod ung;

pub const CALLER: u32 = 501;
pub const T0: OffsetDateTime = datetime!(2026-11-21 00:00 UTC);

/// Readable UUIDs: `id(0xA1)` → `a1a1a1a1-…`.
pub fn id(byte: u8) -> Uuid {
    let mut bytes = [byte; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub fn event_id() -> Uuid {
    Uuid::now_v7()
}

/// Signs an envelope as the App does: canonical text without `payload_digest`, then SHA-256.
pub fn sign(mut value: Value) -> String {
    let object = value.as_object_mut().unwrap();
    object.remove("payload_digest");
    let digest = sha256_hex(canonical(&Value::Object(object.clone())).as_bytes());
    object.insert("payload_digest".into(), digest.into());
    canonical(&value)
}

fn at(time: OffsetDateTime) -> String {
    time.format(&Rfc3339).unwrap()
}

fn sorted(mut recipients: Vec<Uuid>) -> Vec<String> {
    recipients.sort();
    recipients.iter().map(Uuid::to_string).collect()
}

/// `comment.root.approved` of `comment` by `actor` on `work`, owned by `owner`.
pub fn root(
    event: Uuid,
    actor: Uuid,
    owner: Uuid,
    work: Uuid,
    comment: Uuid,
    version: i64,
    when: OffsetDateTime,
) -> String {
    sign(json!({
        "event_id": event.to_string(),
        "event_type": "comment.root.approved",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": version,
        "publication_stamp": {"epoch": 1, "readable": true},
        "occurred_at": at(when),
        "actor_id": actor.to_string(),
        "recipients": sorted(vec![owner]),
        "target": {
            "kind": "work_comment_thread",
            "work_id": work.to_string(),
            "comment_id": comment.to_string(),
            "moderation_version": version
        },
        "semantic_kind": "approved",
        "action_required": false
    }))
}

/// `comment.reply.approved` directly under `root_comment`, to `recipients`.
pub fn reply(
    event: Uuid,
    actor: Uuid,
    recipients: Vec<Uuid>,
    work: Uuid,
    root_comment: Uuid,
    comment: Uuid,
    when: OffsetDateTime,
) -> String {
    sign(json!({
        "event_id": event.to_string(),
        "event_type": "comment.reply.approved",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": 2,
        "publication_stamp": {"epoch": 1, "readable": true},
        "occurred_at": at(when),
        "actor_id": actor.to_string(),
        "recipients": sorted(recipients),
        "target": {
            "kind": "work_comment_reply",
            "work_id": work.to_string(),
            "root_id": root_comment.to_string(),
            "reply_context_kind": "root",
            "reply_target_id": root_comment.to_string(),
            "comment_id": comment.to_string(),
            "moderation_version": 2
        },
        "semantic_kind": "approved",
        "action_required": false
    }))
}

/// `work.endorsement.tham.changed`: `add` (with `first`) or `remove`.
#[allow(clippy::too_many_arguments)]
pub fn tham(
    event: Uuid,
    actor: Uuid,
    owner: Uuid,
    work: Uuid,
    revision: Uuid,
    add: Option<bool>,
    order: i64,
    readable: bool,
    when: OffsetDateTime,
) -> String {
    let mut value = json!({
        "event_id": event.to_string(),
        "event_type": "work.endorsement.tham.changed",
        "schema_version": 1,
        "source_owner": "endorsement",
        "activation_epoch": 1,
        "source_order": order,
        "publication_stamp": {"epoch": 1, "readable": readable},
        "occurred_at": at(when),
        "actor_id": actor.to_string(),
        "recipients": sorted(vec![owner]),
        "target": {
            "kind": "work_endorsement",
            "work_id": work.to_string(),
            "revision_id": revision.to_string()
        },
        "semantic_kind": if add.is_some() { "add" } else { "remove" },
        "action_required": false
    });
    if let Some(first) = add {
        value["first_selection"] = json!(first);
    }
    sign(value)
}

/// `comment.endorsement.ung.changed` (#365): Ưng on `comment` of `work`, owed to `writer`.
/// `add: Some(first_selection)` adds, `None` removes.
#[allow(clippy::too_many_arguments)]
pub fn ung(
    event: Uuid,
    actor: Uuid,
    writer: Uuid,
    work: Uuid,
    comment: Uuid,
    add: Option<bool>,
    order: i64,
    when: OffsetDateTime,
) -> String {
    let mut value = json!({
        "event_id": event.to_string(),
        "event_type": "comment.endorsement.ung.changed",
        "schema_version": 1,
        "source_owner": "endorsement",
        "activation_epoch": 1,
        "source_order": order,
        "publication_stamp": {"epoch": 1, "readable": true},
        "occurred_at": at(when),
        "actor_id": actor.to_string(),
        "recipients": sorted(vec![writer]),
        "target": {
            "kind": "work_comment_endorsement",
            "work_id": work.to_string(),
            "comment_id": comment.to_string()
        },
        "semantic_kind": if add.is_some() { "add" } else { "remove" },
        "action_required": false
    });
    if let Some(first) = add {
        value["first_selection"] = json!(first);
    }
    sign(value)
}

/// `work.moderation.decided` for `revision` at `version`.
pub fn decided(
    event: Uuid,
    owner: Uuid,
    work: Uuid,
    revision: Uuid,
    version: i64,
    rejected: bool,
    when: OffsetDateTime,
) -> String {
    sign(json!({
        "event_id": event.to_string(),
        "event_type": "work.moderation.decided",
        "schema_version": 1,
        "source_owner": "publication_moderation",
        "activation_epoch": 1,
        "source_order": version,
        "occurred_at": at(when),
        "actor_id": null,
        "recipients": [owner.to_string()],
        "target": {
            "kind": "work_moderation_decision",
            "work_id": work.to_string(),
            "revision_id": revision.to_string(),
            "decision_version": version
        },
        "semantic_kind": if rejected { "rejected" } else { "approved" },
        "action_required": rejected
    }))
}

/// `notification.invalidation` of a comment that stopped being public at `version`.
pub fn comment_gone(
    event: Uuid,
    work: Uuid,
    comment: Uuid,
    version: i64,
    when: OffsetDateTime,
) -> String {
    sign(json!({
        "event_id": event.to_string(),
        "event_type": "notification.invalidation",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": version,
        "occurred_at": at(when),
        "actor_id": null,
        "recipients": [],
        "target": {"kind": "comment", "work_id": work.to_string(), "comment_id": comment.to_string()},
        "semantic_kind": "invalidate",
        "action_required": false
    }))
}

/// Ingests one signed envelope; returns status and effect.
pub async fn send(
    pool: &PgPool,
    text: &str,
    now: OffsetDateTime,
) -> (IngestStatus, Option<Effect>) {
    let event: Value = serde_json::from_str(text).unwrap();
    let event_id = Uuid::parse_str(event["event_id"].as_str().unwrap()).unwrap();
    let ingested = store::ingest(pool, event_id, text, now).await;
    (ingested.status, ingested.effect)
}

pub async fn applied(pool: &PgPool, text: &str, now: OffsetDateTime) -> Effect {
    let (status, effect) = send(pool, text, now).await;
    assert_eq!(status, IngestStatus::Applied);
    effect.unwrap()
}

pub async fn page(pool: &PgPool, recipient: Uuid, now: OffsetDateTime) -> Page {
    store::page(pool, recipient, 50, None, now).await.unwrap()
}

pub async fn badge(pool: &PgPool, recipient: Uuid, now: OffsetDateTime) -> i64 {
    store::count(pool, recipient, now)
        .await
        .unwrap()
        .unseen_group_count
}

pub async fn run(
    pool: &PgPool,
    recipient: Uuid,
    key: &str,
    command: Command,
    now: OffsetDateTime,
) -> Value {
    store::command(pool, recipient, key, &command, now)
        .await
        .unwrap()
}

pub async fn open_groups(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM notification_groups WHERE open")
        .fetch_one(pool)
        .await
        .unwrap()
}

pub fn app(pool: PgPool, scope: Scope) -> Router {
    router(NotificationState::new(pool, scope, Some(CALLER)))
        .layer(axum::Extension(ConnectInfo(Peer { uid: Some(CALLER) })))
}

pub async fn call(router: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[sqlx::test(migrations = false)]
async fn serve_is_not_ready_until_migrated_and_migrating_twice_is_a_no_op(pool: PgPool) {
    let router = app(pool.clone(), Scope::Ingest);
    let (status, body) = call(&router, "GET", "/health/ready", Value::Null).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["schema"], "pending_migrations");

    let first = migrate(&pool).await.unwrap();
    assert_eq!(
        first.applied,
        vec![20261128000000, 20261129000000, 20261202000000]
    );
    assert!(migrate(&pool).await.unwrap().applied.is_empty());
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);

    // Current schema, but the harness role is a superuser: serving with it is refused. Ready
    // with the provisioned runtime role is checked by `verify postgres` on the real binary.
    let (status, body) = call(&router, "GET", "/health/ready", Value::Null).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["checks"]["schema"], "current");
    assert_eq!(body["checks"]["role"], "privileged");
}

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn the_service_owns_only_inbox_tables(pool: PgPool) {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    // No work, comment, principal, endorsement or session table: those belong to App/Identity.
    assert_eq!(
        tables,
        [
            "_sqlx_migrations",
            "comment_fences",
            "group_members",
            "inbox_commands",
            "inboxes",
            "ingestion_receipts",
            "notification_groups",
            "recovery_changes",
            "recovery_state",
            "tham_marks",
            "ung_marks"
        ]
    );
}
