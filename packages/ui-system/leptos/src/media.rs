//! Identity and media: Icon, Avatar, AvatarGroup, AuthorByline, ImageFrame. Components receive
//! resolved facts — a display name, a stable key, an image state — and never look up a profile,
//! an email or an authorised image. Loading, network and authorisation belong to an adapter.

use leptos::prelude::*;

use crate::icons::{IconName, VIEW_BOX};
use crate::identity::{initials, tone};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconSize {
    /// `size.icon-sm` (20).
    Small,
    /// `size.icon` (24).
    #[default]
    Medium,
}

/// A registry icon drawn in `currentColor`. Without `label` it is decorative (hidden from
/// assistive technology), so a labelled control never announces its icon twice.
#[component]
pub fn Icon(
    name: IconName,
    #[prop(optional, into)] label: Option<String>,
    #[prop(optional)] size: IconSize,
) -> impl IntoView {
    let class = match size {
        IconSize::Small => "vk-ui-icon vk-ui-icon--sm",
        IconSize::Medium => "vk-ui-icon",
    };
    let decorative = label.is_none();
    view! {
        <svg
            class=class
            viewBox=VIEW_BOX
            data-icon=name.id()
            aria-hidden=decorative.then_some("true")
            role=(!decorative).then_some("img")
            aria-label=label
            focusable="false"
        >
            <path d=name.path() fill="currentColor"></path>
        </svg>
    }
}

/// An image the adapter has resolved. Only `Ready` with an accepted `src` shows an image.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageState {
    #[default]
    Absent,
    Loading,
    Ready {
        src: String,
    },
    Failed,
}

impl ImageState {
    pub fn ready(src: impl Into<String>) -> Self {
        Self::Ready { src: src.into() }
    }
}

const IMAGE_DATA: [&str; 5] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
    "data:image/avif;base64,",
    "data:image/gif;base64,",
];

/// `src` when it is https/http, root-relative (`/…`, not `//host`), a `blob:` the adapter made,
/// or a base64 raster `data:` URL; `None` (treated as failed) otherwise.
pub fn safe_image_src(src: &str) -> Option<&str> {
    let src = src.trim();
    if src.is_empty() || src.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = src.to_ascii_lowercase();
    let accepted = (src.starts_with('/') && !src.starts_with("//"))
        || lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("blob:")
        || IMAGE_DATA.iter().any(|prefix| lower.starts_with(prefix));
    accepted.then_some(src)
}

/// What a frame or avatar shows for a state, given the `src` that already failed to load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    Image(String),
    Fallback(&'static str),
}

pub fn shown(state: &ImageState, failed: Option<&str>) -> Shown {
    match state {
        ImageState::Ready { src } => match safe_image_src(src) {
            Some(src) if failed != Some(src) => Shown::Image(src.to_string()),
            _ => Shown::Fallback("failed"),
        },
        ImageState::Absent => Shown::Fallback("absent"),
        ImageState::Loading => Shown::Fallback("loading"),
        ImageState::Failed => Shown::Fallback("failed"),
    }
}

/// An `<img>` that reports a load/decode failure (including an unsupported AVIF) once, also when
/// the failure happened before hydration attached the handler.
fn image(
    src: String,
    alt: String,
    class: &'static str,
    failed: RwSignal<Option<String>>,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Img>::new();
    let failed_src = src.clone();
    Effect::new(move |_| {
        if let Some(img) = node.get()
            && img.complete()
            && img.natural_width() == 0
        {
            failed.set(Some(img.get_attribute("src").unwrap_or_default()));
        }
    });
    view! {
        <img
            node_ref=node
            class=class
            src=src
            alt=alt
            decoding="async"
            on:error=move |_| failed.set(Some(failed_src.clone()))
        />
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AvatarSize {
    /// `size.avatar-sm` (32): groups and dense lists.
    Small,
    /// `size.avatar-md` (40): rail/account.
    #[default]
    Medium,
    /// Feed author: 44 wide / 40 below 760px.
    Feed,
    /// Spotlight author: 40 / 38.
    Spotlight,
    /// Reader header author: 50 / 44.
    Reader,
}

impl AvatarSize {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Small => "sm",
            Self::Medium => "md",
            Self::Feed => "feed",
            Self::Spotlight => "spotlight",
            Self::Reader => "reader",
        }
    }
}

