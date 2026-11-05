//! LVE pure decision seam. No adapter calls it yet (epic #73, PR1).
//! See docs/architecture/literary-endorsement.md for policy and transaction preconditions.
//! Accepted order advances even for same-state intent; mutation version advances only on change.
//! A receipt returns historical commitment separately from the currently observed actor state.
//!
//! ```compile_fail
//! use vink_app::domain::endorsement::{AcceptedOrder, EndorsementVersion};
//! let mutation = EndorsementVersion::FIRST;
//! let expected: AcceptedOrder = mutation;
//! ```
//! ```compile_fail
//! use vink_app::domain::endorsement::EndorsementState;
//! let mut state = EndorsementState::INITIAL;
//! state.selected = true;
//! ```
//! Lawful twin: the same public setup compiles; token substitution and field mutation do not.
//! ```
//! use vink_app::domain::endorsement::{AcceptedOrder, EndorsementVersion, EndorsementState};
//! let mutation = EndorsementVersion::FIRST;
//! let expected = AcceptedOrder::FIRST;
//! let state = EndorsementState::INITIAL;
//! assert_eq!(mutation.get(), expected.get());
//! assert!(!state.selected());
//! ```

use uuid::Uuid;

use super::discussion::CommentPlacement;
use super::discussion::lifecycle::CommentState;
use super::ids::{CommentId, PrincipalId, RevisionId, ThreadId, WorkId};

#[cfg(kani)]
mod proofs;
#[cfg(test)]
mod replay;
#[cfg(test)]
mod tests;

/// Work endorsement is revision-specific. Comments are immutable in the current pilot;
/// root and flat reply use the same comment identity without propagating a vote to their parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndorsementTarget {
    Work { work: WorkId, revision: RevisionId },
    Comment { work: WorkId, comment: CommentId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementAnchor {
    Revision(RevisionId),
    OriginalComment(CommentId),
}

impl EndorsementTarget {
    pub const fn work(self) -> WorkId {
        match self {
            Self::Work { work, .. } | Self::Comment { work, .. } => work,
        }
    }

    pub const fn anchor(self) -> EndorsementAnchor {
        match self {
            Self::Work { revision, .. } => EndorsementAnchor::Revision(revision),
            Self::Comment { comment, .. } => EndorsementAnchor::OriginalComment(comment),
        }
    }
}

macro_rules! positive_token {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i64);
        impl $name {
            pub const FIRST: Self = Self(1);
            pub fn new(raw: i64) -> Option<Self> {
                (raw >= 1).then_some(Self(raw))
            }
            pub const fn get(self) -> i64 {
                self.0
            }
            fn next(self) -> Option<Self> {
                self.0.checked_add(1).map(Self)
            }
        }
    };
}
positive_token!(EndorsementVersion);
positive_token!(AcceptedOrder);

