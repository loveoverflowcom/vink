//! Server-side principal resolution.
//!
//! A request yields a [`Resolution`]: a principal decided on the server, `Anonymous`, an
//! authenticated subject without a principal (`Unbound`), a `Suspended` principal, or
//! `Unavailable` when a dependency could not decide. Only `Principal` grants anything: private
//! routes answer `401` for the three non-principal outcomes and `503 dependency_unavailable` for
//! `Unavailable`; public routes never resolve. Request bodies, query strings and headers such as
//! `x-user-id` are never evidence of identity.
//!
//! [`IdentityService`] (D-05/S-03 spike) asks the separate Identity process whether the presented
//! session credential is valid — the `__Host-vink_session` HttpOnly cookie for the Web, or
//! `Authorization: Bearer` for non-browser clients — then looks the authenticated subject up in
//! the App's own binding table and suspension list ([`PrincipalDirectory`]). Identity never names a
//! principal and the App never sees an email address. [`NoAuthority`] closes everything.
//! [`LocalFixture`] is a development stand-in that exists only with `VINK_PROFILE=local`, a
//! loopback bind and a loopback peer; it proves nothing about ACC-01, ACC-02 or S-03.

pub mod client;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use axum::http::header;
use axum::http::request::Parts;

pub use client::IdentityClient;
use client::{CallError, Verification};

use crate::application::Vink;
use crate::application::ports::{BoundPrincipal, PrincipalStore};
use crate::config::{Config, Profile};
use crate::domain::ids::PrincipalId;
use crate::domain::principal::{Standing, SubjectKey};

/// The Web session cookie. `__Host-` forces `Secure`, `Path=/` and no `Domain`, so neither a
/// subdomain nor plain HTTP can plant or read it; `HttpOnly` keeps it from page scripts.
pub const SESSION_COOKIE: &str = "__Host-vink_session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The server decided that this request acts as this principal.
    Principal(PrincipalId),
    /// No principal: no credential, no authority, or a rejected/expired/revoked credential.
    Anonymous,
    /// Identity authenticated this subject, but no Vink principal is bound to it yet. Grants
    /// nothing until `POST /api/v1/auth/principal` provisions one.
    Unbound(SubjectKey),
    /// The bound principal is suspended by Vink. Grants nothing, whatever Identity says.
    Suspended(PrincipalId),
    /// A dependency could not decide; nothing may be granted or denied as final.
    Unavailable,
}

impl Resolution {
    /// The only outcome that lets a request act.
    pub fn principal(&self) -> Option<PrincipalId> {
        match self {
            Resolution::Principal(principal) => Some(*principal),
            _ => None,
        }
    }
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

/// The App's binding table and suspension list, as seen by the resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectoryUnavailable;

pub type DirectoryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<BoundPrincipal>, DirectoryUnavailable>> + Send + 'a>>;

pub trait PrincipalDirectory: Send + Sync + 'static {
    fn lookup<'a>(&'a self, subject: &'a SubjectKey) -> DirectoryFuture<'a>;
}

impl<S> PrincipalDirectory for Vink<S>
where
    S: PrincipalStore + 'static,
{
    fn lookup<'a>(&'a self, subject: &'a SubjectKey) -> DirectoryFuture<'a> {
        Box::pin(async move {
            self.principal_for_subject(subject)
                .await
                .map_err(|_| DirectoryUnavailable)
        })
    }
}

/// The resolution for an authenticated subject, given the directory's answer. Total and pure:
/// an unreadable directory is `Unavailable`, never `Unbound` (which would invite provisioning a
/// second principal) and never `Principal`.
pub fn bind(
    subject: SubjectKey,
    lookup: Result<Option<BoundPrincipal>, DirectoryUnavailable>,
) -> Resolution {
    match lookup {
        Err(DirectoryUnavailable) => Resolution::Unavailable,
        Ok(None) => Resolution::Unbound(subject),
        Ok(Some(BoundPrincipal {
            principal,
            standing: Standing::Active,
        })) => Resolution::Principal(principal),
        Ok(Some(BoundPrincipal {
            principal,
            standing: Standing::Suspended,
        })) => Resolution::Suspended(principal),
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

/// Consults the Identity process for a presented session credential, then the App's own
/// directory for the subject's principal and standing.
#[derive(Clone)]
pub struct IdentityService {
    client: IdentityClient,
    directory: Arc<dyn PrincipalDirectory>,
}

impl std::fmt::Debug for IdentityService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityService")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl IdentityService {
    pub fn new(client: IdentityClient, directory: Arc<dyn PrincipalDirectory>) -> Self {
        Self { client, directory }
    }
}

fn acceptable(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= client::MAX_CREDENTIAL_BYTES
        && token
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b';' && b != b',')
}

