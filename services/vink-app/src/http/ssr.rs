//! Leptos SSR host. Server rendering reads through the same application facade and the same
//! v1 wire representation as the API, in-process and without an HTTP round trip.

use std::sync::Arc;

use axum::Router;
use leptos::prelude::provide_context;
use leptos_axum::{LeptosRoutes, file_and_error_handler_with_context, generate_route_list};
use serde::Serialize;
use serde::de::DeserializeOwned;
use vink_web::api::LoadError;
use vink_web::contract as web;
use vink_web::server::{
    LocalDiscussionFixture, PortFuture, PublicReads, SharedPublicReads, SignInOpen, SiteOrigin,
};
use vink_web::{App, shell};

use super::AppState;
use super::contract::{Publication, WorkList};
use crate::application::author_discovery::DiscoverAuthors;
use crate::application::discovery::{DiscoverWorks, DiscoveryError, DiscoveryInput};
use crate::application::{ReadError, Vink};
use crate::domain::ids::WorkId;
use crate::persistence::PgStore;

struct PublicReadsAdapter {
    vink: Arc<Vink<PgStore>>,
}

/// Converts the provider's wire type into the web app's own type through JSON, so SSR and
/// browser fetches see exactly the representation the API serves.
fn to_web<P: Serialize, W: DeserializeOwned>(value: &P) -> Result<W, LoadError> {
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|error| {
            tracing::error!(%error, "SSR contract conversion failed");
            LoadError::Unavailable
        })
}

fn load_error(error: ReadError) -> LoadError {
    match error {
        ReadError::NotFound => LoadError::NotFound,
        ReadError::Unavailable | ReadError::Internal => LoadError::Unavailable,
    }
}

fn discovery_load_error(error: DiscoveryError) -> LoadError {
    use vink_web::api::{InvalidInputField, InvalidRead};
    match error {
        DiscoveryError::Invalid(input) => LoadError::Invalid(match input {
            DiscoveryInput::Cursor(_) | DiscoveryInput::CursorScope => InvalidRead::RestartCursor,
            DiscoveryInput::Query(_) => InvalidRead::InvalidInput(InvalidInputField::Query),
            DiscoveryInput::Tag(_) => InvalidRead::InvalidInput(InvalidInputField::Tag),
            DiscoveryInput::Limit(_) => InvalidRead::InvalidInput(InvalidInputField::Limit),
        }),
        DiscoveryError::Unavailable | DiscoveryError::Internal => LoadError::Unavailable,
    }
}

