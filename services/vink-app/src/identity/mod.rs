//! Server-side principal resolution.
//!
//! A request yields a [`Resolution`]: a principal decided on the server, `Anonymous`, or
//! `Unavailable` when the identity dependency could not decide. Private routes answer `401` for
//! `Anonymous` and `503 dependency_unavailable` for `Unavailable`; public routes never resolve.
//! Request bodies, query strings and headers such as `x-user-id` are never evidence of identity.
//!
//! The production auth authority is still open (D-05 / S-03). [`IdentityService`] consults the
//! separate Identity process (`services/vink-identity`) through the App's own client; it forwards a
//! presented credential (`Authorization: Bearer`, a provisional carrier until D-05 picks a session
//! model) and never maps an authenticated subject to a principal, because no binding exists yet.
//! [`NoAuthority`] closes everything. [`LocalFixture`] is a development stand-in that exists only
//! with `VINK_PROFILE=local`, a loopback bind and a loopback peer; it proves nothing about ACC-01,
//! ACC-02 or S-03.

pub mod client;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use axum::http::header;
use axum::http::request::Parts;

pub use client::IdentityClient;
use client::{CallError, Verification};

use crate::config::{Config, Profile};
use crate::domain::ids::PrincipalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The server decided that this request acts as this principal.
    Principal(PrincipalId),
    /// No principal: no credential, no authority, a rejected credential or an unbound subject.
    Anonymous,
    /// The identity dependency could not decide; nothing may be granted or denied as final.
    Unavailable,
}

pub type ResolveFuture<'a> = Pin<Box<dyn Future<Output = Resolution> + Send + 'a>>;

pub trait IdentityResolver: Send + Sync + 'static {
    /// Decides who acts. `peer` is the TCP peer when known.
    fn resolve<'a>(&'a self, parts: &'a Parts, peer: Option<SocketAddr>) -> ResolveFuture<'a>;
    /// Diagnostic host hint only. Production resolvers never expose a marker. It grants
    /// nothing: each API request still resolves its actor and enforces the loopback peer.
    fn local_discussion_fixture_scope(&self) -> Option<String> {
        None
    }
}

/// No auth authority configured: private routes stay closed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAuthority;

impl IdentityResolver for NoAuthority {
    fn resolve<'a>(&'a self, _parts: &'a Parts, _peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(std::future::ready(Resolution::Anonymous))
    }
}

/// Treats loopback requests as one configured development principal.
#[derive(Debug, Clone, Copy)]
pub struct LocalFixture {
    principal: PrincipalId,
}

impl IdentityResolver for LocalFixture {
    fn local_discussion_fixture_scope(&self) -> Option<String> {
        // Per-process marker, unrelated to the principal's UUID or display name.
        static MARKERS: std::sync::OnceLock<
            std::sync::Mutex<std::collections::BTreeMap<PrincipalId, String>>,
        > = std::sync::OnceLock::new();
        let mut markers = MARKERS.get_or_init(Default::default).lock().ok()?;
        Some(
            markers
                .entry(self.principal)
                .or_insert_with(|| uuid::Uuid::now_v7().to_string())
                .clone(),
        )
    }
    fn resolve<'a>(&'a self, _parts: &'a Parts, peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        let resolution = match peer.filter(|addr| addr.ip().is_loopback()) {
            Some(_) => Resolution::Principal(self.principal),
            None => Resolution::Anonymous,
        };
        Box::pin(std::future::ready(resolution))
    }
}

/// Consults the Identity process for a presented credential. With no authority there, or with no
/// principal binding in the App (not implemented), every outcome is `Anonymous` or `Unavailable`.
#[derive(Debug, Clone)]
pub struct IdentityService {
    client: IdentityClient,
}

impl IdentityService {
    pub fn new(client: IdentityClient) -> Self {
        Self { client }
    }
}

/// The credential a browser presents: `Authorization: Bearer <token>`, non-empty and within the
/// contract's size. Anything else is no credential (anonymous), not an outage.
fn presented_credential(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?.trim();
    (!token.is_empty() && token.len() <= client::MAX_CREDENTIAL_BYTES).then_some(token)
}

