use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, RawQuery, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use tower_http::set_header::SetResponseHeaderLayer;

use super::conditional::{self, PUBLIC_ERROR_CACHE_CONTROL};
use super::contract::{
    CommentEndorsementRequest, CommentsSettings, CommentsSettingsRequest, DiscussionPage, Draft,
    DraftCard, DraftList, DraftTags, EndorsementQuery, EndorsementSummaryList,
    EndorsementWriteResponse, ModerationDecisionRequest, ModerationDecisionResponse,
    ModerationQueueResponse, OwnCommentList, OwnCommentsParams, OwnEndorsementList, PageQuery,
    PublicDiscussionPage, PublicReplyList, Publication, RemovedCommentResponse, ReplyList,
    SaveDraftRequest, SaveDraftTagsRequest, SavedDraftResponse, SavedDraftTagsResponse,
    SubmitCommentRequest, SubmittedCommentResponse, TagList, TagListQuery, TagResolutionView,
    TagSuggestionList, TagSuggestionQuery, WorkEndorsementRequest, WorkList,
};
use super::{Actor, ApiError, AppState};
use crate::application::SaveDraft;
use crate::application::catalog::SearchTags;
use crate::application::discovery::DiscoverWorks;
use crate::application::discussion::{ModerateComment, SetCommentsMode, SubmitComment};
use crate::application::endorsement::{SetCommentEndorsement, SetWorkEndorsement};
use crate::application::taxonomy::SaveWorkTags;
use crate::domain::discovery::MAX_FILTER_REFS;
use crate::domain::discussion::settings::CommentsMode;
use crate::domain::ids::{CommentId, WorkId};

pub fn routes(state: AppState) -> Router<AppState> {
    let drafts = Router::new()
        .route("/drafts", get(list_drafts).post(create_draft))
        .route("/drafts/{work_id}", get(get_draft).put(save_draft))
        // Draft tags (TAG-PR-03): a resource of its own with its own version (#96 CHOICE-02 B),
        // so a text save never carries, needs or clears tags.
        .route(
            "/drafts/{work_id}/tags",
            get(get_draft_tags).put(save_draft_tags),
        )
        // Account-scoped responses must never enter a shared cache.
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    // Vink-owned discussion (D2T-02b). Responses carry viewer-specific state (own pending
    // comments, `can_remove`), so none may enter a shared cache. The public projection (#120
    // DISC-02/03) resolves no actor, but has no validator yet: `no-store` like the rest, errors
    // included, so a 404/503 or an earlier body never outlives recovery or revocation.
    let discussion = Router::new()
        .route("/works/{work_id}/discussion", get(get_discussion))
        .route(
            "/works/{work_id}/discussion/public",
            get(get_public_discussion),
        )
        .route(
            "/works/{work_id}/discussion/public/roots/{root_id}/replies",
            get(get_public_replies),
        )
        .route("/works/{work_id}/discussion/me", get(get_own_comments))
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
    // Literary endorsement (LVE #73). Own state is account-scoped and summaries are volatile
    // projections without a validator, so no response may be stored by any cache. Summaries do
    // not resolve the actor: an identity outage never hides public counts.
    let endorsement = Router::new()
        .route(
            "/works/{work_id}/endorsements",
            get(get_endorsement_summaries),
        )
        .route(
            "/works/{work_id}/endorsements/me",
            get(get_own_endorsements),
        )
        .route("/works/{work_id}/endorsement/me", put(set_work_endorsement))
        .route(
            "/works/{work_id}/discussion/comments/{comment_id}/endorsement/me",
            put(set_comment_endorsement),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    // Curated catalog (TAG-PR-03). Public: no actor is resolved, so an Identity outage never
    // hides the catalog. No validator yet, so nothing may be stored by a cache. There is no
    // write route: catalog governance is the operator command `vink-app tag …`.
    let catalog = Router::new()
        .route("/tags", get(list_tags))
        .route("/tags/suggestions", get(suggest_tags))
        .route("/tags/slugs/{slug}", get(resolve_tag))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    // Public reads (READ-01): a success sets its own validator and `private, no-cache`; errors
    // carry `no-store` so a 404/503 never outlives recovery in a cache.
    let public = Router::new()
        .route("/works", get(list_works))
        .route("/works/{work_id}/publication", get(get_publication))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static(PUBLIC_ERROR_CACHE_CONTROL),
        ));
    Router::new()
        .merge(public)
        .merge(drafts)
        .merge(discussion)
        .merge(endorsement)
        .merge(catalog)
        .merge(super::auth::routes())
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

