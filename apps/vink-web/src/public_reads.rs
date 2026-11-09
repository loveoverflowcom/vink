//! READ-02 Web pilot: the browser tab's owner of the two public reads (`/api/v1/works` and
//! `/api/v1/works/{id}/publication`).
//!
//! One memory-only store per tab ([`crate::freshness::Store`]), one scheduler timer, one set of
//! lifecycle listeners and single-flight requests. The browser HTTP cache owns validators
//! (READ-01 ETag + `private, no-cache`); this owner only decides *when* to ask and what a view may
//! keep showing. SSR never touches it: on the server every view follows its request's own
//! in-process snapshot, so nothing crosses requests or users. Drafts, discussion and the editor
//! are outside this store.

use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use crate::api::LoadError;
use crate::contract::{Publication, WorkList};

/// What a cached representation is: the endpoint and the parameter that selects it. No
/// identity, cookie or token belongs here (public reads take none).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PublicKey {
    Works(crate::discovery::ExploreQuery),
    Publication(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Snapshot {
    Works(WorkList),
    Publication(Publication),
}

impl PublicKey {
    pub fn path(&self) -> String {
        match self {
            Self::Works(query) => query.api_path(),
            Self::Publication(id) => format!("/api/v1/works/{id}/publication"),
        }
    }
}

/// What a mounted view renders: the latest permitted snapshot and independent refresh facts.
#[derive(Clone)]
pub struct Live<T: Send + Sync + 'static> {
    pub data: Signal<T>,
    /// A check is running while `data` stays on screen.
    pub refreshing: Signal<bool>,
    /// The last check failed temporarily; `data` is the last success.
    pub failed: Signal<bool>,
    /// The origin answered 404: the snapshot must no longer be shown.
    pub removed: Signal<bool>,
    key: PublicKey,
}

impl<T: Send + Sync + 'static> Live<T> {
    /// Explicit refresh: skips the stale threshold and backoff, still one request at a time.
    pub fn refresh(&self) {
        #[cfg(feature = "hydrate")]
        browser::check(&self.key, crate::freshness::Trigger::Manual);
        #[cfg(not(feature = "hydrate"))]
        let _ = &self.key;
    }
}

pub fn follow_works(query: crate::discovery::ExploreQuery, initial: WorkList) -> Live<WorkList> {
    follow(
        PublicKey::Works(query),
        initial,
        Snapshot::Works,
        |s| match s {
            Snapshot::Works(list) => Some(list.clone()),
            Snapshot::Publication(_) => None,
        },
    )
}

pub fn follow_publication(initial: Publication) -> Live<Publication> {
    let key = PublicKey::Publication(initial.work_id.clone());
    follow(key, initial, Snapshot::Publication, |s| match s {
        Snapshot::Publication(p) => Some(p.clone()),
        Snapshot::Works(_) => None,
    })
}

fn follow<T>(
    key: PublicKey,
    initial: T,
    wrap: fn(T) -> Snapshot,
    unwrap: fn(&Snapshot) -> Option<T>,
) -> Live<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    #[cfg(feature = "hydrate")]
    {
        let status = browser::follow(key.clone(), wrap(initial.clone()));
        let data = Memo::new(move |previous: Option<&T>| {
            status
                .read()
                .as_ref()
                .and_then(|s| s.data.as_ref())
                .and_then(unwrap)
                .or_else(|| previous.cloned())
                .unwrap_or_else(|| initial.clone())
        });
        Live {
            data: data.into(),
            refreshing: Signal::derive(move || {
                status.read().as_ref().is_some_and(|s| s.refreshing)
            }),
            failed: Signal::derive(move || status.read().as_ref().is_some_and(|s| s.failed)),
            removed: Signal::derive(move || status.read().is_none()),
            key,
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (wrap, unwrap);
        Live {
            data: Signal::stored(initial),
            refreshing: Signal::stored(false),
            failed: Signal::stored(false),
            removed: Signal::stored(false),
            key,
        }
    }
}

/// The value a public `Resource` resolves to in the browser: a permitted cached snapshot at
/// once, otherwise the (shared) request's result.
#[cfg(feature = "hydrate")]
pub async fn load(key: PublicKey) -> Result<Snapshot, LoadError> {
    browser::load(key).await
}

