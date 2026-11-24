//! Isolated EMO-01A consumer. Static Preview fixture and exact Unicode, no comment/reaction authority.
use leptos::{html, prelude::*};
use vink_expressions::catalog::{ASSETS, COUNT};
use vink_ui::{Button, ButtonVariant, Heading, HeadingLevel, TypeRole};

const UNICODE_SAMPLE: &str =
    "Một câu thơ còn ấm ❤️‍🔥\n  lời cảm ơn vẫn nguyên vẹn 🙏 👍🏽\n\nTrang sách có cả 👩‍👩‍👧‍👦, 🇻🇳 và 1️⃣.";

#[component]
fn ExpressionPilot(on_exit: Callback<()>) -> impl IntoView {
    let host = NodeRef::<html::Div>::new();
    let failed = RwSignal::new(false);
    #[cfg(all(target_arch = "wasm32", feature = "media-hydrate"))]
    {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let alive = Arc::new(AtomicBool::new(true));
        let cleanup_alive = alive.clone();
        let handle = StoredValue::new_local(None::<bridge::PilotHandle>);
        on_cleanup(move || {
            cleanup_alive.store(false, Ordering::Relaxed);
            handle.try_update_value(|value| *value = None);
        });
        Effect::new(move |_| {
            let Some(host) = host.get() else {
                return;
            };
            let alive = alive.clone();
            leptos::task::spawn_local(async move {
                let loaded = bridge::PilotHandle::load().await;
                if !alive.load(Ordering::Relaxed) {
                    return;
                }
                match loaded.and_then(|module| bridge::PilotHandle::mount(&module, &host)) {
                    Ok(mounted) => {
                        handle.set_value(Some(mounted));
                    }
                    Err(_) => {
                        failed.set(true);
                    }
                }
            });
        });
    }
    view! {
        <div id="expression-pilot" class="vk-expression-pilot" node_ref=host data-ready="false">
            <header id="pilot-heading">
                <p class="vk-eyebrow">"Thử nghiệm độc lập · Mẫu"</p>
                <Heading level=HeadingLevel::H1 role=TypeRole::Headline>"Một chút biểu cảm"</Heading>
                <p id="pilot-hint">"Hình luôn hiện tĩnh trước. Chọn một biểu cảm để phát một lần; tối đa hai lượt cùng lúc. Khi giảm chuyển động, hình giữ nguyên tĩnh."</p>
                <a id="expression-exit" class="vk-ui-link" href="/" on:click=move |ev| {
                    if !ev.ctrl_key() && !ev.meta_key() && !ev.shift_key() && !ev.alt_key() {
                        ev.prevent_default();
                        on_exit.run(());
                    }
                }>"Về thư viện mẫu"</a>
            </header>
            <section id="unicode-sample" aria-labelledby="unicode-title">
                <Heading level=HeadingLevel::H2 role=TypeRole::Title id="unicode-title">"Câu chữ vẫn nguyên vẹn"</Heading>
                <p class="vk-expression-poem">{UNICODE_SAMPLE}</p>
                <p class="vk-muted">"Emoji trong văn bản vẫn là Unicode. Thử nghiệm này không gửi bình luận, sticker hay reaction."</p>
            </section>
            <section aria-labelledby="catalog-title" aria-describedby="pilot-hint">
                <Heading level=HeadingLevel::H2 role=TypeRole::Title id="catalog-title">"Chọn để phát một lần"</Heading>
                <ul id="emoji-catalog" class="vk-expression-grid" role="list">
                    {ASSETS.into_iter().enumerate().map(|(index, asset)| view! {
                        <li data-expression-index=index data-phase="static" data-generation="0">
                            <div class="vk-expression-art" aria-hidden="true">
                                <span class="vk-expression-unicode">{asset.unicode}</span>
                                <img src=format!("/pkg/expressions/assets/{}", asset.poster) alt=asset.unicode width="96" height="96" />
                            </div>
                            <Button id=format!("expression-{index}") variant=ButtonVariant::Tonal>{asset.label_vi}</Button>
                            <p class="vk-expression-status" data-player-status>"Phát một lần"</p>
                        </li>
                    }).collect_view()}
                </ul>
            </section>
            <p id="expression-status" class="vk-muted" role="status" aria-live="polite">"Chuyển động chỉ tải khi bạn chọn một biểu cảm."</p>
            <Show when=move || failed.get()>
                <p role="status">"Chưa tải được phần thử chuyển động. Bạn vẫn có thể xem hình và đọc Unicode."</p>
            </Show>
            <details id="runtime-diagnostics">
                <summary>"Thông tin thử nghiệm"</summary>
                <p id="expression-budget">{format!("0 / 2 lượt hoạt động · {COUNT} biểu cảm Google Noto")}</p>
                <p>"dotLottie Web · Software · chưa so sánh hiệu năng với Worker/WebGL2. Hình tĩnh không cần engine chuyển động."</p>
            </details>
            <footer class="vk-expression-attribution">
                <p>"Artwork: Google Noto Emoji. SVG tĩnh: Apache-2.0. Animated Noto Emoji: CC BY 4.0. Artwork không thay đổi; nhãn Việt và tên tệp có hash do Vink bổ sung."</p>
                <p>
                    <a href="https://googlefonts.github.io/noto-emoji-files/?view=docs">"Nguồn Google"</a>" · "
                    <a href="https://creativecommons.org/licenses/by/4.0/">"CC BY 4.0"</a>" · "
                    <a href="/pkg/expressions/NOTICE.txt">"Giấy phép và ghi công"</a>
                </p>
            </footer>
        </div>
    }
}