fn endorsement_query(
    query: Result<Query<EndorsementQuery>, QueryRejection>,
) -> Result<EndorsementQuery, ApiError> {
    query
        .map(|Query(query)| query)
        .map_err(|rejection| ApiError::Validation {
            field: "query",
            message: rejection.body_text(),
        })
}

fn query_params<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
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

/// `?q=&tag=&tag=&limit=&cursor=` as untrusted strings. `tag` repeats, as a native GET form
/// with several tag fields submits it; the others appear at most once; anything else is refused,
/// so a misspelled parameter never silently widens a search. Values are percent-decoded here
/// and validated only by the use case.
fn works_query(raw: Option<&str>) -> Result<DiscoverWorks, ApiError> {
    let mut search = DiscoverWorks::default();
    let invalid = |message: String| ApiError::Validation {
        field: "query",
        message,
    };
    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        let once = |slot: &mut Option<String>| {
            if slot.replace(value.clone().into_owned()).is_some() {
                Err(invalid(format!("{key} appears more than once")))
            } else {
                Ok(())
            }
        };
        match &*key {
            "q" => once(&mut search.query)?,
            "limit" => once(&mut search.limit)?,
            "cursor" => once(&mut search.cursor)?,
            "tag" => {
                // Bounded before anything is kept; the use case reports the same limit.
                if search.tags.len() > MAX_FILTER_REFS {
                    return Err(ApiError::Validation {
                        field: "tag",
                        message: "too many tag parameters".into(),
                    });
                }
                search.tags.push(value.into_owned());
            }
            _ => return Err(invalid(format!("unknown query parameter `{key}`"))),
        }
    }
    Ok(search)
}

/// Visibility and every error are decided by the use case first; only an existing representation
/// is evaluated against `If-None-Match` (GET and HEAD share this handler).
async fn list_works(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let search = works_query(raw.as_deref())?;
    let page = state.vink.discover_works(search).await?;
    conditional::respond(&headers, &WorkList::from(&page))
}

