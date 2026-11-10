//! Data access for views. Public reads resolve in-process on the server (through
//! [`crate::server::PublicReads`]) and over `/api/v1` in the browser; both yield the same
//! contract types. Desk calls exist only in the browser.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "hydrate")]
use crate::contract::{ApiError, Draft, DraftList, SaveDraftRequest, SavedDraft};
use crate::contract::{DiscussionPage, EndorsementSummaries, Publication, WorkList};

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

#[cfg(feature = "hydrate")]
pub(crate) async fn current_account() -> Result<crate::contract::CurrentAccount, ApiError> {
    http::get("/api/v1/auth/session/account").await
}

/// The anonymous discussion page and the public endorsement summaries (LVE #73) of its public
/// comments, read one after the other so SSR and hydration render the same counts. A failed
/// summary read never hides the discussion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscussionLoad {
    pub page: DiscussionPage,
    pub endorsements: Result<EndorsementSummaries, LoadError>,
}

fn first_batch(page: &DiscussionPage) -> Vec<String> {
    // A first page has at most 20 roots with 3 preview replies each (80 < 100); later pages
    // are read in batches by the browser adapter.
    crate::endorsement_state::page_targets(page)
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
                    let page: DiscussionPage =
                        public_get(format!("/api/v1/works/{id}/discussion")).await?;
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

#[cfg(feature = "hydrate")]
pub async fn load_discussion(work: &str, cursor: Option<&str>) -> Result<DiscussionPage, ApiError> {
    http::get(&discussion_path(work, None, cursor)).await
}

#[cfg(feature = "hydrate")]
pub async fn load_replies(
    work: &str,
    root: &str,
    cursor: Option<&str>,
) -> Result<crate::contract::ReplyList, ApiError> {
    http::get(&discussion_path(work, Some(root), cursor)).await
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

/// Public read failures; serialized into the hydration payload with the page data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadError {
    NotFound,
    Invalid,
    Unavailable,
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
                            Ok(crate::public_reads::Snapshot::Publication(_)) => {
                                Err(LoadError::Unavailable)
                            }
                            Err(error) => Err(error),
                        };
                    (query, result)
                })
            },
        )
    }
}

/// Public catalog is no-store, refreshed for each applied query and when opening the picker.
pub fn catalog_resource(
    refresh: Memo<(crate::discovery::ExploreQuery, u64)>,
) -> Resource<Result<crate::contract::TagList, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            move || refresh.get(),
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
        Resource::new(
            move || refresh.get(),
            |_| browser(public_get("/api/v1/tags".into())),
        )
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
                    match public(crate::public_reads::PublicKey::Publication(id)).await? {
                        crate::public_reads::Snapshot::Publication(p) => Ok(p),
                        crate::public_reads::Snapshot::Works(_) => Err(LoadError::Unavailable),
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
            ApiError::Invalid { .. } => LoadError::Invalid,
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

#[cfg(feature = "hydrate")]
pub async fn create_draft() -> Result<Draft, ApiError> {
    http::send::<(), Draft>("POST", "/api/v1/drafts", None).await
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

#[cfg(feature = "hydrate")]
mod http {
    use gloo_net::http::{Request, Response};
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    use crate::contract::{ApiError, ErrorEnvelope};
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

    /// These calls end after 30 s. For writes, a lost answer is then uncertain.
    fn bounded(path: &str) -> bool {
        path.contains("/discussion")
            || path.contains("/endorsement")
            || path.contains("/tags")
            || path == "/api/v1/auth/session/account"
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
        decode(response).await
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
        let builder = match method {
            "POST" => Request::post(path),
            "PUT" => Request::put(path),
            "DELETE" => Request::delete(path),
            _ => return Err(ApiError::Unexpected { status: 0 }),
        }
        .abort_signal(signal.as_ref())
        .header("accept", "application/json");
        let request = match body {
            Some(body) => builder
                .json(body)
                .map_err(|_| ApiError::Unexpected { status: 0 })?,
            None => builder
                .build()
                .map_err(|_| ApiError::Unexpected { status: 0 })?,
        };
        let response = request.send().await.map_err(|_| ApiError::Network)?;
        decode(response).await
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
            match public_status(response.status()) {
                PublicStatus::Body => {
                    return response.json::<T>().await.map_or(temporary, Outcome::Fresh);
                }
                PublicStatus::Unconditional => continue,
                PublicStatus::Removed => return Outcome::Removed,
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

    async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
        let status = response.status();
        if (200..300).contains(&status) {
            return response
                .json::<T>()
                .await
                .map_err(|_| ApiError::Unexpected { status });
        }
        let envelope = response.json::<ErrorEnvelope>().await.ok();
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
