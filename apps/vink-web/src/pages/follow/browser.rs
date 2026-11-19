//! Hydrated browser controller for `#follow` and paginated `/:author/:relation` views (#294).

use std::sync::atomic::{AtomicU64, Ordering};

use leptos::{prelude::*, task::spawn_local};
use send_wrapper::SendWrapper;
use vink_ui::{ActionState, Button, ButtonVariant};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use super::{LiveListState, RelationshipPersonRow};
use crate::account::Session;
use crate::api::{self, LoadError};
use crate::contract::ApiError;
use crate::contract::follow::{
    AuthorRelationshipsLoad, FollowDirection, OwnRelationshipList, OwnRelationshipsSummary,
    PublicRelationshipList, PublicRelationships, ViewerRelationship,
};
use crate::follow_state::{FollowController, FollowInvalidation, FollowPhase, FollowScope};

const FOLLOW_CHANNEL: &str = "vink-follow-changes";
const SLOW_RECONCILE_DELAY_MS: i32 = 1000;

static NEXT_SCOPE_ID: AtomicU64 = AtomicU64::new(1);

fn next_scope_id() -> u64 {
    NEXT_SCOPE_ID.fetch_add(1, Ordering::Relaxed)
}

fn generate_command_id() -> Option<String> {
    let mut random = [0u8; 16];
    window()
        .crypto()
        .ok()?
        .get_random_values_with_u8_array(&mut random)
        .ok()?;
    Some(crate::endorsement_state::uuid_v4(random))
}

fn focus_element_in_follow() -> bool {
    let Some(active) = document().active_element() else {
        return false;
    };
    let Some(container) = document().get_element_by_id("follow") else {
        return false;
    };
    container.contains(Some(&active))
}

fn announce_follow_changed(invalidation: &FollowInvalidation) {
    let detail = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &detail,
        &JsValue::from_str("handle"),
        &JsValue::from_str(&invalidation.target_handle),
    );
    let _ = js_sys::Reflect::set(
        &detail,
        &JsValue::from_str("following"),
        &JsValue::from_bool(invalidation.following),
    );
    let _ = js_sys::Reflect::set(
        &detail,
        &JsValue::from_str("acceptedOrder"),
        &JsValue::from_f64(invalidation.accepted_order as f64),
    );

    if let Ok(event) = web_sys::Event::new("vink:follow-changed") {
        let _ = js_sys::Reflect::set(&event, &JsValue::from_str("detail"), &detail);
        let _ = window().dispatch_event(&event);
    }
    if let Ok(channel) = web_sys::BroadcastChannel::new(FOLLOW_CHANNEL) {
        // Other tabs only need to revalidate with their own current session.
        let _ = channel.post_message(&JsValue::NULL);
        channel.close();
    }
}

