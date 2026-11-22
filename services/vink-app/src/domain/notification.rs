//! Notifications v1 producer and relay rules (#346; Epic #305 R3; contract
//! `contracts/http/v1/notifications.md` §1). Pure: the adapter loads facts under the locks of the
//! business transaction, asks these functions what the committed transition owes and writes the
//! returned notices in that same transaction; no clock, SQL, network or environment is read here.
//!
//! **What is owed (inventory §4.1).** Five runtime groups and one invalidation type:
//! - `comment.root.approved` / `comment.reply.approved`: a comment became public (approved and
//!   present) while its work was readable (COM-11 stamp at commit). Recipients are computed here
//!   (P-01B): the work owner for a root; the direct target's writer and the root's writer for a
//!   reply. The actor is excluded and principals are deduplicated.
//! - `work.moderation.decided`: a revision's decision concluded `approved` or `rejected` and the
//!   previous conclusion of that revision was different (P-11b). Pending, needs-admin, a retry, a
//!   reaffirmation or the same verdict again owe nothing. Only a rejection asks the author to act
//!   (P-11a).
//! - `comment.moderation.adverse`: moderation rejected or removed a comment; its writer is told,
//!   never the work owner (COM-08). A writer's own removal owes nothing to anyone.
//! - `work.endorsement.tham.changed`: an accepted Thấm on a work revision changed the selection
//!   (`Add`/`Remove`). A same-state command (`None`), a replay, a refusal and a comment target
//!   (Hữu ích, outside R3) owe nothing. `first_selection` comes from the retained row: from
//!   OFF/v1 every change advances the version by one, so the first selection is version 2.
//! - `notification.invalidation`: a public contribution or readable work stopped being so
//!   (rejection/removal/hold of an approved comment, a cleared public pointer, a standing change).
//!   It carries no recipients: the receiver reconciles by target.
//!
//! **Envelope.** Identifiers, versions, codes and booleans only: no body, email, display name,
//! moderator identity or reason. [`Envelope::canonical`] writes the contract's canonical form
//! (every object's keys in ASCII order, recipients in ascending lowercase UUID order, compact
//! UTF-8, no `payload_digest`); [`Envelope::digest`] is its SHA-256. The two contract examples are
//! checked byte for byte in the tests.
//!
//! **Relay.** Claims reuse the discussion outbox lifecycle ([`super::discussion::outbox`]): the
//! claim generation is the fence and the lease only a liveness timer. [`decide_relay_completion`]
//! maps what the receiver answered (or how the transport failed) to the fenced write: a result
//! from a worker that lost its claim records nothing.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};
use uuid::Uuid;

use super::discussion::lifecycle::{CommentState, Removal, Review};
use super::discussion::outbox::{
    ClaimGeneration, DeliveryFacts, OutboxPolicy, Ownership, StaleReason, check_ownership,
};
use super::endorsement::EndorsementDelta;
use super::ids::{CommentId, PrincipalId, RevisionId, WorkId};
use super::publication::{Moderation, NotificationStamp, PublicEpoch};

/// The only envelope schema this binary writes.
pub const SCHEMA_VERSION: i16 = 1;

/// Closed set of `event_type` codes (contract §1.1). `AdminNoticeReserved` is type-only (P-05):
/// no producer exists and [`EventType::produced`] is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    CommentRootApproved,
    CommentReplyApproved,
    WorkModerationDecided,
    CommentModerationAdverse,
    WorkThamChanged,
    /// Ưng on a comment (#365, VDR-NOTIFICATIONS@r2), owed to the comment's writer.
    CommentUngChanged,
    AdminNoticeReserved,
    Invalidation,
}

