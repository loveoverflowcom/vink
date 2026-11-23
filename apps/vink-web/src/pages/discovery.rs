//! TAG-PR-06 page glue. Shared chips/bylines render facts; this module owns GET/URL/focus.
use super::{ResponseStatus, Unavailable, drop_unnamed_submitter, focus_recovery};
use crate::api::{self, LoadError};
use crate::contract::{PublicTagDto, TagDto, TagList, WorkCard, WorkList};
use crate::discovery::{
    ExploreContext, ExploreMode, ExploreQuery, author_search_href, requested_slug, tag_href,
};
use crate::public_reads;
use crate::ui::{
    adapters,
    empty_state::{EmptyArtwork, EmptyState},
    patterns::{AuthorIdentity, TagChip, TagFacts},
};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::Form;
use leptos_router::hooks::{use_params_map, use_query_map};
use std::sync::Arc;
use vink_ui::{
    ActionState, ActionSurface, Button, ButtonKind, ButtonVariant, Checkbox, HiddenField,
};
use vink_ui::{
    Dialog, DialogKind, Heading, HeadingLevel, Icon, IconButton, IconName, IconSize, InlineNotice,
    RecoveryAction, RecoveryIntent, SearchField, SearchVariant, Tone, TypeRole,
};

mod feed;

fn query_from(params: &leptos_router::params::ParamsMap) -> ExploreQuery {
    ExploreQuery::new(
        params.get("q").unwrap_or_default(),
        params.get_all("tag").unwrap_or_default(),
        params.get("cursor"),
    )
}

