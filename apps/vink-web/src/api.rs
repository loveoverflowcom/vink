//! Data access for views. Public reads resolve in-process on the server (through
//! [`crate::server::PublicReads`]) and over `/api/v1` in the browser; both yield the same
//! contract types. Desk calls exist only in the browser.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "hydrate")]
use crate::contract::{ApiError, Draft, DraftList, SaveDraftRequest, SavedDraft};
use crate::contract::{EndorsementSummaries, Publication, WorkList};

const DISCUSSION_ID: percent_encoding::AsciiSet =
    percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_');

pub fn local_discussion_fixture_scope() -> Option<String> {
    #[cfg(feature = "ssr")]
    {
        use_context::<crate::server::LocalDiscussionFixture>().map(|scope| scope.0)
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        web_sys::window()?
            .document()?
            .query_selector("meta[name='vink-local-discussion-fixture']")
            .ok()??
            .get_attribute("content")
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        None
    }
}

/// Whether the server said this deployment may start new sessions. A hint for which controls to
/// render; each sign-in request is still decided by the server (`501` when closed).
pub fn sign_in_open() -> bool {
    #[cfg(feature = "ssr")]
    {
        use_context::<crate::server::SignInOpen>().is_some_and(|open| open.0)
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| {
                document
                    .query_selector("meta[name='vink-sign-in']")
                    .ok()
                    .flatten()
            })
            .is_some()
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        false
    }
}

