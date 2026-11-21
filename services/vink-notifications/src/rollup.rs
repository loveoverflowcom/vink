//! Pure rules of the write-time rollup (N5, P-02/03/06/07/09/10/13). The store loads the facts
//! under the recipient's inbox lock, asks these functions what to do and writes the answer in the
//! same transaction as the receipt. No clock, SQL or I/O is read here.
//!
//! **Groups (P-07A).** `group_key = recipient + event_type + target_key`. At most one *open*
//! group per key: unread, not empty, not expired. A signal joins the open group or starts a new
//! one; a group never reopens once read, emptied or expired (no revival, P-03A/P-06A).
//!
//! **Versions (T-05 CHOICE-19 A).** Every change to an inbox takes the next value of that
//! inbox's sequence while holding its row lock, so sequence order is commit order per recipient.
//! `group_version` is the sequence of the group's last observable change; `signal_seq` the
//! sequence of its last new signal; `read_through` the highest sequence acknowledged as read. A
//! group is unread while `signal_seq > read_through`; it counts on the badge while its
//! `signal_seq` is also above the inbox's Seen watermark.
//!
//! **Thấm (T-04 CHOICE-18 A).** One mark per (recipient, actor, work, revision) keeps the latest
//! desired state with its source order (`accepted_order`). An older event never overrides a newer
//! one. Only an add the App flagged `first_selection` may signal, once (P-02B); every other add or
//! remove reconciles membership of the open group silently (P-03A). The App's flag, not the
//! absence of a receipt, is the evidence of a first selection (contract §1.2).

use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::envelope::{Event, EventType, ReplyContext, SemanticKind, Target};

/// P-06A: a group lives at most this long from its creation; updates never extend it.
pub const GROUP_MAX_AGE: Duration = Duration::days(180);
/// P-13B: an event that occurred longer ago than this may reconcile marks and membership but
/// never signals (it cannot create or revive a group). Equals the App's replay horizon.
pub const LATE_EVENT_CUTOFF: Duration = Duration::days(30);
/// How long a receipt (and a comment fence) is kept: past the App's replay horizon and the
/// late cutoff, so any event the App can still send finds its receipt.
pub const RECEIPT_HORIZON: Duration = Duration::days(45);
/// How long a Thấm mark is kept after its last change. It is defense in depth for P-02B
/// (ordering and replay); the lifetime guarantee is the App's `first_selection` fact.
pub const MARK_HORIZON: Duration = Duration::days(400);
/// How long an acknowledged command answer is replayable for the same `command_key`.
pub const COMMAND_HORIZON: Duration = Duration::days(7);
/// P-04B: the published bound on how old a badge count served by the Gateway may be.
pub const MAX_STALENESS_SECONDS: i64 = 60;
/// Preview size (N5).
pub const PREVIEW: usize = 3;

/// The rollup key of a grouped event, without the recipient (one per recipient applies).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupKey {
    pub event_type: EventType,
    pub target_key: String,
}

/// How a group counts its contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counting {
    /// Distinct actors with a valid contribution (root/reply approvals, Thấm).
    Actors,
    /// One item per decision (work decision, adverse comment decision).
    Items,
}

/// The inventory's `group_key` (§4.1) for a grouped event; `None` for invalidation.
pub fn group_key(event: &Event) -> Option<GroupKey> {
    let target_key = match event.target {
        Target::WorkCommentThread { work, .. } => format!("work/{work}"),
        Target::WorkCommentReply {
            root,
            context,
            comment,
            ..
        } => match context {
            ReplyContext::Root => format!("root/{root}"),
            ReplyContext::Reply(target) => format!("reply/{target}"),
            // §4.3 B: the anchor is gone, so the approved reply gets its own key.
            ReplyContext::Purged => format!("purged/{comment}"),
        },
        Target::WorkEndorsement { work, revision } => format!("revision/{work}/{revision}"),
        Target::WorkModerationDecision { work, revision, .. } => {
            format!("revision/{work}/{revision}/{}", event.semantic.code())
        }
        Target::CommentModerationDecision { comment, .. } => {
            format!("comment/{comment}/{}", event.semantic.code())
        }
        Target::InvalidWork { .. }
        | Target::InvalidComment { .. }
        | Target::InvalidPrincipal { .. } => {
            return None;
        }
    };
    event.event_type.grouped().then(|| GroupKey {
        event_type: event.event_type,
        target_key,
    })
}

