//! The Discussion adapter owns the native field, focus and the explicit upgrade gesture.
use leptos::{html, prelude::*};
use vink_ui::{FieldMode, FieldVariant, TextArea, TextInput};

use crate::discussion_state::{ComposerKey, DiscussionController};

#[cfg(feature = "browser")]
pub(super) fn focus_target(doc: &web_sys::Document, id: &str) -> Option<web_sys::Element> {
    doc.get_element_by_id(&format!("{id}-rich"))
        .and_then(|host| host.query_selector(".cm-content").ok().flatten())
        .or_else(|| doc.get_element_by_id(id))
}

#[component]
pub(super) fn CompactField(
    id: String,
    label: &'static str,
    description: Option<String>,
    controller: RwSignal<DiscussionController>,
    composer: StoredValue<ComposerKey>,
    value: Memo<String>,
    on_input: Callback<TextInput>,
    mode: Signal<FieldMode>,
) -> impl IntoView {
    let host = NodeRef::<html::Div>::new();
    let active = RwSignal::new(false);
    let loading = RwSignal::new(false);
    let failed = RwSignal::new(false);
    let edited = RwSignal::new(false);
    let field_id = StoredValue::new(id.clone());
    let activate_id = format!("{id}-format");
    let rich_id = format!("{id}-rich");
    let label_id = format!("{id}-rich-label");
    #[cfg(feature = "hydrate")]
    if document()
        .document_element()
        .is_some_and(|root| !root.has_attribute("data-hydrated"))
        && let Some(field) = document().get_element_by_id(&id).and_then(|node| {
            wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlTextAreaElement>(node).ok()
        })
        && value.get_untracked().is_empty()
        && !field.value().is_empty()
    {
        // SSR was already editable before WASM started. Adopt the DOM before binding value.
        edited.set(true);
        on_input.run(TextInput {
            value: field.value(),
            composing: false,
        });
    }
    let native_input = move |event: TextInput| {
        edited.set(true);
        on_input.run(event);
    };

    #[cfg(feature = "browser")]
    let (activate, command) = {
        use crate::editor::{
            BodyFormat, EditorPreset,
            bridge::{EditorHandle, MountOptions},
        };
        use wasm_bindgen::JsCast;
        let handle = StoredValue::new_local(None::<EditorHandle>);
        let owner = Memo::new(move |_| controller.read().draft_owner);
        let native_epoch = StoredValue::new(0_u64);
        let observed = StoredValue::new(None);
        let description = StoredValue::new(description.clone().unwrap_or_default());
        let label_id = StoredValue::new(label_id.clone());
        let dispose = move || {
            handle.try_update_value(|view| *view = None);
            active.try_set(false);
        };
        on_cleanup(move || {
            handle.try_update_value(|view| *view = None);
        });
        // A changed scope, a closed thread, or an acknowledged clear ends this view's history.
        Effect::new(move |_| {
            let scope = owner.get();
            let text = value.get();
            let changed_owner = observed.with_value(|old| old.as_ref() != Some(&scope));
            if changed_owner
                || mode.get() != FieldMode::Editable
                || handle.with_value(|view| view.as_ref().is_some_and(|v| v.text() != text))
            {
                native_epoch.update_value(|n| *n += 1);
                dispose();
                loading.set(false);
            }
            if changed_owner
                || (text.is_empty()
                    && controller.with(|c| c.draft(&composer.get_value()).acknowledged.is_some()))
            {
                edited.set(!text.is_empty());
            }
            observed.set_value(Some(scope));
        });
        let activate = move || {
            if active.get_untracked()
                || loading.get_untracked()
                || edited.get_untracked()
                || mode.get_untracked() != FieldMode::Editable
            {
                return;
            }
            let Some(node) = host.get_untracked() else {
                return;
            };
            let Some(doc) = node.owner_document() else {
                return;
            };
            let Some(button) = doc.get_element_by_id(&format!("{}-format", field_id.get_value()))
            else {
                return;
            };
            let Some(field) = doc
                .get_element_by_id(&field_id.get_value())
                .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
            else {
                return;
            };
            // A cold import never replaces a selected or actively edited native draft.
            if !field.value().is_empty()
                || controller
                    .read_untracked()
                    .draft(&composer.get_value())
                    .composing
            {
                return;
            }
            let generation = owner.get_untracked();
            let scope = controller.read_untracked().scope.clone();
            let epoch = native_epoch.get_value();
            let focused = doc.active_element();
            failed.set(false);
            loading.set(true);
            leptos::task::spawn_local(async move {
                let loaded = EditorHandle::load().await;
                if !node.is_connected() || native_epoch.try_get_value() != Some(epoch) {
                    return;
                }
                loading.set(false);
                if mode.get_untracked() != FieldMode::Editable
                    || controller.read_untracked().scope != scope
                    || owner.get_untracked() != generation
                    || !button.is_connected()
                    || !node
                        .parent_element()
                        .is_some_and(|el| el.get_bounding_client_rect().width() > 0.0)
                    || doc.active_element() != focused
                    || doc.active_element().as_ref() == Some(field.as_ref())
                    || edited.get_untracked()
                    || !field.value().is_empty()
                    || !value.get_untracked().is_empty()
                    || controller
                        .read_untracked()
                        .draft(&composer.get_value())
                        .composing
                {
                    return;
                }
                let emit = move |composing: bool| {
                    if controller
                        .with_untracked(|c| c.draft_owner == generation && c.settings.comments_open)
                    {
                        handle.with_value(|view| {
                            if let Some(view) = view {
                                on_input.run(TextInput {
                                    value: view.text(),
                                    composing,
                                });
                            }
                        });
                    }
                };
                let composition = std::rc::Rc::new(std::cell::Cell::new(false));
                let mounted = loaded.and_then(|()| {
                    EditorHandle::mount(
                        &node,
                        MountOptions {
                            preset: EditorPreset::Comment,
                            format: BodyFormat::Markdown,
                            text: "",
                            labelled_by: &label_id.get_value(),
                            described_by: &description.get_value(),
                            read_only: false,
                            on_change: Box::new({
                                let emit_change = emit;
                                let composition = composition.clone();
                                move || emit_change(composition.get())
                            }),
                            on_composition: Some(Box::new(move |state| {
                                composition.set(state);
                                emit(state);
                            })),
                            on_outline: None,
                        },
                    )
                });
                match mounted {
                    Ok(view) => {
                        handle.set_value(Some(view));
                        active.set(true);
                        // Expose the rich control before restoring focus in this gesture.
                        node.remove_attribute("hidden").ok();
                        handle.with_value(|view| view.as_ref().unwrap().focus());
                    }
                    Err(_) => failed.set(true),
                }
            });
        };
        let command = move |kind: &'static str| {
            handle.with_value(|view| {
                if let Some(view) = view {
                    view.apply_inline_format(kind);
                }
            });
        };
        (activate, command)
    };
    #[cfg(not(feature = "browser"))]
    let (activate, command) = {
        let _ = (controller, composer, field_id);
        (|| {}, |_: &'static str| {})
    };
    view! {
        <div class="vk-compact-field">
            <div hidden=move || active.get()><TextArea id label described_by=description value on_input=native_input
                mode variant=FieldVariant::FloatingTonal rows=2 help="" /></div>
            <span id=label_id class="vk-ui-visually-hidden">{label}</span>
            <div id=rich_id node_ref=host class="vk-comment-editor" hidden=move || !active.get()></div>
            <div class="vk-comment-format" hidden=move || mode.get() != FieldMode::Editable>
                <button id=activate_id type="button" hidden=move || active.get() || edited.get() on:click=move |_| activate()
                    aria-busy=move || loading.get().to_string()>"Định dạng"</button>
                <div role="group" aria-label="Định dạng nhận xét" hidden=move || !active.get()>
                    {[("bold","Đậm","B"),("italic","Nghiêng","I"),("link","Liên kết","Liên kết"),("quote","Trích dẫn","❝"),("list","Danh sách","•")].into_iter().map(|(kind,name,text)| view! {
                        <button type="button" aria-label=name title=name data-format-action=kind
                            on:mousedown=|event| event.prevent_default() on:click=move |_| command(kind)>{text}</button>
                    }).collect_view()}
                </div>
            </div>
            <Show when=move || failed.get()><p role="status">"Chưa tải được định dạng. Bạn vẫn có thể viết và gửi."</p></Show>
        </div>
    }
}
