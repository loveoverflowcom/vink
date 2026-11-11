//! Moderator hide and restore of an approved comment (ADMIN-MODERATION-20261111, MOD-13).
//!
//! A comment moderator hides a comment *that is approved and present*: the comment becomes a
//! rejected review on the lifecycle axes (so every existing read path stops returning it) and a
//! [`HiddenApproval`] remembers exactly which state was hidden. Restoring is only lawful while the
//! comment is still exactly that hidden state: same moderation version, still present, still
//! rejected. Any later change (the writer removed it, another decision, the old terminal
//! moderation removal) makes the restore stale. A restore re-approves the comment, so it follows
//! the original visibility rules again (root/reply listing, anchors, reply context) through
//! [`super::visibility`]; nothing here bypasses them.
//!
//! The writer's own removal and the older terminal [`super::lifecycle::Removal::ByModeration`]
//! keep working unchanged.

use super::lifecycle::{
    CommentState, Decision, LifecycleError, ModerationVersion, Review, Transition,
};
use super::moderation::ModerationReason;
use crate::domain::ids::{CommentId, PrincipalId};

/// Record of one hide: which comment, at which approved version, the hidden version it produced,
/// who hid it and why (shown to the writer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenApproval {
    pub comment: CommentId,
    pub approved_version: ModerationVersion,
    pub hidden_version: ModerationVersion,
    pub by: PrincipalId,
    pub reason: ModerationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HideError {
    #[error("only an approved, present comment can be hidden")]
    NotApproved,
    #[error("the comment changed since it was hidden")]
    Stale,
    #[error("the hide record belongs to another comment")]
    WrongComment,
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
}

impl HideError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApproved => "not_approved",
            Self::Stale => "stale",
            Self::WrongComment => "wrong_comment",
            Self::Lifecycle(LifecycleError::Removed) => "removed",
            Self::Lifecycle(LifecycleError::Stale { .. }) => "stale",
            Self::Lifecycle(LifecycleError::VersionExhausted) => "version_exhausted",
        }
    }
}

fn changed(transition: Transition) -> Result<CommentState, HideError> {
    match transition {
        Transition::Changed(state) => Ok(state),
        Transition::Unchanged(_) => Err(HideError::Stale),
    }
}

/// Hide the approved comment the moderator saw at `expected`.
pub fn hide(
    comment: CommentId,
    state: CommentState,
    expected: ModerationVersion,
    by: PrincipalId,
    reason: ModerationReason,
) -> Result<(CommentState, HiddenApproval), HideError> {
    if !state.is_public() {
        return Err(if state.is_present() {
            HideError::NotApproved
        } else {
            LifecycleError::Removed.into()
        });
    }
    let hidden = changed(state.decide(expected, Decision::Reject)?)?;
    Ok((
        hidden,
        HiddenApproval {
            comment,
            approved_version: state.version(),
            hidden_version: hidden.version(),
            by,
            reason,
        },
    ))
}

/// Restore exactly the approval that `record` hid.
pub fn restore(
    comment: CommentId,
    state: CommentState,
    record: &HiddenApproval,
) -> Result<CommentState, HideError> {
    if record.comment != comment {
        return Err(HideError::WrongComment);
    }
    if !state.is_present() {
        return Err(LifecycleError::Removed.into());
    }
    if state.version() != record.hidden_version || state.review() != Review::Rejected {
        return Err(HideError::Stale);
    }
    changed(state.decide(record.hidden_version, Decision::Approve)?)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::discussion::visibility::{CommentView, view};

    fn principal(id: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(id))
    }

    fn comment(id: u128) -> CommentId {
        CommentId::from_uuid(Uuid::from_u128(id))
    }

    fn approved() -> CommentState {
        match CommentState::SUBMITTED.decide(ModerationVersion::FIRST, Decision::Approve) {
            Ok(Transition::Changed(state)) => state,
            other => panic!("{other:?}"),
        }
    }

    fn reason() -> ModerationReason {
        ModerationReason::parse("lạc đề").unwrap()
    }

    #[test]
    fn hide_then_restore_brings_back_exactly_the_approved_comment() {
        let writer = principal(5);
        let state = approved();
        let (hidden, record) =
            hide(comment(1), state, state.version(), principal(3), reason()).unwrap();
        assert_eq!(view(writer, hidden, None), CommentView::Unavailable);
        assert_eq!(view(writer, hidden, Some(writer)), CommentView::OwnRejected);
        let restored = restore(comment(1), hidden, &record).unwrap();
        assert_eq!(view(writer, restored, None), CommentView::Public);
        assert!(restored.version() > hidden.version());
        // A second restore with the same record is stale: the version moved.
        assert_eq!(
            restore(comment(1), restored, &record),
            Err(HideError::Stale)
        );
    }

    #[test]
    fn hide_needs_an_approved_present_comment_at_the_seen_version() {
        assert_eq!(
            hide(
                comment(1),
                CommentState::SUBMITTED,
                ModerationVersion::FIRST,
                principal(3),
                reason()
            ),
            Err(HideError::NotApproved)
        );
        let state = approved();
        assert!(matches!(
            hide(
                comment(1),
                state,
                ModerationVersion::FIRST,
                principal(3),
                reason()
            ),
            Err(HideError::Lifecycle(LifecycleError::Stale { .. }))
        ));
    }

    #[test]
    fn writer_removal_or_a_later_decision_makes_restore_stale() {
        let state = approved();
        let (hidden, record) =
            hide(comment(1), state, state.version(), principal(3), reason()).unwrap();
        let Ok(Transition::Changed(removed)) = hidden.remove_by_author() else {
            panic!("writer removal applies");
        };
        assert_eq!(
            restore(comment(1), removed, &record),
            Err(HideError::Lifecycle(LifecycleError::Removed))
        );
        let Ok(Transition::Changed(reapproved)) =
            hidden.decide(hidden.version(), Decision::Approve)
        else {
            panic!("a direct decision applies");
        };
        let Ok(Transition::Changed(rejected)) =
            reapproved.decide(reapproved.version(), Decision::Reject)
        else {
            panic!("a direct decision applies");
        };
        assert_eq!(
            restore(comment(1), rejected, &record),
            Err(HideError::Stale)
        );
        assert_eq!(
            restore(comment(2), hidden, &record),
            Err(HideError::WrongComment)
        );
    }
}
