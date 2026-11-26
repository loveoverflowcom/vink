//! App-owned Empty State composition; callers keep read/create/focus and announcement ownership.
//!
//! Placement (#558): a full-size state centres itself in the free block space of its owner
//! (Desk index, Explore, or a page marked `.vk-state-page`, optionally through one
//! `.vk-state-slot`). A `compact` state belongs to a region that stays in normal flow
//! (discussion, a dialog, a profile list) and never turns `.vk-main` into a flex column.
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyArtwork {
    NoResults,
    NoPublicWorks,
    /// Desk private first-use only (#259); never a claim about public discovery.
    FirstDraft,
}

impl EmptyArtwork {
    pub const fn src(self) -> &'static str {
        match self {
            Self::NoResults => "/pkg/empty/vink-empty-no-results.webp",
            Self::NoPublicWorks => "/pkg/empty/vink-empty-no-public-works.webp",
            Self::FirstDraft => "/pkg/empty/vink-empty-first-draft.webp",
        }
    }
}

/// One decoration (artwork, icon or none), an HTML title/message and real caller-owned actions.
/// `announce` makes the text group (title + message, never the actions) one polite status;
/// opt in only when the caller has no other result announcement.
#[component]
pub fn EmptyState(
    #[prop(optional)] artwork: Option<EmptyArtwork>,
    #[prop(optional)] icon: Option<vink_ui::IconName>,
    #[prop(default = vink_ui::HeadingLevel::H3)] heading: vink_ui::HeadingLevel,
    #[prop(optional)] title: Option<&'static str>,
    #[prop(optional, into)] message: Option<String>,
    #[prop(optional)] announce: bool,
    /// Region-local state in normal flow: smaller decoration and rhythm, no artwork.
    #[prop(optional)]
    compact: bool,
    /// Stable caller hook (test id or legacy selector); never a style override.
    #[prop(optional)]
    id: Option<&'static str>,
    #[prop(optional)] hook: Option<&'static str>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    debug_assert!(
        !(compact && artwork.is_some()),
        "compact Empty States carry an icon or no decoration"
    );
    let class = match (compact, hook) {
        (false, None) => "vk-empty-state".to_owned(),
        (true, None) => "vk-empty-state vk-empty-state--compact".to_owned(),
        (false, Some(hook)) => format!("vk-empty-state {hook}"),
        (true, Some(hook)) => format!("vk-empty-state vk-empty-state--compact {hook}"),
    };
    view! {
        <div class=class id=id>
            {artwork.map(|artwork| view! {
                <img class="vk-empty-state__art" src=crate::asset_url(artwork.src()) alt="" width="512" height="427" decoding="async" />
            })}
            {icon.map(|name| view! {
                <span class="vk-empty-state__icon"><vink_ui::Icon name /></span>
            })}
            <div class="vk-empty-state__text" role=announce.then_some("status") aria-atomic=announce.then_some("true")>
                {title.map(|title| view! { <vink_ui::Heading class:vk-empty-state__title=true
                    level=heading role=vink_ui::TypeRole::Title>{title}</vink_ui::Heading> })}
                {message.map(|message| view! { <p class="vk-empty-state__message">{message}</p> })}
            </div>
            {children.map(|children| view! { <div class="vk-empty-state__actions">{children()}</div> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(view: impl FnOnce() -> AnyView) -> String {
        Owner::new().with(|| view().to_html())
    }

    #[test]
    fn announce_covers_title_and_message_but_not_actions() {
        let out = html(|| {
            view! {
                <EmptyState title="Chưa có" message="Thử lại sau." announce=true>
                    <a href="/">"Về Khám phá"</a>
                </EmptyState>
            }
            .into_any()
        });
        let status = out.find("role=\"status\"").expect("status");
        let actions = out.find("vk-empty-state__actions").expect("actions");
        assert!(
            status < out.find("Chưa có").unwrap() && out.find("Thử lại sau.").unwrap() < actions,
            "{out}"
        );
        assert_eq!(out.matches("role=\"status\"").count(), 1, "{out}");
    }

    #[test]
    fn compact_keeps_hooks_and_optional_message() {
        let out = html(|| {
            view! { <EmptyState compact=true id="relationship-empty" hook="vk-rel-empty"
            icon=vink_ui::IconName::PersonOutline message="Chưa có người theo dõi." /> }
            .into_any()
        });
        assert!(
            out.contains("class=\"vk-empty-state vk-empty-state--compact vk-rel-empty\""),
            "{out}"
        );
        assert!(out.contains("id=\"relationship-empty\""), "{out}");
        assert!(!out.contains("role=\"status\""), "{out}");
        let titled = html(|| {
            view! { <EmptyState heading=vink_ui::HeadingLevel::H1 title="Nội dung không còn khả dụng" /> }.into_any()
        });
        assert!(
            titled.contains("<h1") && !titled.contains("vk-empty-state__message"),
            "{titled}"
        );
    }
}
