//! The desk's body editor: a CodeMirror 6 view behind a thin wasm-bindgen bridge.
//!
//! The runtime lives in `apps/vink-web/editor` (TypeScript, built by `cargo xtask build` into
//! `/pkg/vink-editor.js`) and is fetched only when an editor mounts. CodeMirror owns the live body
//! text; [`BodySource::snapshot`] is the one way save, preview, export and the leave guard read
//! it. Save state stays with [`crate::desk_state::EditorSession`]. One view lives per open draft:
//! reactive updates around it (status, notices, dialogs, theme) never remount it.

use leptos::html;
use leptos::prelude::*;

/// Ids of the visible label and hint that name and describe the text box.
pub const BODY_LABEL_ID: &str = "editor-body-label";
pub const BODY_HINT_ID: &str = "editor-body-hint";
/// Id of the sticky toolbar above the text; the caret is scrolled clear of it.
pub const TOOLBAR_ID: &str = "editor-toolbar";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyState {
    Loading,
    Ready,
    Failed,
}

/// The body text of one open draft: the mounted view once ready, the loaded text before that.
#[derive(Clone, Copy)]
pub struct BodySource {
    loaded: StoredValue<String>,
    #[cfg(feature = "hydrate")]
    view: StoredValue<Option<bridge::EditorHandle>, LocalStorage>,
    pub state: RwSignal<BodyState>,
}

impl BodySource {
    pub fn new(loaded: String) -> Self {
        Self {
            loaded: StoredValue::new(loaded),
            #[cfg(feature = "hydrate")]
            view: StoredValue::new_local(None),
            state: RwSignal::new(BodyState::Loading),
        }
    }

    /// The text being written right now. Before the view mounts nothing can have changed it.
    pub fn snapshot(&self) -> String {
        #[cfg(feature = "hydrate")]
        if let Some(text) = self
            .view
            .try_with_value(|view| view.as_ref().map(bridge::EditorHandle::text))
            .flatten()
        {
            return text;
        }
        self.loaded.try_get_value().unwrap_or_default()
    }

    pub fn focus(&self) {
        #[cfg(feature = "hydrate")]
        self.view.try_with_value(|view| {
            if let Some(view) = view {
                view.focus();
            }
        });
    }

    /// True while an input method composes text that is not committed yet.
    pub fn is_composing(&self) -> bool {
        #[cfg(feature = "hydrate")]
        {
            self.view
                .try_with_value(|view| {
                    view.as_ref()
                        .is_some_and(bridge::EditorHandle::is_composing)
                })
                .unwrap_or(false)
        }
        #[cfg(not(feature = "hydrate"))]
        {
            false
        }
    }
}

/// Host element of the CodeMirror view. `on_change` runs after every document change and
/// `on_ready` once the view is mounted. A load that finishes after the owner is gone, or after
/// another draft replaced it, creates nothing.
#[component]
pub fn BodyEditor(
    source: BodySource,
    on_change: Callback<()>,
    on_ready: Callback<()>,
) -> impl IntoView {
    let host = NodeRef::<html::Div>::new();
    #[cfg(feature = "hydrate")]
    {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let alive = Arc::new(AtomicBool::new(true));
        let cleanup_alive = alive.clone();
        on_cleanup(move || {
            cleanup_alive.store(false, Ordering::Relaxed);
            // Destroys the view before its callback closure is released.
            source.view.try_update_value(|view| *view = None);
        });
        Effect::new(move |_| {
            let Some(host) = host.get() else {
                return;
            };
            if source.view.with_value(Option::is_some) {
                return;
            }
            let alive = alive.clone();
            leptos::task::spawn_local(async move {
                let loaded = bridge::EditorHandle::load().await;
                if !alive.load(Ordering::Relaxed) {
                    return;
                }
                let mounted = loaded.and_then(|()| {
                    bridge::EditorHandle::mount(&host, &source.loaded.get_value(), move || {
                        on_change.run(())
                    })
                });
                match mounted {
                    Ok(view) => {
                        source.view.set_value(Some(view));
                        source.state.set(BodyState::Ready);
                        on_ready.run(());
                    }
                    Err(error) => {
                        leptos::logging::warn!("Vink editor did not load: {error:?}");
                        source.state.set(BodyState::Failed);
                    }
                }
            });
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (on_change, on_ready);
    view! {
        <div
            class="vk-editor-body"
            data-testid="editor-body"
            data-state=move || match source.state.get() {
                BodyState::Loading => "loading",
                BodyState::Ready => "ready",
                BodyState::Failed => "failed",
            }
            node_ref=host
        ></div>
    }
}

#[cfg(feature = "hydrate")]
mod bridge {
    use js_sys::{Object, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::*;
    use web_sys::HtmlElement;

    use super::{BODY_HINT_ID, BODY_LABEL_ID, TOOLBAR_ID};

    #[wasm_bindgen(module = "/editor/bridge.js")]
    extern "C" {
        #[wasm_bindgen(js_name = loadEditorModule)]
        fn load_editor_module() -> js_sys::Promise;
        #[wasm_bindgen(catch, js_name = createEditor)]
        fn create_editor(host: &HtmlElement, options: &JsValue) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_name = destroyEditor)]
        fn destroy_editor(handle: &JsValue);
        #[wasm_bindgen(js_name = getText)]
        fn get_text(handle: &JsValue) -> String;
        #[wasm_bindgen(js_name = focusEditor)]
        fn focus_editor(handle: &JsValue);
        #[wasm_bindgen(js_name = isComposing)]
        fn is_composing(handle: &JsValue) -> bool;
    }

    /// A mounted view. Dropping it destroys the view, then releases the change callback.
    pub struct EditorHandle {
        handle: JsValue,
        _on_change: Closure<dyn FnMut()>,
    }

    impl EditorHandle {
        pub async fn load() -> Result<(), JsValue> {
            wasm_bindgen_futures::JsFuture::from(load_editor_module()).await?;
            Ok(())
        }

        pub fn mount(
            host: &HtmlElement,
            text: &str,
            on_change: impl FnMut() + 'static,
        ) -> Result<Self, JsValue> {
            let on_change = Closure::wrap(Box::new(on_change) as Box<dyn FnMut()>);
            let options = Object::new();
            for (key, value) in [
                ("text", JsValue::from_str(text)),
                ("labelledBy", JsValue::from_str(BODY_LABEL_ID)),
                ("describedBy", JsValue::from_str(BODY_HINT_ID)),
                ("stickyAboveId", JsValue::from_str(TOOLBAR_ID)),
            ] {
                Reflect::set(&options, &JsValue::from_str(key), &value)?;
            }
            Reflect::set(&options, &JsValue::from_str("onChange"), on_change.as_ref())?;
            let handle = create_editor(host, &options)?;
            Ok(Self {
                handle,
                _on_change: on_change,
            })
        }

        pub fn text(&self) -> String {
            get_text(&self.handle)
        }

        pub fn focus(&self) {
            focus_editor(&self.handle);
        }

        pub fn is_composing(&self) -> bool {
            is_composing(&self.handle)
        }
    }

    impl Drop for EditorHandle {
        fn drop(&mut self) {
            destroy_editor(&self.handle);
        }
    }
}
