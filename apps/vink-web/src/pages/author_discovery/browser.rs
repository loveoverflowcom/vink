use leptos::{prelude::*, task::spawn_local};
use send_wrapper::SendWrapper;
use vink_ui::{ActionState, Button, ButtonVariant};
use wasm_bindgen::{JsCast, closure::Closure};

use super::{RESULTS_HEADING, SearchResults, rows};
use crate::account::Session;
use crate::api::{self, InvalidRead, LoadError};
use crate::contract::follow::{MembershipItem, ViewerRelationship};
use crate::follow_state::{FollowController, FollowEffect, FollowPhase, FollowScope};
use crate::read_lifecycle::{Intent, PageChain, ReadOwner};

#[wasm_bindgen::prelude::wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = authorSearchReturnCursor)]
    fn author_search_return_cursor() -> Option<String>;
}

pub(super) fn observe(
    cursor: RwSignal<Option<String>>,
    renew: impl Fn() + Copy + Send + Sync + 'static,
) {
    Effect::new(move |_| {
        if let Some(restored) = author_search_return_cursor()
            && cursor.get_untracked().as_ref() != Some(&restored)
        {
            cursor.set(Some(restored));
            renew();
        }
    });
    super::super::profile::observe_changes(renew);
    crate::read_events::observe_lifecycle(move |hint| {
        if hint == crate::read_events::Hint::HistoryRestored {
            renew();
        }
    });
}

#[derive(Clone, PartialEq)]
pub(super) struct Membership {
    scope: String,
    items: Vec<MembershipItem>,
}

pub(super) fn enhance(
    query: StoredValue<String>,
    results: RwSignal<SearchResults>,
) -> (impl Fn() -> bool + Copy, Signal<Option<Membership>>) {
    // Four hydrated pages (12 rows per page) fit in one Graph membership batch.
    // The same next link then opens a fresh page over GET.
    const MAX_HYDRATED_PAGES: usize = 4;
    let paging = RwSignal::new({
        let mut owner = ReadOwner::new(query.get_value());
        let mut chain = PageChain::default();
        if let Some(ticket) = owner.begin(Intent::Initial, None) {
            results.with_untracked(|state| {
                let handles = state.rows.iter().map(|row| row.author.handle.clone());
                owner.accept_page(&ticket, &mut chain, handles, state.next.clone());
            });
            owner.finish(&ticket);
        }
        (owner, chain)
    });
    let append = move || {
        if paging.with_untracked(|(_, chain)| chain.pages >= MAX_HYDRATED_PAGES) {
            return false;
        }
        let next = results.with_untracked(|state| state.next.clone());
        let Some(ticket) = paging
            .try_update(|(owner, _)| owner.begin(Intent::Append, next))
            .flatten()
        else {
            return true;
        };
        results.update(|state| {
            state.appending = true;
            state.append_error = None;
        });
        spawn_local(async move {
            let cursor = ticket.cursor.as_deref();
            let outcome = api::load_author_search(&ticket.scope, cursor).await;
            if paging.try_with_untracked(|(owner, _)| owner.accepts(&ticket)) != Some(true) {
                return;
            }
            let mut focus = None;
            results.update(|state| {
                state.appending = false;
                match outcome {
                    Ok(page) => {
                        let more = rows(page.authors, cursor);
                        let accepted = paging
                            .try_update(|(owner, chain)| {
                                owner.accept_page(
                                    &ticket,
                                    chain,
                                    more.iter().map(|row| row.author.handle.clone()),
                                    page.next_cursor.clone(),
                                )
                            })
                            .unwrap_or(false);
                        if accepted {
                            state.next = page.next_cursor;
                            if state.next.is_none()
                                && document()
                                    .active_element()
                                    .is_some_and(|el| el.id() == "author-search-more")
                            {
                                focus = Some(more.first().map_or_else(
                                    || RESULTS_HEADING.into(),
                                    |row| format!("search-author-{}", row.author.handle),
                                ));
                            }
                            state.rows.extend(more);
                        } else {
                            state.append_error =
                                Some(LoadError::Invalid(InvalidRead::RestartCursor));
                        }
                    }
                    Err(error) => state.append_error = Some(error),
                }
            });
            paging.update(|(owner, _)| {
                owner.finish(&ticket);
            });
            if let Some(id) = focus {
                super::super::focus_recovery(id);
            }
        });
        true
    };

    let session = Session::current();
    let handles = Memo::new(move |_| {
        results.with(|state| {
            state
                .rows
                .iter()
                .map(|row| row.author.handle.clone())
                .collect::<Vec<_>>()
        })
    });
    let generation = RwSignal::new(0u64);
    let items = RwSignal::new(None::<Membership>);
    let hint = RwSignal::new(0u64);
    observe_follow_changes(move || hint.update(|stamp| *stamp += 1));
    Effect::new(move |_| {
        let scope = session.profile_scope.get();
        let handles = handles.get();
        hint.track();
        let stamp = generation.get_untracked() + 1;
        generation.set(stamp);
        let Some(scope) = scope.filter(|_| !handles.is_empty()) else {
            items.set(None);
            return;
        };
        spawn_local(async move {
            let result = api::load_follow_membership(&handles).await;
            if generation.try_get_untracked() != Some(stamp)
                || session.profile_scope.get_untracked().as_ref() != Some(&scope)
            {
                return;
            }
            items.set(result.ok().map(|result| Membership {
                scope,
                items: result.items,
            }));
        });
    });
    let membership = Signal::derive(move || {
        let scope = session.profile_scope.get()?;
        items.get().filter(|items| items.scope == scope)
    });
    (append, membership)
}

