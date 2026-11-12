//! Vink UI preview (Leptos): the shared catalog rendered through the renderer's public API.
//!
//! Region IDs follow the LayoutSpec in packages/ui-system/README.md: `#catalog-nav`,
//! `#preview-controls`, `#specimen`, `#inspector`. Preferences live in memory only; the preview
//! reads no storage and calls no service. The URL hash (`#<entry>/<scenario>`) is the exact,
//! shareable scenario link. Fixtures and the clock are fixed by `shared/fixtures`. Navigation
//! specimens dispatch into an in-memory route; the app's router is checked by the Web smoke.

// Deeply nested specimen views exceed the default type-query depth.
#![recursion_limit = "256"]

mod components;
mod endorsement;
mod navigation;
mod screens;
mod states;
mod tags;

use leptos::prelude::*;
use unicode_normalization::UnicodeNormalization;
use vink_ui::catalog::{self, ENTRIES, fixture};
use vink_ui::tokens::{self, CONTRAST, ColorRole, dim, text};
use vink_ui::values::{CatalogEntry, Dimension, TextSize};
use vink_ui::{Density, MotionPreference, Preferences, ThemePreference, ThemeScope, UiRoot};

pub use components::EventLog;

/// Catalog IDs this preview renders; a test keeps it equal to the shared catalog.
pub const REGISTERED: [&str; 53] = [
    "foundation.color-roles",
    "foundation.typography",
    "foundation.shape-spacing",
    "foundation.motion",
    "text.text",
    "text.heading",
    "layout.surface",
    "layout.container",
    "layout.stack",
    "layout.grid",
    "layout.divider",
    "action.button",
    "action.icon-button",
    "action.link",
    "action.filter-chip",
    "action.toggle-action",
    "field.text-field",
    "field.text-area",
    "field.search-field",
    "field.checkbox",
    "field.radio",
    "field.switch",
    "field.select",
    "field.form",
    "identity.icon",
    "identity.avatar",
    "identity.avatar-group",
    "identity.author-byline",
    "identity.image-frame",
    "state.loading",
    "state.progress",
    "state.empty",
    "state.error",
    "state.notice",
    "state.status",
    "state.dialog",
    "state.playground",
    "navigation.rail",
    "navigation.bottom-bar",
    "navigation.top-bar",
    "navigation.scaffold",
    "navigation.overlay",
    "navigation.adapter",
    "composition.nested",
    "screen.explore",
    "screen.reader",
    "screen.desk",
    "screen.editor",
    "screen.me",
    "composition.feedback",
    "composition.endorsement",
    "composition.tag-selector",
    "composition.interaction",
];

/// Custom properties the inspector reads back from the rendered specimen scope.
#[cfg(feature = "csr")]
const INSPECTED: [&str; 9] = [
    "--vk-surface",
    "--vk-on-surface",
    "--vk-primary",
    "--vk-action-primary-container",
    "--vk-type-body-size",
    "--vk-density-control-padding",
    "--vk-duration-short",
    "--vk-loop-iterations",
    "color-scheme",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub entry: &'static CatalogEntry,
    pub scenario: &'static str,
}

/// `#foundation.typography/combining-marks` → that entry and scenario; anything unknown falls
/// back to the first entry's default scenario.
pub fn parse_hash(hash: &str) -> Selection {
    let hash = hash.trim_start_matches('#');
    let (id, scenario) = hash.split_once('/').unwrap_or((hash, "default"));
    let entry = ENTRIES.iter().find(|e| e.id == id).unwrap_or(&ENTRIES[0]);
    let scenario = entry
        .scenarios
        .iter()
        .find(|s| s.id == scenario)
        .map_or("default", |s| s.id);
    Selection { entry, scenario }
}

/// Case- and diacritic-insensitive key: "Màu" and "mau" match, as do Đ and D.
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .map(|c| match c {
            'đ' | 'Đ' => 'd',
            c => c,
        })
        .collect::<String>()
        .to_lowercase()
}

