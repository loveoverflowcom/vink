//! Reader owns the modal and its return target; the shell still owns browser navigation,
//! and the tab-wide AuthGate/SessionOwner own all authentication and reconciliation.
//! One dialog serves the composers (#328) and “Lưu” (#388, D2); the caller names its purpose,
//! its return target and when its own private state has applied the confirmed session.
use std::sync::Arc;

use leptos::{html, prelude::*};
use vink_ui::{CloseReason, IconButton, IconName};

use crate::{
    account::{AuthEntry, ReaderPresentation, Session, standing_id},
    discussion_state::DiscussionController,
    navigation::use_navigation_guard,
    session::Standing,
};

const DIALOG: &str = "reader-auth";
const CLOSE: &str = "reader-auth-close";

/// Why the guest is asked to sign in from the Reader. Copy only: no write is implied.
#[cfg_attr(not(feature = "browser"), allow(dead_code))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReaderAuthPurpose {
    #[default]
    Comment,
    Save,
}

impl ReaderAuthPurpose {
    pub(crate) const fn presentation(self) -> ReaderPresentation {
        match self {
            Self::Comment => ReaderPresentation {
                title: "Đăng nhập để nhận xét",
                lead: None,
            },
            Self::Save => ReaderPresentation {
                title: "Đăng nhập để lưu bài",
                lead: Some("Bài sẽ nằm trong danh sách Đã lưu riêng trên hồ sơ của bạn."),
            },
        }
    }
}

/// Whether the caller's private owner already applied the confirmed session scope. A plain
/// closure kept by the dialog: the button that opened it may re-render (and dispose its own
/// reactive owner) before the session is confirmed.
pub(crate) type ReaderAuthReady = Arc<dyn Fn(&str) -> bool + Send + Sync>;
/// Told whether the dialog closed after a confirmed sign-in.
pub(crate) type ReaderAuthSettled = Arc<dyn Fn(bool) + Send + Sync>;

#[cfg_attr(not(feature = "browser"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(crate) struct ReaderAuth {
    dialog: NodeRef<html::Dialog>,
    origin: RwSignal<Option<String>>,
    purpose: RwSignal<ReaderAuthPurpose>,
    ready: StoredValue<Option<ReaderAuthReady>>,
    settled: StoredValue<Option<ReaderAuthSettled>>,
    cycle: StoredValue<u64>,
    scroll: StoredValue<(f64, f64)>,
    editor_top: StoredValue<Option<f64>>,
}

impl ReaderAuth {
    pub(crate) fn provide() -> Self {
        let this = Self {
            dialog: NodeRef::new(),
            origin: RwSignal::new(None),
            purpose: RwSignal::new(ReaderAuthPurpose::Comment),
            ready: StoredValue::new(None),
            settled: StoredValue::new(None),
            cycle: StoredValue::new(0),
            scroll: StoredValue::new((0.0, 0.0)),
            editor_top: StoredValue::new(None),
        };
        provide_context(this);
        this
    }

    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    pub(crate) fn current() -> Option<Self> {
        use_context::<Self>()
    }

    /// Reanchor the composer once the new session's own section has settled above it.
    pub(super) fn open_for_comment(
        self,
        composer: String,
        controller: RwSignal<DiscussionController>,
        state: RwSignal<super::ViewState>,
    ) {
        let ready: ReaderAuthReady = Arc::new(move |scope: &str| {
            controller.try_with(|c| c.scope.session.as_deref() == Some(scope)) == Some(true)
                && state.try_with(|s| {
                    s.own.scope.session.as_deref() == Some(scope)
                        && (s.unavailable
                            || (!s.own.loading
                                && !s.own.capability_loading
                                && (s.own.has_loaded_page() || s.own.error.is_some())))
                }) == Some(true)
        });
        self.open(composer, ReaderAuthPurpose::Comment, ready, None);
    }

