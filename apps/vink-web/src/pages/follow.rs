//! Social Follow Graph UI views (#294): action control, counts/links and paginated list pages.

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::{use_params_map, use_query_map};
use vink_ui::{ButtonVariant, Heading, HeadingLevel, Link as UiLink, LinkVariant};

use crate::account::Session;
use crate::contract::PublicAuthorRef;
use crate::contract::follow::{FollowDirection, OwnRelationshipsSummary, PublicRelationships};
use crate::follow_state::{format_vi_count, safe_profile_return};
use crate::session::Standing;

#[cfg(feature = "hydrate")]
mod browser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RelationshipPersonRow {
    pub author: PublicAuthorRef,
    pub mutual: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LiveListState {
    #[cfg(feature = "hydrate")]
    Loading,
    Page {
        author: PublicAuthorRef,
        direction: FollowDirection,
        is_own: bool,
        count: Option<i64>,
        people: Vec<RelationshipPersonRow>,
        next_cursor: Option<String>,
        appending: bool,
        append_error: bool,
    },
    Restart {
        author: PublicAuthorRef,
        direction: FollowDirection,
    },
    Private,
    Gone,
    Missing,
    Unavailable,
}

fn encode_next_target(handle: &str) -> String {
    let raw = format!("/@{handle}");
    match safe_profile_return(&raw) {
        Some(valid) => format!(
            "/toi?next={}",
            percent_encoding::utf8_percent_encode(&valid, percent_encoding::NON_ALPHANUMERIC)
        ),
        None => "/toi".to_string(),
    }
}

#[component]
pub(super) fn ProfileActionRegion(
    handle: String,
    live_override: RwSignal<Option<(String, PublicRelationships)>>,
    own_relationships: RwSignal<Option<(String, OwnRelationshipsSummary)>>,
) -> impl IntoView {
    let session = Session::current();
    let sign_in_open = crate::api::sign_in_open();
    let stored_handle = StoredValue::new(handle);

    #[cfg(feature = "hydrate")]
    Effect::new(move |_| {
        if session.profile_scope.get().is_none() {
            own_relationships.set(None);
        }
    });
    #[cfg(not(feature = "hydrate"))]
    let _ = (live_override, own_relationships);

    view! {
        <div id="follow" class="vk-follow-region">
            {move || match session.standing.get() {
                Standing::Anonymous if !sign_in_open => view! {
                    <p id="follow-sign-in-closed" class="vk-muted">"Vink chưa mở đăng nhập."</p>
                }.into_any(),
                Standing::Anonymous => {
                    let href = encode_next_target(&stored_handle.get_value());
                    view! {
                        <UiLink
                            variant=LinkVariant::Standalone
                            attr:id="follow-sign-in"
                            attr:data-profile-return="true"
                            href=href>
                            "Đăng nhập để theo dõi"
                        </UiLink>
                    }.into_any()
                }
                Standing::NeedsPrincipal => {
                    let href = encode_next_target(&stored_handle.get_value());
                    view! {
                        <UiLink
                            variant=LinkVariant::Standalone
                            attr:id="follow-sign-in"
                            attr:data-profile-return="true"
                            href=href>
                            "Hoàn tất bút danh để theo dõi"
                        </UiLink>
                    }.into_any()
                }
                Standing::Active(_) => {
                    #[cfg(feature = "hydrate")]
                    {
                        let scoped_session = move || session.profile_scope.get().into_iter().collect::<Vec<_>>();
                        view! {
                            <For
                                each=scoped_session
                                key=Clone::clone
                                children=move |scope| view! {
                                    <browser::ActiveFollowRegion
                                        scope
                                        handle=stored_handle.get_value()
                                        live_override
                                        own_relationships
                                    />
                                }
                            />
                        }.into_any()
                    }
                    #[cfg(not(feature = "hydrate"))]
                    {
                        view! { <span class="vk-follow-ssr-placeholder"></span> }.into_any()
                    }
                }
                Standing::Unknown | Standing::Suspended | Standing::Unavailable => {
                    view! { <span class="vk-follow-ssr-placeholder"></span> }.into_any()
                }
            }}
        </div>
    }
}

#[component]
pub(super) fn ProfileRelationships(
    public: Signal<PublicRelationships>,
    own: ReadSignal<Option<(String, OwnRelationshipsSummary)>>,
) -> impl IntoView {
    let session = Session::current();
    let summary = Signal::derive(move || {
        if let Some((scope, own_rel)) = own.get()
            && session.profile_scope.get().as_deref() == Some(scope.as_str())
        {
            (
                Some(own_rel.followers_count),
                own_rel.valid_followers_href().map(str::to_owned),
                Some(own_rel.following_count),
                own_rel.valid_following_href().map(str::to_owned),
            )
        } else {
            let pub_rel = public.get();
            (
                pub_rel.followers_count,
                pub_rel.valid_followers_href().map(str::to_owned),
                pub_rel.following_count,
                pub_rel.valid_following_href().map(str::to_owned),
            )
        }
    });

    move || {
        let (f_count, f_href, g_count, g_href) = summary.get();
        if f_count.is_none() && f_href.is_none() && g_count.is_none() && g_href.is_none() {
            return view! { <span></span> }.into_any();
        }
        view! {
            <nav id="profile-relationships" class="vk-profile-relationships" aria-label="Quan hệ theo dõi">
                {match (f_href, f_count) {
                    (Some(href), Some(c)) => view! {
                        <UiLink
                            variant=LinkVariant::Standalone
                            attr:id="profile-followers-link"
                            attr:data-profile-return="true"
                            href=href>
                            {format!("{} người theo dõi", format_vi_count(c))}
                        </UiLink>
                    }.into_any(),
                    (Some(href), None) => view! {
                        <UiLink
                            variant=LinkVariant::Standalone
                            attr:id="profile-followers-link"
                            attr:data-profile-return="true"
                            href=href>
                            "Người theo dõi"
                        </UiLink>
                    }.into_any(),
                    (None, Some(c)) => view! {
                        <span id="profile-followers-count" class="vk-rel-stat">
                            {format!("{} người theo dõi", format_vi_count(c))}
                        </span>
                    }.into_any(),
                    (None, None) => view! { <span></span> }.into_any(),
                }}
                {match (g_href, g_count) {
                    (Some(href), Some(c)) => view! {
                        <UiLink
                            variant=LinkVariant::Standalone
                            attr:id="profile-following-link"
                            attr:data-profile-return="true"
                            href=href>
                            {format!("{} đang theo dõi", format_vi_count(c))}
                        </UiLink>
                    }.into_any(),
                    (Some(href), None) => view! {
                        <UiLink
                            variant=LinkVariant::Standalone
                            attr:id="profile-following-link"
                            attr:data-profile-return="true"
                            href=href>
                            "Đang theo dõi"
                        </UiLink>
                    }.into_any(),
                    (None, Some(c)) => view! {
                        <span id="profile-following-count" class="vk-rel-stat">
                            {format!("{} đang theo dõi", format_vi_count(c))}
                        </span>
                    }.into_any(),
                    (None, None) => view! { <span></span> }.into_any(),
                }}
            </nav>
        }
        .into_any()
    }
}

#[component]
pub fn RelationshipList() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    view! {
        <For
            each=move || {
                vec![(
                    params.read().get("author").unwrap_or_default(),
                    params.read().get("relation").unwrap_or_default(),
                    query.read().get("cursor"),
                )]
            }
            key=Clone::clone
            children=|(author, relation, cursor)| match (
                author.strip_prefix('@'),
                FollowDirection::parse_web(&relation),
            ) {
                (Some(name), Some(direction)) => view! {
                    <RelationshipListPage name=name.to_string() direction cursor />
                }.into_any(),
                _ => view! { <super::NotFound /> }.into_any(),
            }
        />
    }
}

