//! Private v1 saved memberships. No public representation depends on these DTOs.
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State, rejection::PathRejection},
    http::{HeaderMap, Method},
    routing::get,
};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{Actor, ApiError, AppState, contract::AuthorProjection, drafts::decode};
use crate::{
    application::saved_works::{ListItem, SavedWorksError},
    domain::{
        ids::{PrincipalId, WorkId},
        saved_work::{SaveError, SaveIntent, SavedGeneration, SavedWork},
    },
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/saved-works", get(list))
        .route("/me/saved-works/{work_id}", get(state).put(set).delete(set))
}

#[derive(Serialize)]
struct Membership {
    saved: bool,
    generation: i64,
}

impl From<SavedWork> for Membership {
    fn from(pair: SavedWork) -> Self {
        Self {
            saved: pair.saved_at().is_some(),
            generation: pair.generation().get(),
        }
    }
}

#[derive(Serialize)]
struct Page {
    items: Vec<Item>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Item {
    Readable {
        work_id: Uuid,
        generation: i64,
        #[serde(with = "time::serde::rfc3339")]
        saved_at: OffsetDateTime,
        title: String,
        form: Option<String>,
        author: AuthorProjection,
    },
    Hidden {
        work_id: Uuid,
        generation: i64,
    },
    AuthorDeleted {
        work_id: Uuid,
        generation: i64,
    },
}

impl From<ListItem> for Item {
    fn from(item: ListItem) -> Self {
        match item {
            ListItem::Readable {
                work_id,
                generation,
                saved_at,
                title,
                form,
                author,
            } => Self::Readable {
                work_id,
                generation,
                saved_at,
                title,
                form,
                author: AuthorProjection::from(&author),
            },
            ListItem::Hidden {
                work_id,
                generation,
            } => Self::Hidden {
                work_id,
                generation,
            },
            ListItem::AuthorDeleted {
                work_id,
                generation,
            } => Self::AuthorDeleted {
                work_id,
                generation,
            },
        }
    }
}

fn invalid(field: &'static str) -> ApiError {
    ApiError::Validation {
        field,
        message: format!("invalid {field}"),
    }
}

impl From<SavedWorksError> for ApiError {
    fn from(error: SavedWorksError) -> Self {
        match error {
            SavedWorksError::Store(e) => e.into(),
            SavedWorksError::InvalidCursor => invalid("cursor"),
            SavedWorksError::WrongPrincipal => Self::Forbidden,
            SavedWorksError::Decision(e) => match e {
                SaveError::Suspended => Self::PrincipalSuspended,
                SaveError::NotReadable => Self::NotFound,
                SaveError::Stale { current } => Self::SavedWorkConflict(current.get()),
                SaveError::LimitReached => Self::SavedWorksLimitReached,
                SaveError::WrongKey | SaveError::GenerationExhausted => Self::Internal,
            },
        }
    }
}

fn work_id(path: Result<Path<String>, PathRejection>) -> Result<WorkId, ApiError> {
    path.ok()
        .and_then(|Path(raw)| WorkId::parse(&raw))
        .ok_or(ApiError::NotFound)
}

fn parameter(raw: RawQuery, allowed: &'static str) -> Result<Option<String>, ApiError> {
    let raw = raw.0.as_deref().unwrap_or_default();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.len() > 512 {
        return Err(invalid("query"));
    }
    let mut result = None;
    for part in raw.split('&') {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        if allowed.is_empty() || decode(key)? != allowed {
            return Err(invalid("query"));
        }
        if result.is_some() || value.is_empty() {
            return Err(invalid(allowed));
        }
        result = Some(decode(value).map_err(|_| invalid(allowed))?);
    }
    Ok(result)
}

async fn state(
    State(app): State<AppState>,
    Actor(actor): Actor,
    path: Result<Path<String>, PathRejection>,
    raw: RawQuery,
) -> Result<Json<Membership>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    parameter(raw, "")?;
    Ok(Json(
        app.vink.saved_work(actor, work_id(path)?).await?.into(),
    ))
}

async fn set(
    State(app): State<AppState>,
    Actor(actor): Actor,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    method: Method,
    raw: RawQuery,
) -> Result<Json<Membership>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let mut values = headers.get_all("x-vink-expected-principal").iter();
    let expected = values
        .next()
        .and_then(|v| v.to_str().ok())
        .and_then(PrincipalId::parse)
        .filter(|_| values.next().is_none())
        .ok_or_else(|| invalid("expected_principal"))?;
    let generation = parameter(raw, "expected_generation")?
        .and_then(|s| s.parse().ok())
        .and_then(SavedGeneration::new)
        .ok_or_else(|| invalid("expected_generation"))?;
    let intent = if method == Method::PUT {
        SaveIntent::Saved
    } else {
        SaveIntent::NotSaved
    };
    Ok(Json(
        app.vink
            .set_saved_work(actor, expected, work_id(path)?, intent, generation)
            .await?
            .into(),
    ))
}

async fn list(
    State(app): State<AppState>,
    Actor(actor): Actor,
    raw: RawQuery,
) -> Result<Json<Page>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let cursor = parameter(raw, "cursor")?;
    let page = app.vink.saved_works(actor, cursor.as_deref()).await?;
    Ok(Json(Page {
        items: page.items.into_iter().map(Item::from).collect(),
        next_cursor: page.next_cursor,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::profile::AuthorAttribution;
    use serde_json::Value;
    use time::macros::datetime;

    #[test]
    fn saved_works_contract_fixtures() {
        let id = |s| Uuid::parse_str(s).unwrap();
        let page = Page {
            items: vec![
                ListItem::Readable {
                    work_id: id("33333333-3333-4333-8333-333333333333"),
                    generation: 2,
                    title: "Mưa trên phố cũ".into(),
                    form: Some("Thơ".into()),
                    saved_at: datetime!(2026-11-25 08:00 UTC),
                    author: AuthorAttribution::project(
                        "Nhật Ánh".into(),
                        Some("ngay-nang".into()),
                        false,
                    ),
                },
                ListItem::Hidden {
                    work_id: id("55555555-5555-4555-8555-555555555555"),
                    generation: 4,
                },
                ListItem::AuthorDeleted {
                    work_id: id("66666666-6666-4666-8666-666666666666"),
                    generation: 2,
                },
            ]
            .into_iter()
            .map(Item::from)
            .collect(),
            next_cursor: None,
        };
        for (file, actual) in [
            (
                "saved-work.state.json",
                serde_json::to_value(Membership {
                    saved: true,
                    generation: 2,
                })
                .unwrap(),
            ),
            ("saved-works.page.json", serde_json::to_value(page).unwrap()),
            (
                "error.saved-work-conflict.json",
                serde_json::to_value(ApiError::SavedWorkConflict(3).envelope()).unwrap(),
            ),
            (
                "error.saved-works-limit.json",
                serde_json::to_value(ApiError::SavedWorksLimitReached.envelope()).unwrap(),
            ),
        ] {
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("contracts/http/v1/fixtures")
                    .join(file),
            )
            .unwrap();
            assert_eq!(
                actual,
                serde_json::from_slice::<Value>(&bytes).unwrap(),
                "{file}"
            );
        }
    }
}
