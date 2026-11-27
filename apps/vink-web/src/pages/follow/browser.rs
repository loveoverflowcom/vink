//! Hydrated browser controller for `#follow` and paginated `/:author/:relation` views (#294).

use std::sync::atomic::{AtomicU64, Ordering};

use leptos::{prelude::*, task::spawn_local};
use send_wrapper::SendWrapper;
use vink_ui::{ActionState, Button, ButtonVariant, Link, LinkVariant};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use super::{LiveListState, RelationshipPersonRow};
use crate::account::Session;
use crate::api::{self, LoadError};
use crate::contract::ApiError;
use crate::contract::follow::{
    AuthorRelationshipsLoad, FollowDirection, OwnRelationshipList, OwnRelationshipsSummary,
    PublicRelationshipList, PublicRelationships, ViewerRelationship,
};
use crate::follow_state::{
    FollowController, FollowInvalidation, FollowPhase, FollowScope, own_relationship_list_matches,
};

pub(in crate::pages) fn observe_changes(on_change: impl Fn() + Clone + 'static) {
    let owner = Owner::current().map(|owner| owner.downgrade());
    let local = {
        let owner = owner.clone();
        let on_change = on_change.clone();
        window_event_listener_untyped("vink:follow-changed", move |_| {
            if let Some(owner) = owner.as_ref().and_then(|owner| owner.upgrade()) {
                owner.with(&on_change);
            }
        })
    };
    on_cleanup(move || local.remove());
    if let Ok(channel) = web_sys::BroadcastChannel::new(FOLLOW_CHANNEL) {
        let listener = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| {
            if let Some(owner) = owner.as_ref().and_then(|owner| owner.upgrade()) {
                owner.with(&on_change);
            }
        });
        channel.set_onmessage(Some(listener.as_ref().unchecked_ref()));
        let guard = SendWrapper::new((channel, listener));
        on_cleanup(move || {
            guard.0.set_onmessage(None);
            guard.0.close();
        });
    }
}

const FOLLOW_CHANNEL: &str = "vink-follow-changes";
const SLOW_RECONCILE_DELAY_MS: i32 = 1000;

static NEXT_SCOPE_ID: AtomicU64 = AtomicU64::new(1);

fn next_scope_id() -> u64 {
    NEXT_SCOPE_ID.fetch_add(1, Ordering::Relaxed)
}

