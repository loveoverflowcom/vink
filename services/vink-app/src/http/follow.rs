//! HTTP v1 adapter for the Social Follow Graph (#294).

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, RawQuery, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::set_header::SetResponseHeaderLayer;
use uuid::Uuid;

use super::conditional::{self, PUBLIC_ERROR_CACHE_CONTROL};
use super::{Actor, ApiError, AppState, PrivateSession};
use crate::application::follow::{FollowError, PublicRelationshipList, SetFollowInput};
use crate::domain::follow::{FollowDirection, MAX_MEMBERSHIP_BATCH};
use crate::domain::handle;

const FOLLOW_BODY_LIMIT: usize = 4096;
const MAX_CURSOR_QUERY_LEN: usize = 1024;
const MAX_MEMBERSHIP_QUERY_LEN: usize = 4096;

pub(super) fn routes() -> Router<AppState> {
    let public = Router::new()
        .route("/authors/{name}/followers", get(public_followers))
        .route("/authors/{name}/following", get(public_following))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static(PUBLIC_ERROR_CACHE_CONTROL),
        ));

    let private = Router::new()
        .route(
            "/authors/{name}/relationship/me",
            get(viewer_relationship).put(set_follow),
        )
        .route("/me/follows/receipts/{command_id}", get(receipt))
        .route("/me/follows/membership", get(membership))
        .route("/me/relationships/followers", get(own_followers))
        .route("/me/relationships/following", get(own_following))
        .layer(DefaultBodyLimit::max(FOLLOW_BODY_LIMIT))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));

    Router::new().merge(public).merge(private)
}

fn parse_cursor_query(RawQuery(raw): RawQuery) -> Result<Option<String>, ApiError> {
    let invalid = || ApiError::Validation {
        field: "query",
        message: "invalid cursor query".into(),
    };
    let raw = raw.unwrap_or_default();
    if raw.len() > MAX_CURSOR_QUERY_LEN {
        return Err(invalid());
    }
    if raw.is_empty() {
        return Ok(None);
    }
    let mut cursor = None;
    for part in raw.split('&') {
        let (key, value) = part.split_once('=').ok_or_else(invalid)?;
        let key = super::drafts::decode(key)?;
        if key != "cursor" || cursor.is_some() {
            return Err(invalid());
        }
        let decoded = super::drafts::decode(value)?;
        cursor = Some(decoded);
    }
    Ok(cursor)
}

fn parse_membership_query(RawQuery(raw): RawQuery) -> Result<Vec<String>, ApiError> {
    let invalid_query = || ApiError::Validation {
        field: "query",
        message: "invalid membership query".into(),
    };
    let invalid_handles = || ApiError::Validation {
        field: "handles",
        message: "handles must contain 1..=50 valid author handles".into(),
    };
    let Some(raw) = raw else {
        return Err(invalid_handles());
    };
    if raw.is_empty() || raw.len() > MAX_MEMBERSHIP_QUERY_LEN {
        return Err(invalid_query());
    }
    let mut handles_param = None;
    for part in raw.split('&') {
        let (key, value) = part.split_once('=').ok_or_else(invalid_query)?;
        let key = super::drafts::decode(key)?;
        if key != "handles" || handles_param.is_some() {
            return Err(invalid_query());
        }
        handles_param = Some(super::drafts::decode(value)?);
    }
    let Some(list) = handles_param else {
        return Err(invalid_handles());
    };
    if list.is_empty() {
        return Err(invalid_handles());
    }
    let mut out = Vec::new();
    for raw_handle in list.split(',') {
        if raw_handle.is_empty() || !handle::valid(raw_handle) {
            return Err(invalid_handles());
        }
        out.push(raw_handle.to_owned());
        if out.len() > MAX_MEMBERSHIP_BATCH {
            return Err(invalid_handles());
        }
    }
    Ok(out)
}

async fn public_followers(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    raw: RawQuery,
) -> Result<Response, ApiError> {
    public_list(state, name, FollowDirection::Followers, headers, raw).await
}

async fn public_following(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    raw: RawQuery,
) -> Result<Response, ApiError> {
    public_list(state, name, FollowDirection::Following, headers, raw).await
}

async fn public_list(
    state: AppState,
    name: String,
    direction: FollowDirection,
    headers: HeaderMap,
    raw: RawQuery,
) -> Result<Response, ApiError> {
    let cursor = parse_cursor_query(raw).map_err(|_| FollowError::Invalid("query"));
    let list = state
        .vink
        .store()
        .public_relationships(&name, direction, cursor)
        .await?;
    match list {
        PublicRelationshipList::Page { .. } => conditional::respond(&headers, &list),
        PublicRelationshipList::Alias { .. } | PublicRelationshipList::Restart { .. } => {
            Ok(Json(list).into_response())
        }
    }
}

