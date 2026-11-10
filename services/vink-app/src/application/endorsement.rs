//! Literary endorsement use cases (LVE, #73): "Đánh giá cao" on one published revision of a
//! work, "Hữu ích" on a root comment or a flat reply.
//!
//! Each write authenticates the actor, validates raw input into an [`EndorsementCommand`]
//! (command key, full target with its anchor, expected accepted order, Select/Remove), then hands
//! it to ONE [`EndorsementStore`] transaction. The store loads the facts, the actor's retained
//! record and the key's receipt under locks, calls [`decide_endorsement`] and commits before
//! answering; this module only maps the outcome it reports. Nothing here restates an
//! eligibility, ordering or replay rule.
//!
//! Protocol (CHOICE-03 shape, selected for this slice on top of the core's CHOICE-03 B): a
//! desired-state `PUT` per target carrying an actor-scoped command key and the accepted order the
//! client last saw. Method, deduplication and ordering stay separate axes: the method names the
//! desired selection; a retry with the same key and payload is a historical replay (the receipt),
//! never re-applied; ordering is the per-(actor, target) accepted order, which every accepted
//! command advances, same-state ones included. A replay reports the committed command AND the
//! current state; the current state is the one to show.
//!
//! Reads are split (CHOICE-13 A): public summaries need no principal and never carry viewer
//! state; the actor's own state needs one. Capabilities in an own state are advisory: they come
//! from the same pure decision on a read snapshot and authorize nothing at commit.

use uuid::Uuid;

use super::Vink;
use super::ports::{EndorsementStore, EndorsementWriteRecord, StoreError};
use crate::domain::endorsement::{
    AcceptedOrder, EndorsementCommand, EndorsementCommandKey, EndorsementCount, EndorsementFacts,
    EndorsementIntent, EndorsementOutcome, EndorsementRefusal, EndorsementState, EndorsementTarget,
    EndorsementTargetFacts, StoredEndorsement, decide_endorsement, public_count,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

/// Most comment IDs one batch read accepts (before deduplication). A default discussion page
/// (20 roots + 3 previews each) fits; a larger page splits into several requests.
pub const MAX_BATCH_COMMENTS: usize = 100;

/// A public aggregate: `None` when the target is not visible, never shown as zero.
pub type Summary = Option<EndorsementCount>;

/// Untrusted request to select or remove the actor's endorsement of one revision of a work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetWorkEndorsement {
    pub work_id: WorkId,
    /// The revision the page rendered: part of the target (CHOICE-02 B).
    pub revision: String,
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
}

/// Untrusted request to select or remove the actor's endorsement of a root comment or reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCommentEndorsement {
    pub work_id: WorkId,
    pub comment_id: CommentId,
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
}

/// A validated batch: at most [`MAX_BATCH_COMMENTS`] IDs, deduplicated, in first-seen order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentBatch(Vec<CommentId>);

impl CommentBatch {
    /// Parses `comment_ids`: comma-separated comment IDs, no empty element. Absent or empty is
    /// an empty batch.
    pub fn parse(raw: Option<&str>) -> Result<Self, EndorsementInput> {
        let raw = raw.unwrap_or_default();
        if raw.is_empty() {
            return Ok(Self::default());
        }
        // Bounded before any per-item work: 100 IDs in the longest form `CommentId::parse`
        // accepts (`urn:uuid:…`, 45 bytes) and their commas.
        if raw.len() > MAX_BATCH_COMMENTS * 46 {
            return Err(EndorsementInput::TooManyComments);
        }
        let parts: Vec<&str> = raw.split(',').collect();
        if parts.len() > MAX_BATCH_COMMENTS {
            return Err(EndorsementInput::TooManyComments);
        }
        let mut ids: Vec<CommentId> = Vec::with_capacity(parts.len());
        for part in parts {
            let id = CommentId::parse(part).ok_or(EndorsementInput::CommentId)?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        Ok(Self(ids))
    }

    pub fn ids(&self) -> &[CommentId] {
        &self.0
    }
}

/// What the actor could do with "select" now, from the pure decision on a read snapshot.
/// Advisory only: the write transaction decides again on the facts it locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Allowed,
    /// The actor wrote the comment or owns the work (CHOICE-08 A).
    OwnContent,
    /// The actor has no current endorsement capability.
    NotPermitted,
    /// The target is not visible to the actor.
    Unavailable,
}

