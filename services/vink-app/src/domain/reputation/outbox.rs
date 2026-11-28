//! What a source transaction owes the Điểm Vink outbox (#645; contract K3), and the activation
//! cutover. Only an accepted endorsement whose version changed is a community event; markers for
//! work, comment and principal facts are written by database triggers on the rows the source
//! transactions lock (`migrations/20261208000000_reputation_outbox.sql`).

use crate::domain::endorsement::{EndorsementDelta, EndorsementState, EndorsementTarget};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

/// The activation row: producers write only while active, stamped with the epoch they read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activation {
    active: bool,
    epoch: i64,
}

impl Activation {
    pub fn restore(active: bool, epoch: i64) -> Option<Self> {
        (epoch >= 0 && (!active || epoch >= 1)).then_some(Self { active, epoch })
    }

    pub const fn active(self) -> bool {
        self.active
    }

    pub const fn epoch(self) -> i64 {
        self.epoch
    }

    /// The epoch a producer stamps, if any.
    pub const fn producing(self) -> Option<i64> {
        if self.active { Some(self.epoch) } else { None }
    }

    /// Activation starts a new epoch; deactivation keeps the last one.
    pub fn change(self, active: bool) -> ActivationChange {
        match (self.active, active) {
            (false, true) => self
                .epoch
                .checked_add(1)
                .map_or(ActivationChange::Exhausted, |epoch| {
                    ActivationChange::Changed(Self { active, epoch })
                }),
            (true, false) => ActivationChange::Changed(Self {
                active,
                epoch: self.epoch,
            }),
            _ => ActivationChange::Unchanged(self),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationChange {
    Changed(Activation),
    Unchanged(Activation),
    Exhausted,
}

/// The context a community event carries, read under the endorsement transaction's own locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunityContext {
    /// A current public pointer exists, and the revision moderation hid (never an anchor).
    Work {
        work_public: bool,
        hidden_revision: Option<RevisionId>,
    },
    /// The work is public, and the comment is approved and present.
    Comment {
        work_public: bool,
        comment_public: bool,
    },
}

/// One accepted Thấm or Ưng whose selection changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunityEvent {
    Tham {
        actor: PrincipalId,
        recipient: PrincipalId,
        work: WorkId,
        revision: RevisionId,
        state: EndorsementState,
        work_public: bool,
        hidden_revision: Option<RevisionId>,
    },
    Ung {
        actor: PrincipalId,
        recipient: PrincipalId,
        work: WorkId,
        comment: CommentId,
        state: EndorsementState,
        work_public: bool,
        comment_public: bool,
    },
}

impl CommunityEvent {
    /// The event an accepted endorsement write owes, or `None` for a same-state intent (only the
    /// order advanced) or a target/context mismatch.
    pub fn of(
        actor: PrincipalId,
        recipient: PrincipalId,
        target: EndorsementTarget,
        delta: EndorsementDelta,
        state: EndorsementState,
        context: CommunityContext,
    ) -> Option<Self> {
        if delta == EndorsementDelta::None {
            return None;
        }
        match (target, context) {
            (
                EndorsementTarget::Work { work, revision },
                CommunityContext::Work {
                    work_public,
                    hidden_revision,
                },
            ) => Some(Self::Tham {
                actor,
                recipient,
                work,
                revision,
                state,
                work_public,
                hidden_revision,
            }),
            (
                EndorsementTarget::Comment { comment, work },
                CommunityContext::Comment {
                    work_public,
                    comment_public,
                },
            ) => Some(Self::Ung {
                actor,
                recipient,
                work,
                comment,
                state,
                work_public,
                comment_public,
            }),
            _ => None,
        }
    }

    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Tham { .. } => "tham",
            Self::Ung { .. } => "ung",
        }
    }
}
