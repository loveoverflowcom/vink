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
    AddNoteReq, AuthorPublicationState, AuthorRevisionView, AutomationStateDto,
    CaptureSnapshotRequest, CapturedSnapshotResponse, CaseDetailDto, CaseExpectedVersionReq,
    CaseOpenedDto, CaseStatusDto, CaseSummaryDto, CommentEndorsementRequest, CommentsSettings,
    CommentsSettingsRequest, ConcludeOutcomeDto, ConcludeReq, CreateGrantReq, CreateGrantRes,
    CreateGroupReq, CreateGroupRes, DiscussionPage, Draft, DraftCard, DraftList, DraftTags,
    EndorsementQuery, EndorsementSummaryList, EndorsementWriteResponse, GrantFactsDto,
    GroupMemberReq, ModerationDecisionRequest, ModerationDecisionResponse, ModerationQueueResponse,
    NoteRecordDto, OpenTicketView, OwnCommentList, OwnCommentsParams, OwnEndorsementList,
    PageQuery, PublicDiscussionPage, PublicPointerView, PublicReplyList, Publication,
    PublishIntentView, ReaderResponse, RemovedCommentResponse, ReplyList, RequestPublicRequest,
    RequestPublicResponse, SaveDraftRequest, SaveDraftTagsRequest, SavedDraftResponse,
    SavedDraftTagsResponse, SetAutomationModeReq, SetPrivateRequest, SetPrivateResponse,
    SubmitCommentRequest, SubmittedCommentResponse, TagList, TagListQuery, TagResolutionView,
    TagSuggestionList, TagSuggestionQuery, TakeoverReq, WorkEndorsementRequest, WorkList,
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
use crate::domain::ids::{CaseId, CommentId, GrantId, GroupId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::DecisionReason;
use crate::domain::moderation::authority::{GrantScope, GrantSpec, Grantee, ReviewCapability};
use crate::domain::moderation::automation::{AutomationMode, AutomationVersion};
use crate::domain::moderation::case::{CaseVerdict, CaseVersion, InternalNote};
use time::OffsetDateTime;

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
        .route("/works/{work_id}/reader", get(get_reader))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static(PUBLIC_ERROR_CACHE_CONTROL),
        ));
    // Author publication commands and state read (PUB-RUNTIME-01, #158).
    // Private/author responses must never enter a shared cache.
    let publication_author = Router::new()
        .route("/works/{work_id}/snapshots", post(capture_snapshot))
        .route("/works/{work_id}/publication/submit", post(request_public))
        .route(
            "/works/{work_id}/publication/request-public",
            post(request_public),
        )
        .route("/works/{work_id}/publication/private", post(set_private))
        .route(
            "/works/{work_id}/publication/set-private",
            post(set_private),
        )
        .route(
            "/works/{work_id}/publication/state",
            get(get_author_publication_state),
        )
        .route(
            "/works/{work_id}/publication/me",
            get(get_author_publication_state),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    // Work moderation cases and administration (MOD-09..15, #159).
    let moderation_works = Router::new()
        .route("/moderation/works/queue", get(list_work_moderation_queue))
        .route(
            "/moderation/works/{work_id}/revisions/{revision_id}/cases",
            post(open_work_moderation_case),
        )
        .route(
            "/moderation/works/cases/{case_id}",
            get(get_work_moderation_case),
        )
        .route(
            "/moderation/works/cases/{case_id}/claim",
            post(claim_work_moderation_case),
        )
        .route(
            "/moderation/works/cases/{case_id}/release",
            post(release_work_moderation_case),
        )
        .route(
            "/moderation/works/cases/{case_id}/note",
            post(add_work_moderation_case_note),
        )
        .route(
            "/moderation/works/cases/{case_id}/escalate",
            post(escalate_work_moderation_case),
        )
        .route(
            "/moderation/works/cases/{case_id}/takeover",
            post(takeover_work_moderation_case),
        )
        .route(
            "/moderation/works/cases/{case_id}/conclude",
            post(conclude_work_moderation_case),
        )
        .route("/moderation/grants", get(list_grants).post(create_grant))
        .route("/moderation/grants/{grant_id}/revoke", post(revoke_grant))
        .route("/moderation/groups", post(create_group))
        .route(
            "/moderation/groups/{group_id}/members",
            post(add_group_member),
        )
        .route(
            "/moderation/groups/{group_id}/members/{principal_id}",
            axum::routing::delete(remove_group_member),
        )
        .route("/moderation/automation", get(get_automation_state))
        .route("/moderation/automation/mode", post(set_automation_mode))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    Router::new()
        .merge(public)
        .merge(drafts)
        .merge(publication_author)
        .merge(moderation_works)
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
        (
            [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
            ApiError::Forbidden,
        )
            .into_response()
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

async fn get_reader(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let work = state.vink.reader_work(work_id(&raw)?).await?;
    let resp = ReaderResponse::from(work);
    match &resp {
        ReaderResponse::Published(_) => conditional::respond(&headers, &resp),
        ReaderResponse::ReviewWithdrawn { .. } => {
            let mut response = Json(resp).into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            Ok(response)
        }
    }
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

async fn capture_snapshot(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<CaptureSnapshotRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CapturedSnapshotResponse>), ApiError> {
    let work = work_id(&raw)?;
    let request = json_body(body)?;
    let capture = state
        .vink
        .capture_snapshot(
            actor,
            work,
            &request.command_key,
            request.expected_draft_version,
            request.expected_metadata_version,
        )
        .await?;
    let status = if capture.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        status,
        Json(CapturedSnapshotResponse {
            outcome: "saved",
            revision_id: capture.revision_id.to_string(),
            content_hash: capture.content_hash.to_string(),
            metadata_snapshot_id: capture.metadata_snapshot_id.map(|id| id.to_string()),
            draft_version: capture.draft_version.get(),
            metadata_version: capture.metadata_version.get(),
            replayed: capture.replayed,
        }),
    ))
}

async fn request_public(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<RequestPublicRequest>, JsonRejection>,
) -> Result<Json<RequestPublicResponse>, ApiError> {
    let work = work_id(&raw)?;
    let request = json_body(body)?;
    let rev = RevisionId::parse(&request.revision_id).ok_or(ApiError::Validation {
        field: "revision_id",
        message: "revision_id must be a valid UUID".into(),
    })?;
    let outcome = state
        .vink
        .request_public(
            actor,
            work,
            &request.command_key,
            rev,
            request.expected_generation,
        )
        .await?;
    Ok(Json(RequestPublicResponse {
        outcome: outcome.outcome.as_str(),
        revision_id: request.revision_id,
        generation: outcome.generation.get(),
        ticket_id: outcome.ticket.map(|t| t.id.to_string()),
        replayed: outcome.replayed,
    }))
}

async fn set_private(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<SetPrivateRequest>, JsonRejection>,
) -> Result<Json<SetPrivateResponse>, ApiError> {
    let work = work_id(&raw)?;
    let request = json_body(body)?;
    let outcome = state
        .vink
        .set_private(
            actor,
            work,
            &request.command_key,
            request.expected_generation,
        )
        .await?;
    Ok(Json(SetPrivateResponse {
        outcome: outcome.outcome.as_str(),
        generation: outcome.generation.get(),
        replayed: outcome.replayed,
    }))
}

async fn get_author_publication_state(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
) -> Result<Json<AuthorPublicationState>, ApiError> {
    let work = work_id(&raw)?;
    let facts = state
        .vink
        .load_author_publication_state(actor, work)
        .await?;

    let pub_facts = facts.publication.facts();
    let mod_facts = facts.moderation.facts();

    let audience = match pub_facts.audience {
        crate::domain::publication::Audience::Private => "private",
        crate::domain::publication::Audience::Public => "public",
    };

    let lifecycle = match pub_facts.lifecycle {
        crate::domain::publication::Lifecycle::Active => "active",
        crate::domain::publication::Lifecycle::AuthorDeleted(_) => "author_deleted",
        crate::domain::publication::Lifecycle::Removed => "removed",
    };

    let intent = pub_facts.intent.map(|i| PublishIntentView {
        revision_id: i.revision.to_string(),
        generation: i.generation.get(),
    });

    let pointer = pub_facts.pointer.map(|p| PublicPointerView {
        revision_id: p.revision.to_string(),
        intent_generation: p.intent_generation.get(),
    });

    let mut revisions = Vec::with_capacity(pub_facts.revisions.len());
    for rev in &pub_facts.revisions {
        let meta = facts
            .revision_metadata
            .iter()
            .find(|(id, ..)| *id == rev.snapshot.revision);
        let (title, meta_snapshot_id, created_at) = match meta {
            Some((_, t, m, c)) => (t.clone(), m.map(|id| id.to_string()), *c),
            None => (
                rev.snapshot.content.title().as_str().to_string(),
                None,
                time::OffsetDateTime::now_utc(),
            ),
        };

        let mod_decision = mod_facts
            .revisions
            .iter()
            .find(|r| r.revision == rev.snapshot.revision);

        let moderation = match rev.moderation {
            crate::domain::publication::Moderation::Unsubmitted => "unsubmitted",
            crate::domain::publication::Moderation::Pending => "pending",
            crate::domain::publication::Moderation::Approved => "approved",
            crate::domain::publication::Moderation::Rejected => "rejected",
            crate::domain::publication::Moderation::NeedsAdmin => "needs_admin",
        };

        revisions.push(AuthorRevisionView {
            revision_id: rev.snapshot.revision.to_string(),
            title,
            content_hash: rev.snapshot.content.hash().to_string(),
            moderation,
            decision_version: mod_decision.map(|d| d.version.get()),
            metadata_snapshot_id: meta_snapshot_id,
            created_at,
        });
    }

    let open_ticket = mod_facts
        .jobs
        .iter()
        .find(|j| j.state == crate::domain::moderation::JobState::Open)
        .map(|j| OpenTicketView {
            evaluation_id: j.id.to_string(),
            revision_id: j.target.revision().to_string(),
            policy_version: j.policy.get(),
        });

    Ok(Json(AuthorPublicationState {
        work_id: work.to_string(),
        audience,
        lifecycle,
        generation: pub_facts.generation.get(),
        intent,
        pointer,
        revisions,
        open_ticket,
    }))
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

// ---------------------------------------------------------------------------
// Moderation: Work Cases, Grants, Groups and Automation (MOD-09..15, #159)
// ---------------------------------------------------------------------------
//
// Each command runs in one transaction inside the store (standing fence, directory, locked
// case or work, audit). Reads check authority per case: a reviewer never sees the cases on
// their own work (recusal), and a work-scoped grant shows only that work.

use crate::persistence::moderation as moderation_store;

fn expected_case_version(raw: i64) -> Result<CaseVersion, ApiError> {
    CaseVersion::new(raw).ok_or(ApiError::Validation {
        field: "expected_version",
        message: "invalid case version".into(),
    })
}

async fn read_conn(
    state: &AppState,
) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, ApiError> {
    state
        .vink
        .store()
        .pool()
        .acquire()
        .await
        .map_err(|_| ApiError::DependencyUnavailable)
}

async fn list_work_moderation_queue(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<Json<Vec<CaseSummaryDto>>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let now = OffsetDateTime::now_utc();
    let mut conn = read_conn(&state).await?;
    let directory = moderation_store::load_moderation_directory(&mut conn, now).await?;
    if !directory.has_any_capability(actor, ReviewCapability::Works, now) {
        return Err(ApiError::Forbidden);
    }
    let cases = moderation_store::list_open_work_cases(&mut conn).await?;
    Ok(Json(
        cases
            .into_iter()
            .filter(|c| directory.resolve(actor, &c.target(), now).is_ok())
            .map(|c| CaseSummaryDto {
                id: c.id.to_string(),
                subject_kind: c.subject_kind,
                work_id: c.work_id.to_string(),
                revision_id: c.revision_id.map(|r| r.to_string()),
                decision_version: c.decision_version,
                origin: c.origin,
                version: c.version,
                state: c.state,
                claimed_by: c.claimed_by.map(|p| p.to_string()),
                created_at: c.created_at,
            })
            .collect(),
    ))
}

async fn get_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
) -> Result<Json<CaseDetailDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let case_id = CaseId::parse(&raw).ok_or(ApiError::NotFound)?;
    let now = OffsetDateTime::now_utc();
    let mut conn = read_conn(&state).await?;
    let directory = moderation_store::load_moderation_directory(&mut conn, now).await?;
    if !directory.has_any_capability(actor, ReviewCapability::Works, now) {
        return Err(ApiError::Forbidden);
    }
    let detail = moderation_store::get_case_detail(&mut conn, case_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    directory
        .resolve(actor, &detail.target(), now)
        .map_err(|refusal| ApiError::CaseForbidden(refusal.as_str()))?;
    Ok(Json(CaseDetailDto {
        id: detail.id.to_string(),
        subject_kind: detail.subject_kind,
        work_id: detail.work_id.to_string(),
        revision_id: detail.revision_id.map(|r| r.to_string()),
        content_digest: detail.content_digest,
        title: detail.title,
        document: detail.document,
        author_id: detail.author_id.to_string(),
        author_name: detail.author_name,
        decision_version: detail.decision_version,
        origin: detail.origin,
        version: detail.version,
        state: detail.state,
        claimed_by: detail.claimed_by.map(|p| p.to_string()),
        notes: detail
            .notes
            .into_iter()
            .map(|n| NoteRecordDto {
                id: n.id.to_string(),
                author_id: n.author_id.to_string(),
                note: n.note,
                created_at: n.created_at,
            })
            .collect(),
        created_at: detail.created_at,
    }))
}

async fn open_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path((raw_work, raw_revision)): Path<(String, String)>,
) -> Result<(StatusCode, Json<CaseOpenedDto>), ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let work = work_id(&raw_work)?;
    let revision = RevisionId::parse(&raw_revision).ok_or(ApiError::NotFound)?;
    let case = moderation_store::open_intervention_case(
        state.vink.store().pool(),
        actor,
        work,
        revision,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CaseOpenedDto {
            case_id: case.to_string(),
            version: 1,
        }),
    ))
}

