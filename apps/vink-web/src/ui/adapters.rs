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
        "own_pending" => Some("Chờ duyệt · Chưa công khai".into()),
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

/// What this tab has read about tags (draft tags, the catalog list, suggestions): labels for the
/// IDs a selection or a conflict carries. A view cache only, never an authority: an unread ID
/// is shown as such, not guessed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagLabels(std::collections::BTreeMap<String, crate::contract::TagDto>);

impl TagLabels {
    pub fn learn<'a>(&mut self, tags: impl IntoIterator<Item = &'a crate::contract::TagDto>) {
        for tag in tags {
            self.0.insert(tag.tag_id.clone(), tag.clone());
        }
    }

    pub fn label(&self, id: &str) -> Option<&str> {
        self.0.get(id).map(|tag| tag.label.as_str())
    }

    /// The chip facts of a selected tag; `refusal` marks it when the server refused this tag.
    pub fn facts(
        &self,
        id: &str,
        refusal: Option<&crate::contract::TagRefusal>,
    ) -> super::patterns::TagFacts {
        use super::patterns::{TagFacts, TagLink};
        use crate::contract::TagRefusalCode;
        let refused = refusal.filter(|r| r.tag_id.as_deref() == Some(id));
        let mut facts = match self.0.get(id) {
            Some(tag) => TagFacts {
                id: tag.tag_id.clone(),
                label: tag.label.clone(),
                description: tag.description.clone(),
                retired: tag.status == "retired",
                successor: tag.successor.as_ref().map(|s| TagLink {
                    id: s.tag_id.clone(),
                    label: s.label.clone(),
                }),
                problem: None,
            },
            None => TagFacts::active(id, "Thẻ chưa tải được tên"),
        };
        if let Some(refusal) = refused {
            facts.problem = Some(tag_refusal_text(refusal));
            if refusal.code == TagRefusalCode::Retired {
                facts.retired = true;
                if let Some(successor) = refusal.successor_id.as_deref() {
                    facts.successor = Some(TagLink {
                        id: successor.to_string(),
                        label: self.label(successor).unwrap_or("thẻ kế nhiệm").to_string(),
                    });
                }
            }
        }
        facts
    }

    pub fn names(&self, selection: &crate::tag_state::Selection) -> String {
        let mut names: Vec<&str> = selection
            .form
            .iter()
            .chain(selection.themes.iter())
            .map(|id| self.label(id).unwrap_or("thẻ chưa tải được tên"))
            .collect();
        if names.is_empty() {
            names.push("không có thẻ");
        }
        names.join(", ")
    }
}

pub fn tag_selection(tags: &crate::contract::DraftTags) -> crate::tag_state::Selection {
    crate::tag_state::Selection::new(
        tags.form.as_ref().map(|f| f.tag_id.clone()),
        tags.themes.iter().map(|t| t.tag_id.clone()),
    )
}

/// Active catalog tags as choices; the list carries only active tags already.
pub fn tag_catalog(list: &crate::contract::TagList) -> super::patterns::CatalogFacts {
    let mut labels = TagLabels::default();
    labels.learn(list.tags.iter());
    let pick = |facet: &str| {
        list.tags
            .iter()
            .filter(|t| t.facet == facet && t.status == "active")
            .map(|t| labels.facts(&t.tag_id, None))
            .collect()
    };
    super::patterns::CatalogFacts::Ready {
        forms: pick("form"),
        themes: pick("theme"),
        truncated: list.truncated,
    }
}

pub fn tag_options(
    phase: &crate::tag_state::QueryPhase<crate::contract::TagSuggestion>,
) -> super::patterns::TagOptions {
    use super::patterns::{TagOption, TagOptions};
    use crate::tag_state::QueryPhase;
    match phase {
        QueryPhase::Idle => TagOptions::Idle,
        QueryPhase::Composing => TagOptions::Composing,
        QueryPhase::Loading => TagOptions::Loading,
        QueryPhase::Invalid => TagOptions::Invalid,
        QueryPhase::Unavailable => TagOptions::Unavailable,
        QueryPhase::Ready(found) => {
            let mut labels = TagLabels::default();
            labels.learn(found.iter().map(|s| &s.tag));
            TagOptions::Ready(
                found
                    .iter()
                    .map(|s| TagOption {
                        tag: labels.facts(&s.tag.tag_id, None),
                        via_alias: s.via_alias.clone(),
                    })
                    .collect(),
            )
        }
    }
}

