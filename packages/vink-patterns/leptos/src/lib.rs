//! Vink compositions: no fetch, route lookup, account resolution or write authority.
#![recursion_limit = "256"]

use leptos::prelude::*;
use vink_ui::*;

mod notification;
pub use notification::{
    NotificationGroup, NotificationLeading, NotificationRow, NotificationRowFacts,
};

mod timestamp;
use timestamp::Timestamp;
pub use timestamp::{first_public_date_hcm, updated_at_label, work_view_date_hcm};

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
    pub profile: Option<ProfileLink>,
    /// `name` is a terminal or placeholder label from the adapter ("Tài khoản đã xóa", a cache
    /// not yet verified), not a pen name: it gets the neutral mark, never initials or a handle.
    pub neutral: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileLink {
    pub href: String,
    pub handle: String,
}

/// The accessible name of a profile link: two people may share a pen name, the handle may not.
pub fn profile_label(name: &str, handle: &str) -> String {
    format!("Mở hồ sơ {name} (@{handle})")
}

/// Inline reference (reply context): natural reading, handle in the accessible name only.
#[component]
fn ProfileName(
    name: String,
    #[prop(optional_no_strip)] profile: Option<ProfileLink>,
    id: String,
) -> impl IntoView {
    match profile {
        Some(profile) => {
            let label = profile_label(&name, &profile.handle);
            view! {
                <Link href=profile.href attr:id=id attr:data-profile-return="true" attr:aria-label=label>
                    {name}
                </Link>
            }.into_any()
        }
        None => view! { <span>{name}</span> }.into_any(),
    }
}

/// One author cluster for card, ReaderHeader, root and reply (#352): avatar → pen name →
/// @handle, one link only for a verified profile. `name_view` may render the name (highlight).
#[component]
pub fn AuthorIdentity(
    author: Author,
    #[prop(optional_no_strip)] id: Option<String>,
    #[prop(default = AvatarSize::Feed)] size: AvatarSize,
    #[prop(optional_no_strip)] name_view: Option<AnyView>,
    #[prop(optional)] highlight_handle: bool,
) -> impl IntoView {
    let profile = author.profile.filter(|_| !author.neutral);
    let label = profile
        .as_ref()
        .map(|p| profile_label(&author.name, &p.handle));
    let (href, handle) = match profile {
        Some(p) => (p.href, Some(p.handle)),
        None => (String::new(), None),
    };
    view! {
        <AuthorByline name=author.name identity_key=author.key.unwrap_or_default() size
            neutral=author.neutral href handle link_id=id link_label=label name_view highlight_handle />
    }
}

#[component]
pub fn WorkByline(
    author: Author,
    #[prop(optional_no_strip)] profile_id: Option<String>,
    #[prop(default = AvatarSize::Feed)] size: AvatarSize,
) -> impl IntoView {
    let published_at = author.published_at.clone();
    view! {
        <div class="vk-work-byline grid [row-gap:min(var(--vk-space-1),4px)] min-w-0 wrap-anywhere" data-size=size.id()>
            <AuthorIdentity author id=profile_id size />
            // Outside the profile link: time is metadata of the work, not of the person.
            <span class="vk-work-byline__date"><Timestamp instant=published_at /></span>
        </div>
    }
}

#[component]
pub fn WorkCard(
    title: String,
    author: Author,
    #[prop(into)] href: Signal<String>,
    #[prop(optional)] tags: Vec<String>,
    #[prop(optional_no_strip)] profile_id: Option<String>,
    #[prop(optional, into)] on_open: Option<Callback<leptos::ev::MouseEvent>>,
) -> impl IntoView {
    view! {
        <div class="vk-work-card p-vk-4 rounded-vk-lg [background:var(--vk-surface-container-low)]">
            <Heading level=HeadingLevel::H3 role=TypeRole::Title>
                <Link href variant=LinkVariant::Standalone attr:class="vk-public-row__link"
                    on:click=move |event| {
                        // A refreshed row may dispose its callback during click dispatch.
                        // The title is still a native link in that case.
                        if let Some(open) = on_open { let _ = open.try_run(event); }
                    }>
                    {title}
                </Link>
            </Heading>
            <WorkByline author profile_id />
            {(!tags.is_empty()).then(|| view! { <p class="vk-public-row-tags">{tags.join(" · ")}</p> })}
        </div>
    }
}