#[cfg(feature = "hydrate")]
mod browser {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use futures_channel::oneshot;
    use leptos::prelude::*;
    use wasm_bindgen::{JsCast, closure::Closure};

    use super::{LoadError, PublicKey, Snapshot};
    use crate::contract::{Publication, WorkList};
    use crate::freshness::{
        Applied, Load, Millis, Outcome, Policy, Status, Store, Ticket, Trigger,
    };

    type Reply = Result<Snapshot, LoadError>;

    struct Owner {
        store: Store<PublicKey, Snapshot>,
        /// Mounting views with no displayable snapshot, waiting for the key's one request.
        waiters: BTreeMap<PublicKey, Vec<oneshot::Sender<Reply>>>,
        timer: Option<i32>,
        tick: Option<Closure<dyn FnMut()>>,
        listening: bool,
    }

    thread_local! {
        static OWNER: RefCell<Owner> = RefCell::new(Owner {
            store: Store::new(Policy::V1),
            waiters: BTreeMap::new(),
            timer: None,
            tick: None,
            listening: false,
        });
        static CHANGED: ArcTrigger = ArcTrigger::new();
    }

    /// Never hold the borrow across a reactive notification, a spawn or a send.
    fn with<R>(f: impl FnOnce(&mut Owner) -> R) -> R {
        OWNER.with(|owner| f(&mut owner.borrow_mut()))
    }

    fn changed() {
        CHANGED.with(|trigger| trigger.notify());
    }

    fn now() -> Millis {
        window().performance().map(|p| p.now()).unwrap_or(0.0)
    }

    fn visible() -> bool {
        document().visibility_state() != web_sys::VisibilityState::Hidden
    }

    /// Lower bound for when the server read the snapshot this page hydrated with: the
    /// navigation start (`performance.now()` = 0). A back/forward document may come from the
    /// HTTP cache with unknown age, so it is treated as stale.
    fn rendered_at() -> Millis {
        let back_forward = window()
            .performance()
            .map(|p| p.get_entries_by_type("navigation").get(0))
            .and_then(|entry| js_sys::Reflect::get(&entry, &"type".into()).ok())
            .and_then(|kind| kind.as_string())
            .is_some_and(|kind| kind == "back_forward");
        if back_forward {
            Millis::NEG_INFINITY
        } else {
            0.0
        }
    }

    pub async fn load(key: PublicKey) -> Reply {
        let (reply, receive) = oneshot::channel();
        let start = with(|owner| match owner.store.load(&key, now()) {
            Load::Ready(data) => {
                let _ = reply.send(Ok(data));
                None
            }
            Load::Fetch(ticket) => {
                owner.waiters.entry(key.clone()).or_default().push(reply);
                Some(ticket)
            }
            Load::Join => {
                owner.waiters.entry(key.clone()).or_default().push(reply);
                None
            }
        });
        if let Some(ticket) = start {
            spawn_fetch(key, ticket);
        }
        receive.await.unwrap_or(Err(LoadError::Unavailable))
    }

    pub fn follow(key: PublicKey, initial: Snapshot) -> Memo<Option<Status<Snapshot>>> {
        let at = now();
        with(|owner| {
            owner.store.seed(&key, initial, rendered_at(), at);
            owner.store.observe(&key, at);
        });
        listen();
        // After hydration, never during it: a refresh flag set mid-hydration would disagree
        // with the server HTML.
        let mounted = key.clone();
        Effect::new(move |_| check(&mounted, Trigger::Signal));
        let released = key.clone();
        on_cleanup(move || {
            with(|owner| owner.store.release(&released, now()));
            reschedule();
        });
        Memo::new(move |_| {
            CHANGED.with(|trigger| trigger.track());
            with(|owner| owner.store.status(&key))
        })
    }

    pub fn check(key: &PublicKey, trigger: Trigger) {
        if trigger != Trigger::Manual && !visible() {
            return;
        }
        let ticket = with(|owner| owner.store.begin(key, trigger, now()));
        if let Some(ticket) = ticket {
            spawn_fetch(key.clone(), ticket);
            changed();
        }
        reschedule();
    }