impl IdentityResolver for IdentityService {
    fn resolve<'a>(&'a self, parts: &'a Parts, _peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(async move {
            let Some(credential) = presented_credential(parts) else {
                return Resolution::Anonymous;
            };
            match self.client.verify(credential).await {
                Ok(Verification::Authenticated { authority, .. }) => {
                    tracing::info!(
                        %authority,
                        "authenticated subject has no Vink principal binding (not implemented)"
                    );
                    Resolution::Anonymous
                }
                Ok(Verification::Rejected) | Err(CallError::NoAuthority) => Resolution::Anonymous,
                Err(CallError::Unavailable(failure)) => {
                    tracing::warn!(
                        reason = failure.label(),
                        "identity service could not decide"
                    );
                    Resolution::Unavailable
                }
            }
        })
    }
}

/// Chooses the resolver for a validated configuration. The local fixture, when allowed, wins:
/// development keeps a usable desk while the Identity process is only probed for readiness.
pub fn from_config(config: &Config) -> Box<dyn IdentityResolver> {
    match (config.profile, config.dev_principal, &config.identity) {
        (Profile::Local, Some(principal), _) if config.bind.ip().is_loopback() => {
            Box::new(LocalFixture { principal })
        }
        (_, _, Some(identity)) => Box::new(IdentityService::new(IdentityClient::new(
            identity.socket.clone(),
            identity.server_uid,
        ))),
        _ => Box::new(NoAuthority),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    fn parts() -> Parts {
        Request::builder()
            .header("x-vink-principal", "11111111-1111-4111-8111-111111111111")
            .header("x-user-id", "11111111-1111-4111-8111-111111111111")
            .body(())
            .unwrap()
            .into_parts()
            .0
    }

    #[tokio::test]
    async fn default_authority_ignores_client_supplied_identity() {
        assert!(NoAuthority.local_discussion_fixture_scope().is_none());
        assert_eq!(
            NoAuthority
                .resolve(&parts(), Some(([127, 0, 0, 1], 9).into()))
                .await,
            Resolution::Anonymous
        );
    }

    #[tokio::test]
    async fn fixture_requires_a_loopback_peer() {
        let id = PrincipalId::parse("22222222-2222-4222-8222-222222222222").unwrap();
        let fixture = LocalFixture { principal: id };
        let marker = fixture.local_discussion_fixture_scope().unwrap();
        assert_ne!(marker, id.to_string());
        assert_eq!(
            fixture.local_discussion_fixture_scope(),
            Some(marker.clone())
        );
        let other = LocalFixture {
            principal: PrincipalId::from_uuid(uuid::Uuid::now_v7()),
        };
        assert_ne!(other.local_discussion_fixture_scope(), Some(marker));
        assert_eq!(
            fixture
                .resolve(&parts(), Some(([127, 0, 0, 1], 9).into()))
                .await,
            Resolution::Principal(id)
        );
        assert_eq!(
            fixture
                .resolve(&parts(), Some(([192, 168, 1, 5], 9).into()))
                .await,
            Resolution::Anonymous
        );
        assert_eq!(fixture.resolve(&parts(), None).await, Resolution::Anonymous);
    }

    #[test]
    fn only_a_bearer_token_is_a_presented_credential() {
        let with = |value: &str| {
            Request::builder()
                .header(header::AUTHORIZATION, value)
                .body(())
                .unwrap()
                .into_parts()
                .0
        };
        assert_eq!(presented_credential(&with("Bearer abc")), Some("abc"));
        assert_eq!(presented_credential(&with("Bearer ")), None);
        assert_eq!(presented_credential(&with("Basic dXNlcg==")), None);
        let long = format!("Bearer {}", "t".repeat(client::MAX_CREDENTIAL_BYTES + 1));
        assert_eq!(presented_credential(&with(&long)), None);
        assert_eq!(presented_credential(&parts()), None);
    }
}
