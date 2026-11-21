//! The account-scoped results region of Khám phá.
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use vink_ui::{Button, ButtonVariant, Heading, HeadingLevel, Link as UiLink};

use crate::{
    account::Session,
    api,
    discovery::{ExploreContext, ExploreMode, encode},
    session::Standing,
};

#[cfg(feature = "hydrate")]
mod browser;
#[cfg(feature = "hydrate")]
pub(super) use browser::remember_position;

#[component]
pub(super) fn FollowingFeed(context: ExploreContext) -> impl IntoView {
    let session = Session::current();
    let enabled = api::following_feed_enabled();
    let expired = RwSignal::new(false);
    let stream = NodeRef::<leptos::html::Section>::new();
    #[cfg(feature = "hydrate")]
    let settled_scope = RwSignal::new(None::<(String, bool)>);
    #[cfg(feature = "hydrate")]
    {
        let settled = Memo::new(move |_| {
            if let Some(scope) = session.profile_scope.get() {
                settled_scope.with(|value| value.as_ref().is_some_and(|(key, _)| key == &scope))
            } else {
                matches!(
                    session.standing.get(),
                    Standing::Anonymous | Standing::NeedsPrincipal | Standing::Suspended
                ) && !session.checking()
            }
        });
        let allow_anchor = Signal::derive(move || {
            let scope = session.profile_scope.get();
            settled_scope.with(|value| {
                value
                    .as_ref()
                    .is_some_and(|(owner, restore)| *restore && Some(owner) == scope.as_ref())
            })
        });
        super::super::restore_explore_position(
            stream,
            settled,
            allow_anchor,
            Some("following-feed-heading"),
        );
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = context;

    view! {
        <Title text="Đang theo dõi — Vink" />
        <Meta name="robots" content="noindex" />
        <Link rel="canonical" href=crate::canonical_url("/") />
        <div id="explore" class="vk-explore vk-discovery vk-discovery--following">
            <header class="vk-explore__heading">
                <Heading level=HeadingLevel::H1>"Khám phá"</Heading>
            </header>
            <super::ExploreModes mode=ExploreMode::Following />
            <section node_ref=stream id="community-stream" aria-labelledby="following-feed-heading" tabindex="-1">
                <Heading id="following-feed-heading" level=HeadingLevel::H2 role=vink_ui::TypeRole::Title attr:tabindex="-1">"Đang theo dõi"</Heading>
                {move || if !enabled {
                    view! { <FeedOff /> }.into_any()
                } else {
                    view! {
                        <Show when=move || session.profile_scope.get().is_none()>
                            {move || match session.standing.get() {
                                Standing::Anonymous => view! { <SignIn expired=expired.get() /> }.into_any(),
                                Standing::NeedsPrincipal => view! { <SignIn needs_name=true /> }.into_any(),
                                Standing::Suspended => view! { <Restricted /> }.into_any(),
                                Standing::Unavailable if !session.checking() => view! {
                                    <div class="vk-feed-state" data-feed-state="unavailable">
                                        <p role="status">"Chưa kiểm tra được phiên đăng nhập."</p>
                                        <Button variant=ButtonVariant::Tonal on_press=move |_| session.refresh()>"Thử lại"</Button>
                                    </div>
                                }.into_any(),
                                _ => view! {
                                    <p role="status" data-feed-state="checking">"Đang kiểm tra phiên…"</p>
                                    <noscript>"Bật JavaScript để mở Đang theo dõi."</noscript>
                                }.into_any(),
                            }}
                        </Show>
                        {
                            #[cfg(feature = "hydrate")]
                            {
                                let context = context.clone();
                                view! {
                                    <For each=move || session.profile_scope.get().into_iter()
                                        key=Clone::clone
                                        children=move |scope| view! {
                                            <browser::ActiveFeed scope context=context.clone() expired settled_scope />
                                        } />
                                }.into_any()
                            }
                            #[cfg(not(feature = "hydrate"))]
                            ()
                        }
                    }.into_any()
                }}
            </section>
        </div>
    }
}

#[component]
fn SignIn(#[prop(optional)] expired: bool, #[prop(optional)] needs_name: bool) -> impl IntoView {
    let sign_in_open = api::sign_in_open();
    let href = format!("/toi?next={}", encode(ExploreMode::FOLLOWING_HREF));
    view! {
        <div class="vk-feed-state" data-feed-state=if expired { "expired" } else { "guest" }>
            <p role="status">
                {if needs_name { "Hoàn tất bút danh để xem tác phẩm đang theo dõi." }
                    else if expired { "Phiên đã hết. Đăng nhập lại để xem tác phẩm đang theo dõi." }
                    else if sign_in_open { "Đăng nhập để xem tác phẩm của các tác giả bạn theo dõi." }
                    else { "Vink chưa mở đăng nhập." }}
            </p>
            {(sign_in_open || needs_name).then(|| view! {
                <UiLink attr:id="feed-sign-in" href=href>
                    {if needs_name { "Hoàn tất bút danh" } else { "Đăng nhập" }}
                </UiLink>
            })}
        </div>
    }
}

#[component]
fn Restricted() -> impl IntoView {
    view! {
        <div class="vk-feed-state" data-feed-state="restricted">
            <p role="status">"Tài khoản hiện chưa có quyền xem Đang theo dõi."</p>
            <UiLink href="/toi">"Mở Tài khoản"</UiLink>
        </div>
    }
}

#[component]
fn FeedOff() -> impl IntoView {
    view! {
        <div class="vk-feed-state" data-feed-state="disabled">
            <p role="status">"Đang theo dõi hiện chưa khả dụng."</p>
            <UiLink href="/">"Khám phá tác phẩm"</UiLink>
        </div>
    }
}
