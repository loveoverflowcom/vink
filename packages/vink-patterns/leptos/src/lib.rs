//! Vink compositions: no fetch, route lookup, account resolution or write authority.
#![recursion_limit = "256"]

use leptos::prelude::*;
use vink_ui::*;
pub fn display_date(timestamp: &str) -> String {
    let date = timestamp.get(..10).unwrap_or(timestamp);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(d)) => format!("{d}/{m}/{y}"),
        _ => timestamp.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub key: Option<String>,
    pub published_at: String,
}

#[component]
pub fn WorkByline(
    author: Author,
    #[prop(default = AvatarSize::Feed)] size: AvatarSize,
) -> impl IntoView {
    view! {
        <div class="vk-work-byline">
            <AuthorByline name=author.name identity_key=author.key.unwrap_or_default() size />
            <time class="vk-work-byline__date" datetime=author.published_at.clone()>
                {display_date(&author.published_at)} " (UTC)"
            </time>
        </div>
    }
}

#[component]
pub fn WorkCard(
    title: String,
    author: Author,
    #[prop(into)] href: Signal<String>,
    #[prop(optional, into)] on_open: Option<Callback<leptos::ev::MouseEvent>>,
) -> impl IntoView {
    view! {
        <WorkByline author />
        <Link href variant=LinkVariant::Standalone attr:class="vk-public-row__link"
            on:click=move |event| { if let Some(open) = on_open { open.run(event); } }>
            <Heading level=HeadingLevel::H3 role=TypeRole::Title>{title}</Heading>
            <span class="vk-public-row__open">"Đọc tác phẩm →"</span>
        </Link>
    }
}

#[component]
pub fn ReaderHeader(title: String, author: Author) -> impl IntoView {
    view! {
        <header id="reader-header" class="vk-reader__header">
            <Text role=TypeRole::Caption tone=TextTone::Muted>"Bản công khai"</Text>
            <Heading level=HeadingLevel::H1 role=TypeRole::Display>{title}</Heading>
            <WorkByline author size=AvatarSize::Reader />
        </header>
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftSummary {
    pub title: String,
    pub version: i64,
    pub updated_at: String,
    pub href: String,
}

#[component]
pub fn DraftMetadata(version: i64, updated_at: String) -> impl IntoView {
    let label = updated_at_label(&updated_at);
    view! {
        <span class="vk-draft-meta">
            <span>"Phiên bản " {version}</span>
            <span>"Cập nhật " <time datetime=updated_at>{label}</time></span>
        </span>
    }
}

#[component]
pub fn DraftResume(draft: DraftSummary) -> impl IntoView {
    // Only presentation expands: the complete Unicode title and destination stay untouched.
    // Short titles wrap in full; long ones have a disclosure in the compact layout.
    // The container query also follows text enlargement without a viewport resize.
    let expandable = draft.title.chars().count() > 60;
    let expanded = RwSignal::new(false);
    view! {
        <div class="vk-draft-resume">
            <Surface tone=SurfaceTone::Primary radius=Radius::Lg padding=Space::S4>
                <div class="vk-draft-resume__content">
                    <Text role=TypeRole::Label id="resume-label">"Trở lại trang đang viết"</Text>
                    <Heading level=HeadingLevel::H2 role=TypeRole::Title class:vk-draft-resume__title=true>{draft.title}</Heading>
                    {expandable.then(|| view! {
                        <Button variant=ButtonVariant::Text class:vk-draft-resume__disclosure=true
                            id="resume-title-toggle" expanded
                            on_press=move |_| expanded.update(|value| *value = !*value)>
                            {move || if expanded.get() { "Thu gọn tiêu đề" } else { "Xem đầy đủ tiêu đề" }}
                        </Button>
                    })}
                    <DraftMetadata version=draft.version updated_at=draft.updated_at />
                </div>
                <Link variant=LinkVariant::Standalone class:vk-draft-resume__continue=true
                    attr:data-testid="resume-draft" href=draft.href>"Viết tiếp"</Link>
            </Surface>
        </div>
    }
}

/// One scannable row: a constant writing mark (not a genre or status), the complete title with
/// its version/date, then the open action. The whole row is the link; its name keeps all three.
#[component]
pub fn DraftRow(draft: DraftSummary) -> impl IntoView {
    view! {
        <Link href=draft.href attr:class="vk-draft-row__link" variant=LinkVariant::Standalone>
            <span class="vk-draft-row__mark"><Icon name=IconName::Edit /></span>
            <span class="vk-draft-row__text">
                <span class="vk-draft-row__title">{draft.title}</span>
                <DraftMetadata version=draft.version updated_at=draft.updated_at />
            </span>
            <span class="vk-draft-row__open">
                <span class="vk-draft-row__open-label">"Mở bản thảo"</span>
                <Icon name=IconName::ArrowBack />
            </span>
        </Link>
    }
}

/// A toolbar slot keeps the save authority in EditorSession and the body in CodeMirror.
#[component]
pub fn EditorToolbar(
    #[prop(into)] status: Signal<String>,
    #[prop(into)] status_token: Signal<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="vk-editor-toolbar">
            <p class="vk-save-status" role="status" aria-live="polite" data-testid="save-status" data-status=move || status_token.get()>
                {move || status.get()}
            </p>
            <div class="vk-editor-toolbar__actions">{children()}</div>
        </div>
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackIntent {
    Unspecified,
    Sharing,
    Impressions,
    Technique,
}
impl FeedbackIntent {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unspecified => "Nhận xét",
            Self::Sharing => "Chỉ chia sẻ",
            Self::Impressions => "Muốn nghe cảm nhận",
            Self::Technique => "Xin góp ý kỹ thuật",
        }
    }
}

/// Caller supplied public feedback only. No counts, timestamps or people are invented here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub id: String,
    pub person: Person,
    pub text: String,
    pub context: String,
    pub href: String,
}

