//! Social Follow Graph UI views (#294): action control, counts/links and paginated list pages.

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::{use_params_map, use_query_map};
use vink_ui::{
    ActionState, AvatarSize, Button, ButtonVariant, Heading, HeadingLevel, Link as UiLink,
    LinkVariant,
};

use crate::ui::patterns::{Author as BylineAuthor, AuthorIdentity, ProfileLink};

use crate::account::Session;
use crate::contract::PublicAuthorRef;
use crate::contract::follow::{
    AuthorRelationshipsLoad, FollowDirection, OwnRelationshipsSummary, PublicRelationships,
};
use crate::follow_state::{format_vi_count, safe_profile_return};
use crate::session::Standing;

#[cfg(feature = "hydrate")]
mod browser;
#[cfg(feature = "hydrate")]
pub(super) use browser::{announce_follow_changed, generate_command_id, observe_changes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RelationshipPersonRow {
    pub author: PublicAuthorRef,
    pub mutual: bool,
    pub return_cursor: Option<String>,
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
    Alias {
        href: String,
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

impl From<AuthorRelationshipsLoad> for LiveListState {
    fn from(loaded: AuthorRelationshipsLoad) -> Self {
        match loaded {
            AuthorRelationshipsLoad::Page {
                author,
                direction,
                count,
                people,
                next_cursor,
            } => Self::Page {
                author,
                direction,
                is_own: false,
                count,
                people: people
                    .into_iter()
                    .map(|author| RelationshipPersonRow {
                        author,
                        mutual: false,
                        return_cursor: None,
                    })
                    .collect(),
                next_cursor,
                appending: false,
                append_error: false,
            },
            AuthorRelationshipsLoad::Restart { author, direction } => {
                Self::Restart { author, direction }
            }
            AuthorRelationshipsLoad::Alias { href } => Self::Alias { href },
            AuthorRelationshipsLoad::Private => Self::Private,
            AuthorRelationshipsLoad::Gone => Self::Gone,
            AuthorRelationshipsLoad::Missing => Self::Missing,
        }
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RelationshipSummary {
    Ready(PublicRelationships),
    #[cfg(feature = "hydrate")]
    Reading,
    #[cfg(feature = "hydrate")]
    Unavailable,
}

#[component]
pub(super) fn ProfileActionRegion(
    handle: String,
    on_summary: Callback<Option<PublicRelationships>>,
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
    let _ = (on_summary, own_relationships);

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
                                        on_summary
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
    public: Signal<RelationshipSummary>,
    own: ReadSignal<Option<(String, OwnRelationshipsSummary)>>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let session = Session::current();
    let summary = Signal::derive(move || {
        if let Some((scope, own_rel)) = own.get()
            && session.profile_scope.get().as_deref() == Some(scope.as_str())
        {
            Ok((
                Some(own_rel.followers_count),
                own_rel.valid_followers_href().map(str::to_owned),
                Some(own_rel.following_count),
                own_rel.valid_following_href().map(str::to_owned),
            ))
        } else {
            #[cfg(not(feature = "hydrate"))]
            let RelationshipSummary::Ready(pub_rel) = public.get();
            #[cfg(feature = "hydrate")]
            let pub_rel = match public.get() {
                RelationshipSummary::Ready(rel) => rel,
                RelationshipSummary::Reading => return Err(false),
                RelationshipSummary::Unavailable => return Err(true),
            };
            Ok((
                pub_rel.followers_count,
                pub_rel.valid_followers_href().map(str::to_owned),
                pub_rel.following_count,
                pub_rel.valid_following_href().map(str::to_owned),
            ))
        }
    });

    move || {
        let (f_count, f_href, g_count, g_href) = match summary.get() {
            Ok(facts) => facts,
            Err(failed) => return view! {
                <div id="profile-relationships" class="vk-profile-relationships">
                    <p role="status">{if failed { "Chưa tải được danh sách và số đếm theo dõi." }
                        else { "Đang cập nhật danh sách và số đếm theo dõi…" }}</p>
                    {failed.then(|| view! {
                        <Button id="profile-relationships-retry" variant=ButtonVariant::Tonal on_press=on_retry>
                            "Thử lại thông tin theo dõi"
                        </Button>
                    })}
                </div>
            }.into_any(),
        };
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
pub fn FollowersList() -> impl IntoView {
    view! { <RelationshipList direction=FollowDirection::Followers /> }
}

#[component]
pub fn FollowingList() -> impl IntoView {
    view! { <RelationshipList direction=FollowDirection::Following /> }
}

#[component]
fn RelationshipList(direction: FollowDirection) -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    view! {
        <For
            each=move || {
                vec![(
                    params.read().get("author").unwrap_or_default(),
                    query.read().get("cursor"),
                )]
            }
            key=Clone::clone
            children=move |(author, cursor)| match author.strip_prefix('@') {
                Some(name) => view! {
                    <RelationshipListPage name=name.to_string() direction cursor />
                }.into_any(),
                None => view! { <super::NotFound /> }.into_any(),
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
    let (generation, _advance) = signal(0u64);
    let live_state: RwSignal<Option<LiveListState>> = RwSignal::new(None);
    let hydrated = RwSignal::new(false);
    let append_trigger = RwSignal::new(0u64);
    let reload_trigger = RwSignal::new(0u64);
    let first_trigger = RwSignal::new(0u64);

    let data = crate::api::author_relationships_resource(
        name.clone(),
        direction,
        cursor.clone(),
        generation,
    );
    let status = super::ResponseStatus::capture();
    status.set_no_store();

    let renew = move || {
        #[cfg(feature = "hydrate")]
        reload_trigger.update(|n| *n += 1);
        #[cfg(not(feature = "hydrate"))]
        _advance.update(|n| *n += 1);
    };

    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |_| {
            hydrated.set(true);
        });
    }

    let name_for_view = StoredValue::new(name);
    let route_cursor = StoredValue::new(cursor.clone());

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
                            first_trigger
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
                {move || data.get().map(|(_, result)| view! {
                    <RelationshipListProjection
                        name=name_for_view.get_value()
                        direction
                        cursor=route_cursor.get_value()
                        status=status.clone()
                        initial=result.map_or(LiveListState::Unavailable, LiveListState::from)
                        live=live_state
                        hydrated
                        append_generation=append_trigger.read_only()
                        on_append=Callback::new(move |()| append_trigger.update(|n| *n += 1))
                        on_retry=Callback::new(move |()| renew())
                        on_first=Callback::new(move |()| first_trigger.update(|n| *n += 1))
                    />
                })}
            </Transition>
        </div>
    }
}

#[component]
fn RelationshipListProjection(
    name: String,
    direction: FollowDirection,
    cursor: Option<String>,
    status: super::ResponseStatus,
    initial: LiveListState,
    live: RwSignal<Option<LiveListState>>,
    hydrated: RwSignal<bool>,
    append_generation: ReadSignal<u64>,
    on_append: Callback<()>,
    on_retry: Callback<()>,
    on_first: Callback<()>,
) -> impl IntoView {
    let initial = StoredValue::new(initial);
    let continued = cursor.is_some();
    let state = Memo::new(move |_| live.get().unwrap_or_else(|| initial.get_value()));
    let page_key = Memo::new(move |_| {
        state.with(|state| match state {
            LiveListState::Page { author, is_own, .. } => Some((
                author.handle.clone(),
                author.pen_name.clone(),
                author.href.clone(),
                *is_own,
            )),
            _ => None,
        })
    });

    view! {
        <For
            each=move || page_key.get().into_iter()
            key=Clone::clone
            children=move |(handle, pen_name, href, is_own)| view! {
                <RelationshipResults handle pen_name href direction is_own
                    continued state hydrated append_generation on_append on_first />
            }
        />
        {move || state.with(|state| match state {
            LiveListState::Page { .. } => ().into_any(),
            current => render_list_status(
                current.clone(),
                &status,
                &name,
                direction,
                cursor.clone(),
                move || on_retry.run(()),
            ),
        })}
    }
}

const RELATIONSHIP_PAGE_CLASS: &str =
    "vk-rel-page grid gap-vk-3 [max-inline-size:var(--vk-measure)] mx-auto py-vk-4";

#[component]
fn RelationshipResults(
    handle: String,
    pen_name: String,
    href: String,
    direction: FollowDirection,
    is_own: bool,
    continued: bool,
    state: Memo<LiveListState>,
    hydrated: RwSignal<bool>,
    append_generation: ReadSignal<u64>,
    on_append: Callback<()>,
    on_first: Callback<()>,
) -> impl IntoView {
    let title = direction_heading(direction, &pen_name);
    let list_url = StoredValue::new(format!("/@{handle}/{}", direction.web_segment()));
    let canonical = crate::canonical_url(&list_url.get_value());
    let count = Memo::new(move |_| {
        state.with(|state| match state {
            LiveListState::Page { count, .. } => *count,
            _ => None,
        })
    });
    let rows = Memo::new(move |_| {
        state.with(|state| match state {
            LiveListState::Page { people, .. } => people
                .iter()
                .filter(|row| row.author.valid_href().is_some())
                .cloned()
                .collect::<Vec<_>>(),
            _ => vec![],
        })
    });
    let has_previous_page = Memo::new(move |_| {
        continued
            || rows.with(|people| {
                people
                    .first()
                    .is_some_and(|row| row.return_cursor.is_some())
            })
    });
    let cursor = Memo::new(move |_| {
        state.with(|state| match state {
            LiveListState::Page { next_cursor, .. } => next_cursor.clone(),
            _ => None,
        })
    });
    let failed = Memo::new(move |_| {
        matches!(
            &*state.read(),
            LiveListState::Page {
                append_error: true,
                ..
            }
        )
    });
    let pending = Memo::new(move |_| {
        matches!(
            &*state.read(),
            LiveListState::Page {
                appending: true,
                ..
            }
        )
    });
    let button_state = Signal::derive(move || {
        if pending.get() {
            ActionState::pending("Đang tải thêm…")
        } else {
            ActionState::Enabled
        }
    });

    view! {
        <Title text=format!("{title} · Vink") />
        {if is_own {
            view! { <Meta name="robots" content="noindex" /> }.into_any()
        } else {
            view! { <Link rel="canonical" href=canonical /> }.into_any()
        }}
        <section id="relationship-list" class=RELATIONSHIP_PAGE_CLASS aria-labelledby="relationship-title">
            <UiLink
                variant=LinkVariant::Standalone
                attr:id="rel-back-profile"
                attr:data-profile-return="true"
                href=href>
                "Quay lại hồ sơ"
            </UiLink>
            <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                {title}
            </Heading>
            <RelationshipDirections handle direction is_own append_generation />
            {move || count.get().map(|n| view! {
                <p id="relationship-count" class="vk-rel-count m-0 text-vk-on-surface-variant">{count_copy(direction, n)}</p>
            })}
            <Show when=move || rows.with(Vec::is_empty) fallback=move || view! {
                <ul class="vk-rel-list">
                    <For
                        each=move || rows.get()
                        key=|row| (row.author.handle.clone(), row.author.pen_name.clone(),
                            row.author.href.clone(), row.mutual, row.return_cursor.clone())
                        children=move |row| {
                            let row_id = format!("rel-person-{}", row.author.handle);
                            let author = BylineAuthor {
                                name: row.author.pen_name,
                                key: None,
                                published_at: String::new(),
                                profile: Some(ProfileLink { href: row.author.href, handle: row.author.handle }),
                                neutral: false,
                            };
                            view! {
                                <li class="vk-rel-item" data-rel-cursor=row.return_cursor>
                                    <AuthorIdentity author id=Some(row_id) size=AvatarSize::Medium />
                                    {(is_own && row.mutual).then(|| view! {
                                        <span class="vk-follow-mutual">"Theo dõi lẫn nhau"</span>
                                    })}
                                </li>
                            }
                        }
                    />
                </ul>
            }>
                <p id="relationship-empty" class="vk-rel-empty">{empty_copy(direction, continued)}</p>
            </Show>
            <Show when=move || has_previous_page.get()>
                <UiLink variant=LinkVariant::Standalone attr:id="rel-first-page"
                    attr:data-profile-return="true" href=list_url.get_value()
                    on:click=move |event: leptos::ev::MouseEvent| {
                        // A Back-resumed page can share the first page's address.
                        if !continued && event.button() == 0 && !event.ctrl_key() && !event.meta_key()
                            && !event.shift_key() && !event.alt_key()
                        {
                            event.prevent_default();
                            on_first.run(());
                        }
                    }>
                    "Về trang đầu"
                </UiLink>
            </Show>
            <Show when=move || cursor.with(Option::is_some)>
                <div class="vk-rel-pagination">
                    <Show when=move || hydrated.get() fallback=move || {
                        cursor.get().map(|c| view! {
                            <UiLink variant=LinkVariant::Standalone attr:id="rel-next-link"
                                href=format!("{}?cursor={c}", list_url.get_value())>
                                "Trang tiếp"
                            </UiLink>
                        })
                    }>
                        <p class="vk-rel-append-error m-0 text-vk-error" role="status">
                            {move || if failed.get() {
                                "Chưa tải thêm được danh sách. Bạn có thể thử lại tại đây."
                            } else { "" }}
                        </p>
                        <Button id="rel-load-more" variant=ButtonVariant::Tonal state=button_state on_press=on_append>
                            {move || if failed.get() { "Thử lại tải thêm" } else { "Tải thêm" }}
                        </Button>
                    </Show>
                </div>
            </Show>
        </section>
    }
}

const RELATIONSHIP_DIRECTIONS_CLASS: &str =
    "vk-rel-directions flex flex-wrap items-center gap-x-vk-4 gap-y-vk-2";

// The public author projection already carries the two independent list grants.
#[component]
fn RelationshipDirections(
    handle: String,
    direction: FollowDirection,
    is_own: bool,
    append_generation: ReadSignal<u64>,
) -> impl IntoView {
    let (opposite, current_label, other_label) = match direction {
        FollowDirection::Followers => (
            FollowDirection::Following,
            "Người theo dõi",
            "Đang theo dõi",
        ),
        FollowDirection::Following => (
            FollowDirection::Followers,
            "Đang theo dõi",
            "Người theo dõi",
        ),
    };
    let expected = format!("/@{handle}/{}", opposite.web_segment());
    let (generation, advance) = signal(0);
    Effect::new(move |previous: Option<u64>| {
        let append = append_generation.get();
        if previous.is_some_and(|old| old != append) {
            advance.update(|n| *n += 1);
        }
        append
    });
    let data = (!is_own).then(|| crate::api::author_resource(handle.clone(), None, generation));
    view! {
        <Suspense fallback=|| ()>
            {move || {
                let result = if is_own { Some(Ok(true)) } else {
                    data.and_then(|read| read.get()).and_then(|(stamp, result)| {
                        (stamp == generation.get()).then(|| match result {
                            Ok(crate::contract::AuthorResolution::Current { author, relationships, .. })
                                if author.handle == handle => {
                                let href = match opposite {
                                    FollowDirection::Followers => relationships.valid_followers_href(),
                                    FollowDirection::Following => relationships.valid_following_href(),
                                };
                                Ok(href == Some(expected.as_str()))
                            }
                            Ok(_) => Ok(false),
                            Err(_) => Err(()),
                        })
                    })
                };
                match result {
                    Some(Ok(true)) => view! {
                        <nav class=RELATIONSHIP_DIRECTIONS_CLASS aria-label="Danh sách theo dõi">
                            <span aria-current="page">{current_label}</span>
                            <UiLink variant=LinkVariant::Standalone attr:id="rel-other-direction"
                                attr:data-profile-return="true" href=expected.clone()>
                                {other_label}
                            </UiLink>
                        </nav>
                    }.into_any(),
                    Some(Err(())) => view! {
                        <div class=RELATIONSHIP_DIRECTIONS_CLASS>
                            <p role="status">"Chưa kiểm tra được danh sách theo dõi còn lại."</p>
                            <Button id="rel-direction-retry" variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |()| advance.update(|n| *n += 1))>
                                "Thử lại"
                            </Button>
                        </div>
                    }.into_any(),
                    _ => ().into_any(),
                }
            }}
        </Suspense>
    }
}

fn render_list_status(
    state: LiveListState,
    status: &super::ResponseStatus,
    name: &str,
    direction: FollowDirection,
    cursor: Option<String>,
    retry_initial: impl Fn() + Copy + Send + Sync + 'static,
) -> AnyView {
    match state {
        #[cfg(feature = "hydrate")]
        LiveListState::Loading => view! { <p role="status">"Đang tải danh sách…"</p> }.into_any(),
        LiveListState::Page { .. } => ().into_any(),
        LiveListState::Alias { href } => {
            super::handle::redirect_alias(
                status,
                &format!("/@{name}/{}", direction.web_segment()),
                &href,
            );
            view! { <UiLink href=href>"Mở địa chỉ hiện tại"</UiLink> }.into_any()
        }
        LiveListState::Restart { author, direction } => {
            let list_url = format!("/@{}/{}", author.handle, direction.web_segment());
            view! {
                <Title text="Danh sách theo dõi · Vink" />
                <section id="relationship-list" class=RELATIONSHIP_PAGE_CLASS aria-labelledby="relationship-title">
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
                <section id="relationship-list" class=RELATIONSHIP_PAGE_CLASS aria-labelledby="relationship-title">
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
                <section id="relationship-list" class=RELATIONSHIP_PAGE_CLASS aria-labelledby="relationship-title">
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
                <section id="relationship-list" class=RELATIONSHIP_PAGE_CLASS aria-labelledby="relationship-title">
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
            let path = format!("/@{name}/{}", direction.web_segment());
            view! {
                <Title text="Chưa tải được danh sách · Vink" />
                <section id="relationship-list" class=RELATIONSHIP_PAGE_CLASS aria-labelledby="relationship-title">
                    <Heading id="relationship-title" level=HeadingLevel::H1 attr:tabindex="-1">
                        "Chưa tải được danh sách"
                    </Heading>
                    <p>"Hãy kiểm tra kết nối rồi thử lại."</p>
                    <super::handle::RetryRead id="rel-retry" path cursor
                        on_retry=Callback::new(move |()| retry_initial()) />
                </section>
            }
            .into_any()
        }
    }
}
