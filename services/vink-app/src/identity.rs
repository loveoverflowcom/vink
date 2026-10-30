//! Server-side principal resolution seam.
//!
//! The production auth authority is still open (D-05 / S-03), so the default resolver
//! authenticates nobody and every private route answers `unauthenticated`. Request bodies,
//! query strings and headers chosen by the browser are never evidence of identity here.
//!
//! [`LocalFixture`] is a development stand-in: it exists only when `VINK_PROFILE=local`,
//! the server binds to loopback and the peer address is loopback. It proves nothing about
//! ACC-01, ACC-02 or S-03.

use std::net::SocketAddr;

use axum::http::request::Parts;

use crate::config::{Config, Profile};
use crate::domain::ids::PrincipalId;

pub trait IdentityResolver: Send + Sync + 'static {
    /// Returns the authenticated Vink principal, if any. `peer` is the TCP peer when known.
    fn resolve(&self, parts: &Parts, peer: Option<SocketAddr>) -> Option<PrincipalId>;
}

/// No auth authority configured: private routes stay closed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAuthority;

impl IdentityResolver for NoAuthority {
    fn resolve(&self, _parts: &Parts, _peer: Option<SocketAddr>) -> Option<PrincipalId> {
        None
    }
}

/// Treats loopback requests as one configured development principal.
#[derive(Debug, Clone, Copy)]
pub struct LocalFixture {
    principal: PrincipalId,
}

impl IdentityResolver for LocalFixture {
    fn resolve(&self, _parts: &Parts, peer: Option<SocketAddr>) -> Option<PrincipalId> {
        peer.filter(|addr| addr.ip().is_loopback())
            .map(|_| self.principal)
    }
}

/// Chooses the resolver for a validated configuration.
pub fn from_config(config: &Config) -> Box<dyn IdentityResolver> {
    match (config.profile, config.dev_principal) {
        (Profile::Local, Some(principal)) if config.bind.ip().is_loopback() => {
            Box::new(LocalFixture { principal })
        }
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
            .body(())
            .unwrap()
            .into_parts()
            .0
    }

    #[test]
    fn default_authority_ignores_client_supplied_identity() {
        assert_eq!(
            NoAuthority.resolve(&parts(), Some(([127, 0, 0, 1], 9).into())),
            None
        );
    }

    #[test]
    fn fixture_requires_a_loopback_peer() {
        let id = PrincipalId::parse("22222222-2222-4222-8222-222222222222").unwrap();
        let fixture = LocalFixture { principal: id };
        assert_eq!(
            fixture.resolve(&parts(), Some(([127, 0, 0, 1], 9).into())),
            Some(id)
        );
        assert_eq!(
            fixture.resolve(&parts(), Some(([192, 168, 1, 5], 9).into())),
            None
        );
        assert_eq!(fixture.resolve(&parts(), None), None);
    }
}