pub fn matches(entry: &CatalogEntry, query: &str) -> bool {
    let query = fold(query.trim());
    if query.is_empty() {
        return true;
    }
    let mut haystack = vec![entry.id, entry.family, entry.label, entry.summary];
    haystack.extend(entry.scenarios.iter().flat_map(|s| [s.id, s.label]));
    haystack.iter().any(|field| fold(field).contains(&query))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frame {
    Fluid,
    Width(u16),
}

#[component]
pub fn App() -> impl IntoView {
    let theme = RwSignal::new(ThemePreference::System);
    let motion = RwSignal::new(MotionPreference::System);
    let frame = RwSignal::new(Frame::Fluid);
    let scale = RwSignal::new(100u16);
    let query = RwSignal::new(String::new());
    let hash = RwSignal::new(current_hash());
    let log = RwSignal::new(Vec::<String>::new());
    let effective = RwSignal::new(Vec::<(String, String)>::new());
    let selection = Memo::new(move |_| parse_hash(&hash.get()));
    let preferences = Signal::derive(move || Preferences {
        theme: theme.get(),
        density: Density::Comfortable,
        motion: motion.get(),
    });
    let record = move |message: String| {
        log.update(|entries| {
            let n = entries.len() + 1;
            entries.insert(0, format!("{} · #{n} {message}", fixture::CLOCK));
        });
    };
    provide_context(EventLog(Callback::new(record)));
    // Loops stop while the tab is hidden; the preview is the host that injects activity.
    vink_ui::provide_motion_activity(vink_ui::document_visibility());

    // Bumped when the system colour scheme or motion setting changes, so `System` re-reads.
    let media = RwSignal::new(0u32);
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;

        let _ = window_event_listener(leptos::ev::hashchange, move |_| hash.set(current_hash()));
        for (query, what) in [
            ("(prefers-color-scheme: dark)", "giao diện"),
            ("(prefers-reduced-motion: reduce)", "chuyển động"),
        ] {
            if let Ok(Some(list)) = window().match_media(query) {
                let changed = Closure::<dyn Fn()>::new(move || {
                    media.update(|n| *n += 1);
                    record(format!("hệ thống đổi {what}"));
                });
                let _ = list
                    .add_event_listener_with_callback("change", changed.as_ref().unchecked_ref());
                // Lives as long as the page; the preview is never unmounted.
                changed.forget();
            }
        }
        Effect::new(move |_| {
            let percent = scale.get();
            if let Some(root) = document().document_element() {
                let _ = root.set_attribute("style", &format!("font-size: {percent}%"));
            }
        });
        // Read effective values after the browser has applied the new attributes.
        Effect::new(move |_| {
            theme.track();
            motion.track();
            selection.track();
            scale.track();
            media.track();
            request_animation_frame(move || effective.set(read_effective()));
        });
    }
    #[cfg(not(feature = "csr"))]
    let _ = (effective, scale, media);

    let visible = move || {
        let q = query.get();
        ENTRIES
            .iter()
            .filter(|entry| matches(entry, &q))
            .collect::<Vec<_>>()
    };

    view! {
        <UiRoot preferences=preferences label="Vink UI preview">
            <div class="vkp-page">
                <header class="vkp-header">
                    <p class="vkp-brand">"Vink UI · preview Leptos"</p>
                    <p class="vkp-meta">
                        {format!(
                            "tokens.json {} · catalog {} · Leptos 0.8",
                            tokens::VERSION,
                            catalog::CATALOG_VERSION,
                        )}
                    </p>
                    <p class="vkp-offline" data-testid="offline-note">
                        {format!(
                            "Không kết nối dịch vụ · dữ liệu [Mẫu] · đồng hồ cố định {}",
                            fixture::CLOCK,
                        )}
                    </p>
                </header>
                <div class="vkp-workbench">
                    <nav id="catalog-nav" class="vkp-nav" aria-label="Danh mục preview">
                        <label class="vkp-search">
                            <span>"Tìm họ, thành phần, kịch bản"</span>
                            <input
                                type="search"
                                prop:value=move || query.get()
                                on:input=move |ev| query.set(event_target_value(&ev))
                            />
                        </label>
                        <Show
                            when=move || !visible().is_empty()
                            fallback=|| view! { <p class="vkp-empty">"Không có mục khớp."</p> }
                        >
                            <ul class="vkp-entries">
                                {move || {
                                    visible()
                                        .into_iter()
                                        .map(|entry| {
                                            let current = move || {
                                                (selection.get().entry.id == entry.id).then_some("page")
                                            };
                                            view! {
                                                <li>
                                                    <a
                                                        href=format!("#{}/default", entry.id)
                                                        aria-current=current
                                                        data-catalog-link=entry.id
                                                    >
                                                        <span>{entry.label}</span>
                                                        <small>{entry.id}</small>
                                                    </a>
                                                </li>
                                            }
                                        })
                                        .collect_view()
                                }}
                            </ul>
                        </Show>
                    </nav>
                    <main class="vkp-stage">
                        <section id="preview-controls" class="vkp-controls" aria-label="Điều khiển preview">
                            <Choice
                                legend="Giao diện"
                                name="theme"
                                options=vec![
                                    (ThemePreference::Light, "Sáng"),
                                    (ThemePreference::Dark, "Tối"),
                                    (ThemePreference::System, "Hệ thống"),
                                ]
                                value=theme
                                on_pick=Callback::new(move |label: &'static str| record(format!("giao diện → {label}")))
                            />
                            <Choice
                                legend="Khung mẫu"
                                name="frame"
                                options=vec![
                                    (Frame::Fluid, "Tự do"),
                                    (Frame::Width(320), "320"),
                                    (Frame::Width(390), "390"),
                                    (Frame::Width(834), "834"),
                                    (Frame::Width(1440), "1440"),
                                ]
                                value=frame
                                on_pick=Callback::new(move |label: &'static str| record(format!("khung → {label}")))
                            />
                            <Choice
                                legend="Cỡ chữ"
                                name="scale"
                                options=vec![(100u16, "100%"), (200u16, "200%")]
                                value=scale
                                on_pick=Callback::new(move |label: &'static str| record(format!("cỡ chữ → {label}")))
                            />
                            <Choice
                                legend="Chuyển động"
                                name="motion"
                                options=vec![
                                    (MotionPreference::System, "Hệ thống"),
                                    (MotionPreference::Reduce, "Giảm"),
                                ]
                                value=motion
                                on_pick=Callback::new(move |label: &'static str| record(format!("chuyển động → {label}")))
                            />
                        </section>
                        <section id="specimen" class="vkp-specimen" aria-labelledby="specimen-title">
                            <div class="vkp-specimen__head">
                                <h1 id="specimen-title">{move || selection.get().entry.label}</h1>
                                <p>{move || selection.get().entry.summary}</p>
                                <div class="vkp-scenarios" role="list" aria-label="Kịch bản">
                                    {move || {
                                        let current = selection.get();
                                        current
                                            .entry
                                            .scenarios
                                            .iter()
                                            .map(|s| {
                                                let here = (s.id == current.scenario).then_some("true");
                                                view! {
                                                    <a
                                                        role="listitem"
                                                        href=format!("#{}/{}", current.entry.id, s.id)
                                                        aria-current=here
                                                        data-scenario=s.id
                                                    >
                                                        {s.label}
                                                    </a>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </div>
                            </div>
                            <div
                                class="vkp-frame"
                                style=move || match frame.get() {
                                    Frame::Fluid => String::new(),
                                    Frame::Width(px) => format!("inline-size: min(100%, {px}px)"),
                                }
                            >
                                {move || {
                                    let current = selection.get();
                                    specimen(current.entry.id, current.scenario)
                                }}
                            </div>
                        </section>
                        <section id="inspector" class="vkp-inspector" aria-label="Inspector">
                            <h2>"Inspector"</h2>
                            <dl class="vkp-facts">
                                <dt>"Catalog ID"</dt>
                                <dd data-testid="inspector-id">
                                    {move || format!("{} / {}", selection.get().entry.id, selection.get().scenario)}
                                </dd>
                                <dt>"Nguồn"</dt>
                                <dd>{format!("{} @ {}", tokens::SOURCE, tokens::VERSION)}</dd>
                                <dt>"Trạng thái"</dt>
                                <dd data-testid="inspector-status">
                                    {move || {
                                        let entry = selection.get().entry;
                                        format!("Leptos: {} · CMP: {}", entry.leptos, entry.cmp)
                                    }}
                                </dd>
                            </dl>
                            <h3>"Giá trị hiệu lực trong specimen"</h3>
                            <div class="vkp-table">
                            <table class="vkp-effective" data-testid="effective">
                                <thead>
                                    <tr>
                                        <th scope="col">"Thuộc tính"</th>
                                        <th scope="col">"Giá trị"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {move || {
                                        effective
                                            .get()
                                            .into_iter()
                                            .map(|(name, value)| {
                                                let property = name.clone();
                                                view! {
                                                    <tr data-property=property>
                                                        <th scope="row"><code>{name}</code></th>
                                                        <td><code>{value}</code></td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </tbody>
                            </table>
                            </div>
                            <h3>"Nhật ký sự kiện"</h3>
                            <ol class="vkp-log" data-testid="event-log">
                                {move || {
                                    let entries = log.get();
                                    if entries.is_empty() {
                                        view! { <li>"Chưa có thao tác."</li> }.into_any()
                                    } else {
                                        entries
                                            .into_iter()
                                            .map(|line| view! { <li>{line}</li> })
                                            .collect_view()
                                            .into_any()
                                    }
                                }}
                            </ol>
                        </section>
                    </main>
                </div>
            </div>
        </UiRoot>
    }
}

/// A radio group bound to a signal; `on_pick` receives the chosen label for the event log.
#[component]
fn Choice<T>(
    legend: &'static str,
    name: &'static str,
    options: Vec<(T, &'static str)>,
    value: RwSignal<T>,
    on_pick: Callback<&'static str>,
) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    view! {
        <fieldset class="vkp-choice">
            <legend>{legend}</legend>
            {options
                .into_iter()
                .map(|(option, label)| {
                    view! {
                        <label>
                            <input
                                type="radio"
                                name=name
                                data-label=label
                                prop:checked=move || value.get() == option
                                on:change=move |_| {
                                    value.set(option);
                                    on_pick.run(label);
                                }
                            />
                            <span>{label}</span>
                        </label>
                    }
                })
                .collect_view()}
        </fieldset>
    }
}

/// The specimen for a catalog entry and scenario, always inside a scope the inspector reads.
pub fn specimen(id: &str, scenario: &str) -> AnyView {
    let body = match id {
        "composition.interaction" => view! {
            <p>"Mẫu dành riêng cho native: IME Next/Search/Done, FocusRequester và Back của Android. Web dùng input/form DOM và NavigationGuard của host; xem các mẫu field và navigation hiện có."</p>
        }.into_any(),
        "foundation.color-roles" => color_roles(scenario == "nested-theme"),
        "foundation.typography" => typography(scenario == "combining-marks"),
        "foundation.shape-spacing" => shape_spacing(),
        "foundation.motion" => motion_specimen(),
        other => screens::specimen(other, scenario)
            .or_else(|| endorsement::specimen(other, scenario))
            .or_else(|| tags::specimen(other, scenario))
            .or_else(|| components::specimen(other, scenario))
            .unwrap_or_else(|| view! { <p>"Chưa đăng ký trong preview Leptos."</p> }.into_any()),
    };
    let density = (scenario == "compact").then_some(Density::Compact);
    let reduce = scenario == "reduced";
    let (id, scenario) = (id.to_string(), scenario.to_string());
    view! {
        <ThemeScope density=density reduce_motion=reduce>
            <div id="specimen-scope" class="vkp-scope" data-catalog-id=id data-scenario-id=scenario>
                {body}
            </div>
        </ThemeScope>
    }
    .into_any()
}

fn swatches() -> impl IntoView {
    let pairs = [
        (ColorRole::Surface, ColorRole::OnSurface),
        (ColorRole::SurfaceContainer, ColorRole::OnSurfaceVariant),
        (ColorRole::Primary, ColorRole::OnPrimary),
        (ColorRole::PrimaryContainer, ColorRole::OnPrimaryContainer),
        (
            ColorRole::SecondaryContainer,
            ColorRole::OnSecondaryContainer,
        ),
        (ColorRole::TertiaryContainer, ColorRole::OnTertiaryContainer),
        (ColorRole::ErrorContainer, ColorRole::OnErrorContainer),
        (ColorRole::WarningContainer, ColorRole::OnWarningContainer),
        (ColorRole::SuccessContainer, ColorRole::OnSuccessContainer),
        (ColorRole::InfoContainer, ColorRole::OnInfoContainer),
        (ColorRole::InverseSurface, ColorRole::InverseOnSurface),
        (
            ColorRole::ActionPrimaryContainer,
            ColorRole::ActionPrimaryContent,
        ),
    ];
    view! {
        <ul class="vkp-swatches">
            {pairs
                .into_iter()
                .map(|(bg, fg)| {
                    view! {
                        <li
                            class="vkp-swatch"
                            data-role=bg.token()
                            style=format!("background: var({}); color: var({})", bg.css_var(), fg.css_var())
                        >
                            <span class="vkp-swatch__sample">"Aa Chữ mẫu"</span>
                            <code>{bg.token()}</code>
                            <code>{fg.token()}</code>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}

fn color_roles(nested: bool) -> AnyView {
    if nested {
        return view! {
            <p class="vkp-note">
                "Hai scope đặt theme tường minh, đè giao diện của preview; scope trong cùng đặt Hệ thống bên trong scope Tối."
            </p>
            <div class="vkp-nested">
                <ThemeScope theme=Some(ThemePreference::Light) label="Scope Sáng">
                    <div class="vkp-nested__pane" data-nested="light">
                        <h2>"Sáng (tường minh)"</h2>
                        {swatches()}
                    </div>
                </ThemeScope>
                <ThemeScope theme=Some(ThemePreference::Dark) label="Scope Tối">
                    <div class="vkp-nested__pane" data-nested="dark">
                        <h2>"Tối (tường minh)"</h2>
                        {swatches()}
                        <ThemeScope theme=Some(ThemePreference::System) label="Scope Hệ thống">
                            <div class="vkp-nested__pane" data-nested="system">
                                <h3>"Hệ thống bên trong Tối"</h3>
                                <p>"Theo prefers-color-scheme của máy."</p>
                            </div>
                        </ThemeScope>
                    </div>
                </ThemeScope>
            </div>
        }
        .into_any();
    }
    view! {
        {swatches()}
        <div class="vkp-table" tabindex="0" role="region" aria-label="Bảng tương phản">
        <table class="vkp-contrast" data-testid="contrast-table">
            <caption>"Tỉ lệ tương phản tính từ token (WCAG 2.x); không phải chứng nhận màn hình."</caption>
            <thead>
                <tr>
                    <th scope="col">"Cặp"</th>
                    <th scope="col">"Dùng cho"</th>
                    <th scope="col">"Sáng"</th>
                    <th scope="col">"Tối"</th>
                    <th scope="col">"Ngưỡng"</th>
                </tr>
            </thead>
            <tbody>
                {CONTRAST
                    .iter()
                    .map(|pair| {
                        let light = pair.fg.light().contrast(pair.bg.light());
                        let dark = pair.fg.dark().contrast(pair.bg.dark());
                        view! {
                            <tr>
                                <th scope="row">
                                    <code>{format!("{} / {}", pair.fg.token(), pair.bg.token())}</code>
                                </th>
                                <td>{pair.usage}</td>
                                <td>{format!("{light:.2}")}</td>
                                <td>{format!("{dark:.2}")}</td>
                                <td>{format!("{}", pair.min)}</td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
        </div>
    }
    .into_any()
}

fn size_label(size: TextSize) -> String {
    match size {
        TextSize::Fixed(px) => format!("{px}px"),
        TextSize::Fluid { min, max } => format!("{min}–{max}px (fluid)"),
    }
}

fn typography(combining: bool) -> AnyView {
    let sample = |token: &str| match token {
        "type.display" if combining => fixture::LONG_TITLE,
        "type.title" | "type.reading" if combining => fixture::COMBINING,
        "type.display" => fixture::DISPLAY,
        "type.headline" => fixture::HEADLINE,
        "type.title" => fixture::TITLE,
        "type.body" => fixture::BODY,
        "type.label" => fixture::LABEL,
        "type.caption" => fixture::CAPTION,
        "type.reading" => fixture::READING,
        _ => fixture::CODE,
    };
    view! {
        {combining.then(|| view! {
            <p class="vkp-note" data-testid="nfd-note">
                {format!(
                    "Chuỗi dấu tổ hợp lưu ở dạng NFD: {} điểm mã, {} khi chuẩn hóa NFC.",
                    fixture::COMBINING.chars().count(),
                    fixture::COMBINING.nfc().count(),
                )}
            </p>
        })}
        <ul class="vkp-type">
            {[text::DISPLAY, text::HEADLINE, text::TITLE, text::BODY, text::LABEL, text::CAPTION, text::READING, text::CODE]
                .into_iter()
                .map(|role| {
                    view! {
                        <li data-role=role.token>
                            <p class="vkp-type__meta">
                                <code>{role.token}</code>
                                {format!(
                                    " · {} · {} · {}× · {}em",
                                    size_label(role.size),
                                    role.weight,
                                    role.line_height,
                                    role.tracking_em,
                                )}
                            </p>
                            <p class="vkp-type__sample" style=role.style()>{sample(role.token)}</p>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
    .into_any()
}

fn shape_spacing() -> AnyView {
    let radii = [
        dim::RADIUS_SM,
        dim::RADIUS_MD,
        dim::RADIUS_LG,
        dim::RADIUS_XL,
        dim::RADIUS_HERO,
        dim::RADIUS_FULL,
    ];
    let spaces = [
        dim::SPACE_1,
        dim::SPACE_2,
        dim::SPACE_3,
        dim::SPACE_4,
        dim::SPACE_6,
        dim::SPACE_8,
        dim::SPACE_12,
    ];
    let label = |d: Dimension| format!("{} · {}px", d.token, d.value);
    view! {
        <h2 class="vkp-h">"Radius"</h2>
        <ul class="vkp-radii">
            {radii
                .into_iter()
                .map(|d| view! {
                    <li><span class="vkp-radius" style=format!("border-radius: {}", d.var())></span><code>{label(d)}</code></li>
                })
                .collect_view()}
        </ul>
        <h2 class="vkp-h">"Khoảng cách"</h2>
        <ul class="vkp-spaces">
            {spaces
                .into_iter()
                .map(|d| view! {
                    <li><span class="vkp-space" style=format!("inline-size: {}", d.var())></span><code>{label(d)}</code></li>
                })
                .collect_view()}
        </ul>
        <h2 class="vkp-h">"Kích thước chạm và mật độ"</h2>
        <div class="vkp-density" data-testid="density-sample">
            <button type="button" class="vkp-control">"Lưu bản nháp"</button>
            <button type="button" class="vkp-control">"Xem trước"</button>
            <p>
                <code>{label(dim::SIZE_HIT_TARGET)}</code>
                " · padding theo "
                <code>"density.control-padding"</code>
                {format!(
                    " ({}px thoải mái / {}px gọn)",
                    dim::DENSITY_CONTROL_PADDING.value,
                    dim::DENSITY_CONTROL_PADDING.compact.unwrap_or_default(),
                )}
            </p>
        </div>
    }
    .into_any()
}

fn motion_specimen() -> AnyView {
    let moved = RwSignal::new(false);
    view! {
        <div class="vkp-motion">
            <div>
                <h2 class="vkp-h">"Chuyển tiếp hữu hạn"</h2>
                <button type="button" class="vkp-control" on:click=move |_| moved.update(|m| *m = !*m)>
                    "Chạy chuyển tiếp"
                </button>
                <div class="vkp-track">
                    <span class="vkp-mover" class:is-moved=move || moved.get()></span>
                </div>
                <p><code>"duration.medium · ease.spatial"</code>" — giảm chuyển động: 0ms (tức thì)."</p>
            </div>
            <div>
                <h2 class="vkp-h">"Vòng lặp tải"</h2>
                <span class="vkp-loop" role="img" aria-label="Chỉ báo đang tải (mẫu)"></span>
                <p><code>"duration.loop · loop.iterations"</code>" — giảm chuyển động: dừng, không lặp 0ms."</p>
            </div>
        </div>
    }
    .into_any()
}

fn current_hash() -> String {
    #[cfg(feature = "csr")]
    {
        window().location().hash().unwrap_or_default()
    }
    #[cfg(not(feature = "csr"))]
    {
        String::new()
    }
}

#[cfg(feature = "csr")]
fn read_effective() -> Vec<(String, String)> {
    let Some(element) = document().get_element_by_id("specimen-scope") else {
        return Vec::new();
    };
    let Ok(Some(style)) = window().get_computed_style(&element) else {
        return Vec::new();
    };
    INSPECTED
        .iter()
        .map(|name| {
            let value = style.get_property_value(name).unwrap_or_default();
            (name.to_string(), value.trim().to_string())
        })
        .collect()
}

#[cfg(feature = "csr")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shared_catalog_entry_is_registered_here() {
        let ids: Vec<&str> = ENTRIES.iter().map(|e| e.id).collect();
        assert_eq!(ids, REGISTERED);
        for entry in ENTRIES {
            assert!(catalog::STATUSES.contains(&entry.leptos), "{}", entry.id);
        }
    }

    #[test]
    fn each_scenario_renders_its_specimen_inside_a_scope() {
        for entry in ENTRIES {
            for scenario in entry.scenarios {
                let html = Owner::new().with(|| specimen(entry.id, scenario.id).to_html());
                assert!(html.contains("class=\"vk-scope\""), "{html}");
                assert!(
                    html.contains(&format!("data-catalog-id=\"{}\"", entry.id)),
                    "{}",
                    entry.id
                );
                assert!(
                    !html.contains("Chưa đăng ký"),
                    "{} / {}",
                    entry.id,
                    scenario.id
                );
            }
        }
        let compact =
            Owner::new().with(|| specimen("foundation.shape-spacing", "compact").to_html());
        assert!(compact.contains("data-density=\"compact\""));
        let reduced = Owner::new().with(|| specimen("foundation.motion", "reduced").to_html());
        assert!(reduced.contains("data-motion=\"reduce\""));
    }

    #[test]
    fn no_scenario_repeats_an_element_id() {
        for entry in ENTRIES {
            for scenario in entry.scenarios {
                let html = Owner::new().with(|| specimen(entry.id, scenario.id).to_html());
                let mut ids: Vec<&str> = html
                    .split(" id=\"")
                    .skip(1)
                    .filter_map(|rest| rest.split('"').next())
                    .collect();
                let total = ids.len();
                ids.sort_unstable();
                ids.dedup();
                assert_eq!(ids.len(), total, "{} / {}: {html}", entry.id, scenario.id);
            }
        }
    }

    #[test]
    fn hashes_select_exact_scenarios_and_fall_back_safely() {
        let s = parse_hash("#foundation.typography/combining-marks");
        assert_eq!(
            (s.entry.id, s.scenario),
            ("foundation.typography", "combining-marks")
        );
        let s = parse_hash("#foundation.motion/nope");
        assert_eq!((s.entry.id, s.scenario), ("foundation.motion", "default"));
        let s = parse_hash("");
        assert_eq!((s.entry.id, s.scenario), (ENTRIES[0].id, "default"));
    }

    #[test]
    fn search_ignores_case_and_vietnamese_marks() {
        let colors = &ENTRIES[0];
        assert!(matches(colors, "MAU"));
        assert!(matches(colors, "màu"));
        assert!(matches(colors, "  "));
        assert!(!matches(colors, "chuyen dong"));
        assert!(matches(&ENTRIES[3], "chuyen dong"));
        assert!(matches(&ENTRIES[1], "dau to hop"));
    }

    #[test]
    fn typography_specimen_shows_every_text_role() {
        let html = Owner::new().with(|| typography(false).to_html());
        for role in text::ALL {
            assert!(
                html.contains(&format!("data-role=\"{}\"", role.token)),
                "{}",
                role.token
            );
        }
    }

    #[test]
    fn nfd_fixture_is_decomposed() {
        assert!(fixture::COMBINING.chars().count() > fixture::COMBINING.nfc().count());
    }
}
