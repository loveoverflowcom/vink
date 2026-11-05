//! Literary endorsement use cases (LVE, #73): "Đánh giá cao" on a public work, "Hữu ích" on a
//! root comment or a flat reply.
//!
//! Each write authenticates the actor, validates raw input into a pure request (desired state +
//! the version the client saw, plus the revision it read for a work), then hands that request to
//! ONE [`EndorsementStore`] transaction. The store loads the facts under locks, calls the pure
//! decision of `domain::endorsement` and commits before answering; this module only maps the
//! decision it reports. Nothing here restates an eligibility or ordering rule.
//!
//! Protocol (CHOICE-03, selected for this slice): a desired-state `PUT` bound to the actor's
//! accepted-intent version, with no command key or receipt. Method, deduplication and ordering
//! are separate axes: the method states the desired selection; a retry or duplicate is recognized
//! by its effect (`accepted: false`, the state already holds); ordering is the per-(actor, target)
//! version, which an accepted request advances even when the state already held. The version
//! orders only that actor's requests on that target; nothing orders targets or actors.
//!
//! Reads are split (CHOICE-13 A): public summaries need no principal and never carry viewer
//! state; the actor's own state needs one. A guest therefore gets counts and `401` for own state,
//! never a fabricated "not endorsed". Capabilities in an own state are advisory: they come from
//! the same pure decision on a read snapshot and authorize nothing at commit.

use super::Vink;
use super::ports::{CommentEndorsementWrite, EndorsementStore, StoreError, WorkEndorsementWrite};
use crate::domain::endorsement::comment::{
    CommentDecision, CommentFacts, CommentIntent, CommentRequest, comment_summary,
    decide_comment_endorsement, own_comment_view,
};
use crate::domain::endorsement::work::{
    WorkDecision, WorkFacts, WorkIntent, WorkRequest, decide_work_endorsement, own_work_view,
    work_summary,
};
use crate::domain::endorsement::{
    CommentAnchor, Endorsement, EndorsementVersion, OwnView, Summary, Transition,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

/// Most comment IDs one batch read accepts (before deduplication). A default discussion page
/// (20 roots + 3 previews each) fits; a larger page splits into several requests.
pub const MAX_BATCH_COMMENTS: usize = 100;

/// Untrusted request to set or clear the actor's endorsement of a work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetWorkEndorsement {
    pub work_id: WorkId,
    pub expected_version: i64,
    pub endorsed: bool,
    /// The revision the reader's page rendered; required to endorse, refused when clearing.
    pub observed_revision: Option<String>,
}

/// Untrusted request to set or clear the actor's endorsement of a root comment or a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetCommentEndorsement {
    pub work_id: WorkId,
    pub comment_id: CommentId,
    pub expected_version: i64,
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

/// What the actor could do with "endorse" now, from the pure decision on a read snapshot.
/// Advisory only: the write transaction decides again on the facts it locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Selecting (or reaffirming the selection) at the returned version would be eligible.
    Allowed,
    /// The actor wrote the comment or owns the work (CHOICE-08 A).
    OwnContent,
    /// The work's author closed comments: no new comment endorsement; clearing still works.
    CommentsClosed,
    /// The target is not available to the actor.
    Unavailable,
}

/// The actor's own endorsement as they may see it. Private to that actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnEndorsement {
    pub view: OwnView,
    /// Send back as `expected_version`. `None` exactly when the target is unavailable: a hidden
    /// target exposes no stored state.
    pub version: Option<EndorsementVersion>,
    /// Works only: the revision the selection is anchored to, while selected.
    pub revision: Option<RevisionId>,
    pub capability: Capability,
}

impl OwnEndorsement {
    pub const UNAVAILABLE: Self = Self {
        view: OwnView::Unavailable,
        version: None,
        revision: None,
        capability: Capability::Unavailable,
    };
}