async fn run_case_command(
    state: &AppState,
    actor: PrincipalId,
    raw: &str,
    expected_version: i64,
    command: moderation_store::CaseCommand,
) -> Result<Json<CaseStatusDto>, ApiError> {
    let case_id = CaseId::parse(raw).ok_or(ApiError::NotFound)?;
    let expected = expected_case_version(expected_version)?;
    let next = moderation_store::case_command(
        state.vink.store().pool(),
        actor,
        case_id,
        expected,
        command,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok(Json(CaseStatusDto {
        status: "ok",
        version: next.facts().version.get(),
    }))
}

async fn claim_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<CaseExpectedVersionReq>, JsonRejection>,
) -> Result<Json<CaseStatusDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    run_case_command(
        &state,
        actor,
        &raw,
        body.expected_version,
        moderation_store::CaseCommand::Claim,
    )
    .await
}

async fn release_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<CaseExpectedVersionReq>, JsonRejection>,
) -> Result<Json<CaseStatusDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    run_case_command(
        &state,
        actor,
        &raw,
        body.expected_version,
        moderation_store::CaseCommand::Release,
    )
    .await
}

async fn add_work_moderation_case_note(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<AddNoteReq>, JsonRejection>,
) -> Result<Json<CaseStatusDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    let note = InternalNote::parse(&body.note).map_err(|_| ApiError::Validation {
        field: "note",
        message: "invalid internal note".into(),
    })?;
    run_case_command(
        &state,
        actor,
        &raw,
        body.expected_version,
        moderation_store::CaseCommand::Note(note),
    )
    .await
}

