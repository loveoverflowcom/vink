//! Inputs and choices. Values are controlled by the caller and passed back raw: no trimming,
//! normalisation or parsing, so an empty string and "0" survive. Labels are real `<label>`s or
//! legends (never only a placeholder); help and error text are linked by `aria-describedby`.
//! Validation and saving belong to the caller.

use leptos::prelude::*;

use crate::action::IconButton;
use crate::icons::IconName;
use crate::media::{Icon, IconSize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FieldMode {
    #[default]
    Editable,
    /// Focusable, selectable and copyable; not editable.
    ReadOnly,
    /// Not focusable; the value is shown but not submitted by a native form.
    Disabled,
}

/// Short-field presentation; both variants keep one stable native input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FieldVariant {
    #[default]
    ExternalLabel,
    FloatingTonal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextKind {
    #[default]
    Text,
    Email,
    Url,
    Tel,
    Search,
}

impl TextKind {
    const fn value(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Email => "email",
            Self::Url => "url",
            Self::Tel => "tel",
            Self::Search => "search",
        }
    }
}

/// One edit as typed. `composing` is true while an IME composition is open; a final event with
/// `composing: false` follows `compositionend`. Validate or save only non-composing values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextInput {
    pub value: String,
    pub composing: bool,
}

/// A selectable option for RadioGroup and Select.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChoiceOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl ChoiceOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }
}

/// Blank error text is not an error.
fn present(error: Option<String>) -> Option<String> {
    error.filter(|e| !e.trim().is_empty())
}

/// `{id}-help {id}-error`, only for the messages that exist.
fn described_by(id: &str, help: bool, error: bool) -> Option<String> {
    let ids: Vec<String> = [(help, "help"), (error, "error")]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, part)| format!("{id}-{part}"))
        .collect();
    (!ids.is_empty()).then(|| ids.join(" "))
}

fn required_mark(required: bool) -> impl IntoView {
    required.then(|| view! { <span class="vk-ui-field__required">" (bắt buộc)"</span> })
}

