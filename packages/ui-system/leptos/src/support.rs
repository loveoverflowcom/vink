//! Optional supporting information. Plain tooltips never contain controls; native details
//! remains in flow and works without JavaScript. Neither component owns an overlay layer.

use leptos::prelude::*;

use crate::{ActionState, Icon, IconButton, IconName};

/// A hoverable, persistent plain tooltip. The trigger keeps its own accessible name and
/// references `id` with `aria-describedby`. Escape dismisses only this tooltip, without moving
/// focus or closing an enclosing panel. Use [`SupportingDetails`] for interactive content.
#[component]
pub fn PlainTooltip(
    #[prop(into)] id: String,
    #[prop(into)] text: Signal<String>,
    children: Children,
) -> impl IntoView {
    let hovered = RwSignal::new(false);
    let focused = RwSignal::new(false);
    let dismissed = RwSignal::new(false);
    let positioned = RwSignal::new(false);
    let visible = Memo::new(move |_| (hovered.get() || focused.get()) && !dismissed.get());
    let anchor = NodeRef::<leptos::html::Span>::new();
    let tip = NodeRef::<leptos::html::Span>::new();
    let show = move || {
        // A newly unhidden fixed element has its static position until placement. Keep it
        // out of pointer hit-testing in that frame so focus on pointerdown cannot steal the
        // following pointerup (and the action's click).
        positioned.set(false);
        dismissed.set(false);
        #[cfg(target_arch = "wasm32")]
        request_animation_frame(move || {
            if visible.try_get_untracked().unwrap_or(false) {
                place_tooltip(anchor, tip);
                positioned.try_set(true);
            }
        });
    };
    #[cfg(target_arch = "wasm32")]
    {
        // Keyboard focus may scroll its target into view. Keep that tooltip anchored rather
        // than dismissing it immediately after focus; never revive one dismissed by Escape.
        // Pointer-only tooltips still close when the viewport moves.
        let resize = window_event_listener(leptos::ev::resize, move |_| dismissed.set(true));
        let scroll = window_event_listener(leptos::ev::scroll, move |_| {
            if focused.get_untracked() && !dismissed.get_untracked() {
                request_animation_frame(move || {
                    if visible.try_get_untracked().unwrap_or(false) {
                        place_tooltip(anchor, tip);
                    }
                });
            } else {
                dismissed.set(true);
            }
        });
        on_cleanup(move || {
            resize.remove();
            scroll.remove();
        });
    }
    view! {
        <span class="vk-ui-plain-tooltip" node_ref=anchor
            on:mouseenter=move |_| { hovered.set(true); show(); }
            on:mouseleave=move |_| hovered.set(false)
            on:focusin=move |_| { focused.set(true); show(); }
            on:focusout=move |_| focused.set(false)
            on:keydown=move |event| {
                if event.key() == "Escape" && visible.get_untracked() {
                    dismissed.set(true);
                    event.stop_propagation();
                }
            }>
            {children()}
            <span id=id role="tooltip" class="vk-ui-plain-tooltip__text" node_ref=tip
                style:visibility=move || if positioned.get() { "visible" } else { "hidden" }
                hidden=move || !visible.get()>{move || text.get()}</span>
        </span>
    }
}

#[cfg(target_arch = "wasm32")]
fn place_tooltip(anchor: NodeRef<leptos::html::Span>, tip: NodeRef<leptos::html::Span>) {
    if let (Some(anchor), Some(tip)) = (
        anchor.try_get_untracked().flatten(),
        tip.try_get_untracked().flatten(),
    ) {
        let rect = anchor.get_bounding_client_rect();
        let size = tip.get_bounding_client_rect();
        let width = window()
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(320.0);
        let height = window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(600.0);
        let x = (rect.left() + (rect.width() - size.width()) / 2.0)
            .max(8.0)
            .min((width - size.width() - 8.0).max(8.0));
        let above =
            rect.bottom() + size.height() + 8.0 > height && rect.top() >= size.height() + 8.0;
        let y = if above {
            rect.top() - size.height() - 8.0
        } else {
            rect.bottom() + 8.0
        };
        let style = web_sys::HtmlElement::style(&tip);
        let _ = style.set_property("left", &format!("{x}px"));
        let _ = style.set_property("top", &format!("{}px", y.max(8.0)));
        let _ = tip.set_attribute("data-side", if above { "above" } else { "below" });
    }
}

/// Named IconButton with the shared tooltip. A required stable ID makes the SSR relationship
/// deterministic; the text defaults to its independent accessible name.
#[component]
pub fn TooltipIconButton(
    #[prop(into)] id: String,
    icon: IconName,
    #[prop(into)] label: String,
    #[prop(optional, into)] tooltip: Option<String>,
    #[prop(optional, into)] state: Signal<ActionState>,
    #[prop(optional, into)] pressed: Option<Signal<bool>>,
    #[prop(optional, into)] on_press: Option<Callback<()>>,
) -> impl IntoView {
    let tip_id = format!("{id}-tooltip");
    let text = tooltip.unwrap_or_else(|| label.clone());
    let mut button = crate::action::IconButtonProps::builder()
        .id(id)
        .icon(icon)
        .label(label)
        .state(state)
        .described_by(tip_id.clone())
        .build();
    button.pressed = pressed;
    button.on_press = on_press;
    view! {
        <PlainTooltip id=tip_id.clone() text=Signal::stored(text)>
            {IconButton(button)}
        </PlainTooltip>
    }
}

/// In-flow supporting detail with native toggle/focus and no-JS behavior. Escape is deliberately
/// not a close command: it belongs to tooltips/overlays, not native details. Keep essential
/// errors, blockers and unsaved state visible outside this optional disclosure.
#[component]
pub fn SupportingDetails(
    #[prop(into)] id: String,
    #[prop(into)] summary: String,
    #[prop(default = false)] compact: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <details id=id class="vk-ui-supporting-details">
            <summary>
                <Icon name=IconName::Info />
                <span class:vk-ui-visually-hidden=compact>{summary}</span>
            </summary>
            <div class="vk-ui-supporting-details__content">{children()}</div>
        </details>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;

    #[test]
    fn tooltip_action_keeps_its_name_and_native_details_needs_no_script() {
        let html = render(|| {
            view! {
                <TooltipIconButton id="refresh" icon=IconName::Refresh label="Tải lại" />
                <SupportingDetails id="details" summary="Thông tin thêm" compact=true>
                    <p>"Nội dung hỗ trợ"</p>
                </SupportingDetails>
            }
            .into_any()
        });
        for expected in [
            "type=\"button\"",
            "aria-label=\"Tải lại\"",
            "aria-describedby=\"refresh-tooltip\"",
            "id=\"refresh-tooltip\"",
            "role=\"tooltip\"",
            "<details",
            "<summary",
            "Thông tin thêm",
            "Nội dung hỗ trợ",
        ] {
            assert!(html.contains(expected), "{expected} in {html}");
        }
        assert!(
            !html.contains("tabindex"),
            "tooltip creates no extra focus target: {html}"
        );
    }
}