/// The durable result of an endorsement request, reported only after the store committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementOutcome {
    pub work_id: WorkId,
    /// `None` for the work's own endorsement.
    pub comment_id: Option<CommentId>,
    /// True when this request was accepted and advanced the version (a change or a
    /// reaffirmation). False when the stored state already satisfied it at a version the client
    /// had not seen (a retry or a duplicate): nothing was written and nothing was fenced.
    pub accepted: bool,
    /// The actor's endorsement as committed (or as found, when not accepted).
    pub own: OwnEndorsement,
    /// The target's public aggregate read in the same transaction, after the write.
    pub summary: Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentSummary {
    pub comment_id: CommentId,
    pub summary: Summary,
}

/// Public summaries of a public work and of requested comments of its thread, one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementSummaries {
    pub work_id: WorkId,
    /// The current publication's revision, the one `work` counts.
    pub revision: RevisionId,
    pub work: Summary,
    /// One entry per distinct requested ID, in request order. Missing, foreign and hidden
    /// comments are all [`Summary::Unavailable`].
    pub comments: Vec<CommentSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnCommentEndorsement {
    pub comment_id: CommentId,
    pub own: OwnEndorsement,
}

/// The actor's own endorsements of a public work and of requested comments, one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnEndorsements {
    pub work_id: WorkId,
    pub work: OwnEndorsement,
    pub comments: Vec<OwnCommentEndorsement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EndorsementInput {
    #[error("expected_version must be >= 1")]
    ExpectedVersion,
    #[error("observed_revision_id must be the revision ID the page rendered when endorsing")]
    ObservedRevision,
    #[error("observed_revision_id must be null when clearing an endorsement")]
    RevisionWhenClearing,
    #[error("comment_ids must be comma-separated comment IDs")]
    CommentId,
    #[error("comment_ids accepts at most {MAX_BATCH_COMMENTS} IDs")]
    TooManyComments,
}

