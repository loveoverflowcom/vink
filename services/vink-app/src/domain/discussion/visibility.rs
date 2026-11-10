//! What a viewer may see of a comment (D2T-INV-04/05). Pure projection of the lifecycle axes; the
//! PostgreSQL read filters must select exactly the comments these functions list, which the
//! adapter re-checks row by row and the PostgreSQL suite compares exhaustively.
//!
//! - Present + approved: public to everyone, with text and author.
//! - Present + pending/rejected: visible only to its own writer, as the status of their comment.
//!   The work's author gets nothing extra (COM-08: no moderation through work ownership).
//! - Removed: visible to nobody. A root that is not visible but still has public replies is shown
//!   as an anchor without text or identity, so the replies keep their place (no cascade).
//! - A reply's context that is no longer public is "unavailable", never "deleted by its author".

use super::lifecycle::{CommentState, Review};
use crate::domain::ids::{CommentId, PrincipalId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentView {
    Public,
    /// The viewer's own comment awaiting review.
    OwnPending,
    /// The viewer's own comment that review rejected.
    OwnRejected,
    Unavailable,
}

pub fn view(author: PrincipalId, state: CommentState, viewer: Option<PrincipalId>) -> CommentView {
    if state.is_public() {
        return CommentView::Public;
    }
    if !state.is_present() || viewer != Some(author) {
        return CommentView::Unavailable;
    }
    match state.review() {
        Review::Pending => CommentView::OwnPending,
        Review::Rejected => CommentView::OwnRejected,
        Review::Approved => CommentView::Public,
    }
}

/// How a root appears in the root list, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootListing {
    Shown(CommentView),
    /// Placeholder that keeps public replies attached; carries no text, author or reason.
    Anchor,
}

pub fn root_listing(view: CommentView, public_replies: i64) -> Option<RootListing> {
    match view {
        CommentView::Unavailable if public_replies > 0 => Some(RootListing::Anchor),
        CommentView::Unavailable => None,
        shown => Some(RootListing::Shown(shown)),
    }
}

/// Replies are listed only when the viewer may see them; hidden replies leave no placeholder.
pub fn reply_listed(view: CommentView) -> bool {
    view != CommentView::Unavailable
}

/// Context shown with a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyContext {
    /// Direct reply to its root.
    Root,
    /// Replies to this reply, which is public.
    Reply(CommentId),
    /// It replied to a reply that is hidden, removed or purged. Shows no text or identity.
    Unavailable,
}

/// `context` is the replied-to comment when it still exists; `replied_to_reply` is the immutable
/// fact recorded at creation, so a purged context is not mistaken for a direct reply.
pub fn reply_context(
    replied_to_reply: bool,
    context: Option<(CommentId, CommentState)>,
) -> ReplyContext {
    match (replied_to_reply, context) {
        (false, _) => ReplyContext::Root,
        (true, Some((id, state))) if state.is_public() => ReplyContext::Reply(id),
        (true, _) => ReplyContext::Unavailable,
    }
}

/// What a writer sees of one of their own comments in the private overlay (#120 DISC-02/03),
/// read apart from the public lists so a pending comment is recoverable wherever it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnOverlay {
    /// The writer's own text, only while the comment is present (removal drops or hides it).
    pub text: bool,
    /// The moderator's reason (MOD-04), only while the comment is present and rejected.
    pub reason: bool,
    /// Present and not public: pending or rejected. Exactly what the overlay list pages through;
    /// a public comment is read from the public lists, a removed one is gone for everyone.
    pub unpublished: bool,
}

/// The overlay of `viewer` includes a comment only when they wrote it. For anyone else, the work's
/// author and moderators included, there is nothing: not its state and not that it exists.
pub fn own_overlay(
    writer: PrincipalId,
    state: CommentState,
    viewer: PrincipalId,
) -> Option<OwnOverlay> {
    if writer != viewer {
        return None;
    }
    let present = state.is_present();
    Some(OwnOverlay {
        text: present,
        reason: present && state.review() == Review::Rejected,
        unpublished: present && !state.is_public(),
    })
}