async fn get_publication(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let work = state.vink.publication(work_id(&raw)?).await?;
    conditional::respond(&headers, &Publication::from(&work))
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

/// No `Actor`: an Identity outage or a stale session cookie never hides the public discussion.
async fn get_public_discussion(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<PublicDiscussionPage>, ApiError> {
    let work_id = work_id(&raw)?;
    let query = page_query(query)?;
    let page = state
        .vink
        .public_discussion(work_id, query.cursor.as_deref(), query.limit)
        .await?;
    Ok(Json(PublicDiscussionPage::new(work_id, &page)))
}

async fn get_public_replies(
    State(state): State<AppState>,
    Path((raw_work, raw_root)): Path<(String, String)>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<PublicReplyList>, ApiError> {
    let work_id = work_id(&raw_work)?;
    let root_id = comment_id(&raw_root)?;
    let query = page_query(query)?;
    let page = state
        .vink
        .public_replies(work_id, root_id, query.cursor.as_deref(), query.limit)
        .await?;
    Ok(Json(PublicReplyList::from(&page)))
}

/// The viewer's private overlay: `401` for guests, `503` while Identity cannot answer, never an
/// empty list in place of either.
async fn get_own_comments(
    State(state): State<AppState>,
    Actor(viewer): Actor,
    Path(raw): Path<String>,
    query: Result<Query<OwnCommentsParams>, QueryRejection>,
) -> Result<Json<OwnCommentList>, ApiError> {
    // Authentication first: a guest learns nothing about the work or the query's validity.
    if viewer.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw)?;
    let params = query_params(query)?;
    let page = state
        .vink
        .own_comments(viewer, work_id, &params.into())
        .await?;
    Ok(Json(OwnCommentList::new(work_id, &page)))
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

async fn get_endorsement_summaries(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    query: Result<Query<EndorsementQuery>, QueryRejection>,
) -> Result<Json<EndorsementSummaryList>, ApiError> {
    let work_id = work_id(&raw)?;
    let query = endorsement_query(query)?;
    let summaries = state
        .vink
        .endorsement_summaries(work_id, query.comment_ids.as_deref())
        .await?;
    Ok(Json(EndorsementSummaryList::from(&summaries)))
}

async fn get_own_endorsements(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    query: Result<Query<EndorsementQuery>, QueryRejection>,
) -> Result<Json<OwnEndorsementList>, ApiError> {
    // A guest is unauthenticated, never an actor who has not endorsed.
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw)?;
    let query = endorsement_query(query)?;
    let own = state
        .vink
        .own_endorsements(actor, work_id, query.comment_ids.as_deref())
        .await?;
    Ok(Json(OwnEndorsementList::from(&own)))
}

async fn set_work_endorsement(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<WorkEndorsementRequest>, JsonRejection>,
) -> Result<Json<EndorsementWriteResponse>, ApiError> {
    // Authentication is decided before the body is inspected.
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw)?;
    let body = json_body(body)?;
    let outcome = state
        .vink
        .set_work_endorsement(
            actor,
            SetWorkEndorsement {
                work_id,
                revision: body.revision_id,
                command_key: body.command_key,
                expected_order: body.expected_order,
                endorsed: body.endorsed,
            },
        )
        .await?;
    Ok(Json(EndorsementWriteResponse::from(&outcome)))
}

async fn set_comment_endorsement(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path((raw_work, raw_comment)): Path<(String, String)>,
    body: Result<Json<CommentEndorsementRequest>, JsonRejection>,
) -> Result<Json<EndorsementWriteResponse>, ApiError> {
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw_work)?;
    let comment_id = comment_id(&raw_comment)?;
    let body = json_body(body)?;
    let outcome = state
        .vink
        .set_comment_endorsement(
            actor,
            SetCommentEndorsement {
                work_id,
                comment_id,
                command_key: body.command_key,
                expected_order: body.expected_order,
                endorsed: body.endorsed,
            },
        )
        .await?;
    Ok(Json(EndorsementWriteResponse::from(&outcome)))
}

async fn list_tags(
    State(state): State<AppState>,
    query: Result<Query<TagListQuery>, QueryRejection>,
) -> Result<Json<TagList>, ApiError> {
    let query = query_params(query)?;
    let page = state.vink.catalog_tags(query.facet.as_deref()).await?;
    Ok(Json(TagList::from(&page)))
}

async fn suggest_tags(
    State(state): State<AppState>,
    query: Result<Query<TagSuggestionQuery>, QueryRejection>,
) -> Result<Json<TagSuggestionList>, ApiError> {
    let query = query_params(query)?;
    let echoed = query.q.clone().unwrap_or_default();
    let suggestions = state
        .vink
        .search_tags(SearchTags {
            query: query.q,
            facet: query.facet,
            limit: query.limit,
        })
        .await?;
    Ok(Json(TagSuggestionList::new(echoed, &suggestions)))
}

async fn resolve_tag(
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> Result<Json<TagResolutionView>, ApiError> {
    let resolution = state.vink.resolve_tag(&raw).await?;
    Ok(Json(TagResolutionView::from(&resolution)))
}

async fn get_draft_tags(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
) -> Result<Json<DraftTags>, ApiError> {
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let tags = state.vink.work_tags(actor, work_id(&raw)?).await?;
    Ok(Json(DraftTags::from(&tags)))
}

async fn save_draft_tags(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<SaveDraftTagsRequest>, JsonRejection>,
) -> Result<Json<SavedDraftTagsResponse>, ApiError> {
    // Authentication is decided before the body is inspected.
    if actor.is_none() {
        return Err(ApiError::Unauthenticated);
    }
    let work_id = work_id(&raw)?;
    let body = json_body(body)?;
    let saved = state
        .vink
        .save_work_tags(
            actor,
            SaveWorkTags {
                work_id,
                expected_version: body.expected_version,
                form: body.form,
                themes: body.themes,
            },
        )
        .await?;
    Ok(Json(SavedDraftTagsResponse::from(&saved)))
}