async fn escalate_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<CaseExpectedVersionReq>, JsonRejection>,
) -> Result<Json<CaseStatusDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    run_case_command(
        &state,
        actor,
        &raw,
        body.expected_version,
        moderation_store::CaseCommand::Escalate,
    )
    .await
}

async fn takeover_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<TakeoverReq>, JsonRejection>,
) -> Result<Json<CaseStatusDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    let reason = DecisionReason::parse(&body.reason).map_err(|e| ApiError::Validation {
        field: "reason",
        message: e.to_string(),
    })?;
    run_case_command(
        &state,
        actor,
        &raw,
        body.expected_version,
        moderation_store::CaseCommand::TakeOver(reason),
    )
    .await
}

async fn conclude_work_moderation_case(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<ConcludeReq>, JsonRejection>,
) -> Result<Json<ConcludeOutcomeDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let case_id = CaseId::parse(&raw).ok_or(ApiError::NotFound)?;
    let body = json_body(body)?;
    let expected = expected_case_version(body.expected_version)?;
    let verdict = match body.verdict.as_str() {
        "approved" => CaseVerdict::Approve,
        "rejected" => CaseVerdict::Reject,
        other => {
            return Err(ApiError::Validation {
                field: "verdict",
                message: format!("invalid verdict: {other}"),
            });
        }
    };
    let reason = body
        .reason
        .as_deref()
        .map(DecisionReason::parse)
        .transpose()
        .map_err(|e| ApiError::Validation {
            field: "reason",
            message: e.to_string(),
        })?;
    moderation_store::conclude_case(
        state.vink.store().pool(),
        actor,
        case_id,
        expected,
        verdict,
        reason,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok(Json(ConcludeOutcomeDto {
        status: "ok",
        verdict: body.verdict,
    }))
}

