//! The authentication authority behind the contract.
//!
//! The application layer of Identity: the transport maps a request to one use case here, the use
//! case checks that the configuration serves its method and orchestrates the store, and the
//! store decides inside one transaction. Callers never receive the store.
//!
//! Configurations in a built binary:
//! - [`Authority::not_configured`] (default): every verification answers `authority_not_configured`.
//! - [`Authority::new_email_code`] (`VINK_IDENTITY_AUTHORITY=email-code`, D-05/S-03 spike):
//!   native accounts proven by a one-time code sent to the address, with opaque server-side sessions.
//! - [`Authority::new_password`] (`VINK_IDENTITY_AUTHORITY=password`, ACC-03, O-12):
//!   native accounts with username and Argon2id password credentials, without mail dependency.
//! - [`Authority::new_native`] with both methods, which also serves registration and enrollment.
//!
//! Production authentication stays NOT_IMPLEMENTED until a real transport, edge rate limits
//! and an independent security review exist.
//!
//! The positive fixture below compiles only into this crate's unit tests; it proves response
//! shapes and is never a fallback.

use uuid::Uuid;

use crate::accounts::{AccountStore, StoreError};
use crate::contract::{Availability, Credential, SessionFacts, Subject, VerifyCredentialResponse};
use crate::email_code::{AUTHORITY, ChallengeHandle, Code, EmailAddress};
use crate::outcome::{
    ChallengeOutcome, DeletionReceipt, DeletionRejection, EmailChangeOutcome, EmailChangeStart,
    EnrollmentOutcome, EnrollmentStart, NewSession, PasswordRegistrationError,
    PasswordSignInRejection, SessionEmail, SignInOutcome,
};
use crate::password::{Password, Username};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthorityError {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MethodCapabilities {
    pub email_code: bool,
    pub password: bool,
}

/// A group of operations that one configuration either serves or answers `not_configured`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    /// Session operations of any native configuration: verify, revoke, verified address,
    /// deletion receipts.
    Sessions,
    /// Email-code sign-in, email change and the code proof for deletion.
    EmailCode,
    /// Username/password sign-in and the password proof for deletion.
    Password,
    /// Registration and enrollment of a username/password: needs both native methods.
    Enrollment,
}

/// The configured authority. Each use case checks its method before it reaches the store, so
/// a caller cannot run an operation the configuration does not serve, and the storage behind the
/// authority is not part of its interface.
#[derive(Debug)]
pub struct Authority {
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    NotConfigured,
    Native {
        store: AccountStore,
        methods: MethodCapabilities,
    },
    #[cfg(test)]
    Fixture(fixture::Fixture),
}

impl Authority {
    /// Every verification answers `authority_not_configured`.
    pub fn not_configured() -> Self {
        Self {
            kind: Kind::NotConfigured,
        }
    }

    pub fn new_email_code(store: AccountStore) -> Self {
        Self::new_native(
            store,
            MethodCapabilities {
                email_code: true,
                password: false,
            },
        )
    }

    pub fn new_password(store: AccountStore) -> Self {
        Self::new_native(
            store,
            MethodCapabilities {
                email_code: false,
                password: true,
            },
        )
    }