fn observe_follow_changes(callback: impl Fn() + 'static) {
    let channel = web_sys::BroadcastChannel::new("vink-follow-changes").ok();
    let owner = Owner::current().map(|owner| owner.downgrade());
    let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        if let Some(owner) = owner.as_ref().and_then(|owner| owner.upgrade()) {
            owner.with(&callback);
        }
    });
    if let Some(channel) = &channel {
        channel.set_onmessage(Some(listener.as_ref().unchecked_ref()));
    }
    let _ = window()
        .add_event_listener_with_callback("vink:follow-changed", listener.as_ref().unchecked_ref());
    let resources = SendWrapper::new((channel, listener));
    on_cleanup(move || {
        let _ = window().remove_event_listener_with_callback(
            "vink:follow-changed",
            resources.1.as_ref().unchecked_ref(),
        );
        if let Some(channel) = &resources.0 {
            channel.set_onmessage(None);
            channel.close();
        }
    });
}

#[component]
pub(super) fn AuthorFollow(
    handle: String,
    membership: Signal<Option<Membership>>,
    on_refresh: Callback<()>,
) -> impl IntoView {
    let target = StoredValue::new(handle);
    let scope = Memo::new(move |_| {
        membership.with(|value| {
            let value = value.as_ref()?;
            target
                .with_value(|target| value.items.iter().any(|item| &item.handle == target))
                .then(|| value.scope.clone())
        })
    });
    view! {
        <For each={move || scope.get().into_iter().collect::<Vec<_>>()} key=Clone::clone
            children=move |scope| view! { <ActiveAuthorFollow target scope membership on_refresh /> } />
    }
}