async fn list_grants(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<Json<Vec<GrantFactsDto>>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let now = OffsetDateTime::now_utc();
    let mut conn = read_conn(&state).await?;
    let directory = moderation_store::load_moderation_directory(&mut conn, now).await?;
    directory
        .require_admin(actor, now)
        .map_err(|refusal| ApiError::CaseForbidden(refusal.as_str()))?;
    let grants = moderation_store::list_moderation_grants(&mut conn).await?;
    Ok(Json(
        grants
            .into_iter()
            .map(|g| GrantFactsDto {
                id: g.id.to_string(),
                grantee_kind: match g.grantee {
                    Grantee::Principal(_) => "principal".to_string(),
                    Grantee::Group(_) => "group".to_string(),
                },
                grantee_id: match g.grantee {
                    Grantee::Principal(p) => p.to_string(),
                    Grantee::Group(grp) => grp.to_string(),
                },
                capability: g.capability.as_str().to_string(),
                scope_kind: match g.scope {
                    GrantScope::Global => "global".to_string(),
                    GrantScope::Work(_) => "work".to_string(),
                },
                scope_work_id: match g.scope {
                    GrantScope::Global => None,
                    GrantScope::Work(w) => Some(w.to_string()),
                },
                expires_at: g.expires_at.map(|t| {
                    t.format(&time::format_description::well_known::Rfc3339)
                        .unwrap_or_default()
                }),
                revoked: g.revoked,
                origin: match g.origin {
                    crate::domain::moderation::authority::GrantOrigin::Admin => "admin".to_string(),
                    crate::domain::moderation::authority::GrantOrigin::InheritedComment => {
                        "inherited_comment".to_string()
                    }
                },
            })
            .collect(),
    ))
}

