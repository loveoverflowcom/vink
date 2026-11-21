//! Minimal intake contract for the Account owner. This contract has no HTTP or Identity transport.
//! The caller supplies an already verified, stable binding and reuses the operation ID on retry.
use uuid::Uuid;

use super::ports::StoreError;
use crate::domain::ids::PrincipalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocialRetirement {
    pub(crate) principal: PrincipalId,
    pub(crate) operation_id: Uuid,
}

impl SocialRetirement {
    pub fn new(principal: PrincipalId, operation_id: Uuid) -> Option<Self> {
        (!operation_id.is_nil()).then_some(Self {
            principal,
            operation_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialRetirementProgress {
    Fenced,
    Complete,
}

#[derive(Debug, thiserror::Error)]
pub enum SocialRetirementError {
    #[error("principal is not bound to an account")]
    Unbound,
    #[error("retirement has not been fenced")]
    NotFenced,
    #[error("retirement operation conflicts with a recorded operation")]
    OperationConflict,
    #[error(transparent)]
    Store(#[from] StoreError),
}
