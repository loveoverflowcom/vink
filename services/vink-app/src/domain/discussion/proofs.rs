//! Kani 0.67.0 / bundled nightly-2025-11-21 harnesses for the discussion core (owner-local
//! manifest `services/vink-app/kani`). Every harness calls the production function on symbolic
//! input over its whole domain: any `i64` version, generation or cursor position, any `u32` page
//! request or count, any 32-byte digest, every review/removal/viewer relation and reaction kind.
//! Identities range over a few symbolic values, enough to make every pair equal or distinct.
//! Nothing here re-implements a rule; the cursor bounds are the `time` calendar range written as
//! constants, an independent oracle. The one fixed-length loop (four tally slots) is unrolled
//! with unwinding assertions on. Locking, transactions and I/O stay in the `postgres` lane; the
//! serialized multi-step lifecycle, receipts and reactions stay in the Quint models.

use uuid::Uuid;

use super::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Removal, Review, Transition,
};
use super::moderation::ModerationAction;
use super::page::{MAX_PAGE, PageCursor, PageLimit};
use super::policy::{Denied, authorize_writer_removal};
use super::reaction::{
    ActorReaction, KINDS, ReactionChange, ReactionDecision, ReactionError, ReactionFacts,
    ReactionGeneration, ReactionKind, ReactionTally, TallyError, decide_reaction, public_tally,
};
use super::settings::{
    CommentsMode, DiscussionSettings, ModeChange, SettingsError, SettingsGeneration,
};
use super::submission::{
    PayloadDigest, Receipt, SubmissionDecision, SubmissionFacts, TargetFacts, decide_submission,
};
use super::visibility::{CommentView, ReplyContext, can_reply_to, reply_context, view};
use super::{CommentPlacement, StoredPosition, StructureError, reply_placement, root_placement};
use crate::domain::draft::DraftVersion;
use crate::domain::ids::{CommentId, PrincipalId, ThreadId};

fn any_review() -> Review {
    match kani::any::<u8>() % 3 {
        0 => Review::Pending,
        1 => Review::Approved,
        _ => Review::Rejected,
    }
}

fn any_removal() -> Option<Removal> {
    match kani::any::<u8>() % 3 {
        0 => None,
        1 => Some(Removal::ByAuthor),
        _ => Some(Removal::ByModeration),
    }
}

fn any_version() -> ModerationVersion {
    let raw: i64 = kani::any();
    kani::assume(raw >= 1);
    ModerationVersion::new(raw).unwrap()
}

/// Any state storage could hand back, including combinations no transition produces.
fn any_state() -> CommentState {
    CommentState::restore(any_review(), any_removal(), any_version())
}

fn any_action() -> ModerationAction {
    match kani::any::<u8>() % 3 {
        0 => ModerationAction::Approve,
        1 => ModerationAction::Reject,
        _ => ModerationAction::Remove,
    }
}

fn any_mode() -> CommentsMode {
    if kani::any() {
        CommentsMode::Open
    } else {
        CommentsMode::Closed
    }
}

fn any_generation() -> SettingsGeneration {
    let raw: i64 = kani::any();
    kani::assume(raw >= 1);
    SettingsGeneration::new(raw).unwrap()
}

/// Three principals are enough to separate writer, work author and anyone else.
fn principal() -> PrincipalId {
    let raw: u8 = kani::any();
    kani::assume(raw < 3);
    PrincipalId::from_uuid(Uuid::from_u128(u128::from(raw) + 1))
}

/// Four comment identities: enough for a new comment, its target, the target's root and another.
fn comment() -> CommentId {
    let raw: u8 = kani::any();
    kani::assume(raw < 4);
    CommentId::from_uuid(Uuid::from_u128(u128::from(raw) + 1))
}

fn thread() -> ThreadId {
    ThreadId::from_uuid(Uuid::from_u128(if kani::any() { 10 } else { 20 }))
}

/// Any placement storage could hand back: a root or a reply with any root and context.
fn any_placement() -> Option<CommentPlacement> {
    let position = if kani::any() {
        StoredPosition::Root
    } else {
        StoredPosition::Reply {
            root_id: comment(),
            reply_to: if kani::any() { Some(comment()) } else { None },
        }
    };
    CommentPlacement::from_stored(comment(), thread(), position).ok()
}

