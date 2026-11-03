//! Who may change what (COM-07, COM-08). Owning a work grants exactly one discussion power:
//! opening or closing comments on that work. It grants no edit, removal or review of anyone
//! else's comment; those belong to the comment's writer (removal only) or to the system's
//! moderation authority, which the application obtains from its own port.

use super::visibility::CommentView;
use crate::domain::ids::PrincipalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Denied {
    /// Only the work's author opens or closes its comments.
    #[error("only the work's author can change its comments mode")]
    NotWorkAuthor,
    /// The comment is visible to the actor but written by someone else.
    #[error("only the writer can remove this comment")]
    NotCommentWriter,
    /// The actor may not even learn that the comment exists.
    #[error("comment not found")]
    Hidden,
}

pub fn authorize_mode_change(actor: PrincipalId, work_author: PrincipalId) -> Result<(), Denied> {
    if actor == work_author {
        Ok(())
    } else {
        Err(Denied::NotWorkAuthor)
    }
}

/// Removal by its writer. `actor_view` is what the actor may see of the comment: a stranger's
/// hidden comment answers `Hidden` so the attempt does not reveal that it exists.
pub fn authorize_writer_removal(
    actor: PrincipalId,
    writer: PrincipalId,
    actor_view: CommentView,
) -> Result<(), Denied> {
    if actor == writer {
        Ok(())
    } else if actor_view == CommentView::Public {
        Err(Denied::NotCommentWriter)
    } else {
        Err(Denied::Hidden)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn principal(id: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(id))
    }

    #[test]
    fn only_the_work_author_changes_the_comments_mode() {
        assert_eq!(authorize_mode_change(principal(1), principal(1)), Ok(()));
        assert_eq!(
            authorize_mode_change(principal(2), principal(1)),
            Err(Denied::NotWorkAuthor)
        );
    }

    #[test]
    fn the_work_author_cannot_remove_someone_elses_comment() {
        let work_author = principal(1);
        let writer = principal(2);
        assert_eq!(
            authorize_writer_removal(work_author, writer, CommentView::Public),
            Err(Denied::NotCommentWriter)
        );
        for hidden in [
            CommentView::Unavailable,
            CommentView::OwnPending,
            CommentView::OwnRejected,
        ] {
            assert_eq!(
                authorize_writer_removal(work_author, writer, hidden),
                Err(Denied::Hidden)
            );
        }
        assert_eq!(
            authorize_writer_removal(writer, writer, CommentView::OwnPending),
            Ok(())
        );
    }
}
