//! Owner-local planning use case (#150, CHOICE-02 B). No persistent adapter or route yet.
//! Storage integration must execute this plan against freshly locked facts and commit every
//! effect atomically; this call alone does not publish anything or dispatch evaluation.
use super::ports::BoundPrincipal;
use crate::domain::principal::Standing;
use crate::domain::publication::{AuthorCommand, AuthorTransition, Publication, PublicationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PublicationPlanError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("principal suspended")]
    Suspended,
    #[error(transparent)]
    Rejected(#[from] PublicationError),
}

pub fn plan_publication(
    actor: Option<BoundPrincipal>,
    current: &Publication,
    command: AuthorCommand,
) -> Result<AuthorTransition, PublicationPlanError> {
    let actor = actor.ok_or(PublicationPlanError::Unauthenticated)?;
    if actor.standing != Standing::Active {
        return Err(PublicationPlanError::Suspended);
    }
    Ok(current.decide_author(actor.principal, command)?)
}

#[cfg(test)]
mod tests;