fn any_kind() -> ReactionKind {
    match kani::any::<u8>() % 4 {
        0 => ReactionKind::Heart,
        1 => ReactionKind::Moved,
        2 => ReactionKind::Thoughtful,
        _ => ReactionKind::Applause,
    }
}

fn any_desire() -> Option<ReactionKind> {
    if kani::any() { Some(any_kind()) } else { None }
}

fn any_reaction_generation() -> ReactionGeneration {
    let raw: i64 = kani::any();
    kani::assume(raw >= 1);
    ReactionGeneration::new(raw).unwrap()
}

fn any_reaction() -> ActorReaction {
    ActorReaction::restore(any_desire(), any_reaction_generation())
}

/// Raw numbers become refined scalars exactly when their predicate holds, and keep their value.
#[kani::proof]
fn refined_scalars_accept_exactly_their_predicate() {
    let raw: i64 = kani::any();
    let accepted = raw >= 1;
    assert_eq!(
        ModerationVersion::new(raw).map(ModerationVersion::get),
        accepted.then_some(raw)
    );
    assert_eq!(
        SettingsGeneration::new(raw).map(SettingsGeneration::get),
        accepted.then_some(raw)
    );
    assert_eq!(
        DraftVersion::new(raw).map(DraftVersion::get),
        accepted.then_some(raw)
    );

    assert_eq!(
        ReactionGeneration::new(raw).map(ReactionGeneration::get),
        accepted.then_some(raw)
    );

    let requested: Option<u32> = kani::any();
    match PageLimit::new(requested) {
        Ok(limit) => {
            assert!((1..=MAX_PAGE).contains(&limit.get()));
            if let Some(raw) = requested {
                assert_eq!(u32::from(limit.get()), raw);
            }
        }
        Err(_) => {
            let raw = requested.unwrap();
            assert!(raw == 0 || raw > u32::from(MAX_PAGE));
        }
    }
    kani::cover!(
        requested == Some(u32::from(MAX_PAGE)),
        "largest page is reachable"
    );
}

/// MOD-02 and D2T-INV-07: a moderator's decision changes a comment only at the version they saw,
/// advances it by exactly one without wrapping, and never revives a removed comment.
#[kani::proof]
fn moderation_decisions_are_version_bound_and_removal_is_terminal() {
    let state = any_state();
    let expected = any_version();
    let action = any_action();
    let decided = action.apply(state, expected);
    match decided {
        Ok(Transition::Changed(next)) => {
            assert!(state.is_present());
            assert_eq!(expected, state.version());
            assert_eq!(
                state.version().get().checked_add(1),
                Some(next.version().get())
            );
            assert_ne!(
                (next.review(), next.removal()),
                (state.review(), state.removal())
            );
            match action {
                ModerationAction::Approve => assert!(next.is_public()),
                ModerationAction::Reject => {
                    assert_eq!(next.review(), Review::Rejected);
                    assert!(!next.is_public());
                }
                ModerationAction::Remove => {
                    assert_eq!(next.review(), state.review());
                    assert_eq!(next.removal(), Some(Removal::ByModeration));
                }
            }
        }
        Ok(Transition::Unchanged(same)) => {
            assert_eq!(same, state);
            if state.is_present() {
                assert_eq!(expected, state.version());
            } else {
                assert_eq!(action, ModerationAction::Remove);
            }
        }
        Err(LifecycleError::Removed) => {
            assert!(!state.is_present());
            assert_ne!(action, ModerationAction::Remove);
        }
        Err(LifecycleError::Stale { current }) => {
            assert!(state.is_present());
            assert_eq!(current, state.version());
            assert_ne!(expected, current);
        }
        Err(LifecycleError::VersionExhausted) => {
            assert!(state.is_present());
            assert_eq!(state.version().get(), i64::MAX);
        }
    }
    kani::cover!(
        matches!(decided, Ok(Transition::Changed(_))),
        "a decision can change a comment"
    );
    kani::cover!(
        matches!(decided, Err(LifecycleError::Stale { .. })),
        "a stale decision is reachable"
    );
    kani::cover!(
        matches!(decided, Err(LifecycleError::VersionExhausted)),
        "the last version is reachable"
    );
}

