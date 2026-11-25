use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::account::AccountPanel;
use crate::appearance::{AppearanceChoices, AppearanceTitle};

#[component]
pub fn Me() -> impl IntoView {
    let query = leptos_router::hooks::use_query_map();
    let session = crate::account::Session::current();
    let feed_enabled = crate::api::following_feed_enabled();
    #[cfg(feature = "browser")]
    {
        use crate::session::Standing;
        let navigation = crate::navigation::use_navigation_guard();
        let desk_auth_entry = StoredValue::new(false);
        Effect::new(move |_| {
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
    let review_access = LocalResource::new(move || {
        let scope = session.profile_scope.get();
        async move {
            let scope = scope?;
            crate::api::can_read_work_review_queue()
                .await
                .then_some(scope)
        }
    });

    view! {
        <Title text="Tôi — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="me" class="vk-me grid gap-vk-6" aria-labelledby="me-title">
            <header class="vk-notif-me-header">
                <vink_ui::Heading id="me-title" level=vink_ui::HeadingLevel::H1 attr:tabindex="-1">"Tôi"</vink_ui::Heading>
                <crate::notifications::MobileBell />
            </header>
            <AccountPanel id="page-account" />
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
            <super::profile::ProfileEntry />
            {move || {
                let scope = session.profile_scope.get()?;
                (review_access.get().flatten().as_ref() == Some(&scope)).then(|| view! {
                    <vink_ui::Link attr:id="me-moderation-link" href="/kiem-duyet">"Kiểm duyệt"</vink_ui::Link>
                })
            }}
            <section id="page-appearance" tabindex="-1" class="vk-appearance-card" aria-labelledby="page-appearance-title">
                <AppearanceTitle id="page-appearance-title" />
                <AppearanceChoices prefix="page" />
            </section>
        </section>
    }
}
