use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::account::AccountPanel;
use crate::appearance::{AppearanceChoices, AppearanceTitle};

#[component]
pub fn Me() -> impl IntoView {
    let query = leptos_router::hooks::use_query_map();
    let session = crate::account::Session::current();
    let return_href = Memo::new(move |_| {
        query
            .read()
            .get("next")
            .and_then(|raw| crate::follow_state::safe_profile_return(&raw))
    });

    view! {
        <Title text="Tôi — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="me" class="vk-me" aria-labelledby="me-title">
            <vink_ui::Heading id="me-title" level=vink_ui::HeadingLevel::H1 attr:tabindex="-1">"Tôi"</vink_ui::Heading>
            <AccountPanel id="page-account" />
            {move || (session.profile_scope.get().is_some())
                .then(|| return_href.get())
                .flatten()
                .map(|href| view! {
                    <vink_ui::Link
                        variant=vink_ui::LinkVariant::Standalone
                        attr:id="me-return-link"
                        href=href>
                        "Quay lại trang tác giả"
                    </vink_ui::Link>
                })}
            <super::profile::ProfileEntry />
            <super::handle::HandleRegion entry=true />
            <vink_ui::Link href="/kiem-duyet">"Kiểm duyệt — dành cho người được phân quyền"</vink_ui::Link>
            <section class="vk-appearance-card" aria-labelledby="page-appearance-title">
                <AppearanceTitle id="page-appearance-title" />
                <AppearanceChoices prefix="page" />
            </section>
        </section>
    }
}