pub const fn counting(event_type: EventType) -> Counting {
    match event_type {
        EventType::WorkModerationDecided | EventType::CommentModerationAdverse => Counting::Items,
        _ => Counting::Actors,
    }
}

/// The unit of membership an actor-counted event contributes: the approved comment, or the
/// revision for a Thấm (one contribution per actor).
pub fn contribution(event: &Event) -> Option<(Uuid, Uuid)> {
    let actor = event.actor?;
    let unit = match event.target {
        Target::WorkCommentThread { comment, .. } | Target::WorkCommentReply { comment, .. } => {
            comment
        }
        Target::WorkEndorsement { revision, .. } => revision,
        _ => return None,
    };
    Some((actor, unit))
}

/// The work an event concerns, for the Gateway's visibility checks.
pub const fn work_of(target: Target) -> Option<Uuid> {
    match target {
        Target::WorkCommentThread { work, .. }
        | Target::WorkCommentReply { work, .. }
        | Target::WorkEndorsement { work, .. }
        | Target::WorkModerationDecision { work, .. }
        | Target::CommentModerationDecision { work, .. }
        | Target::InvalidWork { work }
        | Target::InvalidComment { work, .. } => Some(work),
        Target::InvalidPrincipal { .. } => None,
    }
}

/// Whether the event is too old to signal (P-13B).
pub fn is_late(occurred_at: OffsetDateTime, now: OffsetDateTime) -> bool {
    occurred_at < now - LATE_EVENT_CUTOFF
}

/// A group's expiry: fixed at creation (P-06A).
pub fn expiry(created_at: OffsetDateTime) -> OffsetDateTime {
    created_at + GROUP_MAX_AGE
}

/// When a signal counts as having happened: its source commit, never later than now. A group
/// created by a replayed event is therefore aged from the original event (P-06A, P-13B), and a
/// source clock ahead of this one cannot push an expiry into the future.
pub fn signal_time(occurred_at: OffsetDateTime, now: OffsetDateTime) -> OffsetDateTime {
    occurred_at.min(now)
}

/// One membership row as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    pub actor: Uuid,
    pub contribution: Uuid,
    pub active: bool,
    /// Inbox sequence of the member's latest contribution (orders the preview).
    pub last_seq: i64,
}

/// Distinct actors with at least one active contribution, and the [`PREVIEW`] most recent of
/// them (by their latest active contribution; ties by actor ID, so the order is total). An actor
/// outside the preview moves in when one inside leaves (P-03A b).
pub fn preview(members: &[Member]) -> (i32, Vec<Uuid>) {
    let mut latest: Vec<(i64, Uuid)> = Vec::new();
    for member in members.iter().filter(|member| member.active) {
        match latest.iter_mut().find(|(_, actor)| *actor == member.actor) {
            Some(entry) => entry.0 = entry.0.max(member.last_seq),
            None => latest.push((member.last_seq, member.actor)),
        }
    }
    latest.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let count = i32::try_from(latest.len()).unwrap_or(i32::MAX);
    (
        count,
        latest
            .into_iter()
            .take(PREVIEW)
            .map(|(_, actor)| actor)
            .collect(),
    )
}

/// The stored Thấm mark of one (recipient, actor, work, revision).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub selected: bool,
    pub source_order: i64,
    /// A signal was already created for this actor and revision (P-02B).
    pub signaled: bool,
}