#[component]
pub fn ParticipantSummary(items: Vec<Feedback>) -> impl IntoView {
    let mut people = Vec::new();
    for item in &items {
        if let Some(key) = &item.person.key
            && !key.is_empty()
            && !people.iter().any(|p: &Person| p.key.as_ref() == Some(key))
        {
            people.push(item.person.clone());
        }
    }
    view! {
        <Stack direction=Direction::Row gap=Space::S3 wrap=true>
            {(!people.is_empty()).then(|| view! { <AvatarGroup people label="Người góp ý" /> })}
            <Text role=TypeRole::Caption>{if items.is_empty() { "Mời một lời góp ý đầu tiên".into() } else { format!("{} nhận xét trong danh sách", items.len()) }}</Text>
        </Stack>
    }
}

#[component]
pub fn FeedbackItem(item: Feedback) -> impl IntoView {
    view! {
        <article class="vk-feedback-item">
            <AuthorByline name=item.person.name.unwrap_or_default() identity_key=item.person.key.unwrap_or_default() size=AvatarSize::Medium />
            <Text>{item.text}</Text>
            <Link href=item.href>{item.context}</Link>
        </article>
    }
}

#[component]
pub fn FeedbackThread(items: Vec<Feedback>) -> impl IntoView {
    let summary = items.clone();
    view! {
        <ParticipantSummary items=summary />
        <For each=move || items.clone() key=|item| item.id.clone() children=|item| view! { <FeedbackItem item /> } />
    }
}