async fn viewer_relationship(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(name): Path<String>,
) -> Result<Json<crate::application::follow::ViewerRelationship>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    Ok(Json(
        state.vink.store().viewer_relationship(actor, &name).await?,
    ))
}

async fn set_follow(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(name): Path<String>,
    body: Result<Json<SetFollowInput>, JsonRejection>,
) -> Result<Json<crate::application::follow::SetFollowResponse>, ApiError> {
    // Session precedes body validation so anonymous callers learn nothing about payload rules.
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let input = super::api::json_body(body)?;
    Ok(Json(
        state.vink.store().set_follow(actor, &name, input).await?,
    ))
}

async fn receipt(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw_command_id): Path<String>,
) -> Result<Json<crate::application::follow::FollowReceiptView>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let command_id = Uuid::parse_str(&raw_command_id).map_err(|_| ApiError::Validation {
        field: "command_id",
        message: "invalid command_id UUID".into(),
    })?;
    Ok(Json(
        state.vink.store().follow_receipt(actor, command_id).await?,
    ))
}

async fn membership(
    State(state): State<AppState>,
    Actor(actor): Actor,
    raw: RawQuery,
) -> Result<Json<crate::application::follow::MembershipBatch>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let handles = parse_membership_query(raw)?;
    Ok(Json(
        state
            .vink
            .store()
            .batch_membership_by_handles(actor, &handles)
            .await?,
    ))
}

async fn own_followers(
    State(state): State<AppState>,
    session: PrivateSession,
    raw: RawQuery,
) -> Result<Json<crate::application::follow::OwnRelationshipList>, ApiError> {
    own_list(state, session, FollowDirection::Followers, raw).await
}

async fn own_following(
    State(state): State<AppState>,
    session: PrivateSession,
    raw: RawQuery,
) -> Result<Json<crate::application::follow::OwnRelationshipList>, ApiError> {
    own_list(state, session, FollowDirection::Following, raw).await
}

async fn own_list(
    state: AppState,
    session: PrivateSession,
    direction: FollowDirection,
    raw: RawQuery,
) -> Result<Json<crate::application::follow::OwnRelationshipList>, ApiError> {
    let cursor = parse_cursor_query(raw)?;
    Ok(Json(
        state
            .vink
            .store()
            .own_relationships(session.actor, direction, session.digest, cursor.as_deref())
            .await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{parse_cursor_query, parse_membership_query};
    use axum::extract::RawQuery;

    #[test]
    fn cursor_query_accepts_only_single_cursor() {
        assert_eq!(parse_cursor_query(RawQuery(None)).unwrap(), None);
        assert_eq!(
            parse_cursor_query(RawQuery(Some(String::new()))).unwrap(),
            None
        );
        assert_eq!(
            parse_cursor_query(RawQuery(Some("cursor=abc".into()))).unwrap(),
            Some("abc".into())
        );
        assert!(parse_cursor_query(RawQuery(Some("foo=1".into()))).is_err());
        assert!(parse_cursor_query(RawQuery(Some("cursor=a&cursor=b".into()))).is_err());
        assert!(parse_cursor_query(RawQuery(Some("cursor".into()))).is_err());
    }

    #[test]
    fn membership_query_validates_strictly_before_store() {
        assert_eq!(
            parse_membership_query(RawQuery(Some("handles=an-may,binh-yen".into()))).unwrap(),
            vec!["an-may".to_string(), "binh-yen".to_string()]
        );
        for bad in [
            None,
            Some(""),
            Some("handles="),
            Some("handles=an-may,,binh-yen"),
            Some("handles=BAD!"),
            Some("handles=an-may&extra=1"),
            Some("handles=a&handles=b"),
        ] {
            assert!(
                parse_membership_query(RawQuery(bad.map(str::to_owned))).is_err(),
                "{bad:?}"
            );
        }
        let fifty: String = (0..50)
            .map(|i| format!("tac-gia-{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse_membership_query(RawQuery(Some(format!("handles={fifty}"))))
                .unwrap()
                .len(),
            50
        );
        let fifty_one: String = (0..51)
            .map(|i| format!("tac-gia-{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse_membership_query(RawQuery(Some(format!("handles={fifty_one}")))).is_err());
    }
}