/// The actor's own endorsement of a target as they may see it. Private to that actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnEndorsement {
    /// `None` exactly when the target is not visible: a hidden target exposes no stored state.
    /// Otherwise the retained record, or [`EndorsementState::INITIAL`] without one.
    pub state: Option<EndorsementState>,
    pub capability: Capability,
}

impl OwnEndorsement {
    pub const UNAVAILABLE: Self = Self {
        state: None,
        capability: Capability::Unavailable,
    };
}

/// What a committed request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementResult {
    /// A new command was accepted and advanced the order (same-state commands included).
    Accepted(EndorsementState),
    /// The same key and payload had already committed: `committed` is that historical result,
    /// `observed` the current state. Nothing was written; show `observed`.
    Replayed {
        committed: EndorsementState,
        observed: EndorsementState,
    },
}

/// The durable result of an endorsement request, reported only after the store committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementOutcomeView {
    pub target: EndorsementTarget,
    pub result: EndorsementResult,
    /// The target's public aggregate read in the same transaction, after the write.
    pub summary: Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentSummary {
    pub comment_id: CommentId,
    pub summary: Summary,
}

/// Public summaries of a public work's current revision and of requested comments, one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementSummaries {
    pub work_id: WorkId,
    /// The current publication's revision: the work target `work` counts.
    pub revision: RevisionId,
    pub work: EndorsementCount,
    /// One entry per distinct requested ID, in request order; missing, foreign and hidden
    /// comments are all `None`.
    pub comments: Vec<CommentSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnCommentEndorsement {
    pub comment_id: CommentId,
    pub own: OwnEndorsement,
}

/// The actor's own endorsements of a public work's current revision and of requested comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnEndorsements {
    pub work_id: WorkId,
    pub revision: RevisionId,
    pub work: OwnEndorsement,
    pub comments: Vec<OwnCommentEndorsement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EndorsementInput {
    #[error("expected_order must be >= 1")]
    ExpectedOrder,
    #[error("command_key must be a UUID")]
    CommandKey,
    #[error("revision_id must be the revision ID the page rendered")]
    Revision,
    #[error("comment_ids must be comma-separated comment IDs")]
    CommentId,
    #[error("comment_ids accepts at most {MAX_BATCH_COMMENTS} IDs")]
    TooManyComments,
}