impl EventType {
    pub const ALL: [Self; 8] = [
        Self::CommentRootApproved,
        Self::CommentReplyApproved,
        Self::WorkModerationDecided,
        Self::CommentModerationAdverse,
        Self::WorkThamChanged,
        Self::CommentUngChanged,
        Self::AdminNoticeReserved,
        Self::Invalidation,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::CommentRootApproved => "comment.root.approved",
            Self::CommentReplyApproved => "comment.reply.approved",
            Self::WorkModerationDecided => "work.moderation.decided",
            Self::CommentModerationAdverse => "comment.moderation.adverse",
            Self::WorkThamChanged => "work.endorsement.tham.changed",
            Self::CommentUngChanged => "comment.endorsement.ung.changed",
            Self::AdminNoticeReserved => "admin.notice.reserved",
            Self::Invalidation => "notification.invalidation",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }

    /// Whether v1 has a producer for the type. The reserved admin type never has one.
    pub const fn produced(self) -> bool {
        !matches!(self, Self::AdminNoticeReserved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOwner {
    Discussion,
    PublicationModeration,
    Endorsement,
    Principal,
}

impl SourceOwner {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Discussion => "discussion",
            Self::PublicationModeration => "publication_moderation",
            Self::Endorsement => "endorsement",
            Self::Principal => "principal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticKind {
    Approved,
    Rejected,
    Removed,
    Add,
    Remove,
    Invalidate,
}

impl SemanticKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Removed => "removed",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Invalidate => "invalidate",
        }
    }
}

/// What a reply answered, as the source still knows it (§4.3 B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyTarget {
    /// A direct reply to the root (`replied_to_reply = false`).
    Root,
    /// A reply to another reply of the same root, whose row still exists.
    Reply { id: CommentId, author: PrincipalId },
    /// A reply to a reply whose row was purged (`reply_to_id` set NULL by the foreign key).
    Purged,
}

/// Placement of the moderated comment, loaded with its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementFacts {
    Root,
    Reply {
        root: CommentId,
        root_author: PrincipalId,
        target: ReplyTarget,
    },
}

/// The typed target of an envelope (contract §2.4; invalidation kinds from §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    WorkCommentThread {
        work: WorkId,
        comment: CommentId,
        moderation_version: i64,
    },
    WorkCommentReply {
        work: WorkId,
        root: CommentId,
        target: ReplyTarget,
        comment: CommentId,
        moderation_version: i64,
    },
    WorkEndorsement {
        work: WorkId,
        revision: RevisionId,
    },
    /// Ưng on a comment (#365): opens the Reader at that comment.
    WorkCommentEndorsement {
        work: WorkId,
        comment: CommentId,
    },
    WorkModerationDecision {
        work: WorkId,
        revision: RevisionId,
        decision_version: i64,
    },
    CommentModerationDecision {
        work: WorkId,
        comment: CommentId,
        moderation_version: i64,
    },
    InvalidWork {
        work: WorkId,
    },
    InvalidComment {
        work: WorkId,
        comment: CommentId,
    },
    InvalidPrincipal {
        principal: PrincipalId,
    },
}

impl Target {
    fn json(self) -> Value {
        let id = |raw: Uuid| Value::String(raw.to_string());
        match self {
            Self::WorkCommentThread {
                work,
                comment,
                moderation_version,
            } => json!({
                "kind": "work_comment_thread",
                "work_id": id(work.as_uuid()),
                "comment_id": id(comment.as_uuid()),
                "moderation_version": moderation_version,
            }),
            Self::WorkCommentReply {
                work,
                root,
                target,
                comment,
                moderation_version,
            } => {
                let (context, target_id) = match target {
                    ReplyTarget::Root => ("root", id(root.as_uuid())),
                    ReplyTarget::Reply { id: reply, .. } => ("reply", id(reply.as_uuid())),
                    ReplyTarget::Purged => ("reply", Value::Null),
                };
                json!({
                    "kind": "work_comment_reply",
                    "work_id": id(work.as_uuid()),
                    "root_id": id(root.as_uuid()),
                    "reply_context_kind": context,
                    "reply_target_id": target_id,
                    "comment_id": id(comment.as_uuid()),
                    "moderation_version": moderation_version,
                })
            }
            Self::WorkEndorsement { work, revision } => json!({
                "kind": "work_endorsement",
                "work_id": id(work.as_uuid()),
                "revision_id": id(revision.as_uuid()),
            }),
            Self::WorkCommentEndorsement { work, comment } => json!({
                "kind": "work_comment_endorsement",
                "work_id": id(work.as_uuid()),
                "comment_id": id(comment.as_uuid()),
            }),
            Self::WorkModerationDecision {
                work,
                revision,
                decision_version,
            } => json!({
                "kind": "work_moderation_decision",
                "work_id": id(work.as_uuid()),
                "revision_id": id(revision.as_uuid()),
                "decision_version": decision_version,
            }),
            Self::CommentModerationDecision {
                work,
                comment,
                moderation_version,
            } => json!({
                "kind": "comment_moderation_decision",
                "work_id": id(work.as_uuid()),
                "comment_id": id(comment.as_uuid()),
                "moderation_version": moderation_version,
            }),
            Self::InvalidWork { work } => json!({
                "kind": "work",
                "work_id": id(work.as_uuid()),
            }),
            Self::InvalidComment { work, comment } => json!({
                "kind": "comment",
                "work_id": id(work.as_uuid()),
                "comment_id": id(comment.as_uuid()),
            }),
            Self::InvalidPrincipal { principal } => json!({
                "kind": "principal",
                "principal_id": id(principal.as_uuid()),
            }),
        }
    }