#[component]
pub(super) fn ActiveFollowRegion(
    scope: String,
    handle: String,
    live_override: RwSignal<Option<(String, PublicRelationships)>>,
    own_relationships: RwSignal<Option<(String, OwnRelationshipsSummary)>>,
) -> impl IntoView {
    let session = Session::current();
    let follow_scope = FollowScope {
        session_gen: next_scope_id(),
        target_handle: handle.clone(),
    };
    let controller = RwSignal::new(FollowController::new(follow_scope.clone()));
    let slow_pending = RwSignal::new(false);
    let load_ticket = RwSignal::new(0u64);
    let action_ref = NodeRef::<leptos::html::Button>::new();

    let scope_token = StoredValue::new(scope);
    let typed_scope = StoredValue::new(follow_scope);
    let stored_handle = StoredValue::new(handle);

    let still_live = move || -> bool {
        scope_token.try_get_value().is_some_and(|expected| {
            session.profile_scope.get_untracked().as_deref() == Some(expected.as_str())
        })
    };

    let apply_invalidation = move |invalidation: FollowInvalidation| {
        if let Some(rel) = invalidation.relationships.clone() {
            live_override.set(Some((invalidation.target_handle.clone(), rel)));
        } else {
            let target = invalidation.target_handle.clone();
            let my_scope = typed_scope.get_value();
            let accepted_order = invalidation.accepted_order;
            spawn_local(async move {
                if let Ok(rel) = api::load_author_relationships_summary(&target).await
                    && still_live()
                    && controller.try_with_untracked(|c| {
                        c.scope() == &my_scope && c.highest_accepted_order() == accepted_order
                    }) == Some(true)
                {
                    live_override.set(Some((target, rel)));
                }
            });
        }
        announce_follow_changed(&invalidation);
    };

    let trigger_load = move || {
        let target = stored_handle.get_value();
        let my_scope = typed_scope.get_value();
        let my_ticket = load_ticket.with_untracked(|n| *n) + 1;
        load_ticket.set(my_ticket);
        controller.update(|c| c.retry_load());
        spawn_local(async move {
            let result = api::load_viewer_relationship(&target).await;
            if !still_live() || load_ticket.get_untracked() != my_ticket {
                return;
            }
            let was_focused = focus_element_in_follow();
            let Some(eff) = controller.try_update(|c| c.finish_load(&my_scope, result)) else {
                return;
            };
            if let Some(FollowPhase::SelfProfile(summary)) =
                controller.try_with_untracked(|c| c.phase().clone())
            {
                own_relationships.set(Some((scope_token.get_value(), summary)));
            } else {
                own_relationships.set(None);
            }
            if eff.should_focus_heading && was_focused {
                super::super::focus_recovery("author-profile-heading");
            }
        });
    };

    trigger_load();

    let run_reconcile = move |command_id: String| {
        let target = stored_handle.get_value();
        let my_scope = typed_scope.get_value();
        spawn_local(async move {
            let receipt = Some(api::load_follow_receipt(&command_id).await);
            let viewer = api::load_viewer_relationship(&target).await;
            if !still_live() {
                return;
            }
            let was_focused = focus_element_in_follow();
            let Some(eff) = controller
                .try_update(|c| c.finish_reconcile(&my_scope, &command_id, receipt, viewer))
            else {
                return;
            };
            slow_pending.set(false);
            if let Some(inv) = eff.invalidation {
                apply_invalidation(inv);
            }
            if eff.should_focus_heading && was_focused {
                super::super::focus_recovery("author-profile-heading");
            }
        });
    };

    let on_press_follow = Callback::new(move |()| {
        let Some(command_id) = generate_command_id() else {
            return;
        };
        let Some(req) = controller
            .try_update(|c| c.begin_intent(command_id.clone(), None))
            .flatten()
        else {
            return;
        };
        slow_pending.set(false);
        let my_scope = typed_scope.get_value();
        let target = stored_handle.get_value();
        let cmd_for_timer = command_id.clone();

        let timer_cb = Closure::once_into_js(move || {
            if controller.try_with_untracked(|c| {
                matches!(
                    c.phase(),
                    FollowPhase::Pending { command, .. } if command.command_id == cmd_for_timer
                )
            }) == Some(true)
            {
                slow_pending.set(true);
            }
        });
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(
            timer_cb.unchecked_ref(),
            SLOW_RECONCILE_DELAY_MS,
        );

        spawn_local(async move {
            let result = api::set_follow(&target, &req).await;
            if !still_live() {
                return;
            }
            let was_focused = focus_element_in_follow();
            let Some(eff) =
                controller.try_update(|c| c.finish_write(&my_scope, &command_id, result))
            else {
                return;
            };
            let phase_now = controller.try_with_untracked(|c| c.phase().clone());
            if !matches!(phase_now, Some(FollowPhase::Pending { .. })) {
                slow_pending.set(false);
            }
            if let Some(inv) = eff.invalidation {
                apply_invalidation(inv);
            }
            if eff.should_focus_heading && was_focused {
                super::super::focus_recovery("author-profile-heading");
            }
            if matches!(phase_now, Some(FollowPhase::Reconciling { .. })) {
                run_reconcile(command_id);
            }
        });
    });

    // Keep one persistent `#follow-action` DOM element across Ready/Pending/Reconciling so
    // keyboard focus never drops during state transitions.
    let show_action = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Ready {
                can_follow: true, ..
            }
            | FollowPhase::Pending { .. }
            | FollowPhase::Reconciling { .. } => true,
            _ => false,
        })
    });
    let action_following = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Ready { state, .. } => state.following,
            FollowPhase::Pending { shown, .. } | FollowPhase::Reconciling { shown, .. } => {
                shown.following
            }
            _ => false,
        })
    });
    let action_mutual = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Ready { state, .. } => state.mutual,
            FollowPhase::Pending { shown, .. } | FollowPhase::Reconciling { shown, .. } => {
                shown.mutual
            }
            _ => false,
        })
    });
    let action_variant = Signal::derive(move || {
        if action_following.get() {
            ButtonVariant::Outlined
        } else {
            ButtonVariant::Filled
        }
    });
    let action_state = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Ready {
                can_follow: true, ..
            } => ActionState::Enabled,
            FollowPhase::Pending { .. } => ActionState::pending("Đang cập nhật theo dõi…"),
            FollowPhase::Reconciling { .. } => ActionState::pending("Đang đối chiếu trạng thái…"),
            _ => ActionState::Disabled { reason: None },
        })
    });
    let reconcile_cmd_id = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Pending { command, .. } if slow_pending.get() => {
                Some(command.command_id.clone())
            }
            FollowPhase::Reconciling { command, .. } => Some(command.command_id.clone()),
            _ => None,
        })
    });
    let status_info = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Loading => (false, "Đang kiểm tra trạng thái theo dõi…"),
            FollowPhase::SelfProfile(_) => (false, ""),
            FollowPhase::Unavailable | FollowPhase::Gone => {
                (false, "Hiện không thể theo dõi tác giả này.")
            }
            FollowPhase::Ready {
                can_follow: false, ..
            } => (false, "Hiện không thể theo dõi tác giả này."),
            FollowPhase::Ready {
                notice: Some(n), ..
            } => (n.is_error, n.text),
            FollowPhase::Ready { notice: None, .. } => (false, ""),
            FollowPhase::Pending { .. } => (false, "Đang gửi yêu cầu theo dõi…"),
            FollowPhase::Reconciling { .. } => (
                true,
                "Chưa nhận được phản hồi rõ ràng; đang đối chiếu trạng thái.",
            ),
            FollowPhase::Error => (true, "Chưa kiểm tra được trạng thái theo dõi."),
        })
    });

    view! {
        <div class="vk-follow-controls">
            <Show when=move || show_action.get()>
                <Button
                    id="follow-action"
                    variant=action_variant
                    state=action_state
                    button_ref=action_ref
                    on_press=on_press_follow>
                    {move || if action_following.get() { "Bỏ theo dõi" } else { "Theo dõi" }}
                </Button>
                <Show when=move || action_mutual.get()>
                    <span id="follow-mutual" class="vk-follow-mutual">"Theo dõi lẫn nhau"</span>
                </Show>
            </Show>
            {move || reconcile_cmd_id.get().map(|cmd_id| view! {
                <Button
                    id="follow-reconcile"
                    variant=ButtonVariant::Tonal
                    on_press=Callback::new(move |()| run_reconcile(cmd_id.clone()))>
                    "Đối chiếu trạng thái"
                </Button>
            })}
            <Show when=move || controller.with(|c| matches!(c.phase(), FollowPhase::Error))>
                <Button
                    id="follow-retry"
                    variant=ButtonVariant::Tonal
                    on_press=Callback::new(move |()| trigger_load())>
                    "Thử lại"
                </Button>
            </Show>
            <Show when=move || !status_info.get().1.is_empty()>
                <p
                    id="follow-status"
                    role=move || if status_info.get().0 { "alert" } else { "status" }
                    aria-live="polite">
                    {move || status_info.get().1}
                </p>
            </Show>
        </div>
    }
}

