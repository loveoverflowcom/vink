use std::time::Duration;

use leptos::{prelude::*, task::spawn_local};
use vink_ui::BadgeState;
use wasm_bindgen::JsCast;

use super::{
    Notifications,
    contract::*,
    state::{self, Inbox, Load},
};
use crate::{account::Session, api, contract::ApiError};

#[derive(Default)]
pub(super) struct Poll {
    timer: Option<leptos::leptos_dom::helpers::TimeoutHandle>,
    running: bool,
    /// Invalidates an in-flight result when Seen or MarkAllRead finishes.
    revision: u64,
    failures: u32,
}

impl Notifications {
    pub(super) fn connect(self, open: Signal<bool>) {
        let previous = StoredValue::new(None);
        Effect::new(move |_| {
            let key = self
                .session
                .profile_scope
                .get()
                .zip(self.session.notification_scope.get());
            if previous.get_value() == key {
                return;
            }
            previous.set_value(key);
            self.stop_timer();
            self.poll.set_value(Poll::default());
            self.count.set(BadgeState::Hidden);
            self.poll_now();
        });

        Effect::new(move |_| {
            let scope = self
                .session
                .profile_scope
                .get()
                .zip(self.session.notification_scope.get());
            let show = open.get() && scope.is_some();
            self.view_generation.update(|current| *current += 1);
            self.inbox.set(Inbox::default());
            if show {
                self.load(false);
            }
        });

        let on_environment = move || {
            let online = window().navigator().on_line();
            self.offline.set(!online);
            self.stop_timer();
            if !online {
                self.count.update(|badge| *badge = state::stale(*badge));
            }
            if can_poll() && self.scope().is_some() {
                self.schedule(500 + jitter(1500));
                if let Some(command) = self
                    .inbox
                    .with_untracked(|inbox| inbox.seen_command.clone())
                {
                    self.seen(command, self.view_generation.get_untracked());
                }
            }
        };
        self.offline.set(!window().navigator().on_line());
        let visible =
            window_event_listener(leptos::ev::visibilitychange, move |_| on_environment());
        let online = window_event_listener(leptos::ev::online, move |_| on_environment());
        let offline = window_event_listener(leptos::ev::offline, move |_| on_environment());
        let freeze = window_event_listener(leptos::ev::pagehide, move |event| {
            if event.persisted() {
                self.stop_timer();
                self.session.suspend_private_scope();
                self.inbox.set(Inbox::default());
                self.count.set(BadgeState::Hidden);
            }
        });
        let restore = window_event_listener(leptos::ev::pageshow, move |event| {
            if event.persisted() {
                self.session.refresh();
            }
        });
        on_cleanup(move || {
            self.stop_timer();
            visible.remove();
            online.remove();
            offline.remove();
            freeze.remove();
            restore.remove();
        });
    }

    fn scope(self) -> Option<(String, String)> {
        self.session
            .profile_scope
            .try_get_untracked()
            .flatten()
            .zip(
                self.session
                    .notification_scope
                    .try_get_untracked()
                    .flatten(),
            )
    }

    fn live(self, scope: &(String, String), generation: u64) -> bool {
        self.view_generation.try_get_untracked() == Some(generation)
            && self.scope().as_ref() == Some(scope)
    }

    fn stop_timer(self) {
        self.poll.try_update_value(|poll| {
            if let Some(timer) = poll.timer.take() {
                timer.clear();
            }
        });
    }

    fn schedule(self, delay_ms: u64) {
        self.stop_timer();
        if let Ok(timer) =
            set_timeout_with_handle(move || self.poll_now(), Duration::from_millis(delay_ms))
        {
            self.poll.update_value(|poll| poll.timer = Some(timer));
        }
    }

    pub(super) fn poll_now(self) {
        let Some(scope) = self.scope().filter(|_| can_poll()) else {
            return;
        };
        let Some(revision) = self
            .poll
            .try_update_value(|poll| {
                if poll.running {
                    return None;
                }
                poll.running = true;
                Some(poll.revision)
            })
            .flatten()
        else {
            return;
        };
        self.stop_timer();
        spawn_local(async move {
            let response = get::<Count>("count", &scope.1).await;
            if self.scope().as_ref() != Some(&scope) {
                return;
            }
            let current = self.poll.try_update_value(|poll| {
                poll.running = false;
                poll.revision == revision
            });
            if current == Some(false) {
                self.poll_now();
                return;
            }
            if current.is_none() {
                return;
            }
            let failed = !matches!(
                &response,
                Ok(Count {
                    freshness: Freshness::Fresh { .. }
                })
            );
            match response {
                Ok(count) => {
                    let badge = count.freshness.badge();
                    self.count.set(if self.offline.get_untracked() {
                        state::stale(badge)
                    } else {
                        badge
                    });
                    if self
                        .inbox
                        .with_untracked(|inbox| inbox.has_new_signal(&count.freshness))
                    {
                        self.inbox.update(|inbox| inbox.updates = true);
                    }
                }
                Err(error) => {
                    if !self.offline.get_untracked() {
                        self.count.set(BadgeState::Unknown);
                    }
                    if self.fence(&error) {
                        return;
                    }
                }
            }
            let delay = self.poll.try_update_value(|poll| {
                poll.failures = if failed {
                    (poll.failures + 1).min(4)
                } else {
                    0
                };
                state::poll_delay(poll.failures, js_sys::Math::random())
            });
            if let Some(delay) = delay.filter(|_| can_poll()) {
                self.schedule(delay);
            }
        });
    }

