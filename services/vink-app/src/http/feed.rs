//! Following Feed has one fixed sort/query and an authenticated, cache-free HTTP boundary.
use axum::{
    Json, Router,
    extract::{RawQuery, State},
    http::{HeaderMap, HeaderValue, header},
    response::IntoResponse,
    routing::get,
};
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha2::Sha256;
use tower_http::set_header::SetResponseHeaderLayer;
use uuid::Uuid;

use super::{ApiError, AppState, PrivateSession, contract::WorkCard};
use crate::persistence::feed::{FEED_CHECK_LIMIT, FeedRead, FeedReadout};

const RETURN_SCOPE: &str = "x-vink-feed-return-scope";

// A navigation binding. It exposes neither the private session digest nor a cursor, and
// cannot authorize any read. A new credential for the same principal gets a different value.
fn return_scope(session: &PrivateSession) -> HeaderValue {
    let mut mac = Hmac::<Sha256>::new_from_slice(&session.digest)
        .expect("HMAC supports a 32-byte session key");
    mac.update(b"vink:following-feed:return-scope:v1");
    mac.update(session.actor.as_uuid().as_bytes());
    let signature = mac.finalize().into_bytes();
    let value: String = signature.iter().map(|byte| format!("{byte:02x}")).collect();
    HeaderValue::from_str(&value).expect("hex is a valid header value")
}

pub(super) fn routes(enabled: bool) -> Router<AppState> {
    let read = if enabled {
        get(feed)
    } else {
        get(|| async { ApiError::NotFound })
    };
    Router::new()
        .route("/me/following-feed", read)
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        ))
}

#[derive(Debug, Default)]
struct Query {
    cursor: Option<String>,
    around: Option<Uuid>,
    seen: Option<String>,
}

impl Query {
    fn seen_ids(&self) -> Result<Vec<Uuid>, ApiError> {
        let Some(ids) = self.seen.as_deref() else {
            return Ok(Vec::new());
        };
        let invalid = || query_error("seen");
        let mut seen = Vec::new();
        for raw in ids.split(',') {
            let id = Uuid::try_parse(raw).map_err(|_| invalid())?;
            if seen.len() == FEED_CHECK_LIMIT || seen.contains(&id) {
                return Err(invalid());
            }
            seen.push(id);
        }
        Ok(seen)
    }

    fn read<'a>(&'a self, seen: &'a [Uuid]) -> Result<FeedRead<'a>, ApiError> {
        match (self.cursor.as_deref(), self.around, seen.is_empty()) {
            (None, around, true) => Ok(FeedRead::First { around }),
            (None, None, false) => Ok(FeedRead::Check(seen)),
            (Some(cursor), None, _) => Ok(FeedRead::Next { cursor, seen }),
            _ => Err(query_error("query")),
        }
    }
}

fn query_error(field: &'static str) -> ApiError {
    ApiError::Validation {
        field,
        message: format!("invalid feed {field}"),
    }
}

fn parse_query(RawQuery(raw): RawQuery) -> Result<Query, ApiError> {
    let raw = raw.as_deref().unwrap_or_default();
    if raw.len() > 2304 {
        return Err(query_error("query"));
    }
    let mut query = Query::default();
    for pair in raw.split('&').filter(|_| !raw.is_empty()) {
        let invalid = || query_error("query");
        let (key, value) = pair.split_once('=').ok_or_else(invalid)?;
        let key = super::drafts::decode(key)?;
        let value = super::drafts::decode(value)?;
        match key.as_str() {
            "cursor" if query.cursor.is_none() => query.cursor = Some(value),
            "around" if query.around.is_none() => {
                query.around = Some(Uuid::try_parse(&value).map_err(|_| invalid())?);
            }
            "seen" if query.seen.is_none() => query.seen = Some(value),
            _ => return Err(invalid()),
        }
    }
    Ok(query)
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum FeedResponse {
    Page {
        viewer: Uuid,
        has_following: bool,
        works: Vec<WorkCard>,
        retained: Vec<WorkCard>,
        next_cursor: Option<String>,
    },
    Checked {
        viewer: Uuid,
        retained: Vec<WorkCard>,
    },
    Restart,
}

async fn feed(
    State(state): State<AppState>,
    session: PrivateSession,
    headers: HeaderMap,
    raw: RawQuery,
) -> Result<impl IntoResponse, ApiError> {
    let query = parse_query(raw)?;
    let seen = query.seen_ids()?;
    let scope = return_scope(&session);
    let read = match query.read(&seen)? {
        FeedRead::First { around: Some(_) }
            if headers.get(RETURN_SCOPE).is_some_and(|hint| hint != scope) =>
        {
            FeedRead::First { around: None }
        }
        read => read,
    };
    let result = state
        .vink
        .store()
        .following_feed(session.actor, session.digest, read)
        .await?;
    let viewer = session.actor.as_uuid();
    let body = match result {
        FeedReadout::Page {
            has_following,
            works,
            retained,
            next_cursor,
        } => FeedResponse::Page {
            viewer,
            has_following,
            works: works.iter().map(WorkCard::from).collect(),
            retained: retained.iter().map(WorkCard::from).collect(),
            next_cursor,
        },
        FeedReadout::Checked(retained) => FeedResponse::Checked {
            viewer,
            retained: retained.iter().map(WorkCard::from).collect(),
        },
        FeedReadout::Restart => FeedResponse::Restart,
    };
    Ok(([(RETURN_SCOPE, scope)], Json(body)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fixed_feed_query_is_accepted() {
        for raw in [
            "q=Vink",
            "tag=tho",
            "mode=works",
            "limit=12",
            "cursor=a&cursor=b",
        ] {
            assert!(parse_query(RawQuery(Some(raw.into()))).is_err(), "{raw}");
        }
        let id = Uuid::from_u128(1);
        let query = parse_query(RawQuery(Some(format!("around={id}&cursor=a")))).unwrap();
        assert!(query.read(&[]).is_err());
        let invalid = parse_query(RawQuery(Some(format!("seen={id},{id}")))).unwrap();
        assert!(invalid.seen_ids().is_err());
    }
}