fn direction_heading(direction: FollowDirection, pen_name: &str) -> String {
    match direction {
        FollowDirection::Followers => format!("Người theo dõi {pen_name}"),
        FollowDirection::Following => format!("{pen_name} đang theo dõi"),
    }
}

fn count_copy(direction: FollowDirection, count: i64) -> String {
    match direction {
        FollowDirection::Followers => format!("{} người theo dõi", format_vi_count(count)),
        FollowDirection::Following => format!("{} đang theo dõi", format_vi_count(count)),
    }
}

fn empty_copy(direction: FollowDirection, continued: bool) -> &'static str {
    if continued {
        return "Trang này không còn người nào.";
    }
    match direction {
        FollowDirection::Followers => "Chưa có người theo dõi.",
        FollowDirection::Following => "Chưa theo dõi tác giả nào.",
    }
}

#[component]
fn RelationshipListPage(
    name: String,
    direction: FollowDirection,
    cursor: Option<String>,
) -> impl IntoView {
    let (generation, advance) = signal(0u64);
    let live_state: RwSignal<Option<LiveListState>> = RwSignal::new(None);
    let hydrated = RwSignal::new(false);
    let append_trigger = RwSignal::new(0u64);
    let reload_trigger = RwSignal::new(0u64);

    let data = crate::api::author_relationships_resource(
        name.clone(),
        direction,
        cursor.clone(),
        generation,
    );
    let status = super::ResponseStatus::capture();
    status.set_no_store();

    let renew = move || {
        live_state.set(None);
        advance.update(|n| *n += 1);
        reload_trigger.update(|n| *n += 1);
    };

    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |_| {
            hydrated.set(true);
        });
    }

    let name_for_view = StoredValue::new(name);
    let continued = cursor.is_some();

    view! {
        <div class="vk-rel-wrapper">
            {
                #[cfg(feature = "hydrate")]
                {
                    view! {
                        <browser::ListSyncHost
                            name=name_for_view.get_value()
                            direction
                            cursor
                            ssr_data=data
                            live_state
                            append_trigger
                            reload_trigger
                        />
                    }.into_any()
                }
                #[cfg(not(feature = "hydrate"))]
                {
                    let _ = (&cursor, &append_trigger, &reload_trigger);
                    view! { <span></span> }.into_any()
                }
            }
            <Transition fallback=|| view! { <p role="status">"Đang mở danh sách…"</p> }>
                {move || {
                    let current_name = name_for_view.get_value();
                    if let Some(current) = live_state.get() {
                        return render_list_state(
                            current,
                            &status,
                            &current_name,
                            direction,
                            continued,
                            hydrated,
                            renew,
                            move || append_trigger.update(|n| *n += 1),
                        );
                    }
                    match data.get() {
                        Some((_, Ok(loaded))) => {
                            let converted = match loaded {
                                crate::contract::follow::AuthorRelationshipsLoad::Page {
                                    author,
                                    direction,
                                    count,
                                    people,
                                    next_cursor,
                                } => LiveListState::Page {
                                    author,
                                    direction,
                                    is_own: false,
                                    count,
                                    people: people
                                        .into_iter()
                                        .map(|author| RelationshipPersonRow {
                                            author,
                                            mutual: false,
                                        })
                                        .collect(),
                                    next_cursor,
                                    appending: false,
                                    append_error: false,
                                },
                                crate::contract::follow::AuthorRelationshipsLoad::Restart {
                                    author,
                                    direction,
                                } => LiveListState::Restart { author, direction },
                                crate::contract::follow::AuthorRelationshipsLoad::Alias { href } => {
                                    #[cfg(feature = "ssr")]
                                    if let Some(opts) = &status.0
                                        && let Ok(url) = http::HeaderValue::from_str(&href)
                                    {
                                        opts.set_status(http::StatusCode::MOVED_PERMANENTLY);
                                        opts.insert_header(http::header::LOCATION, url);
                                    }
                                    #[cfg(feature = "hydrate")]
                                    {
                                        let location = window().location();
                                        let _ = location.replace(&format!(
                                            "{}{}",
                                            href,
                                            location.hash().unwrap_or_default()
                                        ));
                                    }
                                    return view! {
                                        <UiLink href=href>"Mở địa chỉ hiện tại"</UiLink>
                                    }.into_any();
                                }
                                crate::contract::follow::AuthorRelationshipsLoad::Private => {
                                    LiveListState::Private
                                }
                                crate::contract::follow::AuthorRelationshipsLoad::Gone => {
                                    LiveListState::Gone
                                }
                                crate::contract::follow::AuthorRelationshipsLoad::Missing => {
                                    LiveListState::Missing
                                }
                            };
                            render_list_state(
                                converted,
                                &status,
                                &current_name,
                                direction,
                                continued,
                                hydrated,
                                renew,
                                move || append_trigger.update(|n| *n += 1),
                            )
                        }
                        Some((_, Err(_))) => render_list_state(
                            LiveListState::Unavailable,
                            &status,
                            &current_name,
                            direction,
                            continued,
                            hydrated,
                            renew,
                            move || append_trigger.update(|n| *n += 1),
                        ),
                        None => view! { <p role="status">"Đang mở danh sách…"</p> }.into_any(),
                    }
                }}
            </Transition>
        </div>
    }
}