    /// The scope that `source_order` is compared within, for the logical event identity.
    fn scope(self) -> String {
        match self {
            Self::WorkCommentThread { comment, .. }
            | Self::WorkCommentReply { comment, .. }
            | Self::CommentModerationDecision { comment, .. }
            | Self::WorkCommentEndorsement { comment, .. }
            | Self::InvalidComment { comment, .. } => format!("comment/{comment}"),
            Self::WorkEndorsement { work, revision }
            | Self::WorkModerationDecision { work, revision, .. } => {
                format!("revision/{work}/{revision}")
            }
            Self::InvalidWork { work } => format!("work/{work}"),
            Self::InvalidPrincipal { principal } => format!("principal/{principal}"),
        }
    }
}

/// Recipients after self-exclusion and deduplication by principal, in canonical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipients(Vec<PrincipalId>);

impl Recipients {
    /// No recipient: only an invalidation, which the receiver reconciles by target.
    pub const NONE: Self = Self(Vec::new());

    /// `candidates` minus `actor`, deduplicated, sorted by lowercase UUID text (which is the
    /// order of the UUIDs' bytes).
    pub fn of(
        candidates: impl IntoIterator<Item = PrincipalId>,
        actor: Option<PrincipalId>,
    ) -> Self {
        let mut ids: Vec<PrincipalId> = candidates
            .into_iter()
            .filter(|candidate| Some(*candidate) != actor)
            .collect();
        ids.sort_unstable_by_key(PrincipalId::as_uuid);
        ids.dedup();
        Self(ids)
    }

    pub fn as_slice(&self) -> &[PrincipalId] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What a committed source transition owes, before the adapter pins the event identity, the
/// activation epoch and the commit-time clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    event_type: EventType,
    source_owner: SourceOwner,
    source_order: i64,
    stamp: Option<NotificationStamp>,
    actor: Option<PrincipalId>,
    recipients: Recipients,
    target: Target,
    semantic: SemanticKind,
    first_selection: Option<bool>,
    action_required: bool,
}

impl Notice {
    pub const fn event_type(&self) -> EventType {
        self.event_type
    }

    pub const fn source_owner(&self) -> SourceOwner {
        self.source_owner
    }

    pub const fn source_order(&self) -> i64 {
        self.source_order
    }

    pub const fn stamp(&self) -> Option<NotificationStamp> {
        self.stamp
    }

    pub const fn actor(&self) -> Option<PrincipalId> {
        self.actor
    }

    pub fn recipients(&self) -> &[PrincipalId] {
        self.recipients.as_slice()
    }

    pub const fn target(&self) -> Target {
        self.target
    }

    pub const fn semantic(&self) -> SemanticKind {
        self.semantic
    }

    pub const fn first_selection(&self) -> Option<bool> {
        self.first_selection
    }

    pub const fn action_required(&self) -> bool {
        self.action_required
    }