/// The writer withdraws without a version; withdrawal is terminal and hides the text.
#[kani::proof]
fn writer_removal_is_terminal_and_hides_the_comment() {
    let state = any_state();
    match state.remove_by_author() {
        Ok(Transition::Changed(next)) => {
            assert!(state.is_present());
            assert_eq!(next.removal(), Some(Removal::ByAuthor));
            assert_eq!(next.review(), state.review());
            assert_eq!(
                state.version().get().checked_add(1),
                Some(next.version().get())
            );
            assert!(!next.is_public());
        }
        Ok(Transition::Unchanged(same)) => {
            assert!(!state.is_present());
            assert_eq!(same, state);
        }
        Err(LifecycleError::VersionExhausted) => {
            assert!(state.is_present());
            assert_eq!(state.version().get(), i64::MAX);
        }
        Err(LifecycleError::Removed | LifecycleError::Stale { .. }) => {
            panic!("writer removal is neither version-bound nor refused once removed")
        }
    }
}

/// COM-07 and D-13: a mode change needs the generation the author saw; asking for the current mode
/// is a no-op at any generation, so a retry cannot flip it back.
#[kani::proof]
fn comments_mode_changes_are_generation_bound() {
    let settings = DiscussionSettings::restore(any_mode(), any_generation());
    let expected = any_generation();
    let wanted = any_mode();
    let changed = settings.change_mode(expected, wanted);
    match changed {
        Ok(ModeChange::Changed(next)) => {
            assert_ne!(wanted, settings.mode());
            assert_eq!(expected, settings.generation());
            assert_eq!(next.mode(), wanted);
            assert_eq!(
                settings.generation().get().checked_add(1),
                Some(next.generation().get())
            );
        }
        Ok(ModeChange::Unchanged(same)) => {
            assert_eq!(wanted, settings.mode());
            assert_eq!(same, settings);
        }
        Err(SettingsError::Stale { current }) => {
            assert_ne!(wanted, settings.mode());
            assert_ne!(expected, settings.generation());
            assert_eq!(current, settings);
        }
        Err(SettingsError::GenerationExhausted) => {
            assert_eq!(settings.generation().get(), i64::MAX);
        }
    }
    kani::cover!(
        matches!(changed, Err(SettingsError::GenerationExhausted)),
        "the last generation is reachable"
    );
}

/// D2T-INV-04/05 and COM-08: only present and approved comments are public. Anyone but the writer,
/// including the work's author, sees nothing else and cannot learn that a hidden comment exists.
#[kani::proof]
fn viewer_projection_never_reveals_a_hidden_comment() {
    let writer = principal();
    let actor = principal();
    let viewer = if kani::any() { Some(actor) } else { None };
    let state = any_state();
    let seen = view(writer, state, viewer);

    assert_eq!(seen == CommentView::Public, state.is_public());
    if viewer != Some(writer) {
        assert!(matches!(
            seen,
            CommentView::Public | CommentView::Unavailable
        ));
    }
    if !state.is_present() {
        assert_eq!(seen, CommentView::Unavailable);
    }
    if viewer == Some(writer) && state.is_present() {
        assert_ne!(seen, CommentView::Unavailable);
    }

    assert_eq!(can_reply_to(state), state.is_public());
    let context = CommentId::from_uuid(Uuid::from_u128(7));
    let shown = reply_context(true, Some((context, state)));
    assert_eq!(shown == ReplyContext::Reply(context), state.is_public());

    let removal = authorize_writer_removal(actor, writer, view(writer, state, Some(actor)));
    if actor == writer {
        assert_eq!(removal, Ok(()));
    } else if state.is_public() {
        assert_eq!(removal, Err(Denied::NotCommentWriter));
    } else {
        assert_eq!(removal, Err(Denied::Hidden));
    }
    kani::cover!(
        viewer == Some(writer) && seen == CommentView::OwnRejected,
        "the writer sees their rejected comment"
    );
}

