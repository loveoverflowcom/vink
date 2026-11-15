//! READ-02 Web pilot: the browser tab's owner of the two public reads (`/api/v1/works` and
//! `/api/v1/works/{id}/reader`).
//!
//! One read coordinator per tab with optional snapshot retention, one scheduler timer, one set of
//! lifecycle listeners and single-flight requests. The browser HTTP cache owns validators
//! (READ-01 ETag + `private, no-cache`); this owner only decides *when* to ask and what a view may
//! keep showing. SSR never touches it: on the server every view follows its request's own
//! in-process snapshot, so nothing crosses requests or users. Drafts, discussion and the editor
//! are outside this store.

use leptos::prelude::*;

#[cfg(any(feature = "hydrate", test))]
use crate::api::LoadError;
use crate::contract::{Publication, ReaderResponse, WorkList};
use crate::freshness::Rejection;

/// What a cached representation is: the endpoint and the parameter that selects it. No
/// identity, cookie or token belongs here (public reads take none).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum PublicKey {
    Works(crate::discovery::ExploreQuery),
    Reader(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Snapshot {
    Works(WorkList),
    Publication(Publication),
    /// Live control fact only; never retained by SnapshotCache.
    Control(ReaderResponse),
}

impl PublicKey {
    pub fn path(&self) -> String {
        match self {
            Self::Works(query) => query.api_path(),
            Self::Reader(id) => format!("/api/v1/works/{id}/reader"),
        }
    }
}

/// What a mounted view renders: the latest permitted snapshot and independent refresh facts.
#[derive(Clone)]
pub struct Live<T: Send + Sync + 'static> {
    pub data: Signal<T>,
    pub cached_unverified: Signal<bool>,
    /// A check is running while `data` stays on screen.
    pub refreshing: Signal<bool>,
    /// The last check failed temporarily; `data` is the last success.
    pub failed: Signal<bool>,
    /// The origin answered 404 or withdrew the work: the snapshot must no longer be shown.
    pub removed: Signal<bool>,
    /// Non-retryable 4xx rejection observed while following this key (e.g. 422 cursor).
    pub rejected: Signal<Option<Rejection>>,
    pub(crate) key: PublicKey,
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
            Snapshot::Publication(_) | Snapshot::Control(_) => None,
        },
    )
}

