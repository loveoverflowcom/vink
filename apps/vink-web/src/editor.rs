//! The desk's body editor: a CodeMirror 6 view behind a thin wasm-bindgen bridge.
//!
//! The runtime lives in `apps/vink-web/editor` (TypeScript, built by `cargo xtask build` into
//! `/pkg/vink-editor.js`) and is fetched only when an editor mounts. CodeMirror owns the live body
//! text; [`BodySource::snapshot`] is the one way save, the format conversion preview and the leave
//! guard read it. Save state stays with [`crate::desk_state::EditorSession`]. One view lives per open draft:
//! reactive updates around it (status, notices, dialogs, theme, format) never remount it.

use leptos::html;
use leptos::prelude::*;
use serde::Deserialize;

use crate::contract::DocumentFormatDto;

/// The surface controls editing capabilities independently of the persisted body's representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPreset {
    Document,
    Comment,
}

impl EditorPreset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Comment => "comment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFormat {
    Plain,
    Markdown,
}

impl BodyFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Markdown => "markdown",
        }
    }
}

impl From<DocumentFormatDto> for BodyFormat {
    fn from(format: DocumentFormatDto) -> Self {
        match format {
            DocumentFormatDto::Default => Self::Plain,
            DocumentFormatDto::Markdown => Self::Markdown,
        }
    }
}