fn messages(id: String, help: Option<String>, error: Signal<Option<String>>) -> impl IntoView {
    let help_id = format!("{id}-help");
    let error_id = format!("{id}-error");
    view! {
        {help.map(|help| view! { <p class="vk-ui-field__help" id=help_id>{help}</p> })}
        {move || present(error.get()).map(|error| view! {
            <p class="vk-ui-field__error" id=error_id.clone()>
                <Icon name=IconName::Error size=IconSize::Small />
                <span>{error}</span>
            </p>
        })}
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextControl {
    Field(TextKind),
    Area(u16),
    Search,
}

struct TextProps {
    id: String,
    value: Signal<String>,
    on_input: Callback<TextInput>,
    on_submit: Option<Callback<String>>,
    mode: FieldMode,
    invalid: Signal<bool>,
    required: bool,
    described: Signal<Option<String>>,
    placeholder: Option<String>,
    autocomplete: Option<String>,
    name: Option<String>,
    max_length: Option<u32>,
    on_focus: Option<Callback<()>>,
}

/// The native text control. The DOM value is written only when the caller's value changes (a
/// same-value write is a no-op in browsers), so caret, selection and composition are kept.
fn text_control(control: TextControl, p: TextProps, node: NodeRef<leptos::html::Input>) -> AnyView {
    let composing = StoredValue::new(false);
    let on_input = p.on_input;
    let emit = move |ev: &leptos::ev::Event, composing: bool| {
        on_input.run(TextInput {
            value: event_target_value(ev),
            composing,
        });
    };
    let initial = p.value.get_untracked();
    let value = p.value;
    let invalid = move || p.invalid.get().then_some("true");
    let described = p.described;
    let required = p.required.then_some("true");
    let readonly = p.mode == FieldMode::ReadOnly;
    let disabled = p.mode == FieldMode::Disabled;
    let on_submit = p.on_submit;
    match control {
        TextControl::Area(rows) => view! {
            <textarea
                id=p.id
                class="vk-ui-input vk-ui-input--area"
                rows=rows
                prop:value=move || value.get()
                readonly=readonly
                disabled=disabled
                aria-invalid=invalid
                aria-required=required
                aria-describedby=move || described.get()
                placeholder=p.placeholder
                autocomplete=p.autocomplete
                on:compositionstart=move |_| composing.set_value(true)
                on:compositionend=move |ev| {
                    composing.set_value(false);
                    emit(&ev, false);
                }
                on:input=move |ev| emit(&ev, composing.get_value())
            >
                {initial}
            </textarea>
        }
        .into_any(),
        TextControl::Field(_) | TextControl::Search => {
            let kind = match control {
                TextControl::Field(kind) => kind.value(),
                _ => "search",
            };
            view! {
                <input
                    node_ref=node
                    id=p.id
                    class="vk-ui-input"
                    type=kind
                    name=p.name
                    maxlength=p.max_length
                    on:focus=move |_| { if let Some(focus) = p.on_focus { focus.run(()); } }
                    value=initial
                    prop:value=move || value.get()
                    readonly=readonly
                    disabled=disabled
                    aria-invalid=invalid
                    aria-required=required
                    aria-describedby=move || described.get()
                    placeholder=p.placeholder
                    autocomplete=p.autocomplete
                    on:compositionstart=move |_| composing.set_value(true)
                    on:compositionend=move |ev| {
                        composing.set_value(false);
                        emit(&ev, false);
                    }
                    on:input=move |ev| emit(&ev, composing.get_value())
                    on:keydown=move |ev| {
                        // 229: Safari's keydown for the key that ends a composition.
                        let ime = composing.get_value() || ev.is_composing() || ev.key_code() == 229;
                        if let (Some(submit), false, "Enter") = (on_submit, ime, ev.key().as_str()) {
                            ev.prevent_default();
                            submit.run(event_target_value(&ev));
                        }
                    }
                />
            }
            .into_any()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn text_field(
    control: TextControl,
    id: String,
    label: String,
    value: Signal<String>,
    on_input: Callback<TextInput>,
    on_submit: Option<Callback<String>>,
    help: Option<String>,
    error: Signal<Option<String>>,
    required: bool,
    mode: FieldMode,
    placeholder: Option<String>,
    autocomplete: Option<String>,
    name: Option<String>,
    max_length: Option<u32>,
    input_ref: Option<NodeRef<leptos::html::Input>>,
    on_focus: Option<Callback<()>>,
    variant: FieldVariant,
) -> impl IntoView {
    let invalid = Signal::derive(move || present(error.get()).is_some());
    let help =
        help.or_else(|| (mode == FieldMode::Disabled).then(|| "Tính năng chưa khả dụng.".into()));
    let has_help = help.is_some();
    let described = {
        let id = id.clone();
        Signal::derive(move || described_by(&id, has_help, invalid.get()))
    };
    let floating = variant == FieldVariant::FloatingTonal;
    let node = input_ref.unwrap_or_default();
    let props = TextProps {
        id: id.clone(),
        value,
        on_input,
        on_submit,
        mode,
        invalid,
        required,
        described,
        placeholder: if floating {
            Some(placeholder.unwrap_or_else(|| " ".into()))
        } else {
            placeholder
        },
        autocomplete,
        name,
        max_length,
        on_focus,
    };
    let search = control == TextControl::Search;
    let clear = move || {
        (search && !value.get().is_empty() && mode == FieldMode::Editable).then(|| {
            view! {
                <IconButton
                    icon=IconName::Close
                    label="Xóa chữ tìm"
                    on_press=move |_| {
                        on_input.run(TextInput { value: String::new(), composing: false });
                        if let Some(input) = node.get_untracked() {
                            let _ = input.focus();
                        }
                    }
                />
            }
        })
    };
    view! {
        <div
            class=if floating { "vk-ui-field vk-ui-field--floating" } else { "vk-ui-field" }
            data-mode=match mode {
                FieldMode::Editable => "editable",
                FieldMode::ReadOnly => "read-only",
                FieldMode::Disabled => "disabled",
            }
            data-invalid=move || invalid.get().then_some("true")
        >
            {(!floating).then(|| view! {
                <label class="vk-ui-field__label" for=id.clone() id=format!("{id}-label")>
                    {label.clone()}
                    {required_mark(required)}
                </label>
            })}
            <div class="vk-ui-field__control" data-search=search.then_some("true")>
                {search.then(|| view! { <Icon name=IconName::Search /> })}
                {text_control(control, props, node)}
                {floating.then(|| view! {
                    <label class="vk-ui-field__label" for=id.clone() id=format!("{id}-label")>
                        {label}
                        {required_mark(required)}
                    </label>
                })}
                {clear}
            </div>
            {messages(id, help, error)}
        </div>
    }
}

#[component]
pub fn TextField(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<TextInput>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional, into)] error: Signal<Option<String>>,
    #[prop(optional)] required: bool,
    #[prop(optional)] mode: FieldMode,
    #[prop(optional)] kind: TextKind,
    #[prop(optional)] variant: FieldVariant,
    #[prop(optional, into)] placeholder: Option<String>,
    #[prop(optional, into)] autocomplete: Option<String>,
    #[prop(optional, into)] name: Option<String>,
    #[prop(optional)] max_length: Option<u32>,
    #[prop(optional)] input_ref: Option<NodeRef<leptos::html::Input>>,
    #[prop(optional, into)] on_focus: Option<Callback<()>>,
) -> impl IntoView {
    text_field(
        TextControl::Field(kind),
        id,
        label,
        value,
        on_input,
        None,
        help,
        error,
        required,
        mode,
        placeholder,
        autocomplete,
        name,
        max_length,
        input_ref,
        on_focus,
        variant,
    )
}

/// Multi-line text. Newlines, blank lines (stanza gaps) and leading spaces are kept verbatim.
#[component]
pub fn TextArea(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<TextInput>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional, into)] error: Signal<Option<String>>,
    #[prop(optional)] required: bool,
    #[prop(optional)] mode: FieldMode,
    #[prop(default = 4)] rows: u16,
    #[prop(optional, into)] placeholder: Option<String>,
) -> impl IntoView {
    text_field(
        TextControl::Area(rows),
        id,
        label,
        value,
        on_input,
        None,
        help,
        error,
        required,
        mode,
        placeholder,
        None,
        None,
        None,
        None,
        None,
        FieldVariant::ExternalLabel,
    )
}

/// A search input with a clear button while it has text. Enter calls `on_submit` unless an IME
/// composition is open. Searching and debouncing are the caller's.
#[component]
pub fn SearchField(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<TextInput>,
    #[prop(optional, into)] on_submit: Option<Callback<String>>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional)] mode: FieldMode,
    #[prop(optional, into)] placeholder: Option<String>,
) -> impl IntoView {
    text_field(
        TextControl::Search,
        id,
        label,
        value,
        on_input,
        on_submit,
        help,
        Signal::default(),
        false,
        mode,
        placeholder,
        None,
        None,
        None,
        None,
        None,
        FieldVariant::ExternalLabel,
    )
}

/// After a change, put the DOM back to the caller's value: a caller that rejects the change
/// keeps the control truthful. A same-value write is a no-op.
fn resync_checked(ev: &leptos::ev::Event, checked: bool) {
    use leptos::wasm_bindgen::JsCast;
    if let Some(input) = ev
        .target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::HtmlInputElement>().ok())
    {
        input.set_checked(checked);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Toggle {
    Checkbox,
    Switch,
}

#[allow(clippy::too_many_arguments)]
fn toggle(
    kind: Toggle,
    id: String,
    label: String,
    checked: Signal<bool>,
    on_change: Callback<bool>,
    help: Option<String>,
    error: Signal<Option<String>>,
    disabled: Signal<bool>,
) -> impl IntoView {
    let invalid = Signal::derive(move || present(error.get()).is_some());
    let has_help = help.is_some();
    let described = {
        let id = id.clone();
        move || described_by(&id, has_help, invalid.get())
    };
    let class = match kind {
        Toggle::Checkbox => "vk-ui-choice",
        Toggle::Switch => "vk-ui-choice vk-ui-choice--switch",
    };
    view! {
        <div class=class data-invalid=move || invalid.get().then_some("true")>
            <label class="vk-ui-choice__target">
                <input
                    type="checkbox"
                    id=id.clone()
                    class="vk-ui-choice__input"
                    role=(kind == Toggle::Switch).then_some("switch")
                    checked=checked.get_untracked()
                    prop:checked=move || checked.get()
                    disabled=move || disabled.get()
                    aria-invalid=move || invalid.get().then_some("true")
                    aria-describedby=described
                    on:change=move |ev| {
                        on_change.run(event_target_checked(&ev));
                        resync_checked(&ev, checked.get_untracked());
                    }
                />
                <span class="vk-ui-choice__label">{label}</span>
                {(kind == Toggle::Switch).then(|| view! {
                    <span class="vk-ui-choice__state" aria-hidden="true">
                        {move || if checked.get() { "Bật" } else { "Tắt" }}
                    </span>
                })}
            </label>
            {messages(id, help, error)}
        </div>
    }
}

/// A checkbox whose box and label are one target with one handler.
#[component]
pub fn Checkbox(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] on_change: Callback<bool>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional, into)] error: Signal<Option<String>>,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    toggle(
        Toggle::Checkbox,
        id,
        label,
        checked,
        on_change,
        help,
        error,
        disabled,
    )
}

