//! Vink compositions: no fetch, route lookup, account resolution or write authority.
#![recursion_limit = "256"]

use leptos::prelude::*;
use vink_ui::*;

mod tags;
pub use tags::{
    CatalogFacts, THEME_LIMIT, TagChip, TagFacts, TagLink, TagOption, TagOptions, TagSelector,
    options_status,
};
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
    #[prop(optional)] tags: Vec<String>,
    #[prop(optional, into)] on_open: Option<Callback<leptos::ev::MouseEvent>>,
) -> impl IntoView {
    view! {
        <Link href variant=LinkVariant::Standalone attr:class="vk-public-row__link"
            on:click=move |event| { if let Some(open) = on_open { open.run(event); } }>
            <Heading level=HeadingLevel::H3 role=TypeRole::Title>{title}</Heading>
            <WorkByline author />
            {(!tags.is_empty()).then(|| view! { <p class="vk-public-row-tags">{tags.join(" · ")}</p> })}
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
            <span><span aria-hidden="true">"· "</span><span class="vk-ui-visually-hidden">"Cập nhật "</span><time datetime=updated_at>{label}</time></span>
        </span>
    }
}

#[component]
pub fn DraftResume(draft: DraftSummary) -> impl IntoView {
    // Disclosure only changes presentation; the full Unicode title and destination stay intact.
    let expandable = draft.title.chars().count() > 60;
    let expanded = RwSignal::new(false);
    view! {
        <div class="vk-draft-resume">
            <Surface tone=SurfaceTone::Container radius=Radius::Xl padding=Space::S4>
                <div class="vk-draft-resume__content">
                    <Text role=TypeRole::Label id="resume-label">"Bản thảo gần nhất"</Text>
                    <Heading level=HeadingLevel::H2 role=TypeRole::Title class:vk-draft-resume__title=true>{draft.title}</Heading>
                    <DraftMetadata version=draft.version updated_at=draft.updated_at />
                </div>
                <div class="vk-draft-resume__actions">
                    <Link variant=LinkVariant::Standalone class:vk-draft-resume__continue=true
                        attr:data-testid="resume-draft" href=draft.href>"Viết tiếp"</Link>
                    {expandable.then(|| view! {
                        <Button variant=ButtonVariant::Text class:vk-draft-resume__disclosure=true
                            id="resume-title-toggle" expanded
                            on_press=move |_| expanded.update(|value| *value = !*value)>
                            {move || if expanded.get() { "Thu gọn tên" } else { "Xem đủ tên" }}
                            <span class="vk-ui-visually-hidden">" bản thảo"</span>
                        </Button>
                    })}
                </div>
            </Surface>
        </div>
    }
}