    pub(crate) fn open(
        self,
        composer: String,
        purpose: ReaderAuthPurpose,
        ready: ReaderAuthReady,
        settled: Option<ReaderAuthSettled>,
    ) {
        #[cfg(feature = "browser")]
        if self.origin.get_untracked().is_none()
            && let Some(dialog) = self.dialog.get_untracked()
        {
            self.ready.set_value(Some(ready));
            self.settled.set_value(settled);
            self.purpose.set(purpose);
            self.scroll.set_value((
                window().scroll_x().unwrap_or_default(),
                window().scroll_y().unwrap_or_default(),
            ));
            self.editor_top.set_value(
                document()
                    .get_element_by_id(&composer)
                    .map(|editor| editor.get_bounding_client_rect().top()),
            );
            if dialog.show_modal().is_ok() {
                self.cycle.update_value(|cycle| *cycle += 1);
                self.origin.set(Some(composer));
                dialog.set_scroll_top(0);
                focus(CLOSE);
                restore_scroll(self.scroll.get_value());
            }
        }
        #[cfg(not(feature = "browser"))]
        let _ = (self, composer, purpose, ready, settled);
    }

    #[cfg(feature = "browser")]
    pub(super) fn is_open(self) -> bool {
        self.origin.get_untracked().is_some()
    }

    /// Dismissals release UI ownership only. A request that may have changed the cookie
    /// still runs through the shared lane and reconciles without reopening this modal.
    fn close(self, reason: CloseReason, authenticated: bool) -> bool {
        let Some(Some(composer)) = self.origin.try_update(Option::take) else {
            return false;
        };
        if let Some(dialog) = self.dialog.get_untracked() {
            dialog.close();
        }
        self.ready.set_value(None);
        if let Some(settled) = self.settled.try_update_value(Option::take).flatten() {
            settled(authenticated);
        }
        #[cfg(feature = "browser")]
        if reason != CloseReason::Navigate {
            let cycle = self.cycle.get_value();
            let scroll = self.scroll.get_value();
            let editor_top = self.editor_top.get_value();
            let restore = move || {
                if self.cycle.try_get_value() != Some(cycle)
                    || self.origin.try_get_untracked() != Some(None)
                {
                    return;
                }
                let focused = if authenticated {
                    focus(&composer)
                } else {
                    focus(&format!("{composer}-auth")) || focus(&composer)
                };
                if !focused {
                    focus("feedback-heading");
                }
                // The newly visible private comments can sit above a public reply.
                // Keeping scrollY unchanged in that case would move the focused editor
                // out of view. Anchor the same editor to its original viewport position.
                let position = if authenticated
                    && let Some(original_top) = editor_top
                    && let Some(editor) = document().get_element_by_id(&composer)
                {
                    (
                        scroll.0,
                        window().scroll_y().unwrap_or_default()
                            + editor.get_bounding_client_rect().top()
                            - original_top,
                    )
                } else {
                    scroll
                };
                restore_scroll(position);
            };
            if reason == CloseReason::Back {
                // The guard and browser can both adjust scroll on history traversal.
                // Queue our frame after their popstate work, but before the next paint.
                leptos::task::spawn_local(async move { request_animation_frame(restore) });
            } else {
                request_animation_frame(restore);
            }
        }
        #[cfg(not(feature = "browser"))]
        let _ = (composer, reason, authenticated);
        true
    }
}