    fn spawn_fetch(key: PublicKey, ticket: Ticket) {
        leptos::task::spawn_local(async move {
            let outcome = match &key {
                PublicKey::Works(_) => crate::api::fetch_public::<WorkList>(&key.path())
                    .await
                    .map(Snapshot::Works),
                PublicKey::Publication(_) => crate::api::fetch_public::<Publication>(&key.path())
                    .await
                    .map(Snapshot::Publication),
            };
            finish(key, ticket, outcome);
        });
    }

    fn finish(key: PublicKey, ticket: Ticket, outcome: Outcome<Snapshot>) {
        let at = now();
        let random = js_sys::Math::random();
        let (replies, restart) = with(|owner| {
            let applied = owner.store.complete(&key, ticket, outcome, at, random);
            if let (Applied::Removed, PublicKey::Publication(id)) = (applied, &key) {
                // The work's card is a derived preview of what the origin just removed.
                owner
                    .store
                    .invalidate_where(|key| matches!(key, PublicKey::Works(_)));
                owner.store.edit_all(|key, snapshot| {
                    if matches!(key, PublicKey::Works(_))
                        && let Snapshot::Works(list) = snapshot
                    {
                        list.works.retain(|card| &card.work_id != id);
                    }
                });
            }
            let reply = match applied {
                Applied::Fresh { .. } => owner.store.status(&key).and_then(|s| s.data).map(Ok),
                Applied::Removed => Some(Err(LoadError::NotFound)),
                Applied::Failed => Some(Err(LoadError::Unavailable)),
                Applied::Discarded => None,
            };
            if let Some(reply) = reply {
                let waiting = owner.waiters.remove(&key).unwrap_or_default();
                return (
                    waiting.into_iter().map(|w| (w, reply.clone())).collect(),
                    None,
                );
            }
            // Superseded while views wait: give them a request of their own.
            if !owner.waiters.contains_key(&key) {
                return (Vec::new(), None);
            }
            match owner.store.load(&key, at) {
                Load::Fetch(ticket) => (Vec::new(), Some(ticket)),
                Load::Join => (Vec::new(), None),
                Load::Ready(data) => {
                    let waiting = owner.waiters.remove(&key).unwrap_or_default();
                    (
                        waiting.into_iter().map(|w| (w, Ok(data.clone()))).collect(),
                        None,
                    )
                }
            }
        });
        for (waiter, reply) in replies {
            let _ = waiter.send(reply);
        }
        if let Some(ticket) = restart {
            spawn_fetch(key, ticket);
        }
        changed();
        reschedule();
    }

    /// The single scheduler timer: armed only while visible and something observed can refresh.
    fn reschedule() {
        let at = now();
        let visible = visible();
        with(|owner| {
            if let Some(timer) = owner.timer.take() {
                window().clear_timeout_with_handle(timer);
            }
            if !visible {
                return;
            }
            let Some(wake) = owner.store.next_wake(at) else {
                return;
            };
            let tick = owner
                .tick
                .get_or_insert_with(|| Closure::<dyn FnMut()>::new(tick));
            let delay = (wake - at).ceil().clamp(0.0, f64::from(i32::MAX)) as i32;
            owner.timer = window()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    tick.as_ref().unchecked_ref(),
                    delay,
                )
                .ok();
        });
    }

    fn tick() {
        with(|owner| owner.timer = None);
        if visible() {
            let at = now();
            for key in with(|owner| owner.store.due(at)) {
                check(&key, Trigger::Due);
            }
        }
        reschedule();
    }

    /// Focus, becoming visible, `online` and a back/forward-cache restore revalidate observed
    /// stale snapshots (`online` is only a hint: a failure still backs off). Hidden pauses the
    /// timer. Installed once for the tab's lifetime; views only observe/release.
    fn listen() {
        if with(|owner| std::mem::replace(&mut owner.listening, true)) {
            return;
        }
        let signal = Closure::<dyn FnMut()>::new(|| {
            if visible() {
                for key in with(|owner| owner.store.observed()) {
                    check(&key, Trigger::Signal);
                }
            }
            reschedule();
        });
        let callback = signal.as_ref().unchecked_ref();
        let _ = document().add_event_listener_with_callback("visibilitychange", callback);
        for event in ["focus", "online", "pageshow"] {
            let _ = window().add_event_listener_with_callback(event, callback);
        }
        signal.forget();
    }
}
