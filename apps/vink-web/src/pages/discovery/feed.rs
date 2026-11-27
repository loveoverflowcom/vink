//! The account-scoped results region of Khám phá.
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use vink_ui::{Button, ButtonVariant, Heading, HeadingLevel, IconName, Link as UiLink};

use crate::{
    account::Session,
    api,
    discovery::{ExploreContext, ExploreMode, encode},
    session::Standing,
    ui::empty_state::EmptyState,
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
    let sign_in_open = api::sign_in_open();
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
        <section node_ref=stream id="community-stream" aria-labelledby="following-feed-heading" tabindex="-1">
            <Heading id="following-feed-heading" level=HeadingLevel::H2 role=vink_ui::TypeRole::Title attr:tabindex="-1">"Đang theo dõi"</Heading>
            {move || if !enabled {
                view! { <FeedOff /> }.into_any()
            } else {
                view! {
                    <Show when=move || session.profile_scope.get().is_none()>
                        {move || match session.standing.get() {
                            Standing::Anonymous if expired.get() => view! { <AccountRecovery sign_in_open /> }.into_any(),
                            Standing::Anonymous => view! { <GuestAccess sign_in_open /> }.into_any(),
                            Standing::NeedsPrincipal => view! { <AccountRecovery needs_name=true sign_in_open /> }.into_any(),
                            Standing::Suspended => view! { <Restricted /> }.into_any(),
                            Standing::Unavailable if !session.checking() => view! {
                                <div class=FEED_STATE_CLASS data-feed-state="unavailable">
                                    <p class="m-0" role="status">"Chưa kiểm tra được phiên đăng nhập."</p>
                                    <Button variant=ButtonVariant::Tonal on_press=move |_| session.refresh()>"Thử lại"</Button>
                                </div>
                            }.into_any(),
                            _ => view! {
                                <crate::ui::PendingRead label="Đang kiểm tra phiên…" attr:data-feed-state="checking" />
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
    }
}

#[component]
fn GuestAccess(sign_in_open: bool) -> impl IntoView {
    view! {
        <div class="vk-following-guest" data-feed-state="guest">
            {if sign_in_open {
                view! {
                    <EmptyState icon=IconName::PersonAdd title="Gần hơn với tác giả" announce=true
                        message="Theo dõi những cây bút bạn yêu thích và đón đọc tác phẩm mới.">
                        <FeedAccountLink label="Đăng nhập" />
                    </EmptyState>
                }.into_any()
            } else {
                view! { <EmptyState announce=true message="Vink chưa mở đăng nhập." /> }.into_any()
            }}
        </div>
    }
}

#[component]
fn AccountRecovery(sign_in_open: bool, #[prop(optional)] needs_name: bool) -> impl IntoView {
    view! {
        <div class=FEED_STATE_CLASS data-feed-state=if needs_name { "guest" } else { "expired" }>
            <p class="m-0" role="status">
                {if needs_name { "Hoàn tất bút danh để xem tác phẩm đang theo dõi." }
                    else { "Phiên đã hết. Đăng nhập lại để xem tác phẩm đang theo dõi." }}
            </p>
            {(needs_name || sign_in_open).then(|| view! {
                <FeedAccountLink label=if needs_name { "Hoàn tất bút danh" } else { "Đăng nhập" } />
            })}
        </div>
    }
}

#[component]
fn FeedAccountLink(label: &'static str) -> impl IntoView {
    let href = format!("/toi?next={}", encode(ExploreMode::FOLLOWING_HREF));
    view! {
        <a id="feed-sign-in" class=ButtonVariant::Filled.classes() href=href>{label}</a>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn following_access_actions_respect_sign_in_gate_and_standing() {
        Owner::new().with(|| {
            let guest = view! { <GuestAccess sign_in_open=true /> }.to_html();
            let closed = view! { <GuestAccess sign_in_open=false /> }.to_html();
            let needs_name =
                view! { <AccountRecovery sign_in_open=false needs_name=true /> }.to_html();
            let expired = view! { <AccountRecovery sign_in_open=false /> }.to_html();

            assert!(guest.contains("Gần hơn với tác giả") && guest.contains(">Đăng nhập<"));
            assert!(closed.contains("Vink chưa mở đăng nhập.") && !closed.contains("feed-sign-in"));
            assert!(
                needs_name.contains(r#"data-feed-state="guest""#)
                    && needs_name.contains(">Hoàn tất bút danh<")
            );
            for view in [&guest, &needs_name] {
                assert!(view.contains(r#"href="/toi?next=%2F%3Fmode%3Dfollowing""#));
                assert_eq!(view.matches(r#"role="status""#).count(), 1);
            }
            assert!(
                expired.contains(r#"data-feed-state="expired""#)
                    && !expired.contains("feed-sign-in")
            );
        });
    }
}
