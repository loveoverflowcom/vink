use super::patterns::{Author, DraftSummary};
use crate::contract::{DraftCard, Publication, WorkCard};

/// The current provider exposes a display name, not a stable author ID or profile URL.
/// Keep the avatar neutral rather than treating a name or the current account as that ID.
pub fn card_author(card: &WorkCard) -> Author {
    Author {
        name: card.author_display_name.clone(),
        key: None,
        published_at: card.published_at.clone(),
    }
}

pub fn publication_author(publication: &Publication) -> Author {
    Author {
        name: publication.author_display_name.clone(),
        key: None,
        published_at: publication.published_at.clone(),
    }
}

pub fn draft_summary(draft: &DraftCard) -> DraftSummary {
    DraftSummary {
        title: crate::draft_list::display_title(&draft.title),
        version: draft.version,
        updated_at: draft.updated_at.clone(),
        href: format!("/ban-viet/{}", draft.work_id),
    }
}

pub fn discussion_thread(
    root: &crate::contract::RootThread,
    replies: &[crate::contract::CommentDto],
) -> super::patterns::DiscussionThreadFacts {
    use crate::contract::RootView;
    super::patterns::DiscussionThreadFacts {
        root_id: root_id(root).to_owned(),
        root: match &root.root {
            RootView::Comment { comment } => discussion_comment(comment, replies),
            RootView::Anchor { .. } => None,
        },
        replies: replies
            .iter()
            .filter_map(|comment| discussion_comment(comment, replies))
            .collect(),
    }
}

pub fn root_id(root: &crate::contract::RootThread) -> &str {
    match &root.root {
        crate::contract::RootView::Comment { comment } => &comment.comment_id,
        crate::contract::RootView::Anchor { comment_id, .. } => comment_id,
    }
}

fn discussion_comment(
    comment: &crate::contract::CommentDto,
    loaded: &[crate::contract::CommentDto],
) -> Option<super::patterns::DiscussionComment> {
    use super::patterns::{DiscussionComment, DiscussionContext};
    use crate::contract::ReplyTo;
    // A future visibility value has no known privacy contract. Fail closed, not as public.
    let status = match comment.visibility.as_str() {
        "public" => None,
        "own_pending" => Some("Đang chờ duyệt · Chỉ bạn thấy nhận xét này".into()),
        "own_rejected" => Some(format!(
            "Không được duyệt · {}",
            comment
                .rejection_reason
                .as_deref()
                .unwrap_or("Chưa có lý do được cấp")
        )),
        _ => return None,
    };
    Some(DiscussionComment {
        id: comment.comment_id.clone(),
        author: Author {
            name: comment.author_display_name.clone(),
            key: None,
            published_at: comment.created_at.clone(),
        },
        body: comment.body.clone(),
        status,
        can_reply: comment.visibility == "public",
        can_remove: comment.can_remove,
        context: comment.reply_to.as_ref().map(|target| match target {
            ReplyTo::Root => DiscussionContext::Root,
            ReplyTo::Unavailable => DiscussionContext::Unavailable,
            ReplyTo::Reply {
                comment_id,
                author_display_name,
            } => {
                let href = loaded
                    .iter()
                    .any(|c| c.comment_id == *comment_id && c.visibility == "public")
                    .then(|| format!("#comment-{comment_id}"));
                DiscussionContext::Reply {
                    name: author_display_name.clone(),
                    href,
                }
            }
        }),
    })
}

#[cfg(test)]
mod discussion_tests {
    use super::*;
    #[test]
    fn d2t_03_projection_keeps_anchor_private_states_and_context_scope_distinct() {
        let page: crate::contract::DiscussionPage = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/discussion.get.json"
        ))
        .unwrap();
        let first = discussion_thread(&page.roots[0], &page.roots[0].preview);
        assert!(first.root.as_ref().unwrap().author.key.is_none());
        assert!(
            first.replies[2]
                .status
                .as_deref()
                .unwrap()
                .contains("Chỉ bạn thấy")
        );
        assert!(!first.replies[2].can_reply);
        assert!(matches!(
            first.replies[1].context,
            Some(super::super::patterns::DiscussionContext::Reply { href: Some(_), .. })
        ));
        let anchor = discussion_thread(&page.roots[1], &page.roots[1].preview);
        assert!(anchor.root.is_none());
        assert_eq!(anchor.replies.len(), 1);
        let mut unknown = page.roots[0].preview[0].clone();
        unknown.visibility = "future".into();
        assert!(discussion_comment(&unknown, &[]).is_none());
    }
}
