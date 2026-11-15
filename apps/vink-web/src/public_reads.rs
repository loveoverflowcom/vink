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

#[cfg(any(feature = "hydrate", test))]
use crate::api::LoadError;
use crate::contract::{Publication, WorkList};
use crate::freshness::Rejection;

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
    /// The origin answered 404 or withdrew the work: the snapshot must no longer be shown.
    pub removed: Signal<bool>,
    /// Non-retryable 4xx rejection observed while following this key (e.g. 422 cursor).
    pub rejected: Signal<Option<Rejection>>,
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

/// Call after withdrawing or hiding a work: purge its cached publication, drop its card from
/// cached searches, and wake observed owners promptly.
pub fn invalidate_withdrawn(work_id: &str) {
    #[cfg(feature = "hydrate")]
    browser::invalidate_withdrawn(work_id);
    #[cfg(not(feature = "hydrate"))]
    let _ = work_id;
}

/// Call after a confirmed publication change so observed publication/list views revalidate without
/// waiting for the 120s timer. Unlike [`invalidate_withdrawn`] no card is optimistically removed.
pub fn invalidate_publication(work_id: &str) {
    #[cfg(feature = "hydrate")]
    browser::invalidate_publication(work_id);
    #[cfg(not(feature = "hydrate"))]
    let _ = work_id;
}

/// Allocates a monotonic causal sequence token before an out-of-band origin read
/// (`/api/v1/works/{id}/reader`) so subsequent withdrawals fence stale arrivals even when
/// `performance.now()` is quantized or frozen.
#[cfg(feature = "hydrate")]
pub fn begin_external_read() -> u64 {
    browser::begin_external_read()
}