fn reader_snapshot(reader: ReaderResponse) -> Snapshot {
    match reader {
        ReaderResponse::Published(p) => Snapshot::Publication(p),
        control => Snapshot::Control(control),
    }
}
fn snapshot_reader(snapshot: &Snapshot) -> Option<ReaderResponse> {
    match snapshot {
        Snapshot::Publication(p) => Some(ReaderResponse::Published(p.clone())),
        Snapshot::Control(control) => Some(control.clone()),
        Snapshot::Works(_) => None,
    }
}
pub fn follow_reader(work_id: String, initial: ReaderResponse) -> Live<ReaderResponse> {
    follow(
        PublicKey::Reader(work_id),
        initial,
        reader_snapshot,
        snapshot_reader,
    )
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

/// Reserved for a policy-approved public-device-cache control; no private storage is touched.
/// The boolean acknowledges only durable completion. No UI claims a save/clear yet.
#[cfg(feature = "hydrate")]
pub async fn clear_device_cache() -> bool {
    browser::clear_device_cache().await
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
        let cached_key = key.clone();
        Live {
            data: data.into(),
            cached_unverified: Memo::new(move |_| {
                status.track();
                browser::cached_unverified(&cached_key)
            })
            .into(),
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
            cached_unverified: Signal::stored(false),
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
    use crate::read_cache::{InMemoryCache, SnapshotCache};

    fn valid_snapshot(key: &PublicKey, snapshot: &Snapshot) -> bool {
        match (key, snapshot) {
            (PublicKey::Works(_), Snapshot::Works(list)) => {
                let mut ids = std::collections::BTreeSet::new();
                list.works.iter().all(|w| ids.insert(&w.work_id))
            }
            (PublicKey::Reader(id), Snapshot::Publication(p)) => id == &p.work_id,
            (
                PublicKey::Reader(id),
                Snapshot::Control(
                    crate::contract::ReaderResponse::Hidden { work_id }
                    | crate::contract::ReaderResponse::ReviewWithdrawn { work_id, .. },
                ),
            ) => id == work_id,
            _ => false,
        }
    }
    pub type Reply = Result<Snapshot, LoadError>;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct WaiterId(pub u64);

    #[derive(Debug, Default)]
    pub struct Effects {
        pub replies: Vec<(WaiterId, Reply)>,
        pub fetches: Vec<(PublicKey, Ticket)>,
        pub notify_changed: bool,
        pub reschedule: bool,
        pub purges: Vec<String>,
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
    pub struct Coordinator<C = InMemoryCache<PublicKey, Snapshot>> {
        pub store: Store<PublicKey, Snapshot, C>,
        waiters: BTreeMap<PublicKey, Vec<WaiterId>>,
        withdrawn: BTreeMap<String, WithdrawnFence>,
        next_waiter: u64,
        seq: u64,
        seed_barrier: Millis,
    }

    impl Coordinator {
        pub fn new(policy: Policy) -> Self {
            Self::with_cache(policy, InMemoryCache::default())
        }
    }
    impl<C: SnapshotCache<PublicKey, Snapshot>> Coordinator<C> {
        pub fn with_cache(policy: Policy, cache: C) -> Self {
            Self {
                store: Store::with_cache(policy, cache, |s| !matches!(s, Snapshot::Control(_))),
                waiters: BTreeMap::new(),
                withdrawn: BTreeMap::new(),
                next_waiter: 1,
                seq: 0,
                seed_barrier: Millis::NEG_INFINITY,
            }
        }

        #[cfg(test)]
        pub fn begin_reader_read(&mut self, key: &PublicKey, at: Millis) -> Ticket {
            self.store.begin_external_read(key, at)
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
                    if self.store.matching_observed(|k| k == key).is_empty() {
                        self.store.mark_stale(key);
                    }
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
            if (self.seed_barrier.is_finite() && rendered_at <= self.seed_barrier)
                || self.store.is_removed_at(&key, rendered_at)
                || matches!((&key, &initial), (PublicKey::Reader(id), Snapshot::Publication(_))
                    if self.withdrawn.get(id).is_some_and(|f| rendered_at <= f.at))
            {
                if self.seed_barrier.is_finite()
                    && rendered_at <= self.seed_barrier
                    && !self.store.is_removed_at(&key, rendered_at)
                    && self.store.status(&key).is_none_or(|s| s.data.is_none())
                {
                    self.store.remove_at(&key, at);
                }
                self.store.observe(&key, at);
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
            let mut effects = Effects {
                reschedule: true,
                ..Effects::default()
            };
            // A remaining Resource waiter is still an observer in the logical sense.
            // Restart a superseded flight immediately instead of waiting for its late completion.
            self.wake_matching(|k| k == key, at, false, &mut effects);
            effects
        }

        pub fn status(&self, key: &PublicKey) -> Option<Status<Snapshot>> {
            // Starting a new request creates an empty entry; it does not restore removed text.
            if self.store.is_removed_at(key, Millis::NEG_INFINITY) {
                return None;
            }
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
            let target = PublicKey::Reader(work_id.to_string());
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

        #[cfg(test)]
        pub fn finish_reader(
            &mut self,
            key: PublicKey,
            ticket: Ticket,
            result: Result<crate::contract::ReaderResponse, LoadError>,
            at: Millis,
            random: f64,
            visible: bool,
        ) -> (Result<crate::contract::ReaderResponse, LoadError>, Effects) {
            use crate::contract::ReaderResponse;
            // Gate the resource result as well as the cache: a discarded body must never seed a view.
            if !self.store.is_current(&key, ticket) {
                return (Err(LoadError::Unavailable), Effects::default());
            }
            let outcome = match &result {
                Ok(ReaderResponse::Published(publication)) => {
                    Outcome::Fresh(Snapshot::Publication(publication.clone()))
                }
                Ok(
                    control @ (ReaderResponse::Hidden { .. }
                    | ReaderResponse::ReviewWithdrawn { .. }),
                ) => Outcome::Fresh(Snapshot::Control(control.clone())),
                Err(LoadError::NotFound) => Outcome::Removed,
                Err(LoadError::Invalid(rejection)) => Outcome::Rejected(*rejection),
                Err(LoadError::Unavailable) => Outcome::Temporary { retry_after: None },
            };
            let effects = self.finish(key, ticket, outcome, at, random, visible);
            (result, effects)
        }

        fn apply_withdrawal(
            &mut self,
            work_id: &str,
            at: Millis,
            visible: bool,
            remove_entry: bool,
            effects: &mut Effects,
        ) {
            effects.purges.push(work_id.to_owned());
            self.seq += 1;
            self.withdrawn
                .insert(work_id.to_string(), WithdrawnFence { at, seq: self.seq });
            self.prune_withdrawn(at);
            let target = PublicKey::Reader(work_id.to_string());
            if remove_entry {
                self.store.remove_at(&target, at);
            }
            if remove_entry || self.store.status(&target).is_none() {
                self.push_waiter_replies(&target, &Err(LoadError::NotFound), effects);
            }
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
            let target = PublicKey::Reader(work_id.to_owned());
            let observers = self.store.observer_count(&target);
            self.apply_withdrawal(work_id, at, visible, true, &mut effects);
            // A confirmed local withdrawal revokes text immediately, then asks the reader
            // endpoint for its typed control state. Hidden tabs defer this read until visible.
            for _ in 0..observers {
                self.store.observe(&target, at);
            }
            if observers > 0 {
                self.store.mutation_confirmed(&target);
                if visible && let Some(ticket) = self.store.begin(&target, Trigger::Signal, at) {
                    effects.fetches.push((target, ticket));
                }
            }
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
            let outcome = match outcome {
                Outcome::Fresh(snapshot) if !valid_snapshot(&key, &snapshot) => {
                    Outcome::Temporary { retry_after: None }
                }
                other => other,
            };
            let applied = self.store.complete(&key, ticket, outcome, at, random);
            match applied {
                Applied::Fresh { .. } => {
                    if let PublicKey::Reader(id) = &key {
                        if self
                            .store
                            .status(&key)
                            .is_some_and(|s| matches!(s.data, Some(Snapshot::Control(_))))
                        {
                            self.apply_withdrawal(id, at, visible, false, &mut effects);
                        } else {
                            self.withdrawn.remove(id);
                        }
                    }
                    if let Some(reply) = self.store.status(&key).and_then(|s| s.data).map(Ok) {
                        self.push_waiter_replies(&key, &reply, &mut effects);
                    }
                }
                Applied::Removed => {
                    self.push_waiter_replies(&key, &Err(LoadError::NotFound), &mut effects);
                    if let PublicKey::Reader(id) = &key {
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

        /// Same owner accepts asynchronous restoration, with an independent local ticket.
        pub fn restore(
            &mut self,
            key: &PublicKey,
            ticket: Ticket,
            snapshot: Snapshot,
            age: Millis,
            max_age: Option<Millis>,
            at: Millis,
        ) -> Effects {
            let mut effects = Effects::default();
            if valid_snapshot(key, &snapshot)
                && self
                    .store
                    .restore(key, ticket, snapshot.clone(), age, max_age, at)
            {
                self.push_waiter_replies(key, &Ok(snapshot), &mut effects);
                effects.notify_changed = true;
                effects.reschedule = true;
            }
            effects
        }

        /// A durable epoch changed (including a missed broadcast discovered on resume).
        /// Drop RAM before the next read; preserve mounted observers and supersede SSR seeds.
        pub fn storage_barrier(&mut self, at: Millis, visible: bool) -> Effects {
            let observed: Vec<_> = self
                .store
                .observed()
                .into_iter()
                .map(|key| {
                    let n = self.store.observer_count(&key);
                    (key, n)
                })
                .collect();
            let mut effects = self.clear();
            self.seed_barrier = at;
            for (key, n) in observed {
                self.store.remove_at(&key, at);
                for _ in 0..n {
                    self.store.observe(&key, at);
                }
                self.store.mutation_confirmed(&key);
            }
            self.wake_matching(|_| true, at, visible, &mut effects);
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
    use crate::contract::WorkList;
    use crate::freshness::{Millis, Outcome, Policy, Status, Ticket, Trigger};

    struct Owner {
        coord: Coordinator,
        channels: BTreeMap<WaiterId, oneshot::Sender<Reply>>,
        timer: Option<i32>,
        tick: Option<Closure<dyn FnMut()>>,
        listener: Option<u64>,
        storage_lease: Option<crate::public_persistence::browser::Lease>,
    }

    thread_local! {
        static OWNER: RefCell<Owner> = RefCell::new(Owner {
            coord: Coordinator::new(Policy::V1),
            channels: BTreeMap::new(),
            timer: None,
            tick: None,
            listener: None,
            storage_lease: None,
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
        for id in &effects.purges {
            let id = id.clone();
            leptos::task::spawn_local(async move {
                let _ = crate::public_persistence::browser::barrier(Some(id)).await;
            });
        }
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
        synchronize_storage().await;
        let at = now();
        if crate::public_persistence::ENABLED {
            let restore = with(|o| o.coord.store.begin_restore(&key, at));
            if let Some(ticket) = restore {
                spawn_restore(key.clone(), ticket);
            }
        }
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
                    let cancelled_key = key.clone();
                    on_cleanup(move || {
                        with(|owner| {
                            owner.channels.remove(&id);
                            owner.coord.cancel_waiter(&cancelled_key, id);
                        });
                    });
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

    pub fn cached_unverified(key: &PublicKey) -> bool {
        with(|o| o.coord.store.cached_unverified(key))
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

    fn spawn_fetch(key: PublicKey, ticket: Ticket) {
        leptos::task::spawn_local(async move {
            let mut lease = crate::public_persistence::browser::capture().await;
            let started_wall = js_sys::Date::now() as u64;
            let outcome = match &key {
                PublicKey::Works(_) => crate::api::fetch_public::<WorkList>(&key.path())
                    .await
                    .map(Snapshot::Works),
                PublicKey::Reader(_) => {
                    let outcome =
                        crate::api::fetch_public::<crate::contract::ReaderResponse>(&key.path())
                            .await;
                    outcome.map(super::reader_snapshot)
                }
            };
            if let Some(current_lease) = &lease {
                match crate::public_persistence::browser::current(current_lease).await {
                    Some(false) => {
                        let effects = with(|o| o.coord.storage_barrier(now(), visible()));
                        apply_effects(effects);
                        return;
                    }
                    None => lease = None, // storage failure cannot revoke an online response
                    Some(true) => {}
                }
            }
            // Keep storage writes below the authority fence. Clone only the accepted public
            // representation; no private API, no SSR seed and no control response is admitted.
            let persist = with(|o| o.coord.store.is_current(&key, ticket))
                .then(|| match &outcome {
                    Outcome::Fresh(snapshot) => crate::public_persistence::Envelope::from_origin(
                        key.clone(),
                        snapshot,
                        window().location().origin().unwrap_or_default(),
                        started_wall,
                    ),
                    _ => None,
                })
                .flatten();
            finish(key, ticket, outcome);
            if let (Some(lease), Some(envelope)) = (lease, persist) {
                let _ = crate::public_persistence::browser::write(&lease, &envelope).await;
            }
        });
    }

    async fn synchronize_storage() {
        if !crate::public_persistence::ENABLED {
            return;
        }
        let previous = with(|o| o.storage_lease.clone());
        if let Some(previous) = previous {
            match crate::public_persistence::browser::current(&previous).await {
                Some(false) => {
                    let effects = with(|o| {
                        o.storage_lease = None;
                        o.coord.storage_barrier(now(), visible())
                    });
                    apply_effects(effects);
                }
                // Keep the previous lease on failure for the next durable check. Replacing
                // it with a newly captured epoch here could silently miss a concurrent clear.
                Some(true) | None => return,
            }
        }
        let lease = crate::public_persistence::browser::capture().await;
        with(|o| {
            // Another lifecycle task may have installed a lease while this capture awaited.
            // Keep that reference so an epoch change cannot be silently acknowledged here.
            if o.storage_lease.is_none() {
                o.storage_lease = lease;
            }
        });
    }

    pub async fn clear_device_cache() -> bool {
        let effects = with(|o| o.coord.storage_barrier(now(), visible()));
        apply_effects(effects);
        crate::public_persistence::browser::barrier(None).await
    }

    fn spawn_restore(key: PublicKey, ticket: Ticket) {
        leptos::task::spawn_local(async move {
            let Some(lease) = crate::public_persistence::browser::capture().await else {
                return;
            };
            let origin = window().location().origin().unwrap_or_default();
            let crate::public_persistence::browser::RestoreRead::Hit(envelope) =
                crate::public_persistence::browser::read(&lease, &key, &origin).await
            else {
                return;
            };
            let Some(age) = envelope.age_ms(js_sys::Date::now() as u64) else {
                return;
            };
            if crate::public_persistence::browser::current(&lease).await != Some(true) {
                return;
            }
            // Display policy intentionally absent until POLICY-01/02 exact-revision signoff.
            let effects = with(|o| {
                o.coord
                    .restore(&key, ticket, envelope.snapshot(), age as f64, None, now())
            });
            apply_effects(effects);
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
            crate::public_persistence::browser::listen(|| {
                leptos::task::spawn_local(synchronize_storage());
            });
            leptos::task::spawn_local(synchronize_storage());
            let signal = crate::read_events::register(|| {
                leptos::task::spawn_local(synchronize_storage());
                let effects = with(|owner| owner.coord.signal_all(now(), visible()));
                apply_effects(effects);
            });
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
    use crate::read_cache::{InMemoryCache, NoOpCache, SnapshotCache};

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
    fn restore_suite_memory_and_noop_keep_owner_authority() {
        fn run<C: SnapshotCache<PublicKey, Snapshot>>(cache: C) {
            let mut c = Coordinator::with_cache(Policy::V1, cache);
            let k = PublicKey::Reader("w".into());
            let r = c.store.begin_restore(&k, 1.0).unwrap();
            let LoadStart::Wait { effects, .. } = c.load(&k, 2.0) else {
                panic!()
            };
            let network = effects.fetches[0].1;
            c.restore(&k, r, sample_pub("w", "cached"), 200.0, Some(1000.0), 3.0);
            assert!(c.store.cached_unverified(&k));
            c.finish(
                k.clone(),
                network,
                Outcome::Fresh(sample_pub("w", "new")),
                4.0,
                0.0,
                false,
            );
            assert!(!c.store.cached_unverified(&k));
            assert!(
                c.restore(&k, r, sample_pub("w", "old"), 1.0, Some(1000.0), 5.0)
                    .replies
                    .is_empty()
            );
            assert_eq!(c.status(&k).unwrap().data, Some(sample_pub("w", "new")));
            c.storage_barrier(6.0, false);
            c.follow_seed(k.clone(), sample_pub("w", "ssr"), 0.0, 7.0);
            assert!(c.status(&k).is_none());
            c.restore(&k, r, sample_pub("w", "old"), 1.0, Some(1000.0), 8.0);
            assert!(c.status(&k).is_none());
        }
        run(InMemoryCache::default());
        run(NoOpCache);
    }

    #[test]
    fn pending_restore_loses_to_ssr_network_removal_scope_and_clear() {
        for event in 0..6 {
            let mut c = Coordinator::new(Policy::V1);
            let k = PublicKey::Reader("w".into());
            let r = c.store.begin_restore(&k, 0.0).unwrap();
            match event {
                0 => c.follow_seed(k.clone(), sample_pub("w", "ssr"), 1.0, 1.0),
                1 => {
                    let t = c.store.begin_external_read(&k, 1.0);
                    c.finish(
                        k.clone(),
                        t,
                        Outcome::Fresh(sample_pub("w", "net")),
                        2.0,
                        0.0,
                        false,
                    );
                }
                2 => {
                    c.withdraw_publication("w", 2.0, false);
                }
                3 => {
                    c.store.mark_stale(&k);
                }
                4 => {
                    c.clear();
                }
                _ => {
                    c.storage_barrier(2.0, false);
                }
            }
            let before = c.status(&k);
            c.restore(&k, r, sample_pub("w", "cached"), 10.0, Some(100.0), 3.0);
            assert_eq!(c.status(&k), before, "event {event}");
        }
        let mut c = Coordinator::new(Policy::V1);
        let k = PublicKey::Reader("w".into());
        let r = c.store.begin_restore(&k, 0.0).unwrap();
        c.restore(&k, r, sample_pub("w", "cached"), 10.0, None, 3.0);
        assert!(c.status(&k).unwrap().data.is_none(), "policy gate");
        c.restore(&k, r, sample_pub("w", "cached"), 100.0, Some(100.0), 3.0);
        assert!(c.status(&k).unwrap().data.is_none(), "expired");
    }

    #[test]
    fn mutation_ack_wakes_only_observed_visible_owners_and_coalesces_signals() {
        let mut c = Coordinator::new(Policy::V1);
        let pub_k = PublicKey::Reader("w1".into());
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
        let pub_k = PublicKey::Reader("w1".into());
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
        assert_eq!(w_eff.fetches.len(), 2);
        assert!(w_eff.fetches.iter().any(|(key, _)| key == &list_k));
        assert!(w_eff.fetches.iter().any(|(key, _)| key == &pub_k));

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
        assert!(
            effects.fetches.is_empty(),
            "joins the control verification already started by withdrawal"
        );
        let repub_ticket = w_eff
            .fetches
            .iter()
            .find(|(key, _)| key == &pub_k)
            .unwrap()
            .1;
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
    }

    fn reader_publication(revision: &str) -> crate::contract::ReaderResponse {
        let Snapshot::Publication(publication) = sample_pub("w1", revision) else {
            panic!()
        };
        crate::contract::ReaderResponse::Published(publication)
    }

    #[test]
    fn reader_ticket_survives_republication_and_private_pointer_invalidation() {
        for republish in [false, true] {
            let mut c = Coordinator::new(Policy::V1);
            let key = PublicKey::Reader("w1".into());
            let old = c.begin_reader_read(&key, 70.0);
            c.withdraw_publication("w1", 70.0, true);
            // The existing pointer observer signals both Some -> None and publication changes.
            c.invalidate_publication("w1", 70.0, true);
            if republish {
                let new = c.begin_reader_read(&key, 70.0);
                assert_eq!(
                    c.status(&key),
                    None,
                    "pending re-publication must not expose the removed view"
                );
                let (reply, _) = c.finish_reader(
                    key.clone(),
                    new,
                    Ok(reader_publication("r3")),
                    70.0,
                    0.0,
                    true,
                );
                assert_eq!(reply, Ok(reader_publication("r3")));
            }
            // A frozen clock must not let the old read win after the new fence was cleared.
            let (reply, _) = c.finish_reader(
                key.clone(),
                old,
                Ok(reader_publication("r2")),
                70.0,
                0.0,
                true,
            );
            assert_eq!(reply, Err(LoadError::Unavailable));
            if republish {
                assert_eq!(c.status(&key).unwrap().data, Some(sample_pub("w1", "r3")));
            } else {
                c.follow_seed(key.clone(), sample_pub("w1", "r2"), 0.0, 70.0);
                assert_eq!(c.status(&key), None);
            }
        }
    }

    #[test]
    fn reader_ticket_orders_cache_refreshes_and_records_request_start() {
        let mut c = Coordinator::new(Policy::V1);
        let key = PublicKey::Reader("w1".into());
        let first = c.begin_reader_read(&key, 0.0);
        let second = c.begin_reader_read(&key, 0.0);
        let (reply, _) = c.finish_reader(
            key.clone(),
            second,
            Ok(reader_publication("r2")),
            30_000.0,
            0.0,
            true,
        );
        assert_eq!(reply, Ok(reader_publication("r2")));
        c.follow_seed(key.clone(), sample_pub("w1", "r2"), 0.0, 30_000.0);
        assert_eq!(c.store.next_wake(30_000.0), Some(120_000.0));
        let (reply, _) = c.finish_reader(
            key.clone(),
            first,
            Ok(reader_publication("r1")),
            30_001.0,
            0.0,
            true,
        );
        assert_eq!(reply, Err(LoadError::Unavailable));
        assert_eq!(c.status(&key).unwrap().data, Some(sample_pub("w1", "r2")));

        let pending = c.begin_reader_read(&key, 40_000.0);
        c.invalidate_publication("w1", 40_001.0, true);
        let (reply, _) = c.finish_reader(
            key.clone(),
            pending,
            Ok(reader_publication("old")),
            40_002.0,
            0.0,
            true,
        );
        assert_eq!(reply, Err(LoadError::Unavailable));
        assert!(
            c.store.has_in_flight(&key),
            "discarded reader must preserve the newer scheduled flight"
        );
    }

    #[test]
    fn cancelled_reader_settles_joined_waiter_and_late_denial_cannot_hide_newer_read() {
        let mut c = Coordinator::new(Policy::V1);
        let key = PublicKey::Reader("w1".into());
        let cancelled = c.begin_reader_read(&key, 0.0);
        let LoadStart::Wait { id, .. } = c.load(&key, 1.0) else {
            panic!()
        };
        let (_, effects) = c.finish_reader(
            key.clone(),
            cancelled,
            Err(LoadError::Unavailable),
            2.0,
            0.0,
            true,
        );
        assert_eq!(effects.replies, vec![(id, Err(LoadError::Unavailable))]);
        assert!(!c.store.has_in_flight(&key));
        let stale = c.begin_reader_read(&key, 3.0);
        let fresh = c.begin_reader_read(&key, 3.0);
        let (reply, _) = c.finish_reader(
            key.clone(),
            fresh,
            Ok(reader_publication("r3")),
            4.0,
            0.0,
            true,
        );
        assert_eq!(reply, Ok(reader_publication("r3")));
        let (reply, _) =
            c.finish_reader(key.clone(), stale, Err(LoadError::NotFound), 5.0, 0.0, true);
        assert_eq!(reply, Err(LoadError::Unavailable));
        assert_eq!(c.status(&key).unwrap().data, Some(sample_pub("w1", "r3")));
    }

    #[test]
    fn pending_initial_waiters_never_hang_on_invalidate_withdraw_remove_clear_or_cancel() {
        // 1. Invalidate while initial read pending restarts waiter fetch; old response discarded.
        let mut c = Coordinator::new(Policy::V1);
        let pub_k = PublicKey::Reader("w1".into());
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
        let pub_w2 = PublicKey::Reader("w2".into());
        let LoadStart::Wait { id: w2_waiter, .. } = c.load(&pub_w2, 30.0) else {
            panic!()
        };
        let wd = c.withdraw_publication("w2", 35.0, true);
        assert_eq!(wd.replies, vec![(w2_waiter, Err(LoadError::NotFound))]);
        assert_eq!(c.pending_waiter_count(&pub_w2), 0);

        // 3. Clear while multiple initial loads pending resolves every waiter without hanging.
        let k_a = PublicKey::Works(ExploreQuery::default());
        let k_b = PublicKey::Reader("w3".into());
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
        let LoadStart::Wait { effects, .. } = c.load(&bad_q, 60.0) else {
            panic!("cancelled owner cannot commit a rejection into a new visit")
        };
        assert_eq!(effects.fetches.len(), 1);
        c.finish(
            bad_q.clone(),
            effects.fetches[0].1,
            Outcome::Rejected(Rejection::RestartCursor),
            61.0,
            0.0,
            true,
        );
        assert!(matches!(
            c.load(&bad_q, 62.0),
            LoadStart::Immediate(Err(LoadError::Invalid(Rejection::RestartCursor)))
        ));

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

        let backoff_k = PublicKey::Reader("w4".into());
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
    fn conformance(cache: impl SnapshotCache<PublicKey, Snapshot>) {
        // Page-chain correctness uses the same primitive for either storage adapter.
        use crate::read_lifecycle::{Intent, PageChain, ReadOwner};
        let mut reads = ReadOwner::new("desk:a/1:q:sort:limit");
        let mut chain = PageChain::default();
        let head = reads.begin(Intent::Initial, None).unwrap();
        assert!(reads.accept_page(&head, &mut chain, ["a".into()], Some("c1".into())));
        reads.finish(&head);
        let append = reads
            .begin(Intent::Append, chain.next_cursor.clone())
            .unwrap();
        assert!(!reads.accept_page(&append, &mut chain, ["a".into()], Some("c2".into())));
        assert_eq!(chain.next_cursor.as_deref(), Some("c1"));
        reads.finish(&append); // failed/cancelled: caller retains data and cursor
        let delayed = reads
            .begin(Intent::Append, chain.next_cursor.clone())
            .unwrap();
        let refresh = reads.begin(Intent::Refresh, None).unwrap();
        assert!(!reads.accept_page(&delayed, &mut chain, ["late".into()], None));
        assert!(reads.accept_page(
            &refresh,
            &mut chain,
            ["new-head".into()],
            Some("new-cursor".into())
        ));
        reads.finish(&refresh);
        reads.change_scope("desk:b/2:q:sort:limit");
        reads.change_scope("desk:a/1:q:sort:limit");
        assert!(!reads.accept_page(&refresh, &mut chain, ["old-head".into()], None));
        let mut c = Coordinator::with_cache(Policy::V1, cache);
        let key = PublicKey::Reader("w1".into());
        c.follow_seed(key.clone(), sample_pub("w1", "r1"), 0.0, 0.0);
        let first = c.signal_all(120_000.0, true).fetches[0].1;
        for _ in 0..5 {
            assert!(c.signal_all(120_000.0, true).fetches.is_empty());
        }
        assert!(c.tick(120_000.0, true).fetches.is_empty());
        c.finish(
            key.clone(),
            first,
            Outcome::Temporary { retry_after: None },
            120_001.0,
            0.0,
            true,
        );
        assert_eq!(c.status(&key).unwrap().data, Some(sample_pub("w1", "r1")));
        assert!(c.signal_all(120_002.0, true).fetches.is_empty());
        assert!(c.tick(120_002.0, true).fetches.is_empty());
        let stale = c.check(&key, Trigger::Manual, 120_003.0, true).fetches[0].1;
        c.release(&key, 120_004.0);
        c.follow_seed(key.clone(), sample_pub("w1", "r1"), 0.0, 120_005.0);
        c.finish(
            key.clone(),
            stale,
            Outcome::Fresh(sample_pub("w1", "obsolete")),
            120_006.0,
            0.0,
            true,
        );
        assert_ne!(
            c.status(&key).unwrap().data,
            Some(sample_pub("w1", "obsolete"))
        );
        for control in [
            crate::contract::ReaderResponse::Hidden {
                work_id: "w1".into(),
            },
            crate::contract::ReaderResponse::ReviewWithdrawn {
                work_id: "w1".into(),
                author_name: "Tác giả".into(),
            },
        ] {
            let ticket = c.check(&key, Trigger::Manual, 130_000.0, true).fetches[0].1;
            c.finish(
                key.clone(),
                ticket,
                Outcome::Fresh(Snapshot::Control(control.clone())),
                130_001.0,
                0.0,
                true,
            );
            assert_eq!(
                c.status(&key).unwrap().data,
                Some(Snapshot::Control(control))
            );
            c.follow_seed(key.clone(), sample_pub("w1", "resurrected"), 0.0, 130_002.0);
            assert!(!matches!(
                c.status(&key).unwrap().data,
                Some(Snapshot::Publication(_))
            ));
        }
        let ticket = c.check(&key, Trigger::Manual, 140_000.0, true).fetches[0].1;
        c.finish(key.clone(), ticket, Outcome::Removed, 140_001.0, 0.0, true);
        assert!(c.status(&key).is_none());
        c.finish(
            key.clone(),
            first,
            Outcome::Fresh(sample_pub("w1", "r1")),
            140_002.0,
            0.0,
            true,
        );
        assert!(c.status(&key).is_none());
        let LoadStart::Wait { effects, .. } = c.load(&key, 140_003.0) else {
            panic!()
        };
        c.finish(
            key.clone(),
            effects.fetches[0].1,
            Outcome::Fresh(sample_pub("w1", "new")),
            140_004.0,
            0.0,
            true,
        );
        assert_eq!(c.status(&key).unwrap().data, Some(sample_pub("w1", "new")));
        // A→B→A with cancelled initial transport; same key cannot revive the old request.
        c.clear();
        let LoadStart::Wait { id, effects } = c.load(&key, 150_000.0) else {
            panic!()
        };
        let abandoned = effects.fetches[0].1;
        c.cancel_waiter(&key, id);
        let LoadStart::Wait { effects, .. } = c.load(&key, 150_001.0) else {
            panic!()
        };
        assert_eq!(effects.fetches.len(), 1);
        c.finish(
            key.clone(),
            abandoned,
            Outcome::Fresh(sample_pub("w1", "late")),
            150_002.0,
            0.0,
            true,
        );
        assert!(c.status(&key).unwrap().data.is_none());
        c.finish(
            key.clone(),
            effects.fetches[0].1,
            Outcome::Fresh(sample_pub("wrong-work", "bad")),
            150_003.0,
            0.0,
            true,
        );
        assert!(
            c.status(&key).unwrap().data.is_none(),
            "one validator regardless of cache"
        );
    }
    #[test]
    fn memory_and_noop_share_authority_error_scope_and_single_flight_contract() {
        conformance(InMemoryCache::default());
        conformance(NoOpCache);
    }
    #[test]
    fn only_memory_reopens_a_permitted_snapshot_and_controls_are_never_retained() {
        fn reopen(cache: impl SnapshotCache<PublicKey, Snapshot>, expected_hit: bool) {
            let mut c = Coordinator::with_cache(Policy::V1, cache);
            let key = PublicKey::Reader("w1".into());
            c.follow_seed(key.clone(), sample_pub("w1", "r1"), 0.0, 0.0);
            c.release(&key, 1.0);
            assert_eq!(
                matches!(
                    c.load(&key, 2.0),
                    LoadStart::Immediate(Ok(Snapshot::Publication(_)))
                ),
                expected_hit
            );
        }
        reopen(InMemoryCache::default(), true);
        reopen(NoOpCache, false);
        let mut c = Coordinator::new(Policy::V1);
        let key = PublicKey::Reader("w1".into());
        c.follow_seed(
            key.clone(),
            Snapshot::Control(crate::contract::ReaderResponse::Hidden {
                work_id: "w1".into(),
            }),
            0.0,
            0.0,
        );
        c.release(&key, 1.0);
        assert!(c.status(&key).unwrap().data.is_none());
        assert!(matches!(c.load(&key, 2.0), LoadStart::Wait { .. }));
    }
}
