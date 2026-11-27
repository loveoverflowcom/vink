use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::account::{AccountDeletionPage, AccountHub, account_deletion_available};
#[cfg(feature = "browser")]
use crate::session::Standing;

#[component]
pub fn Me() -> impl IntoView {
    let query = leptos_router::hooks::use_query_map();
    let session = crate::account::Session::current();
    let deletion = Memo::new(move |_| {
        account_deletion_available() && query.with(|query| query.get_str("quan-ly") == Some("xoa"))
    });
    let feed_enabled = crate::api::following_feed_enabled();
    #[cfg(feature = "browser")]
    {
        let navigation = crate::navigation::use_navigation_guard();
        let desk_auth_entry = StoredValue::new(false);
        Effect::new(move |_| {
            if deletion.get() {
                desk_auth_entry.set_value(false);
                return;
            }
            let next = query.with(|query| {
                query
                    .get_str("next")
                    .and_then(super::desk_review::desk_return_href)
            });
            let Some(next) = next else {
                desk_auth_entry.set_value(false);
                return;
            };
            if matches!(
                session.standing.get(),
                Standing::Anonymous | Standing::NeedsPrincipal
            ) {
                desk_auth_entry.set_value(true);
            }
            if session.profile_scope.with(Option::is_some)
                && !session.checking()
                && desk_auth_entry.get_value()
            {
                // An already-active visit (including Back) can stay on Tôi.
                desk_auth_entry.set_value(false);
                navigation.navigate(&next);
            }
        });
    }
    let return_href = Memo::new(move |_| {
        query.read().get("next").and_then(|raw| {
            if feed_enabled && raw == crate::discovery::ExploreMode::FOLLOWING_HREF {
                Some((raw, "Quay lại Đang theo dõi"))
            } else {
                crate::follow_state::safe_profile_return(&raw)
                    .map(|href| (href, "Quay lại trang tác giả"))
            }
        })
    });
    view! {
        <Title text=move || if deletion.get() { "Quản lý xóa tài khoản — Vink" } else { "Tài khoản — Vink" } />
        <Meta name="robots" content="noindex" />
        <section id="me" class="vk-me grid gap-vk-6" aria-labelledby="me-title">
            <Show when=move || deletion.get() fallback=move || view! {
                <AccountHub>
                    {move || (session.profile_scope.get().is_some())
                        .then(|| return_href.get())
                        .flatten()
                        .map(|(href, label)| view! {
                            <vink_ui::Link
                                variant=vink_ui::LinkVariant::Standalone
                                attr:id="me-return-link"
                                href=href>
                                {label}
                            </vink_ui::Link>
                        })}
                </AccountHub>
            }>
                <AccountDeletionPage />
            </Show>
        </section>
    }
}
