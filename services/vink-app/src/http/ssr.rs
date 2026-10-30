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
use vink_web::server::{PortFuture, PublicReads, SharedPublicReads, SiteOrigin};
use vink_web::{App, shell};

use super::AppState;
use super::contract::{Publication, WorkCard, WorkList};
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

impl PublicReads for PublicReadsAdapter {
    fn works(&self) -> PortFuture<Result<web::WorkList, LoadError>> {
        let vink = self.vink.clone();
        Box::pin(async move {
            let works = vink.public_works().await.map_err(load_error)?;
            to_web(&WorkList {
                works: works.iter().map(WorkCard::from).collect(),
            })
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
}

pub fn routes(state: &AppState) -> Router<AppState> {
    let reads: SharedPublicReads = Arc::new(PublicReadsAdapter {
        vink: state.vink.clone(),
    });
    let origin = SiteOrigin(state.public_origin.to_string());
    let context = move || {
        provide_context(reads.clone());
        provide_context(origin.clone());
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