/// What a Thấm event does to its mark and to the open group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThamStep {
    /// Older than (or equal to) the mark: nothing changes.
    Stale,
    /// The first selection: a new signal joins or starts the open group.
    Signal,
    /// The actor's membership in the open group (if any) becomes `active`; no signal.
    Reconcile { active: bool },
}

/// P-02B/P-03A. `eligible` is false when the event is late or the work was not readable at the
/// source commit: such an add still updates the mark but never signals.
pub fn tham_step(
    mark: Option<Mark>,
    semantic: SemanticKind,
    source_order: i64,
    first_selection: bool,
    eligible: bool,
) -> ThamStep {
    if mark.is_some_and(|mark| source_order <= mark.source_order) {
        return ThamStep::Stale;
    }
    let signaled = mark.is_some_and(|mark| mark.signaled);
    match semantic {
        SemanticKind::Add if first_selection && eligible && !signaled => ThamStep::Signal,
        SemanticKind::Add => ThamStep::Reconcile { active: true },
        _ => ThamStep::Reconcile { active: false },
    }
}

/// The mark after a non-stale step.
pub fn next_mark(mark: Option<Mark>, step: ThamStep, source_order: i64, selected: bool) -> Mark {
    Mark {
        selected,
        source_order,
        signaled: mark.is_some_and(|mark| mark.signaled) || step == ThamStep::Signal,
    }
}

/// Why a Read command was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadRefusal {
    /// The client claims a version the group never had.
    FutureVersion,
}

/// P-10A: a Read of `observed` acknowledges every signal up to that version. A newer signal stays
/// unread; a newer silent change (preview, membership) does not keep the group unread.
/// Returns the new `read_through`, never lower than the current one (a late ACK is a no-op).
pub fn read_through(current: i64, group_version: i64, observed: i64) -> Result<i64, ReadRefusal> {
    if observed > group_version || observed < 1 {
        return Err(ReadRefusal::FutureVersion);
    }
    Ok(current.max(observed))
}

/// An inbox snapshot token: the inbox sequence at the time it was issued. Opaque to clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Snapshot(pub i64);

impl Snapshot {
    pub fn token(self) -> String {
        format!("s{}", self.0)
    }