/// D2T-INV-09: a decoded cursor position is accepted exactly when it is a representable
/// timestamp and keeps its identity, so `decode` neither clamps a forged position onto another
/// key nor rejects a position the server could have issued. Converting back to a timestamp
/// (`created_at`, `after`) over the whole calendar did not finish within 7 minutes in CBMC; unit
/// tests check that round trip at the extremes and in between.
#[kani::proof]
fn page_cursors_accept_exactly_the_representable_positions() {
    // -9999-01-01T00:00:00Z and 9999-12-31T23:59:59.999999Z in microseconds.
    const MIN_MICROS: i64 = -377_705_116_800_000_000;
    const MAX_MICROS: i64 = 253_402_300_799_999_999;
    let micros: i64 = kani::any();
    let id = CommentId::from_uuid(Uuid::from_u128(kani::any()));
    let decoded = PageCursor::at_micros(micros, id);
    assert_eq!(decoded.is_ok(), (MIN_MICROS..=MAX_MICROS).contains(&micros));
    if let Ok(cursor) = decoded {
        assert_eq!(cursor.id(), id);
    }
    kani::cover!(micros == MAX_MICROS, "the last representable position");
    kani::cover!(micros == MIN_MICROS, "the first representable position");
}

/// D2T-INV-01/02: the shared reply seam only accepts a target of the same thread, always yields
/// a reply under the target's root, keeps a reply target as context and a root target as `None`,
/// and never produces a shape storage would reject (self root, self or root context).
#[kani::proof]
fn reply_placement_keeps_two_tiers_inside_one_thread() {
    let Some(target) = any_placement() else {
        return;
    };
    let thread_id = thread();
    let id = comment();
    match reply_placement(thread_id, id, target) {
        Ok(placement) => {
            assert_eq!(placement.thread_id(), target.thread_id());
            assert_eq!(placement.thread_id(), thread_id);
            assert_eq!(placement.id(), id);
            assert!(!placement.is_root());
            assert_eq!(placement.root_id(), target.root_id());
            let expected_context = (!target.is_root()).then_some(target.id());
            assert_eq!(placement.reply_to(), expected_context);
            let stored = StoredPosition::Reply {
                root_id: placement.root_id().as_comment_id(),
                reply_to: placement.reply_to(),
            };
            assert_eq!(
                CommentPlacement::from_stored(id, thread_id, stored),
                Ok(placement)
            );
        }
        Err(StructureError::TargetNotFound) => assert_ne!(thread_id, target.thread_id()),
        Err(StructureError::CommentExists) => {
            assert_eq!(thread_id, target.thread_id());
            assert!(id == target.id() || id == target.root_id().as_comment_id());
        }
    }
    let root = root_placement(thread_id, id);
    assert!(root.is_root() && root.reply_to().is_none());
    assert_eq!(root.root_id().as_comment_id(), id);
    kani::cover!(
        !target.is_root() && reply_placement(thread_id, id, target).is_ok(),
        "a reply to a reply is reachable"
    );
    kani::cover!(
        !target.is_root()
            && id == target.root_id().as_comment_id()
            && thread_id == target.thread_id(),
        "an identity colliding with the target's root is reachable"
    );
}