/// Author and account avatars are different identities; one never stands in for the other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AvatarKind {
    #[default]
    Author,
    Account,
}

/// A circular avatar: an allowed, loaded image over the initials, the initials over a person
/// mark. Its tone comes from `identity_key` only. `decorative` hides it when a visible name
/// sits next to it (byline), so the name is not read twice.
#[component]
pub fn Avatar(
    #[prop(optional, into)] name: Option<String>,
    #[prop(optional, into)] identity_key: Option<String>,
    #[prop(optional, into)] image: Signal<ImageState>,
    #[prop(optional)] size: AvatarSize,
    #[prop(optional)] kind: AvatarKind,
    #[prop(optional)] decorative: bool,
) -> impl IntoView {
    let letters = initials(name.as_deref());
    let tone = tone(identity_key.as_deref());
    let palette = tone.map_or("neutral".to_string(), |t| t.to_string());
    let style = format!(
        "--vk-ui-avatar-bg: var(--vk-avatar-{palette}); --vk-ui-avatar-fg: var(--vk-on-avatar-{palette});"
    );
    let label = (!decorative).then(|| {
        name.clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Chưa có tên hiển thị".into())
    });
    let failed = RwSignal::new(None::<String>);
    let data_initials = letters.clone().unwrap_or_default();
    view! {
        <span
            class=format!("vk-ui-avatar vk-ui-avatar--{}", size.id())
            style=style
            data-tone=palette
            data-kind=match kind {
                AvatarKind::Author => "author",
                AvatarKind::Account => "account",
            }
            data-initials=data_initials
            role=label.is_some().then_some("img")
            aria-label=label
            aria-hidden=decorative.then_some("true")
        >
            <span class="vk-ui-avatar__initials" aria-hidden="true">
                {match letters {
                    Some(letters) => letters.into_any(),
                    None => view! { <Icon name=IconName::Person /> }.into_any(),
                }}
            </span>
            {move || match shown(&image.get(), failed.get().as_deref()) {
                Shown::Image(src) => Some(image_view(src, failed)),
                Shown::Fallback(_) => None,
            }}
        </span>
    }
}

fn image_view(src: String, failed: RwSignal<Option<String>>) -> AnyView {
    image(src, String::new(), "vk-ui-avatar__image", failed).into_any()
}

/// A person in a group: shown in the caller's order, coloured by `key`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Person {
    pub key: Option<String>,
    pub name: Option<String>,
    pub image: ImageState,
}

/// "+N" only when the total is known and larger than what is shown. An unknown total (`None`)
/// or a total smaller than the list (caller error) shows no count — never "+0" or a guess.
pub fn overflow(shown: usize, total: Option<usize>) -> Option<usize> {
    total.filter(|t| *t > shown).map(|t| t - shown)
}

/// The one accessible name of a group.
pub fn group_summary(
    label: &str,
    people: &[Person],
    max_visible: usize,
    total: Option<usize>,
) -> String {
    let names: Vec<String> = people
        .iter()
        .take(max_visible)
        .map(|p| {
            p.name
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| "một người chưa có tên".into())
        })
        .collect();
    let shown = names.len();
    match (names.is_empty(), overflow(shown, total)) {
        (true, None) if total == Some(0) => format!("{label}: chưa có ai"),
        (true, None) => format!("{label}: chưa rõ"),
        (_, Some(more)) => format!("{label}: {} và {more} người khác", names.join(", ")),
        (false, None) => format!("{label}: {}", names.join(", ")),
    }
}