    /// The logical event: one per type, scope, order (and Thấm/Ưng actor). A producer that ran
    /// twice for the same transition owes one obligation, never two.
    pub fn dedup_key(&self) -> String {
        let mut key = format!(
            "{}/{}/{}",
            self.event_type.code(),
            self.target.scope(),
            self.source_order
        );
        if let (EventType::WorkThamChanged | EventType::CommentUngChanged, Some(actor)) =
            (self.event_type, self.actor)
        {
            key.push_str(&format!("/{actor}"));
        }
        key
    }

    fn approval(
        event_type: EventType,
        source_order: i64,
        stamp: NotificationStamp,
        actor: PrincipalId,
        recipients: Recipients,
        target: Target,
    ) -> Option<Self> {
        (!recipients.is_empty()).then_some(Self {
            event_type,
            source_owner: SourceOwner::Discussion,
            source_order,
            stamp: Some(stamp),
            actor: Some(actor),
            recipients,
            target,
            semantic: SemanticKind::Approved,
            first_selection: None,
            action_required: false,
        })
    }

    fn invalidation(owner: SourceOwner, source_order: i64, target: Target) -> Self {
        Self {
            event_type: EventType::Invalidation,
            source_owner: owner,
            source_order,
            stamp: None,
            actor: None,
            recipients: Recipients::NONE,
            target,
            semantic: SemanticKind::Invalidate,
            first_selection: None,
            action_required: false,
        }
    }
}

/// A moderation transition of one comment, loaded under its row lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentModerationFacts {
    pub work: WorkId,
    pub work_owner: PrincipalId,
    pub comment: CommentId,
    pub writer: PrincipalId,
    pub placement: PlacementFacts,
    pub before: CommentState,
    pub after: CommentState,
    /// The work's stamp at commit. `None` when it was not loaded, which never notifies an
    /// approval (fail closed).
    pub stamp: Option<NotificationStamp>,
}

/// What a moderator's comment decision owes (approve, reject or remove).
pub fn comment_moderated(facts: CommentModerationFacts) -> Vec<Notice> {
    let CommentModerationFacts {
        work,
        work_owner,
        comment,
        writer,
        placement,
        before,
        after,
        stamp,
    } = facts;
    let mut owed = Vec::new();
    if before == after {
        return owed;
    }
    let version = after.version().get();
    if !before.is_public()
        && after.is_public()
        && let Some(stamp) = stamp.filter(|stamp| stamp.readable)
    {
        let notice = match placement {
            PlacementFacts::Root => Notice::approval(
                EventType::CommentRootApproved,
                version,
                stamp,
                writer,
                Recipients::of([work_owner], Some(writer)),
                Target::WorkCommentThread {
                    work,
                    comment,
                    moderation_version: version,
                },
            ),
            PlacementFacts::Reply {
                root,
                root_author,
                target,
            } => {
                let direct = match target {
                    ReplyTarget::Reply { author, .. } => Some(author),
                    ReplyTarget::Root | ReplyTarget::Purged => None,
                };
                Notice::approval(
                    EventType::CommentReplyApproved,
                    version,
                    stamp,
                    writer,
                    Recipients::of(direct.into_iter().chain([root_author]), Some(writer)),
                    Target::WorkCommentReply {
                        work,
                        root,
                        target,
                        comment,
                        moderation_version: version,
                    },
                )
            }
        };
        owed.extend(notice);
    }
    let adverse = if after.removal() == Some(Removal::ByModeration) && before.removal().is_none() {
        Some(SemanticKind::Removed)
    } else if after.removal().is_none()
        && after.review() == Review::Rejected
        && before.review() != Review::Rejected
    {
        Some(SemanticKind::Rejected)
    } else {
        None
    };
    if let Some(semantic) = adverse {
        owed.push(Notice {
            event_type: EventType::CommentModerationAdverse,
            source_owner: SourceOwner::Discussion,
            source_order: version,
            stamp: None,
            actor: None,
            recipients: Recipients::of([writer], None),
            target: Target::CommentModerationDecision {
                work,
                comment,
                moderation_version: version,
            },
            semantic,
            first_selection: None,
            action_required: false,
        });
    }
    owed.extend(comment_left_public(work, comment, before, after));
    owed
}

