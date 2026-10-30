//! Data access for views. Public reads resolve in-process on the server (through
//! [`crate::server::PublicReads`]) and over `/api/v1` in the browser; both yield the same
//! contract types. Desk calls exist only in the browser.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "hydrate")]
use crate::contract::{ApiError, Draft, DraftList, SaveDraftRequest, SavedDraft};
use crate::contract::{Publication, WorkList};

/// Public read failures; serialized into the hydration payload with the page data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadError {
    NotFound,
    Unavailable,
}

pub fn works_resource() -> Resource<Result<WorkList, LoadError>> {
    #[cfg(feature = "ssr")]
    {
        let port = use_context::<crate::server::SharedPublicReads>();
        Resource::new(
            || (),
            move |()| {
                let port = port.clone();
                async move {
                    match port {
                        Some(port) => port.works().await,
                        None => Err(LoadError::Unavailable),
                    }
                }
            },
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Resource::new(|| (), |()| browser(public_get("/api/v1/works".to_string())))
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
            |id| browser(public_get(format!("/api/v1/works/{id}/publication"))),
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

#[cfg(not(feature = "ssr"))]
async fn public_get<T: serde::de::DeserializeOwned>(path: String) -> Result<T, LoadError> {
    #[cfg(feature = "hydrate")]
    {
        http::get(&path).await.map_err(|error| match error {
            ApiError::NotFound => LoadError::NotFound,
            _ => LoadError::Unavailable,
        })
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = path;
        Err(LoadError::Unavailable)
    }
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

#[cfg(feature = "hydrate")]
mod http {
    use gloo_net::http::{Request, Response};
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    use crate::contract::{ApiError, ErrorEnvelope};

    pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
        let response = Request::get(path)
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
        let builder = match method {
            "POST" => Request::post(path),
            "PUT" => Request::put(path),
            _ => return Err(ApiError::Unexpected { status: 0 }),
        }
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
