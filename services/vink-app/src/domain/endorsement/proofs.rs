//! Kani 0.67.0 / bundled nightly-2025-11-21 harnesses for the literary endorsement core (#73),
//! compiled by the owner-local manifest `services/vink-app/kani` with the rest of `src/domain`.
//! Every harness calls the production function on symbolic input over its whole domain: any
//! `i64` version or expected version, any `u32` count, every review/removal/mode/generation of a
//! comment, every selection, intent and binding. Identities range over a few symbolic values,
//! enough to make every pair equal or distinct (three revisions: selected, published, other).
//! There are no loops, so no unwind bound is needed. One harness checks a two-request sequence
//! (the stream-order fence); longer interleavings, publication/moderation steps and the client
//! shell stay in `endorsement.qnt`; locking, transactions and I/O are #73 PR3.

use uuid::Uuid;

use super::comment::{
    CommentDecision, CommentFacts, CommentIntent, CommentRequest, comment_summary,
    decide_comment_endorsement, own_comment_view,
};
use super::work::{
    WorkDecision, WorkFacts, WorkIntent, WorkRequest, decide_work_endorsement, own_work_view,
    work_summary,
};
use super::{
    CommentAnchor, CountError, Endorsement, EndorsementCount, EndorsementVersion, OwnView, Summary,
    Transition,
};
use crate::domain::discussion::lifecycle::{CommentState, ModerationVersion, Removal, Review};
use crate::domain::discussion::settings::{CommentsMode, DiscussionSettings, SettingsGeneration};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

fn any_version() -> EndorsementVersion {
    let raw: i64 = kani::any();
    kani::assume(raw >= 1);
    EndorsementVersion::new(raw).unwrap()
}

fn revision() -> RevisionId {
    let raw: u8 = kani::any();
    kani::assume(raw < 3);
    RevisionId::from_uuid(Uuid::from_u128(u128::from(raw) + 1))
}

fn principal() -> PrincipalId {
    let raw: u8 = kani::any();
    kani::assume(raw < 3);
    PrincipalId::from_uuid(Uuid::from_u128(u128::from(raw) + 1))
}

fn work_id() -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(if kani::any() { 10 } else { 20 }))
}

fn comment_id() -> CommentId {
    CommentId::from_uuid(Uuid::from_u128(if kani::any() { 30 } else { 40 }))
}

fn any_selection() -> Option<RevisionId> {
    if kani::any() { Some(revision()) } else { None }
}

/// Any work endorsement storage could hand back.
fn any_work_endorsement() -> Endorsement<RevisionId> {
    Endorsement::restore(any_selection(), any_version())
}

fn any_comment_endorsement() -> Endorsement<CommentAnchor> {
    Endorsement::restore(kani::any::<bool>().then_some(CommentAnchor), any_version())
}

/// Any comment state storage could hand back, including combinations no transition produces.
fn any_state() -> CommentState {
    let review = match kani::any::<u8>() % 3 {
        0 => Review::Pending,
        1 => Review::Approved,
        _ => Review::Rejected,
    };
    let removal = match kani::any::<u8>() % 3 {
        0 => None,
        1 => Some(Removal::ByAuthor),
        _ => Some(Removal::ByModeration),
    };
    let raw: i64 = kani::any();
    kani::assume(raw >= 1);
    CommentState::restore(review, removal, ModerationVersion::new(raw).unwrap())
}

fn any_settings() -> DiscussionSettings {
    let mode = if kani::any() {
        CommentsMode::Open
    } else {
        CommentsMode::Closed
    };
    let raw: i64 = kani::any();
    kani::assume(raw >= 1);
    DiscussionSettings::restore(mode, SettingsGeneration::new(raw).unwrap())
}