/// A submit owns one focus intent, not subsequent typing or an async read completion.
fn focus_submitted_results(data: &leptos::web_sys::FormData) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        let path = window().location().pathname().ok();
        let query = data.get("q").as_string().unwrap_or_default();
        let origin = document().active_element();
        let picker_submit = origin
            .as_ref()
            .is_some_and(|element| element.closest("#tag-picker").ok().flatten().is_some());
        let Some(input) = document()
            .get_element_by_id("public-query")
            .and_then(|input| input.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let caret = (input.selection_start().ok(), input.selection_end().ok());
        request_animation_frame(move || {
            let Some(input) = document()
                .get_element_by_id("public-query")
                .and_then(|input| input.dyn_into::<web_sys::HtmlInputElement>().ok())
            else {
                return;
            };
            let active = document().active_element();
            let moved_focus = active.as_ref().is_some_and(|element| {
                Some(element) != origin.as_ref()
                    && element.tag_name() != "BODY"
                    && element.id() != "community-stream"
                    // Closing this submitted picker restores its trigger before our frame.
                    && !(picker_submit && element.id() == "open-tag-filters")
            });
            let moved_caret = origin
                .as_ref()
                .is_some_and(|element| element == input.unchecked_ref::<web_sys::Element>())
                && caret != (input.selection_start().ok(), input.selection_end().ok());
            if window().location().pathname().ok() == path
                && input.value().trim() == query.trim()
                && !moved_focus
                && !moved_caret
                && let Some(target) = document().get_element_by_id("community-stream")
                && let Ok(target) = target.dyn_into::<web_sys::HtmlElement>()
            {
                let _ = target.focus();
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = data;
}

fn mode_from(params: &leptos_router::params::ParamsMap) -> ExploreMode {
    ExploreMode::from_param(params.get("mode").as_deref())
}

pub(super) fn reader_context() -> Memo<ExploreContext> {
    let params = use_query_map();
    let feed_enabled = api::following_feed_enabled();
    Memo::new(move |_| {
        let params = params.read();
        let mode = if feed_enabled && mode_from(&params) == ExploreMode::Following {
            ExploreMode::Following
        } else {
            ExploreMode::Works
        };
        ExploreContext {
            mode,
            query: if mode == ExploreMode::Works {
                query_from(&params)
            } else {
                ExploreQuery::default()
            },
            tag_page: params.get("from_tag").filter(|s| !s.is_empty()),
        }
    })
}

#[cfg(feature = "hydrate")]
fn is_narrow_viewport() -> bool {
    window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .is_some_and(|width| width < 760.0)
}

#[cfg(feature = "hydrate")]
fn focus_no_scroll(id: &str) {
    use wasm_bindgen::JsCast;
    if let Some(target) = document().get_element_by_id(id)
        && let Ok(element) = target.dyn_into::<web_sys::HtmlElement>()
    {
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = element.focus_with_options(&options);
    }
}

/// Escape closes the in-place dock only from inside it; the phone dock owns the whole screen.
#[cfg(feature = "hydrate")]
fn in_dock(event: &leptos::ev::KeyboardEvent) -> bool {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .and_then(|element| element.closest(".vk-discovery-toolbar").ok().flatten())
        .is_some()
}

/// The focused element takes text, so “/” belongs to it.
#[cfg(feature = "hydrate")]
fn typing_target() -> bool {
    use wasm_bindgen::JsCast;
    document().active_element().is_some_and(|element| {
        matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
            || element
                .dyn_into::<web_sys::HtmlElement>()
                .is_ok_and(|element| element.is_content_editable())
    })
}

#[cfg(feature = "hydrate")]
fn any_modal_dialog_open() -> bool {
    document()
        .query_selector("dialog[open]")
        .ok()
        .flatten()
        .is_some()
}

#[cfg(feature = "hydrate")]
fn restore_browse(browse_scroll_y: RwSignal<Option<f64>>) {
    // Native traversal restores its scroll after popstate. Focus only the mounted browse view.
    request_animation_frame(move || {
        request_animation_frame(move || {
            let Some(y) = browse_scroll_y.try_get_untracked() else {
                return;
            };
            if document()
                .query_selector("#explore[data-search=closed]")
                .ok()
                .flatten()
                .is_none()
            {
                return;
            }
            if let Some(y) = y {
                browse_scroll_y.set(None);
                window().scroll_to_with_x_and_y(0.0, y);
            }
            focus_no_scroll("open-public-search");
        });
    });
}

#[component]
pub fn Home() -> impl IntoView {
    view! { <DiscoveryPage /> }
}

#[component]
pub fn TagDetail() -> impl IntoView {
    let params = use_params_map();
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());
    view! { <DiscoveryPage tag_page=slug /> }
}

#[component]
fn DiscoveryPage(#[prop(optional)] tag_page: Option<Memo<String>>) -> AnyView {
    ResponseStatus::capture().set_no_store();
    let params = use_query_map();
    let context = Memo::new(move |_| {
        let params = params.read();
        let tag_page = tag_page.map(|slug| slug.get());
        let mode = if tag_page.is_none() {
            mode_from(&params)
        } else {
            ExploreMode::Works
        };
        let mut query = match mode {
            ExploreMode::Works => query_from(&params),
            ExploreMode::Following => ExploreQuery::default(),
            ExploreMode::Authors => ExploreQuery::new(
                params.get("q").unwrap_or_default(),
                Vec::new(),
                params.get("cursor"),
            ),
        };
        if let Some(slug) = &tag_page {
            query.tags.push(slug.clone());
            query.tags.sort();
            query.tags.dedup();
        }
        ExploreContext {
            mode,
            query,
            tag_page,
        }
    });
    let dock_open = RwSignal::new(false);
    let browse_scroll_y = RwSignal::new(None::<f64>);
    #[cfg(feature = "hydrate")]
    if tag_page.is_none() {
        let guard = expect_context::<crate::navigation::NavigationGuard>();
        // The tag picker registers later and handles Back first. Let the router perform
        // real history traversal, including Forward into the empty search entry. The search
        // dock follows the URL at every width (R4.2 #404); only the full-screen phone dock
        // restores the browse position it covered.
        let registration = guard.set_overlay(move |reason| {
            if reason == vink_ui::CloseReason::Back
                && let Ok(href) = window().location().href()
                && let Ok(next) = url::Url::parse(&href)
            {
                let public = !next
                    .query_pairs()
                    .any(|(k, v)| k == "mode" && matches!(v.as_ref(), "following" | "authors"));
                let search = public
                    && (next.fragment() == Some("tim-kiem")
                        || next
                            .query_pairs()
                            .any(|(k, v)| k == "q" && !v.trim().is_empty()));
                let previous = context.get_untracked();
                let was_search = previous.mode == ExploreMode::Works
                    && (dock_open.get_untracked() || !previous.query.q.is_empty());
                dock_open.set(next.path() == "/" && search);
                if was_search && !search && next.path() == "/" && is_narrow_viewport() {
                    restore_browse(browse_scroll_y);
                }
            }
            false
        });
        on_cleanup(move || guard.clear_overlay(registration));
    }
    // A URL scope owns its resource before I/O starts. Replacing A with B drops A's
    // suspended view immediately; B never waits for A's outstanding transport.
    view! {
        <For each=move || vec![context.get()] key=|context| context.href(None)
            children=move |context| view! { <DiscoveryScope context dock_open browse_scroll_y /> } />
    }
    .into_any()
}

#[component]
fn DiscoveryScope(
    context: ExploreContext,
    dock_open: RwSignal<bool>,
    browse_scroll_y: RwSignal<Option<f64>>,
) -> AnyView {
    match context.mode {
        ExploreMode::Following => return view! { <feed::FollowingFeed context /> }.into_any(),
        ExploreMode::Authors => return view! {
            <super::author_discovery::AuthorSearch q=context.query.q cursor=context.query.cursor />
        }
        .into_any(),
        ExploreMode::Works => {}
    }
    let initial_query = context.query.clone();
    let query = Memo::new(move |_| initial_query.clone());
    let catalog = context.tag_page.is_none().then(api::catalog_resource);
    let works = api::works_resource(query);
    view! { <DiscoveryChrome context works catalog status=ResponseStatus::capture() dock_open browse_scroll_y /> }.into_any()
}

/// R4.2 header row of the Following and Authors modes (#404): the same page title role and
/// V-pen logo as Tất cả. Their own search/filter actions are out of this slice (#354, #355).
#[component]
pub(super) fn ExploreTitleRow(title: &'static str) -> impl IntoView {
    view! {
        <div class="vk-explore__top">
            <header id="explore-heading" class="vk-explore__heading">
                <h1 class="vk-ui-heading vk-ui-type-page-title">{title}</h1>
            </header>
            <div class="vk-discovery-toolbar"><ExploreBrand /></div>
        </div>
    }
}

#[component]
fn ExploreBrand() -> impl IntoView {
    view! {
        <div id="discovery-tools" class="vk-discovery-tools"><a class="vk-brand" href="/" aria-label="Vink — trang chủ">
            <span class="vk-brand__lockup" aria-hidden="true"></span></a></div>
    }
}

/// R4.2 mode group (#404, DQ-03): a connected group of real links; the current mode is primary
/// with a check mark and `aria-current`. When the group would not fit on one line, CSS shows the
/// same links in a `details` menu instead (works before hydration; Escape closes it after).
#[component]
pub(super) fn ExploreModes(
    mode: ExploreMode,
    #[prop(optional)] author_query: String,
) -> impl IntoView {
    let mut modes = vec![(ExploreMode::Works, "works", "/".to_owned(), "Tất cả")];
    if api::following_feed_enabled() {
        modes.push((
            ExploreMode::Following,
            "following",
            ExploreMode::FOLLOWING_HREF.to_owned(),
            "Đang theo dõi",
        ));
    }
    modes.push((
        ExploreMode::Authors,
        "authors",
        author_search_href(&author_query, None),
        "Tác giả",
    ));
    let current = modes
        .iter()
        .find(|(entry, ..)| *entry == mode)
        .map_or("Tất cả", |(.., label)| *label);
    let links = |prefix: &'static str| {
        modes
            .iter()
            .map(|(entry, key, href, label)| {
                let selected = *entry == mode;
                view! {
                    <a id=format!("{prefix}-{key}") href=href.clone() aria-current=selected.then_some("page")>
                        {selected.then(|| view! { <Icon name=IconName::Check size=IconSize::Small /> })}
                        <span>{*label}</span>
                    </a>
                }
            })
            .collect_view()
    };
    view! {
        <nav class="vk-explore-modes" aria-label="Chế độ Khám phá">
            <div class="vk-explore-modes__group">{links("explore-mode")}</div>
            <details id="explore-mode-menu" class="vk-explore-modes__menu"
                on:keydown=move |event: leptos::ev::KeyboardEvent| {
                    #[cfg(feature = "hydrate")]
                    {
                        use wasm_bindgen::JsCast;
                        if event.key() == "Escape"
                            && let Some(menu) = event.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                            && menu.has_attribute("open")
                        {
                            event.prevent_default();
                            event.stop_propagation();
                            let _ = menu.remove_attribute("open");
                            if let Ok(Some(summary)) = menu.query_selector("summary")
                                && let Ok(summary) = summary.dyn_into::<web_sys::HtmlElement>()
                            {
                                let _ = summary.focus();
                            }
                        }
                    }
                    #[cfg(not(feature = "hydrate"))]
                    let _ = event;
                }>
                <summary id="explore-mode-menu-button">
                    <span class="vk-ui-visually-hidden">"Chế độ: "</span>
                    <span>{current}</span>
                    <Icon name=IconName::ExpandMore size=IconSize::Small />
                </summary>
                <div class="vk-explore-modes__list">{links("explore-menu")}</div>
            </details>
        </nav>
    }
}

#[component]
fn DiscoveryChrome(
    context: ExploreContext,
    works: Resource<(ExploreQuery, Result<WorkList, LoadError>)>,
    catalog: Option<Resource<Result<TagList, LoadError>>>,
    status: ResponseStatus,
    dock_open: RwSignal<bool>,
    browse_scroll_y: RwSignal<Option<f64>>,
) -> AnyView {
    let stream = NodeRef::<leptos::html::Section>::new();
    let query_input = NodeRef::<leptos::html::Input>::new();
    #[cfg(feature = "hydrate")]
    super::restore_public_position(stream, Memo::new(move |_| works.with(Option::is_some)));
    let buffer = RwSignal::new(context.query.q.clone());
    let open = RwSignal::new(false);
    let draft = RwSignal::new(context.query.tags.clone());
    let initial = StoredValue::new(context.query.tags.clone());
    let is_tag = context.tag_page.is_some();
    let has_query = !context.query.q.is_empty();
    let location = leptos_router::hooks::use_location();
    let search_open = Memo::new(move |_| {
        !is_tag && (dock_open.get() || has_query || location.hash.get() == "#tim-kiem")
    });
    let toolbar_status = RwSignal::new(None::<String>);
    let dismiss = move || {
        draft.set(initial.get_value());
        open.set(false);
    };
    let dismiss_picker = move || {
        dismiss();
        #[cfg(feature = "hydrate")]
        if document().get_element_by_id("open-tag-filters").is_none() {
            if !search_open.get_untracked() {
                focus_recovery("open-public-search");
            } else {
                focus_recovery("public-query");
            }
        }
    };
    let enhanced = RwSignal::new(false);
    Effect::new(move |_| enhanced.set(true));
    let action = context
        .tag_page
        .as_deref()
        .map(tag_href)
        .unwrap_or_else(|| "/".into());
    let hidden_tags = context
        .query
        .tags
        .iter()
        .filter(|tag| Some(*tag) != context.tag_page.as_ref())
        .cloned()
        .collect::<Vec<_>>();
    let applied_count = hidden_tags.len();
    let filter_label = StoredValue::new(if applied_count == 0 {
        "Lọc thẻ".to_owned()
    } else {
        format!("Lọc thẻ, đã chọn {applied_count} thẻ")
    });
    let filter_query = StoredValue::new(context.query.q.clone());
    let filter_retry = StoredValue::new(context.href(None));
    let mut cleared = context.clone();
    cleared.query.q.clear();
    cleared.query.cursor = None;
    let clear_href = cleared.href(None);
    let clear_nav = clear_href.clone();
    let navigate = leptos_router::hooks::use_navigate();
    // R4.2 search dock (#404, DQ-02): the search icon opens it at every width. On a phone it is
    // a full-screen history entry (`#tim-kiem`, #325); on wider windows it replaces the icon
    // cluster in place. Closing drops unsent text and any submitted query, then returns focus
    // to the search icon.
    let close_search = {
        let navigate = navigate.clone();
        let href = clear_href.clone();
        Callback::new(move |()| {
            buffer.set(String::new());
            #[cfg(feature = "hydrate")]
            if browse_scroll_y.get_untracked().is_some()
                && location.hash.get_untracked() == "#tim-kiem"
                && let Ok(history) = window().history()
                && history.back().is_ok()
            {
                return;
            }
            dock_open.set(false);
            if has_query || location.hash.get_untracked() == "#tim-kiem" {
                navigate(
                    &href,
                    leptos_router::NavigateOptions {
                        replace: true,
                        scroll: false,
                        ..Default::default()
                    },
                );
            }
            #[cfg(feature = "hydrate")]
            restore_browse(browse_scroll_y);
        })
    };
    let open_search = {
        let navigate = navigate.clone();
        let href = format!("{}#tim-kiem", context.href(None));
        Callback::new(move |()| {
            #[cfg(feature = "hydrate")]
            {
                buffer.set(String::new());
                dock_open.set(true);
                if is_narrow_viewport() {
                    browse_scroll_y.set(Some(window().scroll_y().unwrap_or_default()));
                    navigate(
                        &href,
                        leptos_router::NavigateOptions {
                            scroll: false,
                            ..Default::default()
                        },
                    );
                    window().scroll_to_with_x_and_y(0.0, 0.0);
                }
                request_animation_frame(move || focus_no_scroll("public-query"));
            }
            #[cfg(not(feature = "hydrate"))]
            let _ = (&navigate, &href, browse_scroll_y);
        })
    };
    let submit_search = {
        let navigate = navigate.clone();
        let context = context.clone();
        move |event: leptos::ev::SubmitEvent| {
            let Ok(data) = leptos::form::form_data_from_event(&event) else {
                return;
            };
            let mut next = context.clone();
            next.query.q = data.get("q").as_string().unwrap_or_default();
            next.query.cursor = None;
            let mut url = next.href(None);
            #[cfg(feature = "hydrate")]
            let mobile = !is_tag && is_narrow_viewport();
            #[cfg(not(feature = "hydrate"))]
            let mobile = false;
            if mobile && next.query.q.is_empty() {
                url.push_str("#tim-kiem");
            }
            event.prevent_default();
            focus_submitted_results(&data);
            navigate(
                &url,
                leptos_router::NavigateOptions {
                    replace: mobile && browse_scroll_y.get_untracked().is_some(),
                    scroll: false,
                    ..Default::default()
                },
            );
        }
    };
    let composing = StoredValue::new(false);
    // “/” opens the dock (or returns to its field) unless the person is typing elsewhere, a
    // dialog is open or an IME composition is in progress.
    #[cfg(feature = "hydrate")]
    if !is_tag {
        let shortcut = window_event_listener(leptos::ev::keydown, move |event| {
            if event.key() != "/"
                || event.default_prevented()
                || event.is_composing()
                || event.ctrl_key()
                || event.meta_key()
                || event.alt_key()
                || composing.get_value()
                || any_modal_dialog_open()
                || typing_target()
            {
                return;
            }
            event.prevent_default();
            if search_open.get_untracked() {
                focus_no_scroll("public-query");
            } else {
                open_search.run(());
            }
        });
        on_cleanup(move || shortcut.remove());
    }
    #[cfg(feature = "hydrate")]
    if catalog.is_some() {
        let guard = expect_context::<crate::navigation::NavigationGuard>();
        let registration = guard.set_overlay(move |reason| {
            if open.get_untracked() {
                if reason == vink_ui::CloseReason::Navigate {
                    dismiss();
                } else {
                    dismiss_picker();
                }
                true
            } else {
                false
            }
        });
        on_cleanup(move || guard.clear_overlay(registration));
    }
    let heading_context = context.clone();
    let chip_context = context.clone();
    let result_context = context.clone();
    view! {
        <div id="explore" class="vk-explore vk-discovery" class:vk-discovery--tag=is_tag
            on:compositionstart=move |_| composing.set_value(true)
            on:compositionend=move |_| composing.set_value(false)
            on:keydown=move |event| {
                #[cfg(feature = "hydrate")]
                if event.key() == "Escape" && !event.default_prevented() && !event.is_composing()
                    && event.key_code() != 229 && !composing.get_value()
                    && search_open.get_untracked() && !any_modal_dialog_open()
                    && (is_narrow_viewport() || in_dock(&event))
                {
                    event.prevent_default();
                    close_search.run(());
                }
                #[cfg(not(feature = "hydrate"))]
                let _ = event;
            }
            data-enhanced=move || enhanced.get().to_string()
            data-search=move || if search_open.get() { "open" } else { "closed" }>
            <div class="vk-explore__top">
                <Suspense fallback=move || view! { <header id="explore-heading" class="vk-explore__heading">
                    <PageTitle is_tag>
                        {if is_tag { "Đang mở thẻ…" } else if search_open.get() { "Tìm tác phẩm" } else { "Khám phá" }}
                    </PageTitle>
                </header> }>
                    {move || {
                        let context = heading_context.clone();
                        let status = status.clone();
                        Suspend::new(async move {
                            let (_, result) = works.await;
                            view! { <DiscoveryHeader context result status search_open=Signal::from(search_open) /> }
                        })
                    }}
                </Suspense>
                <div class="vk-discovery-toolbar">
                    <form method="get" action=action.clone() on:submit=submit_search role="search" id="public-search" class="vk-public-search">
                        {hidden_tags.into_iter().map(|tag| view! { <HiddenField name="tag" value=Signal::stored(tag) /> }).collect_view()}
                        <SearchField id="public-query"
                            label=if is_tag { "Tìm trong thẻ" } else { "Tìm theo tên hoặc tác giả" }
                            placeholder=if is_tag { "Tìm trong thẻ" } else { "Tìm tác phẩm hoặc tác giả" }
                            compact_placeholder=if is_tag { "Tìm trong thẻ" } else { "Tìm tác phẩm" }
                            name="q"
                            variant=SearchVariant::Bar
                            value=buffer on_input=move |input: vink_ui::TextInput| buffer.set(input.value)
                            input_ref=query_input
                            leading=move || (!is_tag).then(|| view! {
                                <span class="vk-search-close">
                                    <IconButton id="close-public-search" label="Về Khám phá" icon=IconName::ArrowBack
                                        on_press=close_search />
                                </span>
                            })
                            clear_href clear_id="clear-public-search" on_clear=move |_| {
                                if !is_tag {
                                    dock_open.set(true);
                                }
                                #[cfg(feature = "hydrate")]
                                let mobile = !is_tag && is_narrow_viewport();
                                #[cfg(not(feature = "hydrate"))]
                                let mobile = false;
                                navigate(&clear_nav, leptos_router::NavigateOptions { replace: mobile, scroll: false, ..Default::default() });
                                focus_recovery("public-query");
                            } />
                    </form>
                    {(!is_tag).then(|| view! {
                        <span class="vk-search-launcher">
                            <IconButton id="open-public-search" label="Tìm tác phẩm" icon=IconName::Search
                                expanded=Signal::from(search_open) on_press=open_search />
                        </span>
                    })}
                    {catalog.map(|catalog| view! {
                        <Suspense fallback=|| view! { <p class="vk-muted" role="status" data-testid="tag-catalog-loading">"Đang tải danh mục thẻ…"</p> }>
                            <Show when=move || catalog.map(|result| !matches!(result, Ok(list) if list.tags.is_empty())).unwrap_or(false)>
                                <span class="vk-tag-launcher" data-count=applied_count>
                                    <IconButton id="open-tag-filters" icon=IconName::FilterList label=filter_label.get_value()
                                        expanded=Signal::from(open) on_press=move |_| open.set(true) />
                                    {(applied_count > 0).then(|| view! {
                                        <span class="vk-tag-launcher__badge" aria-hidden="true">{applied_count}</span>
                                    })}
                                </span>
                            </Show>
                        </Suspense>
                    })}
                    {(!is_tag).then(|| view! {
                        <p class="vk-discovery-toolbar-status" aria-hidden="true">{move || toolbar_status.get().unwrap_or_default()}</p>
                    })}
                    {(!is_tag).then(|| view! { <ExploreBrand /> })}
                </div>
            </div>
            {catalog.map(|catalog| view! {
                <Suspense fallback=|| ()>
                    {move || catalog.get().and_then(|result| match result {
                        Ok(list) if list.tags.is_empty() => None,
                        Ok(list) => Some(view! {
                            <details id="tag-dock" class="vk-tag-dock">
                                <summary>"Lọc thẻ"</summary>
                                <FilterForm id="dock" q=filter_query.get_value() draft initial list open />
                            </details>
                        }.into_any()),
                        Err(_) => Some(view! {
                            <details id="tag-dock" class="vk-tag-dock">
                                <summary>"Lọc thẻ"</summary>
                                <p role="alert">"Chưa tải được danh mục thẻ."</p>
                                <a class="vk-ui-link" href=filter_retry.get_value()>"Tải lại danh mục thẻ"</a>
                            </details>
                        }.into_any()),
                    })}
                </Suspense>
                <Dialog id="tag-picker" open title="Lọc thẻ" kind=DialogKind::Sheet focus_title=true
                    on_dismiss=move |_| dismiss_picker()
                    header_action=move || view! { <IconButton id="close-tag-filters" label="Đóng lọc thẻ" icon=IconName::Close
                        surface=ActionSurface::Tonal on_press=move |_| dismiss_picker() /> }>
                    <Suspense fallback=|| view! { <p role="status">"Đang tải danh mục thẻ…"</p> }>
                        {move || catalog.get().map(|result| match result {
                            Ok(list) if list.tags.is_empty() => view! {
                                <p role="status">"Chưa có thẻ để lọc."</p>
                            }.into_any(),
                            Ok(list) => view! {
                                <FilterForm id="modal" q=filter_query.get_value() draft initial list open modal=true />
                            }.into_any(),
                            Err(_) => view! {
                                <div class="vk-discovery-catalog-error">
                                    <p role="alert">"Chưa tải được danh mục thẻ. Các thẻ đang áp dụng vẫn được giữ."</p>
                                    <Button variant=ButtonVariant::Tonal id="retry-tag-catalog" on_press=move |_| {
                                        focus_recovery("tag-picker-title");
                                        catalog.refetch();
                                    }>"Thử tải lại"</Button>
                                </div>
                            }.into_any(),
                        })}
                    </Suspense>
                </Dialog>
            })}
            {(!is_tag).then(|| view! { <ExploreModes mode=ExploreMode::Works author_query=context.query.q.clone() /> })}
            <Suspense fallback=|| ()>
                {move || {
                    let context = chip_context.clone();
                    Suspend::new(async move {
                        let (_, result) = works.await;
                        view! { <AppliedFilters context list=result.ok() /> }
                    })
                }}
            </Suspense>
            <section node_ref=stream id="community-stream" aria-label="Tác phẩm khám phá" tabindex="-1">
                <Suspense fallback=|| view! { <p role="status" data-testid="discovery-loading">"Đang tải kết quả…"</p> }>
                    {move || {
                        let context = result_context.clone();
                        Suspend::new(async move {
                            let (_, result) = works.await;
                            match result {
                                Ok(list) => {
                                    let live = public_reads::follow_works(context.query.clone(), list);
                                    view! { <PublicResults context live toolbar_status /> }.into_any()
                                }
                                Err(error) => view! { <DiscoveryFailure context error /> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </section>
        </div>
    }.into_any()
}

/// Tất cả uses the R4.2 page title (#402/#404); a tag page keeps its Display heading.
#[component]
fn PageTitle(is_tag: bool, children: Children) -> AnyView {
    if is_tag {
        view! { <Heading level=HeadingLevel::H1 role=TypeRole::Display>{children()}</Heading> }
            .into_any()
    } else {
        view! { <h1 class="vk-ui-heading vk-ui-type-page-title">{children()}</h1> }.into_any()
    }
}

#[component]
fn DiscoveryHeader(
    context: ExploreContext,
    result: Result<WorkList, LoadError>,
    status: ResponseStatus,
    search_open: Signal<bool>,
) -> AnyView {
    let is_tag = context.tag_page.is_some();
    let list = match result {
        Ok(list) => list,
        Err(error) => {
            status.set(if error.is_invalid() { 422 } else { 503 });
            return view! {
                <Title text="Chưa tải được kết quả — Vink" />
                <Meta name="robots" content="noindex" />
                <Link rel="canonical" href=crate::canonical_url(&context.tag_page.as_deref().map(tag_href).unwrap_or_else(|| "/".into())) />
                <header id="explore-heading" class="vk-explore__heading">
                    <PageTitle is_tag>"Khám phá"</PageTitle>
                </header>
            }.into_any();
        }
    };
    let canonical = context.canonical(&list);
    if canonical.href(None) != context.href(None) {
        status.redirect(&canonical.href(None));
        #[cfg(feature = "hydrate")]
        {
            let href = canonical.href(None);
            let navigate = leptos_router::hooks::use_navigate();
            Effect::new(move |_| {
                navigate(
                    &href,
                    leptos_router::NavigateOptions {
                        replace: true,
                        scroll: false,
                        ..Default::default()
                    },
                )
            });
        }
    }
    let page_slug = context.tag_page.clone();
    let live = public_reads::follow_works(context.query, list);
    let data = live.data;
    let tag = Memo::new(move |_| {
        page_slug.as_ref().and_then(|slug| {
            data.with(|list| {
                list.filters
                    .tags
                    .iter()
                    .find(|f| f.requested == requested_slug(slug))
                    .cloned()
            })
        })
    });
    if is_tag
        && tag
            .get_untracked()
            .is_some_and(|f| f.resolution == "not_found")
    {
        status.set(404);
    }
    let label = move || {
        tag.get()
            .and_then(|f| f.tag)
            .map(|t| t.label)
            .unwrap_or_else(|| {
                if is_tag {
                    "Không tìm thấy thẻ".into()
                } else {
                    "Khám phá".into()
                }
            })
    };
    let title = ArcRwSignal::new(label());
    Effect::new({
        let title = title.clone();
        move |_| title.set(label())
    });
    view! {
        <Title text={ let title = title.clone(); move || format!("{} — Vink", title.get()) } />
        <Meta name="description" content="Tìm tác phẩm công khai theo tên, tác giả và thẻ trên Vink." />
        <Link rel="canonical" href=crate::canonical_url(&canonical.href(None)) />
        <header id="explore-heading" class="vk-explore__heading">
            {is_tag.then(|| view! { <a class="vk-ui-link" href="/">"Về Khám phá"</a> })}
            <PageTitle is_tag>
                {move || if is_tag {
                    title.get()
                } else if search_open.get() {
                    "Tìm tác phẩm".into()
                } else {
                    "Khám phá".into()
                }}
            </PageTitle>
            {move || if is_tag {
                match tag.get() {
                    Some(filter) if filter.resolution == "not_found" => view! { <p>"Không tìm thấy thẻ này. Hãy quay về Khám phá để chọn thẻ khác."</p> }.into_any(),
                    Some(filter) => {
                        let retired = filter.resolution == "retired";
                        let tag = filter.tag;
                        view! {
                            {tag.as_ref().and_then(|t| t.description.clone()).map(|text| view! { <p>{text}</p> })}
                            {retired.then(|| view! { <p>"Thẻ đã ngừng dùng; không còn dùng để tìm tác phẩm công khai."</p> })}
                            {tag.and_then(|t| t.successor).map(|next| view! { <a class="vk-ui-link" href=tag_href(&next.slug)>{format!("Xem thẻ {}", next.label)}</a> })}
                        }.into_any()
                    }
                    None => ().into_any(),
                }
            } else { ().into_any() }}
        </header>
    }.into_any()
}

#[component]
fn AppliedFilters(context: ExploreContext, list: Option<WorkList>) -> AnyView {
    let tags = context
        .query
        .tags
        .iter()
        .filter(|tag| Some(*tag) != context.tag_page.as_ref())
        .cloned()
        .collect::<Vec<_>>();
    if tags.is_empty() {
        return ().into_any();
    }
    let live = list.map(|list| public_reads::follow_works(context.query.clone(), list));
    // R4.2 AppliedRow (#404): one link clears every applied tag and keeps the query.
    let mut cleared = context.clone();
    cleared
        .query
        .tags
        .retain(|tag| Some(tag) == context.tag_page.as_ref());
    cleared.query.cursor = None;
    let clear_all = cleared.href(None);
    view! {
        <div id="applied-filters" class="vk-applied-filters">
            {move || {
                let resolved = live.as_ref().map(|live| live.data.with(|list| list.filters.tags.clone())).unwrap_or_default();
                tags.iter().map(|slug| {
                    let filter = resolved.iter().find(|filter| filter.requested == requested_slug(slug));
                    let label = filter.and_then(|filter| filter.tag.as_ref().map(|tag| &tag.label)).unwrap_or(slug);
                    let mut without = context.clone();
                    without.query = without.query.without_tag(slug);
                    let mut facts = TagFacts::active(slug, label);
                    if let Some(filter) = filter {
                        facts.retired = filter.resolution == "retired";
                        if filter.resolution == "not_found" { facts.problem = Some("Không tìm thấy thẻ này.".into()); }
                    }
                    view! { <a class="vk-applied-tag" href=without.href(None) aria-label=format!("Bỏ lọc {label}")>
                        <TagChip id=format!("applied-{}", crate::discovery::encode(slug)) tag=facts remove_label=format!("Bỏ lọc {label}") />
                        <Icon name=IconName::Close />
                    </a> }
                }).collect_view()
            }}
            <a id="clear-applied-tags" class="vk-ui-link vk-applied-filters__clear" href=clear_all>"Xóa lọc"</a>
        </div>
    }.into_any()
}

#[component]
fn FilterForm(
    id: &'static str,
    q: String,
    draft: RwSignal<Vec<String>>,
    initial: StoredValue<Vec<String>>,
    list: TagList,
    open: RwSignal<bool>,
    #[prop(optional)] modal: bool,
) -> AnyView {
    let known: Vec<_> = list.tags.iter().map(|tag| tag.slug.clone()).collect();
    let changed = Memo::new(move |_| {
        draft.with(|selected| {
            initial.with_value(|applied| {
                selected.len() != applied.len() || !selected.iter().all(|tag| applied.contains(tag))
            })
        })
    });
    view! {
        <Form method="get" action="/" noscroll=true on_form_data=Arc::new(move |data| {
            drop_unnamed_submitter(data);
            focus_submitted_results(data);
            open.set(false);
        }) attr:class="vk-discovery-filter-form" attr:id=format!("{id}-tag-form") attr:aria-describedby=format!("{id}-tag-help")>
            <HiddenField name="q" value=Signal::stored(q) />
            {move || draft.get().into_iter().filter(|slug| !known.contains(slug))
                .map(|slug| view! { <HiddenField name="tag" value=Signal::stored(slug) /> }).collect_view()}
            {["form", "theme"].into_iter().filter_map(|facet| {
                let options: Vec<TagDto> = list.tags.iter().filter(|tag| tag.facet == facet).cloned().collect();
                (!options.is_empty()).then(|| view! {
                    <fieldset>
                        <legend>{if facet == "form" { "Dạng bài" } else { "Chủ đề" }}</legend>
                        <div class="vk-discovery-options">
                            {options.into_iter().map(|tag| {
                                let slug = tag.slug.clone();
                                let checked_slug = slug.clone();
                                let disabled_slug = slug.clone();
                                view! { <span class="vk-discovery-option">
                                    <Checkbox id=format!("{id}-tag-{}", tag.tag_id) label=tag.label name="tag" value=tag.slug
                                        checked=Signal::derive(move || draft.with(|tags| tags.contains(&checked_slug)))
                                        // Native GET still submits freely. The server validates the four-tag limit.
                                        disabled=Signal::derive(move || modal && draft.with(|tags| tags.len() >= 4 && !tags.contains(&disabled_slug)))
                                        on_change=move |checked| {
                                            draft.update(|tags| {
                                                tags.retain(|tag| tag != &slug);
                                                if checked && tags.len() < 4 { tags.push(slug.clone()); }
                                            });
                                        } />
                                </span> }
                            }).collect_view()}
                        </div>
                    </fieldset>
                })
            }).collect_view()}
            {list.truncated.then(|| view! { <p>"Danh mục đang hiển thị 200 thẻ đầu. Thẻ khác vẫn có thể mở từ tác phẩm hoặc đường dẫn riêng."</p> })}
            <div class="vk-discovery-filter-footer">
                <div>
                    <p id=format!("{id}-tag-help")>{if modal { "Khớp tất cả thẻ" } else { "Khớp tất cả thẻ · Tối đa 4" }}</p>
                    {modal.then(|| view! { <p id="tag-selection-count" role="status" aria-atomic="true">
                        {move || format!("Đã chọn {}/4", draft.with(Vec::len))}
                    </p> })}
                </div>
                <Button kind=ButtonKind::Submit variant=ButtonVariant::Filled
                    state=Signal::derive(move || if modal && !changed.get() { ActionState::Disabled { reason: None } } else { ActionState::Enabled })>
                    "Áp dụng"
                </Button>
            </div>
        </Form>
    }.into_any()
}

#[component]
fn PublicResults(
    context: ExploreContext,
    live: public_reads::Live<WorkList>,
    #[prop(optional)] toolbar_status: Option<RwSignal<Option<String>>>,
) -> AnyView {
    let data = live.data;
    let refreshing = live.refreshing;
    let cached_unverified = live.cached_unverified;
    let failed = live.failed;
    let rejected = live.rejected;
    let removed = live.removed;
    let works = Memo::new(move |_| data.with(|list| list.works.clone()));
    if let Some(toolbar_status) = toolbar_status {
        Effect::new(move |_| {
            if rejected.get().is_some() || removed.get() {
                toolbar_status.set(None);
                return;
            }
            let count = works.with(|list| list.len());
            toolbar_status.set((count > 0).then(|| format!("Trang này: {count} tác phẩm")));
        });
    }
    let show_inline_scope =
        context.tag_page.is_some() || !context.query.q.is_empty() || !context.query.tags.is_empty();
    let list_context = context.clone();
    let page_context = context.clone();
    let empty_context = context.clone();
    let rejected_context = context.clone();
    let removed_context = context.clone();
    view! {
        {move || if let Some(rejection) = rejected.get() {
            view! { <DiscoveryFailure context=rejected_context.clone() error=LoadError::Invalid(rejection) /> }.into_any()
        } else if removed.get() {
            view! { <DiscoveryFailure context=removed_context.clone() error=LoadError::NotFound /> }.into_any()
        } else {
            let list_context = list_context.clone();
            let page_context = page_context.clone();
            let empty_context = empty_context.clone();
            let live = live.clone();
            let retry_context_cursor = context.query.cursor.is_some();
            view! {
                <div id="public-collection" aria-busy=move || refreshing.get().to_string() data-refreshing=move || refreshing.get().to_string()>
                    <Show when=move || failed.get()>
                        <div data-testid="public-refresh-failed"><InlineNotice tone=Tone::Warning
                            message="Chưa làm mới được kết quả. Bạn vẫn đang xem bản đã tải cho cùng bộ lọc."
                            action=RecoveryAction { intent: RecoveryIntent::Retry, label: "Thử lại".into() }
                            on_recover={ let live = live.clone(); move |_| live.refresh() } /></div>
                    </Show>
                    <div id="public-results">
                        {move || if works.read().is_empty() {
                            let has_tags = !empty_context.query.tags.is_empty();
                            let has_query = !empty_context.query.q.is_empty();
                            let filtered = has_tags || has_query;
                            // A page tail or unresolved tag never establishes an empty public corpus.
                            let unavailable_tag = data.with(|list| list.filters.tags.iter().any(|tag| matches!(tag.resolution.as_str(), "not_found" | "retired")));
                            let continuation = data.with(|list| list.next_cursor.is_some());
                            let page_empty = empty_context.query.cursor.is_some() || continuation;
                            let mut no_tags = empty_context.clone(); no_tags.query.tags.clear(); no_tags.query.cursor = None; no_tags.tag_page = None;
                            let mut no_query = empty_context.clone(); no_query.query.q.clear(); no_query.query.cursor = None;
                            if unavailable_tag {
                                view! { <div data-testid="public-tag-unavailable">
                                    <p role="status" aria-atomic="true">"Không thể tìm tác phẩm theo thẻ này."</p>
                                    <a id="clear-public-tags" class="vk-ui-link" href=no_tags.href(None)>"Bỏ lọc thẻ"</a>
                                </div> }.into_any()
                            } else {
                                let message = if page_empty {
                                    if continuation { "Trang này chưa có tác phẩm; bạn có thể xem trang tiếp." } else { "Trang này không còn tác phẩm." }
                                } else if filtered { "Chưa có tác phẩm phù hợp." } else { "Chưa có tác phẩm công khai." };
                                let message = if cached_unverified.get() || refreshing.get() || failed.get() {
                                    format!("Bản đã tải: {message}")
                                } else { message.to_owned() };
                                view! { <div data-testid=if page_empty { "public-empty-page" } else if filtered { "public-no-results" } else { "public-empty" }>
                                    <EmptyState artwork=if filtered || page_empty { EmptyArtwork::NoResults } else { EmptyArtwork::NoPublicWorks } message announce=true>
                                        {has_tags.then(|| view! { <a id="clear-public-tags" class="vk-ui-button vk-ui-button--tonal" href=no_tags.href(None)>"Bỏ lọc thẻ"</a> })}
                                        {has_query.then(|| view! { <a id="clear-public-results" class="vk-ui-button vk-ui-button--tonal" href=no_query.href(None)>"Xóa tìm kiếm"</a> })}
                                        {(!filtered && !page_empty).then(|| view! { <a class="vk-ui-button vk-ui-button--tonal" href="/ban-viet">"Đến Bàn viết"</a> })}
                                    </EmptyState>
                                </div> }.into_any()
                            }
                        } else {
                            let context = list_context.clone();
                            view! {
                                <p class="vk-muted vk-public-scope" class:vk-ui-visually-hidden=!show_inline_scope role="status" aria-atomic="true">
                                    {move || format!("Trang này: {} tác phẩm", works.read().len())}
                                </p>
                                <ul class="vk-public-list" role="list">
                                    <For each=move || works.with(|works| works.iter().map(|work| work.work_id.clone()).collect::<Vec<_>>()) key=Clone::clone
                                        children=move |work_id| view! { <ResultRow work_id data cached_unverified context=context.clone() /> } />
                                </ul>
                            }.into_any()
                        }}
                        <nav class="vk-public-pagination" aria-label="Trang kết quả">
                            {
                                let page_context = page_context.clone();
                                move || data.with(|list| list.next_cursor.clone()).map(|cursor| {
                                    let mut next = page_context.clone(); next.query.cursor = Some(cursor);
                                    view! { <a id="public-next" class="vk-ui-link" href=next.href(None)>"Trang tiếp →"</a> }
                                })
                            }
                            {retry_context_cursor.then(|| {
                                let mut first = page_context.clone(); first.query.cursor = None;
                                view! { <a id="public-first" class="vk-ui-link" href=first.href(None)>"Về trang đầu"</a> }
                            })}
                        </nav>
                    </div>
                </div>
            }.into_any()
        }}
    }.into_any()
}

pub(super) fn highlighted_text(text: &str, query: &str) -> AnyView {
    crate::search::highlight_spans(text, query)
        .into_iter()
        .map(|(part, matched)| {
            if matched {
                view! { <mark class="vk-search-mark">{part}</mark> }.into_any()
            } else {
                part.into_any()
            }
        })
        .collect_view()
        .into_any()
}

#[component]
fn ResultRow(
    work_id: String,
    data: Signal<WorkList>,
    cached_unverified: Signal<bool>,
    context: ExploreContext,
) -> AnyView {
    let id = work_id.clone();
    let work = Memo::new(move |_| {
        data.with(|list| list.works.iter().find(|work| work.work_id == id).cloned())
    });
    view! { <WorkCardRow work_id work cached_unverified context /> }.into_any()
}

/// Shared card for both scopes. The feed's date is first-public, supplied by Publication.
#[component]
fn WorkCardRow(
    work_id: String,
    work: Memo<Option<WorkCard>>,
    cached_unverified: Signal<bool>,
    context: ExploreContext,
) -> impl IntoView {
    #[cfg(feature = "hydrate")]
    let navigate = leptos_router::hooks::use_navigate();
    #[cfg(feature = "hydrate")]
    let feed_scope = use_context::<feed::ReturnScope>();
    view! {
        <li id=crate::public_list::work_anchor(&work_id) class="vk-public-row" data-work-id=work_id>
            {move || work.get().map(|work| {
                let href = context.reader_href(&work.work_id);
                let title_id = format!("work-title-{}", work.work_id);
                let title_label = title_id.clone();
                #[cfg(feature = "hydrate")]
                let card_context = context.clone();
                #[cfg(feature = "hydrate")]
                let return_id = work.work_id.clone();
                #[cfg(feature = "hydrate")]
                let navigate = navigate.clone();
                let query = context.query.q.clone();
                let author = adapters::card_author(&work, cached_unverified.get());
                let primary_tag = work.tags.first().map(|tag| tag.label.clone());
                let excerpt = work.excerpt.clone();
                let following = context.mode == ExploreMode::Following;
                let date = following.then(|| first_public_date(&work.published_at)).flatten();
                let separate_metadata = primary_tag.is_some() && date.is_some();
                let date = date.map(|label| view! {
                    <time class="vk-feed-date" datetime=work.published_at.clone() title="Công khai lần đầu">{label}</time>
                });
                #[cfg(feature = "hydrate")]
                let author_return = work.work_id.clone();
                // Placeholder labels are not author names: never mark them as search matches.
                let author_text = (!author.neutral).then(|| highlighted_text(&author.name, &query));
                let byline = view! {
                    <div class="vk-public-row__byline" on:click=move |_event| {
                        #[cfg(feature = "hydrate")]
                        if following && super::is_primary_link_click(&_event)
                            && event_target::<leptos::web_sys::Element>(&_event)
                                .closest("a[data-profile-return]").ok().flatten().is_some()
                            && let Some(scope) = feed_scope
                        {
                            scope.remember(&author_return);
                        }
                    }>
                        <AuthorIdentity author id=Some(format!("work-{}-author", work.work_id)) name_view=author_text />
                    </div>
                };
                view! {
                    <div class="vk-public-row__card">
                        <div class="vk-public-row__main">
                            // Heading first in the DOM (heading navigation, Tab: work → author).
                            <h2 class="vk-public-row__title" id=title_id>
                                <a class="vk-public-row__link" href=href aria-labelledby=title_label on:click=move |_event| {
                                    #[cfg(feature = "hydrate")]
                                    {
                                        if super::is_primary_link_click(&_event) && let Some(scope) = feed_scope {
                                            scope.remember(&return_id);
                                        }
                                        super::remember_public_position(&_event, &card_context, &return_id, &navigate);
                                    }
                                }>{highlighted_text(&work.title, &query)}</a>
                            </h2>
                            {(primary_tag.is_some() || date.is_some()).then(|| view! {
                                <p class="vk-public-row__eyebrow">
                                    {primary_tag.map(|tag| view! { <span class="vk-public-row__tag">{tag}</span> })}
                                    {separate_metadata.then_some(" · ")}
                                    {date}
                                </p>
                            })}
                            {excerpt.map(|text| view! { <p class="vk-public-row__excerpt">{text}</p> })}
                            {byline}
                        </div>
                    </div>
                }
            })}
        </li>
    }
}

fn first_public_date(value: &str) -> Option<String> {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsValue;
        let date = js_sys::Date::new(&JsValue::from_str(value));
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"timeZone".into(), &"Asia/Ho_Chi_Minh".into()).ok()?;
        date.get_time()
            .is_finite()
            .then(|| date.to_locale_date_string("vi-VN", &options).as_string())
            .flatten()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = value;
        None
    }
}

#[component]
pub(super) fn PublicTags(tags: Vec<PublicTagDto>) -> impl IntoView {
    view! { <ul class="vk-reader-tags" aria-label="Thẻ tác phẩm">
        {tags.into_iter().map(|tag| view! { <li><a class="vk-ui-link" href=tag_href(&tag.slug)>{tag.label}</a></li> }).collect_view()}
    </ul> }
}

#[component]
fn DiscoveryFailure(context: ExploreContext, error: LoadError) -> AnyView {
    use crate::api::{InvalidInputField, InvalidRead};
    let mut first = context.clone();
    first.query.cursor = None;
    match error {
        LoadError::Invalid(InvalidRead::RestartCursor) => view! {
            <div role="alert" data-testid="public-invalid" data-invalid-kind="cursor">
                <p>"Trang kết quả không còn hợp lệ hoặc không còn khớp tìm kiếm hiện tại."</p>
            </div>
            <a id="restart-public" class="vk-ui-link" href=first.href(None)>"Thử từ trang đầu"</a>
            <a class="vk-ui-link" href="/">"Bỏ toàn bộ bộ lọc"</a>
        }
        .into_any(),
        LoadError::Invalid(InvalidRead::InvalidInput(field)) => {
            let (kind, message) = match field {
                InvalidInputField::Query => (
                    "q",
                    "Từ khóa tìm kiếm không hợp lệ (cần ít nhất một chữ hoặc số và không vượt quá giới hạn).",
                ),
                InvalidInputField::Tag => (
                    "tag",
                    "Bộ lọc thẻ không hợp lệ hoặc vượt quá số lượng cho phép.",
                ),
                InvalidInputField::Limit => ("limit", "Số lượng kết quả mỗi trang không hợp lệ."),
                InvalidInputField::Parameter => {
                    ("query", "Tham số tìm kiếm trên đường dẫn không hợp lệ.")
                }
            };
            view! {
                <div role="alert" data-testid="public-invalid" data-invalid-kind=kind>
                    <p>{message}</p>
                </div>
                <a id="clear-invalid-filters" class="vk-ui-link" href="/">"Bỏ toàn bộ bộ lọc"</a>
            }
            .into_any()
        }
        LoadError::Invalid(InvalidRead::Fallback) => view! {
            <div role="alert" data-testid="public-invalid" data-invalid-kind="fallback">
                <p>"Không thể tải danh sách tác phẩm với yêu cầu hiện tại."</p>
            </div>
            <a class="vk-ui-link" href="/">"Bỏ toàn bộ bộ lọc"</a>
        }
        .into_any(),
        LoadError::NotFound | LoadError::Unavailable => view! {
            <Unavailable />
            <a id="restart-public" class="vk-ui-link" href=first.href(None)>"Thử từ trang đầu"</a>
            <a class="vk-ui-link" href="/">"Bỏ toàn bộ bộ lọc"</a>
        }
        .into_any(),
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::contract::{TagFilter, WorkFilters};

    fn render(context: ExploreContext, list: WorkList, failed: bool, removed: bool) -> String {
        Owner::new().with(|| {
            let mut live = public_reads::follow_works(context.query.clone(), list);
            live.failed = Signal::stored(failed);
            live.removed = Signal::stored(removed);
            provide_context(leptos_router::location::RequestUrl::new("/"));
            view! { <leptos_router::components::Router><PublicResults context live /></leptos_router::components::Router> }.to_html()
        })
    }
    fn empty() -> WorkList {
        WorkList {
            works: vec![],
            filters: WorkFilters {
                q: None,
                tags: vec![],
            },
            next_cursor: None,
        }
    }
    fn context(q: &str, cursor: Option<&str>) -> ExploreContext {
        ExploreContext {
            mode: ExploreMode::Works,
            query: ExploreQuery::new(q.into(), vec![], cursor.map(str::to_owned)),
            tag_page: None,
        }
    }
    #[test]
    fn empty_corpus_and_filtered_miss_keep_one_html_status_and_real_recovery() {
        let corpus = render(context("", None), empty(), false, false);
        assert!(corpus.contains("Chưa có tác phẩm công khai."));
        assert!(corpus.contains("/pkg/empty/vink-empty-no-public-works.webp"));
        assert!(corpus.contains("href=\"/ban-viet\""));
        let filtered = render(context("mưa", None), empty(), false, false);
        assert!(filtered.contains("Chưa có tác phẩm phù hợp."));
        assert!(filtered.contains("/pkg/empty/vink-empty-no-results.webp"));
        assert!(filtered.contains("id=\"clear-public-results\""));
        assert!(!filtered.contains("Chưa có tác phẩm công khai."));
        for html in [corpus, filtered] {
            assert_eq!(html.matches("role=\"status\"").count(), 1);
            assert!(html.contains("alt=\"\""));
            assert!(!html.contains("<h3") && !html.contains("Trang này: 0"));
        }
    }
    #[test]
    fn empty_cursor_page_and_continuation_never_claim_an_empty_corpus() {
        for cursor in [None, Some("opaque-page")] {
            let mut list = empty();
            list.next_cursor = Some("next-page".into());
            let html = render(context("", cursor), list, false, false);
            assert!(html.contains("public-empty-page") && html.contains("xem trang tiếp"));
            assert!(html.contains("id=\"public-next\"") && html.contains("cursor=next-page"));
            assert_eq!(html.contains("id=\"public-first\""), cursor.is_some());
            assert!(!html.contains("Chưa có tác phẩm công khai."));
        }
        let tail = render(context("", Some("tail")), empty(), false, false);
        assert!(tail.contains("Trang này không còn tác phẩm."));
        assert!(tail.contains("id=\"public-first\""));
    }
    #[test]
    fn unknown_and_retired_tags_have_recovery_without_empty_artwork() {
        for resolution in ["not_found", "retired"] {
            let mut list = empty();
            list.filters.tags.push(TagFilter {
                requested: "tho".into(),
                resolution: resolution.into(),
                tag: None,
            });
            let mut scope = context("", None);
            scope.query.tags.push("tho".into());
            scope.tag_page = Some("tho".into());
            let html = render(scope, list, false, false);
            assert!(html.contains("Không thể tìm tác phẩm theo thẻ này."));
            assert!(html.contains("id=\"clear-public-tags\""));
            assert!(!html.contains("/pkg/empty/"));
        }
    }
    #[test]
    fn retained_empty_keeps_refresh_failure_facts_and_removed_snapshot_has_no_art() {
        let stale = render(context("mưa", None), empty(), true, false);
        assert!(stale.contains("public-refresh-failed") && stale.contains("bản đã tải"));
        assert!(stale.contains("Bản đã tải: Chưa có tác phẩm phù hợp."));
        let removed = render(context("", None), empty(), false, true);
        assert!(!removed.contains("/pkg/empty/"));
        assert!(removed.contains("id=\"restart-public\""));
    }
    #[test]
    fn discovery_r1_cards_render_excerpt_highlights_and_unfiltered_scope_visibility() {
        let card = crate::contract::WorkCard {
            work_id: "33333333-3333-4333-8333-333333333333".into(),
            title: "Mưa trên phố cũ".into(),
            excerpt: Some("Mưa rơi nhẹ qua phố cổ.".into()),
            author_display_name: "Lê Thu Mưa".into(),
            author: None,
            published_at: "2026-10-30T08:00:00Z".into(),
            tags: vec![PublicTagDto {
                tag_id: "11111111-1111-4111-8111-111111111111".into(),
                facet: "form".into(),
                label: "Thơ".into(),
                slug: "tho".into(),
            }],
        };
        let list = WorkList {
            works: vec![card],
            filters: WorkFilters {
                q: None,
                tags: vec![],
            },
            next_cursor: None,
        };
        let home_html = render(context("", None), list.clone(), false, false);
        assert!(
            home_html.contains("vk-public-scope vk-ui-visually-hidden"),
            "{home_html}"
        );
        assert!(home_html.contains("Trang này: 1 tác phẩm"), "{home_html}");
        assert!(home_html.contains(">LM</span>"), "{home_html}");
        assert!(
            home_html.contains("vk-public-row__tag\">Thơ</span>"),
            "{home_html}"
        );
        assert!(!home_html.contains("<mark"), "{home_html}");
        assert!(
            home_html.contains("vk-public-row__excerpt\">Mưa rơi nhẹ qua phố cổ.</p>"),
            "{home_html}"
        );

        let search_html = render(context("mua", None), list, false, false);
        assert!(
            !search_html.contains("vk-public-scope vk-ui-visually-hidden"),
            "{search_html}"
        );
        assert_eq!(
            search_html
                .matches("<mark class=\"vk-search-mark\">Mưa</mark>")
                .count(),
            2,
            "{search_html}"
        );
        assert!(
            search_html.contains("vk-public-row__excerpt\">Mưa rơi nhẹ qua phố cổ.</p>"),
            "{search_html}"
        );
    }
}
