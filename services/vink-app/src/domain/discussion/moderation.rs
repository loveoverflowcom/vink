//! System moderation of comments (COM-02, MOD-02, MOD-04). Who may decide is in
//! [`super::policy::authorize_moderation`]; this module owns what a decision is: its action, the
//! reason shown to the writer, and the single transition dispatch the store applies.
//!
//! Authority comes from an active moderator grant in Vink's own database, created and revoked by
//! an operator command and rechecked in each decision's transaction. No request field, session
//! claim or work ownership confers it.

use unicode_normalization::UnicodeNormalization;

use super::lifecycle::{CommentState, Decision, LifecycleError, ModerationVersion, Transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationAction {
    Approve,
    Reject,
    Remove,
}

impl ModerationAction {
    /// The one dispatch from an action to its lifecycle transition, used by every store.
    pub fn apply(
        self,
        state: CommentState,
        expected: ModerationVersion,
    ) -> Result<Transition, LifecycleError> {
        match self {
            Self::Approve => state.decide(expected, Decision::Approve),
            Self::Reject => state.decide(expected, Decision::Reject),
            Self::Remove => state.remove_by_moderation(expected),
        }
    }
}

pub const MAX_REASON_CHARS: usize = 500;

/// Why a moderator rejected or removed a comment, written for its writer to act on (MOD-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationReason(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReasonError {
    #[error("reject and remove need a reason the writer can act on")]
    Required,
    #[error("reason is empty")]
    Empty,
    #[error("reason is longer than {MAX_REASON_CHARS} characters")]
    TooLong,
    #[error("reason contains a control character")]
    ControlCharacter,
}

impl ModerationReason {
    pub fn parse(raw: &str) -> Result<Self, ReasonError> {
        let text: String = raw.replace("\r\n", "\n").trim().nfc().collect();
        if text.is_empty() {
            return Err(ReasonError::Empty);
        }
        if text.chars().count() > MAX_REASON_CHARS {
            return Err(ReasonError::TooLong);
        }
        if text.chars().any(|c| c.is_control() && c != '\n') {
            return Err(ReasonError::ControlCharacter);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reject and remove must explain themselves; an approval may carry an optional note.
pub fn decision_reason(
    action: ModerationAction,
    raw: Option<&str>,
) -> Result<Option<ModerationReason>, ReasonError> {
    match (action, raw) {
        (ModerationAction::Approve, None) => Ok(None),
        (_, Some(raw)) => ModerationReason::parse(raw).map(Some),
        (ModerationAction::Reject | ModerationAction::Remove, None) => Err(ReasonError::Required),
    }
}

pub const MAX_NOTE_CHARS: usize = 200;

/// Operator's record of why a moderator grant exists (who asked, which policy). One line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantNote(String);

impl GrantNote {
    pub fn parse(raw: &str) -> Option<Self> {
        let text: String = raw.trim().nfc().collect();
        let valid = !text.is_empty()
            && text.chars().count() <= MAX_NOTE_CHARS
            && !text.chars().any(char::is_control);
        valid.then_some(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::discussion::lifecycle::Review;

    #[test]
    fn reject_and_remove_need_a_reason_approval_does_not() {
        assert_eq!(decision_reason(ModerationAction::Approve, None), Ok(None));
        for action in [ModerationAction::Reject, ModerationAction::Remove] {
            assert_eq!(decision_reason(action, None), Err(ReasonError::Required));
            assert_eq!(
                decision_reason(action, Some(" \n ")),
                Err(ReasonError::Empty)
            );
        }
        let reason = decision_reason(
            ModerationAction::Reject,
            Some("  Bình luận nêu tên thật của người khác.\r\nHãy viết lại.  "),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            reason.as_str(),
            "Bình luận nêu tên thật của người khác.\nHãy viết lại."
        );
        assert_eq!(
            ModerationReason::parse(&"ạ".repeat(MAX_REASON_CHARS + 1)),
            Err(ReasonError::TooLong)
        );
        assert_eq!(
            ModerationReason::parse("a\u{0007}b"),
            Err(ReasonError::ControlCharacter)
        );
    }

    #[test]
    fn actions_dispatch_to_the_version_bound_transitions() {
        let first = ModerationVersion::FIRST;
        let Ok(Transition::Changed(approved)) =
            ModerationAction::Approve.apply(CommentState::SUBMITTED, first)
        else {
            panic!("approve")
        };
        assert_eq!(approved.review(), Review::Approved);
        let Ok(Transition::Changed(rejected)) =
            ModerationAction::Reject.apply(CommentState::SUBMITTED, first)
        else {
            panic!("reject")
        };
        assert_eq!(rejected.review(), Review::Rejected);
        let Ok(Transition::Changed(removed)) =
            ModerationAction::Remove.apply(CommentState::SUBMITTED, first)
        else {
            panic!("remove")
        };
        assert!(!removed.is_present());
        for action in [
            ModerationAction::Approve,
            ModerationAction::Reject,
            ModerationAction::Remove,
        ] {
            assert!(matches!(
                action.apply(approved, first),
                Err(LifecycleError::Stale { .. })
            ));
        }
    }

    #[test]
    fn grant_notes_are_one_bounded_line() {
        assert_eq!(
            GrantNote::parse("  Ban biên tập, theo yêu cầu #52 ")
                .unwrap()
                .as_str(),
            "Ban biên tập, theo yêu cầu #52"
        );
        for bad in ["", "   ", "hai\ndòng", &"x".repeat(MAX_NOTE_CHARS + 1)] {
            assert_eq!(GrantNote::parse(bad), None, "{bad:?}");
        }
    }
}