/// An on/off switch (`role="switch"`) with a visible Bật/Tắt state.
#[component]
pub fn Switch(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] on_change: Callback<bool>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    toggle(
        Toggle::Switch,
        id,
        label,
        checked,
        on_change,
        help,
        Signal::default(),
        disabled,
    )
}

/// One of several options, as a native radio group in a fieldset.
#[component]
pub fn RadioGroup(
    #[prop(into)] id: String,
    #[prop(into)] legend: String,
    options: Vec<ChoiceOption>,
    #[prop(into)] value: Signal<Option<String>>,
    #[prop(into)] on_change: Callback<String>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional, into)] error: Signal<Option<String>>,
    #[prop(optional)] required: bool,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    let invalid = Signal::derive(move || present(error.get()).is_some());
    let has_help = help.is_some();
    let described = {
        let id = id.clone();
        move || described_by(&id, has_help, invalid.get())
    };
    let name = id.clone();
    let group_id = id.clone();
    view! {
        <fieldset
            class="vk-ui-group"
            id=group_id
            role="radiogroup"
            aria-required=required.then_some("true")
            aria-invalid=move || invalid.get().then_some("true")
            aria-describedby=described
            data-invalid=move || invalid.get().then_some("true")
        >
            <legend class="vk-ui-field__label">{legend}{required_mark(required)}</legend>
            {options
                .into_iter()
                .map(|option| {
                    let current = option.value.clone();
                    let picked = option.value.clone();
                    let off = option.disabled;
                    let selected = move || value.with(|v| v.as_deref() == Some(current.as_str()));
                    let initially = selected();
                    view! {
                        <label class="vk-ui-choice__target">
                            <input
                                type="radio"
                                class="vk-ui-choice__input"
                                name=name.clone()
                                value=option.value
                                checked=initially
                                prop:checked=selected.clone()
                                disabled=move || off || disabled.get()
                                on:change=move |ev| {
                                    on_change.run(picked.clone());
                                    resync_checked(&ev, selected());
                                }
                            />
                            <span class="vk-ui-choice__label">{option.label}</span>
                        </label>
                    }
                })
                .collect_view()}
            {messages(id, help, error)}
        </fieldset>
    }
}