pub(crate) fn following_feed_enabled() -> bool {
    #[cfg(feature = "ssr")]
    {
        use_context::<crate::server::FollowingFeedEnabled>().is_some_and(|flag| flag.0)
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        document()
            .query_selector("meta[name='vink-following-feed']")
            .ok()
            .flatten()
            .is_some()
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        false
    }
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_following_feed(
    query: &str,
    return_scope: Option<&str>,
) -> Result<(crate::contract::FeedResponse, Option<String>), ApiError> {
    http::following_feed(&format!("/api/v1/me/following-feed{query}"), return_scope).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn current_account()
-> Result<crate::contract::CurrentAccount, crate::session::AuthError> {
    http::auth::<(), _>("GET", "/api/v1/auth/session/account", &[200], None).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn session_state()
-> Result<crate::contract::SessionState, crate::session::AuthError> {
    http::auth::<(), _>("GET", "/api/v1/auth/session", &[200], None).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn request_sign_in_code(
    email: String,
) -> Result<crate::contract::ChallengeSent, crate::session::AuthError> {
    http::auth(
        "POST",
        "/api/v1/auth/sign-in/challenges",
        &[202],
        Some(&crate::contract::StartSignInRequest { email }),
    )
    .await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn complete_sign_in(
    code: String,
) -> Result<crate::contract::SessionState, crate::session::AuthError> {
    http::auth(
        "POST",
        "/api/v1/auth/sign-in/complete",
        &[200],
        Some(&crate::contract::CompleteSignInRequest { code }),
    )
    .await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn register_password(
    credentials: crate::contract::PasswordCredentials,
) -> Result<crate::contract::SessionState, crate::session::AuthError> {
    http::auth(
        "POST",
        "/api/v1/auth/password/register",
        &[200],
        Some(&credentials),
    )
    .await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn login_password(
    credentials: crate::contract::PasswordCredentials,
) -> Result<crate::contract::SessionState, crate::session::AuthError> {
    http::auth(
        "POST",
        "/api/v1/auth/password/login",
        &[200],
        Some(&credentials),
    )
    .await
}

#[cfg(feature = "hydrate")]
const ENROLL_PASSWORD: &str = "/api/v1/auth/password/enroll";

#[cfg(feature = "hydrate")]
pub(crate) async fn request_enrollment_code()
-> Result<crate::contract::ChallengeSent, crate::session::AuthError> {
    http::auth::<(), _>(
        "POST",
        "/api/v1/auth/password/enroll/challenges",
        &[202],
        None,
    )
    .await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn enroll_password(
    proof: crate::contract::EnrollPasswordRequest,
) -> Result<crate::contract::PasswordEnrollmentStatus, crate::session::AuthError> {
    http::auth("POST", ENROLL_PASSWORD, &[200], Some(&proof)).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn password_enrollment_status()
-> Result<crate::contract::PasswordEnrollmentStatus, crate::session::AuthError> {
    http::auth::<(), _>("GET", ENROLL_PASSWORD, &[200], None).await
}

#[cfg(feature = "hydrate")]
const EMAIL_ATTACHMENT: &str = "/api/v1/auth/email/attachment";

#[cfg(feature = "hydrate")]
pub(crate) async fn email_attachment_status()
-> Result<crate::contract::EmailAttachmentStatus, crate::session::AuthError> {
    http::auth::<(), _>("GET", EMAIL_ATTACHMENT, &[200], None).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn request_email_attachment(
    input: crate::contract::StartEmailAttachment,
) -> Result<crate::contract::EmailAttachmentChallenge, crate::session::AuthError> {
    http::auth(
        "POST",
        &format!("{EMAIL_ATTACHMENT}/challenges"),
        &[202],
        Some(&input),
    )
    .await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn complete_email_attachment(
    input: crate::contract::CompleteEmailAttachment,
) -> Result<(), crate::session::AuthError> {
    http::auth(
        "POST",
        &format!("{EMAIL_ATTACHMENT}/complete"),
        &[204],
        Some(&input),
    )
    .await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn provision_principal(
    pen_name: String,
) -> Result<crate::contract::ProvisionedPrincipal, crate::session::AuthError> {
    http::auth(
        "POST",
        "/api/v1/auth/principal",
        &[200, 201],
        Some(&crate::contract::ProvisionPrincipalRequest { pen_name }),
    )
    .await
}

/// `DELETE /auth/session` answers `204` only when the session is revoked (or there was none).
#[cfg(feature = "hydrate")]
pub(crate) async fn sign_out() -> Result<(), crate::session::AuthError> {
    http::auth::<(), ()>("DELETE", "/api/v1/auth/session", &[204], None).await
}

/// Called when a private request is answered `401`: the session owner re-reads the session.
#[cfg(feature = "hydrate")]
pub(crate) fn on_unauthenticated(hook: impl Fn() + 'static) {
    http::UNAUTHENTICATED.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

/// The anonymous discussion page and the public endorsement summaries (LVE #73) of its public
/// comments, read one after the other so SSR and hydration render the same counts. A failed
/// summary read never hides the discussion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscussionLoad {
    pub page: crate::contract::PublicDiscussionPage,
    pub endorsements: Result<EndorsementSummaries, LoadError>,
}

fn first_batch(page: &crate::contract::PublicDiscussionPage) -> Vec<String> {
    // A first page has at most 20 roots with 3 preview replies each (80 < 100); later pages
    // are read in batches by the browser adapter.
    crate::endorsement_state::page_targets(&page.clone().into())
        .into_iter()
        .filter_map(|target| match target {
            crate::endorsement_state::TargetKey::Comment { comment_id } => Some(comment_id),
            crate::endorsement_state::TargetKey::Work { .. } => None,
        })
        .take(crate::endorsement_state::MAX_BATCH)
        .collect()
}

pub fn discussion_resource(work_id: String) -> Resource<Result<DiscussionLoad, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || work_id.clone(),
            move |id| {
                let port = port.clone();
                async move {
                    let Some(port) = port else {
                        return Err(LoadError::Unavailable);
                    };
                    let page = port.discussion(id.clone()).await?;
                    let endorsements = port.endorsement_summaries(id, first_batch(&page)).await;
                    Ok(DiscussionLoad { page, endorsements })
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(
            move || work_id.clone(),
            |id| {
                browser(async move {
                    let page: crate::contract::PublicDiscussionPage =
                        public_get(public_discussion_path(&id, None, None)).await?;
                    let endorsements =
                        public_get(endorsements_path(&id, false, &first_batch(&page))).await;
                    Ok(DiscussionLoad { page, endorsements })
                })
            },
        )
    }
}

/// Public endorsement summary of the work's current revision (no comments), for the reader.
pub fn work_endorsement_resource(
    work_id: String,
) -> Resource<Result<EndorsementSummaries, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || work_id.clone(),
            move |id| {
                let port = port.clone();
                async move {
                    match port {
                        Some(port) => port.endorsement_summaries(id, Vec::new()).await,
                        None => Err(LoadError::Unavailable),
                    }
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(
            move || work_id.clone(),
            |id| browser(public_get(endorsements_path(&id, false, &[]))),
        )
    }
}

/// `GET …/endorsements[/me]?comment_ids=` with each ID encoded; commas separate them.
pub fn endorsements_path(work: &str, own: bool, comment_ids: &[String]) -> String {
    use percent_encoding::utf8_percent_encode;
    let mut path = format!(
        "/api/v1/works/{}/endorsements{}",
        utf8_percent_encode(work, &DISCUSSION_ID),
        if own { "/me" } else { "" }
    );
    if !comment_ids.is_empty() {
        let ids: Vec<String> = comment_ids
            .iter()
            .map(|id| utf8_percent_encode(id, &DISCUSSION_ID).to_string())
            .collect();
        path.push_str("?comment_ids=");
        path.push_str(&ids.join(","));
    }
    path
}

/// `PUT` path of one endorsement target.
pub fn endorsement_path(work: &str, comment: Option<&str>) -> String {
    use percent_encoding::utf8_percent_encode;
    match comment {
        None => format!(
            "/api/v1/works/{}/endorsement/me",
            utf8_percent_encode(work, &DISCUSSION_ID)
        ),
        Some(comment) => format!(
            "{}/comments/{}/endorsement/me",
            discussion_path(work, None, None),
            utf8_percent_encode(comment, &DISCUSSION_ID)
        ),
    }
}

#[cfg(feature = "hydrate")]
pub async fn load_endorsement_summaries(
    work: &str,
    comment_ids: &[String],
) -> Result<EndorsementSummaries, ApiError> {
    http::get(&endorsements_path(work, false, comment_ids)).await
}

#[cfg(feature = "hydrate")]
pub async fn load_own_endorsements(
    work: &str,
    comment_ids: &[String],
) -> Result<crate::contract::OwnEndorsements, ApiError> {
    http::get(&endorsements_path(work, true, comment_ids)).await
}

#[cfg(feature = "hydrate")]
pub async fn set_endorsement(
    work: &str,
    request: &crate::endorsement_state::Request,
) -> Result<crate::contract::EndorsementWrite, ApiError> {
    use crate::endorsement_state::Request;
    match request {
        Request::Work(body) => http::send("PUT", &endorsement_path(work, None), Some(body)).await,
        Request::Comment { comment_id, body } => {
            http::send("PUT", &endorsement_path(work, Some(comment_id)), Some(body)).await
        }
    }
}

/// Cursors are opaque. Encode rather than concatenate provider text into a query string.
pub fn discussion_path(work: &str, root: Option<&str>, cursor: Option<&str>) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let work = utf8_percent_encode(work, &DISCUSSION_ID);
    let mut path = format!("/api/v1/works/{work}/discussion");
    if let Some(root) = root {
        path.push_str(&format!(
            "/roots/{}/replies",
            utf8_percent_encode(root, &DISCUSSION_ID)
        ));
    }
    if let Some(cursor) = cursor {
        path.push_str(&format!(
            "?cursor={}",
            utf8_percent_encode(cursor, NON_ALPHANUMERIC)
        ));
    }
    path
}

/// Separate public/own cursor namespaces; writes keep their existing composed base path.
pub fn public_discussion_path(work: &str, root: Option<&str>, cursor: Option<&str>) -> String {
    discussion_path(work, root, cursor).replacen("/discussion", "/discussion/public", 1)
}
pub fn own_comments_path(work: &str, cursor: Option<&str>, ids: Option<&[String]>) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let mut path = format!("{}/me", discussion_path(work, None, None));
    if let Some(ids) = ids {
        path.push_str("?ids=");
        path.push_str(
            &ids.iter()
                .map(|id| utf8_percent_encode(id, NON_ALPHANUMERIC).to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
    } else if let Some(cursor) = cursor {
        path.push_str(&format!(
            "?cursor={}",
            utf8_percent_encode(cursor, NON_ALPHANUMERIC)
        ));
    }
    path
}
#[cfg(feature = "hydrate")]
pub async fn load_own_comments(
    work: &str,
    cursor: Option<&str>,
    ids: Option<&[String]>,
) -> Result<crate::contract::OwnCommentList, ApiError> {
    http::get(&own_comments_path(work, cursor, ids)).await
}
#[cfg(feature = "hydrate")]
pub async fn load_discussion_capability(
    work: &str,
) -> Result<crate::contract::DiscussionCapability, ApiError> {
    http::get(&format!("{}?limit=1", discussion_path(work, None, None))).await
}

#[cfg(feature = "hydrate")]
pub async fn load_discussion(
    work: &str,
    cursor: Option<&str>,
) -> Result<crate::contract::DiscussionPage, ApiError> {
    http::get::<crate::contract::PublicDiscussionPage>(&public_discussion_path(work, None, cursor))
        .await
        .map(Into::into)
}

#[cfg(feature = "hydrate")]
pub async fn load_replies(
    work: &str,
    root: &str,
    cursor: Option<&str>,
) -> Result<crate::contract::ReplyList, ApiError> {
    http::get::<crate::contract::PublicReplyList>(&public_discussion_path(work, Some(root), cursor))
        .await
        .map(Into::into)
}

#[cfg(feature = "hydrate")]
pub async fn submit_comment(
    work: &str,
    request: &crate::contract::SubmitCommentRequest,
) -> Result<crate::contract::SubmittedComment, ApiError> {
    http::send(
        "POST",
        &format!("{}/comments", discussion_path(work, None, None)),
        Some(request),
    )
    .await
}

#[cfg(feature = "hydrate")]
pub async fn remove_comment(
    work: &str,
    comment: &str,
) -> Result<crate::contract::RemovedComment, ApiError> {
    use percent_encoding::utf8_percent_encode;
    http::send::<(), _>(
        "DELETE",
        &format!(
            "{}/comments/{}",
            discussion_path(work, None, None),
            utf8_percent_encode(comment, &DISCUSSION_ID)
        ),
        None,
    )
    .await
}

#[cfg(feature = "hydrate")]
pub async fn set_comments_mode(
    work: &str,
    request: &crate::contract::CommentsSettingsRequest,
) -> Result<crate::contract::CommentsSettings, ApiError> {
    http::send(
        "PUT",
        &format!("{}/settings", discussion_path(work, None, None)),
        Some(request),
    )
    .await
}

pub use crate::freshness::{InputField as InvalidInputField, Rejection as InvalidRead};

/// Public read failures; serialized into the hydration payload with the page data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadError {
    NotFound,
    Invalid(InvalidRead),
    Unavailable,
}

impl LoadError {
    pub const fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }
}

pub(crate) fn author_search_resource(
    q: StoredValue<String>,
    cursor: RwSignal<Option<String>>,
    generation: RwSignal<u64>,
) -> Resource<(u64, Result<crate::contract::PublicAuthorPage, LoadError>)> {
    let source = move || (generation.get(), cursor.get());
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(source, move |(stamp, cursor)| {
            let port = port.clone();
            let q = q.get_value();
            async move {
                let page = match port {
                    Some(port) => port.authors(q, cursor).await,
                    None => Err(LoadError::Unavailable),
                };
                (stamp, page)
            }
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(source, move |(stamp, cursor)| {
            let q = q.get_value();
            browser(async move { (stamp, load_author_search(&q, cursor.as_deref()).await) })
        })
    }
}

#[cfg(not(feature = "ssr"))]
pub(crate) async fn load_author_search(
    query: &str,
    cursor: Option<&str>,
) -> Result<crate::contract::PublicAuthorPage, LoadError> {
    let mut path = format!("/api/v1/authors?q={}", crate::discovery::encode(query));
    if let Some(cursor) = cursor {
        path.push_str("&cursor=");
        path.push_str(&crate::discovery::encode(cursor));
    }
    public_get(path).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_follow_membership(
    handles: &[String],
) -> Result<crate::contract::follow::MembershipBatch, ApiError> {
    http::get(&follow_membership_path(handles)).await
}

pub fn works_resource(
    query: Memo<crate::discovery::ExploreQuery>,
) -> Resource<(crate::discovery::ExploreQuery, Result<WorkList, LoadError>)> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || query.get(),
            move |query| {
                let port = port.clone();
                async move {
                    let result = match port {
                        Some(port) => port.works(query.clone()).await,
                        None => Err(LoadError::Unavailable),
                    };
                    (query, result)
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(
            move || query.get(),
            |query| {
                browser(async move {
                    let result =
                        match public(crate::public_reads::PublicKey::Works(query.clone())).await {
                            Ok(crate::public_reads::Snapshot::Works(list)) => Ok(list),
                            Ok(
                                crate::public_reads::Snapshot::Publication(_)
                                | crate::public_reads::Snapshot::Control(_),
                            ) => Err(LoadError::Unavailable),
                            Err(error) => Err(error),
                        };
                    (query, result)
                })
            },
        )
    }
}

/// One no-store catalog read per discovery scope. The picker can explicitly retry it.
pub fn catalog_resource() -> Resource<Result<crate::contract::TagList, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            || (),
            move |_| {
                let port = port.clone();
                async move {
                    match port {
                        Some(port) => port.catalog().await,
                        None => Err(LoadError::Unavailable),
                    }
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(|| (), |_| browser(public_get("/api/v1/tags".into())))
    }
}

pub fn publication_resource(work_id: Memo<String>) -> Resource<Result<Publication, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || work_id.get(),
            move |id| {
                let port = port.clone();
                async move {
                    match port {
                        Some(port) => port.publication(id).await,
                        None => Err(LoadError::Unavailable),
                    }
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(
            move || work_id.get(),
            |id| {
                browser(async move {
                    match public(crate::public_reads::PublicKey::Reader(id)).await? {
                        crate::public_reads::Snapshot::Publication(p) => Ok(p),
                        crate::public_reads::Snapshot::Works(_)
                        | crate::public_reads::Snapshot::Control(_) => Err(LoadError::Unavailable),
                    }
                })
            },
        )
    }
}

pub fn reader_resource(
    work_id: Memo<String>,
) -> Resource<Result<crate::contract::ReaderResponse, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || work_id.get(),
            move |id| {
                let port = port.clone();
                async move {
                    match port {
                        Some(port) => port.reader(id).await,
                        None => Err(LoadError::Unavailable),
                    }
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(
            move || work_id.get(),
            |id| {
                browser(async move {
                    match public(crate::public_reads::PublicKey::Reader(id)).await? {
                        crate::public_reads::Snapshot::Publication(p) => {
                            Ok(crate::contract::ReaderResponse::Published(p))
                        }
                        crate::public_reads::Snapshot::Control(control) => Ok(control),
                        _ => Err(LoadError::Unavailable),
                    }
                })
            },
        )
    }
}

/// Wraps a browser-only (`!Send`) future for `Resource`, which requires `Send` futures.
#[cfg(not(feature = "ssr"))]
fn browser<F: std::future::Future + 'static>(
    future: F,
) -> impl std::future::Future<Output = F::Output> + Send {
    #[cfg(feature = "hydrate")]
    {
        send_wrapper::SendWrapper::new(future)
    }
    #[cfg(not(feature = "hydrate"))]
    {
        // Neither ssr nor hydrate: a library build for tooling only.
        drop(future);
        std::future::pending()
    }
}

/// Discussion stays `no-store` and outside the READ-02 store.
#[cfg(not(feature = "ssr"))]
async fn public_get<T: serde::de::DeserializeOwned>(path: String) -> Result<T, LoadError> {
    #[cfg(feature = "hydrate")]
    {
        http::get(&path).await.map_err(|error| match error {
            ApiError::NotFound => LoadError::NotFound,
            ApiError::Invalid { field } => {
                LoadError::Invalid(crate::freshness::classify_rejection(
                    422,
                    Some("validation_failed"),
                    field.as_deref(),
                ))
            }
            ApiError::Unexpected {
                status: 400 | 414 | 422,
            } => LoadError::Invalid(InvalidRead::Fallback),
            _ => LoadError::Unavailable,
        })
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = path;
        Err(LoadError::Unavailable)
    }
}

/// Browser public reads go through the tab's READ-02 owner (cache-first, single-flight).
#[cfg(not(feature = "ssr"))]
async fn public(
    key: crate::public_reads::PublicKey,
) -> Result<crate::public_reads::Snapshot, LoadError> {
    #[cfg(feature = "hydrate")]
    {
        crate::public_reads::load(key).await
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = key;
        Err(LoadError::Unavailable)
    }
}

/// One public read for the READ-02 owner, revalidated through the browser HTTP cache.
#[cfg(feature = "hydrate")]
pub(crate) async fn fetch_public<T: serde::de::DeserializeOwned>(
    path: &str,
) -> crate::freshness::Outcome<T> {
    http::public(path).await
}

#[cfg(feature = "hydrate")]
pub async fn list_drafts() -> Result<DraftList, ApiError> {
    http::get("/api/v1/drafts").await
}

/// Private summary search. Each value is encoded separately; it cannot add query parameters.
pub fn drafts_path(
    q: &str,
    sort: crate::draft_list::DraftSort,
    limit: usize,
    cursor: Option<&str>,
) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let sort = match sort {
        crate::draft_list::DraftSort::Recent => "recent",
        crate::draft_list::DraftSort::Title => "title",
    };
    let mut path = format!(
        "/api/v1/drafts?q={}&sort={sort}&limit={limit}",
        utf8_percent_encode(q, NON_ALPHANUMERIC)
    );
    if let Some(cursor) = cursor {
        path.push_str("&cursor=");
        path.push_str(&utf8_percent_encode(cursor, NON_ALPHANUMERIC).to_string());
    }
    path
}

#[cfg(feature = "hydrate")]
pub async fn search_author_works(
    q: &str,
    sort: crate::draft_list::DraftSort,
    limit: usize,
    cursor: Option<&str>,
) -> Result<crate::contract::author_works::AuthorWorkList, ApiError> {
    let path =
        drafts_path(q, sort, limit, cursor).replacen("/api/v1/drafts?", "/api/v1/author/works?", 1);
    http::get(&path).await
}

#[cfg(feature = "hydrate")]
pub async fn search_drafts(
    q: &str,
    sort: crate::draft_list::DraftSort,
    limit: usize,
    cursor: Option<&str>,
) -> Result<DraftList, ApiError> {
    http::get(&drafts_path(q, sort, limit, cursor)).await
}

#[cfg(test)]
mod private_draft_paths {
    #[test]
    fn query_and_cursor_cannot_add_an_owner_or_change_the_page_size() {
        assert_eq!(
            super::drafts_path(
                "Đ &owner=x",
                crate::draft_list::DraftSort::Title,
                24,
                Some("c&limit=100")
            ),
            "/api/v1/drafts?q=%C4%90%20%26owner%3Dx&sort=title&limit=24&cursor=c%26limit%3D100"
        );
    }
}

#[cfg(feature = "hydrate")]
pub async fn create_draft() -> Result<Draft, ApiError> {
    create_draft_with_format(crate::contract::DocumentFormatDto::Default).await
}

#[cfg(feature = "hydrate")]
pub async fn create_draft_with_format(
    format: crate::contract::DocumentFormatDto,
) -> Result<Draft, ApiError> {
    match format {
        crate::contract::DocumentFormatDto::Default => {
            http::send::<(), Draft>("POST", "/api/v1/drafts", None).await
        }
        crate::contract::DocumentFormatDto::Markdown => {
            let body = crate::contract::CreateDraftRequest {
                format: Some(format),
            };
            http::send("POST", "/api/v1/drafts", Some(&body)).await
        }
    }
}

#[cfg(feature = "hydrate")]
pub async fn load_draft(work_id: String) -> Result<Draft, ApiError> {
    http::get(&format!("/api/v1/drafts/{work_id}")).await
}

#[cfg(feature = "hydrate")]
pub async fn save_draft(
    work_id: String,
    request: SaveDraftRequest,
) -> Result<SavedDraft, ApiError> {
    http::send("PUT", &format!("/api/v1/drafts/{work_id}"), Some(&request)).await
}

/// `GET /api/v1/drafts/{id}/tags` (and its `PUT`), with the work ID encoded as one segment.
pub fn draft_tags_path(work_id: &str) -> String {
    use percent_encoding::utf8_percent_encode;
    format!(
        "/api/v1/drafts/{}/tags",
        utf8_percent_encode(work_id, &DISCUSSION_ID)
    )
}

/// `GET /api/v1/tags/suggestions?q=…&facet=…`: the query is the author's raw text, encoded so
/// it can never add a parameter.
pub fn tag_suggestions_path(q: &str, facet: &str) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    format!(
        "/api/v1/tags/suggestions?q={}&facet={}",
        utf8_percent_encode(q, NON_ALPHANUMERIC),
        utf8_percent_encode(facet, NON_ALPHANUMERIC)
    )
}

/// The active catalog (forms first, at most 200, `truncated` beyond). Public, no actor.
#[cfg(feature = "hydrate")]
pub async fn load_tag_catalog() -> Result<crate::contract::TagList, ApiError> {
    http::get("/api/v1/tags").await
}

#[cfg(feature = "hydrate")]
pub async fn suggest_tags(
    q: &str,
    facet: &str,
) -> Result<crate::contract::TagSuggestions, ApiError> {
    http::get(&tag_suggestions_path(q, facet)).await
}

#[cfg(feature = "hydrate")]
pub async fn load_draft_tags(work_id: &str) -> Result<crate::contract::DraftTags, ApiError> {
    http::get(&draft_tags_path(work_id)).await
}

#[cfg(feature = "hydrate")]
pub async fn save_draft_tags(
    work_id: &str,
    request: &crate::contract::SaveDraftTagsRequest,
) -> Result<crate::contract::SavedDraftTags, ApiError> {
    http::send("PUT", &draft_tags_path(work_id), Some(request)).await
}

/// `GET /api/v1/works/{id}/publication/state` (and the author's three commands below): the work
/// ID is one encoded segment, so it can never add a path part or a query.
fn author_work_path(work_id: &str, tail: &str) -> String {
    use percent_encoding::utf8_percent_encode;
    format!(
        "/api/v1/works/{}/{tail}",
        utf8_percent_encode(work_id, &DISCUSSION_ID)
    )
}

pub fn publication_state_path(work_id: &str) -> String {
    author_work_path(work_id, "publication/state")
}

pub(crate) fn author_decisions_path(work_id: &str) -> String {
    author_work_path(work_id, "publication/decisions")
}

pub(crate) fn author_appeals_path(work_id: &str) -> String {
    author_work_path(work_id, "appeals")
}

pub(crate) fn file_appeal_path(work_id: &str, revision_id: &str) -> String {
    use percent_encoding::utf8_percent_encode;
    author_work_path(
        work_id,
        &format!(
            "revisions/{}/appeals",
            utf8_percent_encode(revision_id, &DISCUSSION_ID)
        ),
    )
}

pub fn snapshots_path(work_id: &str) -> String {
    author_work_path(work_id, "snapshots")
}

pub fn request_public_path(work_id: &str) -> String {
    author_work_path(work_id, "publication/request-public")
}

pub fn set_private_path(work_id: &str) -> String {
    author_work_path(work_id, "publication/set-private")
}

/// The author's own publication state. Private (`no-store`) and read only in the browser.
#[cfg(feature = "hydrate")]
pub async fn load_publication_state(
    work_id: &str,
) -> Result<crate::contract::publication::AuthorPublicationState, ApiError> {
    http::get(&publication_state_path(work_id)).await
}

/// Freezes the saved draft and tags. Every call carries a `command_key`; a replay with the same
/// key and versions returns the first revision and writes nothing.
#[cfg(feature = "hydrate")]
pub async fn capture_snapshot(
    work_id: &str,
    request: &crate::contract::publication::CaptureSnapshotRequest,
) -> Result<crate::contract::publication::CapturedSnapshot, ApiError> {
    http::send("POST", &snapshots_path(work_id), Some(request)).await
}

#[cfg(feature = "hydrate")]
pub async fn request_public(
    work_id: &str,
    request: &crate::contract::publication::RequestPublicRequest,
) -> Result<crate::contract::publication::RequestPublicReceipt, ApiError> {
    http::send("POST", &request_public_path(work_id), Some(request)).await
}

#[cfg(feature = "hydrate")]
pub async fn set_private(
    work_id: &str,
    request: &crate::contract::publication::SetPrivateRequest,
) -> Result<crate::contract::publication::SetPrivateReceipt, ApiError> {
    http::send("POST", &set_private_path(work_id), Some(request)).await
}

/// Calls that end after 30 s: a lost answer to a write is then uncertain, never a rejection.
/// Aborting a request proves nothing about the server, which may have committed it.
#[cfg_attr(not(any(test, feature = "hydrate")), allow(dead_code))]
fn bounded(path: &str) -> bool {
    let publication_command = path.ends_with("/snapshots")
        || path.ends_with("/publication/request-public")
        || path.ends_with("/publication/set-private");
    publication_command
        || path.contains("/moderation/")
        || path.contains("/appeals")
        || path.contains("/publication/state")
        || path.contains("/discussion")
        || path.contains("/endorsement")
        || path.starts_with("/api/v1/author/works")
        || path.starts_with("/api/v1/me/following-feed")
        || path == "/api/v1/me/profile"
        || path.starts_with("/api/v1/me/follows/")
        || path.starts_with("/api/v1/me/relationships/")
        || path.starts_with("/api/v1/authors/")
        || path.starts_with("/api/v1/authors?")
        || path.contains("/tags")
        || path.starts_with("/api/v1/auth/")
}

#[cfg(feature = "hydrate")]
mod http {
    use gloo_net::http::{Method, Request, RequestBuilder, Response};
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    use crate::contract::{ApiError, ErrorEnvelope};
    use crate::session::AuthError;
    use wasm_bindgen::{JsCast, closure::Closure};

    /// One timer per physical HTTP attempt. Dropping the future clears its timer and aborts
    /// fetch; an expired/aborted write is Network (uncertain), never a definitive rejection.
    struct Deadline {
        controller: web_sys::AbortController,
        timer: i32,
        _callback: Closure<dyn FnMut()>,
    }

    impl Deadline {
        fn new() -> Result<Self, ApiError> {
            let controller = web_sys::AbortController::new().map_err(|_| ApiError::Network)?;
            let abort = controller.clone();
            let callback = Closure::wrap(Box::new(move || abort.abort()) as Box<dyn FnMut()>);
            let timer = web_sys::window()
                .ok_or(ApiError::Network)?
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.as_ref().unchecked_ref(),
                    30_000,
                )
                .map_err(|_| ApiError::Network)?;
            Ok(Self {
                controller,
                timer,
                _callback: callback,
            })
        }
    }

    impl Drop for Deadline {
        fn drop(&mut self) {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(self.timer);
            }
            self.controller.abort();
        }
    }

    thread_local! {
        pub(super) static UNAUTHENTICATED: std::cell::RefCell<Option<Box<dyn Fn()>>> =
            const { std::cell::RefCell::new(None) };
    }

    fn is_auth(path: &str) -> bool {
        path.starts_with("/api/v1/auth/")
    }

    use super::bounded;

    fn request(
        method: &str,
        path: &str,
        signal: Option<&web_sys::AbortSignal>,
    ) -> Option<gloo_net::http::RequestBuilder> {
        Some(
            match method {
                "GET" => Request::get(path),
                "POST" => Request::post(path),
                "PUT" => Request::put(path),
                "DELETE" => Request::delete(path),
                _ => return None,
            }
            .abort_signal(signal)
            .header("accept", "application/json"),
        )
    }

    /// One `/auth` request. Errors keep their kind and the server's `Retry-After`; an empty
    /// success body (`204`) is a unit answer.
    pub async fn auth<B: Serialize, T: DeserializeOwned>(
        method: &str,
        path: &str,
        expected: &[u16],
        body: Option<&B>,
    ) -> Result<T, AuthError> {
        let unexpected = AuthError::Unexpected { status: 0 };
        let deadline = Deadline::new().map_err(|_| AuthError::Network)?;
        let signal = deadline.controller.signal();
        let builder = request(method, path, Some(&signal))
            .ok_or(unexpected.clone())?
            .cache(web_sys::RequestCache::NoStore);
        let request = match body {
            Some(body) => builder.json(body),
            None => builder.build(),
        }
        .map_err(|_| unexpected)?;
        let response = request.send().await.map_err(|_| AuthError::Network)?;
        let status = response.status();
        if (200..300).contains(&status) {
            if !expected.contains(&status) {
                return Err(AuthError::Unexpected { status });
            }
            let text = response.text().await.map_err(|_| AuthError::Network)?;
            return crate::contract::decode_success(&text).ok_or(AuthError::Unexpected { status });
        }
        let retry_after = response.headers().get("retry-after");
        let body = response.json::<ErrorEnvelope>().await.ok().map(|e| e.error);
        Err(AuthError::from_response(
            status,
            body.as_ref().map(|b| b.code.as_str()),
            body.as_ref().and_then(|b| b.field.as_deref()),
            retry_after.as_deref(),
        ))
    }

    pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
        let deadline = bounded(path).then(Deadline::new).transpose()?;
        let signal = deadline
            .as_ref()
            .map(|deadline| deadline.controller.signal());
        let response = Request::get(path)
            .abort_signal(signal.as_ref())
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|_| ApiError::Network)?;
        decode(path, response).await
    }

    pub(super) async fn can_read_work_review_queue() -> bool {
        let Ok(deadline) = Deadline::new() else {
            return false;
        };
        // Axum applies the same authorization as GET and strips the response body for HEAD.
        RequestBuilder::new("/api/v1/moderation/works/queue/page?limit=1")
            .method(Method::HEAD)
            .cache(web_sys::RequestCache::NoStore)
            .abort_signal(Some(&deadline.controller.signal()))
            .send()
            .await
            .is_ok_and(|response| response.status() == 200 && !response.redirected())
    }

    pub(super) async fn following_feed(
        path: &str,
        return_scope: Option<&str>,
    ) -> Result<(crate::contract::FeedResponse, Option<String>), ApiError> {
        const RETURN_SCOPE: &str = "x-vink-feed-return-scope";
        let deadline = Deadline::new()?;
        let signal = deadline.controller.signal();
        let mut request = Request::get(path)
            .cache(web_sys::RequestCache::NoStore)
            .abort_signal(Some(&signal))
            .header("accept", "application/json");
        if let Some(scope) = return_scope {
            request = request.header(RETURN_SCOPE, scope);
        }
        let response = request.send().await.map_err(|_| ApiError::Network)?;
        let scope = response.headers().get(RETURN_SCOPE);
        Ok((decode(path, response).await?, scope))
    }

    pub async fn send<B: Serialize, T: DeserializeOwned>(
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, ApiError> {
        let deadline = bounded(path).then(Deadline::new).transpose()?;
        let signal = deadline
            .as_ref()
            .map(|deadline| deadline.controller.signal());
        let builder = request(method, path, signal.as_ref())
            .filter(|_| method != "GET")
            .ok_or(ApiError::Unexpected { status: 0 })?;
        let request = match body {
            Some(body) => builder
                .json(body)
                .map_err(|_| ApiError::Unexpected { status: 0 })?,
            None => builder
                .build()
                .map_err(|_| ApiError::Unexpected { status: 0 })?,
        };
        let response = request.send().await.map_err(|_| ApiError::Network)?;
        decode(path, response).await
    }

    /// A public read. `no-cache` makes the browser revalidate any stored representation with
    /// its validator (READ-01 ETag) before use, so a merged 304 is a real origin confirmation and
    /// never a fresh-cache hit dated "now". The app sends no validator itself; should a raw 304
    /// still arrive, one unconditional `reload` request replaces it (never an empty JSON parse).
    pub async fn public<T: DeserializeOwned>(path: &str) -> crate::freshness::Outcome<T> {
        use crate::freshness::{Outcome, PublicStatus, public_status};
        use web_sys::RequestCache;

        let temporary = Outcome::Temporary { retry_after: None };
        for cache in [RequestCache::NoCache, RequestCache::Reload] {
            let Ok(deadline) = Deadline::new() else {
                return temporary;
            };
            let signal = deadline.controller.signal();
            let Ok(response) = Request::get(path)
                .cache(cache)
                .abort_signal(Some(&signal))
                .header("accept", "application/json")
                .send()
                .await
            else {
                return temporary;
            };
            let status = response.status();
            match public_status(status) {
                PublicStatus::Body => {
                    return response.json::<T>().await.map_or(temporary, Outcome::Fresh);
                }
                PublicStatus::Unconditional => continue,
                PublicStatus::Removed => return Outcome::Removed,
                PublicStatus::Rejected => {
                    let body = response.text().await.ok();
                    let rejection = crate::contract::classify_public_read_bytes(
                        status,
                        body.as_deref().map(str::as_bytes),
                    );
                    return Outcome::Rejected(rejection);
                }
                PublicStatus::Temporary => {
                    let retry_after = response
                        .headers()
                        .get("retry-after")
                        .and_then(|value| crate::freshness::retry_after_ms(&value));
                    return Outcome::Temporary { retry_after };
                }
            }
        }
        temporary
    }

    /// An empty success body (`204`) decodes as a unit answer instead of failing the JSON parse.
    fn authority_needs_refresh(status: u16, envelope: Option<&ErrorEnvelope>) -> bool {
        status == 401
            || (status == 403
                && envelope.is_some_and(|e| {
                    matches!(
                        e.error.code.as_str(),
                        "principal_suspended" | "account_disabled"
                    )
                }))
    }

    async fn decode<T: DeserializeOwned>(path: &str, response: Response) -> Result<T, ApiError> {
        let status = response.status();
        if (200..300).contains(&status) {
            let text = response.text().await.map_err(|_| ApiError::Network)?;
            return crate::contract::decode_success(&text).ok_or(ApiError::Unexpected { status });
        }
        let envelope = response.json::<ErrorEnvelope>().await.ok();
        // Refresh only authority refusals, not an ordinary forbidden operation (e.g. self
        // endorsement). The existing tab owner remains the sole session reader.
        if !is_auth(path) && authority_needs_refresh(status, envelope.as_ref()) {
            UNAUTHENTICATED.with(|slot| {
                if let Some(hook) = slot.borrow().as_ref() {
                    hook();
                }
            });
        }
        Err(ApiError::from_response(status, envelope))
    }
}

#[cfg(test)]
mod tag_path_tests {
    #[test]
    fn tag_paths_keep_the_raw_query_and_the_work_in_their_own_parameter() {
        assert_eq!(
            super::draft_tags_path("33333333-3333-4333-8333-333333333333"),
            "/api/v1/drafts/33333333-3333-4333-8333-333333333333/tags"
        );
        assert_eq!(
            super::draft_tags_path("../x?y"),
            "/api/v1/drafts/%2E%2E%2Fx%3Fy/tags"
        );
        assert_eq!(
            super::tag_suggestions_path("hồi & x=1#", "theme"),
            "/api/v1/tags/suggestions?q=h%E1%BB%93i%20%26%20x%3D1%23&facet=theme"
        );
    }
}

#[cfg(test)]
mod publication_path_tests {
    const WORK: &str = "33333333-3333-4333-8333-333333333333";

    #[test]
    fn the_work_id_is_one_encoded_segment_in_every_publication_path() {
        assert_eq!(
            super::publication_state_path(WORK),
            format!("/api/v1/works/{WORK}/publication/state")
        );
        assert_eq!(
            super::snapshots_path(WORK),
            format!("/api/v1/works/{WORK}/snapshots")
        );
        assert_eq!(
            super::request_public_path(WORK),
            format!("/api/v1/works/{WORK}/publication/request-public")
        );
        assert_eq!(
            super::set_private_path(WORK),
            format!("/api/v1/works/{WORK}/publication/set-private")
        );
        for hostile in ["../x?y=1#z", "a/b", "a b", "%2e%2e", "ế?"] {
            for path in [
                super::publication_state_path(hostile),
                super::snapshots_path(hostile),
                super::request_public_path(hostile),
                super::set_private_path(hostile),
            ] {
                let parts = path.trim_start_matches("/api/v1/works/");
                let (id, _) = parts.split_once('/').expect("a segment then the tail");
                assert!(
                    id.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'%' || b == b'-' || b == b'_'),
                    "{hostile} -> {path}"
                );
                assert!(!path.contains(['?', '#', ' ']), "{path}");
            }
        }
    }

    /// Every publication call has a deadline: the three commands (a lost answer is uncertain,
    /// never a rejection) and the state read. Nothing else is claimed by this list.
    #[test]
    fn every_publication_call_ends_by_deadline() {
        for path in [
            super::publication_state_path(WORK),
            super::snapshots_path(WORK),
            super::request_public_path(WORK),
            super::set_private_path(WORK),
        ] {
            assert!(super::bounded(&path), "{path} has no deadline");
        }
        // The hostile ID cannot move a command out of its class.
        assert!(super::bounded(&super::snapshots_path("x/snapshots?a")));
        // Public reads keep their own owner (READ-02) and are not bounded here.
        assert!(!super::bounded(&format!(
            "/api/v1/works/{WORK}/publication"
        )));
        assert!(!super::bounded("/api/v1/drafts"));
    }
}

#[cfg(test)]
mod discussion_tests {
    #[test]
    fn opaque_cursors_and_identifiers_cannot_change_query_or_scope() {
        assert_eq!(
            super::discussion_path("work-a", Some("root-b"), None),
            "/api/v1/works/work-a/discussion/roots/root-b/replies"
        );
        assert_eq!(
            super::discussion_path("work", Some("root"), Some("c1.a+b&limit=50?")),
            "/api/v1/works/work/discussion/roots/root/replies?cursor=c1%2Ea%2Bb%26limit%3D50%3F"
        );
        assert_eq!(
            super::discussion_path("../other", None, None),
            "/api/v1/works/%2E%2E%2Fother/discussion"
        );
    }

    #[test]
    fn endorsement_paths_encode_each_identifier_and_keep_the_batch_shape() {
        assert_eq!(
            super::endorsements_path("work", false, &[]),
            "/api/v1/works/work/endorsements"
        );
        assert_eq!(
            super::endorsements_path("work", true, &["a-1".into(), "b,2&x=1".into()]),
            "/api/v1/works/work/endorsements/me?comment_ids=a-1,b%2C2%26x%3D1"
        );
        assert_eq!(
            super::endorsement_path("../w", None),
            "/api/v1/works/%2E%2E%2Fw/endorsement/me"
        );
        assert_eq!(
            super::endorsement_path("w", Some("c/1")),
            "/api/v1/works/w/discussion/comments/c%2F1/endorsement/me"
        );
    }
}

#[cfg(test)]
mod separated_discussion_paths {
    #[test]
    fn public_and_own_cursor_queries_cannot_inject_each_other_or_exceed_a_batch() {
        assert_eq!(
            super::public_discussion_path("../work", Some("r/1"), Some("c1.+&ids=x")),
            "/api/v1/works/%2E%2E%2Fwork/discussion/public/roots/r%2F1/replies?cursor=c1%2E%2B%26ids%3Dx"
        );
        assert_eq!(
            super::own_comments_path("w", Some("o1.+&limit=50"), None),
            "/api/v1/works/w/discussion/me?cursor=o1%2E%2B%26limit%3D50"
        );
        assert_eq!(
            super::own_comments_path("w", None, Some(&["a,b".into(), "c&cursor=x".into()])),
            "/api/v1/works/w/discussion/me?ids=a%2Cb,c%26cursor%3Dx"
        );
    }
}

/// Private moderation transports. Called only by LocalResource or explicit intents; no SSR data.
pub(crate) async fn can_read_work_review_queue() -> bool {
    #[cfg(feature = "hydrate")]
    {
        http::can_read_work_review_queue().await
    }
    #[cfg(not(feature = "hydrate"))]
    false
}

pub(crate) async fn moderation_get<T: serde::de::DeserializeOwned>(
    _path: &str,
) -> Result<T, crate::contract::ApiError> {
    #[cfg(feature = "hydrate")]
    {
        http::get(_path).await
    }
    #[cfg(not(feature = "hydrate"))]
    {
        Err(crate::contract::ApiError::Unavailable)
    }
}
pub(crate) async fn moderation_post(
    _path: &str,
    _body: &serde_json::Value,
) -> Result<serde_json::Value, crate::contract::ApiError> {
    #[cfg(feature = "hydrate")]
    {
        http::send("POST", _path, Some(_body)).await
    }
    #[cfg(not(feature = "hydrate"))]
    {
        Err(crate::contract::ApiError::Unavailable)
    }
}

pub(crate) fn author_resource(
    name: String,
    cursor: Option<String>,
    version: ReadSignal<u64>,
) -> Resource<(u64, Result<crate::contract::AuthorResolution, LoadError>)> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || version.get(),
            move |generation| {
                let port = port.clone();
                let name = name.clone();
                let cursor = cursor.clone();
                async move {
                    let result = match port {
                        Some(port) => port.author(name, cursor).await,
                        None => Err(LoadError::Unavailable),
                    };
                    (generation, result)
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        let mut path = format!(
            "/api/v1/authors/{}",
            percent_encoding::utf8_percent_encode(&name, &DISCUSSION_ID)
        );
        if let Some(cursor) = cursor {
            path.push_str("?cursor=");
            path.push_str(
                &percent_encoding::utf8_percent_encode(&cursor, &DISCUSSION_ID).to_string(),
            );
        }
        Resource::new(
            move || version.get(),
            move |generation| {
                let path = path.clone();
                browser(async move { (generation, public_get(path).await) })
            },
        )
    }
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_own_profile() -> Result<crate::contract::profile::OwnProfile, ApiError> {
    http::get("/api/v1/me/profile").await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn save_profile(
    request: &crate::contract::profile::SaveProfile,
) -> Result<crate::contract::profile::OwnProfile, ApiError> {
    http::send("PUT", "/api/v1/me/profile", Some(request)).await
}

fn encode_segment(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, &DISCUSSION_ID).to_string()
}

pub fn author_relationships_path(
    handle: &str,
    direction: crate::contract::follow::FollowDirection,
    cursor: Option<&str>,
) -> String {
    let mut path = format!(
        "/api/v1/authors/{}/{}",
        encode_segment(handle),
        direction.api_segment()
    );
    if let Some(cursor) = cursor {
        path.push_str("?cursor=");
        path.push_str(&encode_segment(cursor));
    }
    path
}

pub fn viewer_relationship_path(handle: &str) -> String {
    format!("/api/v1/authors/{}/relationship/me", encode_segment(handle))
}

pub fn follow_receipt_path(command_id: &str) -> String {
    format!("/api/v1/me/follows/receipts/{}", encode_segment(command_id))
}

pub fn own_relationships_path(
    direction: crate::contract::follow::FollowDirection,
    cursor: Option<&str>,
) -> String {
    let mut path = format!("/api/v1/me/relationships/{}", direction.api_segment());
    if let Some(cursor) = cursor {
        path.push_str("?cursor=");
        path.push_str(&encode_segment(cursor));
    }
    path
}

pub fn follow_membership_path(handles: &[String]) -> String {
    let encoded: Vec<String> = handles.iter().map(|h| encode_segment(h)).collect();
    format!(
        "/api/v1/me/follows/membership?handles={}",
        encoded.join(",")
    )
}

pub(crate) fn author_relationships_resource(
    name: String,
    direction: crate::contract::follow::FollowDirection,
    cursor: Option<String>,
    version: ReadSignal<u64>,
) -> Resource<(
    u64,
    Result<crate::contract::follow::AuthorRelationshipsLoad, LoadError>,
)> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || version.get(),
            move |generation| {
                let port = port.clone();
                let name = name.clone();
                let cursor = cursor.clone();
                async move {
                    let result = match port {
                        Some(port) => port.author_relationships(name, direction, cursor).await,
                        None => Err(LoadError::Unavailable),
                    };
                    (generation, result)
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        let path = author_relationships_path(&name, direction, cursor.as_deref());
        Resource::new(
            move || version.get(),
            move |generation| {
                let path = path.clone();
                browser(async move {
                    #[cfg(feature = "hydrate")]
                    {
                        let result = match http::get::<
                            crate::contract::follow::PublicRelationshipList,
                        >(&path)
                        .await
                        {
                            Ok(list) => Ok(list.into()),
                            Err(ApiError::RelationshipListPrivate) => {
                                Ok(crate::contract::follow::AuthorRelationshipsLoad::Private)
                            }
                            Err(ApiError::ProfileGone) => {
                                Ok(crate::contract::follow::AuthorRelationshipsLoad::Gone)
                            }
                            Err(ApiError::NotFound) => {
                                Ok(crate::contract::follow::AuthorRelationshipsLoad::Missing)
                            }
                            Err(ApiError::Invalid { field }) => {
                                Err(LoadError::Invalid(crate::freshness::classify_rejection(
                                    422,
                                    Some("validation_failed"),
                                    field.as_deref(),
                                )))
                            }
                            Err(_) => Err(LoadError::Unavailable),
                        };
                        (generation, result)
                    }
                    #[cfg(not(feature = "hydrate"))]
                    {
                        let _ = &path;
                        (generation, Err(LoadError::Unavailable))
                    }
                })
            },
        )
    }
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_viewer_relationship(
    handle: &str,
) -> Result<crate::contract::follow::ViewerRelationship, ApiError> {
    http::get(&viewer_relationship_path(handle)).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn set_follow(
    handle: &str,
    request: &crate::contract::follow::SetFollowRequest,
) -> Result<crate::contract::follow::SetFollowResponse, ApiError> {
    http::send("PUT", &viewer_relationship_path(handle), Some(request)).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_follow_receipt(
    command_id: &str,
) -> Result<crate::contract::follow::FollowReceipt, ApiError> {
    http::get(&follow_receipt_path(command_id)).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_public_relationships(
    handle: &str,
    direction: crate::contract::follow::FollowDirection,
    cursor: Option<&str>,
) -> Result<crate::contract::follow::PublicRelationshipList, ApiError> {
    http::get(&author_relationships_path(handle, direction, cursor)).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_own_relationships(
    direction: crate::contract::follow::FollowDirection,
    cursor: Option<&str>,
) -> Result<crate::contract::follow::OwnRelationshipList, ApiError> {
    http::get(&own_relationships_path(direction, cursor)).await
}

#[cfg(feature = "hydrate")]
pub(crate) async fn load_author_relationships_summary(
    handle: &str,
) -> Result<crate::contract::follow::PublicRelationships, ApiError> {
    let path = format!("/api/v1/authors/{}", encode_segment(handle));
    match http::get::<crate::contract::AuthorResolution>(&path).await? {
        crate::contract::AuthorResolution::Current {
            author,
            relationships,
            ..
        } if author.handle == handle && author.valid_href().is_some() => Ok(*relationships),
        crate::contract::AuthorResolution::Gone => Err(ApiError::ProfileGone),
        crate::contract::AuthorResolution::Missing
        | crate::contract::AuthorResolution::Current { .. }
        | crate::contract::AuthorResolution::Alias { .. } => Err(ApiError::NotFound),
    }
}

#[cfg(test)]
mod follow_path_tests {
    use super::*;
    use crate::contract::follow::FollowDirection;

    #[test]
    fn follow_paths_encode_segments_and_cursors() {
        assert_eq!(
            author_relationships_path("an-may", FollowDirection::Followers, None),
            "/api/v1/authors/an-may/followers"
        );
        assert_eq!(
            author_relationships_path("../x", FollowDirection::Following, Some("c+1&a=b")),
            "/api/v1/authors/%2E%2E%2Fx/following?cursor=c%2B1%26a%3Db"
        );
        assert_eq!(
            viewer_relationship_path("an-may"),
            "/api/v1/authors/an-may/relationship/me"
        );
        assert_eq!(
            follow_receipt_path("0192f3a1-0000-7000-8000-000000000294"),
            "/api/v1/me/follows/receipts/0192f3a1-0000-7000-8000-000000000294"
        );
        assert_eq!(
            own_relationships_path(FollowDirection::Followers, Some("next")),
            "/api/v1/me/relationships/followers?cursor=next"
        );
        assert_eq!(
            follow_membership_path(&["an-may".into(), "binh-yen".into()]),
            "/api/v1/me/follows/membership?handles=an-may,binh-yen"
        );
    }
}
