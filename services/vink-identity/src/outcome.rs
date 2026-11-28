//! Outcomes of the authority's use cases: the vocabulary shared by the store that decides them
//! inside a transaction ([`crate::accounts`]), the authority that gates and orchestrates them
//! ([`crate::authority`]) and the transport that maps them to wire answers ([`crate::http`]).
//!
//! Nothing here depends on SQL, Axum or a transport, so the HTTP adapter can map outcomes without
//! knowing how they are stored. Wire spellings stay in [`crate::contract`]; the rejection reasons
//! that are themselves part of the contract (`ChallengeRejection`, `CredentialRejection`) are
//! reused rather than copied.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::contract::ChallengeRejection;
use crate::email_code::{ChallengeHandle, SessionCredential};

#[derive(Debug)]
pub enum ChallengeOutcome {
    Issued {
        handle: ChallengeHandle,
        expires_at: OffsetDateTime,
    },
    RateLimited(Throttle),
}

/// Which ceiling refused a challenge. Logged by label; the contract answer is the same for all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Throttle {
    /// Codes per address, any purpose.
    Address,
    /// Verified email changes started per account.
    Account,
    /// Codes across all addresses: caps mail volume when requests rotate addresses and networks.
    Global,
}

impl Throttle {
    pub fn label(self) -> &'static str {
        match self {
            Throttle::Address => "address",
            Throttle::Account => "account",
            Throttle::Global => "global",
        }
    }
}

/// A session verified now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub authenticated_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct NewSession {
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub credential: SessionCredential,
    pub authenticated_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug)]
pub enum SignInOutcome {
    Authenticated(NewSession),
    Rejected(ChallengeRejection),
}

#[derive(Debug)]
pub enum EmailChangeStart {
    Issued(ChallengeOutcome),
    /// The address is already this account's own. An address of ANOTHER account is not reported:
    /// the answer is `Issued` and no mail is sent, so the operation cannot probe who is registered.
    AddressUnavailable,
    CredentialRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailChangeOutcome {
    Changed,
    Rejected(ChallengeRejection),
    CredentialRejected,
}

/// The verified address of the account behind an active session (`POST …/email/verified`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEmail {
    pub session_id: Uuid,
    pub address: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRegistrationError {
    InvalidCode,
    ChallengeClosed,
    UsernameTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordSignInRejection {
    InvalidCredentials,
    AccountDisabled,
    RateLimited,
}

/// Start of a credential enrollment (ACC-03): the proof is a code mailed to the account's own
/// verified address.
#[derive(Debug)]
pub enum EnrollmentStart {
    Issued(ChallengeOutcome),
    /// The account already has a username/password. Changing it is a separate operation.
    AlreadyEnrolled,
    /// The account has no verified address to prove control with.
    NoVerifiedAddress,
    AccountDisabled,
    CredentialRejected,
}

/// Why an enrollment did not attach the credential. Every variant means nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentRejection {
    /// Wrong code; the challenge still accepts another attempt.
    InvalidCode,
    /// Expired, used, out of attempts, not this account's, or its address is no longer the
    /// account's verified address: request a new code.
    ChallengeClosed,
    /// The username belongs to an account. The code is NOT spent: pick another username.
    UsernameTaken,
    AlreadyEnrolled,
    AccountDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentOutcome {
    /// The credential now belongs to the presenting session's own account.
    Enrolled,
    Rejected(EnrollmentRejection),
    CredentialRejected,
}

/// Replayable receipt of an accepted account deletion. The stable operation ID links the Identity
/// commit with the App's later commit; the wire form is `contract::AccountDeletion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeletionReceipt {
    pub account_id: Uuid,
    pub operation_id: Uuid,
    pub accepted_at: OffsetDateTime,
}

/// Why a deletion proof did not retire the account. Nothing was retired in any case; a wrong code
/// is still recorded against its challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionRejection {
    /// The operation ID is the nil UUID.
    MalformedOperation,
    /// Wrong code; the challenge still accepts another attempt.
    InvalidCode,
    /// Expired, used, out of attempts, not the expected account's, or the operation ID was
    /// already accepted.
    ChallengeClosed,
    /// The password (or the expected account) did not verify.
    InvalidCredentials,
    RateLimited,
}