/// The session credential a client presents: `Authorization: Bearer <token>` (non-browser
/// clients), else exactly one `__Host-vink_session` cookie (the Web). Anything else, including two
/// session cookies, is no credential (anonymous), not an outage.
pub fn presented_credential(parts: &Parts) -> Option<&str> {
    if let Some(value) = parts.headers.get(header::AUTHORIZATION) {
        let token = value.to_str().ok()?.strip_prefix("Bearer ")?.trim();
        return acceptable(token).then_some(token);
    }
    let mut found = None;
    for header in parts.headers.get_all(header::COOKIE) {
        let Ok(text) = header.to_str() else { continue };
        for pair in text.split(';') {
            if let Some((name, value)) = pair.trim().split_once('=')
                && name == SESSION_COOKIE
            {
                if found.is_some() {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found.filter(|token| acceptable(token))
}

impl IdentityResolver for IdentityService {
    fn resolve<'a>(&'a self, parts: &'a Parts, _peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(async move {
            let Some(credential) = presented_credential(parts) else {
                return Resolution::Anonymous;
            };
            match self.client.verify(credential).await {
                Ok(Verification::Authenticated { authority, subject }) => {
                    let Some(subject) = SubjectKey::new(&authority, &subject) else {
                        tracing::warn!("identity answered a subject outside the contract");
                        return Resolution::Unavailable;
                    };
                    let lookup = self.directory.lookup(&subject).await;
                    if lookup.is_err() {
                        tracing::warn!("principal directory could not decide");
                    }
                    bind(subject, lookup)
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
pub fn from_config(
    config: &Config,
    directory: Arc<dyn PrincipalDirectory>,
) -> Box<dyn IdentityResolver> {
    match (config.profile, config.dev_principal, &config.identity) {
        (Profile::Local, Some(principal), _) if config.bind.ip().is_loopback() => {
            Box::new(LocalFixture { principal })
        }
        (_, _, Some(identity)) => Box::new(IdentityService::new(
            IdentityClient::new(identity.socket.clone(), identity.server_uid),
            directory,
        )),
        _ => Box::new(NoAuthority),
    }
}

/// The client used by the sign-in endpoints: only when [`IdentityService`] resolves requests,
/// so a session issued here is also the one that authorizes them.
pub fn sign_in_client(config: &Config) -> Option<IdentityClient> {
    match (config.profile, config.dev_principal, &config.identity) {
        (Profile::Local, Some(_), _) if config.bind.ip().is_loopback() => None,
        (_, _, Some(identity)) => Some(IdentityClient::new(
            identity.socket.clone(),
            identity.server_uid,
        )),
        _ => None,
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
    fn a_bearer_token_or_one_session_cookie_is_a_presented_credential() {
        let with = |headers: &[(&str, &str)]| {
            let mut builder = Request::builder();
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            builder.body(()).unwrap().into_parts().0
        };
        let auth = |value: &str| with(&[("authorization", value)]);
        assert_eq!(presented_credential(&auth("Bearer abc")), Some("abc"));
        assert_eq!(presented_credential(&auth("Bearer ")), None);
        assert_eq!(presented_credential(&auth("Basic dXNlcg==")), None);
        let long = format!("Bearer {}", "t".repeat(client::MAX_CREDENTIAL_BYTES + 1));
        assert_eq!(presented_credential(&auth(&long)), None);
        assert_eq!(presented_credential(&parts()), None);

        let cookie = |value: &str| with(&[("cookie", value)]);
        assert_eq!(
            presented_credential(&cookie("theme=dark; __Host-vink_session=vink_s1_abc")),
            Some("vink_s1_abc")
        );
        // Only the exact name: a look-alike or an unprefixed cookie is not the session.
        for value in [
            "vink_session=vink_s1_abc",
            "__host-vink_session=vink_s1_abc",
            "__Host-vink_session_x=vink_s1_abc",
            "__Host-vink_session=",
            // Two session cookies are ambiguous: none is chosen.
            "__Host-vink_session=a; __Host-vink_session=b",
        ] {
            assert_eq!(presented_credential(&cookie(value)), None, "{value}");
        }
        let split = with(&[
            ("cookie", "__Host-vink_session=a"),
            ("cookie", "__Host-vink_session=b"),
        ]);
        assert_eq!(presented_credential(&split), None);
        // An explicit Authorization header decides; a malformed one is no credential.
        let both = with(&[
            ("authorization", "Basic x"),
            ("cookie", "__Host-vink_session=vink_s1_abc"),
        ]);
        assert_eq!(presented_credential(&both), None);
    }

    /// Every directory answer × standing: only an active binding acts (exhaustive over the
    /// finite outcome space of [`bind`]).
    #[test]
    fn only_an_active_binding_resolves_to_a_principal() {
        let subject = SubjectKey::new("vink-identity", "s-1").unwrap();
        let id = PrincipalId::parse("22222222-2222-4222-8222-222222222222").unwrap();
        let cases = [
            (Err(DirectoryUnavailable), Resolution::Unavailable),
            (Ok(None), Resolution::Unbound(subject.clone())),
            (
                Ok(Some(BoundPrincipal {
                    principal: id,
                    standing: Standing::Active,
                })),
                Resolution::Principal(id),
            ),
            (
                Ok(Some(BoundPrincipal {
                    principal: id,
                    standing: Standing::Suspended,
                })),
                Resolution::Suspended(id),
            ),
        ];
        for (lookup, expected) in cases {
            let resolution = bind(subject.clone(), lookup);
            assert_eq!(resolution, expected);
            assert_eq!(
                resolution.principal().is_some(),
                matches!(expected, Resolution::Principal(_))
            );
        }
    }
}
