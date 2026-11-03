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
    view! {
        <Surface tone=SurfaceTone::Primary radius=Radius::Lg padding=Space::S3>
            <Text role=TypeRole::Label id="resume-label">"Trở lại trang đang viết"</Text>
            <Heading level=HeadingLevel::H2 role=TypeRole::Headline>{draft.title}</Heading>
            <DraftMetadata version=draft.version updated_at=draft.updated_at />
            <Link variant=LinkVariant::Standalone attr:data-testid="resume-draft" href=draft.href>"Viết tiếp"</Link>
        </Surface>
    }
}

#[component]
pub fn DraftRow(draft: DraftSummary) -> impl IntoView {
    view! {
        <Link href=draft.href attr:class="vk-draft-row__link" variant=LinkVariant::Standalone>
            <span class="vk-draft-row__title">{draft.title}</span>
            <DraftMetadata version=draft.version updated_at=draft.updated_at />
            <span class="vk-draft-row__open">"Mở bản thảo →"</span>
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
            <Stack direction=Direction::Row gap=Space::S2 wrap=true>{children()}</Stack>
        </div>
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackIntent {
    Sharing,
    Impressions,
    Technique,
}
impl FeedbackIntent {
    pub const fn label(self) -> &'static str {
        match self {
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
) -> impl IntoView {
    view! {
        <section class="vk-feedback-composer">
            <Text role=TypeRole::Label>{intent.label()}</Text>
            {if intent == FeedbackIntent::Sharing {
                view! { <Text tone=TextTone::Muted>"Tác giả chỉ chia sẻ bài viết này."</Text> }.into_any()
            } else {
                view! {
                    <TextArea id label=if intent == FeedbackIntent::Technique { "Góp ý kỹ thuật có ngữ cảnh" } else { "Cảm nhận của bạn" }
                        value on_input help="Nêu chi tiết bạn đã đọc và điều bạn muốn trao đổi với tác giả." />
                    <Button state on_press=on_submit>"Gửi nhận xét"</Button>
                }.into_any()
            }}
        </section>
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
    fn sharing_has_no_criticism_form_or_send_action() {
        let html = Owner::new().with(|| {
            view! {
                <FeedbackComposer id="feedback" intent=FeedbackIntent::Sharing value="giữ chữ"
                    on_input=|_| {} state=ActionState::Enabled on_submit=|_| {} />
            }
            .to_html()
        });
        assert!(html.contains("Tác giả chỉ chia sẻ"), "{html}");
        assert!(
            !html.contains("textarea") && !html.contains("button"),
            "{html}"
        );
    }
}
