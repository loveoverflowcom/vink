use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use tower_http::set_header::SetResponseHeaderLayer;

use super::contract::{
    CommentsSettings, CommentsSettingsRequest, DiscussionPage, Draft, DraftCard, DraftList,
    ModerationDecisionRequest, ModerationDecisionResponse, ModerationQueueResponse, PageQuery,
    Publication, RemovedCommentResponse, ReplyList, SaveDraftRequest, SavedDraftResponse,
    SubmitCommentRequest, SubmittedCommentResponse, WorkCard, WorkList,
};
use super::{Actor, ApiError, AppState};
use crate::application::SaveDraft;
use crate::application::discussion::{ModerateComment, SetCommentsMode, SubmitComment};
use crate::domain::discussion::settings::CommentsMode;
use crate::domain::ids::{CommentId, WorkId};

pub fn routes(state: AppState) -> Router<AppState> {
    let drafts = Router::new()
        .route("/drafts", get(list_drafts).post(create_draft))
        .route("/drafts/{work_id}", get(get_draft).put(save_draft))
        // Account-scoped responses must never enter a shared cache.
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    // Vink-owned discussion (D2T-02b). Responses carry viewer-specific state (own pending
    // comments, `can_remove`), so none may enter a shared cache.
    let discussion = Router::new()
        .route("/works/{work_id}/discussion", get(get_discussion))
        .route(
            "/works/{work_id}/discussion/roots/{root_id}/replies",
            get(get_replies),
        )
        .route("/works/{work_id}/discussion/comments", post(submit_comment))
        .route(
            "/works/{work_id}/discussion/comments/{comment_id}",
            delete(remove_comment),
        )
        .route("/works/{work_id}/discussion/settings", put(set_settings))
        // Moderation (D2T-02c): authority is the actor's active grant, rechecked at commit.
        .route("/moderation/comments", get(get_moderation_queue))
        .route(
            "/moderation/comments/{comment_id}/decisions",
            post(decide_comment),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    Router::new()
        .route("/works", get(list_works))
        .route("/works/{work_id}/publication", get(get_publication))
        .merge(drafts)
        .merge(discussion)
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

fn comment_id(raw: &str) -> Result<CommentId, ApiError> {
    CommentId::parse(raw).ok_or(ApiError::NotFound)
}

fn page_query(query: Result<Query<PageQuery>, QueryRejection>) -> Result<PageQuery, ApiError> {
    query
        .map(|Query(query)| query)
        .map_err(|rejection| ApiError::Validation {
            field: "query",
            message: rejection.body_text(),
        })
}

fn json_body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    body.map(|Json(body)| body)
        .map_err(|rejection| ApiError::Validation {
            field: "body",
            message: rejection.body_text(),
        })
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

async fn get_discussion(
    State(state): State<AppState>,
    Actor(viewer): Actor,
    Path(raw): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<DiscussionPage>, ApiError> {
    let work_id = work_id(&raw)?;
    let query = page_query(query)?;
    let page = state
        .vink
        .discussion(viewer, work_id, query.cursor.as_deref(), query.limit)
        .await?;
    Ok(Json(DiscussionPage::new(work_id, &page, viewer)))
}

async fn get_replies(
    State(state): State<AppState>,
    Actor(viewer): Actor,
    Path((raw_work, raw_root)): Path<(String, String)>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<ReplyList>, ApiError> {
    let work_id = work_id(&raw_work)?;
    let root_id = comment_id(&raw_root)?;
    let query = page_query(query)?;
    let page = state
        .vink
        .replies(
            viewer,
            work_id,
            root_id,
            query.cursor.as_deref(),
            query.limit,
        )
        .await?;
    Ok(Json(ReplyList::new(&page, viewer)))
}

async fn submit_comment(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<SubmitCommentRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<SubmittedCommentResponse>), ApiError> {
    // Authentication is decided before the body is inspected.
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw)?;
    let body = json_body(body)?;
    let target = body
        .target_id
        .as_deref()
        .map(|raw| {
            CommentId::parse(raw).ok_or_else(|| ApiError::Validation {
                field: "target_id",
                message: "target_id must be a comment ID or null".to_string(),
            })
        })
        .transpose()?;
    let submitted = state
        .vink
        .submit_comment(
            actor,
            SubmitComment {
                work_id,
                target,
                body: body.body,
                command_key: body.command_key,
            },
        )
        .await?;
    // A replay reports the earlier commit; only a new comment is `201 Created`.
    let status = if submitted.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(SubmittedCommentResponse::from(&submitted))))
}

async fn remove_comment(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path((raw_work, raw_comment)): Path<(String, String)>,
) -> Result<Json<RemovedCommentResponse>, ApiError> {
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let comment_id = comment_id(&raw_comment)?;
    let status = state
        .vink
        .remove_own_comment(actor, work_id(&raw_work)?, comment_id)
        .await?;
    Ok(Json(RemovedCommentResponse::new(comment_id, status)))
}

async fn set_settings(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<CommentsSettingsRequest>, JsonRejection>,
) -> Result<Json<CommentsSettings>, ApiError> {
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw)?;
    let body = json_body(body)?;
    let settings = state
        .vink
        .set_comments_mode(
            actor,
            SetCommentsMode {
                work_id,
                expected_generation: body.expected_generation,
                mode: if body.comments_open {
                    CommentsMode::Open
                } else {
                    CommentsMode::Closed
                },
            },
        )
        .await?;
    Ok(Json(CommentsSettings::from(settings)))
}

async fn get_moderation_queue(
    State(state): State<AppState>,
    Actor(actor): Actor,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<ModerationQueueResponse>, ApiError> {
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let query = page_query(query)?;
    let queue = state
        .vink
        .moderation_queue(actor, query.cursor.as_deref(), query.limit)
        .await?;
    Ok(Json(ModerationQueueResponse::from(&queue)))
}

async fn decide_comment(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<ModerationDecisionRequest>, JsonRejection>,
) -> Result<Json<ModerationDecisionResponse>, ApiError> {
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let comment_id = comment_id(&raw)?;
    let body = json_body(body)?;
    let decided = state
        .vink
        .moderate_comment(
            actor,
            ModerateComment {
                comment_id,
                expected_version: body.expected_version,
                action: body.action.into(),
                reason: body.reason,
            },
        )
        .await?;
    Ok(Json(ModerationDecisionResponse::new(comment_id, decided)))
}