/// Up to `max_visible` decorative avatars and a group name; no focus stops inside.
#[component]
pub fn AvatarGroup(
    people: Vec<Person>,
    #[prop(into)] label: String,
    #[prop(default = 3)] max_visible: usize,
    #[prop(optional)] total: Option<usize>,
    #[prop(default = AvatarSize::Small)] size: AvatarSize,
) -> impl IntoView {
    let summary = group_summary(&label, &people, max_visible, total);
    let visible: Vec<Person> = people.into_iter().take(max_visible).collect();
    let more = overflow(visible.len(), total);
    view! {
        <div class="vk-ui-avatar-group" role="img" aria-label=summary>
            {visible
                .into_iter()
                .map(|p| view! {
                    <Avatar
                        name=p.name.unwrap_or_default()
                        identity_key=p.key.unwrap_or_default()
                        image=Signal::stored(p.image)
                        size=size
                        decorative=true
                    />
                })
                .collect_view()}
            {more.map(|n| view! {
                <span class=format!("vk-ui-avatar-group__more vk-ui-avatar--{}", size.id()) aria-hidden="true">
                    {format!("+{n}")}
                </span>
            })}
        </div>
    }
}

/// Author name (bold, a link only for a verified `href`) before muted metadata; the avatar is
/// decorative so the name is read once.
#[component]
pub fn AuthorByline(
    #[prop(into)] name: String,
    #[prop(optional, into)] identity_key: Option<String>,
    #[prop(optional, into)] image: Signal<ImageState>,
    /// A profile URL the caller has verified; absent means plain text, never an invented route.
    #[prop(optional, into)]
    href: Option<String>,
    #[prop(optional)] meta: Vec<String>,
    #[prop(default = AvatarSize::Feed)] size: AvatarSize,
) -> impl IntoView {
    let blank = name.trim().is_empty();
    let shown_name = if blank {
        "Chưa có tên tác giả".to_string()
    } else {
        name.clone()
    };
    let href = href.and_then(|h| crate::action::safe_href(&h).map(str::to_string));
    let meta = meta
        .into_iter()
        .filter(|m| !m.trim().is_empty())
        .collect::<Vec<_>>();
    view! {
        <div class=format!("vk-ui-byline vk-ui-byline--{}", size.id())>
            <Avatar
                name=if blank { String::new() } else { name }
                identity_key=identity_key.unwrap_or_default()
                image=image
                size=size
                decorative=true
            />
            <div class="vk-ui-byline__text">
                {match href {
                    Some(href) => view! { <a class="vk-ui-byline__name vk-ui-link" href=href>{shown_name}</a> }.into_any(),
                    None => view! { <span class="vk-ui-byline__name" data-blank=blank.then_some("true")>{shown_name}</span> }.into_any(),
                }}
                {(!meta.is_empty()).then(|| view! { <span class="vk-ui-byline__meta">{meta.join(" · ")}</span> })}
            </div>
        </div>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AspectRatio {
    Square,
    #[default]
    Landscape,
    Wide,
}

impl AspectRatio {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Landscape => "landscape",
            Self::Wide => "wide",
        }
    }
}

/// A fixed-ratio image. Absent, loading and failed images show a quiet neutral fallback with
/// text (no spinner; UIS-03 owns loading indicators). An empty `alt` marks the image decorative.
#[component]
pub fn ImageFrame(
    #[prop(into)] image: Signal<ImageState>,
    #[prop(into)] alt: String,
    #[prop(optional)] ratio: AspectRatio,
) -> impl IntoView {
    let failed = RwSignal::new(None::<String>);
    let decorative = alt.trim().is_empty();
    view! {
        <div class=format!("vk-ui-frame vk-ui-frame--{}", ratio.id())>
            {move || {
                let alt = alt.clone();
                match shown(&image.get(), failed.get().as_deref()) {
                    Shown::Image(src) => image_frame_img(src, alt, failed),
                    Shown::Fallback(state) => {
                        let text = match state {
                            "failed" => "Không tải được ảnh",
                            "loading" => "Đang tải ảnh",
                            _ => "Chưa có ảnh",
                        };
                        let label = (!decorative).then(|| format!("{alt} — {text}"));
                        view! {
                            <div
                                class="vk-ui-frame__fallback"
                                data-state=state
                                role=label.is_some().then_some("img")
                                aria-label=label
                                aria-hidden=decorative.then_some("true")
                            >
                                <Icon name=IconName::Image />
                                <span aria-hidden="true">{text}</span>
                            </div>
                        }
                        .into_any()
                    }
                }
            }}
        </div>
    }
}