#[component]
pub(super) fn ListSyncHost(
    name: String,
    direction: FollowDirection,
    cursor: Option<String>,
    ssr_data: Resource<(u64, Result<AuthorRelationshipsLoad, LoadError>)>,
    live_state: RwSignal<Option<LiveListState>>,
    append_trigger: RwSignal<u64>,
    reload_trigger: RwSignal<u64>,
) -> impl IntoView {
    let session = Session::current();
    let list_gen = RwSignal::new(0u64);
    let stored_name = StoredValue::new(name);
    let stored_cursor = StoredValue::new(cursor);

    let load_fresh = move |cursor_override: Option<String>| {
        let generation = list_gen.with_untracked(|g| *g) + 1;
        list_gen.set(generation);
        let handle = stored_name.get_value();
        let active_scope = session.profile_scope.get_untracked();
        live_state.set(Some(LiveListState::Loading));

        spawn_local(async move {
            let mut use_own = false;
            if active_scope.is_some()
                && let Ok(ViewerRelationship::SelfProfile { .. }) =
                    api::load_viewer_relationship(&handle).await
            {
                use_own = true;
            }
            if list_gen.try_get_untracked() != Some(generation) {
                return;
            }
            if use_own {
                let result =
                    api::load_own_relationships(direction, cursor_override.as_deref()).await;
                if list_gen.try_get_untracked() != Some(generation)
                    || session.profile_scope.get_untracked() != active_scope
                {
                    return;
                }
                match result {
                    Ok(OwnRelationshipList::Page {
                        author,
                        direction,
                        count,
                        people,
                        next_cursor,
                    }) => {
                        live_state.set(Some(LiveListState::Page {
                            author,
                            direction,
                            is_own: true,
                            count: Some(count),
                            people: people
                                .into_iter()
                                .map(|p| RelationshipPersonRow {
                                    author: p.author,
                                    mutual: p.mutual,
                                })
                                .collect(),
                            next_cursor,
                            appending: false,
                            append_error: false,
                        }));
                    }
                    Ok(OwnRelationshipList::Restart { author, direction }) => {
                        live_state.set(Some(LiveListState::Restart { author, direction }));
                    }
                    Err(
                        ApiError::Unauthenticated
                        | ApiError::Forbidden
                        | ApiError::FollowActorUnavailable,
                    ) => {
                        live_state.set(Some(LiveListState::Loading));
                        load_public_into(
                            &handle,
                            direction,
                            cursor_override.as_deref(),
                            generation,
                            list_gen,
                            live_state,
                        )
                        .await;
                    }
                    Err(ApiError::ProfileGone) => live_state.set(Some(LiveListState::Gone)),
                    Err(ApiError::NotFound) => live_state.set(Some(LiveListState::Missing)),
                    Err(_) => live_state.set(Some(LiveListState::Unavailable)),
                }
            } else {
                load_public_into(
                    &handle,
                    direction,
                    cursor_override.as_deref(),
                    generation,
                    list_gen,
                    live_state,
                )
                .await;
            }
        });
    };

    // React to ANY session change (`A -> B -> A` or logout): immediately clear private rows and reload.
    Effect::new(move |prev_scope: Option<Option<String>>| {
        let current_scope = session.profile_scope.get();
        if let Some(prev) = prev_scope {
            if prev != current_scope {
                live_state.set(Some(LiveListState::Loading));
                load_fresh(stored_cursor.get_value());
            }
        } else if current_scope.is_some() {
            load_fresh(stored_cursor.get_value());
        }
        current_scope
    });

    // React to explicit retries or external invalidations.
    Effect::new(move |prev: Option<u64>| {
        let cur = reload_trigger.get();
        if prev.is_some_and(|p| p != cur) {
            load_fresh(stored_cursor.get_value());
        }
        cur
    });

    // React to pagination Append triggers (`Tải thêm`).
    Effect::new(move |prev: Option<u64>| {
        let cur = append_trigger.get();
        if prev.is_none_or(|p| p == cur) {
            return cur;
        }
        let snapshot = live_state
            .get_untracked()
            .or_else(|| match ssr_data.get_untracked()? {
                (
                    _,
                    Ok(AuthorRelationshipsLoad::Page {
                        author,
                        direction,
                        count,
                        people,
                        next_cursor,
                    }),
                ) => Some(LiveListState::Page {
                    author,
                    direction,
                    is_own: false,
                    count,
                    people: people
                        .into_iter()
                        .map(|author| RelationshipPersonRow {
                            author,
                            mutual: false,
                        })
                        .collect(),
                    next_cursor,
                    appending: false,
                    append_error: false,
                }),
                _ => None,
            });
        let Some(mut page_snapshot) = snapshot else {
            return cur;
        };
        let (is_own, cursor) = match &mut page_snapshot {
            LiveListState::Page {
                is_own,
                next_cursor,
                appending,
                append_error,
                ..
            } if !*appending => {
                let Some(next) = next_cursor.clone() else {
                    return cur;
                };
                *appending = true;
                *append_error = false;
                (*is_own, next)
            }
            _ => return cur,
        };

        let generation = list_gen.get_untracked();
        let handle = stored_name.get_value();
        live_state.set(Some(page_snapshot));

        spawn_local(async move {
            if is_own {
                match api::load_own_relationships(direction, Some(&cursor)).await {
                    Ok(OwnRelationshipList::Page {
                        count,
                        people,
                        next_cursor,
                        ..
                    }) => {
                        if list_gen.try_get_untracked() != Some(generation) {
                            return;
                        }
                        live_state.update(|s| {
                            if let Some(LiveListState::Page {
                                count: c,
                                people: existing,
                                next_cursor: nc,
                                appending,
                                append_error,
                                ..
                            }) = s
                            {
                                *c = Some(count);
                                existing.extend(people.into_iter().map(|p| {
                                    RelationshipPersonRow {
                                        author: p.author,
                                        mutual: p.mutual,
                                    }
                                }));
                                *nc = next_cursor;
                                *appending = false;
                                *append_error = false;
                            }
                        });
                    }
                    Ok(OwnRelationshipList::Restart { .. }) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            load_fresh(None);
                        }
                    }
                    Err(ApiError::RelationshipListPrivate) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.set(Some(LiveListState::Private));
                        }
                    }
                    Err(ApiError::ProfileGone) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.set(Some(LiveListState::Gone));
                        }
                    }
                    Err(ApiError::NotFound) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.set(Some(LiveListState::Missing));
                        }
                    }
                    Err(
                        ApiError::Unauthenticated
                        | ApiError::Forbidden
                        | ApiError::FollowActorUnavailable,
                    ) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            load_fresh(None);
                        }
                    }
                    Err(_) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.update(|s| {
                                if let Some(LiveListState::Page {
                                    appending,
                                    append_error,
                                    ..
                                }) = s
                                {
                                    *appending = false;
                                    *append_error = true;
                                }
                            });
                        }
                    }
                }
            } else {
                match api::load_public_relationships(&handle, direction, Some(&cursor)).await {
                    Ok(PublicRelationshipList::Page {
                        count,
                        people,
                        next_cursor,
                        ..
                    }) => {
                        if list_gen.try_get_untracked() != Some(generation) {
                            return;
                        }
                        live_state.update(|s| {
                            if let Some(LiveListState::Page {
                                count: c,
                                people: existing,
                                next_cursor: nc,
                                appending,
                                append_error,
                                ..
                            }) = s
                            {
                                *c = count;
                                existing.extend(people.into_iter().map(|author| {
                                    RelationshipPersonRow {
                                        author,
                                        mutual: false,
                                    }
                                }));
                                *nc = next_cursor;
                                *appending = false;
                                *append_error = false;
                            }
                        });
                    }
                    Ok(PublicRelationshipList::Alias { href }) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            let _ = window().location().replace(&href);
                        }
                    }
                    Ok(PublicRelationshipList::Restart { .. }) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            load_fresh(None);
                        }
                    }
                    Err(ApiError::RelationshipListPrivate) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.set(Some(LiveListState::Private));
                        }
                    }
                    Err(ApiError::ProfileGone) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.set(Some(LiveListState::Gone));
                        }
                    }
                    Err(ApiError::NotFound) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.set(Some(LiveListState::Missing));
                        }
                    }
                    Err(_) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            live_state.update(|s| {
                                if let Some(LiveListState::Page {
                                    appending,
                                    append_error,
                                    ..
                                }) = s
                                {
                                    *appending = false;
                                    *append_error = true;
                                }
                            });
                        }
                    }
                }
            }
        });

        cur
    });

    super::super::profile::observe_changes(move || reload_trigger.update(|n| *n += 1));
    crate::read_events::observe(move || {
        if crate::read_events::visible() {
            reload_trigger.update(|n| *n += 1);
        }
    });
    if let Ok(channel) = web_sys::BroadcastChannel::new(FOLLOW_CHANNEL) {
        let owner = Owner::current().map(|o| o.downgrade());
        let listener = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| {
            if let Some(owner) = owner.as_ref().and_then(|o| o.upgrade()) {
                owner.with(|| reload_trigger.update(|n| *n += 1));
            }
        });
        channel.set_onmessage(Some(listener.as_ref().unchecked_ref()));
        let guard = SendWrapper::new((channel, listener));
        on_cleanup(move || {
            guard.0.set_onmessage(None);
            guard.0.close();
        });
    }

    view! { <span></span> }
}