/// A public contribution that stopped being public (moderation reject/remove, the writer's own
/// removal, an operator post-review hold) owes an invalidation by target, never a new signal.
pub fn comment_left_public(
    work: WorkId,
    comment: CommentId,
    before: CommentState,
    after: CommentState,
) -> Option<Notice> {
    (before.is_public() && !after.is_public()).then(|| {
        Notice::invalidation(
            SourceOwner::Discussion,
            after.version().get(),
            Target::InvalidComment { work, comment },
        )
    })
}

/// A revision's earlier conclusion, as its decision history holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
    Approved,
    Rejected,
}

impl Conclusion {
    pub const fn of(moderation: Moderation) -> Option<Self> {
        match moderation {
            Moderation::Approved => Some(Self::Approved),
            Moderation::Rejected => Some(Self::Rejected),
            Moderation::Unsubmitted | Moderation::Pending | Moderation::NeedsAdmin => None,
        }
    }
}

/// A decision written for one revision, with the latest conclusion recorded for it before (at
/// a version not above this one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkDecisionFacts {
    pub work: WorkId,
    pub revision: RevisionId,
    pub owner: PrincipalId,
    pub decision_version: i64,
    pub moderation: Moderation,
    pub prior: Option<Conclusion>,
}

/// P-11: a new conclusion of a revision tells its author; the same verdict again does not.
pub fn work_decided(facts: WorkDecisionFacts) -> Option<Notice> {
    let conclusion = Conclusion::of(facts.moderation)?;
    if facts.prior == Some(conclusion) {
        return None;
    }
    let semantic = match conclusion {
        Conclusion::Approved => SemanticKind::Approved,
        Conclusion::Rejected => SemanticKind::Rejected,
    };
    Some(Notice {
        event_type: EventType::WorkModerationDecided,
        source_owner: SourceOwner::PublicationModeration,
        source_order: facts.decision_version,
        stamp: None,
        // MOD-12/P-12A: the decision's maker is never named, not even by identifier.
        actor: None,
        // N3: the owner hears about their own work; no self-exclusion here.
        recipients: Recipients::of([facts.owner], None),
        target: Target::WorkModerationDecision {
            work: facts.work,
            revision: facts.revision,
            decision_version: facts.decision_version,
        },
        semantic,
        first_selection: None,
        // P-11a: a rejection is the only conclusion that leaves the author something to do
        // (revise or appeal). An uncertain/needs-admin result never reaches the author.
        action_required: conclusion == Conclusion::Rejected,
    })
}

/// An accepted Thấm command on a work revision, after its write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThamFacts {
    pub work: WorkId,
    pub revision: RevisionId,
    pub owner: PrincipalId,
    pub actor: PrincipalId,
    pub delta: EndorsementDelta,
    /// The retained row's version after the write (even iff selected).
    pub version_after: i64,
    /// The accepted order after the write; advances on every accepted command of the actor.
    pub accepted_order: i64,
    pub stamp: NotificationStamp,
}

/// P-02B/P-03A: every real selection change is owed (so the receiver can reconcile membership);
/// only the first selection in the (actor, Thấm, work, revision) lifetime may signal.
pub fn tham_changed(facts: ThamFacts) -> Option<Notice> {
    let (semantic, first_selection) = match facts.delta {
        EndorsementDelta::None => return None,
        EndorsementDelta::Add => (SemanticKind::Add, Some(facts.version_after == 2)),
        EndorsementDelta::Remove => (SemanticKind::Remove, None),
    };
    let recipients = Recipients::of([facts.owner], Some(facts.actor));
    (!recipients.is_empty()).then_some(Notice {
        event_type: EventType::WorkThamChanged,
        source_owner: SourceOwner::Endorsement,
        source_order: facts.accepted_order,
        stamp: Some(facts.stamp),
        actor: Some(facts.actor),
        recipients,
        target: Target::WorkEndorsement {
            work: facts.work,
            revision: facts.revision,
        },
        semantic,
        first_selection,
        action_required: false,
    })
}

