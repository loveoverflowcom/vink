//! The authentication authority behind the contract.
//!
//! None is selected (D-05, S-03 are open), so a built binary has only [`Authority::NotConfigured`]
//! and every verification answers `authority_not_configured`. Production authentication is
//! NOT_IMPLEMENTED. The positive fixture below compiles only into this crate's unit tests; it
//! proves response shapes and is never a fallback.

use crate::contract::{Availability, Credential, VerifyCredentialResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityError {
    /// No authority exists to ask; a configuration state, not an outage.
    NotConfigured,
    /// The authority or its storage could not answer; nothing was decided.
    Unavailable,
}

#[derive(Debug)]
pub enum Authority {
    NotConfigured,
    #[cfg(test)]
    Fixture(fixture::Fixture),
}

impl Authority {
    pub fn availability(&self) -> Availability {
        match self {
            Authority::NotConfigured => Availability::NotConfigured,
            #[cfg(test)]
            Authority::Fixture(_) => Availability::Available,
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
            #[cfg(test)]
            Authority::Fixture(fixture) => fixture.verify(credential),
        }
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
                    })
                }
                Fixture::Answers => Ok(VerifyCredentialResponse::Rejected),
            }
        }
    }
}