    fn refresh_count(self) {
        self.poll.update_value(|poll| poll.revision += 1);
        self.poll_now();
    }

    fn fence(self, error: &ApiError) -> bool {
        fence_session(self.session, error)
    }

    pub(super) fn load(self, more: bool) {
        let Some(scope) = self.scope() else {
            return;
        };
        let restore_item = (more && has_focus(".vk-notif-more"))
            .then(|| self.inbox.with_untracked(|inbox| inbox.items.len()));
        let Some(cursor) = self
            .inbox
            .try_update(|inbox| {
                if [inbox.first, inbox.more, inbox.mark].contains(&Load::Loading) {
                    return None;
                }
                if more {
                    let cursor = inbox.next.clone()?;
                    inbox.more = Load::Loading;
                    Some(Some(cursor))
                } else {
                    inbox.first = Load::Loading;
                    Some(None)
                }
            })
            .flatten()
        else {
            return;
        };
        let generation = self.view_generation.get_untracked();
        spawn_local(async move {
            let page = page(&scope.1, cursor.as_deref()).await;
            if !self.live(&scope, generation) {
                return;
            }
            match page {
                Ok(page) => {
                    let seen = self
                        .inbox
                        .try_update(|inbox| {
                            inbox.first = Load::Idle;
                            inbox.more = Load::Idle;
                            inbox.append(cursor.clone(), page);
                            if more {
                                return None;
                            }
                            // Reopening/refreshing sees the exact first-page snapshot, never a
                            // continuation's newer snapshot.
                            inbox.mark = Load::Idle;
                            inbox.mark_command = None;
                            let snapshot = inbox.snapshot.clone()?;
                            let command = snapshot_command(snapshot)?;
                            inbox.seen_command = Some(command.clone());
                            Some(command)
                        })
                        .flatten();
                    if let Some(command) = seen {
                        self.seen(command, generation);
                    }
                    if let Some(index) = restore_item {
                        self.restore_list_focus(scope, generation, Some(index));
                    }
                }
                Err(error) => {
                    if self.fence(&error) {
                        return;
                    }
                    self.inbox.update(|inbox| {
                        if more {
                            inbox.more = Load::Failed;
                        } else {
                            inbox.first = Load::Failed;
                        }
                    });
                }
            }
        });
    }

    fn seen(self, command: SnapshotCommand, generation: u64) {
        let Some(scope) = self.scope() else {
            return;
        };
        spawn_local(async move {
            let result = post("seen", &scope.1, &command).await;
            if self.scope().as_ref() != Some(&scope) {
                return;
            }
            match result {
                Ok(_) => {
                    if self.live(&scope, generation) {
                        self.inbox.update(|inbox| {
                            if inbox
                                .seen_command
                                .as_ref()
                                .is_some_and(|pending| pending.command_key == command.command_key)
                            {
                                inbox.seen_command = None;
                            }
                        });
                    }
                    self.refresh_count();
                }
                Err(error) => {
                    self.fence(&error);
                }
            }
        });
    }