pub(in crate::pages) fn generate_command_id() -> Option<String> {
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

pub(in crate::pages) fn announce_follow_changed(invalidation: &FollowInvalidation) {
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
    on_summary: Callback<Option<PublicRelationships>>,
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

    let reconcile_in_flight = RwSignal::<Option<String>>::new(None);
    let scope_token = StoredValue::new(scope);
    let typed_scope = StoredValue::new(follow_scope);
    let stored_handle = StoredValue::new(handle);

    let still_live = move || -> bool {
        scope_token.try_get_value().is_some_and(|expected| {
            session.profile_scope.get_untracked().as_deref() == Some(expected.as_str())
        })
    };

    // Ownership comes from the profile record. Graph supplies counts and the viewer's edge.
    let verified_owner = RwSignal::new(false);
    spawn_local(async move {
        if let Ok(profile) = api::load_own_profile().await
            && still_live()
            && session.principal.get_untracked().as_deref() == Some(profile.principal_id.as_str())
        {
            verified_owner.set(profile.handle == stored_handle.get_value());
        }
    });

    let apply_invalidation = move |invalidation: FollowInvalidation| {
        on_summary.run(invalidation.relationships.clone());
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
        if reconcile_in_flight.with_untracked(|id| id.as_ref() == Some(&command_id))
            || controller.try_with_untracked(|c| c.is_current_command(&command_id)) != Some(true)
        {
            return;
        }
        reconcile_in_flight.set(Some(command_id.clone()));
        let target = stored_handle.get_value();
        let my_scope = typed_scope.get_value();
        spawn_local(async move {
            let receipt = Some(api::load_follow_receipt(&command_id).await);
            let viewer = api::load_viewer_relationship(&target).await;
            if !still_live() {
                return;
            }
            if reconcile_in_flight.with_untracked(|id| id.as_ref() == Some(&command_id)) {
                reconcile_in_flight.set(None);
            }
            if controller.try_with_untracked(|c| c.is_current_command(&command_id)) != Some(true) {
                return;
            }
            let was_focused = focus_element_in_follow();
            let was_reconcile = document()
                .active_element()
                .is_some_and(|el| el.id() == "follow-reconcile");
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
            } else if was_reconcile
                && controller.try_with_untracked(|c| matches!(c.phase(), FollowPhase::Ready { .. }))
                    == Some(true)
            {
                super::super::focus_recovery("follow-action");
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
            if !still_live()
                || controller.try_with_untracked(|c| c.is_latest_command(&command_id)) != Some(true)
            {
                return;
            }
            let was_focused = focus_element_in_follow();
            let was_reconcile = document()
                .active_element()
                .is_some_and(|el| el.id() == "follow-reconcile");
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
            } else if was_reconcile && matches!(phase_now, Some(FollowPhase::Ready { .. })) {
                super::super::focus_recovery("follow-action");
            }
        });
    });

    // Keep one persistent `#follow-action` DOM element across Ready/Pending/Reconciling so
    // keyboard focus never drops during state transitions.
    let is_owner = Signal::derive(move || {
        verified_owner.get()
            || controller.with(|c| matches!(c.phase(), FollowPhase::SelfProfile(_)))
    });
    let show_action = Signal::derive(move || {
        !is_owner.get()
            && controller.with(|c| {
                matches!(
                    c.phase(),
                    FollowPhase::Ready {
                        can_follow: true,
                        ..
                    } | FollowPhase::Pending { .. }
                        | FollowPhase::Reconciling { .. }
                )
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
            // #588: Follow is a tonal action beside the counts, not the page's primary CTA.
            ButtonVariant::Tonal
        }
    });
    let action_state = Signal::derive(move || {
        controller.with(|c| match c.phase() {
            FollowPhase::Ready {
                can_follow: true, ..
            } => ActionState::Enabled,
            FollowPhase::Pending { .. } => ActionState::pending("Đang gửi yêu cầu theo dõi…"),
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
        if is_owner.get() {
            return (false, "");
        }
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

    let show_reconcile = Memo::new(move |_| reconcile_cmd_id.with(Option::is_some));
    let reconcile_state = Signal::derive(move || {
        if reconcile_in_flight.with(Option::is_some) {
            ActionState::Disabled { reason: None }
        } else {
            ActionState::Enabled
        }
    });
    view! {
        <div class="vk-follow-controls">
          <div class="vk-follow-actions">
            <Show when=move || is_owner.get()>
                <Link variant=LinkVariant::Standalone attr:id="own-profile-edit-link"
                    attr:data-profile-return="true" href=super::super::profile::EDIT_PATH>
                    "Chỉnh hồ sơ"
                </Link>
            </Show>
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
            <Show when=move || !is_owner.get() && controller.with(|c| matches!(c.phase(), FollowPhase::Error))>
                <Button
                    id="follow-retry"
                    variant=ButtonVariant::Tonal
                    on_press=Callback::new(move |()| trigger_load())>
                    "Thử lại"
                </Button>
            </Show>
          </div>
          <p id="follow-status" role="status" aria-atomic="true"
              data-error=move || status_info.get().0.then_some("true")>
              {move || status_info.get().1}
          </p>
          <Show when=move || show_reconcile.get()>
              <Button id="follow-reconcile" variant=ButtonVariant::Tonal state=reconcile_state
                  on_press=Callback::new(move |()| {
                      if let Some(cmd) = reconcile_cmd_id.get_untracked() { run_reconcile(cmd); }
                  })>
                  "Đối chiếu trạng thái"
              </Button>
          </Show>
        </div>
    }
}

#[wasm_bindgen::prelude::wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = relationshipReturnCursor)]
    fn relationship_return_cursor(path: &str) -> Option<String>;
    #[wasm_bindgen(js_name = focusRelationshipReturn)]
    fn focus_relationship_return(cursor: &str);
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
    first_trigger: RwSignal<u64>,
) -> impl IntoView {
    let session = Session::current();
    let list_gen = RwSignal::new(0u64);
    let return_cursor =
        relationship_return_cursor(&format!("/@{name}/{}", direction.web_segment()));
    let pending_return = StoredValue::new(return_cursor.clone());
    let resume_cursor = RwSignal::new(return_cursor);
    let stored_name = StoredValue::new(name);
    let stored_cursor = StoredValue::new(cursor);
    let recover_heading = RwSignal::new(false);
    let request_heading = move || {
        if document()
            .active_element()
            .is_some_and(|el| el.closest("#relationship-list").ok().flatten().is_some())
        {
            recover_heading.set(true);
        }
    };
    Effect::new(move |_| {
        if live_state.with(
            |state| matches!(state, Some(current) if !matches!(current, LiveListState::Loading)),
        ) && recover_heading.get_untracked()
        {
            recover_heading.set(false);
            focus_list_after_render("relationship-title".into());
        }
    });

    let load_fresh = move |cursor_override: Option<String>, force_public: bool| {
        request_heading();
        let generation = list_gen.with_untracked(|g| *g) + 1;
        list_gen.set(generation);
        let handle = stored_name.get_value();
        let active_scope = session.profile_scope.get_untracked();
        live_state.set(Some(LiveListState::Loading));

        spawn_local(async move {
            let is_own = !force_public
                && active_scope.is_some()
                && matches!(
                    api::load_viewer_relationship(&handle).await,
                    Ok(ViewerRelationship::SelfProfile { target_handle, .. }) if target_handle == handle
                );
            let still_current = || {
                list_gen.try_get_untracked() == Some(generation)
                    && session.profile_scope.try_get_untracked() == Some(active_scope.clone())
            };
            if !still_current() {
                return;
            }

            let mut state =
                read_relationship_page(&handle, direction, is_own, cursor_override.as_deref())
                    .await;
            if !still_current() {
                return;
            }
            let saved_cursor = pending_return.get_value();
            let resume = resume_cursor.get_untracked();
            let resumed = resume.is_some() && cursor_override == resume;
            if resumed
                && (matches!(state, LiveListState::Restart { .. })
                    || matches!(&state, LiveListState::Page { people, .. } if people.is_empty()))
            {
                // This cursor can be invalid even if a fresh read for the current scope is allowed.
                let route = stored_cursor.get_value();
                state = read_relationship_page(&handle, direction, is_own, route.as_deref()).await;
                if !still_current() {
                    return;
                }
                resume_cursor.set(None);
            } else if resumed && let LiveListState::Page { people, .. } = &mut state {
                for row in people {
                    row.return_cursor.clone_from(&cursor_override);
                }
            }
            live_state.set(Some(state));
            if let Some(saved) = saved_cursor {
                pending_return.set_value(None);
                focus_relationship_return(&saved);
            }
        });
    };

    let requested_cursor = move || {
        resume_cursor
            .get_untracked()
            .or_else(|| stored_cursor.get_value())
    };

    // Re-read a Back cursor using the current session, after its initial check finishes.
    // Revocation or a change of session never reuses previously projected rows.
    Effect::new(move |previous: Option<(bool, Option<String>)>| {
        let known = !matches!(session.standing.get(), crate::session::Standing::Unknown);
        let scope = session.profile_scope.get();
        let current = (known, scope);
        if current.0
            && (current.1.is_some()
                || pending_return.with_value(Option::is_some)
                || previous.as_ref().is_some_and(|old| old.1.is_some()))
            && previous.as_ref() != Some(&current)
        {
            load_fresh(requested_cursor(), false);
        }
        current
    });

    Effect::new(move |prev: Option<u64>| {
        let cur = reload_trigger.get();
        if prev.is_some_and(|p| p != cur)
            && (!pending_return.with_value(Option::is_some)
                || !matches!(
                    session.standing.get_untracked(),
                    crate::session::Standing::Unknown
                ))
        {
            load_fresh(requested_cursor(), false);
        }
        cur
    });

    Effect::new(move |previous: Option<u64>| {
        let current = first_trigger.get();
        if previous.is_some_and(|old| old != current) {
            super::super::clear_profile_return();
            pending_return.set_value(None);
            resume_cursor.set(None);
            load_fresh(None, false);
        }
        current
    });

    // React to pagination Append triggers (`Tải thêm`).
    Effect::new(move |prev: Option<u64>| {
        let cur = append_trigger.get();
        if prev.is_none_or(|p| p == cur) {
            return cur;
        }
        let snapshot = live_state
            .get_untracked()
            .or_else(|| ssr_data.get_untracked()?.1.ok().map(LiveListState::from));
        let Some(mut page_snapshot) = snapshot else {
            return cur;
        };
        let (is_own, cursor) = match &mut page_snapshot {
            LiveListState::Page {
                is_own,
                next_cursor,
                appending,
                ..
            } if !*appending => {
                let Some(next) = next_cursor.clone() else {
                    return cur;
                };
                *appending = true;
                (*is_own, next)
            }
            _ => return cur,
        };

        let generation = list_gen.get_untracked();
        let active_scope = session.profile_scope.get_untracked();
        let handle = stored_name.get_value();
        if is_own && active_scope.is_none() {
            resume_cursor.set(None);
            load_fresh(None, true);
            return cur;
        }
        live_state.set(Some(page_snapshot));

        spawn_local(async move {
            // The HTTP request is already tied to this cursor. Before applying any reply,
            // verify that the mounted list still owns that exact append and session scope.
            let owns_append = || {
                list_gen.try_get_untracked() == Some(generation)
                    && session.profile_scope.try_get_untracked() == Some(active_scope.clone())
                    && live_state.try_with_untracked(|state| {
                        matches!(state,
                            Some(LiveListState::Page { next_cursor, appending: true, .. })
                                if next_cursor.as_deref() == Some(cursor.as_str())
                        )
                    }) == Some(true)
            };
            if is_own {
                let result = api::load_own_relationships(direction, Some(&cursor)).await;
                if !owns_append() {
                    return;
                }
                match result {
                    Ok(ref answer)
                        if !own_relationship_list_matches(answer, &handle, direction) =>
                    {
                        resume_cursor.set(None);
                        load_fresh(None, true);
                    }
                    Ok(OwnRelationshipList::Page {
                        count,
                        people,
                        next_cursor,
                        ..
                    }) => {
                        let focus = append_focus(people.first().map(|row| &row.author));
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
                                        return_cursor: Some(cursor.clone()),
                                    }
                                }));
                                *nc = next_cursor;
                                *appending = false;
                                *append_error = false;
                            }
                        });
                        if let Some(id) = focus {
                            focus_list_after_render(id);
                        }
                    }
                    Ok(OwnRelationshipList::Restart { .. }) => {
                        resume_cursor.set(None);
                        load_fresh(None, false);
                    }
                    Err(ApiError::RelationshipListPrivate) => {
                        request_heading();
                        live_state.set(Some(LiveListState::Private));
                    }
                    Err(ApiError::ProfileGone) => {
                        request_heading();
                        live_state.set(Some(LiveListState::Gone));
                    }
                    Err(ApiError::NotFound) => {
                        request_heading();
                        live_state.set(Some(LiveListState::Missing));
                    }
                    Err(
                        ApiError::Unauthenticated
                        | ApiError::Forbidden
                        | ApiError::FollowActorUnavailable,
                    ) => {
                        resume_cursor.set(None);
                        load_fresh(None, true);
                    }
                    Err(_) => {
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
            } else {
                let result =
                    api::load_public_relationships(&handle, direction, Some(&cursor)).await;
                if !owns_append() {
                    return;
                }
                match result {
                    Ok(PublicRelationshipList::Page {
                        count,
                        people,
                        next_cursor,
                        ..
                    }) => {
                        if list_gen.try_get_untracked() != Some(generation) {
                            return;
                        }
                        let focus = append_focus(people.first());
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
                                        return_cursor: Some(cursor.clone()),
                                    }
                                }));
                                *nc = next_cursor;
                                *appending = false;
                                *append_error = false;
                            }
                        });
                        if let Some(id) = focus {
                            focus_list_after_render(id);
                        }
                    }
                    Ok(PublicRelationshipList::Alias { href }) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            super::super::handle::redirect_alias(
                                &super::super::ResponseStatus::capture(),
                                &format!("/@{handle}/{}", direction.web_segment()),
                                &href,
                            );
                        }
                    }
                    Ok(PublicRelationshipList::Restart { .. }) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            resume_cursor.set(None);
                            load_fresh(None, false);
                        }
                    }
                    Err(ApiError::RelationshipListPrivate) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            request_heading();
                            live_state.set(Some(LiveListState::Private));
                        }
                    }
                    Err(ApiError::ProfileGone) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            request_heading();
                            live_state.set(Some(LiveListState::Gone));
                        }
                    }
                    Err(ApiError::NotFound) => {
                        if list_gen.try_get_untracked() == Some(generation) {
                            request_heading();
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
    observe_changes(move || reload_trigger.update(|n| *n += 1));

    view! { <span></span> }
}

fn append_focus(first: Option<&crate::contract::PublicAuthorRef>) -> Option<String> {
    document()
        .active_element()
        .filter(|el| el.id() == "rel-load-more")?;
    Some(
        first
            .filter(|author| author.valid_href().is_some())
            .map_or_else(
                || "relationship-title".into(),
                |author| format!("rel-person-{}", author.handle),
            ),
    )
}

fn focus_list_after_render(id: String) {
    let path = window().location().href().ok();
    request_animation_frame(move || {
        if window().location().href().ok() == path
            && document()
                .active_element()
                .is_some_and(|el| el.tag_name() == "BODY" || el.id() == "rel-load-more")
            && let Some(target) = document().get_element_by_id(&id)
            && let Ok(target) = target.dyn_into::<web_sys::HtmlElement>()
        {
            let _ = target.focus();
        }
    });
}

async fn read_relationship_page(
    handle: &str,
    direction: FollowDirection,
    is_own: bool,
    cursor: Option<&str>,
) -> LiveListState {
    if is_own {
        match api::load_own_relationships(direction, cursor).await {
            Ok(ref answer) if !own_relationship_list_matches(answer, handle, direction) => {}
            Ok(OwnRelationshipList::Page {
                author,
                direction,
                count,
                people,
                next_cursor,
            }) => {
                return LiveListState::Page {
                    author,
                    direction,
                    is_own: true,
                    count: Some(count),
                    people: people
                        .into_iter()
                        .map(|p| RelationshipPersonRow {
                            author: p.author,
                            mutual: p.mutual,
                            return_cursor: None,
                        })
                        .collect(),
                    next_cursor,
                    appending: false,
                    append_error: false,
                };
            }
            Ok(OwnRelationshipList::Restart { author, direction }) => {
                return LiveListState::Restart { author, direction };
            }
            Err(
                ApiError::Unauthenticated | ApiError::Forbidden | ApiError::FollowActorUnavailable,
            ) => {}
            Err(ApiError::ProfileGone) => return LiveListState::Gone,
            Err(ApiError::NotFound) => return LiveListState::Missing,
            Err(_) => return LiveListState::Unavailable,
        }
    }
    match api::load_public_relationships(handle, direction, cursor).await {
        Ok(page) => LiveListState::from(AuthorRelationshipsLoad::from(page)),
        Err(ApiError::RelationshipListPrivate) => LiveListState::Private,
        Err(ApiError::ProfileGone) => LiveListState::Gone,
        Err(ApiError::NotFound) => LiveListState::Missing,
        Err(_) => LiveListState::Unavailable,
    }
}
