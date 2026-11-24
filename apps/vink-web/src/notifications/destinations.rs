use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;
use vink_ui::{Avatar, AvatarSize, Button, ButtonVariant, Dialog, IconButton, IconName};

use super::contract::{CommentDecision, Endorsers, segment};
use super::inbox::{BackButton, Failure, Loading};
use crate::account::Session;
use crate::contract::ApiError;

#[component]
pub(crate) fn CommentDecisionPage() -> impl IntoView {
    crate::pages::ResponseStatus::capture().set_no_store();
    let params = use_params_map();
    let session = Session::current();
    view! {
        <Title text="Quyết định về nhận xét — Vink" />
        <Meta name="robots" content="noindex" />
        <section class="vk-notif-page vk-notif-decision">
            <a class="vk-ui-link vk-notif-back" href=super::INBOX_PATH>"← Thông báo"</a>
            <Show when=move || session.notification_scope.with(Option::is_none)>
                {move || match session.standing.get() {
                    crate::session::Standing::Unknown | crate::session::Standing::Active(_) => view! { <Loading /> }.into_any(),
                    crate::session::Standing::Unavailable => view! {
                        <Failure message="Chưa kiểm tra được phiên đăng nhập." retry=Callback::new(move |()| session.refresh()) />
                    }.into_any(),
                    _ => view! { <Gone /> }.into_any(),
                }}
            </Show>
            <For each=move || {
                let params = params.read();
                session.profile_scope.get().zip(session.notification_scope.get())
                    .map(|(generation, scope)| (generation, scope, params.get("id").unwrap_or_default(),
                        params.get("version").unwrap_or_default()))
            } key=Clone::clone children=move |(_, scope, id, version)| view! {
                <DecisionContent scope id version />
            } />
        </section>
    }
}

#[component]
fn DecisionContent(scope: String, id: String, version: String) -> impl IntoView {
    let session = Session::current();
    let data = LocalResource::new(move || {
        let path = format!("decisions/comments/{}/{}", segment(&id), segment(&version));
        let scope = scope.clone();
        async move {
            #[cfg(feature = "hydrate")]
            {
                let result = super::browser::get::<CommentDecision>(&path, &scope).await;
                if session
                    .notification_scope
                    .try_get_untracked()
                    .flatten()
                    .as_deref()
                    != Some(&scope)
                {
                    return Err(ApiError::Unauthenticated);
                }
                if let Err(error) = &result {
                    super::browser::fence_session(session, error);
                }
                result
            }
            #[cfg(not(feature = "hydrate"))]
            {
                let _ = (session, path, scope);
                Err::<CommentDecision, _>(ApiError::Unauthenticated)
            }
        }
    });
    view! {
        <Suspense fallback=|| view! { <Loading /> }>
            {move || data.get().map(|result| match result {
                Ok(decision) => view! { <CommentDecisionDetail decision /> }.into_any(),
                Err(ApiError::NotFound | ApiError::Unauthenticated | ApiError::Forbidden) => view! { <Gone /> }.into_any(),
                Err(_) => view! { <Failure message="Chưa tải được quyết định." retry=Callback::new(move |()| data.refetch()) /> }.into_any(),
            })}
        </Suspense>
    }
}

#[component]
fn CommentDecisionDetail(decision: CommentDecision) -> impl IntoView {
    let removed = decision.outcome == "removed";
    let outcome = if removed {
        "Đã gỡ"
    } else {
        "Bị từ chối"
    };
    let title = if removed {
        "Nhận xét của bạn đã bị gỡ"
    } else {
        "Nhận xét của bạn bị từ chối"
    };
    view! {
        <article class="vk-notif-decision__panel">
            <div class="vk-notif-decision__chips">
                <span class="vk-notif-unread">{outcome}</span>
                {(!decision.is_current).then(|| view! { <span class="vk-notif-unread">"Đã có quyết định mới hơn"</span> })}
            </div>
            <h1 class="vk-ui-heading vk-notif-title">{title}</h1>
            <p>{decision.work_title.unwrap_or_else(|| "Tác phẩm không còn hiển thị".into())}" · "
                <time datetime=decision.decided_at.clone()>{vink_patterns::updated_at_label(&decision.decided_at)}</time>
            </p>
            <div class="vk-notif-decision__reason">
                <h2 class="vk-ui-heading">"Lý do"</h2>
                <p>{decision.reason.message.or(decision.reason.code).unwrap_or_else(|| "Không có lý do kèm theo.".into())}</p>
            </div>
            {removed.then(|| view! { <p>"Nội dung nhận xét đã gỡ không hiển thị lại ở đây."</p> })}
        </article>
    }
}

