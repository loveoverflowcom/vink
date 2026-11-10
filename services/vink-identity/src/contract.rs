//! Canonical wire types of the private Identity contract v1, owned by this service. The text and
//! fixtures live in `contracts/rpc/v1/`; consumers keep their own representation and never import
//! this crate. Messages are fixed strings: nothing a caller sends is echoed back or logged.

use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const CONTRACT: &str = "vink.identity.v1";
/// Largest request body accepted, in bytes.
pub const MAX_BODY_BYTES: usize = 8 * 1024;
/// Longest credential accepted, in bytes.
pub const MAX_CREDENTIAL_BYTES: usize = 4096;

/// Body of `POST /identity/v1/verify-credential`. Unknown fields, such as a caller-chosen
/// `subject`, are rejected rather than ignored.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyCredentialRequest {
    pub credential: Credential,
}

/// An opaque credential an end user presented to the caller (for example a session token issued
/// by the future authority). Never logged, echoed or printed by `Debug`.
#[derive(Deserialize)]
#[serde(try_from = "String")]
pub struct Credential(String);

impl TryFrom<String> for Credential {
    type Error = &'static str;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if raw.is_empty() {
            Err("credential is empty")
        } else if raw.len() > MAX_CREDENTIAL_BYTES {
            Err("credential is too long")
        } else {
            Ok(Self(raw))
        }
    }
}

impl Credential {
    /// The raw value, for the authority adapter only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential(<redacted>)")
    }
}

/// A decision the authority made about one credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum VerifyCredentialResponse {
    /// An authentication fact: who the authority says presented the credential, and when it
    /// checked. It is not a session and grants no Vink permission; mapping a subject to a Vink
    /// principal and authorizing it belong to the App.
    Authenticated {
        subject: Subject,
        #[serde(with = "time::serde::rfc3339")]
        authenticated_at: OffsetDateTime,
        /// Added within v1 by the email-code authority: the session the credential names.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<SessionFacts>,
    },
    /// The authority checked the credential and does not accept it.
    Rejected {
        /// Added within v1; informational. Every reason means "no authenticated subject".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<CredentialRejection>,
    },
}

/// Facts about a session; the credential itself is never repeated after issuance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFacts {
    pub id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRejection {
    /// Not a credential this authority issued.
    Unknown,
    Expired,
    /// Logged out, or closed by a verified email change.
    Revoked,
    AccountDisabled,
}

/// Stable identifier of a person within one authority. Never an email or pen name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subject {
    pub authority: String,
    pub id: String,
}

/// Body of `GET /identity/v1/capabilities`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub contract: String,
    pub authority: Availability,
    pub operations: Operations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operations {
    pub verify_credential: Availability,
    /// Added within v1 (email-code authority).
    pub sign_in: Availability,
    pub revoke_session: Availability,
    pub email_change: Availability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    NotConfigured,
}

/// Every error the contract returns. Callers branch on `code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    CallerUnauthenticated,
    MalformedRequest,
    PayloadTooLarge,
    UnsupportedVersion,
    NotFound,
    MethodNotAllowed,
    AuthorityNotConfigured,
    DependencyUnavailable,
    DeadlineExceeded,
    /// Added within v1 for the email-code operations.
    InvalidEmail,
    RateLimited,
    CredentialRejected,
    AddressUnavailable,
}