async fn load_public_into(
    handle: &str,
    direction: FollowDirection,
    cursor: Option<&str>,
    generation: u64,
    list_gen: RwSignal<u64>,
    live_state: RwSignal<Option<LiveListState>>,
) {
    let result = api::load_public_relationships(handle, direction, cursor).await;
    if list_gen.try_get_untracked() != Some(generation) {
        return;
    }
    match result {
        Ok(PublicRelationshipList::Page {
            author,
            direction,
            count,
            people,
            next_cursor,
        }) => {
            live_state.set(Some(LiveListState::Page {
                author,
                direction,
                is_own: false,
                count,
                people: people
                    .into_iter()
                    .map(|author| RelationshipPersonRow {
                        author,
                        mutual: false,
                    })
                    .collect(),
                next_cursor,
                appending: false,
                append_error: false,
            }));
        }
        Ok(PublicRelationshipList::Restart { author, direction }) => {
            live_state.set(Some(LiveListState::Restart { author, direction }));
        }
        Ok(PublicRelationshipList::Alias { href }) => {
            let _ = window().location().replace(&href);
        }
        Err(ApiError::RelationshipListPrivate) => live_state.set(Some(LiveListState::Private)),
        Err(ApiError::ProfileGone) => live_state.set(Some(LiveListState::Gone)),
        Err(ApiError::NotFound) => live_state.set(Some(LiveListState::Missing)),
        Err(_) => live_state.set(Some(LiveListState::Unavailable)),
    }
}