/// The exact match of the current answer, offered to Enter (`match: exact`, same slug).
pub fn exact_option(
    phase: &crate::tag_state::QueryPhase<crate::contract::TagSuggestion>,
) -> Option<String> {
    match phase {
        crate::tag_state::QueryPhase::Ready(found) => found
            .iter()
            .find(|s| s.match_kind == "exact")
            .map(|s| s.tag.tag_id.clone()),
        _ => None,
    }
}

/// A refusal in words: what happened, that nothing was written, what to do.
pub fn tag_refusal_text(refusal: &crate::contract::TagRefusal) -> String {
    use crate::contract::TagRefusalCode;
    match refusal.code {
        TagRefusalCode::UnknownTag => {
            "Thẻ này không có trong danh mục. Chưa lưu thay đổi nào; bỏ thẻ này rồi lưu lại."
        }
        TagRefusalCode::Retired => {
            "Thẻ này vừa ngừng dùng nên không thể thêm. Chưa lưu thay đổi nào; bỏ hoặc thay thẻ này."
        }
        TagRefusalCode::WrongFacet => {
            "Thẻ này thuộc nhóm khác. Chưa lưu thay đổi nào; bỏ thẻ này rồi chọn lại."
        }
        TagRefusalCode::TooManyThemes => {
            "Mỗi tác phẩm có tối đa 3 chủ đề. Chưa lưu thay đổi nào; bỏ bớt chủ đề rồi lưu lại."
        }
    }
    .into()
}

#[cfg(test)]
mod tag_tests {
    use super::*;
    use crate::contract::{DraftTags, TagList, TagRefusal, TagRefusalCode, TagSuggestions};

    fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
        let path = format!(
            "{}/../../services/vink-app/contracts/http/v1/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn draft_tags_project_retired_tags_with_their_successor_and_never_guess_unread_ids() {
        let draft: DraftTags = fixture("draft.tags.get.json");
        let mut labels = TagLabels::default();
        labels.learn(draft.form.iter().chain(draft.themes.iter()));
        let selection = tag_selection(&draft);
        assert_eq!(selection.themes.len(), 2);
        let retired = labels.facts(&selection.themes[1], None);
        assert!(retired.retired);
        assert_eq!(retired.successor.unwrap().label, "Mùa");
        let unread = labels.facts("a0000000-0000-4000-8000-000000000099", None);
        assert_eq!(unread.label, "Thẻ chưa tải được tên");
        assert_eq!(labels.names(&selection), "Tản văn, Ký ức, Đô thị");
    }

    #[test]
    fn a_refusal_marks_only_the_refused_tag_with_actionable_words() {
        let list: TagList = fixture("tags.list.json");
        let mut labels = TagLabels::default();
        labels.learn(list.tags.iter());
        let refusal = TagRefusal {
            code: TagRefusalCode::Retired,
            field: Some("themes".into()),
            tag_id: Some("a0000000-0000-4000-8000-000000000011".into()),
            successor_id: Some("a0000000-0000-4000-8000-000000000002".into()),
        };
        let refused = labels.facts("a0000000-0000-4000-8000-000000000011", Some(&refusal));
        assert!(refused.retired);
        assert!(refused.problem.unwrap().contains("Chưa lưu thay đổi nào"));
        assert_eq!(refused.successor.unwrap().label, "Thơ");
        let other = labels.facts("a0000000-0000-4000-8000-000000000001", Some(&refusal));
        assert!(other.problem.is_none() && !other.retired);
    }