impl ContractError {
    pub fn code(self) -> &'static str {
        match self {
            Self::CallerUnauthenticated => "caller_unauthenticated",
            Self::MalformedRequest => "malformed_request",
            Self::PayloadTooLarge => "payload_too_large",
            Self::UnsupportedVersion => "unsupported_version",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::AuthorityNotConfigured => "authority_not_configured",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::InvalidEmail => "invalid_email",
            Self::RateLimited => "rate_limited",
            Self::CredentialRejected => "credential_rejected",
            Self::AddressUnavailable => "address_unavailable",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::CallerUnauthenticated => "the connecting process is not an allowed caller",
            Self::MalformedRequest => "the request body does not match the contract",
            Self::PayloadTooLarge => "the request body exceeds 8192 bytes",
            Self::UnsupportedVersion => "this contract version is not served",
            Self::NotFound => "no such operation",
            Self::MethodNotAllowed => "method not allowed for this operation",
            Self::AuthorityNotConfigured => {
                "no authentication authority is configured; nothing was verified"
            }
            Self::DependencyUnavailable => {
                "the authority or its storage is unavailable; nothing was decided"
            }
            Self::DeadlineExceeded => {
                "the request did not complete within its deadline; nothing was decided"
            }
            Self::InvalidEmail => "the address is not a deliverable mailbox address",
            Self::RateLimited => "too many codes were requested; try later",
            Self::CredentialRejected => "the session credential is not valid; nothing was changed",
            Self::AddressUnavailable => "the address cannot be used for this account",
        }
    }

    pub fn envelope(self) -> ErrorEnvelope {
        ErrorEnvelope {
            error: ErrorBody {
                code: self.code().into(),
                message: self.message().into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

// --- Email-code authority operations (added within v1; D-05/S-03 spike). ---

/// Body of `POST /identity/v1/sign-in/challenges`. Whether an account exists is not consulted
/// and never disclosed: every valid address gets the same answer and the same mail.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignInChallengeRequest {
    pub email: Bounded<320>,
}

/// `202` answer of both challenge operations. `challenge` is a client-held secret handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeIssued {
    pub challenge: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

/// Body of `POST /identity/v1/sign-in/complete`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignInCompleteRequest {
    pub challenge: Bounded<256>,
    pub code: Bounded<16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SignInResponse {
    /// Mailbox control proven: the subject and a new session. The `session.credential` appears
    /// here once and is never logged; the caller hands it to the client and keeps no copy.
    Authenticated {
        subject: Subject,
        #[serde(with = "time::serde::rfc3339")]
        authenticated_at: OffsetDateTime,
        session: IssuedSession,
    },
    Rejected {
        reason: ChallengeRejection,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuedSession {
    pub id: String,
    pub credential: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

impl std::fmt::Debug for IssuedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedSession")
            .field("id", &self.id)
            .field("credential", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeRejection {
    /// Wrong code; the challenge still accepts another attempt.
    InvalidCode,
    /// Expired, already used, out of attempts or not a challenge: request a new code.
    ChallengeClosed,
    /// The mailbox was proven but its account is disabled; no session was issued.
    AccountDisabled,
    /// Email change only: another account verified the address first.
    AddressUnavailable,
}

/// Body of `POST /identity/v1/sessions/revoke`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeSessionRequest {
    pub credential: Credential,
}

/// `revoked: false` when the credential was unknown, expired or already revoked: logout is
/// idempotent and the result is the same either way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeSessionResponse {
    pub revoked: bool,
}

/// Body of `POST /identity/v1/email-change/challenges`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailChangeChallengeRequest {
    pub credential: Credential,
    pub email: Bounded<320>,
}

/// Body of `POST /identity/v1/email-change/complete`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailChangeCompleteRequest {
    pub credential: Credential,
    pub challenge: Bounded<256>,
    pub code: Bounded<16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum EmailChangeResponse {
    /// The account's address changed; the subject did not. Other sessions were revoked.
    Changed,
    Rejected {
        reason: ChallengeRejection,
    },
}

/// A non-empty string of at most `N` bytes whose content is never printed (addresses, handles
/// and codes are personal data or secrets).
#[derive(Deserialize)]
#[serde(try_from = "String")]
pub struct Bounded<const N: usize>(String);

impl<const N: usize> TryFrom<String> for Bounded<N> {
    type Error = &'static str;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if raw.is_empty() || raw.len() > N {
            Err("value is empty or too long")
        } else {
            Ok(Self(raw))
        }
    }
}

impl<const N: usize> Bounded<N> {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl<const N: usize> fmt::Debug for Bounded<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}