async fn create_grant(
    State(state): State<AppState>,
    Actor(actor): Actor,
    body: Result<Json<CreateGrantReq>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateGrantRes>), ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    let grantee = match body.grantee_kind.as_str() {
        "principal" => Grantee::Principal(PrincipalId::parse(&body.grantee_id).ok_or(
            ApiError::Validation {
                field: "grantee_id",
                message: "invalid principal id".into(),
            },
        )?),
        "group" => Grantee::Group(GroupId::parse(&body.grantee_id).ok_or(
            ApiError::Validation {
                field: "grantee_id",
                message: "invalid group id".into(),
            },
        )?),
        other => {
            return Err(ApiError::Validation {
                field: "grantee_kind",
                message: format!("invalid grantee kind: {other}"),
            });
        }
    };
    let capability = match body.capability.as_str() {
        "works" => ReviewCapability::Works,
        "comments" => ReviewCapability::Comments,
        other => {
            return Err(ApiError::Validation {
                field: "capability",
                message: format!("invalid capability: {other}"),
            });
        }
    };
    let scope = match body.scope_kind.as_str() {
        "global" => GrantScope::Global,
        "work" => {
            let raw = body.scope_work_id.as_deref().ok_or(ApiError::Validation {
                field: "scope_work_id",
                message: "missing work id for scoped grant".into(),
            })?;
            GrantScope::Work(WorkId::parse(raw).ok_or(ApiError::Validation {
                field: "scope_work_id",
                message: "invalid work id".into(),
            })?)
        }
        other => {
            return Err(ApiError::Validation {
                field: "scope_kind",
                message: format!("invalid scope kind: {other}"),
            });
        }
    };
    let expires_at = body
        .expires_at
        .as_deref()
        .map(|raw| OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339))
        .transpose()
        .map_err(|_| ApiError::Validation {
            field: "expires_at",
            message: "invalid rfc3339 timestamp".into(),
        })?;
    let grant_id = moderation_store::create_moderation_grant(
        state.vink.store().pool(),
        actor,
        GrantSpec {
            grantee,
            capability,
            scope,
            expires_at,
        },
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateGrantRes {
            grant_id: grant_id.to_string(),
        }),
    ))
}