/// One flat navigation target with the complete title and its version/update date.
#[component]
pub fn DraftRow(draft: DraftSummary) -> impl IntoView {
    view! {
        <Link href=draft.href attr:class="vk-draft-row__link" variant=LinkVariant::Standalone>
            <span class="vk-draft-row__text">
                <span class="vk-draft-row__title">{draft.title}</span>
                <DraftMetadata version=draft.version updated_at=draft.updated_at />
            </span>
            <span class="vk-draft-row__open" aria-hidden="true">
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
    #[prop(optional, into)] status_detail: Option<Signal<String>>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="vk-editor-toolbar">
            <div class="vk-editor-toolbar__status">
                <p class="vk-save-status" role="status" aria-live="polite" aria-atomic="true" data-testid="save-status" data-status=move || status_token.get()>
                    {move || status.get()}
                </p>
                {status_detail.map(|detail| view! {
                    <SupportingDetails id="editor-save-details" summary="Chi tiết trạng thái lưu" compact=true>
                        <p>{move || detail.get()}</p>
                    </SupportingDetails>
                })}
            </div>
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
    /// `Unspecified` has no label of its own: the host heading already names the form, and the
    /// field label ("Nhận xét của bạn") would otherwise repeat the same word a third time.
    pub const fn label(self) -> Option<&'static str> {
        match self {
            Self::Unspecified => None,
            Self::Sharing => Some("Chỉ chia sẻ"),
            Self::Impressions => Some("Muốn nghe cảm nhận"),
            Self::Technique => Some("Xin góp ý kỹ thuật"),
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
    /// Reader-only tonal recipe; other feedback consumers retain their current form.
    #[prop(optional)]
    tonal: bool,
    #[prop(optional)] children: Option<Children>,
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
        <section class="vk-feedback-composer" class:vk-feedback-composer--tonal=tonal data-filled=move || (!value.get().is_empty()).then_some("true")>
            {children.map(|header| header())}
            {intent.label().map(|label| view! { <Text role=TypeRole::Label>{label}</Text> })}
            <TextArea id label=if intent == FeedbackIntent::Technique { "Góp ý kỹ thuật có ngữ cảnh" } else { "Nhận xét của bạn" }
                value on_input variant=if tonal { FieldVariant::FloatingTonal } else { FieldVariant::ExternalLabel }
                rows=if tonal { 1 } else { 4 }
                help=if tonal { "Tối đa 4.000 ký tự, 200 dòng. Enter xuống dòng." } else { "Tối đa 4.000 ký tự, 200 dòng. Enter xuống dòng; chỉ gửi khi bấm nút." } />
            <Show when=move || tonal && !value.get().is_empty()><p class="vk-feedback-draft-note">"Rời trang có thể mất nháp."</p></Show>
            {if tonal {
                view! { <Button id=submit_id state=submit_state on_press=on_submit leading=IconName::ArrowBack>"Gửi"</Button> }.into_any()
            } else {
                view! { <Button id=submit_id state=submit_state on_press=on_submit>"Gửi nhận xét"</Button> }.into_any()
            }}
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

/// Literary endorsement (LVE #73) of each comment in a thread: the root and every flat reply
/// are separate targets, never summed into each other or the work. `facts(id)` is `None` when
/// that comment has no endorsement target (a hidden root, the viewer's pending or rejected
/// comment), and then nothing renders. The adapter owns command keys, order and sessions.
#[derive(Clone, Copy)]
pub struct CommentEndorsements {
    pub facts: Callback<String, Option<EndorsementFacts>>,
    /// `(comment_id, desired)`.
    pub on_set: Callback<(String, bool)>,
    pub on_recover: Callback<String>,
}

/// Exactly two levels. A contextual reply is still rendered in the same flat reply list.
/// Endorsements never reorder roots or replies: the provider's order is kept as given.
#[component]
pub fn DiscussionThread(
    #[prop(into)] facts: Signal<DiscussionThreadFacts>,
    #[prop(into)] on_reply: Callback<String>,
    #[prop(into)] on_remove: Callback<String>,
    #[prop(optional, into)] action_state: Signal<ActionState>,
    #[prop(optional_no_strip)] endorsements: Option<CommentEndorsements>,
    #[prop(optional, into)] has_more_replies: Signal<bool>,
    #[prop(optional)] reply_footer: Option<ChildrenFn>,
) -> impl IntoView {
    let root = Memo::new(move |_| facts.get().root);
    let shared_block = Memo::new(move |_| {
        let endorsements = endorsements?;
        let thread = facts.get();
        uniform_endorsement_block(
            thread
                .root
                .iter()
                .chain(thread.replies.iter())
                .filter(|comment| comment.status.is_none())
                .map(|comment| endorsements.facts.run(comment.id.clone())),
        )
    });
    let reason_id = format!(
        "thread-{}-endorsement-reason",
        facts.get_untracked().root_id
    );
    let shared_description = {
        let reason_id = reason_id.clone();
        Signal::derive(move || shared_block.get().map(|_| reason_id.clone()))
    };
    view! {
        <div class="vk-discussion-thread">
            {move || root.get().map(|comment| discussion_comment(comment, on_reply, on_remove, action_state, endorsements, shared_description))
                .unwrap_or_else(|| view! { <p class="vk-muted">"Bình luận gốc không còn hiển thị."</p> }.into_any())}
            <div class="vk-discussion-reply-group" hidden=move || facts.get().replies.is_empty() && !has_more_replies.get()>
            <p class="vk-discussion-reply-heading">"Phản hồi"</p>
            <ul class="vk-discussion-replies" aria-label="Phản hồi theo thứ tự cũ đến mới">
                <For each=move || { facts.get().replies.into_iter().map(|comment| comment.id).collect::<Vec<_>>() } key=|id| id.clone()
                    children=move |id| {
                        let comment = Memo::new(move |_| facts.get().replies.into_iter().find(|comment| comment.id == id));
                        view! { <li>{move || comment.get().map(|comment| discussion_comment(comment, on_reply, on_remove, action_state, endorsements, shared_description))}</li> }
                    } />
            </ul>
            {reply_footer.map(|footer| footer())}
            </div>
            <p class="vk-discussion-shared-reason" id=reason_id hidden=move || shared_block.get().is_none()>{move || shared_block.get().map(|block| EndorsementKind::Comment.reason(block))}</p>
        </div>
    }
}

/// One comment's endorsement. It mounts once per presence change, so state updates keep the
/// same button (and its focus) instead of replacing it.
fn comment_endorsement(
    id: String,
    endorsements: CommentEndorsements,
    shared_description: Signal<Option<String>>,
) -> impl IntoView {
    let facts = {
        let id = id.clone();
        Memo::new(move |_| endorsements.facts.run(id.clone()))
    };
    let present = Memo::new(move |_| facts.with(Option::is_some));
    move || {
        present.get().then(|| {
            let (set_id, recover_id) = (id.clone(), id.clone());
            view! {
                <EndorsementAction
                    id=format!("endorse-comment-{id}")
                    kind=EndorsementKind::Comment
                    blocked_description=shared_description
                    facts=Signal::derive(move || facts.get().unwrap_or_default())
                    on_set=move |desired: bool| endorsements.on_set.run((set_id.clone(), desired))
                    on_recover=move |_: RecoveryIntent| endorsements.on_recover.run(recover_id.clone())
                />
            }
        })
    }
}

fn discussion_comment(
    comment: DiscussionComment,
    on_reply: Callback<String>,
    on_remove: Callback<String>,
    state: Signal<ActionState>,
    endorsements: Option<CommentEndorsements>,
    shared_description: Signal<Option<String>>,
) -> AnyView {
    let reply_id = comment.id.clone();
    let remove_id = comment.id.clone();
    let reply_button_id = format!("reply-to-{}", comment.id);
    let remove_button_id = format!("remove-comment-{}", comment.id);
    view! {
        <article id=format!("comment-{}", comment.id) class="vk-feedback-item vk-discussion-comment" tabindex="-1">
            <div class="vk-discussion-byline">
                <div><p class="vk-discussion-name">{comment.author.name.clone()}</p>
                    <time datetime=comment.author.published_at.clone()>{display_date(&comment.author.published_at)} " (UTC)"</time>
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
                </div>
                <Avatar name=comment.author.name identity_key=comment.author.key.unwrap_or_default() size=AvatarSize::Medium decorative=true />
            </div>
            {comment.status.map(|status| view! { <p class="vk-discussion-status">{status}</p> })}
            <p class="vk-discussion-body">{comment.body}</p>
            <div class="vk-discussion-actions">
                {comment.can_reply.then(|| view! { <Button id=reply_button_id variant=ButtonVariant::Text on_press=move |_| on_reply.run(reply_id.clone())>"Trả lời"</Button> })}
                {comment.can_remove.then(|| view! { <Button id=remove_button_id variant=ButtonVariant::Text state on_press=move |_| on_remove.run(remove_id.clone())>"Gỡ nhận xét của tôi"</Button> })}
                // Last: its note (pending, reason) wraps under it without pushing the text actions apart.
                {endorsements.map(|endorsements| comment_endorsement(comment.id.clone(), endorsements, shared_description))}
            </div>
        </article>
    }.into_any()
}

/// What a literary endorsement (LVE #73) is about. Copy only: one positive signal per viewer and
/// target, decided by the App's `domain::endorsement` through its adapter, never here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EndorsementKind {
    /// One published revision of a work: “Đánh giá cao”.
    Work,
    /// A root comment or a flat reply, each its own target: “Hữu ích”.
    Comment,
}

/// Why the viewer cannot endorse here, as the adapter learned it (`blocked_reason`, `401`,
/// `404`); advisory only, every write decides again at its durable boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EndorsementBlock {
    OwnContent,
    NotPermitted,
    SignedOut,
    Unavailable,
}

impl EndorsementKind {
    pub fn copy(self) -> ToggleCopy {
        let (label, selected, unit, on, off, failed, unknown) = match self {
            Self::Work => (
                "Đánh giá cao",
                "Bỏ đánh giá cao",
                "lượt đánh giá cao",
                "Đang gửi đánh giá cao…",
                "Đang bỏ đánh giá cao…",
                "Không thể đánh giá lúc này.",
                "Chưa rõ bạn đã đánh giá cao chưa.",
            ),
            Self::Comment => (
                "Hữu ích",
                "Bỏ đánh dấu hữu ích",
                "lượt hữu ích",
                "Đang gửi đánh dấu hữu ích…",
                "Đang bỏ đánh dấu hữu ích…",
                "Không thể đánh dấu lúc này.",
                "Chưa rõ bạn đã đánh dấu hữu ích chưa.",
            ),
        };
        ToggleCopy {
            count_unit: unit.into(),
            pending_on: on.into(),
            pending_off: off.into(),
            failed: failed.into(),
            unknown: unknown.into(),
            ..ToggleCopy::new(label, selected)
        }
    }

    /// Visible text, never hover-only; no login button is implied (no auth authority yet).
    pub const fn reason(self, block: EndorsementBlock) -> &'static str {
        match (self, block) {
            (Self::Work, EndorsementBlock::OwnContent) => {
                "Bạn không thể đánh giá tác phẩm của chính mình."
            }
            (Self::Comment, EndorsementBlock::OwnContent) => {
                "Bạn không thể đánh dấu nhận xét của chính mình."
            }
            (Self::Work, EndorsementBlock::NotPermitted) => "Tài khoản này chưa thể đánh giá.",
            (Self::Comment, EndorsementBlock::NotPermitted) => {
                "Tài khoản này chưa thể đánh dấu hữu ích."
            }
            (Self::Work, EndorsementBlock::SignedOut) => "Cần đăng nhập để đánh giá.",
            (Self::Comment, EndorsementBlock::SignedOut) => "Cần đăng nhập để đánh dấu hữu ích.",
            (Self::Work, EndorsementBlock::Unavailable) => "Không thể đánh giá lúc này.",
            (Self::Comment, EndorsementBlock::Unavailable) => "Tạm chưa thể đánh dấu hữu ích.",
        }
    }
}

