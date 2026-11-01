use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::appearance::{AccountStatus, ThemePicker};

#[component]
pub fn Me() -> impl IntoView {
    view! {
        <Title text="Tôi — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="me" class="vk-me" aria-labelledby="me-title">
            <h1 id="me-title" class="vk-page-title">"Tôi"</h1>
            <AccountStatus />
            <ThemePicker group="page-theme" />
        </section>
    }
}