#[component]
fn Gone() -> impl IntoView {
    view! {
        <section class="vk-notif-empty">
            <h1 class="vk-ui-heading vk-notif-title">"Nội dung không còn khả dụng"</h1>
            <a class=ButtonVariant::Tonal.classes() href="/">"Về Khám phá"</a>
        </section>
    }
}

fn endorsers_href(work: &str, revision: &str) -> String {
    format!("/tac-pham/{}/tham/{}", segment(work), segment(revision))
}

#[component]
pub(crate) fn EndorsersTrigger(
    work: String,
    revision: String,
    title: String,
    total: Signal<u64>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let href = endorsers_href(&work, &revision);
    #[cfg(feature = "hydrate")]
    {
        let navigation = crate::navigation::use_navigation_guard();
        let registration = navigation.set_overlay(move |_| {
            if open.try_get_untracked() != Some(true) {
                return false;
            }
            open.set(false);
            true
        });
        let resize = window_event_listener(leptos::ev::resize, move |_| {
            if !desktop() {
                open.set(false);
            }
        });
        on_cleanup(move || {
            navigation.clear_overlay(registration);
            resize.remove();
        });
    }
    view! {
        <Show when=move || { total.get() > 0 }>
            <a class=ButtonVariant::Text.classes() href=href.clone()
                on:click=move |event: leptos::ev::MouseEvent| {
                    #[cfg(feature = "hydrate")]
                    if desktop() && event.button() == 0 && !event.default_prevented()
                        && !(event.meta_key() || event.ctrl_key() || event.shift_key() || event.alt_key())
                    {
                        event.prevent_default();
                        open.set(true);
                    }
                    #[cfg(not(feature = "hydrate"))]
                    let _ = event;
                }>{move || format!("{} người đã Thấm", total.get())}</a>
        </Show>
        <Dialog id="work-endorsers" title="Người đã Thấm" open focus_title=true
            on_dismiss=move |_| open.set(false)
            header_action=move || view! { <IconButton label="Đóng danh sách người đã Thấm"
                icon=IconName::Close on_press=move |_| open.set(false) /> }>
            <Show when=move || open.get()>
                <SessionEndorsers work=work.clone() revision=revision.clone() title=title.clone() />
            </Show>
        </Dialog>
    }
}