/// A native `<select>`. The empty placeholder option means "no value" (`None`); it cannot be
/// chosen again once a required field has a value.
#[component]
pub fn Select(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    options: Vec<ChoiceOption>,
    #[prop(into)] value: Signal<Option<String>>,
    #[prop(into)] on_change: Callback<Option<String>>,
    #[prop(optional, into)] placeholder: Option<String>,
    #[prop(optional, into)] help: Option<String>,
    #[prop(optional, into)] error: Signal<Option<String>>,
    #[prop(optional)] required: bool,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    let invalid = Signal::derive(move || present(error.get()).is_some());
    let has_help = help.is_some();
    let described = {
        let id = id.clone();
        move || described_by(&id, has_help, invalid.get())
    };
    let initial = value.get_untracked();
    view! {
        <div class="vk-ui-field" data-invalid=move || invalid.get().then_some("true")>
            <label class="vk-ui-field__label" for=id.clone() id=format!("{id}-label")>
                {label}
                {required_mark(required)}
            </label>
            <div class="vk-ui-field__control vk-ui-field__control--select">
                <select
                    id=id.clone()
                    class="vk-ui-input vk-ui-select"
                    prop:value=move || value.get().unwrap_or_default()
                    disabled=move || disabled.get()
                    aria-invalid=move || invalid.get().then_some("true")
                    aria-required=required.then_some("true")
                    aria-describedby=described
                    on:change=move |ev| {
                        let picked = event_target_value(&ev);
                        on_change.run((!picked.is_empty()).then_some(picked));
                    }
                >
                    <option value="" disabled=required selected=initial.is_none()>
                        {placeholder.unwrap_or_else(|| "Chọn…".into())}
                    </option>
                    {options
                        .into_iter()
                        .map(|o| {
                            let selected = initial.as_deref() == Some(o.value.as_str());
                            view! {
                                <option value=o.value disabled=o.disabled selected=selected>
                                    {o.label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
                <Icon name=IconName::ExpandMore />
            </div>
            {messages(id, help, error)}
        </div>
    }
}

/// A caller-built error summary: announced when it appears, each item links to its field.
#[component]
pub fn FormSummary(
    #[prop(into)] title: String,
    /// `(field id, message)` in form order.
    #[prop(into)]
    items: Signal<Vec<(String, String)>>,
) -> impl IntoView {
    move || {
        let items = items.get();
        (!items.is_empty()).then(|| {
            let title = title.clone();
            view! {
                <div class="vk-ui-summary" role="alert">
                    <p class="vk-ui-summary__title">
                        <Icon name=IconName::Error size=IconSize::Small />
                        <span>{title}</span>
                    </p>
                    <ul class="vk-ui-summary__items">
                        {items
                            .into_iter()
                            .map(|(field, message)| {
                                let target = field.clone();
                                view! {
                                    <li>
                                        <a
                                            class="vk-ui-link"
                                            href=format!("#{field}")
                                            on:click=move |ev| focus_field(&ev, &target)
                                        >
                                            {message}
                                        </a>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                </div>
            }
        })
    }
}

/// Fragment links scroll but do not focus inputs; move focus to the field itself.
fn focus_field(ev: &leptos::ev::MouseEvent, id: &str) {
    use leptos::wasm_bindgen::JsCast;
    if let Some(field) = document()
        .get_element_by_id(id)
        .and_then(|e| e.dyn_into::<leptos::web_sys::HtmlElement>().ok())
    {
        ev.prevent_default();
        let _ = field.focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;

    #[test]
    fn text_fields_link_label_help_and_error_without_touching_the_value() {
        let html = render(|| {
            view! {
                <TextField
                    id="title"
                    label="Tiêu đề"
                    value=Signal::stored("  0 ".to_string())
                    on_input=|_| {}
                    help="Có thể sửa sau."
                    error=Signal::stored(Some("Cần tiêu đề".to_string()))
                    required=true
                />
            }
            .into_any()
        });
        for needle in [
            "for=\"title\"",
            "id=\"title\"",
            "value=\"  0 \"",
            "(bắt buộc)",
            "aria-required=\"true\"",
            "aria-invalid=\"true\"",
            "aria-describedby=\"title-help title-error\"",
            "id=\"title-help\"",
            "id=\"title-error\"",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        assert!(
            !html.contains(" required"),
            "no native validation UI: {html}"
        );
    }

    #[test]
    fn blank_errors_are_not_invalid_and_modes_differ() {
        let html = render(|| {
            view! {
                <TextField
                    id="a"
                    label="A"
                    value=Signal::stored(String::new())
                    on_input=|_| {}
                    error=Signal::stored(Some("   ".to_string()))
                    mode=FieldMode::ReadOnly
                />
            }
            .into_any()
        });
        assert!(
            !html.contains("aria-invalid") && !html.contains("a-error"),
            "{html}"
        );
        assert!(
            html.contains("readonly") && !html.contains("disabled"),
            "{html}"
        );
        let html = render(|| {
            view! {
                <TextArea
                    id="b"
                    label="B"
                    value=Signal::stored("dòng 1\n\n  dòng 3".to_string())
                    on_input=|_| {}
                    mode=FieldMode::Disabled
                />
            }
            .into_any()
        });
        assert!(html.contains("dòng 1\n\n  dòng 3"), "verbatim: {html}");
        assert!(
            html.contains("disabled") && !html.contains("readonly"),
            "{html}"
        );
    }

    #[test]
    fn choices_are_single_targets_and_selects_are_native() {
        let html = render(|| {
            view! {
                <Checkbox id="c" label="Đồng ý" checked=Signal::stored(true) on_change=|_| {} />
            }
            .into_any()
        });
        assert_eq!(html.matches("<label").count(), 1, "{html}");
        assert!(
            html.contains("checked") && html.contains("type=\"checkbox\""),
            "{html}"
        );
        let html = render(|| {
            view! { <Switch id="s" label="Thông báo" checked=Signal::stored(false) on_change=|_| {} /> }
                .into_any()
        });
        assert!(
            html.contains("role=\"switch\"") && html.contains("Tắt"),
            "{html}"
        );
        let html = render(|| {
            view! {
                <RadioGroup
                    id="r"
                    legend="Thể loại"
                    options=vec![ChoiceOption::new("tho", "Thơ"), ChoiceOption::new("van", "Văn xuôi")]
                    value=Signal::stored(Some("van".to_string()))
                    on_change=|_| {}
                    required=true
                />
            }
            .into_any()
        });
        assert!(
            html.contains("<fieldset") && html.contains("<legend"),
            "{html}"
        );
        assert_eq!(html.matches("name=\"r\"").count(), 2, "{html}");
        let html = render(|| {
            view! {
                <Select
                    id="g"
                    label="Thể loại"
                    options=vec![ChoiceOption::new("tho", "Thơ")]
                    value=Signal::stored(None::<String>)
                    on_change=|_| {}
                />
            }
            .into_any()
        });
        assert!(html.contains("<select") && html.contains("Chọn…"), "{html}");
    }

    #[test]
    fn floating_fields_keep_native_form_identity_and_disabled_explanation() {
        let html = render(|| {
            view! {
                <TextField id="draft-title" name="title" label="Tên bản thảo" max_length=200
                    variant=FieldVariant::FloatingTonal value=Signal::stored("Mùa mưa".to_string())
                    autocomplete="name" on_input=|_| {} help="Có thể sửa sau."
                    error=Signal::stored(Some("Lỗi nhập".to_string())) />
            }
            .into_any()
        });
        assert_eq!(html.matches("<input").count(), 1, "{html}");
        assert_eq!(html.matches("<label").count(), 1, "{html}");
        for expected in [
            "vk-ui-field--floating",
            "name=\"title\"",
            "maxlength=\"200\"",
            "for=\"draft-title\"",
            "value=\"Mùa mưa\"",
            "autocomplete=\"name\"",
            "aria-describedby=\"draft-title-help draft-title-error\"",
        ] {
            assert!(html.contains(expected), "{expected} in {html}");
        }
        let disabled = render(|| {
            view! {
                <TextField id="disabled" label="Tên" variant=FieldVariant::FloatingTonal
                    value=Signal::stored(String::new()) on_input=|_| {} mode=FieldMode::Disabled />
            }
            .into_any()
        });
        assert!(
            disabled.contains("aria-describedby=\"disabled-help\""),
            "{disabled}"
        );
        assert!(disabled.contains("Tính năng chưa khả dụng."), "{disabled}");
    }

    #[test]
    fn summaries_render_only_with_items() {
        let empty = render(|| {
            view! { <FormSummary title="Cần sửa" items=Signal::stored(Vec::<(String, String)>::new()) /> }
                .into_any()
        });
        assert!(!empty.contains("role=\"alert\""), "{empty}");
        let html = render(|| {
            view! {
                <FormSummary
                    title="Cần sửa 1 mục"
                    items=Signal::stored(vec![("title".to_string(), "Cần tiêu đề".to_string())])
                />
            }
            .into_any()
        });
        assert!(
            html.contains("role=\"alert\"") && html.contains("href=\"#title\""),
            "{html}"
        );
    }
}