fn image_frame_img(src: String, alt: String, failed: RwSignal<Option<String>>) -> AnyView {
    image(src, alt, "vk-ui-frame__image", failed).into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;

    #[test]
    fn image_sources_are_allow_listed() {
        for ok in [
            "/pkg/a.avif",
            "https://img.vink.vn/a.webp",
            "data:image/avif;base64,AAAA",
            "blob:https://vink.vn/1",
        ] {
            assert_eq!(safe_image_src(ok), Some(ok), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "//evil/a.png",
            "data:image/svg+xml;base64,PHN2Zz4=",
            "data:text/html,x",
            "a.png\" onerror=\"x",
            "",
        ] {
            assert_eq!(safe_image_src(bad), None, "{bad:?}");
        }
        let ready = ImageState::ready("/a.avif");
        assert_eq!(shown(&ready, None), Shown::Image("/a.avif".into()));
        assert_eq!(shown(&ready, Some("/a.avif")), Shown::Fallback("failed"));
        assert_eq!(
            shown(&ImageState::ready("javascript:x"), None),
            Shown::Fallback("failed")
        );
    }

    #[test]
    fn avatars_show_initials_tone_and_role() {
        let html = render(|| {
            view! { <Avatar name="An Nhiên" identity_key="author:an-nhien" size=AvatarSize::Feed /> }
                .into_any()
        });
        for needle in [
            ">AN<",
            "data-tone=\"1\"",
            "var(--vk-avatar-1)",
            "role=\"img\"",
            "aria-label=\"An Nhiên\"",
            "vk-ui-avatar--feed",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        let html = render(|| view! { <Avatar decorative=true /> }.into_any());
        assert!(html.contains("data-tone=\"neutral\""), "{html}");
        assert!(html.contains("data-icon=\"person\""), "{html}");
        assert!(!html.contains("role=\"img\""), "{html}");
    }

    #[test]
    fn groups_never_invent_counts() {
        let people: Vec<Person> = ["An Nhiên", "Trần Linh", "Đỗ Ưng", "Nguyễn"]
            .into_iter()
            .map(|n| Person {
                key: Some(n.into()),
                name: Some(n.into()),
                image: ImageState::Absent,
            })
            .collect();
        assert_eq!(overflow(3, Some(7)), Some(4));
        assert_eq!(overflow(3, None), None);
        assert_eq!(overflow(3, Some(2)), None);
        assert_eq!(overflow(3, Some(3)), None);
        assert_eq!(
            group_summary("Người nhận xét", &people, 3, Some(7)),
            "Người nhận xét: An Nhiên, Trần Linh, Đỗ Ưng và 4 người khác"
        );
        assert_eq!(
            group_summary("Người nhận xét", &people, 3, None),
            "Người nhận xét: An Nhiên, Trần Linh, Đỗ Ưng"
        );
        assert_eq!(
            group_summary("Người nhận xét", &[], 3, None),
            "Người nhận xét: chưa rõ"
        );
        let html = render(move || {
            view! { <AvatarGroup people=people.clone() label="Người nhận xét" /> }.into_any()
        });
        assert!(!html.contains('+'), "unknown total shows no count: {html}");
        assert_eq!(html.matches("aria-label").count(), 1, "{html}");
    }

    #[test]
    fn bylines_read_the_name_once_and_link_only_verified_profiles() {
        let html = render(|| {
            view! {
                <AuthorByline
                    name="Trần Linh"
                    identity_key="author:tran-linh"
                    meta=vec!["02/11/2026".to_string(), "Thơ".to_string()]
                />
            }
            .into_any()
        });
        assert_eq!(html.matches("Trần Linh").count(), 1, "{html}");
        assert!(!html.contains("href"), "{html}");
        assert!(html.contains("02/11/2026 · Thơ"), "{html}");
        let html =
            render(|| view! { <AuthorByline name="Trần Linh" href="javascript:x" /> }.into_any());
        assert!(!html.contains("href"), "{html}");
    }
}
