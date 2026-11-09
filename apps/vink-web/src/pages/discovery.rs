//! TAG-PR-06 page glue. Shared chips/bylines render facts; this module owns GET/URL/focus.
use super::{ResponseStatus, Unavailable, drop_unnamed_submitter, focus_recovery};
use crate::api::{self, LoadError};
use crate::contract::{PublicTagDto, TagDto, TagList, WorkList};
use crate::discovery::{ExploreContext, ExploreQuery, requested_slug, tag_href};
use crate::public_reads;
use crate::ui::{
    adapters,
    patterns::{TagChip, TagFacts, WorkCard as WorkCardPattern},
};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::Form;
use leptos_router::hooks::{use_params_map, use_query_map};
use std::sync::Arc;
use vink_ui::{Button, ButtonKind, ButtonVariant, Checkbox, HiddenField};
use vink_ui::{
    Dialog, Heading, HeadingLevel, Icon, IconName, InlineNotice, RecoveryAction, RecoveryIntent,
    SearchField, SearchVariant, Tone, TypeRole,
};

#[derive(Clone, Copy)]
struct DiscoveryFocus(RwSignal<Option<&'static str>>);

#[derive(Clone, Copy)]
struct DiscoveryDraft {
    query: RwSignal<String>,
    tags: RwSignal<Vec<String>>,
    applied_tags: StoredValue<Vec<String>>,
    open: RwSignal<bool>,
}

fn query_from(params: &leptos_router::params::ParamsMap) -> ExploreQuery {
    ExploreQuery::new(
        params.get("q").unwrap_or_default(),
        params.get_all("tag").unwrap_or_default(),
        params.get("cursor"),
    )
}

pub(super) fn reader_context() -> Memo<ExploreContext> {
    let params = use_query_map();
    Memo::new(move |_| {
        let params = params.read();
        ExploreContext {
            query: query_from(&params),
            tag_page: params.get("from_tag").filter(|s| !s.is_empty()),
        }
    })
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
    let params = use_query_map();
    provide_context(DiscoveryFocus(RwSignal::new(None)));
    let context = Memo::new(move |_| {
        let mut query = query_from(&params.read());
        let tag_page = tag_page.map(|slug| slug.get());
        if let Some(slug) = &tag_page {
            query.tags.push(slug.clone());
            query.tags.sort();
            query.tags.dedup();
        }
        ExploreContext { query, tag_page }
    });
    // A URL scope owns its resource before I/O starts. Replacing A with B drops A's
    // suspended view immediately; B never waits for A's outstanding transport.
    view! {
        <For each=move || vec![context.get()] key=|context| context.href(None)
            children=move |context| view! { <DiscoveryScope context /> } />
    }
    .into_any()
}

#[component]
fn DiscoveryScope(context: ExploreContext) -> AnyView {
    provide_context(DiscoveryDraft {
        query: RwSignal::new(context.query.q.clone()),
        tags: RwSignal::new(context.query.tags.clone()),
        applied_tags: StoredValue::new(context.query.tags.clone()),
        open: RwSignal::new(false),
    });
    let initial_query = context.query.clone();
    let query = Memo::new(move |_| initial_query.clone());
    let catalog_epoch = RwSignal::new(0u64);
    let catalog = api::catalog_resource(Memo::new(move |_| (query.get(), catalog_epoch.get())));
    let works = api::works_resource(query);
    view! { <DiscoveryChrome context works catalog catalog_epoch status=ResponseStatus::capture() /> }.into_any()
}

#[component]
fn DiscoveryChrome(
    context: ExploreContext,
    works: Resource<(ExploreQuery, Result<WorkList, LoadError>)>,
    catalog: Resource<Result<TagList, LoadError>>,
    catalog_epoch: RwSignal<u64>,
    status: ResponseStatus,
) -> AnyView {
    let controls = expect_context::<DiscoveryDraft>();
    let buffer = controls.query;
    let draft = controls.tags;
    let open = controls.open;
    let focus = expect_context::<DiscoveryFocus>().0;
    let enhanced = RwSignal::new(false);
    Effect::new(move |_| enhanced.set(true));
    let composing = StoredValue::new(false);
    let is_tag = context.tag_page.is_some();
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
    let filter_query = context.query.q.clone();
    let modal_query = filter_query.clone();
    let modal_action = action.clone();
    let mut cleared = context.clone();
    cleared.query.q.clear();
    cleared.query.cursor = None;
    let clear_href = cleared.href(None);
    let clear_nav = clear_href.clone();
    let navigate = leptos_router::hooks::use_navigate();
    #[cfg(feature = "hydrate")]
    {
        adapt_picker(open);
        let guard = expect_context::<crate::navigation::NavigationGuard>();
        let registration = guard.set_overlay(move |_| {
            if open.get_untracked() {
                open.set(false);
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
        <div id="explore" class="vk-explore vk-discovery" class:vk-discovery--tag=is_tag data-enhanced=move || enhanced.get().to_string()>
            <Suspense fallback=move || view! { <header id="explore-heading" class="vk-explore__heading">
                <Heading level=HeadingLevel::H1 role=TypeRole::Display>{if is_tag { "Đang mở thẻ…" } else { "Khám phá" }}</Heading>
            </header> }>
                {move || {
                    let context = heading_context.clone();
                    let status = status.clone();
                    Suspend::new(async move {
                        let (_, result) = works.await;
                        view! { <DiscoveryHeader context result status /> }
                    })
                }}
            </Suspense>
            <div class="vk-discovery-layout">
                <div class="vk-discovery-main">
                    <Form method="get" action=action.clone() noscroll=true on_form_data=Arc::new(drop_unnamed_submitter)
                        attr:role="search" attr:id="public-search" attr:class="vk-public-search"
                        on:submit=move |_| focus.set(Some("works-title"))
                        on:compositionstart=move |_| composing.set_value(true)
                        on:compositionend=move |_| composing.set_value(false)
                        on:keydown=move |event: leptos::ev::KeyboardEvent| {
                            if event.key() == "Enter" && (composing.get_value() || event.is_composing() || event.key_code() == 229) { event.prevent_default(); }
                        }>
                        {hidden_tags.into_iter().map(|tag| view! { <HiddenField name="tag" value=Signal::stored(tag) /> }).collect_view()}
                        <SearchField id="public-query" label=if is_tag { "Tìm trong thẻ" } else { "Tìm tác phẩm" } name="q"
                            variant=SearchVariant::Bar help="Trong toàn thư viện công khai"
                            value=buffer on_input=move |input: vink_ui::TextInput| buffer.set(input.value)
                            clear_href clear_id="clear-public-search" on_clear=move |_| {
                                navigate(&clear_nav, leptos_router::NavigateOptions { scroll: false, ..Default::default() });
                                focus_recovery("public-query");
                            } />
                    </Form>
                    <div id="applied-filters" class="vk-applied-filters">
                        {(!is_tag).then(|| view! { <span class="vk-tag-launcher"><Button id="open-tag-filters" variant=ButtonVariant::Tonal expanded=Signal::from(open)
                            on_press=move |_| { catalog_epoch.update(|n| *n += 1); open.set(true); }>"Thẻ"</Button></span> })}
                        <Suspense fallback=|| ()>
                            {move || {
                                let context = chip_context.clone();
                                Suspend::new(async move {
                                    let (_, result) = works.await;
                                    result.ok().map(|list| view! { <AppliedFilters context list /> })
                                })
                            }}
                        </Suspense>
                    </div>
                    <section id="community-stream" aria-labelledby="works-title">
                        <h2 id="works-title" tabindex="-1">{if context.query.q.is_empty() && context.query.tags.is_empty() { "Tác phẩm mới" } else { "Kết quả" }}</h2>
                        <Suspense fallback=|| view! { <p role="status" data-testid="discovery-loading">"Đang tải kết quả…"</p> }>
                            {move || {
                                let context = result_context.clone();
                                Suspend::new(async move {
                                    let (_, result) = works.await;
                                    let focus = expect_context::<DiscoveryFocus>().0;
                                    Effect::new(move |_| {
                                        if let Some(target) = focus.get_untracked() { focus.set(None); focus_recovery(target); }
                                    });
                                    match result {
                                        Ok(list) => {
                                            let live = public_reads::follow_works(context.query.clone(), list);
                                            view! { <PublicResults context live /> }.into_any()
                                        }
                                        Err(error) => view! { <DiscoveryFailure context invalid=error == LoadError::Invalid /> }.into_any(),
                                    }
                                })
                            }}
                        </Suspense>
                    </section>
                </div>
                {(!is_tag).then(|| view! {
                    <details id="tag-dock" class="vk-tag-dock" open><summary>"Bộ lọc thẻ"</summary>
                        <FilterForm id="dock" action q=filter_query draft catalog open />
                    </details>
                    <Dialog id="tag-picker" open title="Bộ lọc thẻ" on_dismiss=move |_| open.set(false)>
                        <FilterForm id="modal" action=modal_action q=modal_query draft catalog open />
                    </Dialog>
                })}
            </div>
        </div>
    }.into_any()
}

#[component]
fn DiscoveryHeader(
    context: ExploreContext,
    result: Result<WorkList, LoadError>,
    status: ResponseStatus,
) -> AnyView {
    let list = match result {
        Ok(list) => list,
        Err(error) => {
            status.set(if error == LoadError::Invalid {
                422
            } else {
                503
            });
            return view! {
                <Title text="Chưa tải được kết quả — Vink" />
                <Meta name="robots" content="noindex" />
                <Link rel="canonical" href=crate::canonical_url(&context.tag_page.as_deref().map(tag_href).unwrap_or_else(|| "/".into())) />
                <header id="explore-heading" class="vk-explore__heading"><Heading level=HeadingLevel::H1 role=TypeRole::Display>"Khám phá"</Heading></header>
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
    let is_tag = context.tag_page.is_some();
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
            {(!is_tag).then(|| view! { <div id="discovery-tools" class="vk-discovery-tools"><a class="vk-brand" href="/" aria-label="Vink — trang chủ">
                <span class="vk-brand__lockup" aria-hidden="true"></span></a></div> })}
            {is_tag.then(|| view! { <a class="vk-ui-link" href="/">"Về Khám phá"</a> })}
            <Heading level=HeadingLevel::H1 role=TypeRole::Display>{move || title.get()}</Heading>
            {move || if is_tag {
                match tag.get() {
                    Some(filter) if filter.resolution == "not_found" => view! { <p role="status">"Không tìm thấy thẻ này. Hãy quay về Khám phá để chọn thẻ khác."</p> }.into_any(),
                    Some(filter) => {
                        let retired = filter.resolution == "retired";
                        let tag = filter.tag;
                        view! {
                            {tag.as_ref().and_then(|t| t.description.clone()).map(|text| view! { <p>{text}</p> })}
                            {retired.then(|| view! { <p role="status">"Thẻ đã ngừng dùng; không còn dùng để tìm tác phẩm công khai."</p> })}
                            {tag.and_then(|t| t.successor).map(|next| view! { <a class="vk-ui-link" href=tag_href(&next.slug)>{format!("Xem thẻ {}", next.label)}</a> })}
                        }.into_any()
                    }
                    None => ().into_any(),
                }
            } else { view! { <p>"Tìm tác phẩm theo tên, tác giả hoặc thẻ."</p> }.into_any() }}
        </header>
    }.into_any()
}

#[component]
fn AppliedFilters(context: ExploreContext, list: WorkList) -> AnyView {
    let live = public_reads::follow_works(context.query.clone(), list);
    view! {
        {move || live.data.with(|list| list.filters.tags.iter().filter(|filter| {
            !context.tag_page.as_ref().is_some_and(|slug| filter.requested == requested_slug(slug))
        }).map(|filter| {
            let mut without = context.clone();
            without.query = without.query.without_tag(&filter.requested);
            let label = filter.tag.as_ref().map(|t| t.label.clone()).unwrap_or_else(|| filter.requested.clone());
            let mut facts = TagFacts::active(&filter.requested, &label);
            facts.retired = filter.resolution == "retired";
            if filter.resolution == "not_found" { facts.problem = Some("Không tìm thấy thẻ này.".into()); }
            view! { <a class="vk-applied-tag" href=without.href(None) aria-label=format!("Bỏ lọc {label}")>
                <TagChip id=format!("applied-{}", crate::discovery::encode(&filter.requested)) tag=facts remove_label=format!("Bỏ lọc {label}") />
                <Icon name=IconName::Close />
            </a> }
        }).collect_view())}
    }.into_any()
}

#[component]
fn FilterForm(
    id: &'static str,
    action: String,
    q: String,
    draft: RwSignal<Vec<String>>,
    catalog: Resource<Result<TagList, LoadError>>,
    open: RwSignal<bool>,
) -> AnyView {
    let focus = expect_context::<DiscoveryFocus>().0;
    let mut initial = expect_context::<DiscoveryDraft>().applied_tags.get_value();
    initial.sort();
    view! {
        <Form method="get" action noscroll=true on_form_data=Arc::new(drop_unnamed_submitter)
            attr:class="vk-discovery-filter-form" attr:id=format!("{id}-tag-form")
            on:submit=move |_| { focus.set(Some("works-title")); open.set(false); }>
            <HiddenField name="q" value=Signal::stored(q) />
            <h2>"Bộ lọc thẻ"</h2>
            <p>"Kết quả cần khớp tất cả thẻ đã chọn. Tối đa 4 thẻ."</p>
            <Suspense fallback=|| view! { <p role="status">"Đang tải danh mục thẻ…"</p> }>
            {move || catalog.get().map(|catalog| match catalog {
                Err(_) => view! { <p role="alert">"Chưa tải được danh mục thẻ. Tải lại trang để thử lại; bộ lọc đang áp dụng vẫn được giữ."</p>
                    {draft.get_untracked().into_iter().map(|slug| view! { <HiddenField name="tag" value=Signal::stored(slug) /> }).collect_view()}
                }.into_any(),
                Ok(catalog) => {
                    let known: Vec<_> = catalog.tags.iter().map(|t| t.slug.clone()).collect();
                    view! {
                    {move || draft.get().into_iter().filter(|slug| !known.contains(slug))
                        .map(|slug| view! { <HiddenField name="tag" value=Signal::stored(slug) /> }).collect_view()}
                    {["form", "theme"].into_iter().map(|facet| {
                        let options: Vec<TagDto> = catalog.tags.iter().filter(|tag| tag.facet == facet).cloned().collect();
                        view! { <fieldset>
                            <legend>{if facet == "form" { "Dạng bài" } else { "Chủ đề" }}</legend>
                            <div class="vk-discovery-options">
                                {options.into_iter().map(|tag| {
                                    let slug = tag.slug.clone();
                                    let checked_slug = slug.clone();  let disabled_slug = slug.clone();
                                    let input_id = format!("{id}-tag-{}", tag.tag_id);
                                    view! { <span class="vk-discovery-option">
                                        <Checkbox id=input_id label=tag.label name="tag" value=tag.slug
                                            checked=Signal::derive(move || draft.with(|tags| tags.contains(&checked_slug)))
                                            disabled=Signal::derive(move || draft.with(|tags| tags.len() >= 4 && !tags.contains(&disabled_slug)))
                                            on_change=move |checked| {
                                                draft.update(|tags| {
                                                    tags.retain(|tag| tag != &slug);
                                                    if checked && tags.len() < 4 { tags.push(slug.clone()); }
                                                });
                                            } />
                                    </span> }
                                }).collect_view()}
                            </div>
                        </fieldset> }
                    }).collect_view()}
                    {catalog.truncated.then(|| view! { <p>"Danh mục đang hiển thị 200 thẻ đầu. Thẻ khác vẫn có thể mở từ tác phẩm hoặc đường dẫn riêng."</p> })}
                }.into_any() },
            })}
            </Suspense>
            {move || {
                let mut selected = draft.get(); selected.sort();
                view! { <Button kind=ButtonKind::Submit variant=if selected != initial { ButtonVariant::Filled } else { ButtonVariant::Tonal }>"Áp dụng"</Button> }
            }}
        </Form>
    }.into_any()
}

#[component]
fn PublicResults(context: ExploreContext, live: public_reads::Live<WorkList>) -> AnyView {
    let applied_query = context.query.q.clone();
    let data = live.data;
    let refreshing = live.refreshing;
    let failed = live.failed;
    let works = Memo::new(move |_| data.with(|list| list.works.clone()));
    #[cfg(feature = "hydrate")]
    super::restore_public_position(works);
    let list_context = context.clone();
    let page_context = context.clone();
    let empty_context = context.clone();
    view! {
        <div id="public-collection" aria-busy=move || refreshing.get().to_string() data-refreshing=move || refreshing.get().to_string()>
            <Show when=move || failed.get()>
                <div data-testid="public-refresh-failed"><InlineNotice tone=Tone::Warning
                    message="Chưa làm mới được kết quả. Bạn vẫn đang xem bản đã tải cho cùng bộ lọc."
                    action=RecoveryAction { intent: RecoveryIntent::Retry, label: "Thử lại".into() }
                    on_recover={ let live = live.clone(); move |_| live.refresh() } /></div>
            </Show>
            <div id="public-results">
                <p class="vk-muted vk-public-scope" role="status" aria-atomic="true">{move || if applied_query.is_empty() { format!("{} tác phẩm trên trang này.", works.read().len()) } else { format!("{} tác phẩm trên trang này cho “{}”.", works.read().len(), applied_query) }}</p>
                {move || if works.read().is_empty() {
                    let filtered = !empty_context.query.tags.is_empty() || !empty_context.query.q.is_empty();
                    let mut no_tags = empty_context.clone(); no_tags.query.tags.clear(); no_tags.query.cursor = None; no_tags.tag_page = None;
                    let mut no_query = empty_context.clone(); no_query.query.q.clear(); no_query.query.cursor = None;
                    view! { <div data-testid=if filtered { "public-no-results" } else { "public-empty" }>
                        <h3>{if filtered { "Không tìm thấy tác phẩm phù hợp." } else { "Chưa có tác phẩm nào được công khai." }}</h3>
                        <p>{if filtered { "Thử từ khóa khác hoặc bỏ bớt bộ lọc." } else { "Bạn có thể bắt đầu một bản nháp ở Bàn viết." }}</p>
                        {(!empty_context.query.tags.is_empty()).then(|| view! { <a id="clear-public-tags" class="vk-ui-link" href=no_tags.href(None)>"Bỏ lọc thẻ"</a> })}
                        {(!empty_context.query.q.is_empty()).then(|| view! { <a id="clear-public-results" class="vk-ui-link" href=no_query.href(None)>"Xóa tìm kiếm"</a> })}
                    </div> }.into_any()
                } else {
                    let context = list_context.clone();
                    view! { <ul class="vk-public-list" role="list">
                        <For each=move || works.with(|works| works.iter().map(|work| work.work_id.clone()).collect::<Vec<_>>()) key=Clone::clone
                            children=move |work_id| view! { <ResultRow work_id data context=context.clone() /> } />
                    </ul> }.into_any()
                }}
                <nav class="vk-public-pagination" aria-label="Trang kết quả">
                    {move || data.with(|list| list.next_cursor.clone()).map(|cursor| {
                        let mut next = page_context.clone(); next.query.cursor = Some(cursor);
                        view! { <a id="public-next" class="vk-ui-link" href=next.href(None)>"Trang tiếp →"</a> }
                    })}
                    {context.query.cursor.is_some().then(|| {
                        let mut first = context.clone(); first.query.cursor = None;
                        view! { <a id="public-first" class="vk-ui-link" href=first.href(None)>"Về trang đầu"</a> }
                    })}
                </nav>
            </div>
        </div>
    }.into_any()
}

#[component]
fn ResultRow(work_id: String, data: Signal<WorkList>, context: ExploreContext) -> AnyView {
    let id = work_id.clone();
    let work = Memo::new(move |_| {
        data.with(|list| list.works.iter().find(|work| work.work_id == id).cloned())
    });
    view! {
        <li id=crate::public_list::work_anchor(&work_id) class="vk-public-row" data-work-id=work_id>
            {move || work.get().map(|work| {
                let href = context.reader_href(&work.work_id);
                #[cfg(feature = "hydrate")]
                let context = context.clone();
                #[cfg(feature = "hydrate")]
                let return_id = work.work_id.clone();
                let tags = work.tags.iter().map(|tag| tag.label.clone()).collect();
                view! { <WorkCardPattern title=work.title.clone() author=adapters::card_author(&work) href=Signal::stored(href) tags
                    on_open=move |_event| {
                        #[cfg(feature = "hydrate")]
                        super::remember_public_position(&_event, &context, &return_id);
                    } /> }
            })}
        </li>
    }.into_any()
}

#[component]
pub(super) fn PublicTags(tags: Vec<PublicTagDto>) -> impl IntoView {
    view! { <ul class="vk-reader-tags" aria-label="Thẻ tác phẩm">
        {tags.into_iter().map(|tag| view! { <li><a class="vk-ui-link" href=tag_href(&tag.slug)>{tag.label}</a></li> }).collect_view()}
    </ul> }
}

#[component]
fn DiscoveryFailure(context: ExploreContext, invalid: bool) -> AnyView {
    let mut first = context.clone();
    first.query.cursor = None;
    view! {
        {if invalid { view! { <div role="alert" data-testid="public-invalid"><p>"Đường dẫn tìm kiếm hoặc trang kết quả không còn hợp lệ."</p></div> }.into_any() }
        else { view! { <Unavailable /> }.into_any() }}
        <a id="restart-public" class="vk-ui-link" href=first.href(None)>"Thử từ trang đầu"</a>
        <a class="vk-ui-link" href="/">"Bỏ toàn bộ bộ lọc"</a>
    }.into_any()
}

/// The draft selection is shared. When reflow hides a focused presentation, carry focus to its
/// paired checkbox, rather than leaving it in a hidden dock or behind the modal.
#[cfg(feature = "hydrate")]
fn adapt_picker(open: RwSignal<bool>) {
    use wasm_bindgen::{JsCast, closure::Closure};
    Effect::new(move |_| {
        let Some(dock) = document().get_element_by_id("tag-dock") else {
            return;
        };
        let callback =
            Closure::<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>::new(move |_, _| {
                let Some(dock) = document().get_element_by_id("tag-dock") else {
                    return;
                };
                let wide = dock.get_bounding_client_rect().width() > 0.0;
                let active = document().active_element();
                let tag = active.as_ref().and_then(|el| {
                    let id = el.id();
                    id.strip_prefix("dock-tag-")
                        .or_else(|| id.strip_prefix("modal-tag-"))
                        .map(str::to_owned)
                });
                let in_dock = active.as_ref().is_some_and(|el| dock.contains(Some(el)));
                let destination = if wide && open.get_untracked() {
                    open.set(false);
                    Some("dock")
                } else if !wide && in_dock {
                    open.set(true);
                    Some("modal")
                } else {
                    None
                };
                if let Some(target) = destination
                    .zip(tag)
                    .map(|(part, id)| format!("{part}-tag-{id}"))
                {
                    request_animation_frame(move || {
                        if let Some(el) = document()
                            .get_element_by_id(&target)
                            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
                        {
                            let _ = el.focus();
                        }
                    });
                }
            });
        if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            observer.observe(&dock);
            let cleanup = send_wrapper::SendWrapper::new((observer, callback));
            on_cleanup(move || {
                let (observer, callback) = cleanup.take();
                observer.disconnect();
                drop(callback);
            });
        }
    });
}
