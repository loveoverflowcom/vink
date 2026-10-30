//! Liveness answers "is the process serving HTTP"; readiness answers "can it serve Vink data".

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use super::AppState;
use crate::persistence::{SchemaState, schema_state};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
}

async fn live() -> Json<Value> {
    Json(json!({ "status": "live" }))
}

/// Integrations that do not exist yet are reported as such, never as healthy.
pub fn integrations() -> Value {
    json!({
        "discourse": "not_implemented",
        "ai_moderation": "not_implemented",
        "jobs": "none",
    })
}

async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let pool = state.vink.store().pool();
    let probe = tokio::time::timeout(Duration::from_secs(5), schema_state(pool)).await;
    let (database, schema) = match probe {
        Ok(Ok(SchemaState::Current)) => ("ok", "current"),
        Ok(Ok(SchemaState::Pending)) => ("ok", "pending_migrations"),
        Ok(Err(_)) | Err(_) => ("unavailable", "unknown"),
    };
    let ready = database == "ok" && schema == "current";
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "status": if ready { "ready" } else { "not_ready" },
            "checks": { "database": database, "schema": schema },
            "integrations": integrations(),
        })),
    )
}