/// An accepted Ưng command on a comment, after its write (#365).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UngFacts {
    pub work: WorkId,
    pub comment: CommentId,
    /// The comment's writer, read from the locked comment row (never from the request).
    pub writer: PrincipalId,
    pub actor: PrincipalId,
    pub delta: EndorsementDelta,
    /// The retained row's version after the write (even iff selected).
    pub version_after: i64,
    /// The accepted order after the write; advances on every accepted command of the actor.
    pub accepted_order: i64,
    pub stamp: NotificationStamp,
}

/// P-02B/P-03A for Ưng (VDR-NOTIFICATIONS@r2): every real change is owed to the writer so the
/// receiver can reconcile membership; only the first selection in the (actor, Ưng, comment)
/// lifetime may signal. Ưng on one's own comment owes nothing (the domain refuses it anyway).
pub fn ung_changed(facts: UngFacts) -> Option<Notice> {
    let (semantic, first_selection) = match facts.delta {
        EndorsementDelta::None => return None,
        EndorsementDelta::Add => (SemanticKind::Add, Some(facts.version_after == 2)),
        EndorsementDelta::Remove => (SemanticKind::Remove, None),
    };
    let recipients = Recipients::of([facts.writer], Some(facts.actor));
    (!recipients.is_empty()).then_some(Notice {
        event_type: EventType::CommentUngChanged,
        source_owner: SourceOwner::Endorsement,
        source_order: facts.accepted_order,
        stamp: Some(facts.stamp),
        actor: Some(facts.actor),
        recipients,
        target: Target::WorkCommentEndorsement {
            work: facts.work,
            comment: facts.comment,
        },
        semantic,
        first_selection,
        action_required: false,
    })
}

/// The work's readable epoch ended (author made it private, moderation withdrew or removed it).
/// The epoch that ended orders it: it advances only when the work becomes readable again.
pub fn work_unreadable(work: WorkId, ended: PublicEpoch) -> Notice {
    Notice::invalidation(
        SourceOwner::PublicationModeration,
        ended.get(),
        Target::InvalidWork { work },
    )
}

/// A principal's standing changed (suspended or reinstated). `standing_version` advances by one
/// on every suspension and every lift.
pub fn principal_standing_changed(principal: PrincipalId, standing_version: i64) -> Notice {
    Notice::invalidation(
        SourceOwner::Principal,
        standing_version,
        Target::InvalidPrincipal { principal },
    )
}

/// The feature's activation as stored. Producers write events only while active, stamped with
/// the epoch read in their own transaction (no backfill: history before activation owes none).
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

    /// Activation starts a new epoch (a new cutover); deactivation keeps the last one.
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

/// The envelope a producer commits: a notice plus its event identity, activation epoch and the
/// transaction clock. Never rebuilt after commit: the relay sends the stored canonical text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    event_id: Uuid,
    activation_epoch: i64,
    occurred_at: String,
    notice: Notice,
}

impl Envelope {
    pub fn new(
        event_id: Uuid,
        activation_epoch: i64,
        occurred_at: OffsetDateTime,
        notice: Notice,
    ) -> Option<Self> {
        let occurred_at = occurred_at
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .ok()?;
        (activation_epoch >= 1).then_some(Self {
            event_id,
            activation_epoch,
            occurred_at,
            notice,
        })
    }

    pub const fn event_id(&self) -> Uuid {
        self.event_id
    }

    pub const fn notice(&self) -> &Notice {
        &self.notice
    }

