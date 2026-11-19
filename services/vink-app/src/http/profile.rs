//! Own profile is session-only. The public handler resolves policy before its validator.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use tower_http::set_header::SetResponseHeaderLayer;

use super::{Actor, ApiError, AppState, conditional};
use crate::application::handle::Resolution;
use crate::application::profile::{OwnProfile, ProfileError, SaveProfile};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/profile", get(own).put(save))
        .route("/authors/{name}", get(resolve))
        .layer(DefaultBodyLimit::max(8192))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}

impl From<ProfileError> for ApiError {
    fn from(error: ProfileError) -> Self {
        match error {
            ProfileError::Store(error) => Self::from(error),
            ProfileError::Missing => Self::NotFound,
            ProfileError::WrongOwner => Self::Forbidden,
            ProfileError::Invalid(field) => Self::Validation {
                field,
                message: format!("invalid profile field: {field}"),
            },
            ProfileError::DisclosurePending => Self::Validation {
                field: "policies",
                message: "profile disclosure requires a supported confirmation".into(),
            },
            ProfileError::Stale(current_version) => Self::VersionConflict { current_version },
        }
    }
}

async fn own(State(s): State<AppState>, Actor(actor): Actor) -> Result<Json<OwnProfile>, ApiError> {
    Ok(Json(
        s.vink
            .store()
            .own_profile(actor.ok_or(ApiError::Unauthenticated)?)
            .await?,
    ))
}

async fn save(
    State(s): State<AppState>,
    Actor(actor): Actor,
    body: Result<Json<SaveProfile>, JsonRejection>,
) -> Result<Json<OwnProfile>, ApiError> {
    // Session first: an anonymous request learns nothing about body validation. Malformed or
    // unknown fields (such as `date_of_birth`) use the contract's `validation_failed` shape.
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let command = super::api::json_body(body)?;
    Ok(Json(s.vink.store().save_profile(actor, command).await?))
}

#[derive(Deserialize)]
struct ProfileQuery {
    cursor: Option<String>,
}

async fn resolve(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ProfileQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let result = s
        .vink
        .store()
        .resolve_handle(&name, query.cursor.as_deref())
        .await?;
    match result {
        Resolution::Current { .. } => conditional::respond(&headers, &result),
        _ => Ok(Json(result).into_response()),
    }
}
