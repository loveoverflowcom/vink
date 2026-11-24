use std::collections::HashMap;

use leptos::{prelude::*, task::spawn_local};
use vink_ui::{ActionState, Button, ButtonVariant, Link};

use super::{FEED_STATE_CLASS, FeedOff, Restricted, SignIn};
use crate::{
    account::Session,
    api,
    contract::{ApiError, FeedResponse, WorkCard},
    discovery::{ExploreContext, ExploreMode, encode},
    pages::ExploreRestore,
    read_lifecycle::{Intent, PageChain, ReadOwner, ReadTicket},
};

const CHECK_LIMIT: usize = 48;

#[wasm_bindgen::prelude::wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = rememberFeedReturn, catch)]
    fn remember_feed_return(work: &str, scope: &str) -> Result<(), wasm_bindgen::JsValue>;
    #[wasm_bindgen(js_name = feedReturnScope)]
    fn feed_return_scope(work: &str) -> Option<String>;
}

#[derive(Clone, Copy)]
pub(in crate::pages::discovery) struct ReturnScope(Signal<Option<String>>);

impl ReturnScope {
    pub(in crate::pages::discovery) fn remember(self, work: &str) {
        if let Some(scope) = self.0.try_get_untracked().flatten() {
            let _ = remember_feed_return(work, &scope);
        }
    }
}

fn return_hint(hash: &str) -> Option<(String, String)> {
    let work = crate::pages::profile_return_work()
        .or_else(|| hash.strip_prefix("#work-").map(str::to_owned))?;
    let scope = feed_return_scope(&work)?;
    Some((work, scope))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Ready { has_following: bool },
    Unavailable,
    Expired,
    Restricted,
    Disabled,
    Restart,
}

struct Feed {
    owner: ReadOwner<String>,
    chain: PageChain,
    phase: Phase,
    works: Vec<WorkCard>,
    append_error: bool,
    return_scope: Option<String>,
}

impl Feed {
    fn reset(&mut self, phase: Phase) {
        self.owner.invalidate();
        self.chain = PageChain::default();
        self.phase = phase;
        self.works.clear();
        self.append_error = false;
        self.return_scope = None;
    }

    fn recheck(&mut self, checked: &[String], fresh: Vec<WorkCard>) {
        let mut by_id: HashMap<_, _> = fresh
            .into_iter()
            .map(|work| (work.work_id.clone(), work))
            .collect();
        self.works.retain_mut(|work| {
            if !checked.contains(&work.work_id) {
                return true;
            }
            if let Some(current) = by_id.remove(&work.work_id) {
                *work = current;
                true
            } else {
                false
            }
        });
    }

    fn fail(&mut self, intent: Intent, error: ApiError) {
        match error {
            ApiError::Unauthenticated => self.reset(Phase::Expired),
            ApiError::Forbidden | ApiError::FollowActorUnavailable => self.reset(Phase::Restricted),
            ApiError::NotFound => self.reset(Phase::Disabled),
            _ if intent == Intent::Append => self.append_error = true,
            _ => self.reset(Phase::Unavailable),
        }
    }
}