#[component]
fn ActiveAuthorFollow(
    target: StoredValue<String>,
    scope: String,
    membership: Signal<Option<Membership>>,
    on_refresh: Callback<()>,
) -> impl IntoView {
    use super::super::follow::{announce_follow_changed, generate_command_id};

    let session = Session::current();
    let expected = StoredValue::new(scope);
    let follow_scope = StoredValue::new(FollowScope {
        session_gen: 0,
        target_handle: target.get_value(),
    });
    let controller = RwSignal::new(FollowController::new(follow_scope.get_value()));
    let checking = RwSignal::new(false);
    let active = move || {
        expected
            .try_get_value()
            .is_some_and(|scope| session.profile_scope.get_untracked().as_ref() == Some(&scope))
    };
    let apply = move |effect: FollowEffect| {
        if let Some(invalidation) = effect.invalidation {
            announce_follow_changed(&invalidation);
        }
        if effect.should_focus_heading {
            super::super::focus_recovery(RESULTS_HEADING);
            on_refresh.run(());
        }
    };
    Effect::new(move |_| {
        if let Some(batch) = membership.get()
            && batch.scope == expected.get_value()
            && let Some(item) = batch
                .items
                .into_iter()
                .find(|item| item.handle == target.get_value())
        {
            controller.update(|owner| {
                owner.finish_load(
                    &follow_scope.get_value(),
                    Ok(ViewerRelationship::Other {
                        target_handle: item.handle,
                        can_follow: true,
                        following: item.following,
                        mutual: item.mutual,
                        pair_version: item.pair_version,
                        accepted_order: item.accepted_order,
                    }),
                );
            });
        }
    });
    let reconcile = move |command_id: String| {
        if checking.get_untracked() || !active() {
            return;
        }
        checking.set(true);
        let target = target.get_value();
        spawn_local(async move {
            let receipt = api::load_follow_receipt(&command_id).await;
            let current = api::load_viewer_relationship(&target).await;
            if !active() {
                return;
            }
            checking.set(false);
            if let Some(effect) = controller.try_update(|owner| {
                owner.finish_reconcile(
                    &follow_scope.get_value(),
                    &command_id,
                    Some(receipt),
                    current,
                )
            }) {
                apply(effect);
            }
        });
    };
    let press = move |()| {
        if !active() {
            return;
        }
        if let Some(command) = controller.with_untracked(|owner| match owner.phase() {
            FollowPhase::Reconciling { command, .. } => Some(command.command_id.clone()),
            _ => None,
        }) {
            reconcile(command);
            return;
        }
        let Some(command_id) = generate_command_id() else {
            return;
        };
        let Some(request) = controller
            .try_update(|owner| owner.begin_intent(command_id.clone(), None))
            .flatten()
        else {
            return;
        };
        let handle = target.get_value();
        spawn_local(async move {
            let response = api::set_follow(&handle, &request).await;
            if !active() {
                return;
            }
            if let Some(effect) = controller.try_update(|owner| {
                owner.finish_write(&follow_scope.get_value(), &command_id, response)
            }) {
                apply(effect);
                if controller.try_with_untracked(|owner| {
                    matches!(owner.phase(), FollowPhase::Reconciling { .. })
                }) == Some(true)
                {
                    reconcile(command_id);
                }
            }
        });
    };

    let following = Signal::derive(move || {
        controller.with(|owner| match owner.phase() {
            FollowPhase::Ready { state, .. } => state.following,
            FollowPhase::Pending { shown, .. } | FollowPhase::Reconciling { shown, .. } => {
                shown.following
            }
            _ => false,
        })
    });
    let variant = Signal::derive(move || {
        if following.get() {
            ButtonVariant::Outlined
        } else {
            ButtonVariant::Tonal
        }
    });
    let state = Signal::derive(move || {
        controller.with(|owner| match owner.phase() {
            FollowPhase::Ready {
                can_follow: true, ..
            } => ActionState::Enabled,
            FollowPhase::Pending { .. } => ActionState::pending("Đang gửi yêu cầu theo dõi…"),
            FollowPhase::Reconciling { .. } if checking.get() => {
                ActionState::pending("Đang đối chiếu trạng thái…")
            }
            FollowPhase::Reconciling { .. } => ActionState::Enabled,
            _ => ActionState::Disabled { reason: None },
        })
    });
    view! {
        <div class="vk-author-search__follow">
            <Button variant state on_press=press
                id=target.with_value(|handle| format!("search-follow-{handle}"))
                described_by=target.with_value(|handle| format!("search-author-{handle}"))>
                {move || controller.with(|owner| match owner.phase() {
                    FollowPhase::Reconciling { .. } => "Đối chiếu",
                    _ if following.get() => "Bỏ theo dõi",
                    _ => "Theo dõi",
                })}
            </Button>
            {move || controller.with(|owner| match owner.phase() {
                FollowPhase::Ready { notice: Some(notice), .. } =>
                    Some(view! { <p role="status">{notice.text}</p> }),
                _ => None,
            })}
        </div>
    }
}
