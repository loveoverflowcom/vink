use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use tower_http::set_header::SetResponseHeaderLayer;

use super::contract::{
    Draft, DraftCard, DraftList, Publication, SaveDraftRequest, SavedDraftResponse, WorkCard,
    WorkList,
};
use super::{Actor, ApiError, AppState};
use crate::application::SaveDraft;
use crate::domain::ids::WorkId;

pub fn routes(state: AppState) -> Router<AppState> {
    let drafts = Router::new()
        .route("/drafts", get(list_drafts).post(create_draft))
        .route("/drafts/{work_id}", get(get_draft).put(save_draft))
        // Account-scoped responses must never enter a shared cache.
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    Router::new()
        .route("/works", get(list_works))
        .route("/works/{work_id}/publication", get(get_publication))
        .merge(drafts)
        .fallback(|| async { ApiError::NotFound })
        .layer(middleware::from_fn_with_state(state, require_same_origin))
}

/// CSRF/Origin seam for unsafe methods: browsers always send `Origin` on PUT/POST, so a missing
/// or foreign origin is refused before any use case runs.
async fn require_same_origin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    let allowed = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        == Some(&*state.public_origin);
    if allowed {
        next.run(request).await
    } else {
        ApiError::Forbidden.into_response()
    }
}

fn work_id(raw: &str) -> Result<WorkId, ApiError> {
    // A malformed identifier cannot name an existing work.
    WorkId::parse(raw).ok_or(ApiError::NotFound)
}

async fn list_works(State(state): State<AppState>) -> Result<Json<WorkList>, ApiError> {
    let works = state.vink.public_works().await?;
    Ok(Json(WorkList {
        works: works.iter().map(WorkCard::from).collect(),
    }))
}

async fn get_publication(
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> Result<Json<Publication>, ApiError> {
    let work = state.vink.publication(work_id(&raw)?).await?;
    Ok(Json(Publication::from(&work)))
}

async fn list_drafts(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<Json<DraftList>, ApiError> {
    let drafts = state.vink.list_drafts(actor).await?;
    Ok(Json(DraftList {
        drafts: drafts.iter().map(DraftCard::from).collect(),
    }))
}

async fn create_draft(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<(StatusCode, Json<Draft>), ApiError> {
    let draft = state.vink.create_draft(actor).await?;
    Ok((StatusCode::CREATED, Json(Draft::from(&draft))))
}

async fn get_draft(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
) -> Result<Json<Draft>, ApiError> {
    let draft = state.vink.load_draft(actor, work_id(&raw)?).await?;
    Ok(Json(Draft::from(&draft)))
}

async fn save_draft(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<SaveDraftRequest>, JsonRejection>,
) -> Result<Json<SavedDraftResponse>, ApiError> {
    // Authentication is decided before the body is inspected.
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let Json(body) = body.map_err(|rejection| ApiError::Validation {
        field: "body",
        message: rejection.body_text(),
    })?;
    let saved = state
        .vink
        .save_draft(
            actor,
            SaveDraft {
                work_id: work_id(&raw)?,
                expected_version: body.expected_version,
                title: body.title,
                document: body.document,
            },
        )
        .await?;
    Ok(Json(SavedDraftResponse::from(&saved)))
}