/// Records an origin-confirmed `ReaderResponse::Published` fetched client-side after
/// `started_seq`, clearing any older removal fence without bypassing newer withdrawals.
#[cfg(feature = "hydrate")]
pub fn record_fresh_publication(publication: crate::contract::Publication, started_seq: u64) {
    browser::record_fresh_publication(publication, started_seq);
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
            refreshing: Memo::new(move |_| status.read().as_ref().is_some_and(|s| s.refreshing))
                .into(),
            failed: Memo::new(move |_| status.read().as_ref().is_some_and(|s| s.failed)).into(),
            removed: Memo::new(move |_| status.read().is_none()).into(),
            rejected: Memo::new(move |_| status.read().as_ref().and_then(|s| s.rejected)).into(),
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
            rejected: Signal::stored(None),
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

#[cfg(any(feature = "hydrate", test))]
mod coordinator {
    use std::collections::BTreeMap;

    use super::{LoadError, PublicKey, Snapshot};
    use crate::freshness::{
        Applied, Load, Millis, Outcome, Policy, Status, Store, Ticket, Trigger,
    };

    pub type Reply = Result<Snapshot, LoadError>;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct WaiterId(pub u64);

    #[derive(Debug, Default)]
    pub struct Effects {
        pub replies: Vec<(WaiterId, Reply)>,
        pub fetches: Vec<(PublicKey, Ticket)>,
        pub notify_changed: bool,
        pub reschedule: bool,
    }

    pub enum LoadStart {
        Immediate(Reply),
        Wait { id: WaiterId, effects: Effects },
    }

    #[derive(Clone, Copy)]
    struct WithdrawnFence {
        at: Millis,
        seq: u64,
    }

    /// Pure browser-tab read coordinator separating waiter accounting, invalidation waking,
    /// withdrawal card stripping, and timer planning from DOM/Web APIs.
    pub struct Coordinator {
        pub store: Store<PublicKey, Snapshot>,
        waiters: BTreeMap<PublicKey, Vec<WaiterId>>,
        withdrawn: BTreeMap<String, WithdrawnFence>,
        next_waiter: u64,
        seq: u64,
    }

    impl Coordinator {
        pub fn new(policy: Policy) -> Self {
            Self {
                store: Store::new(policy),
                waiters: BTreeMap::new(),
                withdrawn: BTreeMap::new(),
                next_waiter: 1,
                seq: 0,
            }
        }

        pub fn begin_external_read(&mut self) -> u64 {
            self.seq += 1;
            self.seq
        }

        fn alloc_waiter(&mut self) -> WaiterId {
            let id = WaiterId(self.next_waiter);
            self.next_waiter += 1;
            id
        }

        fn prune_withdrawn(&mut self, at: Millis) {
            let retain = self.store.policy().retain_unobserved;
            self.withdrawn.retain(|_, f| at - f.at < retain);
            let max = self.store.policy().max_entries;
            while self.withdrawn.len() > max {
                let Some(oldest) = self
                    .withdrawn
                    .iter()
                    .min_by(|a, b| {
                        a.1.at
                            .total_cmp(&b.1.at)
                            .then_with(|| a.1.seq.cmp(&b.1.seq))
                    })
                    .map(|(k, _)| k.clone())
                else {
                    break;
                };
                self.withdrawn.remove(&oldest);
            }
        }

        pub fn load(&mut self, key: &PublicKey, at: Millis) -> LoadStart {
            match self.store.load(key, at) {
                Load::Ready(snapshot) => LoadStart::Immediate(Ok(snapshot)),
                Load::Rejected(rejection) => {
                    LoadStart::Immediate(Err(LoadError::Invalid(rejection)))
                }
                Load::Fetch(ticket) => {
                    let id = self.alloc_waiter();
                    self.waiters.entry(key.clone()).or_default().push(id);
                    LoadStart::Wait {
                        id,
                        effects: Effects {
                            fetches: vec![(key.clone(), ticket)],
                            notify_changed: true,
                            reschedule: true,
                            ..Effects::default()
                        },
                    }
                }
                Load::Join => {
                    let id = self.alloc_waiter();
                    self.waiters.entry(key.clone()).or_default().push(id);
                    LoadStart::Wait {
                        id,
                        effects: Effects::default(),
                    }
                }
            }
        }

        pub fn cancel_waiter(&mut self, key: &PublicKey, id: WaiterId) {
            if let Some(list) = self.waiters.get_mut(key) {
                list.retain(|existing| *existing != id);
                if list.is_empty() {
                    self.waiters.remove(key);
                }
            }
        }

        pub fn follow_seed(
            &mut self,
            key: PublicKey,
            mut initial: Snapshot,
            rendered_at: Millis,
            at: Millis,
        ) {
            self.prune_withdrawn(at);
            if self.store.is_removed_at(&key, rendered_at) {
                return;
            }
            let mut sanitized = false;
            if let Snapshot::Works(list) = &mut initial {
                let withdrawn: Vec<String> = self
                    .withdrawn
                    .iter()
                    .filter(|(_, f)| rendered_at <= f.at)
                    .map(|(id, _)| id.clone())
                    .collect();
                if !withdrawn.is_empty() {
                    let before = list.works.len();
                    list.works
                        .retain(|card| !withdrawn.iter().any(|id| id == &card.work_id));
                    sanitized = list.works.len() != before;
                }
            }
            self.store.seed(&key, initial, rendered_at, at);
            if sanitized {
                self.store.mutation_confirmed(&key);
            }
            self.store.observe(&key, at);
        }

        pub fn release(&mut self, key: &PublicKey, at: Millis) -> Effects {
            self.store.release(key, at);
            Effects {
                reschedule: true,
                ..Effects::default()
            }
        }

        pub fn status(&self, key: &PublicKey) -> Option<Status<Snapshot>> {
            self.store.status(key)
        }

        pub fn check(
            &mut self,
            key: &PublicKey,
            trigger: Trigger,
            at: Millis,
            visible: bool,
        ) -> Effects {
            if trigger != Trigger::Manual && !visible {
                return Effects {
                    reschedule: true,
                    ..Effects::default()
                };
            }
            if let Some(ticket) = self.store.begin(key, trigger, at) {
                Effects {
                    fetches: vec![(key.clone(), ticket)],
                    notify_changed: true,
                    reschedule: true,
                    ..Effects::default()
                }
            } else {
                Effects {
                    reschedule: true,
                    ..Effects::default()
                }
            }
        }

        fn push_waiter_replies(&mut self, key: &PublicKey, reply: &Reply, effects: &mut Effects) {
            if let Some(ids) = self.waiters.remove(key) {
                for id in ids {
                    effects.replies.push((id, reply.clone()));
                }
            }
        }

        fn wake_matching(
            &mut self,
            mut predicate: impl FnMut(&PublicKey) -> bool,
            at: Millis,
            visible: bool,
            effects: &mut Effects,
        ) {
            let waiting_keys: Vec<PublicKey> = self
                .waiters
                .keys()
                .filter(|k| predicate(k) && self.store.needs_fetch_when_waiting(k))
                .cloned()
                .collect();
            for key in waiting_keys {
                match self.store.load(&key, at) {
                    Load::Ready(snap) => self.push_waiter_replies(&key, &Ok(snap), effects),
                    Load::Rejected(rej) => {
                        self.push_waiter_replies(&key, &Err(LoadError::Invalid(rej)), effects);
                    }
                    Load::Fetch(ticket) => effects.fetches.push((key, ticket)),
                    Load::Join => {}
                }
            }
            if visible {
                let observed = self.store.matching_observed(predicate);
                for key in observed {
                    if let Some(ticket) = self.store.begin(&key, Trigger::Signal, at) {
                        effects.fetches.push((key, ticket));
                    }
                }
            }
        }

        pub fn invalidate_publication(
            &mut self,
            work_id: &str,
            at: Millis,
            visible: bool,
        ) -> Effects {
            let target = PublicKey::Publication(work_id.to_string());
            self.withdrawn.remove(work_id);
            self.store.clear_removal(&target);
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            self.store
                .mutation_confirmed_where(|k| k == &target || matches!(k, PublicKey::Works(_)));
            self.wake_matching(
                |k| k == &target || matches!(k, PublicKey::Works(_)),
                at,
                visible,
                &mut effects,
            );
            effects
        }

        pub fn record_fresh_publication(
            &mut self,
            publication: crate::contract::Publication,
            started_seq: u64,
            at: Millis,
            random: f64,
        ) -> Effects {
            self.prune_withdrawn(at);
            if self
                .withdrawn
                .get(&publication.work_id)
                .is_some_and(|w| started_seq <= w.seq)
            {
                return Effects::default();
            }
            let work_id = publication.work_id.clone();
            let key = PublicKey::Publication(work_id.clone());
            self.withdrawn.remove(&work_id);
            self.store.clear_removal(&key);
            let snapshot = Snapshot::Publication(publication);
            let applied = self
                .store
                .record_fresh(&key, snapshot.clone(), at, at, random);
            if applied == Applied::Discarded {
                return Effects::default();
            }
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            self.push_waiter_replies(&key, &Ok(snapshot), &mut effects);
            effects
        }

        fn apply_withdrawal(
            &mut self,
            work_id: &str,
            at: Millis,
            visible: bool,
            remove_entry: bool,
            effects: &mut Effects,
        ) {
            self.seq += 1;
            self.withdrawn
                .insert(work_id.to_string(), WithdrawnFence { at, seq: self.seq });
            self.prune_withdrawn(at);
            let target = PublicKey::Publication(work_id.to_string());
            if remove_entry {
                self.store.remove_at(&target, at);
            }
            self.push_waiter_replies(&target, &Err(LoadError::NotFound), effects);
            self.store.edit_all(|key, snapshot| {
                if matches!(key, PublicKey::Works(_))
                    && let Snapshot::Works(list) = snapshot
                {
                    list.works.retain(|card| card.work_id != work_id);
                }
            });
            self.store
                .mutation_confirmed_where(|k| matches!(k, PublicKey::Works(_)));
            self.wake_matching(|k| matches!(k, PublicKey::Works(_)), at, visible, effects);
        }

        pub fn withdraw_publication(
            &mut self,
            work_id: &str,
            at: Millis,
            visible: bool,
        ) -> Effects {
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            self.apply_withdrawal(work_id, at, visible, true, &mut effects);
            effects
        }

        pub fn finish(
            &mut self,
            key: PublicKey,
            ticket: Ticket,
            outcome: Outcome<Snapshot>,
            at: Millis,
            random: f64,
            visible: bool,
        ) -> Effects {
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            let applied = self.store.complete(&key, ticket, outcome, at, random);
            match applied {
                Applied::Fresh { .. } => {
                    if let PublicKey::Publication(id) = &key {
                        self.withdrawn.remove(id);
                    }
                    if let Some(reply) = self.store.status(&key).and_then(|s| s.data).map(Ok) {
                        self.push_waiter_replies(&key, &reply, &mut effects);
                    }
                }
                Applied::Removed => {
                    self.push_waiter_replies(&key, &Err(LoadError::NotFound), &mut effects);
                    if let PublicKey::Publication(id) = &key {
                        self.apply_withdrawal(id, at, visible, false, &mut effects);
                    }
                }
                Applied::Rejected(rejection) => {
                    self.push_waiter_replies(
                        &key,
                        &Err(LoadError::Invalid(rejection)),
                        &mut effects,
                    );
                }
                Applied::Failed => {
                    self.push_waiter_replies(&key, &Err(LoadError::Unavailable), &mut effects);
                }
                Applied::Discarded => {
                    if self.waiters.get(&key).is_some_and(|ws| !ws.is_empty()) {
                        match self.store.load(&key, at) {
                            Load::Ready(data) => {
                                self.push_waiter_replies(&key, &Ok(data), &mut effects);
                            }
                            Load::Rejected(rejection) => {
                                self.push_waiter_replies(
                                    &key,
                                    &Err(LoadError::Invalid(rejection)),
                                    &mut effects,
                                );
                            }
                            Load::Fetch(new_ticket) => effects.fetches.push((key, new_ticket)),
                            Load::Join => {}
                        }
                    }
                }
            }
            effects
        }

        pub fn tick(&mut self, at: Millis, visible: bool) -> Effects {
            let mut effects = Effects {
                reschedule: true,
                ..Effects::default()
            };
            if !visible {
                return effects;
            }
            for key in self.store.due(at) {
                if let Some(ticket) = self.store.begin(&key, Trigger::Due, at) {
                    effects.fetches.push((key, ticket));
                }
            }
            effects.notify_changed = !effects.fetches.is_empty();
            effects
        }

        pub fn signal_all(&mut self, at: Millis, visible: bool) -> Effects {
            let mut effects = Effects {
                reschedule: true,
                ..Effects::default()
            };
            if !visible {
                return effects;
            }
            for key in self.store.observed() {
                if let Some(ticket) = self.store.begin(&key, Trigger::Signal, at) {
                    effects.fetches.push((key, ticket));
                }
            }
            effects.notify_changed = !effects.fetches.is_empty();
            effects
        }

        #[cfg(test)]
        pub fn mark_stale_scope(
            &mut self,
            mut predicate: impl FnMut(&PublicKey) -> bool,
            at: Millis,
            visible: bool,
        ) -> Effects {
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            self.store.mark_stale_where(&mut predicate);
            self.wake_matching(predicate, at, visible, &mut effects);
            effects
        }

        #[cfg(test)]
        pub fn clear(&mut self) -> Effects {
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            let all = std::mem::take(&mut self.waiters);
            for (_, ids) in all {
                for id in ids {
                    effects.replies.push((id, Err(LoadError::Unavailable)));
                }
            }
            self.store.clear();
            self.withdrawn.clear();
            effects
        }

        #[cfg(test)]
        pub fn pending_waiter_count(&self, key: &PublicKey) -> usize {
            self.waiters.get(key).map_or(0, Vec::len)
        }
    }
}

#[cfg(feature = "hydrate")]
mod browser {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use futures_channel::oneshot;
    use leptos::prelude::*;
    use wasm_bindgen::{JsCast, closure::Closure};

    use super::coordinator::{Coordinator, Effects, LoadStart, Reply, WaiterId};
    use super::{PublicKey, Snapshot};
    use crate::api::LoadError;
    use crate::contract::{Publication, WorkList};
    use crate::freshness::{Millis, Outcome, Policy, Status, Ticket, Trigger};

    struct Owner {
        coord: Coordinator,
        channels: BTreeMap<WaiterId, oneshot::Sender<Reply>>,
        timer: Option<i32>,
        tick: Option<Closure<dyn FnMut()>>,
        listener: Option<Closure<dyn FnMut()>>,
    }

    thread_local! {
        static OWNER: RefCell<Owner> = RefCell::new(Owner {
            coord: Coordinator::new(Policy::V1),
            channels: BTreeMap::new(),
            timer: None,
            tick: None,
            listener: None,
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

    struct WaiterGuard {
        key: PublicKey,
        id: WaiterId,
        settled: bool,
    }

    impl Drop for WaiterGuard {
        fn drop(&mut self) {
            if !self.settled {
                let id = self.id;
                let key = self.key.clone();
                with(|owner| {
                    owner.channels.remove(&id);
                    owner.coord.cancel_waiter(&key, id);
                });
            }
        }
    }

    fn apply_effects(effects: Effects) {
        let deliveries: Vec<(oneshot::Sender<Reply>, Reply)> = if effects.replies.is_empty() {
            Vec::new()
        } else {
            with(|owner| {
                effects
                    .replies
                    .into_iter()
                    .filter_map(|(id, reply)| owner.channels.remove(&id).map(|tx| (tx, reply)))
                    .collect()
            })
        };
        for (tx, reply) in deliveries {
            let _ = tx.send(reply);
        }
        for (key, ticket) in effects.fetches {
            spawn_fetch(key, ticket);
        }
        if effects.notify_changed {
            changed();
        }
        if effects.reschedule {
            reschedule();
        }
    }

    pub async fn load(key: PublicKey) -> Reply {
        let at = now();
        let (receive, mut guard) = {
            let (tx, rx) = oneshot::channel();
            let effects_to_run = with(|owner| match owner.coord.load(&key, at) {
                LoadStart::Immediate(reply) => Err(reply),
                LoadStart::Wait { id, effects } => {
                    owner.channels.insert(id, tx);
                    Ok((id, effects))
                }
            });
            match effects_to_run {
                Err(reply) => return reply,
                Ok((id, effects)) => {
                    apply_effects(effects);
                    (
                        rx,
                        WaiterGuard {
                            key,
                            id,
                            settled: false,
                        },
                    )
                }
            }
        };
        let reply = receive.await.unwrap_or(Err(LoadError::Unavailable));
        guard.settled = true;
        reply
    }

    pub fn follow(key: PublicKey, initial: Snapshot) -> Memo<Option<Status<Snapshot>>> {
        let at = now();
        with(|owner| {
            owner
                .coord
                .follow_seed(key.clone(), initial, rendered_at(), at);
        });
        listen();
        // After hydration, never during it: a refresh flag set mid-hydration would disagree
        // with the server HTML.
        let mounted = key.clone();
        Effect::new(move |_| check(&mounted, Trigger::Signal));
        let released = key.clone();
        on_cleanup(move || {
            let effects = with(|owner| owner.coord.release(&released, now()));
            apply_effects(effects);
        });
        Memo::new(move |_| {
            CHANGED.with(|trigger| trigger.track());
            with(|owner| owner.coord.status(&key))
        })
    }

    pub fn check(key: &PublicKey, trigger: Trigger) {
        let effects = with(|owner| owner.coord.check(key, trigger, now(), visible()));
        apply_effects(effects);
    }

    pub fn invalidate_publication(work_id: &str) {
        let effects = with(|owner| {
            owner
                .coord
                .invalidate_publication(work_id, now(), visible())
        });
        apply_effects(effects);
    }

    pub fn invalidate_withdrawn(work_id: &str) {
        let effects = with(|owner| owner.coord.withdraw_publication(work_id, now(), visible()));
        apply_effects(effects);
    }

    pub fn begin_external_read() -> u64 {
        with(|owner| owner.coord.begin_external_read())
    }

    pub fn record_fresh_publication(publication: crate::contract::Publication, started_seq: u64) {
        let at = now();
        let random = js_sys::Math::random();
        let effects = with(|owner| {
            owner
                .coord
                .record_fresh_publication(publication, started_seq, at, random)
        });
        apply_effects(effects);
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
        let effects = with(|owner| {
            owner
                .coord
                .finish(key, ticket, outcome, at, random, visible())
        });
        apply_effects(effects);
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
            let Some(wake) = owner.coord.store.next_wake(at) else {
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
        let effects = with(|owner| {
            owner.timer = None;
            owner.coord.tick(now(), visible())
        });
        apply_effects(effects);
    }

    /// Focus, becoming visible, `online` and a back/forward-cache restore revalidate observed
    /// stale snapshots (`online` is only a hint: a failure still backs off). Hidden pauses the
    /// timer. Installed once for the tab's lifetime; views only observe/release.
    fn listen() {
        with(|owner| {
            if owner.listener.is_some() {
                return;
            }
            let signal = Closure::<dyn FnMut()>::new(|| {
                let effects = with(|owner| owner.coord.signal_all(now(), visible()));
                apply_effects(effects);
            });
            let callback = signal.as_ref().unchecked_ref();
            let _ = document().add_event_listener_with_callback("visibilitychange", callback);
            for event in ["focus", "online", "pageshow"] {
                let _ = window().add_event_listener_with_callback(event, callback);
            }
            owner.listener = Some(signal);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::coordinator::{Coordinator, LoadStart};
    use super::{LoadError, PublicKey, Snapshot};
    use crate::contract::{DocumentDto, Publication, WorkCard, WorkFilters, WorkList};
    use crate::discovery::ExploreQuery;
    use crate::freshness::{InputField, Outcome, Policy, Rejection, Trigger};

    fn sample_work(id: &str) -> WorkCard {
        WorkCard {
            work_id: id.into(),
            title: format!("Title {id}"),
            author_display_name: "Author".into(),
            published_at: "2026-11-15T00:00:00Z".into(),
            tags: vec![],
        }
    }

    fn sample_list(ids: &[&str]) -> Snapshot {
        Snapshot::Works(WorkList {
            works: ids.iter().map(|id| sample_work(id)).collect(),
            filters: WorkFilters {
                q: None,
                tags: vec![],
            },
            next_cursor: None,
        })
    }

    fn sample_pub(id: &str, rev: &str) -> Snapshot {
        Snapshot::Publication(Publication {
            work_id: id.into(),
            revision_id: rev.into(),
            title: format!("Title {id}"),
            author_display_name: "Author".into(),
            document: DocumentDto {
                schema_version: 1,
                blocks: vec![],
            },
            published_at: "2026-11-15T00:00:00Z".into(),
            tags: vec![],
        })
    }

    #[test]
    fn mutation_ack_wakes_only_observed_visible_owners_and_coalesces_signals() {
        let mut c = Coordinator::new(Policy::V1);
        let pub_k = PublicKey::Publication("w1".into());
        let observed_q = PublicKey::Works(ExploreQuery::default());
        let hidden_q = PublicKey::Works(ExploreQuery::new("mua".into(), vec![], None));

        c.follow_seed(pub_k.clone(), sample_pub("w1", "r1"), 0.0, 10.0);
        c.follow_seed(observed_q.clone(), sample_list(&["w1"]), 0.0, 10.0);
        c.store.seed(&hidden_q, sample_list(&["w1"]), 0.0, 10.0);

        // When hidden, invalidation marks stale/confirmed without spawning network traffic.
        let hidden_eff = c.invalidate_publication("w1", 20.0, false);
        assert!(hidden_eff.fetches.is_empty());

        // Becoming visible signals observed keys once, coalescing concurrent triggers.
        let vis_eff = c.signal_all(30.0, true);
        assert_eq!(vis_eff.fetches.len(), 2);
        let keys: Vec<&PublicKey> = vis_eff.fetches.iter().map(|(k, _)| k).collect();
        assert!(keys.contains(&&pub_k));
        assert!(keys.contains(&&observed_q));
        assert!(!keys.contains(&&hidden_q), "unobserved keys never poll");

        let dup = c.signal_all(31.0, true);
        assert!(dup.fetches.is_empty(), "single-flight coalesces signals");
    }

    #[test]
    fn withdraw_purges_publication_and_list_cards_and_blocks_old_seeds() {
        let mut c = Coordinator::new(Policy::V1);
        let pub_k = PublicKey::Publication("w1".into());
        let list_k = PublicKey::Works(ExploreQuery::default());

        c.follow_seed(pub_k.clone(), sample_pub("w1", "r1"), 0.0, 10.0);
        c.follow_seed(list_k.clone(), sample_list(&["w1", "w2"]), 0.0, 10.0);
        let stale_fetch = c.check(&list_k, Trigger::Manual, 20.0, true);
        assert_eq!(stale_fetch.fetches.len(), 1);
        let (_, old_ticket) = stale_fetch.fetches[0].clone();

        // Confirm withdrawal of w1 at t = 30.
        let w_eff = c.withdraw_publication("w1", 30.0, true);
        assert_eq!(c.status(&pub_k), None);
        let Snapshot::Works(list_after) = c.status(&list_k).unwrap().data.unwrap() else {
            panic!()
        };
        assert_eq!(
            list_after
                .works
                .iter()
                .map(|w| w.work_id.as_str())
                .collect::<Vec<_>>(),
            vec!["w2"]
        );
        // Observed explore owner wakes promptly to confirm with origin.
        assert_eq!(w_eff.fetches.len(), 1);
        assert_eq!(w_eff.fetches[0].0, list_k);

        // Older in-flight response for list_k (started before withdrawal) is discarded!
        let old_commit = c.finish(
            list_k.clone(),
            old_ticket,
            Outcome::Fresh(sample_list(&["w1", "w2"])),
            35.0,
            0.0,
            true,
        );
        assert!(old_commit.fetches.is_empty());
        let Snapshot::Works(still_purged) = c.status(&list_k).unwrap().data.unwrap() else {
            panic!()
        };
        assert_eq!(still_purged.works.len(), 1);

        // Stale SSR/hydration seed rendered at t=0 cannot resurrect withdrawn w1 in publication or list.
        c.follow_seed(pub_k.clone(), sample_pub("w1", "r1"), 0.0, 40.0);
        assert_eq!(c.status(&pub_k), None);
        let other_q = PublicKey::Works(ExploreQuery::new("tho".into(), vec![], None));
        c.follow_seed(other_q.clone(), sample_list(&["w1", "w3"]), 0.0, 40.0);
        let Snapshot::Works(sanitized) = c.status(&other_q).unwrap().data.unwrap() else {
            panic!()
        };
        assert_eq!(sanitized.works.len(), 1);
        assert_eq!(sanitized.works[0].work_id, "w3");

        // Re-publication recovers when a fresh load receives an authoritative 200.
        let LoadStart::Wait {
            id: waiter,
            effects,
        } = c.load(&pub_k, 50.0)
        else {
            panic!("should fetch after withdrawal, not permanent tombstone")
        };
        assert_eq!(effects.fetches.len(), 1);
        let (_, repub_ticket) = effects.fetches[0].clone();
        let done = c.finish(
            pub_k.clone(),
            repub_ticket,
            Outcome::Fresh(sample_pub("w1", "r2")),
            60.0,
            0.0,
            true,
        );
        assert_eq!(done.replies.len(), 1);
        assert_eq!(done.replies[0].0, waiter);
        assert!(matches!(done.replies[0].1, Ok(Snapshot::Publication(_))));
        assert!(c.status(&pub_k).is_some());

        // SPA client navigation (`/reader`) started before a withdrawal cannot resurrect it,
        // whereas one started afterwards clears the fence cleanly even on a frozen clock (`at == 70.0`).
        let reader_before_withdraw = c.begin_external_read();
        c.withdraw_publication("w1", 70.0, true);
        assert_eq!(c.status(&pub_k), None);
        let Snapshot::Publication(p_stale) = sample_pub("w1", "r2") else {
            panic!()
        };
        c.record_fresh_publication(p_stale, reader_before_withdraw, 75.0, 0.0);
        assert_eq!(c.status(&pub_k), None);

        let reader_after_withdraw = c.begin_external_read();
        let Snapshot::Publication(p_new) = sample_pub("w1", "r3") else {
            panic!()
        };
        c.record_fresh_publication(p_new.clone(), reader_after_withdraw, 70.0, 0.0);
        c.follow_seed(pub_k.clone(), Snapshot::Publication(p_new), 0.0, 70.0);
        assert!(matches!(
            c.status(&pub_k).and_then(|s| s.data),
            Some(Snapshot::Publication(ref p)) if p.revision_id == "r3"
        ));
    }

    #[test]
    fn pending_initial_waiters_never_hang_on_invalidate_withdraw_remove_clear_or_cancel() {
        // 1. Invalidate while initial read pending restarts waiter fetch; old response discarded.
        let mut c = Coordinator::new(Policy::V1);
        let pub_k = PublicKey::Publication("w1".into());
        let LoadStart::Wait { id: w1, effects } = c.load(&pub_k, 0.0) else {
            panic!()
        };
        let old_ticket = effects.fetches[0].1;
        let inv = c.invalidate_publication("w1", 10.0, true);
        assert_eq!(inv.fetches.len(), 1);
        let new_ticket = inv.fetches[0].1;
        assert_ne!(old_ticket, new_ticket);

        // Old flight arriving late does not resolve with stale data and does not break new flight.
        let stale_done = c.finish(
            pub_k.clone(),
            old_ticket,
            Outcome::Fresh(sample_pub("w1", "stale")),
            20.0,
            0.0,
            true,
        );
        assert!(stale_done.replies.is_empty());
        assert_eq!(c.pending_waiter_count(&pub_k), 1);

        let fresh_done = c.finish(
            pub_k.clone(),
            new_ticket,
            Outcome::Fresh(sample_pub("w1", "fresh")),
            25.0,
            0.0,
            true,
        );
        assert_eq!(fresh_done.replies.len(), 1);
        assert_eq!(fresh_done.replies[0].0, w1);
        assert_eq!(fresh_done.replies[0].1, Ok(sample_pub("w1", "fresh")));

        // 2. Withdraw / remove while initial load pending resolves waiters immediately as NotFound.
        let pub_w2 = PublicKey::Publication("w2".into());
        let LoadStart::Wait { id: w2_waiter, .. } = c.load(&pub_w2, 30.0) else {
            panic!()
        };
        let wd = c.withdraw_publication("w2", 35.0, true);
        assert_eq!(wd.replies, vec![(w2_waiter, Err(LoadError::NotFound))]);
        assert_eq!(c.pending_waiter_count(&pub_w2), 0);

        // 3. Clear while multiple initial loads pending resolves every waiter without hanging.
        let k_a = PublicKey::Works(ExploreQuery::default());
        let k_b = PublicKey::Publication("w3".into());
        let LoadStart::Wait { id: id_a, .. } = c.load(&k_a, 40.0) else {
            panic!()
        };
        let LoadStart::Wait { id: id_b, .. } = c.load(&k_b, 41.0) else {
            panic!()
        };
        let cleared = c.clear();
        assert_eq!(cleared.replies.len(), 2);
        assert!(
            cleared
                .replies
                .contains(&(id_a, Err(LoadError::Unavailable)))
        );
        assert!(
            cleared
                .replies
                .contains(&(id_b, Err(LoadError::Unavailable)))
        );

        // 4. Dropped waiter (cancel_waiter) + rejected 422 stops pending waits cleanly.
        let bad_q = PublicKey::Works(ExploreQuery::new("!".into(), vec![], Some("bad".into())));
        let LoadStart::Wait {
            id: cancel_id,
            effects,
        } = c.load(&bad_q, 50.0)
        else {
            panic!()
        };
        let t_bad = effects.fetches[0].1;
        c.cancel_waiter(&bad_q, cancel_id);
        assert_eq!(c.pending_waiter_count(&bad_q), 0);
        let rejected = c.finish(
            bad_q.clone(),
            t_bad,
            Outcome::Rejected(Rejection::RestartCursor),
            55.0,
            0.0,
            true,
        );
        assert!(rejected.replies.is_empty());
        let LoadStart::Immediate(second) = c.load(&bad_q, 60.0) else {
            panic!("cached rejection must return immediately without retry loop")
        };
        assert_eq!(second, Err(LoadError::Invalid(Rejection::RestartCursor)));

        // Verify InputField 422 maps distinctly too; mark_stale_scope also respects backoff.
        let bad_q2 = PublicKey::Works(ExploreQuery::new("!".into(), vec![], None));
        let LoadStart::Wait { id: wq, effects } = c.load(&bad_q2, 70.0) else {
            panic!()
        };
        let done_q = c.finish(
            bad_q2,
            effects.fetches[0].1,
            Outcome::Rejected(Rejection::InvalidInput(InputField::Query)),
            75.0,
            0.0,
            true,
        );
        assert_eq!(
            done_q.replies,
            vec![(
                wq,
                Err(LoadError::Invalid(Rejection::InvalidInput(
                    InputField::Query
                )))
            )]
        );

        let backoff_k = PublicKey::Publication("w4".into());
        c.follow_seed(backoff_k.clone(), sample_pub("w4", "r1"), 0.0, 80.0);
        let f1 = c.check(&backoff_k, Trigger::Manual, 85.0, true);
        c.finish(
            backoff_k.clone(),
            f1.fetches[0].1,
            Outcome::Temporary {
                retry_after: Some(30_000.0),
            },
            90.0,
            0.0,
            true,
        );
        let stale_wake = c.mark_stale_scope(|k| k == &backoff_k, 100.0, true);
        assert!(
            stale_wake.fetches.is_empty(),
            "generic stale never wakes before retry_at"
        );

        // Tick & release lifecycle: timer respects backoff deadline, wakes when due, and release pauses polling.
        assert!(c.tick(20_000.0, true).fetches.is_empty());
        let due_tick = c.tick(40_000.0, true);
        assert!(due_tick.reschedule);
        assert_eq!(due_tick.fetches.len(), 1);
        let rel = c.release(&backoff_k, 120_000.0);
        assert!(rel.reschedule);
        assert!(c.tick(500_000.0, true).fetches.is_empty());
    }
}