/// LVE-INV-04/05, CHOICE-03: a request is accepted exactly when it carries the stored version;
/// every acceptance (a reaffirmation too) advances it by one without wrapping; a request at any
/// other version writes nothing and is `AlreadyCurrent` exactly when its desire already holds.
#[kani::proof]
fn transitions_accept_only_the_current_version_and_every_acceptance_advances_it() {
    let raw: i64 = kani::any();
    assert_eq!(
        EndorsementVersion::new(raw).map(EndorsementVersion::get),
        (raw >= 1).then_some(raw)
    );
    let current = any_work_endorsement();
    let expected = any_version();
    let desired = any_selection();
    let transition = current.transition(expected, desired);
    let advanced = current.version().get().checked_add(1);
    match transition {
        Transition::Changed { next, delta } => {
            assert_eq!(expected, current.version());
            assert_ne!(desired, current.selection());
            assert_eq!(next.selection(), desired);
            assert_eq!(advanced, Some(next.version().get()));
            assert_eq!(
                (delta.removed(), delta.added()),
                (current.selection(), desired)
            );
            assert_ne!(delta.removed(), delta.added());
        }
        Transition::Reaffirmed { next } => {
            assert_eq!(expected, current.version());
            assert_eq!(desired, current.selection());
            assert_eq!(next.selection(), current.selection());
            assert_eq!(advanced, Some(next.version().get()));
        }
        Transition::AlreadyCurrent(same) => {
            assert_ne!(expected, current.version());
            assert_eq!(desired, current.selection());
            assert_eq!(same, current);
        }
        Transition::Stale { current: seen } => {
            assert_ne!(expected, current.version());
            assert_ne!(desired, current.selection());
            assert_eq!(seen, current);
        }
        Transition::VersionExhausted => {
            assert_eq!(expected, current.version());
            assert_eq!(current.version().get(), i64::MAX);
        }
    }
    assert_eq!(
        transition.write().is_some(),
        expected == current.version() && advanced.is_some()
    );
    assert_eq!(
        transition.delta().is_some(),
        matches!(transition, Transition::Changed { .. })
    );
    kani::cover!(
        matches!(transition, Transition::Changed { delta, .. }
            if delta.removed().is_some() && delta.added().is_some()),
        "reconfirming at another revision is reachable"
    );
    kani::cover!(
        matches!(transition, Transition::Reaffirmed { .. }),
        "a reaffirmation is reachable"
    );
    kani::cover!(
        matches!(transition, Transition::AlreadyCurrent(_)),
        "a retry of the held state is reachable"
    );
    kani::cover!(
        matches!(transition, Transition::VersionExhausted),
        "the last version is reachable"
    );
}

/// LVE-FR-03, LVE-INV-05: once a request of a stream is confirmed (accepted, or found already
/// current), no request that stream issued earlier is accepted afterwards. A stream learns
/// versions from the server in order, so the earlier request carries a version no newer than
/// the later one, and neither is newer than the stored version. Includes the adversarial trace:
/// OFF@v, the older "endorse@v" delayed behind the newer "clear@v".
#[kani::proof]
fn a_confirmed_request_fences_every_earlier_request_of_its_stream() {
    let start = any_work_endorsement();
    let earlier = any_version();
    let later = any_version();
    kani::assume(earlier <= later && later <= start.version());
    let earlier_desire = any_selection();
    let later_desire = any_selection();
    let confirmed = start.transition(later, later_desire);
    let after = match confirmed {
        Transition::Changed { next, .. } | Transition::Reaffirmed { next } => next,
        Transition::AlreadyCurrent(same) => same,
        Transition::Stale { .. } | Transition::VersionExhausted => return,
    };
    let replayed = after.transition(earlier, earlier_desire);
    assert!(replayed.write().is_none());
    assert!(after.version() > earlier);
    kani::cover!(
        matches!(confirmed, Transition::Reaffirmed { .. })
            && earlier == later
            && earlier_desire.is_some()
            && start.selection().is_none(),
        "the delayed endorse behind a reaffirmed clear at the same version"
    );
    kani::cover!(
        matches!(confirmed, Transition::AlreadyCurrent(_))
            && matches!(replayed, Transition::Stale { .. }),
        "an earlier request after a confirmed retry"
    );
}