#[cfg(all(target_arch = "wasm32", feature = "media-hydrate"))]
mod bridge {
    use super::{ASSETS, COUNT};
    use vink_expressions::policy::{Admission, Policy};
    use wasm_bindgen::{closure::Closure, prelude::*};

    #[wasm_bindgen(module = "/expressions/bridge.js")]
    extern "C" {
        #[wasm_bindgen(js_name = loadPilotModule)]
        fn load_pilot_module() -> js_sys::Promise;
        #[wasm_bindgen(catch, js_name = createPilot)]
        fn create_pilot(
            module: &JsValue,
            host: &web_sys::HtmlElement,
            decide: &JsValue,
        ) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_name = destroyPilot)]
        fn destroy_pilot(handle: &JsValue);
    }

    type DecisionCallback = Closure<dyn FnMut(String, u32, u32, bool) -> String>;
    pub struct PilotHandle {
        handle: JsValue,
        _decide: DecisionCallback,
    }

    impl PilotHandle {
        pub async fn load() -> Result<JsValue, JsValue> {
            wasm_bindgen_futures::JsFuture::from(load_pilot_module()).await
        }

        pub fn mount(module: &JsValue, host: &web_sys::HtmlElement) -> Result<Self, JsValue> {
            let mut policy = Policy::<COUNT>::new(2, ASSETS.map(|asset| asset.animation.is_some()));
            let mut environment = (true, false, false);
            let decide = Closure::wrap(Box::new(
                move |event: String, index: u32, generation: u32, flag: bool| {
                    let index = index as usize;
                    let mut action = "none";
                    match event.as_str() {
                        "activate" => {
                            if matches!(policy.activate(index), Admission::Start(_)) {
                                action = "load";
                            }
                        }
                        "ready" => {
                            if policy.ready(index, generation) {
                                action = "play";
                            }
                        }
                        "complete" => {
                            policy.release(index, generation);
                        }
                        "error" => {
                            policy.fail(index, generation);
                        }
                        "cancel" => policy.cancel(index),
                        "dispose" => policy.dispose(index),
                        "visible" => policy.set_visible(index, flag),
                        "reduced" => environment.0 = flag,
                        "foreground" => environment.1 = flag,
                        "runtime" => environment.2 = flag,
                        _ => {}
                    }
                    if matches!(event.as_str(), "reduced" | "foreground" | "runtime") {
                        policy.set_environment(environment.0, environment.1, environment.2);
                    }
                    let players: Vec<_> = (0..COUNT).filter_map(|i| policy.player(i)).map(|player| serde_json::json!({
                    "phase": player.phase().as_str(), "generation": player.generation(), "label": player.label(),
                })).collect();
                    serde_json::json!({ "action": action, "generation": policy.player(index).map(|p| p.generation()).unwrap_or(0), "active": policy.active(), "capacity": policy.capacity(), "players": players }).to_string()
                },
            )
                as Box<dyn FnMut(String, u32, u32, bool) -> String>);
            let handle = create_pilot(module, host, decide.as_ref())?;
            Ok(Self {
                handle,
                _decide: decide,
            })
        }
    }
    impl Drop for PilotHandle {
        fn drop(&mut self) {
            destroy_pilot(&self.handle);
        }
    }
}

/// Shared entry for the static renderer and its browser hydration example.
pub fn specimen() -> impl IntoView {
    let mounted = RwSignal::new(true);
    view! {
        <main class="vk-ui vk-scope vk-expression-fixture">
            <Show when=move || mounted.get() fallback=|| view! {
                <p id="expression-closed"><a class="vk-ui-link" href="/">"Về thư viện mẫu"</a></p>
            }>
                <ExpressionPilot on_exit=Callback::new(move |()| mounted.set(false)) />
            </Show>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_is_static_named_and_preserves_source_unicode() {
        let owner = Owner::new();
        let html = owner.with(|| {
            specimen().to_html()
        });
        assert_eq!(html.matches("data-expression-index=").count(), COUNT);
        assert!(html.contains(UNICODE_SAMPLE));
        assert!(!html.contains("<canvas"));
        assert!(!html.contains("/runtime/"));
        assert!(!html.contains("lottie.json"));
        assert!(html.contains("CC BY 4.0"));
    }
}