#[component]
pub fn ReaderHeader(
    title: String,
    #[prop(into)] author: Signal<Author>,
    #[prop(optional)] title_in_body: bool,
) -> impl IntoView {
    view! {
        <header id="reader-header" class="vk-reader__header grid justify-items-start">
            <Text role=TypeRole::Caption tone=TextTone::Muted>"Bản công khai"</Text>
            {(!title_in_body).then(|| view! { <Heading level=HeadingLevel::H1 role=TypeRole::Display>{title}</Heading> })}
            {move || view! { <WorkByline author=author.get() profile_id=Some("reader-author".into()) size=AvatarSize::Reader /> }}
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

const DRAFT_META_CLASS: &str = "vk-draft-meta flex flex-wrap gap-vk-1 text-vk-on-surface-variant text-[length:var(--vk-type-caption-size)] font-vk-role-caption [&>span]:min-w-0 [&>span]:wrap-anywhere";

#[component]
pub fn DraftMetadata(version: i64, updated_at: String) -> impl IntoView {
    view! {
        <span class=DRAFT_META_CLASS>
            <span>"Phiên bản " {version}</span>
            <span><span aria-hidden="true">"· "</span><span class="vk-ui-visually-hidden">"Cập nhật "</span><Timestamp instant=updated_at /></span>
        </span>
    }
}

#[component]
pub fn DraftResume(draft: DraftSummary) -> impl IntoView {
    // Disclosure only changes presentation; the full Unicode title and destination stay intact.
    let expandable = draft.title.chars().count() > 60;
    let expanded = RwSignal::new(false);
    view! {
        <div class="vk-draft-resume min-w-0">
            <Surface tone=SurfaceTone::Container radius=Radius::Xl padding=Space::S4>
                <div class="vk-draft-resume__content grid justify-items-start gap-vk-2 min-w-0">
                    <Text role=TypeRole::Label id="resume-label">"Bản thảo gần nhất"</Text>
                    <Heading level=HeadingLevel::H2 role=TypeRole::Title class:vk-draft-resume__title=true>{draft.title}</Heading>
                    <DraftMetadata version=draft.version updated_at=draft.updated_at />
                </div>
                <div class="vk-draft-resume__actions flex flex-wrap items-center gap-vk-2 [max-inline-size:100%] [min-inline-size:0]">
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

const DRAFT_ROW_LINK_CLASS: &str =
    "vk-draft-row__link grid items-center min-w-0 [padding:var(--vk-space-3)_var(--vk-space-4)]";
const DRAFT_ROW_TEXT_CLASS: &str = "vk-draft-row__text grid gap-vk-1 min-w-0";
const DRAFT_ROW_TITLE_CLASS: &str = "vk-draft-row__title wrap-anywhere";

/// One flat navigation target. The complete title stays in the DOM; version stays in facts.
#[component]
pub fn DraftRow(draft: DraftSummary) -> impl IntoView {
    view! {
        <Link href=draft.href attr:class=DRAFT_ROW_LINK_CLASS variant=LinkVariant::Standalone>
            <span class=DRAFT_ROW_TEXT_CLASS>
                <span class=DRAFT_ROW_TITLE_CLASS>{draft.title}</span>
                <span class=DRAFT_META_CLASS><span>"Cập nhật " <Timestamp instant=draft.updated_at /></span></span>
            </span>
            <span class="vk-draft-row__open ms-vk-3 items-center" aria-hidden="true">
                <Icon name=IconName::ArrowBack />
            </span>
        </Link>
    }
}

/// Formats a non-negative integer using Vietnamese dot thousands grouping (`1.234`, `1.234.567`),
/// never abbreviating counts (`VIEW-12`, `UX-04`, `D-04`).
pub fn format_vi_count(count: i64) -> String {
    let negative = count < 0;
    let digits = count.unsigned_abs().to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3 + usize::from(negative));
    if negative {
        out.push('-');
    }
    for (idx, ch) in digits.chars().enumerate() {
        if idx > 0 && (len - idx).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

/// Read-only facts supplied by the host; no DTO, request or endorsement command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorWorkSummary {
    pub title: String,
    pub untitled: bool,
    pub updated_at: String,
    pub editor_href: Option<String>,
    pub status: Vec<AuthorWorkLabel>,
    pub public: Option<AuthorWorkPublic>,
    pub views: AuthorWorkViews,
}

/// Selective tonal emphasis for Work Views r3 status badges (no tertiary while DQ-10 is OPEN).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorWorkLabelTone {
    Quiet,
    Ok,
    Attn,
    Bad,
}

impl AuthorWorkLabelTone {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Ok => "ok",
            Self::Attn => "attn",
            Self::Bad => "bad",
        }
    }

    const fn classes(self) -> &'static str {
        match self {
            Self::Quiet => "py-[3px] ps-0 pe-vk-1 text-vk-on-surface-variant",
            Self::Ok => {
                "py-[3px] ps-vk-2 pe-[10px] rounded-vk-sm bg-vk-success-container text-vk-on-success-container"
            }
            Self::Attn => {
                "py-[3px] ps-vk-2 pe-[10px] rounded-vk-sm bg-vk-secondary-container text-vk-on-secondary-container"
            }
            Self::Bad => {
                "py-[3px] ps-vk-2 pe-[10px] rounded-vk-sm bg-vk-error-container text-vk-on-error-container"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorWorkLabel {
    pub short: &'static str,
    pub detail: &'static str,
    pub icon: IconName,
    pub tone: AuthorWorkLabelTone,
}

impl AuthorWorkLabel {
    pub const fn new(
        short: &'static str,
        detail: &'static str,
        icon: IconName,
        tone: AuthorWorkLabelTone,
    ) -> Self {
        Self {
            short,
            detail,
            icon,
            tone,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorWorkPublic {
    pub title: Option<String>,
    pub tham: Option<i64>,
    pub comments: Option<i64>,
}

/// Author-private view totals (`VIEW-07..12`, `UX-01..10`), independent of `current_public`
/// so historical totals survive when a work becomes private or withdrawn (`OQ-1 = A`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorWorkViews {
    Counted {
        count: i64,
        counted_since: Option<String>,
    },
    NotStarted,
    NeverPublic,
    Unavailable,
}

impl AuthorWorkViews {
    pub fn shows_in_metrics(&self) -> bool {
        !matches!(self, Self::NeverPublic)
    }
}

const AUTHOR_WORK_LINK_CLASS: &str = "vk-draft-row__link grid items-center min-w-0 py-vk-4 [padding-inline:calc(var(--vk-space-4)_+_var(--vk-space-1))]";
const AUTHOR_WORK_STATIC_TEXT_CLASS: &str = "vk-draft-row__text grid gap-vk-2 min-w-0 py-vk-4 [padding-inline:calc(var(--vk-space-4)_+_var(--vk-space-1))]";
const AUTHOR_WORK_TEXT_CLASS: &str = "vk-draft-row__text grid gap-vk-2 min-w-0";
const AUTHOR_WORK_CONTEXT_CLASS: &str = "vk-author-work__context m-0 wrap-anywhere";

#[component]
fn AuthorWorkCount(name: &'static str, icon: IconName, count: Option<i64>) -> impl IntoView {
    let formatted = count.map(format_vi_count);
    let title = formatted.as_ref().map_or_else(
        || format!("Chưa tải được số {name}"),
        |n| format!("{n} {name}"),
    );
    view! {
        <span class="vk-author-work__count inline-flex items-center [gap:6px] wrap-anywhere [max-inline-size:100%]" title=title>
            <Icon name=icon />
            {match formatted {
                Some(n) => view! { <span>{n}<span class="vk-ui-visually-hidden">" " {name}</span></span> }.into_any(),
                None => view! { <span>"— " {name}<span class="vk-ui-visually-hidden">" (chưa tải được)"</span></span> }.into_any(),
            }}
        </span>
    }
}

#[component]
fn AuthorWorkViewsMetric(views: AuthorWorkViews, historical: bool) -> impl IntoView {
    match views {
        AuthorWorkViews::Counted {
            count,
            counted_since,
        } => {
            let formatted = format_vi_count(count);
            let title = match &counted_since {
                Some(date) => format!("{formatted} lượt xem, tính từ {date}"),
                None => format!("{formatted} lượt xem"),
            };
            view! {
                <span
                    class="vk-author-work__count vk-author-work__views inline-flex flex-wrap items-center gap-y-vk-1 [column-gap:8px] [max-inline-size:100%]"
                    class:vk-author-work__views--historical=historical
                    data-testid="author-work-views"
                    data-views-state="counted"
                    title=title
                >
                    <span class="vk-author-work__views-value inline-flex items-center [gap:6px] [max-inline-size:100%]">
                        <Icon name=IconName::VisibilityOutline />
                        <span class="wrap-anywhere whitespace-normal">
                            {formatted}
                            <span class="vk-ui-visually-hidden">" lượt xem"</span>
                        </span>
                    </span>
                    {counted_since.map(|date| view! {
                        <span class="vk-author-work__views-since">{format!("Tính từ {date}")}</span>
                    })}
                </span>
            }
            .into_any()
        }
        AuthorWorkViews::NotStarted => view! {
            <span
                class="vk-author-work__count vk-author-work__views vk-author-work__views--off inline-flex flex-wrap items-center gap-y-vk-1 [column-gap:8px] [max-inline-size:100%]"
                data-testid="author-work-views"
                data-views-state="not_started"
                title="Lượt xem: chưa bắt đầu thống kê"
            >
                <span class="vk-author-work__views-value inline-flex items-center [gap:6px] [max-inline-size:100%]">
                    <Icon name=IconName::VisibilityOutline />
                    <span aria-hidden="true">"—"</span>
                </span>
                <span class="vk-author-work__views-reason">
                    <span class="vk-ui-visually-hidden">"Lượt xem: "</span>
                    "Chưa bắt đầu thống kê"
                </span>
            </span>
        }
        .into_any(),
        AuthorWorkViews::Unavailable => view! {
            <span
                class="vk-author-work__count vk-author-work__views vk-author-work__views--off inline-flex flex-wrap items-center gap-y-vk-1 [column-gap:8px] [max-inline-size:100%]"
                data-testid="author-work-views"
                data-views-state="unavailable"
                title="Lượt xem: chưa tải được"
            >
                <span class="vk-author-work__views-value inline-flex items-center [gap:6px] [max-inline-size:100%]">
                    <Icon name=IconName::VisibilityOutline />
                    <span aria-hidden="true">"—"</span>
                </span>
                <span class="vk-author-work__views-reason">
                    <span class="vk-ui-visually-hidden">"Lượt xem: "</span>
                    "Chưa tải được"
                </span>
            </span>
        }
        .into_any(),
        AuthorWorkViews::NeverPublic => ().into_any(),
    }
}

#[component]
pub fn AuthorWorkRow(
    #[prop(into)] work: Signal<AuthorWorkSummary>,
    #[prop(optional)] review_action: Option<Children>,
) -> impl IntoView {
    let details = move |padded: bool| {
        let text_class = if padded {
            AUTHOR_WORK_STATIC_TEXT_CLASS
        } else {
            AUTHOR_WORK_TEXT_CLASS
        };
        view! {
            <div class=text_class>
                <span class=DRAFT_ROW_TITLE_CLASS class:vk-author-work__untitled=move || work.with(|w| w.untitled)>{move || work.get().title}</span>
                <div class="vk-draft-meta vk-author-work__status flex flex-wrap items-center gap-y-vk-2 [column-gap:10px] m-0 wrap-anywhere text-vk-on-surface-variant text-[length:var(--vk-type-caption-size)] font-vk-role-caption [&>span]:min-w-0 [&>span]:wrap-anywhere">
                    {move || work.get().status.into_iter().map(|label| {
                        let class = format!(
                            "vk-author-work__label inline-flex items-center gap-vk-1 [max-inline-size:100%] {}",
                            label.tone.classes()
                        );
                        view! {
                            <span class=class data-tone=label.tone.id() title=label.detail>
                                <Icon name=label.icon size=IconSize::Small />
                                <span aria-hidden="true">{label.short}</span>
                                <span class="vk-ui-visually-hidden">{label.detail}</span>
                            </span>
                        }
                    }).collect_view()}
                    <span class="vk-author-work__time">"Cập nhật " {move || view! { <Timestamp instant=work.get().updated_at /> }}</span>
                </div>
                <Show when=move || work.with(|w| w.editor_href.is_none())>
                    <p class=AUTHOR_WORK_CONTEXT_CLASS>"Không có bản nháp để sửa"</p>
                </Show>
                {move || {
                    let current = work.get();
                    let has_metrics = current.public.is_some() || current.views.shows_in_metrics();
                    let historical = current.public.is_none();
                    let public_title = current.public.as_ref().and_then(|p| p.title.clone());
                    let never_public = current.views == AuthorWorkViews::NeverPublic;
                    view! {
                        {has_metrics.then(|| {
                            let public = current.public.clone();
                            let views = current.views.clone();
                            view! {
                                <p class="vk-author-work__metrics flex flex-wrap items-center gap-y-vk-1 [column-gap:20px] mt-vk-1 mb-0 mx-0 wrap-anywhere" data-testid="author-work-metrics">
                                    {public.map(|p| view! {
                                        <AuthorWorkCount name="Thấm" icon=IconName::PenFlight count=p.tham />
                                        <AuthorWorkCount name="nhận xét" icon=IconName::ChatBubbleOutline count=p.comments />
                                    })}
                                    <AuthorWorkViewsMetric views historical />
                                </p>
                            }
                        })}
                        {never_public.then(|| view! {
                            <span class="vk-ui-visually-hidden" data-testid="author-work-views-never-public">
                                "Lượt xem: chưa có số vì tác phẩm chưa từng công khai"
                            </span>
                        })}
                        {public_title.map(|title| view! {
                            <p class=AUTHOR_WORK_CONTEXT_CLASS>"Bản công khai: " {title}</p>
                        })}
                    }
                }}
            </div>
        }
    };
    view! {
        <div class="vk-author-work flex flex-wrap items-center [min-inline-size:0] [background:var(--vk-bg-content)] rounded-vk-card">
            <Show when=move || work.with(|w| w.editor_href.is_some()) fallback=move || details(true)>
                <Link href=Signal::derive(move || work.get().editor_href.unwrap_or_default()) attr:class=AUTHOR_WORK_LINK_CLASS variant=LinkVariant::Standalone>
                    {details(false)}
                </Link>
            </Show>
            {review_action.map(|action| action())}
        </div>
    }
}

/// The Editor R4 save bar: one live readout, the caller's save action and a native “Chi tiết
/// lưu” disclosure for the server detail. Save authority stays in EditorSession and the body in
/// CodeMirror; the bar owns no state.
#[component]
pub fn EditorToolbar(
    #[prop(into)] status: Signal<String>,
    #[prop(into)] status_token: Signal<String>,
    #[prop(optional, into)] status_problem: Option<Signal<Option<String>>>,
    #[prop(into)] status_detail: Signal<String>,
    #[prop(optional)] save_action: Option<ChildrenFn>,
) -> impl IntoView {
    let problem = move || status_problem.and_then(|value| value.get());
    view! {
        <div class="vk-editor-toolbar flex flex-wrap items-center gap-y-vk-2 gap-x-vk-4 min-w-0">
            <div
                id="save-slot"
                class="vk-save-slot flex flex-wrap items-center gap-vk-2 min-w-0"
                data-status=move || status_token.get()
                data-problem=problem
            >
                <span class="vk-save-slot__readout inline-flex items-center gap-vk-2 min-w-0">
                    <Show when=move || {
                        matches!(
                            status_token.get().as_str(),
                            "saved" | "failed" | "conflict"
                        )
                    }>
                        <span class="vk-save-slot__icon inline-flex flex-none items-center" aria-hidden="true">
                            {move || {
                                let token = status_token.get();
                                let kind = problem();
                                match token.as_str() {
                                    "saved" => view! { <Icon name=IconName::Check size=IconSize::Small /> }.into_any(),
                                    "conflict" => view! { <Icon name=IconName::Warning size=IconSize::Small /> }.into_any(),
                                    "failed" if kind.as_deref() == Some("uncertain") => {
                                        view! { <Icon name=IconName::Warning size=IconSize::Small /> }.into_any()
                                    }
                                    "failed" => view! { <Icon name=IconName::Error size=IconSize::Small /> }.into_any(),
                                    _ => ().into_any(),
                                }
                            }}
                        </span>
                    </Show>
                    <p
                        class="vk-save-status"
                        role="status"
                        aria-live="polite"
                        aria-atomic="true"
                        data-testid="save-status"
                        data-status=move || status_token.get()
                        data-problem=problem
                    >
                        {move || status.get()}
                    </p>
                </span>
                {save_action.map(|action| view! { <div class="vk-save-slot__action flex-wrap items-center gap-vk-2 min-w-0">{action()}</div> })}
            </div>
            // Native disclosure, not an ARIA menu/overlay: Enter/Space toggle, Escape leaves it
            // open. Its summary is always visible, including when a focused Save disappears after
            // an ACK (#217), so focus has somewhere to land.
            <details id="editor-save-details" class="vk-editor-tools [min-inline-size:0]">
                <summary aria-label="Chi tiết lưu" title="Chi tiết lưu"><span aria-hidden="true">"…"</span></summary>
                <div class="vk-editor-tools__content p-vk-3">
                    <p><strong>"Trạng thái lưu. "</strong>{move || status_detail.get()}</p>
                </div>
            </details>
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
    /// Optional reactive field mode. Closing keeps the native field and its draft.
    #[prop(optional, into)]
    field_mode: Signal<FieldMode>,
    /// Reader-only tonal recipe; other feedback consumers retain their current form.
    #[prop(optional)]
    tonal: bool,
    #[prop(optional, into)] field_label: Option<String>,
    /// The caller may describe the active reply target without changing the editor's name.
    #[prop(optional_no_strip)]
    field_description: Option<String>,
    /// Replace only the action (e.g. read-only recovery), not the editor or its selection.
    #[prop(optional, into)]
    show_alternative: Signal<bool>,
    #[prop(optional)] alternative: Option<ViewFn>,
    /// Emphasis only (R4: tonal while empty, filled with text); `state` still gates presses.
    #[prop(optional, into)]
    send_variant: Option<Signal<ButtonVariant>>,
    /// Decorative leading glyph for the tonal send action. The action state owns its reason.
    #[prop(optional, into)]
    send_icon: Option<Signal<IconName>>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let submit_id = format!("{id}-send");
    let send_variant = send_variant.unwrap_or_else(|| Signal::stored(ButtonVariant::Filled));
    let send_icon = Memo::new(move |_| send_icon.map_or(IconName::Send, |icon| icon.get()));
    let submit_state = Signal::derive(move || {
        if matches!(state.get(), ActionState::Pending { .. }) {
            state.get()
        } else if comments_closed.get() {
            ActionState::disabled("Tác giả đã tắt bình luận. Bản đang soạn được giữ lại.")
        } else {
            state.get()
        }
    });
    let label = field_label.unwrap_or_else(|| {
        if intent == FeedbackIntent::Technique {
            "Góp ý kỹ thuật có ngữ cảnh"
        } else {
            "Nhận xét của bạn"
        }
        .into()
    });
    view! {
        <section class="vk-feedback-composer grid" class:vk-feedback-composer--tonal=tonal>
            {children.map(|header| header())}
            {intent.label().map(|label| view! { <Text role=TypeRole::Label>{label}</Text> })}
            <TextArea id label described_by=field_description
                value on_input mode=field_mode variant=if tonal { FieldVariant::FloatingTonal } else { FieldVariant::ExternalLabel }
                rows=if tonal { 2 } else { 4 }
                help=if tonal { "" } else { "Tối đa 4.000 ký tự, 200 dòng. Enter xuống dòng; chỉ gửi khi bấm nút." } />
            <Show when=move || tonal && !value.get().is_empty() && !comments_closed.get()><p class="vk-feedback-draft-note m-0 [padding-inline:6px]">"Rời trang có thể mất nháp."</p></Show>
            <div class="vk-feedback-composer__footer flex flex-wrap items-center gap-vk-2 [padding-inline:4px] min-w-0">
                <Show when=move || tonal && !value.get().is_empty() && !comments_closed.get() && !show_alternative.get()>
                    <p class="vk-feedback-policy items-center gap-[6px] m-0 min-w-0">
                        <Icon name=IconName::VerifiedUser size=IconSize::Small />
                        <span>"Duyệt trước khi hiện công khai"</span>
                    </p>
                </Show>
                <Show when=move || show_alternative.get() fallback=move || {
                    if tonal {
                        let id = submit_id.clone();
                        view! { {move || view! {
                            <Button id=id.clone() variant=send_variant state=submit_state on_press=on_submit leading=send_icon.get()>"Gửi"</Button>
                        }} }.into_any()
                    } else {
                        view! { <Button id=submit_id.clone() variant=send_variant state=submit_state on_press=on_submit>"Gửi nhận xét"</Button> }.into_any()
                    }
                }>
                    {alternative.as_ref().map(ViewFn::run)}
                </Show>
            </div>
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
    Reply {
        name: String,
        href: Option<String>,
        profile: Option<ProfileLink>,
    },
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

/// Inline confirmation of “Gỡ nhận xét của tôi” (VDR-DISCUSSION-R4@r1). The adapter owns which
/// comment is confirming and whether removal is still allowed; `on_remove` asks, `on_confirm`
/// is the only path to a removal request.
#[derive(Clone, Copy)]
pub struct RemoveConfirmation {
    pub confirming: Callback<String, bool>,
    pub on_confirm: Callback<String>,
    pub on_cancel: Callback<String>,
}

/// “Gỡ nhận xét của tôi” and, once asked, its confirmation in the same place: a destructive
/// “Gỡ” and “Giữ lại”. Nothing is sent until “Gỡ”; the adapter moves focus between the two.
#[component]
pub fn OwnRemoveAction(
    #[prop(into)] comment_id: String,
    #[prop(into)] confirming: Signal<bool>,
    #[prop(optional, into)] state: Signal<ActionState>,
    #[prop(into)] on_ask: Callback<()>,
    #[prop(into)] on_confirm: Callback<()>,
    #[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
    let ask_id = format!("remove-comment-{comment_id}");
    let confirm_id = format!("confirm-remove-comment-{comment_id}");
    let cancel_id = format!("cancel-remove-comment-{comment_id}");
    let prompt_id = format!("remove-comment-{comment_id}-prompt");
    move || {
        if confirming.get() {
            view! {
                <div class="vk-discussion-remove-confirm [inline-size:100%] box-border grid gap-vk-2 min-w-0" role="group" aria-labelledby=prompt_id.clone()>
                    <p id=prompt_id.clone()>
                        <Icon name=IconName::Warning size=IconSize::Small />
                        <span>"Gỡ nhận xét này? Sau khi gỡ, bạn không thể khôi phục."</span>
                    </p>
                    <div class="vk-discussion-remove-actions flex flex-wrap justify-end gap-vk-2">
                        <Button id=cancel_id.clone() variant=ButtonVariant::Text on_press=on_cancel>"Giữ lại"</Button>
                        <Button id=confirm_id.clone() variant=ButtonVariant::Danger state on_press=on_confirm>"Gỡ"</Button>
                    </div>
                </div>
            }
            .into_any()
        } else {
            view! { <Button id=ask_id.clone() variant=ButtonVariant::Text state on_press=on_ask>"Gỡ nhận xét của tôi"</Button> }
                .into_any()
        }
    }
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
    /// Read-only work page (#360 ảnh 10): hides reply actions and shows the static Ưng label without a count.
    #[prop(optional, into)]
    read_only: Signal<bool>,
    /// Whether this thread's reply composer is expanded (`aria-expanded` on root Trả lời).
    #[prop(optional, into)]
    reply_expanded: Signal<bool>,
    /// Currently targeted reply/root comment ID inside this thread.
    #[prop(optional, into)]
    reply_target_id: Signal<Option<String>>,
    #[prop(optional)] reply_footer: Option<ChildrenFn>,
    /// Without it, `on_remove` is the removal itself (samples and previews).
    #[prop(optional)]
    removal: Option<RemoveConfirmation>,
) -> impl IntoView {
    let root = Memo::new(move |_| facts.get().root);
    let shared_block = Memo::new(move |_| {
        if read_only.get() {
            return None;
        }
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
    let ctx = ThreadCommentCtx {
        on_reply,
        on_remove,
        removal,
        state: action_state,
        endorsements,
        shared_description,
        read_only,
        reply_target_id,
    };
    view! {
        <div class="vk-discussion-thread grid gap-vk-3 min-w-0">
            {move || root.get().map(|comment| discussion_comment(comment, true, reply_expanded, ctx))
                .unwrap_or_else(|| view! { <p class="vk-muted">"Bình luận gốc không còn hiển thị."</p> }.into_any())}
            <div class="vk-discussion-reply-group min-w-0" hidden=move || facts.get().replies.is_empty() && !has_more_replies.get()>
            <ul class="vk-discussion-replies grid gap-[16px] [list-style:none] p-0 m-0 [border:0]" aria-label="Phản hồi theo thứ tự cũ đến mới">
                <For each=move || { facts.get().replies.into_iter().map(|comment| comment.id).collect::<Vec<_>>() } key=|id| id.clone()
                    children=move |id| {
                        let comment = Memo::new(move |_| facts.get().replies.into_iter().find(|comment| comment.id == id));
                        view! { <li class="min-w-0">{move || comment.get().map(|comment| discussion_comment(comment, false, Signal::stored(false), ctx))}</li> }
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
    surface: vink_ui::ActionSurface,
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
                    surface
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

#[derive(Clone, Copy)]
struct ThreadCommentCtx {
    on_reply: Callback<String>,
    on_remove: Callback<String>,
    removal: Option<RemoveConfirmation>,
    state: Signal<ActionState>,
    endorsements: Option<CommentEndorsements>,
    shared_description: Signal<Option<String>>,
    read_only: Signal<bool>,
    reply_target_id: Signal<Option<String>>,
}

const DISCUSSION_CONTEXT_CLASS: &str =
    "vk-discussion-context flex flex-wrap items-baseline gap-y-vk-1 gap-x-vk-2";

fn discussion_comment(
    comment: DiscussionComment,
    is_root: bool,
    reply_expanded: Signal<bool>,
    ctx: ThreadCommentCtx,
) -> AnyView {
    let ThreadCommentCtx {
        on_reply,
        on_remove,
        removal,
        state,
        endorsements,
        shared_description,
        read_only,
        reply_target_id,
    } = ctx;
    let reply_id = comment.id.clone();
    let remove_id = comment.id.clone();
    let target_match_id = comment.id.clone();
    let endorsement_id = comment.id.clone();
    let reply_button_id = format!("reply-to-{}", comment.id);
    let remove_button_id = format!("remove-comment-{}", comment.id);
    let author_id = format!("comment-{}-author", comment.id);
    let context_id = format!("comment-{}-context-author", comment.id);
    let author_label = comment.author.name.clone();
    // A reply sits in the tonal reply group: one step smaller than a root (#352).
    let size = if is_root {
        AvatarSize::Reader
    } else {
        AvatarSize::Medium
    };
    let surface = if is_root {
        vink_ui::ActionSurface::Tonal
    } else {
        vink_ui::ActionSurface::Standard
    };
    let published_at = comment.author.published_at.clone();
    let is_target = Signal::derive(move || {
        !is_root
            && !read_only.get()
            && reply_target_id.get().as_deref() == Some(target_match_id.as_str())
    });
    view! {
        <article id=format!("comment-{}", comment.id) class="vk-discussion-comment grid gap-vk-2 min-w-0" tabindex="-1"
            data-reply-target=move || is_target.get().then_some("true")>
            <div class="vk-discussion-byline">
                <AuthorIdentity author=comment.author id=Some(author_id) size />
                // Outside the profile link.
                <Timestamp instant=published_at relative=true />
            </div>
            {comment.context.map(|context| match context {
                DiscussionContext::Root => ().into_any(),
                DiscussionContext::Unavailable => view! { <p class=DISCUSSION_CONTEXT_CLASS>"Bình luận được trả lời không còn hiển thị"</p> }.into_any(),
                DiscussionContext::Reply { name, href, profile } => view! {
                    <p class=DISCUSSION_CONTEXT_CLASS>"Đang trả lời "
                        <ProfileName name profile id=context_id />
                        {href.map(|href| view! { <Link href>"Đến bình luận"</Link> })}
                    </p>
                }.into_any(),
            })}
            {comment.status.map(|status| view! { <p class="vk-discussion-status">{status}</p> })}
            <p class="vk-discussion-body">{comment.body}</p>
            <div class="vk-discussion-actions flex flex-wrap items-center gap-vk-2 min-w-0">
                <div class="vk-discussion-action-segment inline-flex flex-wrap items-center [max-inline-size:100%] min-w-0" data-root=is_root.then_some("true")
                    role=move || (!read_only.get()).then_some("group")
                    aria-label=move || (!read_only.get()).then(|| format!("Thao tác với nhận xét của {author_label}"))>
                    {move || {
                        if read_only.get() {
                            view! {
                                <span class="vk-discussion-static-endorsement inline-flex items-center gap-[6px] [padding:6px_12px] rounded-vk-full" data-endorsement-readonly=endorsement_id.clone()>
                                    <Icon name=IconName::HeartOutline size=IconSize::Small />
                                    <span>"Ưng"</span>
                                </span>
                            }.into_any()
                        } else {
                            endorsements.map(|endorsements| {
                                comment_endorsement(endorsement_id.clone(), surface, endorsements, shared_description)
                            }).into_any()
                        }
                    }}
                    {move || (comment.can_reply && !read_only.get()).then(|| {
                        let reply_id = reply_id.clone();
                        let reply_button_id = reply_button_id.clone();
                        if is_root {
                            view! {
                                <Button id=reply_button_id variant=ButtonVariant::Tonal leading=IconName::Reply
                                    expanded=reply_expanded on_press=move |_| on_reply.run(reply_id.clone())>"Trả lời"</Button>
                            }.into_any()
                        } else {
                            view! {
                                <Button id=reply_button_id variant=ButtonVariant::Text leading=IconName::Reply
                                    on_press=move |_| on_reply.run(reply_id.clone())>"Trả lời"</Button>
                            }.into_any()
                        }
                    })}
                </div>
                {comment.can_remove.then(|| match removal {
                    Some(removal) => {
                        let (ask, confirm, cancel, shown) = (remove_id.clone(), remove_id.clone(), remove_id.clone(), remove_id.clone());
                        view! {
                            <OwnRemoveAction comment_id=remove_id.clone() state
                                confirming=Signal::derive(move || removal.confirming.run(shown.clone()))
                                on_ask=move |_| on_remove.run(ask.clone())
                                on_confirm=move |_| removal.on_confirm.run(confirm.clone())
                                on_cancel=move |_| removal.on_cancel.run(cancel.clone()) />
                        }.into_any()
                    }
                    None => view! { <Button id=remove_button_id variant=ButtonVariant::Text state on_press=move |_| on_remove.run(remove_id.clone())>"Gỡ nhận xét của tôi"</Button> }.into_any(),
                })}
            </div>
        </article>
    }.into_any()
}

/// What a literary endorsement (LVE #73) is about. Copy only: one positive signal per viewer and
/// target, decided by the App's `domain::endorsement` through its adapter, never here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EndorsementKind {
    /// One published revision of a work: “Thấm”.
    Work,
    /// A root comment or a flat reply, each its own target: “Ưng” (heart). The 20/11/2026
    /// rename of “Hữu ích” (#359) changes copy and glyph only, never the signal's semantics.
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
                "Thấm",
                "Bỏ Thấm",
                "lượt Thấm",
                "Đang gửi Thấm…",
                "Đang bỏ Thấm…",
                "Không thể Thấm lúc này.",
                "Chưa rõ bạn đã Thấm chưa.",
            ),
            Self::Comment => (
                "Ưng",
                "Bỏ Ưng",
                "lượt Ưng",
                "Đang gửi Ưng…",
                "Đang bỏ Ưng…",
                "Không thể Ưng lúc này.",
                "Chưa rõ bạn đã Ưng chưa.",
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

    /// Decorative leading glyph for a confirmed selection; the label carries the meaning and
    /// the outline/filled pair keeps “Ưng” readable without colour.
    pub const fn icon(self, selected: bool) -> IconName {
        match (self, selected) {
            (Self::Work, _) => IconName::PenFlight,
            (Self::Comment, false) => IconName::HeartOutline,
            (Self::Comment, true) => IconName::Heart,
        }
    }

    /// Visible text, never hover-only; no login button is implied (no auth authority yet).
    pub const fn reason(self, block: EndorsementBlock) -> &'static str {
        match (self, block) {
            (Self::Work, EndorsementBlock::OwnContent) => {
                "Bạn không thể tự Thấm tác phẩm của mình."
            }
            (Self::Comment, EndorsementBlock::OwnContent) => {
                "Bạn không thể tự Ưng nhận xét của mình."
            }
            (Self::Work, EndorsementBlock::NotPermitted) => "Tài khoản này chưa thể Thấm.",
            (Self::Comment, EndorsementBlock::NotPermitted) => "Tài khoản này chưa thể Ưng.",
            (Self::Work, EndorsementBlock::SignedOut) => "Cần đăng nhập để Thấm.",
            (Self::Comment, EndorsementBlock::SignedOut) => "Cần đăng nhập để Ưng.",
            (Self::Work, EndorsementBlock::Unavailable) => "Không thể Thấm lúc này.",
            (Self::Comment, EndorsementBlock::Unavailable) => "Tạm chưa thể Ưng.",
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
/// owns `command_key`, `expected_order`, session fencing and late answers. “Thấm” keeps the
/// decorative Pen Flight R2 in every phase (CHOICE-07 A); “Ưng” shows an outline heart until
/// the confirmed value is on, then a filled one. Confirmation keeps its own trailing slot.
#[component]
pub fn EndorsementAction(
    /// Presentation only; confirmed facts and authority remain with the adapter.
    #[prop(optional, into)]
    surface: Signal<vink_ui::ActionSurface>,
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
        <span class="vk-endorsement inline-flex [max-inline-size:100%]" data-kind=kind_id>
            <ToggleAction
                surface
                id
                copy=kind.copy()
                icon=kind.icon(false)
                selected_icon=kind.icon(true)
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

/// Why the viewer's last save intent was refused; nothing changed (Epic #384 D6/D11). The
/// quota has its own sentence and never names a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SaveRefusal {
    Save,
    Unsave,
    Limit,
}

impl SaveRefusal {
    pub const fn text(self) -> &'static str {
        match self {
            Self::Save => "Chưa lưu được. Bạn có thể thử lại.",
            Self::Unsave => "Chưa bỏ lưu được. Bạn có thể thử lại.",
            Self::Limit => "Danh sách Đã lưu đã đầy. Bỏ lưu một bài để lưu thêm.",
        }
    }
}

/// A result the reader reports beside the pair, only after the owner confirmed it (D11).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SaveOutcome {
    /// Saved; `list_href` opens the viewer's own Đã lưu list once their handle is known.
    Saved {
        list_href: Option<String>,
    },
    Unsaved,
    /// A guest signed in from the save sheet (D2): nothing was saved on their behalf.
    SignedIn,
}

/// What a save adapter projects for one work and the current session. Membership, generation,
/// session scope and late answers stay with the adapter; the pattern renders facts only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SaveFacts {
    /// Confirmed membership; `Unknown` until the private read settles (never assumed Off).
    pub value: ToggleValue,
    pub phase: TogglePhase,
    /// The confirmed state is being read right now (first read or Reconcile).
    pub reading: bool,
    pub refusal: Option<SaveRefusal>,
    pub outcome: Option<SaveOutcome>,
    /// A visible reason why this account cannot save here.
    pub blocked: Option<&'static str>,
}

/// SAV-04 copy: the visible label stays “Lưu” in every state (D3); the confirmed state is
/// `aria-pressed`, the filled bookmark, the tonal pill and the check mark.
/// The accessible name is also always “Lưu” (owner decision 26/11/2026, PR #546): with
/// `aria-pressed`, a name switching to `selected_name` (“Bỏ lưu”) would read “Bỏ lưu, đã nhấn”.
/// `selected_name` is therefore never rendered here; keep it only for the shared copy shape.
pub fn save_copy() -> ToggleCopy {
    ToggleCopy {
        pending_on: "Đang lưu…".into(),
        pending_off: "Đang bỏ lưu…".into(),
        uncertain: "Chưa xác nhận được lượt lưu.".into(),
        failed: SaveRefusal::Save.text().into(),
        unknown: "Chưa rõ bạn đã lưu bài này chưa.".into(),
        reconcile: "Kiểm tra lại".into(),
        ..ToggleCopy::new("Lưu", "Bỏ lưu")
    }
}

/// “Lưu” beside “Thấm” at the end of the Reader (SAV-04, D1–D5, D11). A press emits the desired
/// membership; the adapter owns the request, the generation fence and the session. Nothing is
/// optimistic: icon, `aria-pressed` and shape change only with `facts.value`.
#[component]
pub fn SaveAction(
    #[prop(into)] id: String,
    #[prop(into)] facts: Signal<SaveFacts>,
    #[prop(into)] on_set: Callback<bool>,
    #[prop(into)] on_recover: Callback<RecoveryIntent>,
) -> impl IntoView {
    let list_id = format!("{id}-list");
    let outcome = Signal::derive(move || {
        facts
            .with(|f| f.outcome.clone())
            .map(|outcome| match outcome {
                SaveOutcome::Saved { list_href } => ToggleOutcome {
                    text: "Đã lưu vào hồ sơ của bạn.".into(),
                    confirmed: true,
                    link: list_href.map(|href| ToggleLink {
                        id: list_id.clone(),
                        label: "Mở Đã lưu".into(),
                        href,
                    }),
                },
                SaveOutcome::Unsaved => ToggleOutcome {
                    text: "Đã bỏ lưu.".into(),
                    confirmed: true,
                    link: None,
                },
                SaveOutcome::SignedIn => ToggleOutcome {
                    text: "Bạn đã đăng nhập. Bấm Lưu để lưu bài này.".into(),
                    confirmed: false,
                    link: None,
                },
            })
    });
    view! {
        <span class="vk-save" data-kind="save">
            <ToggleAction
                id
                copy=save_copy()
                icon=IconName::BookmarkBorder
                selected_icon=IconName::Bookmark
                // Stable name “Lưu” + `aria-pressed` (26/11/2026); Thấm stays action-named.
                pressed_semantics=true
                note_icons=true
                value=Signal::derive(move || facts.with(|f| f.value))
                phase=Signal::derive(move || facts.with(|f| f.phase))
                reading=Signal::derive(move || facts.with(|f| f.reading))
                blocked=Signal::derive(move || facts.with(|f| f.blocked.map(str::to_string)))
                failed_note=Signal::derive(move || facts.with(|f| f.refusal.map(|r| r.text().to_string())))
                outcome
                on_set
                on_recover
            />
        </span>
    }
}

/// A connected M3 Expressive pair of equal actions (“Thấm · Lưu”, D5): one named group, two
/// independent controls, each with its own state and feedback row under the pair. The host is
/// the inline-size container, so the pair stacks before a label or count would be cut.
#[component]
pub fn ActionPair(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="vk-action-pair-host">
            <div id=id class="vk-action-pair" role="group" aria-label=label>
                {children()}
            </div>
        </div>
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

#[cfg(test)]
mod tests {
    use super::*;

    fn save_html(facts: SaveFacts) -> String {
        Owner::new().with(|| {
            view! {
                <ActionPair id="sv-pair" label="Thấm và lưu tác phẩm">
                    <SaveAction id="save-work" facts=Signal::stored(facts)
                        on_set=|_: bool| {} on_recover=|_: RecoveryIntent| {} />
                </ActionPair>
            }
            .to_html()
        })
    }

    fn save_button(html: &str) -> &str {
        html.split("<button")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap()
    }

    #[test]
    fn save_action_keeps_the_label_and_states_membership_with_aria_pressed_shape_and_glyph() {
        let off = save_html(SaveFacts {
            value: ToggleValue::Off,
            ..SaveFacts::default()
        });
        let b = save_button(&off);
        for needle in [
            "id=\"save-work\"",
            "aria-label=\"Lưu\"",
            "aria-pressed=\"false\"",
            "data-state=\"enabled\"",
        ] {
            assert!(b.contains(needle), "{needle} in {b}");
        }
        assert!(
            off.contains("role=\"group\"") && off.contains("aria-label=\"Thấm và lưu tác phẩm\""),
            "{off}"
        );
        assert!(
            off.contains("data-icon=\"bookmark-border\"") && !off.contains("data-icon=\"check\""),
            "{off}"
        );

        let on = save_html(SaveFacts {
            value: ToggleValue::On,
            outcome: Some(SaveOutcome::Saved {
                list_href: Some("/@an/da-luu".into()),
            }),
            ..SaveFacts::default()
        });
        let b = save_button(&on);
        assert!(
            b.contains("aria-label=\"Lưu\"") && b.contains("aria-pressed=\"true\""),
            "D3 label stays: {b}"
        );
        assert!(
            on.contains("data-icon=\"bookmark\"") && on.contains("data-icon=\"check\""),
            "filled + check: {on}"
        );
        assert!(
            on.contains("Đã lưu vào hồ sơ của bạn.")
                && on.contains("href=\"/@an/da-luu\"")
                && on.contains("Mở Đã lưu"),
            "{on}"
        );
        assert!(!on.contains("Bỏ lưu"), "no state-flipping name: {on}");

        let unknown = save_html(SaveFacts {
            reading: true,
            ..SaveFacts::default()
        });
        let b = save_button(&unknown);
        assert!(
            !b.contains("aria-pressed")
                && b.contains("aria-busy=\"true\"")
                && b.contains("data-pressed=\"unknown\""),
            "{b}"
        );
        assert!(
            !unknown.contains("Chưa rõ") && !unknown.contains("Kiểm tra lại"),
            "a first read is quiet: {unknown}"
        );
    }

    #[test]
    fn save_action_pending_refusal_and_lost_answer_never_claim_a_new_state() {
        let pending = save_html(SaveFacts {
            value: ToggleValue::Off,
            phase: TogglePhase::Pending { desired: true },
            outcome: Some(SaveOutcome::Unsaved),
            ..SaveFacts::default()
        });
        let b = save_button(&pending);
        assert!(
            b.contains("aria-pressed=\"false\"") && b.contains("aria-busy=\"true\""),
            "{b}"
        );
        assert!(
            pending.contains("Đang lưu…") && !pending.contains("Đã bỏ lưu."),
            "{pending}"
        );
        assert!(
            pending.contains("data-icon=\"bookmark-border\""),
            "no optimistic glyph: {pending}"
        );

        let pending_off = save_html(SaveFacts {
            value: ToggleValue::On,
            phase: TogglePhase::Pending { desired: false },
            ..SaveFacts::default()
        });
        assert!(
            pending_off.contains("Đang bỏ lưu…") && pending_off.contains("data-icon=\"bookmark\""),
            "{pending_off}"
        );

        let limit = save_html(SaveFacts {
            value: ToggleValue::Off,
            phase: TogglePhase::Failed,
            refusal: Some(SaveRefusal::Limit),
            ..SaveFacts::default()
        });
        assert!(
            limit.contains("Danh sách Đã lưu đã đầy. Bỏ lưu một bài để lưu thêm.")
                && limit.contains("data-note=\"failed\""),
            "{limit}"
        );
        assert!(
            limit.contains("data-icon=\"error\"") && !limit.contains("10.000"),
            "{limit}"
        );
        let unsave = save_html(SaveFacts {
            value: ToggleValue::On,
            phase: TogglePhase::Failed,
            refusal: Some(SaveRefusal::Unsave),
            ..SaveFacts::default()
        });
        assert!(
            unsave.contains("Chưa bỏ lưu được. Bạn có thể thử lại."),
            "{unsave}"
        );

        let lost = save_html(SaveFacts {
            value: ToggleValue::Off,
            phase: TogglePhase::Uncertain { desired: true },
            ..SaveFacts::default()
        });
        let b = save_button(&lost);
        assert!(
            b.contains("aria-disabled=\"true\"") && b.contains("aria-pressed=\"false\""),
            "{b}"
        );
        assert!(
            lost.contains("Chưa xác nhận được lượt lưu.") && lost.contains("Kiểm tra lại"),
            "{lost}"
        );
        let checking = save_html(SaveFacts {
            value: ToggleValue::Off,
            phase: TogglePhase::Uncertain { desired: true },
            reading: true,
            ..SaveFacts::default()
        });
        assert!(
            checking.contains("Chưa xác nhận được lượt lưu.") && !checking.contains("Kiểm tra lại"),
            "{checking}"
        );

        let guest_hint = save_html(SaveFacts {
            value: ToggleValue::Off,
            outcome: Some(SaveOutcome::SignedIn),
            ..SaveFacts::default()
        });
        assert!(
            guest_hint.contains("Bạn đã đăng nhập. Bấm Lưu để lưu bài này.")
                && guest_hint.contains("data-icon=\"info\""),
            "{guest_hint}"
        );
        let blocked = save_html(SaveFacts {
            blocked: Some("Tài khoản đang bị tạm khóa nên chưa thể lưu bài."),
            ..SaveFacts::default()
        });
        assert!(
            blocked.contains("chưa thể lưu bài")
                && save_button(&blocked).contains("data-state=\"disabled\""),
            "{blocked}"
        );
    }

    #[test]
    fn compact_row_keeps_unicode_destination_and_version_facts_outside_row_presentation() {
        let draft = DraftSummary {
            title: "Đường về — ỹ ỷ ữ ợ".into(),
            version: 17,
            updated_at: "2026-11-01T23:30:00Z".into(),
            href: "/ban-viet/work-17".into(),
        };
        let row = Owner::new().with(|| view! { <DraftRow draft=draft.clone() /> }.to_html());
        assert!(
            row.contains(&draft.title) && row.contains("href=\"/ban-viet/work-17\""),
            "{row}"
        );
        assert!(
            row.contains("datetime=\"2026-11-01T23:30:00Z\"") && !row.contains("Phiên bản"),
            "{row}"
        );
        let metadata = Owner::new().with(|| {
            view! { <DraftMetadata version=draft.version updated_at=draft.updated_at /> }.to_html()
        });
        assert!(
            metadata.contains("Phiên bản ") && metadata.contains("17"),
            "other consumers retain version: {metadata}"
        );
    }

    fn person(name: &str, handle: Option<&str>, neutral: bool) -> Author {
        Author {
            name: name.into(),
            key: None,
            published_at: "2026-11-20T02:00:00Z".into(),
            profile: handle.map(|h| ProfileLink {
                href: format!("/@{h}"),
                handle: h.into(),
            }),
            neutral,
        }
    }

    #[test]
    fn author_identity_distinguishes_shared_pen_names_and_never_rebuilds_a_deleted_person() {
        let render = |author: Author| {
            Owner::new()
                .with(|| view! { <AuthorIdentity author id=Some("who".into()) /> }.to_html())
        };
        let a = render(person("Hải Đường Nhỏ", Some("hai-duong-nho"), false));
        let b = render(person("Hải Đường Nhỏ", Some("hai-duong-pho"), false));
        for (html, handle) in [(&a, "hai-duong-nho"), (&b, "hai-duong-pho")] {
            assert_eq!(html.matches("<a").count(), 1, "{html}");
            assert!(html.contains(&format!(r#"href="/@{handle}""#)), "{html}");
            assert!(
                html.contains(&format!(">@{handle}<")),
                "visible handle: {html}"
            );
            assert!(
                html.contains(&format!("Mở hồ sơ Hải Đường Nhỏ (@{handle})")),
                "{html}"
            );
        }
        // A deleted label is never a link, a handle or initials, even if a stale ref leaked in.
        let deleted = render(person("Tài khoản đã xóa", Some("old-person"), true));
        assert!(
            !deleted.contains("href") && !deleted.contains("old-person"),
            "{deleted}"
        );
        assert!(deleted.contains(r#"data-initials="""#), "{deleted}");
        // Unavailable/legacy text keeps its name (initials) without a link or handle.
        let unavailable = render(person("Mộc Miên", None, false));
        assert!(
            !unavailable.contains("href") && unavailable.contains(r#"data-initials="MM""#),
            "{unavailable}"
        );
    }

    #[test]
    fn comment_byline_keeps_time_outside_the_profile_link() {
        let comment = |id: &str, context| DiscussionComment {
            id: id.into(),
            author: person("An Nhiên", Some("an-nhien"), false),
            body: "Nhận xét".into(),
            status: None,
            context,
            can_reply: true,
            can_remove: false,
        };
        let html = Owner::new().with(|| {
            let facts = DiscussionThreadFacts {
                root_id: "root".into(),
                root: Some(comment("root", None)),
                replies: vec![comment(
                    "reply",
                    Some(DiscussionContext::Reply {
                        name: "An Nhiên".into(),
                        href: Some("#comment-root".into()),
                        profile: Some(ProfileLink {
                            href: "/@an-nhien".into(),
                            handle: "an-nhien".into(),
                        }),
                    }),
                )],
            };
            view! {
                <DiscussionThread facts=Signal::stored(facts) on_reply=|_: String| {} on_remove=|_: String| {} />
            }
            .to_html()
        });
        for (id, size) in [("root", "reader"), ("reply", "md")] {
            let start = html
                .find(&format!(r#"id="comment-{id}-author""#))
                .expect("linked author");
            let link = &html[start..start + html[start..].find("</a>").unwrap()];
            assert!(
                link.contains(&format!("vk-ui-avatar--{size}")) && link.contains("@an-nhien"),
                "{link}"
            );
            assert!(!link.contains("<time"), "time stays outside: {link}");
        }
        assert!(
            html.contains(r#"id="comment-reply-context-author""#)
                && html.contains(r##"href="#comment-root""##),
            "reply context keeps two distinct targets: {html}"
        );
    }

    #[test]
    fn public_composition_escapes_user_text_and_never_invents_a_profile() {
        let html = Owner::new().with(|| view! {
            <WorkCard title="<script>private()</script>".into()
                author=Author { name: "<b>Tác giả</b>".into(), key: None, profile: None, published_at: "2026-11-02T02:00:00Z".into(), neutral: false }
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
            work.contains("aria-label=\"Bỏ Thấm\"") && work.contains(">Thấm<"),
            "{work}"
        );
        assert!(
            work.contains("lượt Thấm") && work.contains("data-kind=\"work\""),
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
            comment.contains("aria-label=\"Ưng\"")
                && comment.contains(">Ưng<")
                && comment.contains("Bạn không thể tự Ưng nhận xét của mình."),
            "{comment}"
        );
        assert!(
            comment.contains("data-icon=\"heart-outline\"") && !comment.contains("pen-flight"),
            "outline heart while off: {comment}"
        );
        assert!(!comment.contains("e-count"), "hidden count: {comment}");
        let liked = render(
            EndorsementKind::Comment,
            EndorsementFacts {
                value: ToggleValue::On,
                count: CountFact::Known(3),
                ..Default::default()
            },
        );
        assert!(
            liked.contains("aria-label=\"Bỏ Ưng\"")
                && liked.contains("lượt Ưng")
                && liked.contains("data-icon=\"heart\""),
            "filled heart once confirmed: {liked}"
        );
        let pressing = render(
            EndorsementKind::Comment,
            EndorsementFacts {
                value: ToggleValue::Off,
                phase: TogglePhase::Pending { desired: true },
                ..Default::default()
            },
        );
        assert!(
            pressing.contains("data-icon=\"heart-outline\"") && pressing.contains("Đang gửi Ưng…"),
            "no optimistic heart: {pressing}"
        );
        for copy in [
            EndorsementKind::Comment.copy(),
            EndorsementKind::Work.copy(),
        ] {
            let all = format!("{copy:?}").to_lowercase();
            assert!(!all.contains("hữu ích"), "{all}");
        }
        let guest = render(
            EndorsementKind::Work,
            EndorsementFacts {
                block: Some(EndorsementBlock::SignedOut),
                ..Default::default()
            },
        );
        assert!(
            guest.contains("Cần đăng nhập để Thấm.") && !guest.contains("<a "),
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
    fn r4_removal_asks_in_place_and_only_confirm_reaches_the_adapter() {
        let comment = |id: &str, can_remove| DiscussionComment {
            id: id.into(),
            author: Author {
                name: "Người viết".into(),
                key: None,
                profile: None,
                published_at: "2026-11-20T02:00:00Z".into(),
                neutral: false,
            },
            body: format!("Bình luận {id}"),
            status: None,
            context: None,
            can_reply: false,
            can_remove,
        };
        let render = |confirming: &'static str| {
            Owner::new().with(|| {
                let facts = DiscussionThreadFacts {
                    root_id: "root".into(),
                    root: Some(comment("root", true)),
                    replies: vec![comment("other", false)],
                };
                let removal = RemoveConfirmation {
                    confirming: Callback::new(move |id: String| id == confirming),
                    on_confirm: Callback::new(|_: String| {}),
                    on_cancel: Callback::new(|_: String| {}),
                };
                view! {
                    <DiscussionThread facts on_reply=|_: String| {} on_remove=|_: String| {} removal />
                }
                .to_html()
            })
        };
        let idle = render("none");
        assert!(
            idle.contains("id=\"remove-comment-root\"") && !idle.contains("confirm-remove"),
            "{idle}"
        );
        assert!(
            !idle.contains("remove-comment-other"),
            "only can_remove: {idle}"
        );
        let asking = render("root");
        assert!(
            !asking.contains("id=\"remove-comment-root\""),
            "the ask control is replaced in place: {asking}"
        );
        assert!(
            asking.contains("role=\"group\"")
                && asking.contains("aria-labelledby=\"remove-comment-root-prompt\"")
                && asking.contains("không thể khôi phục"),
            "{asking}"
        );
        let cancel = asking.find("id=\"cancel-remove-comment-root\"").unwrap();
        let confirm = asking.find("id=\"confirm-remove-comment-root\"").unwrap();
        assert!(cancel < confirm, "Giữ lại comes first: {asking}");
        assert!(asking.contains("vk-ui-button--danger"), "{asking}");
    }

    #[test]
    fn r4_send_emphasis_is_presentation_and_never_unlocks_a_blocked_send() {
        let render = |variant: ButtonVariant, state: ActionState| {
            Owner::new().with(|| {
                view! {
                <FeedbackComposer id="c" intent=FeedbackIntent::Unspecified value="" tonal=true
                    on_input=|_| {} state send_variant=Signal::stored(variant) on_submit=|_| {} />
            }.to_html()
            })
        };
        let empty = render(
            ButtonVariant::Tonal,
            ActionState::disabled("Viết nhận xét trước khi gửi."),
        );
        assert!(
            empty.contains("vk-ui-button--tonal") && empty.contains("aria-disabled=\"true\""),
            "{empty}"
        );
        let typed = render(ButtonVariant::Filled, ActionState::Enabled);
        assert!(typed.contains("vk-ui-button--filled"), "{typed}");
    }

    #[test]
    fn thread_endorsements_render_one_action_per_target_in_provider_order() {
        let comment = |id: &str| DiscussionComment {
            id: id.into(),
            author: Author {
                name: "Người đọc".into(),
                key: None,
                profile: None,
                published_at: "2026-11-06T02:00:00Z".into(),
                neutral: false,
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
    #[test]
    fn endorsement_forwards_surface_without_selecting_an_unknown_value() {
        let html = Owner::new().with(|| {
            view! {
                <EndorsementAction id="tonal-endorse" kind=EndorsementKind::Work
                    surface=vink_ui::ActionSurface::Tonal facts=EndorsementFacts::default()
                    on_set=|_: bool| {} on_recover=|_: RecoveryIntent| {} />
            }
            .to_html()
        });
        assert!(
            html.contains("data-surface=\"tonal\"") && html.contains("data-pressed=\"unknown\""),
            "{html}"
        );
        assert!(
            html.contains("aria-label=\"Thấm\"")
                && html.contains("pen-flight")
                && html.contains("Kiểm tra lại"),
            "{html}"
        );
    }

    #[test]
    fn format_vi_count_uses_vietnamese_dot_grouping() {
        assert_eq!(format_vi_count(0), "0");
        assert_eq!(format_vi_count(42), "42");
        assert_eq!(format_vi_count(999), "999");
        assert_eq!(format_vi_count(1_000), "1.000");
        assert_eq!(format_vi_count(1_234), "1.234");
        assert_eq!(format_vi_count(12_345), "12.345");
        assert_eq!(format_vi_count(123_456_789), "123.456.789");
    }

    #[test]
    fn author_work_row_enforces_d01_order_d02_no_read_and_all_view_states() {
        let base = |public: Option<AuthorWorkPublic>, views: AuthorWorkViews| AuthorWorkSummary {
            title: "Mưa trên phố cũ".into(),
            untitled: false,
            updated_at: "2026-11-25T10:00:00Z".into(),
            editor_href: Some("/ban-viet/w1".into()),
            status: vec![
                AuthorWorkLabel::new(
                    "Đã đăng",
                    "Đã đăng",
                    IconName::Check,
                    AuthorWorkLabelTone::Ok,
                ),
                AuthorWorkLabel::new(
                    "Bản mới chờ duyệt",
                    "Bản mới chờ duyệt",
                    IconName::Schedule,
                    AuthorWorkLabelTone::Attn,
                ),
                AuthorWorkLabel::new(
                    "Có bản sửa",
                    "Đã đăng · Có bản sửa",
                    IconName::Edit,
                    AuthorWorkLabelTone::Quiet,
                ),
            ],
            public,
            views,
        };
        let render = |summary: AuthorWorkSummary| {
            Owner::new().with(|| view! { <AuthorWorkRow work=Signal::stored(summary) /> }.to_html())
        };

        let public_counted = render(base(
            Some(AuthorWorkPublic {
                title: None,
                tham: Some(1_004),
                comments: Some(25),
            }),
            AuthorWorkViews::Counted {
                count: 12_345,
                counted_since: Some("01/12/2026".into()),
            },
        ));
        let idx_tham = public_counted.find("data-icon=\"pen-flight\"").unwrap();
        let idx_comments = public_counted
            .find("data-icon=\"chat-bubble-outline\"")
            .unwrap();
        let idx_views = public_counted
            .find("data-icon=\"visibility-outline\"")
            .unwrap();
        assert!(
            idx_tham < idx_comments && idx_comments < idx_views,
            "D-01 order Thấm -> Nhận xét -> Lượt xem: {public_counted}"
        );
        assert!(
            !public_counted.contains("vk-author-work__reader")
                && !public_counted.contains("Đọc bản công khai"),
            "D-02 removes Read link completely: {public_counted}"
        );
        assert!(
            public_counted.contains("1.004")
                && public_counted.contains("12.345")
                && public_counted.contains("Tính từ 01/12/2026")
                && public_counted.contains("data-views-state=\"counted\"")
                && public_counted.contains("data-tone=\"ok\"")
                && public_counted.contains("data-tone=\"attn\"")
                && public_counted.contains("data-tone=\"quiet\""),
            "{public_counted}"
        );

        let private_historical = render(base(
            None,
            AuthorWorkViews::Counted {
                count: 3,
                counted_since: None,
            },
        ));
        assert!(
            private_historical.contains("vk-author-work__views--historical")
                && private_historical.contains("data-icon=\"visibility-outline\"")
                && !private_historical.contains("data-icon=\"pen-flight\"")
                && !private_historical.contains("data-icon=\"chat-bubble-outline\""),
            "OQ-1=A keeps historical views without Thấm/Nhận xét: {private_historical}"
        );

        let not_started = render(base(None, AuthorWorkViews::NotStarted));
        assert!(
            not_started.contains("data-views-state=\"not_started\"")
                && not_started.contains("Chưa bắt đầu thống kê"),
            "{not_started}"
        );

        let unavailable = render(base(None, AuthorWorkViews::Unavailable));
        assert!(
            unavailable.contains("data-views-state=\"unavailable\"")
                && unavailable.contains("Chưa tải được"),
            "{unavailable}"
        );

        let never_public = render(base(None, AuthorWorkViews::NeverPublic));
        assert!(
            !never_public.contains("data-testid=\"author-work-metrics\"")
                && never_public.contains("data-testid=\"author-work-views-never-public\""),
            "{never_public}"
        );
    }
}
