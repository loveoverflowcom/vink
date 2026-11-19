//! Session-scoped commands and public, actor-free resolution.
use super::{Actor, ApiError, AppState};
use crate::application::handle::*;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderValue, header},
    routing::get,
};
use tower_http::set_header::SetResponseHeaderLayer;
use uuid::Uuid;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/handle", get(own).post(write))
        .route("/me/handle/receipts/{id}", get(receipt))
        .layer(DefaultBodyLimit::max(2048))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}

impl From<HandleError> for ApiError {
    fn from(err: HandleError) -> Self {
        match err {
            HandleError::Missing => Self::NotFound,
            HandleError::WrongOwner => Self::Forbidden,
            HandleError::Invalid => Self::Validation {
                field: "handle",
                message: "invalid author handle".into(),
            },
            HandleError::Taken => Self::HandleRefused("handle_taken"),
            HandleError::OwnAlias => Self::HandleRefused("handle_alias_reserved"),
            HandleError::Exhausted => Self::HandleRefused("handle_quota_exhausted"),
            HandleError::Stale(current_version) => Self::VersionConflict { current_version },
            HandleError::KeyReused => Self::HandleRefused("handle_command_reused"),
            HandleError::Store(error) => Self::from(error),
        }
    }
}

async fn own(State(s): State<AppState>, Actor(actor): Actor) -> Result<Json<OwnHandle>, ApiError> {
    Ok(Json(
        s.vink
            .store()
            .own_handle(actor.ok_or(ApiError::Unauthenticated)?)
            .await?,
    ))
}
async fn write(
    State(s): State<AppState>,
    Actor(actor): Actor,
    Json(cmd): Json<HandleCommand>,
) -> Result<Json<HandleReceipt>, ApiError> {
    Ok(Json(
        s.vink
            .store()
            .set_handle(actor.ok_or(ApiError::Unauthenticated)?, &cmd)
            .await?,
    ))
}
async fn receipt(
    State(s): State<AppState>,
    Actor(actor): Actor,
    Path(id): Path<Uuid>,
) -> Result<Json<HandleReceipt>, ApiError> {
    Ok(Json(
        s.vink
            .store()
            .handle_receipt(actor.ok_or(ApiError::Unauthenticated)?, id)
            .await?,
    ))
}