    pub fn parse(token: &str) -> Option<Self> {
        let digits = token.strip_prefix('s')?;
        if digits.is_empty() || digits.len() > 19 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return None;
        }
        digits.parse::<i64>().ok().map(Self)
    }

    /// A token is acceptable only up to what the inbox has committed: a client cannot pick a
    /// future watermark (contract §2.2).
    pub fn within(self, last_seq: i64) -> bool {
        self.0 <= last_seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    fn member(n: u8, contribution: u8, active: bool, last_seq: i64) -> Member {
        Member {
            actor: actor(n),
            contribution: actor(contribution),
            active,
            last_seq,
        }
    }

    #[test]
    fn preview_counts_distinct_active_actors_and_keeps_the_three_latest() {
        let members = [
            member(1, 10, true, 1),
            member(2, 11, true, 2),
            member(3, 12, true, 3),
            member(4, 13, true, 4),
            // Actor 1 again, with a newer comment: counted once, ordered by its latest.
            member(1, 14, true, 5),
        ];
        assert_eq!(preview(&members), (4, vec![actor(1), actor(4), actor(3)]));
        // P-03A b: actor 4 (inside the preview) leaves; actor 2 moves in.
        let mut left = members;
        left[3].active = false;
        assert_eq!(preview(&left), (3, vec![actor(1), actor(3), actor(2)]));
        // An actor outside the preview leaving changes only the count.
        let mut outside = members;
        outside[1].active = false;
        assert_eq!(preview(&outside), (3, vec![actor(1), actor(4), actor(3)]));
        // Every contribution gone: the group is empty.
        let gone: Vec<Member> = members
            .iter()
            .map(|m| Member {
                active: false,
                ..*m
            })
            .collect();
        assert_eq!(preview(&gone), (0, vec![]));
    }

    #[test]
    fn only_a_first_eligible_selection_signals_and_only_once() {
        use SemanticKind::{Add, Remove};
        assert_eq!(tham_step(None, Add, 1, true, true), ThamStep::Signal);
        // Not readable at the source commit, or late: the mark moves, nothing signals.
        assert_eq!(
            tham_step(None, Add, 1, true, false),
            ThamStep::Reconcile { active: true }
        );
        let signaled = Mark {
            selected: true,
            source_order: 1,
            signaled: true,
        };
        assert_eq!(
            tham_step(Some(signaled), Remove, 2, false, true),
            ThamStep::Reconcile { active: false }
        );
        // Relike after unlike (P-02B): silent.
        let removed = Mark {
            selected: false,
            source_order: 2,
            signaled: true,
        };
        assert_eq!(
            tham_step(Some(removed), Add, 3, false, true),
            ThamStep::Reconcile { active: true }
        );
        // A replayed first add after a restore that kept the mark: never a second signal.
        assert_eq!(
            tham_step(
                Some(Mark {
                    source_order: 0,
                    ..removed
                }),
                Add,
                1,
                true,
                true
            ),
            ThamStep::Reconcile { active: true }
        );
    }

    #[test]
    fn an_older_thams_event_never_overrides_a_newer_one() {
        use SemanticKind::{Add, Remove};
        // Remove(2) arrived before the first Add(1): the add is stale and wakes nothing.
        let after_remove = next_mark(None, ThamStep::Reconcile { active: false }, 2, false);
        assert_eq!(
            after_remove,
            Mark {
                selected: false,
                source_order: 2,
                signaled: false
            }
        );
        assert_eq!(
            tham_step(Some(after_remove), Add, 1, true, true),
            ThamStep::Stale
        );
        assert_eq!(
            tham_step(Some(after_remove), Remove, 2, false, true),
            ThamStep::Stale
        );
        let signal = next_mark(None, ThamStep::Signal, 1, true);
        assert!(signal.signaled);
        assert!(
            next_mark(
                Some(signal),
                ThamStep::Reconcile { active: false },
                2,
                false
            )
            .signaled
        );
    }

    #[test]
    fn read_acknowledges_what_was_seen_and_never_goes_back() {
        // Tab A saw v1; a new signal made v2: reading v1 leaves v2 unread.
        assert_eq!(read_through(0, 2, 1), Ok(1));
        // Tab B already read v2; tab A's late ACK of v1 keeps it.
        assert_eq!(read_through(2, 2, 1), Ok(2));
        assert_eq!(read_through(0, 2, 3), Err(ReadRefusal::FutureVersion));
        assert_eq!(read_through(0, 2, 0), Err(ReadRefusal::FutureVersion));
    }

    #[test]
    fn snapshot_tokens_are_strict_and_bounded_by_the_committed_sequence() {
        assert_eq!(Snapshot::parse("s42"), Some(Snapshot(42)));
        assert_eq!(Snapshot(42).token(), "s42");
        assert_eq!(Snapshot::parse("s0"), Some(Snapshot(0)));
        for bad in [
            "42",
            "s",
            "s-1",
            "s01",
            "s 1",
            "s99999999999999999999",
            "S1",
        ] {
            assert_eq!(Snapshot::parse(bad), None, "{bad}");
        }
        assert!(Snapshot(5).within(5));
        assert!(!Snapshot(6).within(5));
    }

    #[test]
    fn age_and_lateness_are_fixed_from_creation_and_occurrence() {
        let created = time::macros::datetime!(2026-11-21 00:00 UTC);
        assert_eq!(
            expiry(created),
            time::macros::datetime!(2027-05-20 00:00 UTC)
        );
        assert!(!is_late(created - Duration::days(30), created));
        assert!(is_late(
            created - Duration::days(30) - Duration::seconds(1),
            created
        ));
    }
}