/// LVE-INV-01/02/06/12, CHOICE-02 C, CHOICE-08 A: a work endorsement needs the loaded work to be
/// the requested one and public; its author is refused; a selection is accepted only for the
/// revision the reader observed and is always that published revision; the own view and the
/// summary hide exactly what admission refuses as unavailable.
#[kani::proof]
fn work_admission_checks_binding_visibility_authorship_and_the_observed_revision() {
    let actor = principal();
    let work = work_id();
    let facts = if kani::any() {
        Some(WorkFacts {
            work: work_id(),
            author: principal(),
            published: any_selection(),
        })
    } else {
        None
    };
    let current = any_work_endorsement();
    let intent = if kani::any() {
        WorkIntent::Endorse {
            observed: revision(),
        }
    } else {
        WorkIntent::Clear
    };
    let request = WorkRequest {
        expected: any_version(),
        intent,
    };
    let decided = decide_work_endorsement(actor, work, request, facts, current);
    let public = facts
        .filter(|facts| facts.work == work)
        .and_then(|facts| facts.published);
    match decided {
        WorkDecision::TargetUnavailable => assert!(public.is_none()),
        WorkDecision::OwnWork => {
            assert!(public.is_some());
            assert_eq!(actor, facts.unwrap().author);
        }
        WorkDecision::RevisionChanged { published } => {
            assert_eq!(public, Some(published));
            assert_ne!(actor, facts.unwrap().author);
            assert!(matches!(intent, WorkIntent::Endorse { observed } if observed != published));
        }
        WorkDecision::Transition(transition) => {
            let published = public.unwrap();
            assert_ne!(actor, facts.unwrap().author);
            let desired = match intent {
                WorkIntent::Endorse { observed } => {
                    assert_eq!(observed, published);
                    Some(published)
                }
                WorkIntent::Clear => None,
            };
            assert_eq!(transition, current.transition(request.expected, desired));
            if let Some(selection) = transition.write().and_then(Endorsement::selection) {
                assert_eq!(selection, published);
            }
        }
    }
    let view = own_work_view(work, facts, current);
    assert_eq!(view == OwnView::Unavailable, public.is_none());
    if let Some(published) = public {
        assert_eq!(
            view == OwnView::Endorsed,
            current.selection() == Some(published)
        );
        assert_eq!(
            view == OwnView::EndorsedEarlierRevision,
            current
                .selection()
                .is_some_and(|selection| selection != published)
        );
    }
    let count = EndorsementCount::restore(kani::any());
    assert_eq!(
        work_summary(work, facts, count),
        if public.is_some() {
            Summary::Count(count)
        } else {
            Summary::Unavailable
        }
    );
    kani::cover!(
        facts.is_some_and(|facts| facts.work != work && facts.published.is_some()),
        "a public but foreign work is reachable (and unavailable)"
    );
    kani::cover!(
        matches!(decided, WorkDecision::RevisionChanged { .. }),
        "a publication that changed after reading is reachable"
    );
    kani::cover!(
        matches!(decided, WorkDecision::Transition(Transition::Changed { delta, .. })
            if delta.removed().is_some() && delta.added().is_some()),
        "reconfirming an earlier-revision endorsement is reachable"
    );
    kani::cover!(
        view == OwnView::EndorsedEarlierRevision,
        "an earlier-revision endorsement is reachable"
    );
}

