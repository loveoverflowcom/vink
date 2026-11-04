//! Kani 0.67.0 / bundled nightly-2025-11-21 harnesses for the discussion core (owner-local
//! manifest `services/vink-app/kani`). Every harness calls the production function on symbolic
//! input over its whole domain: any `i64` version or generation, any `u32` page request, every
//! review/removal/viewer relation. Nothing here re-implements a rule. There are no loops, so no
//! unwind bound applies. Locking, transactions and I/O stay in the `postgres` lane; the serialized
//! multi-step lifecycle stays in the Quint model.

use uuid::Uuid;

use super::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Removal, Review, Transition,
};
use super::moderation::ModerationAction;
use super::page::{MAX_PAGE, PageLimit};
use super::policy::{Denied, authorize_writer_removal};
use super::settings::{
    CommentsMode, DiscussionSettings, ModeChange, SettingsError, SettingsGeneration,
};
use super::visibility::{CommentView, ReplyContext, can_reply_to, reply_context, view};
use crate::domain::draft::DraftVersion;
use crate::domain::ids::{CommentId, PrincipalId};

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
