//! App-owned Empty State composition; callers keep read/create/focus and announcement ownership.
//!
//! Placement (#558): a full-size state centres itself in the free block space of its owner
//! (Desk index, Explore, or a page marked `.vk-state-page`, optionally through one
//! `.vk-state-slot`). A `compact` state belongs to a region that stays in normal flow
//! (discussion, a dialog, a profile list) and never turns `.vk-main` into a flex column.
//! A `region` state (#587) also stays in its region's normal flow, but keeps the full-size
//! artwork → sentence group (the public profile's true-empty works region).
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

// Native Tab can leave an already-visible link behind the fixed bottom navigation.
// Scroll only a focused recovery control that actually intersects that navigation.
fn reveal_recovery(_event: leptos::ev::FocusEvent) {
    #[cfg(feature = "browser")]
    {
        let control = event_target::<leptos::web_sys::Element>(&_event);
        request_animation_frame(move || {
            if document().active_element().as_ref() != Some(&control) {
                return;
            }
            if let Some(bar) = document().get_element_by_id("bottom-navigation") {
                let bounds = bar.get_bounding_client_rect();
                let overlap = control.get_bounding_client_rect().bottom() - bounds.top();
                if bounds.height() > 0.0 && overlap > 0.0 {
                    // Leave room for the focus outline on the scrollable page.
                    window().scroll_by_with_x_and_y(0.0, overlap.ceil() + 8.0);
                }
            }
        });
    }
}

/// One decoration (artwork, decorative mark, icon or none), an HTML title/message and real caller-owned actions.
/// `announce` makes the text group (title + message, never the actions) one polite status;
/// opt in only when the caller has no other result announcement.
#[component]
pub fn EmptyState(
    #[prop(optional)] artwork: Option<EmptyArtwork>,
    #[prop(optional)] icon: Option<vink_ui::IconName>,
    #[prop(optional)] mark: Option<Children>,
    #[prop(default = vink_ui::HeadingLevel::H3)] heading: vink_ui::HeadingLevel,
    #[prop(optional)] title: Option<&'static str>,
    /// A named page heading can also receive focus from the route's navigation owner.
    #[prop(optional)]
    title_id: Option<&'static str>,
    #[prop(optional, into)] message: String,
    #[prop(optional)] announce: bool,
    /// Announce a failed read immediately, keeping retry outside the live region.
    #[prop(optional)]
    alert: bool,
    /// Region-local state in normal flow: smaller decoration and rhythm, no artwork.
    #[prop(optional)]
    compact: bool,
    /// Region-local state in normal flow that may carry artwork with its own breathing room;
    /// excluded from the page-centring chain like `compact` (#587). Never both.
    #[prop(optional)]
    region: bool,
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
    debug_assert!(
        !(compact && region),
        "a region Empty State is the in-flow variant with artwork, not a compact one"
    );
    let placement = if compact {
        " vk-empty-state--compact"
    } else if region {
        " vk-empty-state--region"
    } else {
        ""
    };
    let class = match hook {
        None => format!("vk-empty-state{placement}"),
        Some(hook) => format!("vk-empty-state{placement} {hook}"),
    };
    let decoration = if let Some(artwork) = artwork {
        Some(view! {
            <img class="vk-empty-state__art" src=crate::asset_url(artwork.src()) alt="" width="512" height="427" decoding="async" />
        }.into_any())
    } else if let Some(mark) = mark {
        Some(mark())
    } else {
        icon.map(|name| {
            view! {
                <span class="vk-empty-state__icon"><vink_ui::Icon name /></span>
            }
            .into_any()
        })
    };
    view! {
        <div class=class id=id>
            {decoration}
            <div class="vk-empty-state__text" role=if alert { Some("alert") } else { announce.then_some("status") } aria-atomic=(announce || alert).then_some("true")>
                {title.map(|title| view! { <vink_ui::Heading class:vk-empty-state__title=true
                    level=heading role=vink_ui::TypeRole::Title attr:id=title_id attr:tabindex=title_id.map(|_| "-1")>{title}</vink_ui::Heading> })}
                {(!message.is_empty()).then(|| view! { <p class="vk-empty-state__message">{message}</p> })}
            </div>
            {children.map(|children| view! { <div class="vk-empty-state__actions" on:focusin=reveal_recovery>{children()}</div> })}
        </div>
    }
}