/// A reply target must be public now: nobody replies to a pending, rejected or removed comment,
/// not even its own writer, so a reply never anchors to text others cannot read.
pub fn can_reply_to(target: CommentState) -> bool {
    target.is_public()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::discussion::lifecycle::{ModerationVersion, Removal};

    fn principal(id: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(id))
    }

    fn all_states() -> Vec<CommentState> {
        let mut states = Vec::new();
        for review in [Review::Pending, Review::Approved, Review::Rejected] {
            for removal in [None, Some(Removal::ByAuthor), Some(Removal::ByModeration)] {
                states.push(CommentState::restore(
                    review,
                    removal,
                    ModerationVersion::FIRST,
                ));
            }
        }
        states
    }

    #[test]
    fn only_approved_present_comments_reach_other_readers() {
        let writer = principal(1);
        let work_author = principal(2);
        for state in all_states() {
            for viewer in [None, Some(work_author), Some(principal(3))] {
                let seen = view(writer, state, viewer);
                assert_eq!(
                    seen == CommentView::Public,
                    state.is_public(),
                    "{state:?} for {viewer:?}"
                );
                assert!(matches!(
                    seen,
                    CommentView::Public | CommentView::Unavailable
                ));
            }
        }
    }

    #[test]
    fn writers_see_the_status_of_their_own_present_comments_only() {
        let writer = principal(1);
        for state in all_states() {
            let expected = match (state.removal(), state.review()) {
                (Some(_), _) => CommentView::Unavailable,
                (None, Review::Approved) => CommentView::Public,
                (None, Review::Pending) => CommentView::OwnPending,
                (None, Review::Rejected) => CommentView::OwnRejected,
            };
            assert_eq!(view(writer, state, Some(writer)), expected, "{state:?}");
        }
    }

    #[test]
    fn the_overlay_belongs_to_the_writer_alone() {
        let writer = principal(1);
        for state in all_states() {
            for other in [principal(2), principal(3)] {
                assert_eq!(own_overlay(writer, state, other), None, "{state:?}");
            }
            let own = own_overlay(writer, state, writer).expect("the writer's own comment");
            assert_eq!(own.text, state.is_present(), "{state:?}");
            assert_eq!(
                own.reason,
                state.is_present() && state.review() == Review::Rejected,
                "{state:?}"
            );
            // The list holds exactly what the public lists cannot show the writer.
            assert_eq!(
                own.unpublished,
                matches!(
                    view(writer, state, Some(writer)),
                    CommentView::OwnPending | CommentView::OwnRejected
                ),
                "{state:?}"
            );
            assert!(!(own.unpublished && view(writer, state, None) == CommentView::Public));
        }
    }

    #[test]
    fn hidden_roots_with_public_replies_become_anchors() {
        assert_eq!(
            root_listing(CommentView::Unavailable, 2),
            Some(RootListing::Anchor)
        );
        assert_eq!(root_listing(CommentView::Unavailable, 0), None);
        assert_eq!(
            root_listing(CommentView::OwnPending, 0),
            Some(RootListing::Shown(CommentView::OwnPending))
        );
        assert!(!reply_listed(CommentView::Unavailable));
        assert!(reply_listed(CommentView::OwnRejected));
    }

    #[test]
    fn context_distinguishes_direct_replies_from_hidden_or_purged_contexts() {
        let id = CommentId::from_uuid(Uuid::from_u128(9));
        assert_eq!(reply_context(false, None), ReplyContext::Root);
        assert_eq!(reply_context(true, None), ReplyContext::Unavailable);
        for state in all_states() {
            let expected = if state.is_public() {
                ReplyContext::Reply(id)
            } else {
                ReplyContext::Unavailable
            };
            assert_eq!(reply_context(true, Some((id, state))), expected);
            assert_eq!(can_reply_to(state), state.is_public());
        }
    }
}
