use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderValue, header},
    routing::get,
};
use tower_http::set_header::SetResponseHeaderLayer;

use super::{ApiError, AppState};
use crate::application::author_discovery::{AuthorPage, DiscoverAuthors};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/authors", get(search))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}

async fn search(
    State(state): State<AppState>,
    query: Result<Query<DiscoverAuthors>, QueryRejection>,
) -> Result<Json<AuthorPage>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::Validation {
        field: "query",
        message: "invalid author search parameters".into(),
    })?;
    Ok(Json(state.vink.store().discover_authors(query).await?))
}