    pub(super) fn read(self, group_id: String, observed_group_version: i64) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(command_key) = command_key() else {
            return;
        };
        // The router may unmount the surface immediately. This command owns only the observed
        // version and survives that unmount; it cannot write against another session's cookie.
        spawn_local(async move {
            let command = ReadCommand {
                command_key,
                group_id,
                observed_group_version,
            };
            let mut result = post("read", &scope.1, &command).await;
            if matches!(result, Err(ApiError::Network | ApiError::Unavailable))
                && self.scope().as_ref() == Some(&scope)
            {
                result = post("read", &scope.1, &command).await;
            }
            if let Err(error) = result
                && self.scope().as_ref() == Some(&scope)
            {
                self.fence(&error);
            }
        });
    }

    pub(super) fn mark_all(self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let restore = has_focus("#notification-read-all");
        let Some((command, cursors)) = self
            .inbox
            .try_update(|inbox| {
                if [inbox.first, inbox.more, inbox.mark].contains(&Load::Loading) {
                    return None;
                }
                let command = inbox
                    .mark_command
                    .clone()
                    .or_else(|| snapshot_command(inbox.snapshot.clone()?))?;
                inbox.mark_command = Some(command.clone());
                inbox.mark = Load::Loading;
                Some((command, inbox.pages.clone()))
            })
            .flatten()
        else {
            return;
        };
        let generation = self.view_generation.get_untracked();
        spawn_local(async move {
            let result = async {
                post("mark-all-read", &scope.1, &command).await?;
                let mut current = Vec::new();
                for cursor in cursors {
                    if !self.live(&scope, generation) {
                        return Err(ApiError::Unauthenticated);
                    }
                    current.extend(page(&scope.1, cursor.as_deref()).await?.items);
                }
                Ok(current)
            }
            .await;
            if !self.live(&scope, generation) {
                return;
            }
            match result {
                Ok(rows) => {
                    self.inbox.update(|inbox| {
                        // The receipt is snapshot-bounded, not a claim that every rendered
                        // version is read. Apply only current Gateway facts for displayed IDs.
                        let mut by_id = rows
                            .into_iter()
                            .map(|row| (row.group_id.clone(), row))
                            .collect::<std::collections::HashMap<_, _>>();
                        let updates = &mut inbox.updates;
                        inbox.items.retain_mut(|row| {
                            let Some(current) = by_id.remove(&row.group_id) else {
                                return false;
                            };
                            // A newer event stays behind “Có cập nhật mới”, even if it reused
                            // this group. Do not insert its new excerpt, actors or target.
                            if row.updated_at != current.updated_at {
                                *updates = true;
                                return false;
                            }
                            *row = current;
                            true
                        });
                        inbox.mark = Load::Idle;
                        inbox.mark_command = None;
                    });
                    self.refresh_count();
                    if restore {
                        self.restore_list_focus(scope, generation, None);
                    }
                }
                Err(error) => {
                    if !self.fence(&error) {
                        self.inbox.update(|inbox| inbox.mark = Load::Failed);
                    }
                }
            }
        });
    }
    fn restore_list_focus(self, scope: (String, String), generation: u64, index: Option<usize>) {
        request_animation_frame(move || {
            if !self.live(&scope, generation)
                || document().active_element().is_some_and(|element| {
                    !matches!(element.tag_name().as_str(), "BODY" | "HTML" | "DIALOG")
                })
            {
                return;
            }
            let Ok(Some(body)) = document()
                .query_selector("main .vk-notif-body, #notification-popover[open] .vk-notif-body")
            else {
                return;
            };
            let new_row = index.and_then(|index| {
                body.query_selector_all(".vk-notif-row__link")
                    .ok()?
                    .item(index.try_into().ok()?)?
                    .dyn_into::<web_sys::Element>()
                    .ok()
            });
            crate::shell::focus_without_scroll(new_row.or(Some(body)));
        });
    }
}

pub(super) fn fence_session(session: Session, error: &ApiError) -> bool {
    if matches!(error, ApiError::Unauthenticated) {
        session.fence_profile();
        true
    } else {
        false
    }
}

fn has_focus(selector: &str) -> bool {
    document()
        .active_element()
        .is_some_and(|element| element.closest(selector).ok().flatten().is_some())
}

fn can_poll() -> bool {
    !document().hidden() && window().navigator().on_line()
}

fn jitter(range: u64) -> u64 {
    (js_sys::Math::random() * range as f64) as u64
}

fn command_key() -> Option<String> {
    let mut bytes = [0u8; 16];
    window()
        .crypto()
        .ok()?
        .get_random_values_with_u8_array(&mut bytes)
        .ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn snapshot_command(snapshot: String) -> Option<SnapshotCommand> {
    Some(SnapshotCommand {
        command_key: command_key()?,
        up_to_snapshot: snapshot,
    })
}

pub(super) async fn get<T: serde::de::DeserializeOwned>(
    suffix: &str,
    scope: &str,
) -> Result<T, ApiError> {
    api::notification_request::<(), T>(&format!("{INBOX}/{suffix}"), scope, None).await
}

async fn post(
    suffix: &str,
    scope: &str,
    command: &impl serde::Serialize,
) -> Result<serde_json::Value, ApiError> {
    api::notification_request(&format!("{INBOX}/{suffix}"), scope, Some(command)).await
}

async fn page(scope: &str, cursor: Option<&str>) -> Result<Page, ApiError> {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("limit", "10");
    if let Some(cursor) = cursor {
        query.append_pair("cursor", cursor);
    }
    api::notification_request::<(), Page>(&format!("{INBOX}?{}", query.finish()), scope, None).await
}
