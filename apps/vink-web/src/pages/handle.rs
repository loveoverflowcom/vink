use crate::{
    account::Session,
    contract::{AuthorResolution, ProfileWorks},
};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::{use_params_map, use_query_map};
use vink_ui::{HiddenField, Link as UiLink};

#[component]
pub fn Author() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    view! { <For
        each=move || vec![(params.read().get("author").unwrap_or_default(), query.read().get("cursor"))]
        key=Clone::clone
        children=|(segment, cursor)| match segment.strip_prefix('@') {
            Some(name) => view!{<PublicProfile name=name.to_string() cursor />}.into_any(),
            None => view!{<super::NotFound />}.into_any()
        }
    /> }
}

#[component]
fn PublicProfile(name: String, cursor: Option<String>) -> impl IntoView {
    let (generation, advance) = signal(0u64);
    // Answers older than `floor` may still show a field that was revoked since (a profile
    // change) or the failure the reader asked to retry. Lifecycle wake-ups only revalidate:
    // the current answer stays mounted with its focus and scroll until the new one arrives.
    let (floor, set_floor) = signal(0u64);
    let own_relationships: RwSignal<
        Option<(String, crate::contract::follow::OwnRelationshipsSummary)>,
    > = RwSignal::new(None);
    let retry_focus = RwSignal::new(false);
    let renew = move || {
        advance.update(|stamp| *stamp += 1);
        set_floor.set(generation.get_untracked());
    };
    let data = crate::api::author_resource(name.clone(), cursor.clone(), generation);
    let status = super::ResponseStatus::capture();
    status.set_no_store();
    #[cfg(feature = "hydrate")]
    {
        super::profile::observe_changes(renew);
        crate::read_events::observe(move || {
            if crate::read_events::visible() {
                advance.update(|stamp| *stamp += 1);
            }
        });
    }
    view! {
        <Transition fallback=|| view! { <p role="status">"Đang mở trang tác giả…"</p> }>
            {move || match data.get() {
                Some((stamp, result)) if stamp >= floor.get() => {
                    if retry_focus.get_untracked() {
                        retry_focus.set(false);
                        super::focus_recovery("author-profile-heading");
                    }
                    render_profile(
                        result,
                        &status,
                        &name,
                        cursor.clone(),
                        own_relationships,
                        move || {
                            #[cfg(feature = "hydrate")]
                            retry_focus.set(document().active_element().is_some_and(|el| el.id() == "profile-retry"));
                            renew();
                        },
                    )
                }
                _ => view! { <p role="status">"Đang mở trang tác giả…"</p> }.into_any(),
            }}
        </Transition>
    }
}

