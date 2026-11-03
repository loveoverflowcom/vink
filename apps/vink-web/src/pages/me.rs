use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::appearance::{AccountStatus, ThemePicker};

#[component]
pub fn Me() -> impl IntoView {
    view! {
        <Title text="Tôi — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="me" class="vk-me" aria-labelledby="me-title">
            <vink_ui::Heading id="me-title" level=vink_ui::HeadingLevel::H1>"Tôi"</vink_ui::Heading>
            <AccountStatus id="page-account" />
            <ThemePicker group="page-theme" />
        </section>
    }
}
