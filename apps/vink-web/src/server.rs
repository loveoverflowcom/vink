//! Server-side seam (feature `ssr`). The composition root (`vink-app`) implements
//! [`PublicReads`] over its application layer and provides it through Leptos context, so SSR
//! reads in-process without an HTTP round trip and this crate never imports the service.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::api::LoadError;
use crate::contract::{DiscussionPage, Publication, WorkList};

pub type PortFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Public reads available to server rendering. There is deliberately no draft method:
/// private desk data is only fetched by the browser through the authenticated API.
pub trait PublicReads: Send + Sync + 'static {
    fn works(&self) -> PortFuture<Result<WorkList, LoadError>>;
    fn publication(&self, work_id: String) -> PortFuture<Result<Publication, LoadError>>;
    /// Always anonymous: viewer-private comments never enter public HTML/hydration payloads.
    fn discussion(&self, _work_id: String) -> PortFuture<Result<DiscussionPage, LoadError>> {
        Box::pin(std::future::ready(Err(LoadError::Unavailable)))
    }
}

pub type SharedPublicReads = Arc<dyn PublicReads>;

/// Public origin (scheme://host[:port]) for canonical URLs rendered on the server.
#[derive(Debug, Clone)]
pub struct SiteOrigin(pub String);

/// Opaque, non-authoritative marker supplied only by the existing local identity fixture.
/// Not a credential, principal ID, production session or permission.
#[derive(Debug, Clone)]
pub struct LocalDiscussionFixture(pub String);