/// D2T-INV-06 with INV-02/03: a receipt for the key decides first, replaying exactly the same
/// payload and refusing any other, whatever the work, mode or target now are. Without one, an
/// insert happens only when the work is public, comments are open and a named target is a public
/// comment of this thread; the new reply always sits under that target's root.
#[kani::proof]
fn submission_receipts_take_precedence_and_inserts_pass_every_gate() {
    let thread_id = thread();
    let comment_id = comment();
    let digest = PayloadDigest::from_bytes(&kani::any::<[u8; 32]>()).unwrap();
    let receipt = if kani::any() {
        Some(Receipt {
            digest: PayloadDigest::from_bytes(&kani::any::<[u8; 32]>()).unwrap(),
            comment_id: comment(),
        })
    } else {
        None
    };
    let target = any_placement().map(|placement| TargetFacts {
        placement,
        state: any_state(),
    });
    let requested = if kani::any() { Some(comment()) } else { None };
    let facts = SubmissionFacts {
        receipt,
        work_public: kani::any(),
        settings: DiscussionSettings::restore(any_mode(), any_generation()),
        target,
    };
    let decided = decide_submission(thread_id, comment_id, digest, requested, facts);
    if let Some(receipt) = receipt {
        if receipt.digest == digest {
            assert_eq!(decided, SubmissionDecision::Replay(receipt.comment_id));
        } else {
            assert_eq!(decided, SubmissionDecision::KeyReused);
        }
        kani::cover!(
            !facts.settings.accepts_new_comments() && receipt.digest == digest,
            "a retry replays after comments close"
        );
        return;
    }
    match decided {
        SubmissionDecision::Insert(placement) => {
            assert!(facts.work_public && facts.settings.accepts_new_comments());
            assert_eq!(placement.id(), comment_id);
            assert_eq!(placement.thread_id(), thread_id);
            match requested {
                None => assert!(placement.is_root()),
                Some(named) => {
                    let target = target.unwrap();
                    assert_eq!(target.placement.id(), named);
                    assert!(target.state.is_public());
                    assert_eq!(target.placement.thread_id(), thread_id);
                    assert_eq!(placement.root_id(), target.placement.root_id());
                }
            }
        }
        SubmissionDecision::WorkNotPublic => assert!(!facts.work_public),
        SubmissionDecision::CommentsClosed => {
            assert!(facts.work_public && !facts.settings.accepts_new_comments());
        }
        SubmissionDecision::TargetUnavailable => {
            assert!(facts.work_public && facts.settings.accepts_new_comments());
            let named = requested.unwrap();
            assert!(target.is_none_or(|target| target.placement.id() != named
                || !target.state.is_public()
                || target.placement.thread_id() != thread_id));
        }
        SubmissionDecision::IdentityCollision => {
            let target = target.unwrap();
            assert!(target.state.is_public());
            assert!(
                comment_id == target.placement.id()
                    || comment_id == target.placement.root_id().as_comment_id()
            );
        }
        SubmissionDecision::Replay(_) | SubmissionDecision::KeyReused => {
            panic!("no receipt, no replay")
        }
    }
    kani::cover!(
        matches!(decided, SubmissionDecision::Insert(p) if !p.is_root()),
        "a reply insert is reachable"
    );
}

/// D2T-INV-08: a reaction request is desired state. Asking for the current reaction is a no-op
/// at any generation (never a toggle); a change needs the generation the actor saw, advances it
/// by exactly one without wrapping and reports the delta it applies.
#[kani::proof]
fn reaction_changes_are_desired_state_and_generation_bound() {
    let current = any_reaction();
    let expected = any_reaction_generation();
    let wanted = any_desire();
    let changed = current.desire(expected, wanted);
    match changed {
        Ok(ReactionChange::Changed { next, delta }) => {
            assert_ne!(wanted, current.kind());
            assert_eq!(expected, current.generation());
            assert_eq!(next.kind(), wanted);
            assert_eq!(
                current.generation().get().checked_add(1),
                Some(next.generation().get())
            );
            assert_eq!((delta.removed(), delta.added()), (current.kind(), wanted));
            assert_ne!(delta.removed(), delta.added());
        }
        Ok(ReactionChange::Unchanged(same)) => {
            assert_eq!(wanted, current.kind());
            assert_eq!(same, current);
        }
        Err(ReactionError::Stale { current: seen }) => {
            assert_ne!(wanted, current.kind());
            assert_ne!(expected, current.generation());
            assert_eq!(seen, current);
        }
        Err(ReactionError::GenerationExhausted) => {
            assert_ne!(wanted, current.kind());
            assert_eq!(current.generation().get(), i64::MAX);
        }
    }
    kani::cover!(
        matches!(changed, Ok(ReactionChange::Changed { delta, .. })
            if delta.removed().is_some() && delta.added().is_some()),
        "replacing one kind by another is reachable"
    );
    kani::cover!(
        matches!(changed, Err(ReactionError::GenerationExhausted)),
        "the last generation is reachable"
    );
}