    /// The envelope as a JSON value, without `payload_digest`.
    pub fn json(&self) -> Value {
        let notice = &self.notice;
        let mut object = Map::new();
        object.insert("event_id".into(), self.event_id.to_string().into());
        object.insert("event_type".into(), notice.event_type.code().into());
        object.insert("schema_version".into(), SCHEMA_VERSION.into());
        object.insert("source_owner".into(), notice.source_owner.code().into());
        object.insert("activation_epoch".into(), self.activation_epoch.into());
        object.insert("source_order".into(), notice.source_order.into());
        if let Some(stamp) = notice.stamp {
            object.insert(
                "publication_stamp".into(),
                json!({"epoch": stamp.epoch.get(), "readable": stamp.readable}),
            );
        }
        object.insert("occurred_at".into(), self.occurred_at.clone().into());
        object.insert(
            "actor_id".into(),
            notice
                .actor
                .map_or(Value::Null, |actor| actor.to_string().into()),
        );
        object.insert(
            "recipients".into(),
            notice
                .recipients
                .as_slice()
                .iter()
                .map(|recipient| Value::String(recipient.to_string()))
                .collect(),
        );
        object.insert("target".into(), notice.target.json());
        object.insert("semantic_kind".into(), notice.semantic.code().into());
        if let Some(first) = notice.first_selection {
            object.insert("first_selection".into(), first.into());
        }
        object.insert("action_required".into(), notice.action_required.into());
        Value::Object(object)
    }

    /// The contract's canonical text (§1.1), the input of the digest.
    pub fn canonical(&self) -> String {
        canonical(&self.json())
    }

    pub fn digest(&self) -> String {
        sha256_hex(self.canonical().as_bytes())
    }
}

/// Canonical JSON of the contract: object keys in ASCII order at every level, arrays in their
/// given order, compact. Strings are escaped as `serde_json` does (no escaping of non-ASCII).
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest.as_slice() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// The wire form the relay sends: the stored canonical envelope plus its `payload_digest`.
/// `None` when the stored text is not a JSON object (a defect or a manual edit).
pub fn wire(canonical_envelope: &str, payload_digest: &str) -> Option<String> {
    let Ok(Value::Object(mut object)) = serde_json::from_str::<Value>(canonical_envelope) else {
        return None;
    };
    object.insert("payload_digest".into(), payload_digest.into());
    Some(canonical(&Value::Object(object)))
}

/// What the receiver recomputes before reading its receipt: the digest of a received wire
/// envelope without its `payload_digest`, and whether it matches the one sent.
pub fn verify_wire(wire: &str) -> Option<(Uuid, String, bool)> {
    let Ok(Value::Object(mut object)) = serde_json::from_str::<Value>(wire) else {
        return None;
    };
    let sent = match object.remove("payload_digest") {
        Some(Value::String(sent)) => sent,
        _ => return None,
    };
    let event_id = object
        .get("event_id")
        .and_then(Value::as_str)
        .and_then(|raw| Uuid::parse_str(raw).ok())?;
    let digest = sha256_hex(canonical(&Value::Object(object)).as_bytes());
    let matches = digest == sent;
    Some((event_id, digest, matches))
}

/// What the Notification Service answered for one event (contract §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestStatus {
    Applied,
    AlreadyApplied,
    PayloadMismatch,
    RejectedInvalid,
    TransientUnavailable,
}

impl IngestStatus {
    pub const ALL: [Self; 5] = [
        Self::Applied,
        Self::AlreadyApplied,
        Self::PayloadMismatch,
        Self::RejectedInvalid,
        Self::TransientUnavailable,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::AlreadyApplied => "already_applied",
            Self::PayloadMismatch => "payload_mismatch",
            Self::RejectedInvalid => "rejected_invalid",
            Self::TransientUnavailable => "transient_unavailable",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.code() == code)
    }
}

/// How an attempt failed before any answer for the event was known. Codes only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayFailure {
    /// The send did not finish within its bound (the receiver may still have applied it).
    Timeout,
    /// The transport refused or broke the call.
    Transport,
    /// The receiver answered without a result for this event.
    MissingResult,
    /// The stored envelope cannot be sent as is.
    InvalidStored,
    /// The relay task panicked or the database lost the attempt's outcome.
    Internal,
}

