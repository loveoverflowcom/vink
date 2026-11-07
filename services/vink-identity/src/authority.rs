//! The authentication authority behind the contract.
//!
//! Two exist in a built binary:
//! - [`Authority::NotConfigured`] (default): every verification answers `authority_not_configured`.
//! - [`Authority::EmailCode`] (`VINK_IDENTITY_AUTHORITY=email-code`, D-05/S-03 spike): native
//!   accounts proven by a one-time code sent to the address, with opaque server-side sessions.
//!   Its only mail transport is the local outbox, so **production authentication stays
//!   NOT_IMPLEMENTED** until a real transport, edge rate limits and a security review exist.
//!
//! The positive fixture below compiles only into this crate's unit tests; it proves response
//! shapes and is never a fallback.

use uuid::Uuid;

use crate::accounts::{AccountStore, StoreError};
use crate::contract::{Availability, Credential, SessionFacts, Subject, VerifyCredentialResponse};
use crate::email_code::AUTHORITY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityError {
    /// No authority exists to ask; a configuration state, not an outage.
    NotConfigured,
    /// The authority or its storage could not answer; nothing was decided.
    Unavailable,
}

impl From<StoreError> for AuthorityError {
    fn from(StoreError::Unavailable: StoreError) -> Self {
        AuthorityError::Unavailable
    }
}

#[derive(Debug)]
pub enum Authority {
    NotConfigured,
    EmailCode(AccountStore),
    #[cfg(test)]
    Fixture(fixture::Fixture),
}

impl Authority {
    pub fn availability(&self) -> Availability {
        match self {
            Authority::NotConfigured => Availability::NotConfigured,
            Authority::EmailCode(_) => Availability::Available,
            #[cfg(test)]
            Authority::Fixture(_) => Availability::Available,
        }
    }

    /// Availability of the email-code operations (sign-in, revoke, email change).
    pub fn email_code_availability(&self) -> Availability {
        match self {
            Authority::EmailCode(_) => Availability::Available,
            _ => Availability::NotConfigured,
        }
    }

    /// The account store, for the email-code operations.
    pub fn email_code(&self) -> Result<&AccountStore, AuthorityError> {
        match self {
            Authority::EmailCode(store) => Ok(store),
            _ => Err(AuthorityError::NotConfigured),
        }
    }

    pub async fn verify(
        &self,
        credential: &Credential,
    ) -> Result<VerifyCredentialResponse, AuthorityError> {
        match self {
            Authority::NotConfigured => {
                let _ = credential;
                Err(AuthorityError::NotConfigured)
            }
            Authority::EmailCode(store) => Ok(match store.verify(credential.expose()).await? {
                Ok(session) => VerifyCredentialResponse::Authenticated {
                    subject: subject(session.account_id),
                    authenticated_at: session.authenticated_at,
                    session: Some(SessionFacts {
                        id: session.session_id.to_string(),
                        expires_at: session.expires_at,
                    }),
                },
                Err(reason) => VerifyCredentialResponse::Rejected {
                    reason: Some(reason),
                },
            }),
            #[cfg(test)]
            Authority::Fixture(fixture) => fixture.verify(credential),
        }
    }
}

/// The subject for an account of this authority.
pub fn subject(account: Uuid) -> Subject {
    Subject {
        authority: AUTHORITY.into(),
        id: account.to_string(),
    }
}

/// Labeled test authority (`authority: "test-fixture"`). Not compiled into binaries.
#[cfg(test)]
pub mod fixture {
    use time::macros::datetime;

    use crate::contract::{Credential, Subject, VerifyCredentialResponse};

    use super::AuthorityError;

    pub const AUTHORITY: &str = "test-fixture";
    pub const ACCEPTED: &str = "fixture-credential-accepted";
    pub const SUBJECT: &str = "fixture-subject-0001";

    #[derive(Debug)]
    pub enum Fixture {
        /// Accepts exactly [`ACCEPTED`] as [`SUBJECT`]; rejects anything else.
        Answers,
        Unavailable,
    }

    impl Fixture {
        pub fn verify(
            &self,
            credential: &Credential,
        ) -> Result<VerifyCredentialResponse, AuthorityError> {
            match self {
                Fixture::Unavailable => Err(AuthorityError::Unavailable),
                Fixture::Answers if credential.expose() == ACCEPTED => {
                    Ok(VerifyCredentialResponse::Authenticated {
                        subject: Subject {
                            authority: AUTHORITY.into(),
                            id: SUBJECT.into(),
                        },
                        authenticated_at: datetime!(2026-11-02 00:00:00 UTC),
                        session: None,
                    })
                }
                Fixture::Answers => Ok(VerifyCredentialResponse::Rejected { reason: None }),
            }
        }
    }
}