fn page_query(cursor: Option<&str>, around: Option<&str>, seen: &[String]) -> String {
    let mut pairs = Vec::new();
    if let Some(cursor) = cursor {
        pairs.push(format!("cursor={}", encode(cursor)));
    }
    if let Some(work) = around {
        pairs.push(format!("around={}", encode(work)));
    }
    if !seen.is_empty() {
        pairs.push(format!(
            "seen={}",
            seen.iter()
                .map(|id| encode(id))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

/// Recover only when a control the user was on has actually disappeared. A late response
/// cannot move focus away from a new target or a subsequent route.
fn recover_removed_focus() {
    let Some(active) = document().active_element() else {
        return;
    };
    let Some(region) = document().get_element_by_id("community-stream") else {
        return;
    };
    if active == region || !region.contains(Some(&active)) {
        return;
    }
    request_animation_frame(move || {
        use wasm_bindgen::JsCast;
        if !active.is_connected()
            && region.is_connected()
            && document()
                .active_element()
                .is_some_and(|current| current.tag_name() == "BODY")
            && let Ok(region) = region.dyn_into::<web_sys::HtmlElement>()
        {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = region.focus_with_options(&options);
        }
    });
}

#[component]
pub(super) fn ActiveFeed(
    scope: String,
    context: ExploreContext,
    expired: RwSignal<bool>,
    restore: RwSignal<Option<ExploreRestore>>,
    return_completed: RwSignal<bool>,
) -> impl IntoView {
    let session = Session::current();
    let scope = StoredValue::new(scope);
    let feed = RwSignal::new(Feed {
        owner: ReadOwner::new(scope.get_value()),
        chain: PageChain::default(),
        phase: Phase::Loading,
        works: Vec::new(),
        append_error: false,
        return_scope: None,
    });
    provide_context(ReturnScope(Signal::derive(move || {
        (session.profile_scope.get() == Some(scope.get_value()))
            .then(|| feed.with(|state| state.return_scope.clone()))
            .flatten()
    })));
    let live = move |ticket: &ReadTicket<String>| {
        session.profile_scope.try_get_untracked().flatten().as_ref() == Some(&ticket.scope)
            && feed.try_with_untracked(|state| state.owner.accepts(ticket)) == Some(true)
    };
    let location = leptos_router::hooks::use_location();
    let read = Callback::new(move |intent: Intent| {
        let Some(principal) = session.principal.get_untracked() else {
            return;
        };
        if session.profile_scope.get_untracked() != Some(scope.get_value()) {
            return;
        }
        let Some(ticket) = feed
            .try_update(|state| {
                let cursor = if intent == Intent::Append {
                    state.chain.next_cursor.clone()
                } else {
                    None
                };
                state.owner.begin(intent, cursor)
            })
            .flatten()
        else {
            return;
        };
        let returning = intent != Intent::Append && !return_completed.get_untracked();
        if returning {
            restore.set(None);
        }
        let around = returning
            .then(|| return_hint(&location.hash.get_untracked()))
            .flatten();
        let checked = if intent == Intent::Append {
            feed.with_untracked(|state| {
                state
                    .works
                    .iter()
                    .map(|work| work.work_id.clone())
                    .collect()
            })
        } else {
            Vec::new()
        };
        feed.update(|state| state.append_error = false);

        spawn_local(async move {
            let mut batches = checked.chunks(CHECK_LIMIT);
            let first = batches.next().unwrap_or_default();
            let mut reply = None;
            // Long lists recheck their earlier cards in bounded batches before committing
            // the append. Normal pagination checks the existing cards in the same snapshot.
            for batch in batches {
                let result = api::load_following_feed(&page_query(None, None, batch), None).await;
                if !live(&ticket) {
                    return;
                }
                match result {
                    Ok((FeedResponse::Checked { viewer, retained }, _)) if viewer == principal => {
                        recover_removed_focus();
                        feed.update(|state| state.recheck(batch, retained));
                    }
                    other => {
                        reply = Some(other);
                        break;
                    }
                }
            }
            let result = match reply {
                Some(result) => result,
                None => {
                    api::load_following_feed(
                        &page_query(
                            ticket.cursor.as_deref(),
                            around.as_ref().map(|(work, _)| work.as_str()),
                            first,
                        ),
                        around.as_ref().map(|(_, scope)| scope.as_str()),
                    )
                    .await
                }
            };
            if !live(&ticket) {
                return;
            }
            recover_removed_focus();
            let viewer_changed = result
                .as_ref()
                .ok()
                .and_then(|(page, _)| page.viewer())
                .is_some_and(|viewer| viewer != principal);
            let anchor = result.as_ref().ok().and_then(|(_, returned_scope)| {
                around.as_ref().and_then(|(work, scope)| {
                    (returned_scope.as_ref() == Some(scope)).then(|| work.clone())
                })
            });
            let settled = feed.try_update(|state| {
                match result {
                    Ok(_) if viewer_changed => state.reset(Phase::Loading),
                    Ok((
                        FeedResponse::Page {
                            has_following,
                            works,
                            retained,
                            next_cursor,
                            ..
                        },
                        return_scope,
                    )) => {
                        let ids = works.iter().map(|work| work.work_id.clone());
                        if !state
                            .owner
                            .accept_page(&ticket, &mut state.chain, ids, next_cursor)
                        {
                            state.fail(intent, ApiError::Unexpected { status: 200 });
                        } else {
                            if intent == Intent::Append {
                                state.recheck(first, retained);
                                state.works.extend(works);
                            } else {
                                state.works = works;
                            }
                            state.return_scope = return_scope;
                            state.phase = Phase::Ready { has_following };
                        }
                    }
                    Ok((FeedResponse::Restart, _)) => state.reset(Phase::Restart),
                    Ok((FeedResponse::Checked { .. }, _)) => {
                        state.fail(intent, ApiError::Unexpected { status: 200 })
                    }
                    Err(error) => {
                        if error == ApiError::Unauthenticated {
                            expired.set(true);
                        }
                        state.fail(intent, error);
                    }
                }
                state.owner.finish(&ticket);
                matches!(
                    state.phase,
                    Phase::Ready { .. } | Phase::Expired | Phase::Restricted | Phase::Disabled
                )
            });
            if viewer_changed {
                session.fence_profile();
            } else if returning && settled == Some(true) {
                restore.set(Some(ExploreRestore::Feed { ticket, anchor }));
            }
        });
    });

    let restart = move || {
        restore.set(None);
        recover_removed_focus();
        feed.update(|state| state.reset(Phase::Loading));
        read.run(Intent::Restart);
    };
    Effect::new(move |_| {
        expired.set(false);
        read.run(Intent::Initial);
    });
    super::super::super::follow::observe_changes(restart);
    super::super::super::profile::observe_changes(restart);
    crate::read_events::observe_lifecycle(move |hint| {
        if crate::read_events::visible() {
            match hint {
                crate::read_events::Hint::HistoryRestored => restart(),
                crate::read_events::Hint::Wake => read.run(Intent::Refresh),
            }
        }
    });
    // A frozen history document must never paint rows from before revalidation.
    let pagehide = window_event_listener(leptos::ev::pagehide, move |_| {
        restore.set(None);
        feed.update(|state| state.reset(Phase::Loading));
        if let Some(rows) = document().get_element_by_id("following-cards") {
            let _ = rows.set_attribute("hidden", "");
        }
    });
    on_cleanup(move || pagehide.remove());

    let phase = Memo::new(move |_| feed.with(|state| state.phase));
    let ids = Memo::new(move |_| {
        feed.with(|state| {
            state
                .works
                .iter()
                .map(|work| work.work_id.clone())
                .collect::<Vec<_>>()
        })
    });
    let appending = Memo::new(move |_| {
        feed.with(|state| state.owner.in_flight_intent() == Some(Intent::Append))
    });
    let can_append = Memo::new(move |_| feed.with(|state| state.chain.next_cursor.is_some()));
    view! {
        <div class="vk-feed-content" aria-busy=move || (phase.get() == Phase::Loading || appending.get()).to_string()>
            {move || match phase.get() {
                Phase::Loading => view! { <p role="status" data-feed-state="loading">"Đang tải tác phẩm…"</p> }.into_any(),
                Phase::Ready { has_following } if ids.with(Vec::is_empty) => view! {
                    <div class=FEED_STATE_CLASS data-feed-state=if has_following { "no-works" } else { "no-follows" }>
                        <p class="m-0" role="status">{if has_following { "Chưa có tác phẩm công khai đủ điều kiện." }
                        else { "Bạn chưa theo dõi tác giả nào." }}</p>
                        {(!has_following).then(|| view! { <Link href=ExploreMode::AUTHORS_HREF>"Tìm tác giả"</Link> })}
                    </div>
                }.into_any(),
                Phase::Ready { .. } => ().into_any(),
                Phase::Expired => view! { <SignIn expired=true /> }.into_any(),
                Phase::Restricted => view! { <Restricted /> }.into_any(),
                Phase::Disabled => view! { <FeedOff /> }.into_any(),
                Phase::Restart | Phase::Unavailable => {
                    let cursor_expired = phase.get_untracked() == Phase::Restart;
                    view! {
                        <div class=FEED_STATE_CLASS data-feed-state=if cursor_expired { "restart" } else { "error" }>
                            <p class="m-0" role="alert">{if cursor_expired { "Danh sách đã thay đổi. Hãy tải lại từ đầu." }
                            else { "Chưa tải được tác phẩm đang theo dõi." }}</p>
                            <Button id="feed-restart" variant=ButtonVariant::Tonal
                                on_press=move |_| restart()>
                                {if cursor_expired { "Tải lại từ đầu" } else { "Thử lại" }}
                            </Button>
                        </div>
                    }.into_any()
                }
            }}
            <Show when=move || !ids.with(Vec::is_empty)>
                <ul class=super::super::EXPLORE_WORK_LIST_CLASS id="following-cards" role="list">
                    <For each=move || ids.get() key=Clone::clone children={
                        let context = context.clone();
                        move |id| {
                            let row = id.clone();
                            let work = Memo::new(move |_| feed.with(|state| {
                                state.works.iter().find(|work| work.work_id == row).cloned()
                            }));
                            view! { <super::super::WorkCardRow work_id=id work context=context.clone()
                                cached_unverified=Signal::stored(false) /> }
                        }
                    } />
                </ul>
            </Show>
            <Show when=move || can_append.get()>
                <div class="my-vk-6">
                    <Button id="feed-load-more" variant=ButtonVariant::Tonal
                        state=Signal::derive(move || if appending.get() { ActionState::pending("Đang tải thêm") } else { ActionState::Enabled })
                        on_press=move |_| read.run(Intent::Append)>"Tải thêm"</Button>
                    <Show when=move || feed.with(|state| state.append_error)>
                        <p role="alert" data-feed-state="append-error">"Chưa tải thêm được tác phẩm. Hãy thử lại."</p>
                    </Show>
                </div>
            </Show>
        </div>
    }
}
