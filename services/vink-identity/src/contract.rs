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
    },
    /// The authority checked the credential and does not accept it.
    Rejected,
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
                "the request did not complete within 2 seconds; nothing was decided"
            }
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
