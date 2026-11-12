use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::account::AccountPanel;
use crate::appearance::{AppearanceChoices, AppearanceTitle};

#[component]
pub fn Me() -> impl IntoView {
    view! {
        <Title text="Tôi — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="me" class="vk-me" aria-labelledby="me-title">
            <vink_ui::Heading id="me-title" level=vink_ui::HeadingLevel::H1>"Tôi"</vink_ui::Heading>
            <AccountPanel id="page-account" />
            <vink_ui::Link href="/kiem-duyet">"Kiểm duyệt — dành cho người được phân quyền"</vink_ui::Link>
            <section class="vk-appearance-card" aria-labelledby="page-appearance-title">
                <AppearanceTitle id="page-appearance-title" />
                <AppearanceChoices prefix="page" />
            </section>
        </section>
    }
}