impl RelayFailure {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Transport => "transport",
            Self::MissingResult => "missing_result",
            Self::InvalidStored => "invalid_stored",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayResult {
    Ingested(IngestStatus),
    Failed(RelayFailure),
}

/// Why a notification delivery ended without an acknowledged effect. Stored codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayDeadReason {
    RetryExhausted,
    DeadlineExceeded,
    GenerationExhausted,
    /// Same event ID, different payload: never a valid replay; an operator reconciles.
    PayloadMismatch,
    /// The receiver refused the schema/digest/type; an upgrade or fix precedes a requeue.
    RejectedInvalid,
    InvalidStored,
}

impl RelayDeadReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RetryExhausted => "retry_exhausted",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::GenerationExhausted => "generation_exhausted",
            Self::PayloadMismatch => "payload_mismatch",
            Self::RejectedInvalid => "rejected_invalid",
            Self::InvalidStored => "invalid_stored",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayCompletion {
    /// The receiver committed its receipt and effect (now or earlier): the obligation is done.
    Acked {
        already_applied: bool,
    },
    Retry {
        available_at_micros: i64,
        error: &'static str,
    },
    DeadLetter(RelayDeadReason),
    /// Write nothing: another claim (or none) owns the delivery.
    Stale(StaleReason),
}

impl RelayCompletion {
    /// The stored state code this completion records, if any.
    pub const fn state(self) -> Option<&'static str> {
        match self {
            Self::Acked { .. } => Some("succeeded"),
            Self::Retry { .. } => Some("ready"),
            Self::DeadLetter(_) => Some("dead"),
            Self::Stale(_) => None,
        }
    }

    pub const fn outcome(self) -> Option<&'static str> {
        match self {
            Self::Acked {
                already_applied: false,
            } => Some("applied"),
            Self::Acked {
                already_applied: true,
            } => Some("already_applied"),
            Self::DeadLetter(reason) => Some(reason.code()),
            Self::Retry { .. } | Self::Stale(_) => None,
        }
    }
}

/// What the holder of `held` records for `result`, under the delivery's row lock. A result from
/// a worker that lost its claim (reclaimed, or the delivery already finished) records nothing,
/// so an old worker's late ACK can neither finish nor reschedule the new owner's attempt.
pub fn decide_relay_completion(
    facts: DeliveryFacts,
    held: ClaimGeneration,
    result: RelayResult,
    policy: OutboxPolicy,
    now_micros: i64,
    jitter_micros: i64,
) -> RelayCompletion {
    if let Ownership::Stale(reason) = check_ownership(facts, held) {
        return RelayCompletion::Stale(reason);
    }
    let error = match result {
        RelayResult::Ingested(IngestStatus::Applied) => {
            return RelayCompletion::Acked {
                already_applied: false,
            };
        }
        RelayResult::Ingested(IngestStatus::AlreadyApplied) => {
            return RelayCompletion::Acked {
                already_applied: true,
            };
        }
        RelayResult::Ingested(IngestStatus::PayloadMismatch) => {
            return RelayCompletion::DeadLetter(RelayDeadReason::PayloadMismatch);
        }
        RelayResult::Ingested(IngestStatus::RejectedInvalid) => {
            return RelayCompletion::DeadLetter(RelayDeadReason::RejectedInvalid);
        }
        RelayResult::Failed(RelayFailure::InvalidStored) => {
            return RelayCompletion::DeadLetter(RelayDeadReason::InvalidStored);
        }
        RelayResult::Ingested(IngestStatus::TransientUnavailable) => {
            IngestStatus::TransientUnavailable.code()
        }
        RelayResult::Failed(failure) => failure.code(),
    };
    if facts.attempts >= policy.max_attempts() {
        return RelayCompletion::DeadLetter(RelayDeadReason::RetryExhausted);
    }
    if policy.past_deadline(facts.deadline_from_micros, now_micros) {
        return RelayCompletion::DeadLetter(RelayDeadReason::DeadlineExceeded);
    }
    RelayCompletion::Retry {
        available_at_micros: now_micros
            .saturating_add(policy.backoff_micros(facts.attempts))
            .saturating_add(jitter_micros.clamp(0, policy.base_backoff_micros())),
        error,
    }
}

#[cfg(test)]
mod tests;