async fn revoke_grant(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
) -> Result<StatusCode, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let grant_id = GrantId::parse(&raw).ok_or(ApiError::NotFound)?;
    moderation_store::revoke_moderation_grant(
        state.vink.store().pool(),
        actor,
        grant_id,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_group(
    State(state): State<AppState>,
    Actor(actor): Actor,
    body: Result<Json<CreateGroupReq>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateGroupRes>), ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    let group_id = moderation_store::create_moderation_group(
        state.vink.store().pool(),
        actor,
        &body.name,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateGroupRes {
            group_id: group_id.to_string(),
        }),
    ))
}

async fn set_group_member(
    state: &AppState,
    actor: Option<PrincipalId>,
    raw_group: &str,
    raw_member: &str,
    present: bool,
) -> Result<StatusCode, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let group = GroupId::parse(raw_group).ok_or(ApiError::NotFound)?;
    let member = PrincipalId::parse(raw_member).ok_or(ApiError::Validation {
        field: "principal_id",
        message: "invalid principal id".into(),
    })?;
    moderation_store::set_group_membership(
        state.vink.store().pool(),
        actor,
        group,
        member,
        present,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_group_member(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path(raw): Path<String>,
    body: Result<Json<GroupMemberReq>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let body = json_body(body)?;
    set_group_member(&state, actor, &raw, &body.principal_id, true).await
}

async fn remove_group_member(
    State(state): State<AppState>,
    Actor(actor): Actor,
    Path((raw_group, raw_member)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    set_group_member(&state, actor, &raw_group, &raw_member, false).await
}

async fn get_automation_state(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<Json<AutomationStateDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let now = OffsetDateTime::now_utc();
    let mut conn = read_conn(&state).await?;
    let directory = moderation_store::load_moderation_directory(&mut conn, now).await?;
    directory
        .require_admin(actor, now)
        .map_err(|refusal| ApiError::CaseForbidden(refusal.as_str()))?;
    let automation = moderation_store::load_automation(&mut conn).await?;
    Ok(Json(AutomationStateDto {
        mode: automation.mode().as_str(),
        version: automation.version().get(),
    }))
}

async fn set_automation_mode(
    State(state): State<AppState>,
    Actor(actor): Actor,
    body: Result<Json<SetAutomationModeReq>, JsonRejection>,
) -> Result<Json<AutomationStateDto>, ApiError> {
    let actor = actor.ok_or(ApiError::Unauthenticated)?;
    let body = json_body(body)?;
    let expected_version =
        AutomationVersion::new(body.expected_version).ok_or(ApiError::Validation {
            field: "expected_version",
            message: "invalid automation version".into(),
        })?;
    let mode = match body.mode.as_str() {
        "enabled" => AutomationMode::Enabled,
        "paused" => AutomationMode::Paused,
        other => {
            return Err(ApiError::Validation {
                field: "mode",
                message: format!("invalid automation mode: {other}"),
            });
        }
    };
    let reason = body
        .reason
        .as_deref()
        .map(DecisionReason::parse)
        .transpose()
        .map_err(|e| ApiError::Validation {
            field: "reason",
            message: e.to_string(),
        })?;
    let next = moderation_store::set_automation_mode(
        state.vink.store().pool(),
        actor,
        expected_version,
        mode,
        reason,
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok(Json(AutomationStateDto {
        mode: next.mode().as_str(),
        version: next.version().get(),
    }))
}