    pub fn new_native(store: AccountStore, methods: MethodCapabilities) -> Self {
        Self {
            kind: Kind::Native { store, methods },
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(fixture: fixture::Fixture) -> Self {
        Self {
            kind: Kind::Fixture(fixture),
        }
    }

    /// What capabilities and readiness report for `method`. The test fixture reports the session
    /// operations as available although it serves only `verify`.
    pub(crate) fn availability(&self, method: Method) -> Availability {
        let available = match &self.kind {
            Kind::NotConfigured => false,
            Kind::Native { methods, .. } => serves(*methods, method),
            #[cfg(test)]
            Kind::Fixture(_) => method == Method::Sessions,
        };
        if available {
            Availability::Available
        } else {
            Availability::NotConfigured
        }
    }

    /// `Err(NotConfigured)` unless `method` can run now. A transport calls this before it
    /// validates a request, so an unconfigured operation answers the same for every body.
    pub(crate) fn require(&self, method: Method) -> Result<(), AuthorityError> {
        self.native(method).map(|_| ())
    }

    fn native(&self, method: Method) -> Result<&AccountStore, AuthorityError> {
        match &self.kind {
            Kind::Native { store, methods } if serves(*methods, method) => Ok(store),
            _ => Err(AuthorityError::NotConfigured),
        }
    }

    pub(crate) async fn verify(
        &self,
        credential: &Credential,
    ) -> Result<VerifyCredentialResponse, AuthorityError> {
        match &self.kind {
            Kind::NotConfigured => {
                let _ = credential;
                Err(AuthorityError::NotConfigured)
            }
            Kind::Native { store, .. } => Ok(match store.verify(credential.expose()).await? {
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
            Kind::Fixture(fixture) => fixture.verify(credential),
        }
    }

    // --- Sessions ---

    /// Logout. `true` only when an active session was revoked by this call.
    pub(crate) async fn revoke(&self, credential: &str) -> Result<bool, AuthorityError> {
        Ok(self.native(Method::Sessions)?.revoke(credential).await?)
    }

    pub(crate) async fn verified_email(
        &self,
        credential: &str,
    ) -> Result<Option<SessionEmail>, AuthorityError> {
        Ok(self
            .native(Method::Sessions)?
            .verified_email(credential)
            .await?)
    }

    // --- Email code ---

    pub(crate) async fn start_sign_in(
        &self,
        address: &EmailAddress,
    ) -> Result<ChallengeOutcome, AuthorityError> {
        Ok(self
            .native(Method::EmailCode)?
            .start_sign_in(address)
            .await?)
    }

    pub(crate) async fn complete_sign_in(
        &self,
        handle: &ChallengeHandle,
        code: &Code,
    ) -> Result<SignInOutcome, AuthorityError> {
        Ok(self
            .native(Method::EmailCode)?
            .complete_sign_in(handle, code)
            .await?)
    }

    pub(crate) async fn start_email_change(
        &self,
        credential: &str,
        address: &EmailAddress,
    ) -> Result<EmailChangeStart, AuthorityError> {
        Ok(self
            .native(Method::EmailCode)?
            .start_email_change(credential, address)
            .await?)
    }

    pub(crate) async fn complete_email_change(
        &self,
        credential: &str,
        handle: &ChallengeHandle,
        code: &Code,
    ) -> Result<EmailChangeOutcome, AuthorityError> {
        Ok(self
            .native(Method::EmailCode)?
            .complete_email_change(credential, handle, code)
            .await?)
    }

    // --- Password ---

    /// Signs in with a username/password. A username outside the policy is checked against the
    /// dummy hash first, so its answer takes as long as a wrong password for a real account.
    pub(crate) async fn sign_in_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Result<NewSession, PasswordSignInRejection>, AuthorityError> {
        let store = self.native(Method::Password)?;
        let Ok(username) = Username::parse(username) else {
            crate::password::verify_sign_in(None, password)
                .await
                .map_err(|_| AuthorityError::Unavailable)?;
            return Ok(Err(PasswordSignInRejection::InvalidCredentials));
        };
        Ok(store.sign_in_password(&username, password).await?)
    }

    // --- Registration and enrollment (both native methods) ---

    pub(crate) async fn start_registration(
        &self,
        address: &EmailAddress,
    ) -> Result<ChallengeOutcome, AuthorityError> {
        Ok(self
            .native(Method::Enrollment)?
            .start_registration(address)
            .await?)
    }

    pub(crate) async fn register_password(
        &self,
        address: &EmailAddress,
        handle: &ChallengeHandle,
        code: &Code,
        username: &Username,
        password: &Password,
    ) -> Result<Result<NewSession, PasswordRegistrationError>, AuthorityError> {
        Ok(self
            .native(Method::Enrollment)?
            .register_password(address, handle, code, username, password)
            .await?)
    }

    pub(crate) async fn start_password_enrollment(
        &self,
        credential: &str,
    ) -> Result<EnrollmentStart, AuthorityError> {
        Ok(self
            .native(Method::Enrollment)?
            .start_password_enrollment(credential)
            .await?)
    }

    pub(crate) async fn password_enrollment_status(
        &self,
        credential: &str,
    ) -> Result<Result<bool, crate::contract::CredentialRejection>, AuthorityError> {
        Ok(self
            .native(Method::Enrollment)?
            .password_enrollment_status(credential)
            .await?)
    }

    /// Hashes the password before the store opens its transaction, so no row lock waits on
    /// Argon2, then attaches the credential to the session's own account.
    pub(crate) async fn enroll_password(
        &self,
        credential: &str,
        handle: &ChallengeHandle,
        code: &Code,
        username: &Username,
        password: Password,
    ) -> Result<EnrollmentOutcome, AuthorityError> {
        let store = self.native(Method::Enrollment)?;
        let hash = crate::password::hash_credential(password)
            .await
            .map_err(|_| AuthorityError::Unavailable)?;
        Ok(store
            .enroll_password(credential, handle, code, username, &hash)
            .await?)
    }

    // --- Account deletion ---

    pub(crate) async fn start_deletion(
        &self,
        address: &EmailAddress,
        expected: Option<Uuid>,
    ) -> Result<ChallengeOutcome, AuthorityError> {
        Ok(self
            .native(Method::EmailCode)?
            .start_deletion(address, expected)
            .await?)
    }

    pub(crate) async fn confirm_deletion_code(
        &self,
        handle: &ChallengeHandle,
        code: &Code,
        expected: Option<Uuid>,
        operation_id: Uuid,
    ) -> Result<Result<DeletionReceipt, DeletionRejection>, AuthorityError> {
        Ok(self
            .native(Method::EmailCode)?
            .complete_deletion_code(handle, code, expected, operation_id)
            .await?)
    }

    pub(crate) async fn confirm_deletion_password(
        &self,
        username: &Username,
        password: &str,
        expected: Option<Uuid>,
        operation_id: Uuid,
    ) -> Result<Result<DeletionReceipt, DeletionRejection>, AuthorityError> {
        Ok(self
            .native(Method::Password)?
            .complete_deletion_password(username, password, expected, operation_id)
            .await?)
    }

    pub(crate) async fn deletion(
        &self,
        operation: Uuid,
    ) -> Result<Option<DeletionReceipt>, AuthorityError> {
        Ok(self.native(Method::Sessions)?.deletion(operation).await?)
    }

    pub(crate) async fn pending_deletions(
        &self,
        after: Option<Uuid>,
    ) -> Result<Vec<DeletionReceipt>, AuthorityError> {
        Ok(self
            .native(Method::Sessions)?
            .pending_deletions(after)
            .await?)
    }

    pub(crate) async fn finish_deletion(&self, operation: Uuid) -> Result<(), AuthorityError> {
        Ok(self
            .native(Method::Sessions)?
            .finish_deletion(operation)
            .await?)
    }
}

fn serves(methods: MethodCapabilities, method: Method) -> bool {
    match method {
        Method::Sessions => true,
        Method::EmailCode => methods.email_code,
        Method::Password => methods.password,
        Method::Enrollment => methods.email_code && methods.password,
    }
}

/// The subject for an account of this authority.
pub(crate) fn subject(account: Uuid) -> Subject {
    Subject {
        authority: AUTHORITY.into(),
        id: account.to_string(),
    }
}

/// Labeled test authority (`authority: "test-fixture"`). Not compiled into binaries.
#[cfg(test)]
pub(crate) mod fixture {
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