impl EndorsementInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::ExpectedOrder => "expected_order",
            Self::CommandKey => "command_key",
            Self::Revision => "revision_id",
            Self::CommentId | Self::TooManyComments => "comment_ids",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EndorsementError {
    #[error("unauthenticated")]
    Unauthenticated,
    /// Missing, foreign or not visible; deliberately indistinguishable.
    #[error("not found")]
    NotFound,
    /// Authors and comment writers do not endorse their own content (CHOICE-08 A).
    #[error("own content cannot be endorsed")]
    SelfEndorsement,
    /// The actor has no current endorsement capability.
    #[error("endorsement not permitted")]
    NotPermitted,
    /// Selecting a revision that is no longer the public one; nothing was written.
    #[error("the publication changed; current revision is {published}")]
    RevisionChanged { published: RevisionId },
    /// The expected order is not the current one; nothing was written. `observed` is the
    /// actor's state to reconcile from; the old intent must not be resent with a fresh order.
    #[error("endorsement conflict")]
    Conflict { observed: EndorsementState },
    /// The command key was used for a different command by this actor.
    #[error("command key reused for a different command")]
    KeyReused,
    #[error("invalid input: {0}")]
    Invalid(#[from] EndorsementInput),
    /// Suspended by Vink at the write's commit (standing fence); nothing was written.
    #[error("principal suspended")]
    Suspended,
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for EndorsementError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            // The resolved principal has no Vink record: not an authenticated Vink actor.
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::PrincipalSuspended => Self::Suspended,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

fn expected(raw: i64) -> Result<AcceptedOrder, EndorsementInput> {
    AcceptedOrder::new(raw).ok_or(EndorsementInput::ExpectedOrder)
}

fn key(raw: &str) -> Result<EndorsementCommandKey, EndorsementInput> {
    Uuid::parse_str(raw)
        .map(EndorsementCommandKey::from_uuid)
        .map_err(|_| EndorsementInput::CommandKey)
}

const fn intent(endorsed: bool) -> EndorsementIntent {
    if endorsed {
        EndorsementIntent::Select
    } else {
        EndorsementIntent::Remove
    }
}

/// The actor's own view of `target` with the capability the pure decision gives a Select at
/// the current order. The probe never reaches storage.
fn own(
    actor: PrincipalId,
    can_endorse: bool,
    target: EndorsementTarget,
    facts: Option<EndorsementTargetFacts>,
    state: Option<EndorsementState>,
) -> OwnEndorsement {
    if !facts.is_some_and(|facts| facts.visible(target)) {
        return OwnEndorsement::UNAVAILABLE;
    }
    let current = state.unwrap_or(EndorsementState::INITIAL);
    let probe = EndorsementCommand {
        key: EndorsementCommandKey::from_uuid(Uuid::nil()),
        target,
        expected: current.order(),
        intent: EndorsementIntent::Select,
    };
    let decided = decide_endorsement(
        actor,
        probe,
        EndorsementFacts {
            can_endorse,
            target: facts,
            current: state.map(|state| StoredEndorsement::restore(actor, target, state)),
            receipt: None,
        },
    );
    let capability = match decided {
        EndorsementOutcome::Accepted(_) => Capability::Allowed,
        EndorsementOutcome::Refused(EndorsementRefusal::OwnContent) => Capability::OwnContent,
        EndorsementOutcome::Refused(EndorsementRefusal::AuthorityUnavailable) => {
            Capability::NotPermitted
        }
        _ => Capability::Unavailable,
    };
    OwnEndorsement {
        state: Some(current),
        capability,
    }
}

fn settle(
    command: EndorsementCommand,
    record: EndorsementWriteRecord,
) -> Result<EndorsementOutcomeView, EndorsementError> {
    let summary = public_count(command.target, record.facts.target, record.count);
    let result = match record.outcome {
        EndorsementOutcome::Accepted(write) => EndorsementResult::Accepted(write.next().state()),
        EndorsementOutcome::Replay {
            committed,
            observed,
        } => EndorsementResult::Replayed {
            committed,
            observed,
        },
        EndorsementOutcome::Conflict { observed } => {
            return Err(EndorsementError::Conflict { observed });
        }
        EndorsementOutcome::Refused(refusal) => {
            return Err(match refusal {
                // OPEN(#73 R4, product owner): a retried Select whose key already committed on a
                // target that is now hidden or an older revision lands here (the core does not
                // replay it), so a lost-ACK client hears 404/409 although its earlier command
                // committed. Kept per the PR #80 policy; alternatives and the discriminating test
                // are in docs/development/literary-endorsement-runtime.md (OPEN-R4).
                // R5: a revision ID that never belonged to the work also maps to
                // `revision_changed`; it names only the public current revision, so it is kept.
                EndorsementRefusal::TargetUnavailable => match (command, record.published) {
                    // Selecting the text of an earlier publication: the current publication is
                    // public, so naming it reveals nothing.
                    (
                        EndorsementCommand {
                            target: EndorsementTarget::Work { revision, .. },
                            intent: EndorsementIntent::Select,
                            ..
                        },
                        Some(published),
                    ) if published != revision => EndorsementError::RevisionChanged { published },
                    _ => EndorsementError::NotFound,
                },
                EndorsementRefusal::OwnContent => EndorsementError::SelfEndorsement,
                EndorsementRefusal::AuthorityUnavailable => EndorsementError::NotPermitted,
                EndorsementRefusal::KeyReused => EndorsementError::KeyReused,
                // The store loaded facts that do not bind the command, or a token is exhausted:
                // never reported as a success or as the client's fault.
                EndorsementRefusal::InvalidFacts | EndorsementRefusal::TokenExhausted => {
                    EndorsementError::Internal
                }
            });
        }
    };
    Ok(EndorsementOutcomeView {
        target: command.target,
        result,
        summary,
    })
}

impl<S> Vink<S>
where
    S: EndorsementStore,
{
    /// Selects or removes the actor's endorsement of one revision of a public work.
    pub async fn set_work_endorsement(
        &self,
        actor: Option<PrincipalId>,
        command: SetWorkEndorsement,
    ) -> Result<EndorsementOutcomeView, EndorsementError> {
        let actor = actor.ok_or(EndorsementError::Unauthenticated)?;
        let revision = RevisionId::parse(&command.revision).ok_or(EndorsementInput::Revision)?;
        let command = EndorsementCommand {
            key: key(&command.command_key)?,
            target: EndorsementTarget::Work {
                work: command.work_id,
                revision,
            },
            expected: expected(command.expected_order)?,
            intent: intent(command.endorsed),
        };
        let record = self.store.set_endorsement(actor, command).await?;
        settle(command, record)
    }

    /// Selects or removes the actor's endorsement of one comment (root or reply) of the work.
    pub async fn set_comment_endorsement(
        &self,
        actor: Option<PrincipalId>,
        command: SetCommentEndorsement,
    ) -> Result<EndorsementOutcomeView, EndorsementError> {
        let actor = actor.ok_or(EndorsementError::Unauthenticated)?;
        let command = EndorsementCommand {
            key: key(&command.command_key)?,
            target: EndorsementTarget::Comment {
                work: command.work_id,
                comment: command.comment_id,
            },
            expected: expected(command.expected_order)?,
            intent: intent(command.endorsed),
        };
        let record = self.store.set_endorsement(actor, command).await?;
        settle(command, record)
    }

    /// Public summaries; readable by anyone and independent of identity resolution. A work that
    /// is not public is `NotFound`; within a public work each unavailable comment stays `None`.
    pub async fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comment_ids: Option<&str>,
    ) -> Result<EndorsementSummaries, EndorsementError> {
        let batch = CommentBatch::parse(comment_ids)?;
        let facts = self.store.endorsement_summaries(work_id, &batch).await?;
        let Some(revision) = facts.published else {
            return Err(EndorsementError::NotFound);
        };
        let work_target = EndorsementTarget::Work {
            work: work_id,
            revision,
        };
        let work = public_count(work_target, facts.work, facts.work_count)
            .ok_or(EndorsementError::NotFound)?;
        let comments = batch
            .ids()
            .iter()
            .map(|&comment_id| {
                let target = EndorsementTarget::Comment {
                    work: work_id,
                    comment: comment_id,
                };
                let found = facts.comments.iter().find(|c| c.comment_id == comment_id);
                CommentSummary {
                    comment_id,
                    summary: found.and_then(|c| public_count(target, Some(c.facts), c.count)),
                }
            })
            .collect();
        Ok(EndorsementSummaries {
            work_id,
            revision,
            work,
            comments,
        })
    }

    /// The actor's own endorsements; `401` for a guest rather than "not selected".
    pub async fn own_endorsements(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
        comment_ids: Option<&str>,
    ) -> Result<OwnEndorsements, EndorsementError> {
        let actor = actor.ok_or(EndorsementError::Unauthenticated)?;
        let batch = CommentBatch::parse(comment_ids)?;
        let facts = self.store.own_endorsements(actor, work_id, &batch).await?;
        let Some(revision) = facts.published else {
            return Err(EndorsementError::NotFound);
        };
        let work_target = EndorsementTarget::Work {
            work: work_id,
            revision,
        };
        let work = own(
            actor,
            facts.can_endorse,
            work_target,
            facts.work,
            facts.work_state,
        );
        if work.state.is_none() {
            return Err(EndorsementError::NotFound);
        }
        let comments = batch
            .ids()
            .iter()
            .map(|&comment_id| {
                let target = EndorsementTarget::Comment {
                    work: work_id,
                    comment: comment_id,
                };
                let found = facts.comments.iter().find(|c| c.comment_id == comment_id);
                OwnCommentEndorsement {
                    comment_id,
                    own: match found {
                        Some(c) => own(actor, facts.can_endorse, target, Some(c.facts), c.state),
                        None => OwnEndorsement::UNAVAILABLE,
                    },
                }
            })
            .collect();
        Ok(OwnEndorsements {
            work_id,
            revision,
            work,
            comments,
        })
    }
}