/// A caller-supplied replay identity; scoped to server actor + endorsement operation, across targets.
/// Generated outside the pure core. A new key is a new command, not a retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EndorsementCommandKey(Uuid);
impl EndorsementCommandKey {
    pub const fn from_uuid(raw: Uuid) -> Self {
        Self(raw)
    }
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementIntent {
    Select,
    Remove,
}
impl EndorsementIntent {
    pub const fn selected(self) -> bool {
        matches!(self, Self::Select)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementCommand {
    pub key: EndorsementCommandKey,
    pub target: EndorsementTarget,
    pub expected: AcceptedOrder,
    pub intent: EndorsementIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementState {
    selected: bool,
    version: EndorsementVersion,
    order: AcceptedOrder,
}
impl EndorsementState {
    /// Absent is OFF/v1/order1. The first accepted OFF still writes order2, preventing ABA.
    pub const INITIAL: Self = Self {
        selected: false,
        version: EndorsementVersion::FIRST,
        order: AcceptedOrder::FIRST,
    };
    /// Storage-only reconstruction, never a client DTO. Versions cannot exceed accepted order.
    pub fn restore(selected: bool, version: i64, order: i64) -> Option<Self> {
        let version = EndorsementVersion::new(version)?;
        let order = AcceptedOrder::new(order)?;
        (version.get() <= order.get()).then_some(Self {
            selected,
            version,
            order,
        })
    }
    pub const fn selected(self) -> bool {
        self.selected
    }
    pub const fn version(self) -> EndorsementVersion {
        self.version
    }
    pub const fn order(self) -> AcceptedOrder {
        self.order
    }
}

/// Retain even after removal; actor and full target/anchor are part of the durable key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredEndorsement {
    actor: PrincipalId,
    target: EndorsementTarget,
    state: EndorsementState,
}
impl StoredEndorsement {
    pub const fn restore(
        actor: PrincipalId,
        target: EndorsementTarget,
        state: EndorsementState,
    ) -> Self {
        Self {
            actor,
            target,
            state,
        }
    }
    pub const fn actor(self) -> PrincipalId {
        self.actor
    }
    pub const fn target(self) -> EndorsementTarget {
        self.target
    }
    pub const fn state(self) -> EndorsementState {
        self.state
    }
}

/// Exact command and committed state. No hash implementation or receipt table is assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementReceipt {
    actor: PrincipalId,
    command: EndorsementCommand,
    committed: EndorsementState,
}
impl EndorsementReceipt {
    pub fn restore(
        actor: PrincipalId,
        command: EndorsementCommand,
        committed: EndorsementState,
    ) -> Option<Self> {
        (command.expected.next() == Some(committed.order())
            && committed.selected() == command.intent.selected())
        .then_some(Self {
            actor,
            command,
            committed,
        })
    }
    pub const fn actor(self) -> PrincipalId {
        self.actor
    }
    pub const fn command(self) -> EndorsementCommand {
        self.command
    }
    pub const fn committed(self) -> EndorsementState {
        self.committed
    }
}

/// Facts from server-owned rows, not request DTOs. `comment` checks thread binding using the
/// existing placement type; the future adapter must resolve thread -> work under its locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementTargetFacts {
    work: WorkId,
    subject: TargetSubject,
    author: PrincipalId,
    public: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetSubject {
    Work(Option<RevisionId>),
    Comment(CommentPlacement),
}
impl EndorsementTargetFacts {
    pub const fn work(work: WorkId, owner: PrincipalId, publication: Option<RevisionId>) -> Self {
        Self {
            work,
            subject: TargetSubject::Work(publication),
            author: owner,
            public: publication.is_some(),
        }
    }
    pub fn comment(
        work: WorkId,
        thread: ThreadId,
        placement: CommentPlacement,
        writer: PrincipalId,
        state: CommentState,
        work_public: bool,
    ) -> Option<Self> {
        (placement.thread_id() == thread).then_some(Self {
            work,
            subject: TargetSubject::Comment(placement),
            author: writer,
            public: work_public && state.is_public(),
        })
    }
    fn matches_scope(self, target: EndorsementTarget) -> bool {
        self.work == target.work()
            && match (self.subject, target) {
                (TargetSubject::Work(_), EndorsementTarget::Work { .. }) => true,
                (TargetSubject::Comment(p), EndorsementTarget::Comment { comment, .. }) => {
                    p.id() == comment
                }
                _ => false,
            }
    }
    /// Private, old-revision, missing and hidden targets all have no public projection.
    pub fn visible(self, target: EndorsementTarget) -> bool {
        self.matches_scope(target)
            && self.public
            && match (self.subject, target) {
                (TargetSubject::Work(current), EndorsementTarget::Work { revision, .. }) => {
                    current == Some(revision)
                }
                (TargetSubject::Comment(_), EndorsementTarget::Comment { .. }) => true,
                _ => false,
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementFacts {
    /// Current server capability, rechecked at commit; no production auth authority exists yet.
    pub can_endorse: bool,
    pub target: Option<EndorsementTargetFacts>,
    pub current: Option<StoredEndorsement>,
    /// Lookup by (server actor, endorsement operation, command key), across all targets.
    pub receipt: Option<EndorsementReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementDelta {
    None,
    Add,
    Remove,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountError {
    Underflow,
    Overflow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementCount(i64);
impl EndorsementCount {
    pub const ZERO: Self = Self(0);
    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 0).then_some(Self(raw))
    }
    pub const fn get(self) -> i64 {
        self.0
    }
    /// Immutable checked arithmetic. Failure means the entire future transaction must abort.
    pub fn apply(self, delta: EndorsementDelta) -> Result<Self, CountError> {
        match delta {
            EndorsementDelta::None => Ok(self),
            EndorsementDelta::Add => self.0.checked_add(1).map(Self).ok_or(CountError::Overflow),
            EndorsementDelta::Remove if self.0 == 0 => Err(CountError::Underflow),
            EndorsementDelta::Remove => Ok(Self(self.0 - 1)),
        }
    }
}

pub fn public_count(
    target: EndorsementTarget,
    facts: Option<EndorsementTargetFacts>,
    count: EndorsementCount,
) -> Option<EndorsementCount> {
    facts.filter(|f| f.visible(target)).map(|_| count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementRefusal {
    AuthorityUnavailable,
    TargetUnavailable,
    OwnContent,
    /// Stored actor/target, thread, or receipt data is inconsistent. Fail closed; not a wire error.
    InvalidFacts,
    KeyReused,
    TokenExhausted,
}

/// All three writes (retained state, delta and receipt) must commit together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementWrite {
    next: StoredEndorsement,
    delta: EndorsementDelta,
    receipt: EndorsementReceipt,
}
impl EndorsementWrite {
    pub const fn next(self) -> StoredEndorsement {
        self.next
    }
    pub const fn delta(self) -> EndorsementDelta {
        self.delta
    }
    pub const fn receipt(self) -> EndorsementReceipt {
        self.receipt
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndorsementOutcome {
    /// New command accepted, including same-state intent (delta None). Always persist its fence.
    Accepted(EndorsementWrite),
    /// Historical commitment, NOT a new acceptance and NOT necessarily the current selection.
    Replay {
        committed: EndorsementState,
        observed: EndorsementState,
    },
    /// No write. Reconcile; never refresh expected order and resubmit this old command.
    Conflict {
        observed: EndorsementState,
    },
    Refused(EndorsementRefusal),
}

/// The sole production endorsement transition. Typed facts do not prove their database origin;
/// the application/transaction consumer remains a prerequisite, outside PR1.
pub fn decide_endorsement(
    actor: PrincipalId,
    command: EndorsementCommand,
    facts: EndorsementFacts,
) -> EndorsementOutcome {
    use EndorsementOutcome::{Accepted, Conflict, Refused, Replay};
    use EndorsementRefusal::*;
    if !facts.can_endorse {
        return Refused(AuthorityUnavailable);
    }
    if facts
        .current
        .is_some_and(|s| s.actor != actor || s.target != command.target)
        || facts
            .target
            .is_some_and(|t| !t.matches_scope(command.target))
    {
        return Refused(InvalidFacts);
    }
    let current = facts.current.map_or(EndorsementState::INITIAL, |s| s.state);
    // An authenticated actor may remove only their own retained state even after visibility loss.
    // That private response exposes no target author/content, public count or other actor.
    let own_removal = command.intent == EndorsementIntent::Remove && facts.current.is_some();
    if !own_removal && !facts.target.is_some_and(|t| t.visible(command.target)) {
        return Refused(TargetUnavailable);
    }
    if let Some(receipt) = facts.receipt {
        if receipt.actor != actor || receipt.command.key != command.key {
            return Refused(InvalidFacts);
        }
        if receipt.command != command {
            return Refused(KeyReused);
        }
        if receipt.committed.order > current.order
            || (receipt.committed.order == current.order && receipt.committed != current)
        {
            return Refused(InvalidFacts);
        }
        return Replay {
            committed: receipt.committed,
            observed: current,
        };
    }
    if command.intent == EndorsementIntent::Select
        && facts.target.is_some_and(|t| t.author == actor)
    {
        return Refused(OwnContent);
    }
    // Compare BEFORE equality. A different key is new intent, even when selected is unchanged.
    if command.expected != current.order {
        return Conflict { observed: current };
    }
    let Some(order) = current.order.next() else {
        return Refused(TokenExhausted);
    };
    let selected = command.intent.selected();
    let changed = selected != current.selected;
    let version = if changed {
        let Some(v) = current.version.next() else {
            return Refused(TokenExhausted);
        };
        v
    } else {
        current.version
    };
    let next = EndorsementState {
        selected,
        version,
        order,
    };
    let delta = if !changed {
        EndorsementDelta::None
    } else if selected {
        EndorsementDelta::Add
    } else {
        EndorsementDelta::Remove
    };
    Accepted(EndorsementWrite {
        next: StoredEndorsement::restore(actor, command.target, next),
        delta,
        receipt: EndorsementReceipt {
            actor,
            command,
            committed: next,
        },
    })
}
