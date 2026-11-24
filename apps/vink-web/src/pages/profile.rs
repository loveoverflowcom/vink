//! Own fields are hydrated into a single session generation; the public route is actor-free.
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use vink_ui::{Heading, HeadingLevel, Link};

use crate::account::Session;

pub(super) const EDIT_PATH: &str = "/toi/chinh-ho-so";
const HANDLE_PATH: &str = "/toi/chinh-ho-so#dia-chi-tac-gia";

/// The old private address only redirects. No profile or Handle request is made here.
#[component]
pub(crate) fn LegacyHandle() -> impl IntoView {
    let status = super::ResponseStatus::capture();
    status.set_no_store();
    status.redirect(HANDLE_PATH);
    #[cfg(feature = "hydrate")]
    {
        let navigate = leptos_router::hooks::use_navigate();
        Effect::new(move |_| {
            navigate(
                HANDLE_PATH,
                leptos_router::NavigateOptions { replace: true, ..Default::default() },
            );
        });
    }
    view! { <Link href=HANDLE_PATH>"Chỉnh địa chỉ tác giả trong hồ sơ"</Link> }
}

#[component]
pub(super) fn ProfileEntry() -> impl IntoView {
    let session = Session::current();
    view! {
        <Show when=move || session.profile_scope.get().is_some()>
            <Link attr:id="edit-profile-link" attr:data-profile-return="true" href=EDIT_PATH>
                "Chỉnh hồ sơ"
            </Link>
        </Show>
    }
}

#[component]
pub(crate) fn EditProfile() -> impl IntoView {
    let session = Session::current();
    super::ResponseStatus::capture().set_no_store();

    view! {
        <Title text="Chỉnh hồ sơ · Vink" />
        <Meta name="robots" content="noindex" />
        <section class="vk-me vk-profile-edit grid gap-vk-6" aria-labelledby="edit-profile-heading">
            <Link href="/toi#edit-profile-link">"Tôi"</Link>
            <Heading id="edit-profile-heading" level=HeadingLevel::H1 attr:tabindex="-1">
                "Chỉnh hồ sơ"
            </Heading>
            <For each={move || session.profile_scope.get().into_iter().collect::<Vec<_>>()}
                key=Clone::clone
                children=move |scope| view! { <OwnProfileHost scope /> }
            />
            <Show when=move || session.profile_scope.get().is_none()>
                {move || if session.checking() || matches!(session.standing.get(), crate::session::Standing::Unknown) {
                    view! { <p role="status">"Đang kiểm tra phiên…"</p> }.into_any()
                } else {
                    view! { <p><Link href="/toi">"Mở Tài khoản trong Tôi"</Link></p> }.into_any()
                }}
            </Show>
        </section>
    }
}

#[component]
fn OwnProfileHost(scope: String) -> impl IntoView {
    #[cfg(feature = "hydrate")]
    return view! { <browser::OwnProfileHost scope /> };
    // SSR renders nothing here: the own overlay is fetched no-store after hydration.
    #[cfg(not(feature = "hydrate"))]
    let _ = scope;
}

#[cfg(feature = "hydrate")]
const CHANNEL: &str = "vink-profile-changes";

#[cfg(feature = "hydrate")]
pub(crate) fn observe_changes(on_change: impl Fn() + 'static) {
    use send_wrapper::SendWrapper;
    use wasm_bindgen::{JsCast, closure::Closure};

    if let Ok(channel) = web_sys::BroadcastChannel::new(CHANNEL) {
        let owner = Owner::current().map(|owner| owner.downgrade());
        let listener = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| {
            if let Some(owner) = owner.as_ref().and_then(|owner| owner.upgrade()) {
                owner.with(&on_change);
            }
        });
        channel.set_onmessage(Some(listener.as_ref().unchecked_ref()));
        let connection = SendWrapper::new((channel, listener));
        on_cleanup(move || {
            connection.0.set_onmessage(None);
            connection.0.close();
        });
    }
}

#[cfg(feature = "hydrate")]
pub(crate) mod browser;