#[component]
pub(crate) fn ReaderAuthDialog(host: ReaderAuth) -> impl IntoView {
    let session = Session::current();
    let navigation = use_navigation_guard();
    let overlay = navigation.set_overlay(move |reason| host.close(reason, false));
    on_cleanup(move || navigation.clear_overlay(overlay));

    #[cfg(feature = "browser")]
    {
        // CSS prevents user scrolling of the inert page. Native focus/scrollIntoView
        // can still advance the viewport when an Auth field or error changes; only
        // the dialog itself gets to do that while it is open.
        let scroll = window_event_listener(leptos::ev::scroll, move |_| {
            if host.is_open() {
                let at = host.scroll.get_value();
                if (window().scroll_y().unwrap_or_default() - at.1).abs() > 1.0
                    || (window().scroll_x().unwrap_or_default() - at.0).abs() > 1.0
                {
                    restore_scroll(at);
                }
            }
        });
        on_cleanup(move || scroll.remove());
    }

    #[cfg(feature = "browser")]
    Effect::new(move |_| {
        // Only return after the session owner has confirmed an active account AND the
        // caller's owner has applied that scope (a composer may discard another owner's draft;
        // “Lưu” reads the new account's own state). The callback's reads are tracked here.
        if host.origin.with(Option::is_some)
            && !session.checking()
            && let Some(scope) = session.profile_scope.get()
            && host
                .ready
                .with_value(Clone::clone)
                .is_none_or(|ready| ready(&scope))
        {
            host.close(CloseReason::Dismiss, true);
        }
    });
    let title = move || host.purpose.get().presentation().title;
    view! {
        <dialog id=DIALOG
            class="vk-reader-auth box-border [max-inline-size:none] [border:0] overscroll-contain overflow-auto"
            aria-label=title aria-labelledby="reader-auth-title"
            aria-describedby=move || host.purpose.get().presentation().lead.map(|_| "reader-auth-lead")
            node_ref=host.dialog
            data-standing=move || session.standing.with(standing_id)
            on:cancel=move |event: leptos::ev::Event| { event.prevent_default(); host.close(CloseReason::Escape, false); }
            on:keydown=move |event| trap_tab(event, host.dialog)>
            <header class="vk-reader-auth-bar flex justify-between items-center gap-vk-3 [margin-block-end:var(--vk-space-3)]">
                <span class="vk-brand__lockup" role="img" aria-label="Vink"></span>
                <IconButton icon=IconName::Close label="Đóng" id=CLOSE
                    on_press=move |_| { host.close(CloseReason::Dismiss, false); } />
            </header>
            <Show when=move || host.origin.with(Option::is_some)>
                <AuthEntry reader=host.purpose.get_untracked().presentation() />
                {move || match session.standing.get() {
                    Standing::Anonymous | Standing::NeedsPrincipal => None,
                    standing => Some(view! {
                        <h2 id="reader-auth-title" class="vk-reader-auth-title m-0 wrap-anywhere">{title()}</h2>
                        <p role="status">{if standing == Standing::Suspended {
                            "Tài khoản đang bị tạm khóa trên Vink."
                        } else {
                            "Đang đối chiếu phiên đăng nhập."
                        }}</p>
                    }),
                }}
            </Show>
        </dialog>
    }
}

#[cfg(feature = "browser")]
fn focus(id: &str) -> bool {
    use wasm_bindgen::JsCast;
    let Some(target) = document()
        .get_element_by_id(id)
        .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        .filter(|node| node.is_connected() && node.get_bounding_client_rect().width() > 0.0)
    else {
        return false;
    };
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    target.focus_with_options(&options).is_ok()
}

#[cfg(feature = "browser")]
fn restore_scroll((x, y): (f64, f64)) {
    window().scroll_to_with_x_and_y(x, y);
}

fn trap_tab(event: leptos::ev::KeyboardEvent, node: NodeRef<html::Dialog>) {
    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::JsCast;
        if event.key() != "Tab" || event.is_composing() {
            return;
        }
        let Some(dialog) = node.get_untracked() else {
            return;
        };
        // A native dialog makes the Reader inert. Explicit edge cycling keeps Tab in the
        // dialog (rather than landing in browser chrome) in both directions.
        let Ok(stops) =
            dialog.query_selector_all("button:not(:disabled), input:not(:disabled), a[href]")
        else {
            return;
        };
        let mut stops = (0..stops.length())
            .filter_map(|i| stops.item(i)?.dyn_into::<web_sys::HtmlElement>().ok())
            .filter(|node| node.tab_index() >= 0 && node.get_bounding_client_rect().width() > 0.0);
        let (Some(first), Some(active)) = (stops.next(), document().active_element()) else {
            return;
        };
        let last = stops.next_back().unwrap_or_else(|| first.clone());
        let target = if event.shift_key() && first.is_same_node(Some(&active)) {
            last
        } else if !event.shift_key() && last.is_same_node(Some(&active)) {
            first
        } else {
            return;
        };
        event.prevent_default();
        let _ = target.focus();
    }
    #[cfg(not(feature = "browser"))]
    let _ = (event, node);
}
