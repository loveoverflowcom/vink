//! Liveness answers "is the process serving HTTP"; readiness answers "can it serve Vink data":
//! the database answers and its history equals the embedded migrations (`pending_migrations`
//! and `diverged` are both not ready).

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use super::AppState;
use crate::identity::client::{Authority, CallError};
use crate::persistence::{SchemaState, role_is_privileged, schema_state};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
}

async fn live() -> Json<Value> {
    Json(json!({ "status": "live" }))
}

/// Integrations that do not exist yet are reported as such, never as healthy.
pub fn integrations(identity: &str) -> Value {
    json!({
        "ai_moderation": "not_implemented",
        "jobs": "none",
        "identity": identity,
    })
}

/// The Identity process as the App sees it. Informational: readiness does not depend on it,
/// because public reading must keep working while Identity is down.
async fn identity_status(state: &AppState) -> &'static str {
    let Some(client) = &state.identity_probe else {
        return "not_configured";
    };
    match tokio::time::timeout(Duration::from_secs(1), client.authority()).await {
        Ok(Ok(Authority::NotConfigured)) | Ok(Err(CallError::NoAuthority)) => "no_authority",
        Ok(Ok(Authority::Available)) => "available",
        Ok(Err(CallError::Unavailable(_))) | Err(_) => "unavailable",
    }
}

async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let pool = state.vink.store().pool();
    let probe = tokio::time::timeout(Duration::from_secs(5), async {
        Ok::<_, sqlx::Error>((schema_state(pool).await?, role_is_privileged(pool).await?))
    })
    .await;
    let (schema, privileged) = match probe {
        Ok(Ok((schema, privileged))) => (Some(schema), Some(privileged)),
        Ok(Err(_)) | Err(_) => (None, None),
    };
    let role_ok = privileged == Some(false) || !state.require_restricted_role;
    let ready = schema == Some(SchemaState::Current) && role_ok;
    let role = match privileged {
        Some(false) => "restricted",
        Some(true) => "privileged",
        None => "unknown",
    };
    let (database, schema) = match schema {
        Some(schema) => ("ok", schema.label()),
        None => ("unavailable", "unknown"),
    };
    let identity = identity_status(&state).await;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "status": if ready { "ready" } else { "not_ready" },
            "checks": { "database": database, "schema": schema, "role": role },
            "integrations": integrations(identity),
        })),
    )
}