/// Controlled input: a pending/error/reauth transition never replaces the caller's text.
/// The caller owns request identity and acknowledgment; no fake success lives here.
#[component]
pub fn FeedbackComposer(
    #[prop(into)] id: String,
    intent: FeedbackIntent,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<TextInput>,
    #[prop(into)] state: Signal<ActionState>,
    #[prop(into)] on_submit: Callback<()>,
    /// Author intent never decides the per-work comments policy.
    #[prop(optional, into)]
    comments_closed: Signal<bool>,
) -> impl IntoView {
    let submit_id = format!("{id}-send");
    let submit_state = Signal::derive(move || {
        if matches!(state.get(), ActionState::Pending { .. }) {
            state.get()
        } else if comments_closed.get() {
            ActionState::disabled("Tác giả đã tắt bình luận. Bản đang soạn được giữ lại.")
        } else {
            state.get()
        }
    });
    view! {
        <section class="vk-feedback-composer">
            <Text role=TypeRole::Label>{intent.label()}</Text>
            <TextArea id label=if intent == FeedbackIntent::Technique { "Góp ý kỹ thuật có ngữ cảnh" } else { "Nhận xét của bạn" }
                value on_input help="Tối đa 4.000 ký tự, 200 dòng. Enter xuống dòng; chỉ gửi khi bấm nút." />
            <Button id=submit_id state=submit_state on_press=on_submit>"Gửi nhận xét"</Button>
        </section>
    }
}

