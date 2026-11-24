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

#[cfg(feature = "browser")]
mod browser;
#[cfg(feature = "browser")]
pub(super) use browser::ReturnScope;

const FEED_STATE_CLASS: &str = "vk-feed-state my-vk-6 grid justify-items-start gap-vk-3 [--vk-ui-control-radius:var(--vk-radius-control-56)]";

#[component]
pub(super) fn FollowingFeed(context: ExploreContext) -> impl IntoView {
    let session = Session::current();
    let enabled = api::following_feed_enabled();
    let expired = RwSignal::new(false);
    let stream = NodeRef::<leptos::html::Section>::new();
    #[cfg(feature = "browser")]
    let restore = RwSignal::new(None::<super::super::ExploreRestore>);
    #[cfg(feature = "browser")]
    let return_completed = RwSignal::new(false);
    #[cfg(feature = "browser")]
    {
        use super::super::ExploreRestore;

        let ready = Signal::derive(move || {
            if return_completed.get() {
                return None;
            }
            if let Some(scope) = session.profile_scope.get() {
                restore.get().filter(|permit| match permit {
                    ExploreRestore::Feed { ticket, .. } => ticket.scope == scope,
                    _ => false,
                })
            } else {
                (matches!(
                    session.standing.get(),
                    Standing::Anonymous | Standing::NeedsPrincipal | Standing::Suspended
                ) && !session.checking())
                .then_some(ExploreRestore::Guest)
            }
        });
        super::super::restore_explore_position(
            stream,
            ready,
            Some(("following-feed-heading", return_completed)),
        );
    }
    #[cfg(not(feature = "browser"))]
    let _ = context;

    view! {
        <Title text="Đang theo dõi — Vink" />
        <Meta name="robots" content="noindex" />
        <Link rel="canonical" href=crate::canonical_url("/") />
        <div id="explore" class="vk-explore vk-discovery vk-discovery--following">
            <super::ExploreTitleRow title="Khám phá" />
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
                                    <div class=FEED_STATE_CLASS data-feed-state="unavailable">
                                        <p class="m-0" role="status">"Chưa kiểm tra được phiên đăng nhập."</p>
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
                            #[cfg(feature = "browser")]
                            {
                                let context = context.clone();
                                view! {
                                    <For each=move || session.profile_scope.get().into_iter()
                                        key=Clone::clone
                                        children=move |scope| view! {
                                            <browser::ActiveFeed scope context=context.clone() expired restore return_completed />
                                        } />
                                }.into_any()
                            }
                            #[cfg(not(feature = "browser"))]
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
        <div class=FEED_STATE_CLASS data-feed-state=if expired { "expired" } else { "guest" }>
            <p class="m-0" role="status">
                {if needs_name { "Hoàn tất bút danh để xem tác phẩm đang theo dõi." }
                    else if expired { "Phiên đã hết. Đăng nhập lại để xem tác phẩm đang theo dõi." }
                    else if sign_in_open { "Đăng nhập để xem tác phẩm của các tác giả bạn theo dõi." }
                    else { "Vink chưa mở đăng nhập." }}
            </p>
            {(sign_in_open || needs_name).then(|| view! {
                <a id="feed-sign-in" class=ButtonVariant::Filled.classes() href=href>
                    {if needs_name { "Hoàn tất bút danh" } else { "Đăng nhập" }}
                </a>
            })}
        </div>
    }
}

#[component]
fn Restricted() -> impl IntoView {
    view! {
        <div class=FEED_STATE_CLASS data-feed-state="restricted">
            <p class="m-0" role="status">"Tài khoản hiện chưa có quyền xem Đang theo dõi."</p>
            <UiLink href="/toi">"Mở Tài khoản"</UiLink>
        </div>
    }
}

#[component]
fn FeedOff() -> impl IntoView {
    view! {
        <div class=FEED_STATE_CLASS data-feed-state="disabled">
            <p class="m-0" role="status">"Đang theo dõi hiện chưa khả dụng."</p>
            <UiLink href="/">"Khám phá tác phẩm"</UiLink>
        </div>
    }
}