/// D2T-INV-08 interaction limits: only a public comment of a public work takes a reaction, and a
/// hidden one answers like a missing one; writers never react to their own comment; closing
/// comments stops new reactions but never stops an actor from clearing theirs.
#[kani::proof]
fn reaction_admission_enforces_the_interaction_limits() {
    let actor = principal();
    let writer = principal();
    let state = any_state();
    let target = if kani::any() {
        Some((writer, state))
    } else {
        None
    };
    let facts = ReactionFacts {
        work_public: kani::any(),
        settings: DiscussionSettings::restore(any_mode(), any_generation()),
        target,
        current: any_reaction(),
    };
    let expected = any_reaction_generation();
    let wanted = any_desire();
    let decided = decide_reaction(actor, expected, wanted, facts);
    let reachable = facts.work_public && target.is_some() && state.is_public();
    let new_interaction = wanted.is_some() && wanted != facts.current.kind();
    match decided {
        ReactionDecision::Apply { next, delta } => {
            assert!(reachable);
            assert_eq!(next.kind(), wanted);
            assert_eq!(expected, facts.current.generation());
            assert_eq!(
                (delta.removed(), delta.added()),
                (facts.current.kind(), wanted)
            );
            if wanted.is_some() {
                assert_ne!(actor, writer);
                assert!(facts.settings.accepts_new_comments());
            }
        }
        ReactionDecision::Unchanged(same) => {
            assert!(reachable);
            assert_eq!(same, facts.current);
            assert_eq!(wanted, facts.current.kind());
        }
        ReactionDecision::Stale { current } => {
            assert!(reachable);
            assert_eq!(current, facts.current);
            assert_ne!(expected, current.generation());
        }
        ReactionDecision::WorkNotPublic => assert!(!facts.work_public),
        ReactionDecision::TargetUnavailable => assert!(facts.work_public && !reachable),
        ReactionDecision::OwnComment => {
            assert!(reachable && new_interaction);
            assert_eq!(actor, writer);
        }
        ReactionDecision::CommentsClosed => {
            assert!(reachable && new_interaction);
            assert_ne!(actor, writer);
            assert!(!facts.settings.accepts_new_comments());
        }
        ReactionDecision::GenerationExhausted => {
            assert!(reachable);
            assert_eq!(facts.current.generation().get(), i64::MAX);
        }
    }
    if reachable && wanted.is_none() {
        assert!(!matches!(
            decided,
            ReactionDecision::OwnComment | ReactionDecision::CommentsClosed
        ));
    }
    kani::cover!(
        matches!(decided, ReactionDecision::Apply { next, .. } if next.kind().is_none())
            && !facts.settings.accepts_new_comments(),
        "clearing while comments are closed is reachable"
    );
    kani::cover!(
        matches!(decided, ReactionDecision::CommentsClosed),
        "a closed work refuses a new reaction"
    );
}

/// D2T-INV-08 counts: applying a real change's delta moves the total by exactly that change,
/// touches only the two kinds involved, never goes below zero or wraps, and fails atomically.
/// A hidden comment exposes no tally at all.
#[kani::proof]
#[kani::unwind(5)]
fn reaction_tallies_follow_deltas_without_underflow_or_wrapping() {
    let before = ReactionTally::restore(kani::any::<[u32; KINDS]>());
    let current = any_reaction();
    let Ok(ReactionChange::Changed { delta, .. }) =
        current.desire(current.generation(), any_desire())
    else {
        return;
    };
    match before.apply(delta) {
        Ok(after) => {
            let removed = u64::from(delta.removed().is_some());
            let added = u64::from(delta.added().is_some());
            assert_eq!(after.total() + removed, before.total() + added);
            for kind in ReactionKind::ALL {
                let count = i64::from(before.count(kind))
                    - i64::from(delta.removed() == Some(kind))
                    + i64::from(delta.added() == Some(kind));
                assert_eq!(i64::from(after.count(kind)), count);
            }
        }
        Err(TallyError::Underflow) => {
            assert_eq!(before.count(delta.removed().unwrap()), 0);
        }
        Err(TallyError::Overflow) => {
            assert_eq!(before.count(delta.added().unwrap()), u32::MAX);
        }
    }
    let state = any_state();
    assert_eq!(public_tally(state, before).is_some(), state.is_public());
    kani::cover!(
        before.apply(delta) == Err(TallyError::Overflow),
        "a saturated count is reachable"
    );
}
