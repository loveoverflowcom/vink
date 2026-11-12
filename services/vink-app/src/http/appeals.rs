//! HTTP contract for work appeals. All responses are private/no-store, including refusals.
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderValue, header},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tower_http::set_header::SetResponseHeaderLayer;

use super::{Actor, ApiError, AppState};
use crate::domain::document::DocumentData;
use crate::domain::ids::{AppealId, RevisionId, WorkId};
use crate::domain::moderation::appeal::AppealVerdict;
use crate::domain::moderation::{DecisionReason, DecisionVersion};
use crate::persistence::appeals::{self, AppealRecord};

#[derive(Debug, Serialize)]
pub struct AppealDto {
    id: String,
    work_id: String,
    revision_id: String,
    decision_version: i64,
    state: String,
    notice: &'static str,
    resolution_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}
impl From<AppealRecord> for AppealDto {
    fn from(a: AppealRecord) -> Self {
        Self {
            id: a.id.to_string(),
            work_id: a.work_id.to_string(),
            revision_id: a.revision_id.to_string(),
            decision_version: a.decision_version,
            state: a.state,
            notice: "not_sent",
            resolution_reason: a.resolution_reason,
            created_at: a.created_at,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRequest {
    expected_decision_version: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConcludeRequest {
    expected_decision_version: i64,
    verdict: String,
    reason: String,
}
#[derive(Serialize)]
struct FiledDto {
    appeal: AppealDto,
    replayed: bool,
}
#[derive(Serialize)]
struct DetailDto {
    appeal: AppealDto,
    title: String,
    document: DocumentData,
}

fn version(n: i64) -> Result<DecisionVersion, ApiError> {
    DecisionVersion::new(n).ok_or(ApiError::Validation {
        field: "expected_decision_version",
        message: "must be positive".into(),
    })
}
fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    body.map(|Json(body)| body)
        .map_err(|_| ApiError::Validation {
            field: "body",
            message: "invalid JSON request".into(),
        })
}
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/works/{work}/revisions/{revision}/appeals", post(file))
        .route("/works/{work}/appeals", get(own))
        .route("/moderation/appeals", get(queue))
        .route("/moderation/appeals/{id}", get(detail))
        .route("/moderation/appeals/{id}/conclude", post(conclude))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}
async fn file(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path((work, revision)): Path<(String, String)>,
    request: Result<Json<FileRequest>, JsonRejection>,
) -> Result<Json<FiledDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let request = body(request)?;
    let (appeal, replayed) = appeals::file(
        state.vink.store().pool(),
        actor,
        WorkId::parse(&work).ok_or(ApiError::NotFound)?,
        RevisionId::parse(&revision).ok_or(ApiError::NotFound)?,
        version(request.expected_decision_version)?,
    )
    .await?;
    Ok(Json(FiledDto {
        appeal: appeal.into(),
        replayed,
    }))
}
async fn own(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(work): Path<String>,
) -> Result<Json<Vec<AppealDto>>, ApiError> {
    let records = appeals::list_own(
        state.vink.store().pool(),
        actor.ok_or(ApiError::Unauthenticated)?,
        WorkId::parse(&work).ok_or(ApiError::NotFound)?,
    )
    .await?;
    Ok(Json(records.into_iter().map(Into::into).collect()))
}
async fn queue(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<Json<Vec<AppealDto>>, ApiError> {
    let records = appeals::list_open(
        state.vink.store().pool(),
        actor.ok_or(ApiError::Unauthenticated)?,
    )
    .await?;
    Ok(Json(records.into_iter().map(Into::into).collect()))
}
async fn detail(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
) -> Result<Json<DetailDto>, ApiError> {
    let (appeal, title, document) = appeals::detail(
        state.vink.store().pool(),
        actor.ok_or(ApiError::Unauthenticated)?,
        AppealId::parse(&raw).ok_or(ApiError::NotFound)?,
    )
    .await?;
    Ok(Json(DetailDto {
        appeal: appeal.into(),
        title,
        document,
    }))
}
async fn conclude(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    request: Result<Json<ConcludeRequest>, JsonRejection>,
) -> Result<Json<AppealDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let request = body(request)?;
    let verdict = match request.verdict.as_str() {
        "uphold" => AppealVerdict::Uphold,
        "reverse" => AppealVerdict::Reverse,
        _ => {
            return Err(ApiError::Validation {
                field: "verdict",
                message: "expected uphold or reverse".into(),
            });
        }
    };
    let reason = DecisionReason::parse(&request.reason).map_err(|e| ApiError::Validation {
        field: "reason",
        message: e.to_string(),
    })?;
    Ok(Json(
        appeals::conclude(
            state.vink.store().pool(),
            actor,
            AppealId::parse(&raw).ok_or(ApiError::NotFound)?,
            version(request.expected_decision_version)?,
            verdict,
            reason,
        )
        .await?
        .into(),
    ))
}