/// A failed read with a caller-owned retry. Retained data stays next to an inline notice.
#[component]
pub(crate) fn ReadFailure(
    message: &'static str,
    retry: Callback<()>,
    #[prop(optional)] inline: bool,
    #[prop(optional)] compact: bool,
    #[prop(optional, into)] state: Signal<vink_ui::ActionState>,
) -> AnyView {
    use vink_ui::{Button, ButtonVariant, HeadingLevel, IconName};
    if inline {
        return view! {
            <div class="vk-read-failure vk-read-failure-inline">
                <p role="alert">{message}</p>
                <Button variant=ButtonVariant::Text state on_press=move |_| retry.run(())>"Thử lại"</Button>
            </div>
        }.into_any();
    }
    view! {
        <EmptyState compact hook="vk-read-failure" icon=IconName::Error heading=if compact { HeadingLevel::H3 } else { HeadingLevel::H2 }
            title=message alert=true>
            <Button variant=ButtonVariant::Tonal state on_press=move |_| retry.run(())>"Thử lại"</Button>
        </EmptyState>
    }.into_any()
}

/// A recovery destination, independent of the browser history stack.
#[component]
pub(crate) fn ExploreRecovery(#[prop(default = "/".to_owned())] href: String) -> impl IntoView {
    view! {
        <a class=vink_ui::ButtonVariant::Tonal.classes() href=href>
            <vink_ui::Icon name=vink_ui::IconName::Explore />"Về Khám phá"
        </a>
    }
}

/// Shared presentation. The route keeps ownership of status, robots and cache headers.
#[component]
pub(crate) fn NotFoundContent(
    #[prop(default = "/".to_string())] return_href: String,
    #[prop(default = "not-found-title")] heading_id: &'static str,
) -> impl IntoView {
    view! {
        <leptos_meta::Title text="Không tìm thấy — Vink" />
        <section class="vk-state-page" data-panel="error:not-found" aria-labelledby=heading_id>
            <EmptyState icon=vink_ui::IconName::Search heading=vink_ui::HeadingLevel::H1 title="Không tìm thấy trang này."
                title_id=heading_id message="Hãy kiểm tra lại đường dẫn.">
                <ExploreRecovery href=return_href />
            </EmptyState>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(view: impl FnOnce() -> AnyView) -> String {
        Owner::new().with(|| view().to_html())
    }

    #[test]
    fn not_found_keeps_one_native_destination_with_a_leading_decorative_compass() {
        let out =
            html(|| view! { <NotFoundContent return_href="/?q=tho".to_owned() /> }.into_any());
        let actions = out.split_once("vk-empty-state__actions").unwrap().1;
        let (link, _) = actions.split_once("</a>").unwrap();
        assert_eq!(actions.matches("<a ").count(), 1);
        assert!(link.contains("href=\"/?q=tho\""));
        let (_, glyph) = link.split_once("<svg").unwrap();
        let (svg, label) = glyph.split_once("</svg>").unwrap();
        assert!(svg.contains("data-icon=\"explore\"") && svg.contains("aria-hidden=\"true\""));
        assert!(label.contains("Về Khám phá"));
        assert!(out.contains("<h1") && out.contains("vk-ui-type-title"));
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

    #[test]
    fn region_keeps_artwork_in_flow_without_compact_or_page_placement() {
        let out = html(|| {
            view! { <EmptyState region=true artwork=EmptyArtwork::NoPublicWorks
            hook="vk-author-works-empty" message="Chưa có tác phẩm công khai." /> }
            .into_any()
        });
        assert!(
            out.contains("class=\"vk-empty-state vk-empty-state--region vk-author-works-empty\""),
            "{out}"
        );
        assert!(!out.contains("vk-empty-state--compact"), "{out}");
        assert!(
            out.contains("class=\"vk-empty-state__art\"")
                && out.contains("vink-empty-no-public-works.webp")
                && out.contains("alt=\"\""),
            "{out}"
        );
        let art = out.find("vk-empty-state__art").unwrap();
        assert!(
            art < out.find("Chưa có tác phẩm công khai.").unwrap(),
            "{out}"
        );
        assert!(
            !out.contains("role=\"status\"") && !out.contains("<h"),
            "{out}"
        );
        let page = html(|| view! { <EmptyState message="x" /> }.into_any());
        assert!(page.contains("class=\"vk-empty-state\""), "{page}");
    }
}