impl EndorsementInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::ExpectedVersion => "expected_version",
            Self::ObservedRevision | Self::RevisionWhenClearing => "observed_revision_id",
            Self::CommentId | Self::TooManyComments => "comment_ids",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EndorsementError {
    #[error("unauthenticated")]
    Unauthenticated,
    /// Missing, foreign or not public; deliberately indistinguishable.
    #[error("not found")]
    NotFound,
    /// Authors and comment writers do not endorse their own content (CHOICE-08 A).
    #[error("own content cannot be endorsed")]
    SelfEndorsement,
    /// The reader endorsed a revision that is no longer the public one; nothing was written.
    #[error("the publication changed; current revision is {published}")]
    RevisionChanged { published: RevisionId },
    #[error("comments are closed for this work")]
    CommentsClosed,
    /// An accepted intent changed the selection since the client's version; nothing was
    /// written. `current` is the actor's stored state, to reconcile from.
    #[error("endorsement conflict")]
    Conflict { current: OwnEndorsement },
    #[error("invalid input: {0}")]
    Invalid(#[from] EndorsementInput),
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
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

fn expected(raw: i64) -> Result<EndorsementVersion, EndorsementInput> {
    EndorsementVersion::new(raw).ok_or(EndorsementInput::ExpectedVersion)
}

/// The actor's view of their work endorsement, with the capability the pure decision gives a
/// request to endorse the current text at the current version.
fn own_work(
    actor: PrincipalId,
    work: WorkId,
    facts: Option<WorkFacts>,
    current: Endorsement<RevisionId>,
) -> OwnEndorsement {
    let view = own_work_view(work, facts, current);
    let Some(published) = facts.and_then(|facts| facts.published) else {
        return OwnEndorsement::UNAVAILABLE;
    };
    if view == OwnView::Unavailable {
        return OwnEndorsement::UNAVAILABLE;
    }
    let probe = WorkRequest {
        expected: current.version(),
        intent: WorkIntent::Endorse {
            observed: published,
        },
    };
    let capability = match decide_work_endorsement(actor, work, probe, facts, current) {
        WorkDecision::Transition(transition) if transition.write().is_some() => Capability::Allowed,
        WorkDecision::OwnWork => Capability::OwnContent,
        _ => Capability::Unavailable,
    };
    OwnEndorsement {
        view,
        version: Some(current.version()),
        revision: current.selection(),
        capability,
    }
}

fn own_comment(
    actor: PrincipalId,
    work: WorkId,
    comment: CommentId,
    facts: Option<CommentFacts>,
    current: Endorsement<CommentAnchor>,
) -> OwnEndorsement {
    let view = own_comment_view(work, comment, facts, current);
    if view == OwnView::Unavailable {
        return OwnEndorsement::UNAVAILABLE;
    }
    let probe = CommentRequest {
        expected: current.version(),
        intent: CommentIntent::Endorse,
    };
    let capability = match decide_comment_endorsement(actor, work, comment, probe, facts, current) {
        CommentDecision::Transition(transition) if transition.write().is_some() => {
            Capability::Allowed
        }
        CommentDecision::OwnComment => Capability::OwnContent,
        CommentDecision::CommentsClosed => Capability::CommentsClosed,
        _ => Capability::Unavailable,
    };
    OwnEndorsement {
        view,
        version: Some(current.version()),
        revision: None,
        capability,
    }
}

/// `Changed`/`Reaffirmed` are accepted; `AlreadyCurrent` reports the state that already holds;
/// `Stale` is a conflict carrying the stored state.
fn settled<A: Copy>(
    transition: Transition<A>,
    view: impl Fn(Endorsement<A>) -> OwnEndorsement,
) -> Result<(bool, OwnEndorsement), EndorsementError> {
    match transition {
        Transition::Changed { next, .. } | Transition::Reaffirmed { next } => {
            Ok((true, view(next)))
        }
        Transition::AlreadyCurrent(current) => Ok((false, view(current))),
        Transition::Stale { current } => Err(EndorsementError::Conflict {
            current: view(current),
        }),
        // Unreachable in practice (i64 versions); never reported as a success.
        Transition::VersionExhausted => Err(EndorsementError::Internal),
    }
}

impl<S> Vink<S>
where
    S: EndorsementStore,
{
    /// Sets or clears the actor's endorsement of a public work. To endorse, the request names
    /// the revision the reader read; a publication that changed since is `RevisionChanged`.
    pub async fn set_work_endorsement(
        &self,
        actor: Option<PrincipalId>,
        command: SetWorkEndorsement,
    ) -> Result<EndorsementOutcome, EndorsementError> {
        let actor = actor.ok_or(EndorsementError::Unauthenticated)?;
        let expected = expected(command.expected_version)?;
        let intent = match (command.endorsed, command.observed_revision.as_deref()) {
            (true, Some(raw)) => WorkIntent::Endorse {
                observed: RevisionId::parse(raw).ok_or(EndorsementInput::ObservedRevision)?,
            },
            (true, None) => return Err(EndorsementInput::ObservedRevision.into()),
            (false, None) => WorkIntent::Clear,
            (false, Some(_)) => return Err(EndorsementInput::RevisionWhenClearing.into()),
        };
        let work_id = command.work_id;
        let WorkEndorsementWrite {
            facts,
            decision,
            count,
        } = self
            .store
            .set_work_endorsement(actor, work_id, WorkRequest { expected, intent })
            .await?;
        let transition = match decision {
            WorkDecision::Transition(transition) => transition,
            WorkDecision::TargetUnavailable => return Err(EndorsementError::NotFound),
            WorkDecision::OwnWork => return Err(EndorsementError::SelfEndorsement),
            WorkDecision::RevisionChanged { published } => {
                return Err(EndorsementError::RevisionChanged { published });
            }
        };
        let (accepted, own) =
            settled(transition, |stored| own_work(actor, work_id, facts, stored))?;
        Ok(EndorsementOutcome {
            work_id,
            comment_id: None,
            accepted,
            own,
            summary: work_summary(work_id, facts, count),
        })
    }

    /// Sets or clears the actor's endorsement of one comment (root or reply) of the work.
    pub async fn set_comment_endorsement(
        &self,
        actor: Option<PrincipalId>,
        command: SetCommentEndorsement,
    ) -> Result<EndorsementOutcome, EndorsementError> {
        let actor = actor.ok_or(EndorsementError::Unauthenticated)?;
        let request = CommentRequest {
            expected: expected(command.expected_version)?,
            intent: if command.endorsed {
                CommentIntent::Endorse
            } else {
                CommentIntent::Clear
            },
        };
        let (work_id, comment_id) = (command.work_id, command.comment_id);
        let CommentEndorsementWrite {
            facts,
            decision,
            count,
        } = self
            .store
            .set_comment_endorsement(actor, work_id, comment_id, request)
            .await?;
        let transition = match decision {
            CommentDecision::Transition(transition) => transition,
            CommentDecision::TargetUnavailable => return Err(EndorsementError::NotFound),
            CommentDecision::OwnComment => return Err(EndorsementError::SelfEndorsement),
            CommentDecision::CommentsClosed => return Err(EndorsementError::CommentsClosed),
        };
        let (accepted, own) = settled(transition, |stored| {
            own_comment(actor, work_id, comment_id, facts, stored)
        })?;
        Ok(EndorsementOutcome {
            work_id,
            comment_id: Some(comment_id),
            accepted,
            own,
            summary: comment_summary(work_id, comment_id, facts, count),
        })
    }

    /// Public summaries; readable by anyone and independent of identity resolution. A work that
    /// is not public is `NotFound`; within a public work each unavailable comment stays
    /// `Unavailable`, never zero.
    pub async fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comment_ids: Option<&str>,
    ) -> Result<EndorsementSummaries, EndorsementError> {
        let batch = CommentBatch::parse(comment_ids)?;
        let facts = self.store.endorsement_summaries(work_id, &batch).await?;
        let work = work_summary(work_id, facts.work, facts.work_count);
        let (Summary::Count(_), Some(revision)) = (work, facts.work.and_then(|w| w.published))
        else {
            return Err(EndorsementError::NotFound);
        };
        let comments = batch
            .ids()
            .iter()
            .map(|&comment_id| {
                let found = facts
                    .comments
                    .iter()
                    .find(|entry| entry.facts.comment == comment_id);
                CommentSummary {
                    comment_id,
                    summary: match found {
                        Some(entry) => {
                            comment_summary(work_id, comment_id, Some(entry.facts), entry.count)
                        }
                        None => Summary::Unavailable,
                    },
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

    /// The actor's own endorsements; `401` for a guest rather than "not endorsed".
    pub async fn own_endorsements(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
        comment_ids: Option<&str>,
    ) -> Result<OwnEndorsements, EndorsementError> {
        let actor = actor.ok_or(EndorsementError::Unauthenticated)?;
        let batch = CommentBatch::parse(comment_ids)?;
        let facts = self.store.own_endorsements(actor, work_id, &batch).await?;
        let work = own_work(actor, work_id, facts.work, facts.endorsement);
        if work.view == OwnView::Unavailable {
            return Err(EndorsementError::NotFound);
        }
        let comments = batch
            .ids()
            .iter()
            .map(|&comment_id| {
                let found = facts
                    .comments
                    .iter()
                    .find(|entry| entry.facts.comment == comment_id);
                OwnCommentEndorsement {
                    comment_id,
                    own: match found {
                        Some(entry) => own_comment(
                            actor,
                            work_id,
                            comment_id,
                            Some(entry.facts),
                            entry.endorsement,
                        ),
                        None => OwnEndorsement::UNAVAILABLE,
                    },
                }
            })
            .collect();
        Ok(OwnEndorsements {
            work_id,
            work,
            comments,
        })
    }
}