impl PublicReads for PublicReadsAdapter {
    fn author(
        &self,
        handle: String,
        cursor: Option<String>,
    ) -> PortFuture<Result<web::AuthorResolution, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            to_web(
                &vink
                    .store()
                    .resolve_handle(&handle, cursor.as_deref())
                    .await
                    .map_err(|_| LoadError::Unavailable)?,
            )
        })
    }
    fn discussion(
        &self,
        work_id: String,
    ) -> PortFuture<Result<web::PublicDiscussionPage, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let id = WorkId::parse(&work_id).ok_or(LoadError::NotFound)?;
            // Deliberately no request actor here, including with the development fixture.
            let page =
                vink.public_discussion(id, None, None)
                    .await
                    .map_err(|error| match error {
                        crate::application::discussion::DiscussionError::NotFound => {
                            LoadError::NotFound
                        }
                        _ => LoadError::Unavailable,
                    })?;
            to_web(&super::contract::PublicDiscussionPage::new(id, &page))
        })
    }
    fn endorsement_summaries(
        &self,
        work_id: String,
        comment_ids: Vec<String>,
    ) -> PortFuture<Result<web::EndorsementSummaries, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let id = WorkId::parse(&work_id).ok_or(LoadError::NotFound)?;
            // Public counts only: no actor is resolved, as for `GET …/endorsements`.
            let ids = (!comment_ids.is_empty()).then(|| comment_ids.join(","));
            let summaries = vink
                .endorsement_summaries(id, ids.as_deref())
                .await
                .map_err(|error| match error {
                    crate::application::endorsement::EndorsementError::NotFound => {
                        LoadError::NotFound
                    }
                    _ => LoadError::Unavailable,
                })?;
            to_web(&super::contract::EndorsementSummaryList::from(&summaries))
        })
    }
    fn catalog(&self) -> PortFuture<Result<web::TagList, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let page = vink
                .catalog_tags(None)
                .await
                .map_err(|_| LoadError::Unavailable)?;
            to_web(&super::contract::TagList::from(&page))
        })
    }

    fn authors(
        &self,
        q: String,
        cursor: Option<String>,
    ) -> PortFuture<Result<web::PublicAuthorPage, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let page = vink
                .store()
                .discover_authors(DiscoverAuthors {
                    q: Some(q),
                    cursor,
                    limit: None,
                })
                .await
                .map_err(discovery_load_error)?;
            to_web(&page)
        })
    }

    fn works(
        &self,
        query: vink_web::discovery::ExploreQuery,
    ) -> PortFuture<Result<web::WorkList, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            // The home page is the first page of the unfiltered public search (ED-01: finite).
            let page = vink
                .discover_works(DiscoverWorks {
                    query: Some(query.q),
                    tags: query.tags,
                    cursor: query.cursor,
                    limit: None,
                })
                .await
                .map_err(discovery_load_error)?;
            to_web(&WorkList::from(&page))
        })
    }

    fn publication(&self, work_id: String) -> PortFuture<Result<web::Publication, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let id = WorkId::parse(&work_id).ok_or(LoadError::NotFound)?;
            let work = vink.publication(id).await.map_err(load_error)?;
            to_web(&Publication::from(&work))
        })
    }

    fn reader(&self, work_id: String) -> PortFuture<Result<web::ReaderResponse, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let id = WorkId::parse(&work_id).ok_or(LoadError::NotFound)?;
            let reader_work = vink.reader_work(id).await.map_err(load_error)?;
            let resp = super::contract::ReaderResponse::from(reader_work);
            to_web(&resp)
        })
    }

    fn author_relationships(
        &self,
        handle: String,
        direction: web::follow::FollowDirection,
        cursor: Option<String>,
    ) -> PortFuture<Result<web::follow::AuthorRelationshipsLoad, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let domain_dir = match direction {
                web::follow::FollowDirection::Followers => {
                    crate::domain::follow::FollowDirection::Followers
                }
                web::follow::FollowDirection::Following => {
                    crate::domain::follow::FollowDirection::Following
                }
            };
            match vink
                .store()
                .public_relationships(&handle, domain_dir, Ok(cursor))
                .await
            {
                Ok(list) => {
                    let wire: web::follow::PublicRelationshipList = to_web(&list)?;
                    Ok(wire.into())
                }
                Err(crate::application::follow::FollowError::ListPrivate) => {
                    Ok(web::follow::AuthorRelationshipsLoad::Private)
                }
                Err(crate::application::follow::FollowError::Gone) => {
                    Ok(web::follow::AuthorRelationshipsLoad::Gone)
                }
                Err(crate::application::follow::FollowError::NotFound) => {
                    Ok(web::follow::AuthorRelationshipsLoad::Missing)
                }
                Err(_) => Err(LoadError::Unavailable),
            }
        })
    }
}

pub fn routes(state: &AppState) -> Router<AppState> {
    let reads: SharedPublicReads = Arc::new(PublicReadsAdapter {
        vink: state.vink.clone(),
    });
    let origin = SiteOrigin(state.public_origin.to_string());
    let fixture = state.identity.local_discussion_fixture_scope();
    let sign_in = SignInOpen(state.sign_in.is_some() && state.sign_in_enabled);
    let context = move || {
        provide_context(reads.clone());
        provide_context(origin.clone());
        provide_context(sign_in);
        if let Some(scope) = &fixture {
            provide_context(LocalDiscussionFixture(scope.clone()));
        }
    };
    let options = state.leptos.clone();
    Router::new()
        .leptos_routes_with_context(
            state,
            generate_route_list(App),
            context.clone(),
            move || shell(options.clone()),
        )
        // Serves `site_root` files (the built `pkg/`), otherwise renders the not-found page.
        .fallback(file_and_error_handler_with_context::<AppState, _>(
            context, shell,
        ))
}