#[allow(clippy::too_many_arguments)]
fn render_list_state(
    state: LiveListState,
    status: &super::ResponseStatus,
    name: &str,
    direction: FollowDirection,
    continued: bool,
    hydrated: RwSignal<bool>,
    retry_initial: impl Fn() + Copy + Send + Sync + 'static,
    trigger_append: impl Fn() + Copy + Send + Sync + 'static,
) -> AnyView {
    match state {
        #[cfg(feature = "hydrate")]
        LiveListState::Loading => view! { <p role="status">"Đang tải danh sách…"</p> }.into_any(),
        LiveListState::Page {
            author,
            direction,
            is_own,
            count,
            people,
            next_cursor,
            appending,
            append_error,
        } => {
            let profile_href = author
                .valid_href()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("/@{name}"));
            let title = direction_heading(direction, &author.pen_name);
            let list_url = format!("/@{}/{}", author.handle, direction.web_segment());
            let canonical = crate::canonical_url(&list_url);
            let items = people
                .into_iter()
                .filter_map(|row| {
                    let href = row.author.valid_href()?.to_owned();
                    let row_id = format!("rel-person-{}", row.author.handle);
                    let handle_tag = format!("@{}", row.author.handle);
                    let mutual_badge = (is_own && row.mutual).then(|| {
                        view! { <span class="vk-follow-mutual">"Theo dõi lẫn nhau"</span> }
                    });
                    Some(view! {
                        <li class="vk-rel-item">
                            <UiLink
                                variant=LinkVariant::Standalone
                                attr:id=row_id
                                attr:data-profile-return="true"
                                href=href>
                                <span class="vk-rel-name">{row.author.pen_name}</span>
                                " "
                                <span class="vk-rel-handle">{handle_tag}</span>
                            </UiLink>
                            {mutual_badge}
                        </li>
                    })
                })
                .collect_view();
            let is_empty = items.is_empty();
            let ssr_next_href = next_cursor
                .as_ref()
                .map(|c| format!("{list_url}?cursor={c}"));

            view! {
                <Title text=format!("{title} · Vink") />
                {if is_own {
                    view! { <Meta name="robots" content="noindex" /> }.into_any()
                } else {
                    view! { <Link rel="canonical" href=canonical /> }.into_any()
                }}
                <section id="relationship-list" class="vk-rel-page" aria-labelledby="relationship-title">
                    <UiLink
                        variant=LinkVariant::Standalone
                        attr:id="rel-back-profile"
                        attr:data-profile-return="true"
                        href=profile_href>
                        "Quay lại hồ sơ"
                    </UiLink>
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        {title}
                    </Heading>
                    {count.map(|n| view! {
                        <p id="relationship-count" class="vk-rel-count">{count_copy(direction, n)}</p>
                    })}
                    {is_empty.then(|| view! {
                        <p id="relationship-empty" class="vk-rel-empty">{empty_copy(direction, continued)}</p>
                    })}
                    {(!is_empty).then(|| view! { <ul class="vk-rel-list">{items}</ul> })}
                    {append_error.then(|| view! {
                        <div class="vk-rel-append-error" role="alert">
                            <p>"Chưa tải thêm được danh sách. Các dòng đã tải vẫn được giữ nguyên."</p>
                            <vink_ui::Button
                                id="rel-append-retry"
                                variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |()| trigger_append())>
                                "Thử lại tải thêm"
                            </vink_ui::Button>
                        </div>
                    })}
                    {next_cursor.is_some().then(|| {
                        view! {
                            <div class="vk-rel-pagination">
                                {move || if hydrated.get() {
                                    let btn_state = if appending {
                                        vink_ui::ActionState::pending("Đang tải thêm…")
                                    } else {
                                        vink_ui::ActionState::Enabled
                                    };
                                    view! {
                                        <vink_ui::Button
                                            id="rel-load-more"
                                            variant=ButtonVariant::Tonal
                                            state=btn_state
                                            on_press=Callback::new(move |()| trigger_append())>
                                            "Tải thêm"
                                        </vink_ui::Button>
                                    }.into_any()
                                } else {
                                    ssr_next_href.clone().map(|href| view! {
                                        <UiLink variant=LinkVariant::Standalone attr:id="rel-next-link" href=href>
                                            "Trang tiếp"
                                        </UiLink>
                                    }).into_any()
                                }}
                            </div>
                        }
                    })}
                </section>
            }
            .into_any()
        }
        LiveListState::Restart { author, direction } => {
            let list_url = format!("/@{}/{}", author.handle, direction.web_segment());
            view! {
                <Title text="Danh sách theo dõi · Vink" />
                <section id="relationship-list" class="vk-rel-page" aria-labelledby="relationship-title">
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        "Danh sách đã thay đổi"
                    </Heading>
                    <p role="alert">"Vị trí trang không còn khớp với danh sách hiện tại."</p>
                    <UiLink variant=LinkVariant::Standalone href=list_url>"Về trang đầu"</UiLink>
                </section>
            }
            .into_any()
        }
        LiveListState::Private => {
            status.set(403);
            let back_href = safe_profile_return(&format!("/@{name}")).unwrap_or_else(|| "/".into());
            view! {
                <Title text="Danh sách riêng tư · Vink" />
                <Meta name="robots" content="noindex" />
                <section id="relationship-list" class="vk-rel-page" aria-labelledby="relationship-title">
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        "Danh sách ở chế độ Chỉ mình tôi"
                    </Heading>
                    <p id="relationship-private">"Tác giả đang đặt danh sách này ở chế độ riêng tư."</p>
                    <UiLink variant=LinkVariant::Standalone href=back_href>"Về trang hồ sơ"</UiLink>
                </section>
            }
            .into_any()
        }
        LiveListState::Gone => {
            status.set(410);
            view! {
                <Title text="Hồ sơ không còn trên Vink" />
                <section id="relationship-list" class="vk-rel-page" aria-labelledby="relationship-title">
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        "Hồ sơ không còn trên Vink"
                    </Heading>
                    <p>"Tài khoản của tác giả đã được xóa."</p>
                    <UiLink variant=LinkVariant::Standalone href="/">"Khám phá"</UiLink>
                </section>
            }
            .into_any()
        }
        LiveListState::Missing => {
            status.set(404);
            view! {
                <Title text="Không tìm thấy hồ sơ · Vink" />
                <section id="relationship-list" class="vk-rel-page" aria-labelledby="relationship-title">
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        "Không tìm thấy hồ sơ"
                    </Heading>
                    <p>"Hãy kiểm tra lại địa chỉ trong liên kết."</p>
                    <UiLink variant=LinkVariant::Standalone href="/">"Khám phá"</UiLink>
                </section>
            }
            .into_any()
        }
        LiveListState::Unavailable => {
            status.set(503);
            let fallback_href = format!("/@{name}/{}", direction.web_segment());
            view! {
                <Title text="Chưa tải được danh sách · Vink" />
                <section id="relationship-list" class="vk-rel-page" aria-labelledby="relationship-title">
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        "Chưa tải được danh sách"
                    </Heading>
                    <p>"Hãy kiểm tra kết nối rồi thử lại."</p>
                    <vink_ui::Button
                        id="rel-retry"
                        variant=ButtonVariant::Tonal
                        on_press=Callback::new(move |()| retry_initial())>
                        "Thử lại"
                    </vink_ui::Button>
                    <noscript><a href=fallback_href>"Tải lại"</a></noscript>
                </section>
            }
            .into_any()
        }
    }
}