/// View facts an adapter projects from summary/own-state reads and its own command reducer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EndorsementFacts {
    /// Confirmed own state; `Unknown` for a guest, an unread or unavailable own state.
    pub value: ToggleValue,
    pub phase: TogglePhase,
    /// Public count of this target only; never summed into a root or work.
    pub count: CountFact,
    pub block: Option<EndorsementBlock>,
}

/// Only complete, uniform idle facts can share a branch reason. None is an unread
/// public target, so it vetoes grouping rather than being mistaken for zero or no target.
fn uniform_endorsement_block(
    facts: impl IntoIterator<Item = Option<EndorsementFacts>>,
) -> Option<EndorsementBlock> {
    let mut count = 0;
    let mut shared = None;
    for facts in facts {
        let facts = facts?;
        if facts.phase != TogglePhase::Idle {
            return None;
        }
        let block = facts.block?;
        if !matches!(
            block,
            EndorsementBlock::SignedOut | EndorsementBlock::Unavailable
        ) {
            return None;
        }
        if shared.is_some_and(|previous| previous != block) {
            return None;
        }
        shared = Some(block);
        count += 1;
    }
    (count >= 2).then_some(shared).flatten()
}

/// The endorsement control for one target. A press emits the desired selection; the adapter
/// owns `command_key`, `expected_order`, session fencing and late answers. CHOICE-07 A uses
/// the same decorative Pen Flight R2 in every phase; confirmation keeps its own trailing slot.
#[component]
pub fn EndorsementAction(
    #[prop(into)] id: String,
    kind: EndorsementKind,
    #[prop(into)] facts: Signal<EndorsementFacts>,
    #[prop(optional, into)] blocked_description: Signal<Option<String>>,
    #[prop(into)] on_set: Callback<bool>,
    #[prop(into)] on_recover: Callback<RecoveryIntent>,
) -> impl IntoView {
    let blocked = Signal::derive(move || facts.get().block.map(|b| kind.reason(b).to_string()));
    let kind_id = match kind {
        EndorsementKind::Work => "work",
        EndorsementKind::Comment => "comment",
    };
    view! {
        <span class="vk-endorsement" data-kind=kind_id>
            <ToggleAction
                id
                copy=kind.copy()
                icon=IconName::PenFlight
                value=Signal::derive(move || facts.get().value)
                phase=Signal::derive(move || facts.get().phase)
                count=Signal::derive(move || facts.get().count)
                blocked
                blocked_description
                on_set
                on_recover
            />
        </span>
    }
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
    fn shared_reason_requires_complete_uniform_idle_targets() {
        let guest = EndorsementFacts {
            block: Some(EndorsementBlock::SignedOut),
            count: CountFact::Known(3),
            ..Default::default()
        };
        assert_eq!(
            uniform_endorsement_block([Some(guest), Some(guest)]),
            guest.block
        );
        for other in [
            None,
            Some(EndorsementFacts {
                block: Some(EndorsementBlock::Unavailable),
                ..guest
            }),
            Some(EndorsementFacts {
                phase: TogglePhase::Pending { desired: true },
                ..guest
            }),
            Some(EndorsementFacts {
                phase: TogglePhase::Uncertain { desired: false },
                ..guest
            }),
            Some(EndorsementFacts {
                phase: TogglePhase::Failed,
                ..guest
            }),
            Some(EndorsementFacts {
                block: Some(EndorsementBlock::OwnContent),
                ..guest
            }),
            Some(EndorsementFacts {
                block: None,
                ..guest
            }),
        ] {
            assert_eq!(
                uniform_endorsement_block([Some(guest), other]),
                None,
                "{other:?}"
            );
        }
        assert_eq!(uniform_endorsement_block([Some(guest)]), None);
        assert_eq!(uniform_endorsement_block([]), None);
        let html = Owner::new().with(|| {
            view! {
                <EndorsementAction id="shared" kind=EndorsementKind::Comment facts=guest
                    blocked_description=Some("branch-reason".to_string())
                    on_set=|_: bool| {} on_recover=|_: RecoveryIntent| {} />
            }
            .to_html()
        });
        assert!(
            html.contains("aria-describedby=\"shared-count branch-reason\""),
            "{html}"
        );
        assert!(
            html.contains("aria-disabled=\"true\"") && html.contains(">3<"),
            "{html}"
        );
        assert!(
            !html.contains("Cần đăng nhập"),
            "the branch owns the visible note: {html}"
        );
    }

    #[test]
    fn endorsement_copy_names_its_target_and_keeps_counts_apart() {
        let render = |kind, facts: EndorsementFacts| {
            Owner::new().with(|| {
                view! {
                    <EndorsementAction id="e" kind facts on_set=|_: bool| {} on_recover=|_: RecoveryIntent| {} />
                }
                .to_html()
            })
        };
        let work = render(
            EndorsementKind::Work,
            EndorsementFacts {
                value: ToggleValue::On,
                count: CountFact::Known(12),
                ..Default::default()
            },
        );
        assert!(
            work.contains("aria-label=\"Bỏ đánh giá cao\"") && work.contains(">Đánh giá cao<"),
            "{work}"
        );
        assert!(
            work.contains("lượt đánh giá cao") && work.contains("data-kind=\"work\""),
            "{work}"
        );
        let comment = render(
            EndorsementKind::Comment,
            EndorsementFacts {
                value: ToggleValue::Off,
                block: Some(EndorsementBlock::OwnContent),
                ..Default::default()
            },
        );
        assert!(
            comment.contains("aria-label=\"Hữu ích\"")
                && comment.contains("nhận xét của chính mình"),
            "{comment}"
        );
        assert!(!comment.contains("e-count"), "hidden count: {comment}");
        let guest = render(
            EndorsementKind::Work,
            EndorsementFacts {
                block: Some(EndorsementBlock::SignedOut),
                ..Default::default()
            },
        );
        assert!(
            guest.contains("Cần đăng nhập để đánh giá.") && !guest.contains("<a "),
            "no fake login: {guest}"
        );
        for kind in [EndorsementKind::Work, EndorsementKind::Comment] {
            let copy = kind.copy();
            let label = copy.label.to_lowercase();
            assert!(
                copy.selected_name.to_lowercase().contains(&label),
                "label in name: {copy:?}"
            );
        }
    }

    #[test]
    fn an_unspecified_composer_adds_no_intent_label_while_named_intents_keep_theirs() {
        let render = |intent| {
            Owner::new().with(|| {
                view! {
                    <FeedbackComposer id="composer" intent value="" on_input=|_| {}
                        state=ActionState::Enabled on_submit=|_| {} />
                }
                .to_html()
            })
        };
        let plain = render(FeedbackIntent::Unspecified);
        assert!(
            !plain.contains(">Nhận xét<") && plain.contains("Nhận xét của bạn"),
            "{plain}"
        );
        assert!(
            !plain.contains("vk-ui-text"),
            "no empty label element: {plain}"
        );
        let named = render(FeedbackIntent::Sharing);
        assert!(named.contains("Chỉ chia sẻ"), "{named}");
        assert!(
            named.contains("vk-ui-text"),
            "oracle: a label renders a text element: {named}"
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

    #[test]
    fn thread_endorsements_render_one_action_per_target_in_provider_order() {
        let comment = |id: &str| DiscussionComment {
            id: id.into(),
            author: Author {
                name: "Người đọc".into(),
                key: None,
                published_at: "2026-11-06T02:00:00Z".into(),
            },
            body: format!("Bình luận {id}"),
            status: None,
            context: None,
            can_reply: true,
            can_remove: false,
        };
        let html = Owner::new().with(|| {
            let facts = DiscussionThreadFacts {
                root_id: "root".into(),
                root: Some(comment("root")),
                replies: vec![comment("z-late"), comment("a-early"), comment("pending")],
            };
            let endorsements = CommentEndorsements {
                facts: Callback::new(|id: String| {
                    (id != "pending").then(|| EndorsementFacts {
                        value: ToggleValue::Off,
                        phase: TogglePhase::Idle,
                        count: CountFact::Known(if id == "z-late" { 7 } else { 0 }),
                        block: None,
                    })
                }),
                on_set: Callback::new(|_: (String, bool)| {}),
                on_recover: Callback::new(|_: String| {}),
            };
            view! {
                <DiscussionThread facts=Signal::stored(facts) on_reply=|_: String| {}
                    on_remove=|_: String| {} endorsements=Some(endorsements) />
            }
            .to_html()
        });
        let ids: Vec<usize> = [
            "endorse-comment-root\"",
            "endorse-comment-z-late\"",
            "endorse-comment-a-early\"",
        ]
        .iter()
        .map(|id| html.find(id).unwrap_or_else(|| panic!("{id}: {html}")))
        .collect();
        assert!(
            ids.windows(2).all(|w| w[0] < w[1]),
            "provider order kept: {html}"
        );
        assert!(
            !html.contains("endorse-comment-pending"),
            "no target, no action: {html}"
        );
        assert_eq!(html.matches("vk-ui-toggle__label").count(), 3, "{html}");
    }
}