    #[test]
    fn catalog_and_suggestions_become_choices_with_alias_and_exact_match() {
        let list: TagList = fixture("tags.list.json");
        match tag_catalog(&list) {
            super::super::patterns::CatalogFacts::Ready {
                forms,
                themes,
                truncated,
            } => {
                assert_eq!(forms.len(), 2);
                assert_eq!(themes.len(), 1);
                assert!(!truncated);
            }
            other => panic!("{other:?}"),
        }
        let answer: TagSuggestions = fixture("tags.suggestions.json");
        let mut query = crate::tag_state::QueryState::default();
        let request = query.input("hồi", false).unwrap();
        assert!(query.settle(
            &request,
            Ok((answer.query.clone(), answer.suggestions.clone()))
        ));
        match tag_options(query.phase()) {
            super::super::patterns::TagOptions::Ready(found) => {
                assert_eq!(found[1].via_alias.as_deref(), Some("Hồi ức"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            exact_option(query.phase()),
            None,
            "prefix matches are not chosen by Enter"
        );
    }
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
                .contains("Chưa công khai")
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

/// Private server facts → DTO-free row. Unknown axes never become published/zero.
pub fn author_work_summary(
    work: &crate::contract::author_works::AuthorWorkCard,
) -> super::patterns::AuthorWorkSummary {
    use crate::contract::publication::{Audience, Decision, Lifecycle};
    let mut status = "Chưa xác định trạng thái".to_string();
    let mut metrics = None;
    let mut public_context = None;
    let mut reader_href = None;
    if let Some(p) = &work.publication {
        let known = p.audience != Audience::Unknown && p.lifecycle != Lifecycle::Unknown;
        if known {
            status = match p.lifecycle {
                Lifecycle::AuthorDeleted => "Đã xóa bởi tác giả".into(),
                Lifecycle::Removed => "Đã gỡ".into(),
                Lifecycle::Active => {
                    if p.current_public.is_some() && p.audience == Audience::Public {
                        if p.has_draft_changes == Some(true) {
                            "Đã đăng · Có bản sửa"
                        } else {
                            "Đã đăng"
                        }
                    } else if p.review_withdrawn {
                        "Đã ẩn sau kiểm duyệt"
                    } else if p.audience == Audience::Private {
                        if work.draft_version.is_some()
                            && p.generation == 1
                            && p.latest_revision_id.is_none()
                        {
                            "Bản nháp"
                        } else {
                            "Chỉ mình tôi"
                        }
                    } else {
                        "Chưa có bản công khai"
                    }
                    .into()
                }
                Lifecycle::Unknown => unreachable!(),
            };
            if p.lifecycle == Lifecycle::Active {
                let decision = if p.intent_revision_id.is_some() {
                    p.intent_moderation
                } else {
                    p.latest_moderation
                };
                let different = p.current_public.as_ref().is_none_or(|public| {
                    p.intent_revision_id
                        .as_ref()
                        .or(p.latest_revision_id.as_ref())
                        != Some(&public.revision_id)
                });
                if different {
                    let label = match decision {
                        Some(Decision::Pending) => Some("Bản mới đang chờ duyệt"),
                        Some(Decision::NeedsAdmin) => Some("Bản mới cần người duyệt"),
                        Some(Decision::Rejected) => Some("Bản mới không được duyệt"),
                        Some(Decision::Unknown) => Some("Chưa rõ trạng thái bản mới"),
                        _ => None,
                    };
                    if let Some(label) = label {
                        status.push_str(" · ");
                        status.push_str(label);
                    }
                }
                if let Some(public) = &p.current_public
                    && p.audience == Audience::Public
                {
                    let count = |n: Option<i64>| {
                        n.filter(|n| *n >= 0)
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "—".into())
                    };
                    metrics = Some(format!(
                        "Thấm {} · Nhận xét {}",
                        count(public.tham),
                        count(public.comments)
                    ));
                    public_context = Some(if public.title == work.title {
                        "Của bản đang công khai".into()
                    } else {
                        format!("Bản công khai: {}", public.title)
                    });
                    if public.tham.is_none_or(|n| n < 0) || public.comments.is_none_or(|n| n < 0) {
                        public_context
                            .as_mut()
                            .unwrap()
                            .push_str(" · Chưa tải được số liệu");
                    }
                    reader_href = Some(format!("/tac-pham/{}", work.work_id));
                }
            }
        }
    }
    if work.draft_version.is_none() {
        status.push_str(" · Không có bản nháp để sửa");
    }
    super::patterns::AuthorWorkSummary {
        title: crate::draft_list::display_title(&work.title),
        updated_at: work.updated_at.clone(),
        editor_href: work
            .draft_version
            .filter(|v| *v >= 1)
            .map(|_| format!("/ban-viet/{}", work.work_id)),
        status,
        metrics,
        public_context,
        reader_href,
    }
}