/// DTO-free, viewer-authorized projection. No identity key is invented from a name/ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscussionComment {
    pub id: String,
    pub author: Author,
    pub body: String,
    pub status: Option<String>,
    pub context: Option<DiscussionContext>,
    pub can_reply: bool,
    pub can_remove: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscussionContext {
    Root,
    Reply { name: String, href: Option<String> },
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscussionThreadFacts {
    pub root_id: String,
    pub root: Option<DiscussionComment>,
    pub replies: Vec<DiscussionComment>,
}

/// Exactly two levels. A contextual reply is still rendered in the same flat reply list.
#[component]
pub fn DiscussionThread(
    #[prop(into)] facts: Signal<DiscussionThreadFacts>,
    #[prop(into)] on_reply: Callback<String>,
    #[prop(into)] on_remove: Callback<String>,
    #[prop(optional, into)] action_state: Signal<ActionState>,
) -> impl IntoView {
    let root = Memo::new(move |_| facts.get().root);
    view! {
        <div class="vk-discussion-thread">
            {move || root.get().map(|comment| discussion_comment(comment, on_reply, on_remove, action_state))
                .unwrap_or_else(|| view! { <p class="vk-muted">"Bình luận gốc không còn hiển thị."</p> }.into_any())}
            <ul class="vk-discussion-replies" aria-label="Phản hồi theo thứ tự cũ đến mới">
                <For each=move || { facts.get().replies.into_iter().map(|comment| comment.id).collect::<Vec<_>>() } key=|id| id.clone()
                    children=move |id| {
                        let comment = Memo::new(move |_| facts.get().replies.into_iter().find(|comment| comment.id == id));
                        view! { <li>{move || comment.get().map(|comment| discussion_comment(comment, on_reply, on_remove, action_state))}</li> }
                    } />
            </ul>
        </div>
    }
}

fn discussion_comment(
    comment: DiscussionComment,
    on_reply: Callback<String>,
    on_remove: Callback<String>,
    state: Signal<ActionState>,
) -> AnyView {
    let reply_id = comment.id.clone();
    let remove_id = comment.id.clone();
    let reply_button_id = format!("reply-to-{}", comment.id);
    let remove_button_id = format!("remove-comment-{}", comment.id);
    view! {
        <article id=format!("comment-{}", comment.id) class="vk-feedback-item vk-discussion-comment" tabindex="-1">
            <WorkByline author=comment.author size=AvatarSize::Medium />
            {comment.context.map(|context| match context {
                DiscussionContext::Root => view! { <p class="vk-discussion-context">"Trả lời bình luận gốc"</p> }.into_any(),
                DiscussionContext::Unavailable => view! { <p class="vk-discussion-context">"Bình luận được trả lời không còn hiển thị"</p> }.into_any(),
                DiscussionContext::Reply { name, href: Some(href) } => view! { <Link href>{format!("Đang trả lời @{name}")}</Link> }.into_any(),
                DiscussionContext::Reply { name, href: None } => view! {
                    <p class="vk-discussion-context">{format!("Đang trả lời @{name}")}
                        <span>" · Ngoài trang đã tải; chưa có đường mở ngữ cảnh này."</span>
                    </p>
                }.into_any(),
            })}
            {comment.status.map(|status| view! { <p class="vk-discussion-status">{status}</p> })}
            <p class="vk-discussion-body">{comment.body}</p>
            <div class="vk-discussion-actions">
                {comment.can_reply.then(|| view! { <Button id=reply_button_id variant=ButtonVariant::Text on_press=move |_| on_reply.run(reply_id.clone())>"Trả lời"</Button> })}
                {comment.can_remove.then(|| view! { <Button id=remove_button_id variant=ButtonVariant::Text state on_press=move |_| on_remove.run(remove_id.clone())>"Gỡ nhận xét của tôi"</Button> })}
            </div>
        </article>
    }.into_any()
}

/// No authoritative recent timestamps in today's adapter: use an explicitly labelled sample.
#[component]
pub fn RecentFeedback(items: Vec<Feedback>) -> impl IntoView {
    view! {
        <section aria-label="Từ những cuộc trò chuyện — Mẫu">
            <Heading level=HeadingLevel::H2 role=TypeRole::Title>"Từ những cuộc trò chuyện"</Heading>
            <Text role=TypeRole::Caption tone=TextTone::Muted>"Mẫu · Không có dịch vụ nhận xét"</Text>
            <FeedbackThread items />
        </section>
    }
}

pub fn updated_at_label(timestamp: &str) -> String {
    match (timestamp.get(0..10), timestamp.get(11..16)) {
        (Some(date), Some(clock))
            if timestamp.get(10..11) == Some("T") && timestamp.ends_with('Z') =>
        {
            let parts: Vec<_> = date.split('-').collect();
            if let [year, month, day] = parts.as_slice() {
                return format!("{day}/{month}/{year}, {clock} UTC");
            }
            timestamp.to_string()
        }
        _ => timestamp.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_composition_escapes_user_text_and_never_invents_a_profile() {
        let html = Owner::new().with(|| view! {
            <WorkCard title="<script>private()</script>".into()
                author=Author { name: "<b>Tác giả</b>".into(), key: None, published_at: "2026-11-02T02:00:00Z".into() }
                href="/tac-pham/public?q=tho" />
        }.to_html());
        assert!(
            !html.contains("<script>") && !html.contains("<b>"),
            "{html}"
        );
        assert!(
            html.contains("&lt;script&gt;") && html.contains("02/11/2026"),
            "{html}"
        );
        assert_eq!(
            html.matches("href=").count(),
            1,
            "no invented profile link: {html}"
        );
        assert!(
            html.contains("data-tone=\"neutral\""),
            "no author ID: {html}"
        );
    }

    #[test]
    fn sharing_and_comments_mode_are_independent_and_keep_the_controlled_text() {
        let html = Owner::new().with(|| {
            view! {
                <FeedbackComposer id="feedback" intent=FeedbackIntent::Sharing value="giữ chữ"
                    on_input=|_| {} state=ActionState::Enabled on_submit=|_| {} />
            }
            .to_html()
        });
        assert!(
            html.contains("textarea") && html.contains("Gửi nhận xét"),
            "{html}"
        );
        assert!(html.contains("giữ chữ"), "{html}");
        let closed = Owner::new().with(|| {
            view! {
            <FeedbackComposer id="closed" intent=FeedbackIntent::Sharing value="giữ chữ"
                on_input=|_| {} state=ActionState::Enabled comments_closed=true on_submit=|_| {} />
        }.to_html()
        });
        assert!(
            closed.contains("textarea")
                && closed.contains("giữ chữ")
                && closed.contains("aria-disabled=\"true\"")
                && closed.contains("Tác giả đã tắt"),
            "{closed}"
        );
        let pending = Owner::new().with(|| view! {
            <FeedbackComposer id="pending-closed" intent=FeedbackIntent::Sharing value="giữ chữ"
                on_input=|_| {} state=ActionState::pending("Đang gửi nhận xét…") comments_closed=true on_submit=|_| {} />
        }.to_html());
        assert!(
            pending.contains("Đang gửi nhận xét…") && pending.contains("data-state=\"pending\""),
            "{pending}"
        );
    }
}
