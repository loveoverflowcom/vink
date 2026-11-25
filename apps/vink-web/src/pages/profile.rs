//! Own fields are hydrated into a single session generation; the public route is actor-free.
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use vink_ui::{Heading, HeadingLevel, Link};

use crate::account::Session;

pub(super) const EDIT_PATH: &str = "/toi/chinh-ho-so";
pub(crate) const PRIVACY_PATH: &str = "/toi/chinh-ho-so#quyen-rieng-tu";
const HANDLE_PATH: &str = "/toi/chinh-ho-so#dia-chi-tac-gia";

/// The old private address only redirects. No profile or Handle request is made here.
#[component]
pub(crate) fn LegacyHandle() -> impl IntoView {
    let status = super::ResponseStatus::capture();
    status.set_no_store();
    status.redirect(HANDLE_PATH);
    #[cfg(feature = "browser")]
    {
        let navigate = leptos_router::hooks::use_navigate();
        Effect::new(move |_| {
            navigate(
                HANDLE_PATH,
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        });
    }
    view! { <Link href=HANDLE_PATH>"Chỉnh địa chỉ tác giả trong hồ sơ"</Link> }
}

#[component]
pub(crate) fn ProfileActions() -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-center gap-vk-2 min-w-0">
            <OwnProfileLink />
            <Link attr:id="edit-profile-link" attr:data-profile-return="true"
                attr:class="vk-account-link" variant=vink_ui::LinkVariant::Standalone href=EDIT_PATH>
                <vink_ui::Icon name=vink_ui::IconName::EditOutline />
                "Chỉnh hồ sơ"
            </Link>
        </div>
    }
}

#[component]
fn OwnProfileLink() -> impl IntoView {
    #[cfg(feature = "browser")]
    {
        use crate::contract::{ApiError, follow::valid_canonical_handle};
        use vink_ui::{Button, ButtonVariant, Icon, IconName, LinkVariant};

        let session = Session::current();
        let revision = RwSignal::new(0u64);
        let renew = move || {
            revision.try_update(|value| *value += 1);
        };
        observe_changes(renew);
        crate::read_events::observe_lifecycle(move |hint| {
            if hint == crate::read_events::Hint::HistoryRestored {
                renew();
            }
        });
        let loaded = LocalResource::new(move || {
            let stamp = revision.try_get();
            let scope = session.profile_scope.get();
            async move {
                let stamp = stamp?;
                let scope = scope?;
                let result = crate::api::load_own_profile().await.map(|profile| {
                    let href = (profile.handle_setup_complete
                        && valid_canonical_handle(&profile.handle))
                    .then(|| format!("/@{}", profile.handle));
                    (profile.principal_id, href)
                });
                Some((scope, stamp, result))
            }
        });
        let loading = || {
            view! {
                <p role="status" class="vk-account-profile-note">"Đang tải địa chỉ tác giả…"</p>
            }
        };
        return view! {
            <Suspense fallback=loading>
                {move || {
                    // Suspense may get one last wake as the private Account branch is removed.
                    let Some(latest) = revision.try_get() else {
                        return ().into_any();
                    };
                    let scope = session.profile_scope.get();
                    let Some((_, _, result)) = loaded.try_get().flatten().flatten()
                        .filter(|(key, stamp, _)| Some(key) == scope.as_ref() && *stamp == latest)
                    else {
                        return loading().into_any();
                    };
                    match result {
                        Ok((principal, href)) if session.principal.get().as_ref() == Some(&principal) => match href {
                            Some(href) => view! {
                                <Link attr:id="view-profile-link" attr:data-profile-return="true"
                                    attr:class="vk-account-link vk-account-link--primary"
                                    variant=LinkVariant::Standalone href=href>
                                    <Icon name=IconName::PersonOutline />
                                    "Xem hồ sơ"
                                </Link>
                            }.into_any(),
                            None => view! {
                                <p role="status" class="vk-account-profile-note">"Thiết lập địa chỉ tác giả trong Chỉnh hồ sơ."</p>
                            }.into_any(),
                        },
                        Err(ApiError::NotFound) => view! {
                            <p role="status" class="vk-account-profile-note">"Chưa có địa chỉ tác giả. Kiểm tra trong Chỉnh hồ sơ."</p>
                        }.into_any(),
                        _ => view! {
                            <div class="vk-account-profile-note flex flex-wrap items-center gap-vk-2">
                                <p role="status" class="m-0">"Chưa tải được địa chỉ tác giả."</p>
                                <Button id="own-profile-link-retry" variant=ButtonVariant::Text
                                    on_press=move |_| renew()>"Thử lại"</Button>
                            </div>
                        }.into_any(),
                    }
                }}
            </Suspense>
        };
    }
    #[cfg(not(feature = "browser"))]
    ()
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
    #[cfg(feature = "browser")]
    return view! { <browser::OwnProfileHost scope /> };
    // SSR renders nothing here: the own overlay is fetched no-store after hydration.
    #[cfg(not(feature = "browser"))]
    let _ = scope;
}

#[cfg(feature = "browser")]
const CHANNEL: &str = "vink-profile-changes";

#[cfg(feature = "browser")]
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

#[cfg(feature = "browser")]
pub(crate) mod browser;