#[cfg(feature = "hydrate")]
fn desktop() -> bool {
    window()
        .match_media("(min-width: 760px)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches())
}

#[component]
pub(crate) fn EndorsersPage() -> impl IntoView {
    crate::pages::ResponseStatus::capture().set_no_store();
    let params = use_params_map();
    view! {
        <Title text="Người đã Thấm — Vink" />
        <Meta name="robots" content="noindex" />
        <section class="vk-notif-page" aria-labelledby="work-endorsers-page-title">
            <BackButton />
            <h1 class="vk-ui-heading vk-notif-title" id="work-endorsers-page-title">"Người đã Thấm"</h1>
            <For each=move || Some((params.read().get("id").unwrap_or_default(),
                params.read().get("revision").unwrap_or_default())) key=Clone::clone
                children=move |(work, revision)| view! { <SessionEndorsers work revision title=String::new() /> } />
        </section>
    }
}

#[component]
fn SessionEndorsers(work: String, revision: String, title: String) -> impl IntoView {
    let session = Session::current();
    view! {
        <For each=move || Some(session.profile_scope.get().zip(session.notification_scope.get()))
            key=Clone::clone
            children=move |_| view! { <EndorserList work=work.clone() revision=revision.clone() title=title.clone() /> } />
    }
}

#[component]
fn EndorserList(work: String, revision: String, title: String) -> impl IntoView {
    let data = RwSignal::new(None::<Endorsers>);
    let loading = RwSignal::new(true);
    let problem = RwSignal::new(None::<ApiError>);
    let revision = RwSignal::new(revision);
    let title = RwSignal::new(title);
    let load = Callback::new(move |more: bool| {
        #[cfg(feature = "hydrate")]
        {
            let path = format!(
                "/api/v1/works/{}/revisions/{}/endorsers",
                segment(&work),
                segment(&revision.get_untracked())
            );
            let path = if more {
                let Some(cursor) = data
                    .with_untracked(|page| page.as_ref().and_then(|page| page.next_cursor.clone()))
                else {
                    return;
                };
                format!("{path}?cursor={}", segment(&cursor))
            } else {
                path
            };
            loading.set(true);
            problem.set(None);
            leptos::task::spawn_local(async move {
                let result = crate::api::endorser_request::<Endorsers>(&path).await;
                if loading.try_get_untracked().is_none() {
                    return;
                }
                loading.set(false);
                match result {
                    Ok(mut page) => {
                        data.update(|data| {
                            if more && let Some(previous) = data.as_mut() {
                                let mut old = std::mem::take(&mut previous.items);
                                old.append(&mut page.items);
                                page.items = old;
                            }
                            *data = Some(page);
                        });
                    }
                    Err(error) => {
                        if matches!(error, ApiError::NotFound | ApiError::RevisionChanged { .. }) {
                            data.set(None);
                        }
                        problem.set(Some(error));
                    }
                }
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (&work, &revision, more);
    });
    #[cfg(feature = "hydrate")]
    Effect::new(move |_| load.run(false));

    view! {
        {move || match problem.get() {
            Some(ApiError::NotFound) => view! {
                <div class="vk-notif-empty"><p>"Nội dung không còn khả dụng."</p>
                    <a class=ButtonVariant::Tonal.classes() href="/">"Về Khám phá"</a></div>
            }.into_any(),
            Some(ApiError::RevisionChanged { current_revision_id }) => view! {
                <div class="vk-notif-note" role="status">"Danh sách đã thay đổi."
                    <Button variant=ButtonVariant::Tonal on_press=move |_| {
                        revision.set(current_revision_id.clone());
                        title.set(String::new());
                        load.run(false);
                    }>"Tải lại"</Button>
                </div>
            }.into_any(),
            Some(_) if data.with(Option::is_none) => view! {
                <Failure message="Chưa tải được danh sách người đã Thấm." retry=Callback::new(move |()| load.run(false)) />
            }.into_any(),
            None if data.with(Option::is_none) => view! { <Loading /> }.into_any(),
            _ => ().into_any(),
        }}
        {move || data.get().map(|page| {
            let title = title.get();
            let total = if title.is_empty() { format!("{} người", page.total_count) } else { format!("{} người · {title}", page.total_count) };
            view! {
                <p class="vk-notif-endorser-total">{total}</p>
                {if page.items.is_empty() && page.next_cursor.is_none() {
                    view! { <p>"Chưa có lượt Thấm nào."</p> }.into_any()
                } else {
                    view! { <ul class="vk-notif-endorsers">
                        {page.items.into_iter().map(|item| {
                            let name = item.author.label().to_owned();
                            let handle = item.author.handle().map(str::to_owned);
                            let key = handle.clone().unwrap_or_default();
                            let avatar = view! { <Avatar name=name.clone() identity_key=key size=AvatarSize::Reader decorative=true /> };
                            view! { <li>
                                {if let Some(handle) = handle {
                                    let href = format!("/@{}", segment(&handle));
                                    view! { <a href=href>{avatar}<span><strong>{name}</strong><small>{format!("@{handle}")}</small></span></a> }.into_any()
                                } else {
                                    view! { <div>{avatar}<strong>{name}</strong></div> }.into_any()
                                }}
                            </li> }
                        }).collect_view()}
                    </ul> }.into_any()
                }}
                {page.next_cursor.is_some().then(|| view! {
                    <Show when=move || problem.with(Option::is_some) fallback=move || view! {
                        <Button variant=ButtonVariant::Tonal
                            state=Signal::derive(move || if loading.get() { vink_ui::ActionState::pending("Đang tải thêm…") } else { vink_ui::ActionState::Enabled })
                            on_press=move |_| load.run(true)>"Xem thêm"</Button>
                    }>
                        <Failure message="Chưa tải thêm được." retry=Callback::new(move |()| load.run(true)) tonal=true />
                    </Show>
                })}
            }
        })}
    }
}