/// Ids of the visible label and hint that name and describe the text box.
pub const BODY_LABEL_ID: &str = "editor-body-label";
pub const BODY_HINT_ID: &str = "editor-body-hint";
pub const BODY_TITLE_ERROR_ID: &str = "editor-body-title-error";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyState {
    Loading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItem {
    pub level: u8,
    pub title: String,
    pub from: usize,
    pub is_title: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutlineStatus {
    #[default]
    Ready,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineSnapshot {
    pub status: OutlineStatus,
    pub generation: u64,
    pub items: Vec<OutlineItem>,
    pub title: String,
    pub active_from: Option<usize>,
}

/// The body text of one open draft: the mounted view once ready, the loaded text before that.
#[derive(Clone, Copy)]
pub struct BodySource {
    format: StoredValue<DocumentFormatDto>,
    loaded: StoredValue<String>,
    #[cfg(feature = "browser")]
    view: StoredValue<Option<bridge::EditorHandle>, LocalStorage>,
    pub state: RwSignal<BodyState>,
    pub outline: RwSignal<OutlineSnapshot>,
}

impl BodySource {
    pub fn new(format: DocumentFormatDto, loaded: String) -> Self {
        Self {
            format: StoredValue::new(format),
            loaded: StoredValue::new(loaded),
            #[cfg(feature = "browser")]
            view: StoredValue::new_local(None),
            state: RwSignal::new(BodyState::Loading),
            outline: RwSignal::new(OutlineSnapshot::default()),
        }
    }

    /// The text being written right now. Before the view mounts nothing can have changed it.
    pub fn snapshot(&self) -> String {
        #[cfg(feature = "browser")]
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
        #[cfg(feature = "browser")]
        self.view.try_with_value(|view| {
            if let Some(view) = view {
                view.focus();
            }
        });
    }

    /// True while an input method composes text that is not committed yet.
    pub fn is_composing(&self) -> bool {
        #[cfg(feature = "browser")]
        {
            self.view
                .try_with_value(|view| {
                    view.as_ref()
                        .is_some_and(bridge::EditorHandle::is_composing)
                })
                .unwrap_or(false)
        }
        #[cfg(not(feature = "browser"))]
        {
            false
        }
    }

    pub fn apply_converted_document(&self, format: DocumentFormatDto, text: &str) {
        self.format.try_set_value(format);
        self.loaded.try_set_value(text.to_owned());
        #[cfg(feature = "browser")]
        self.view.try_with_value(|view| {
            if let Some(view) = view {
                view.apply_converted_document(format, text);
            }
        });
    }

    pub fn apply_heading(&self, level: u8) -> bool {
        #[cfg(feature = "browser")]
        {
            self.view
                .try_with_value(|view| view.as_ref().is_some_and(|v| v.apply_heading(level)))
                .unwrap_or(false)
        }
        #[cfg(not(feature = "browser"))]
        {
            let _ = level;
            false
        }
    }

    pub fn apply_inline_format(&self, kind: &str) -> bool {
        #[cfg(feature = "browser")]
        {
            self.view
                .try_with_value(|view| view.as_ref().is_some_and(|v| v.apply_inline_format(kind)))
                .unwrap_or(false)
        }
        #[cfg(not(feature = "browser"))]
        {
            let _ = kind;
            false
        }
    }

    pub fn jump_to_heading(&self, from: usize, generation: u64) -> bool {
        #[cfg(feature = "browser")]
        {
            self.view
                .try_with_value(|view| {
                    view.as_ref()
                        .is_some_and(|v| v.jump_to_heading(from, generation))
                })
                .unwrap_or(false)
        }
        #[cfg(not(feature = "browser"))]
        {
            let _ = (from, generation);
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
    #[cfg(feature = "browser")]
    {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let alive = Arc::new(AtomicBool::new(true));
        let cleanup_alive = alive.clone();
        on_cleanup(move || {
            cleanup_alive.store(false, Ordering::Relaxed);
            // Destroys the view before its callback closures are released.
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
                if !alive.load(Ordering::Relaxed) || !host.is_connected() {
                    return;
                }
                let outline_signal = source.outline;
                let format = source.format.get_value();
                let mounted = loaded.and_then(|()| {
                    bridge::EditorHandle::mount(
                        &host,
                        bridge::MountOptions {
                            preset: EditorPreset::Document,
                            text: &source.loaded.get_value(),
                            format: format.into(),
                            labelled_by: BODY_LABEL_ID,
                            described_by: &format!(
                                "{BODY_HINT_ID} {BODY_TITLE_ERROR_ID} submission-body-errors"
                            ),
                            read_only: false,
                            on_change: Box::new(move || on_change.run(())),
                            // The desk form observes composition from title, body and tag inputs.
                            on_composition: None,
                            on_outline: Some(Box::new(move |json: String| {
                                if let Ok(snapshot) = serde_json::from_str::<OutlineSnapshot>(&json)
                                {
                                    outline_signal.try_set(snapshot);
                                }
                            })),
                        },
                    )
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
    #[cfg(not(feature = "browser"))]
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

#[cfg(feature = "browser")]
pub mod bridge {
    use js_sys::{Object, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::*;
    use web_sys::HtmlElement;

    use super::{BodyFormat, EditorPreset};
    use crate::contract::DocumentFormatDto;

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
        #[wasm_bindgen(js_name = applyConvertedDocument)]
        fn apply_converted_document(handle: &JsValue, format: &str, text: &str);
        #[wasm_bindgen(js_name = applyHeading)]
        fn apply_heading(handle: &JsValue, level: u8) -> bool;
        #[wasm_bindgen(js_name = applyInlineFormat)]
        fn apply_inline_format(handle: &JsValue, kind: &str) -> bool;
        #[wasm_bindgen(js_name = jumpToHeading)]
        fn jump_to_heading(handle: &JsValue, from: f64, generation: f64) -> bool;
    }

    pub struct MountOptions<'a> {
        pub preset: EditorPreset,
        pub format: BodyFormat,
        pub text: &'a str,
        pub labelled_by: &'a str,
        pub described_by: &'a str,
        pub read_only: bool,
        pub on_change: Box<dyn FnMut()>,
        pub on_composition: Option<Box<dyn FnMut(bool)>>,
        pub on_outline: Option<Box<dyn FnMut(String)>>,
    }

    /// A mounted view. Dropping it destroys the view, then releases the callbacks.
    pub struct EditorHandle {
        handle: JsValue,
        _on_change: Closure<dyn FnMut()>,
        _on_outline: Option<Closure<dyn FnMut(String)>>,
        _on_composition: Option<Closure<dyn FnMut(bool)>>,
    }

    impl EditorHandle {
        pub async fn load() -> Result<(), JsValue> {
            wasm_bindgen_futures::JsFuture::from(load_editor_module()).await?;
            Ok(())
        }

        pub fn mount(host: &HtmlElement, config: MountOptions<'_>) -> Result<Self, JsValue> {
            let on_change = Closure::wrap(config.on_change);
            let on_composition = config.on_composition.map(Closure::wrap);
            let on_outline = config
                .on_outline
                .filter(|_| config.preset == EditorPreset::Document)
                .map(Closure::wrap);
            let options = Object::new();
            for (key, value) in [
                ("text", config.text),
                ("preset", config.preset.as_str()),
                ("format", config.format.as_str()),
                ("labelledBy", config.labelled_by),
                ("describedBy", config.described_by),
            ] {
                Reflect::set(&options, &key.into(), &value.into())?;
            }
            Reflect::set(&options, &"readOnly".into(), &config.read_only.into())?;
            Reflect::set(&options, &"onChange".into(), on_change.as_ref())?;
            if let Some(callback) = &on_outline {
                Reflect::set(&options, &"onOutline".into(), callback.as_ref())?;
            }
            if let Some(callback) = &on_composition {
                Reflect::set(&options, &"onComposition".into(), callback.as_ref())?;
            }
            let handle = create_editor(host, &options)?;
            Ok(Self {
                handle,
                _on_change: on_change,
                _on_outline: on_outline,
                _on_composition: on_composition,
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

        pub fn apply_converted_document(&self, format: DocumentFormatDto, text: &str) {
            apply_converted_document(&self.handle, BodyFormat::from(format).as_str(), text);
        }

        pub fn apply_heading(&self, level: u8) -> bool {
            apply_heading(&self.handle, level)
        }

        pub fn apply_inline_format(&self, kind: &str) -> bool {
            apply_inline_format(&self.handle, kind)
        }

        pub fn jump_to_heading(&self, from: usize, generation: u64) -> bool {
            jump_to_heading(&self.handle, from as f64, generation as f64)
        }
    }

    impl Drop for EditorHandle {
        fn drop(&mut self) {
            destroy_editor(&self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_default_maps_to_plain_for_either_surface() {
        assert_eq!(
            BodyFormat::from(DocumentFormatDto::Default).as_str(),
            "plain"
        );
        assert_eq!(
            BodyFormat::from(DocumentFormatDto::Markdown).as_str(),
            "markdown"
        );
        assert_eq!(EditorPreset::Document.as_str(), "document");
        assert_eq!(EditorPreset::Comment.as_str(), "comment");
        assert_eq!(DocumentFormatDto::Default.as_str(), "default");
    }
}