/// LVE-INV-01/02/06/12, CHOICE-08 A, CHOICE-09 B: a comment endorsement needs the loaded comment
/// to be the requested one in the requested work's thread and public; its writer is refused;
/// closed comments refuse only a new selection, so clearing a visible comment is never refused
/// to a non-writer; the own view and the summary hide exactly the unavailable comments.
#[kani::proof]
fn comment_admission_checks_binding_visibility_authorship_and_comments_mode() {
    let actor = principal();
    let work = work_id();
    let comment = comment_id();
    let facts = if kani::any() {
        Some(CommentFacts {
            work: work_id(),
            comment: comment_id(),
            work_public: kani::any(),
            writer: principal(),
            state: any_state(),
            settings: any_settings(),
        })
    } else {
        None
    };
    let current = any_comment_endorsement();
    let intent = if kani::any() {
        CommentIntent::Endorse
    } else {
        CommentIntent::Clear
    };
    let request = CommentRequest {
        expected: any_version(),
        intent,
    };
    let decided = decide_comment_endorsement(actor, work, comment, request, facts, current);
    let visible = facts.filter(|facts| {
        facts.work == work
            && facts.comment == comment
            && facts.work_public
            && facts.state.is_public()
    });
    let desired = matches!(intent, CommentIntent::Endorse).then_some(CommentAnchor);
    let new_selection = desired.is_some() && desired != current.selection();
    match decided {
        CommentDecision::TargetUnavailable => assert!(visible.is_none()),
        CommentDecision::OwnComment => assert_eq!(actor, visible.unwrap().writer),
        CommentDecision::CommentsClosed => {
            let facts = visible.unwrap();
            assert_ne!(actor, facts.writer);
            assert!(new_selection);
            assert!(!facts.settings.accepts_new_comments());
        }
        CommentDecision::Transition(transition) => {
            let facts = visible.unwrap();
            assert_ne!(actor, facts.writer);
            assert!(!new_selection || facts.settings.accepts_new_comments());
            assert_eq!(transition, current.transition(request.expected, desired));
        }
    }
    if visible.is_some_and(|facts| facts.writer != actor) && desired.is_none() {
        assert!(matches!(decided, CommentDecision::Transition(_)));
    }
    let view = own_comment_view(work, comment, facts, current);
    assert_eq!(view == OwnView::Unavailable, visible.is_none());
    assert_ne!(view, OwnView::EndorsedEarlierRevision);
    let count = EndorsementCount::restore(kani::any());
    assert_eq!(
        comment_summary(work, comment, facts, count) == Summary::Unavailable,
        visible.is_none()
    );
    kani::cover!(
        matches!(decided, CommentDecision::Transition(Transition::Changed { delta, .. })
            if delta.removed().is_some())
            && facts.is_some_and(|facts| !facts.settings.accepts_new_comments()),
        "clearing while comments are closed is reachable"
    );
    kani::cover!(
        matches!(decided, CommentDecision::CommentsClosed),
        "closed comments refuse a new endorsement"
    );
    kani::cover!(
        facts.is_some_and(|facts| facts.comment != comment
            && facts.work == work
            && facts.work_public
            && facts.state.is_public()),
        "a public comment loaded under another identity is reachable (and unavailable)"
    );
}

/// LVE-INV-07: applying an accepted change's delta moves exactly the count of the anchor it
/// removes or adds, by one, never below zero or past `u32::MAX`, and fails atomically.
#[kani::proof]
fn counts_follow_deltas_without_underflow_or_wrapping() {
    let before = EndorsementCount::restore(kani::any());
    let current = any_work_endorsement();
    let Transition::Changed { delta, .. } = current.transition(current.version(), any_selection())
    else {
        return;
    };
    let anchor = revision();
    let removed = delta.removed() == Some(anchor);
    let added = delta.added() == Some(anchor);
    assert!(!(removed && added));
    let applied = before.apply(anchor, delta);
    match applied {
        Ok(after) => assert_eq!(
            i64::from(after.get()),
            i64::from(before.get()) - i64::from(removed) + i64::from(added)
        ),
        Err(CountError::Underflow) => {
            assert!(removed);
            assert_eq!(before.get(), 0);
        }
        Err(CountError::Overflow) => {
            assert!(added);
            assert_eq!(before.get(), u32::MAX);
        }
    }
    kani::cover!(
        applied == Err(CountError::Overflow),
        "a saturated count is reachable"
    );
    kani::cover!(
        applied == Err(CountError::Underflow),
        "a drifted projection is reachable"
    );
    kani::cover!(
        !removed && !added && applied == Ok(before),
        "an unrelated anchor is untouched"
    );
}