fn render_profile(
    result: Result<AuthorResolution, crate::api::LoadError>,
    status: &super::ResponseStatus,
    name: &str,
    cursor: Option<String>,
    own_relationships: RwSignal<Option<(String, crate::contract::follow::OwnRelationshipsSummary)>>,
    retry: impl Fn() + Copy + Send + Sync + 'static,
) -> AnyView {
    let continued = cursor.is_some();
    let (code, heading, detail) = match result {
        Ok(AuthorResolution::Current {
            author,
            bio,
            publications,
            has_public_works,
            relationships,
        }) if author.valid_href().is_some() => {
            let initials = author
                .pen_name
                .split_whitespace()
                .filter_map(|s| s.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase();
            // The overlay belongs to this exact public projection. An authoritative refresh
            // may have revoked a list or count since the write; no old overlay may outlive it.
            let (public_summary, on_summary) =
                profile_summary(author.handle.clone(), *relationships);
            return view! {
                <Title text=format!("{} · Vink", author.pen_name) />
                <Link rel="canonical" href=crate::canonical_url(&author.href) />
                <article class="vk-author-public">
                    <MobileBrand />
                    <div class="vk-author-identity">
                        <span class="vk-author-seal" aria-hidden="true">{initials}</span>
                        <header>
                            <h1 id="author-profile-heading" tabindex="-1">{author.pen_name}</h1>
                            <p>{format!("@{}", author.handle)}</p>
                        </header>
                    </div>
                    <super::follow::ProfileActionRegion
                        handle=author.handle.clone()
                        on_summary
                        own_relationships=own_relationships
                    />
                    <super::follow::ProfileRelationships
                        public=public_summary.into()
                        own=own_relationships.read_only()
                        on_retry=Callback::new(move |()| on_summary.run(None))
                    />
                    {bio.map(|text| view! { <p class="vk-author-bio">{text}</p> })}
                    <section aria-labelledby="author-works-heading"
                        class="[margin-block-start:calc(var(--vk-space-8)_+_var(--vk-space-2))] [&>.vk-ui-link]:inline-block [&>.vk-ui-link]:[margin-inline-end:var(--vk-space-3)]">
                        <h2 id="author-works-heading" tabindex="-1">"Tác phẩm công khai"</h2>
                        {match publications {
                            Some(ProfileWorks::Page { works, next_cursor }) => {
                                let items = works.into_iter().map(|work| view! {
                                    <li><UiLink
                                        attr:id=format!("profile-work-{}", work.work_id)
                                        attr:data-profile-return="true"
                                        href=format!("/tac-pham/{}", work.work_id)>{work.title}</UiLink></li>
                                }).collect_view();
                                let empty = items.is_empty();
                                let next = next_cursor.map(|cursor| format!("{}?cursor={cursor}", author.href));
                                view! {
                                    {empty.then(|| view! { <p>{if continued {
                                        "Trang này không còn tác phẩm."
                                    } else { "Chưa có tác phẩm công khai." }}</p> })}
                                    {(!empty).then(|| view! { <ul class="vk-author-works [list-style:none] [margin:0_0_var(--vk-space-4)] p-0">{items}</ul> })}
                                    {next.map(|href| view! { <UiLink href>"Trang tiếp"</UiLink> })}
                                    {continued.then(|| view! { <UiLink href=author.href>"Về trang đầu"</UiLink> })}
                                }.into_any()
                            }
                            Some(ProfileWorks::Restart) => view! {
                                <p role="alert">"Trang tác phẩm không còn khớp với hồ sơ này."</p>
                                <UiLink href=author.href>"Về trang đầu"</UiLink>
                            }.into_any(),
                            None if !has_public_works => view! { <p>"Chưa có tác phẩm công khai."</p> }.into_any(),
                            None => view! {
                                <p role="alert">"Chưa tải được danh sách tác phẩm."</p>
                                <vink_ui::Button variant=vink_ui::ButtonVariant::Tonal
                                    on_press=Callback::new(move |()| retry())>"Thử lại"</vink_ui::Button>
                            }.into_any(),
                        }}
                    </section>
                </article>
            }.into_any();
        }
        Ok(AuthorResolution::Alias { href }) => {
            redirect_alias(status, &format!("/@{name}"), &href);
            return view! { <a href=href>"Mở địa chỉ hiện tại"</a> }.into_any();
        }
        Ok(AuthorResolution::Gone) => (
            410,
            "Hồ sơ không còn trên Vink",
            "Tài khoản của tác giả đã được xóa. Địa chỉ này sẽ không dẫn đến hồ sơ khác.",
        ),
        Ok(AuthorResolution::Missing) => (
            404,
            "Không tìm thấy hồ sơ",
            "Hãy kiểm tra lại địa chỉ trong liên kết hoặc quay về Khám phá.",
        ),
        _ => (
            503,
            "Chưa tải được hồ sơ",
            "Kết nối có thể bị gián đoạn hoặc máy chủ đang bận. Bạn có thể thử lại ngay tại đây.",
        ),
    };
    status.set(code);
    view! {
        <Title text=format!("{heading} · Vink") />
        <Meta name="robots" content="noindex" />
        <section id="profile-unavailable" class="vk-author-public vk-profile-unavailable"
            aria-labelledby="author-profile-heading">
            <h1 id="author-profile-heading" tabindex="-1">{heading}</h1>
            <p>{detail}</p>
            {if code == 503 {
                view! {
                    <RetryRead id="profile-retry" path=format!("/@{name}") cursor
                        on_retry=Callback::new(move |()| retry()) />
                }.into_any()
            } else {
                view! {
                    <UiLink variant=vink_ui::LinkVariant::Standalone attr:id="profile-unavailable-explore"
                        href="/">"Về Khám phá"</UiLink>
                }.into_any()
            }}
        </section>
    }
    .into_any()
}

// The public summary lives with the rendered profile. A viewer change affects the
// command/receipt and the private overlay, but the public read can still settle or retry.
fn profile_summary(
    handle: String,
    initial: crate::contract::follow::PublicRelationships,
) -> (
    ReadSignal<super::follow::RelationshipSummary>,
    Callback<Option<crate::contract::follow::PublicRelationships>>,
) {
    use super::follow::RelationshipSummary;
    let (summary, set_summary) = signal(RelationshipSummary::Ready(initial));
    #[cfg(feature = "hydrate")]
    let publish = {
        let ticket = RwSignal::new(0u64);
        Callback::new(
            move |confirmed: Option<crate::contract::follow::PublicRelationships>| {
                ticket.update(|n| *n += 1);
                match confirmed {
                    Some(relationships) => {
                        set_summary.set(RelationshipSummary::Ready(relationships))
                    }
                    None => {
                        let target = handle.clone();
                        let request = ticket.get_untracked();
                        set_summary.set(RelationshipSummary::Reading);
                        leptos::task::spawn_local(async move {
                            let result =
                                crate::api::load_author_relationships_summary(&target).await;
                            if ticket.try_get_untracked() == Some(request) {
                                set_summary.set(match result {
                                    Ok(relationships) => RelationshipSummary::Ready(relationships),
                                    Err(_) => RelationshipSummary::Unavailable,
                                });
                            }
                        });
                    }
                }
            },
        )
    };
    #[cfg(not(feature = "hydrate"))]
    let publish = Callback::new(
        move |_: Option<crate::contract::follow::PublicRelationships>| {
            let _ = (&handle, set_summary);
        },
    );
    (summary, publish)
}

// A native GET is the fallback for the same path and cursor before hydration or without JS.
#[component]
pub(super) fn RetryRead(
    id: &'static str,
    path: String,
    cursor: Option<String>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let action = crate::follow_state::safe_profile_return(&path);
    view! {
        <form method="get" action=action on:submit=move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            on_retry.run(());
        }>
            {cursor.map(|value| view! { <HiddenField name="cursor" value=Signal::stored(value) /> })}
            <vink_ui::Button id=id variant=vink_ui::ButtonVariant::Tonal kind=vink_ui::ButtonKind::Submit>
                "Thử lại"
            </vink_ui::Button>
        </form>
    }
}

pub(super) fn redirect_alias(status: &super::ResponseStatus, source_path: &str, href: &str) {
    #[cfg(feature = "ssr")]
    if let Some(opts) = &status.0
        && let Ok(url) = http::HeaderValue::from_str(href)
    {
        opts.set_status(http::StatusCode::MOVED_PERMANENTLY);
        opts.insert_header(http::header::LOCATION, url);
    }
    #[cfg(feature = "hydrate")]
    replace_profile_alias(source_path, href);
    let _ = (status, source_path, href);
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = replaceProfileAlias)]
    fn replace_profile_alias(source_path: &str, href: &str);
}

#[component]
fn MobileBrand() -> impl IntoView {
    view! { <a class="vk-brand vk-handle-brand hidden [@media(max-width:759px)]:inline-flex [@media(max-width:759px)]:[margin-block-end:1rem]" href="/" aria-label="Vink — trang chủ">
    <span class="vk-brand__lockup" aria-hidden="true"></span></a> }
}
