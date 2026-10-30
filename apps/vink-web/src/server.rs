//! Server-side seam (feature `ssr`). The composition root (`vink-app`) implements
//! [`PublicReads`] over its application layer and provides it through Leptos context, so SSR
//! reads in-process without an HTTP round trip and this crate never imports the service.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::api::LoadError;
use crate::contract::{Publication, WorkList};

pub type PortFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Public reads available to server rendering. There is deliberately no draft method:
/// private desk data is only fetched by the browser through the authenticated API.
pub trait PublicReads: Send + Sync + 'static {
    fn works(&self) -> PortFuture<Result<WorkList, LoadError>>;
    fn publication(&self, work_id: String) -> PortFuture<Result<Publication, LoadError>>;
}

pub type SharedPublicReads = Arc<dyn PublicReads>;

/// Public origin (scheme://host[:port]) for canonical URLs rendered on the server.
#[derive(Debug, Clone)]
pub struct SiteOrigin(pub String);
