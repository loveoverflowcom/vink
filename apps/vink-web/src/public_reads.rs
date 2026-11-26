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

#[cfg(any(feature = "browser", test))]
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

#[cfg(any(feature = "browser", test))]
impl Snapshot {
    fn clear_attribution(&mut self) {
        use crate::contract::AuthorProjection;
        fn clear(label: &mut String, author: &mut Option<AuthorProjection>) {
            label.clear();
            *author = Some(AuthorProjection::Unsupported);
        }
        match self {
            Self::Works(list) => {
                for card in &mut list.works {
                    clear(&mut card.author_display_name, &mut card.author);
                }
            }
            Self::Publication(publication) => {
                clear(
                    &mut publication.author_display_name,
                    &mut publication.author,
                );
            }
            Self::Control(ReaderResponse::ReviewWithdrawn { author_name, .. }) => {
                *author_name = "Chưa xác nhận được tác giả".into();
            }
            Self::Control(_) => {}
        }
    }

    fn carries_linked_attribution(&self) -> bool {
        use crate::contract::AuthorProjection;
        match self {
            Self::Works(list) => list.works.iter().any(|card| {
                card.author
                    .as_ref()
                    .is_some_and(AuthorProjection::is_linked)
            }),
            Self::Publication(publication) => publication
                .author
                .as_ref()
                .is_some_and(AuthorProjection::is_linked),
            Self::Control(_) => false,
        }
    }
}

/// READ-02 in-tab RAM retention budget (`WEB-PERF-07 / CHOICE-01 B`): 1 MiB per entry and
/// 2 MiB total. The #237 measurements selected these limits for the retired blocks/lines
/// document. The canonical document (#255) retains one source string, so its weight includes
/// exact UTF-8 source bytes (including NFD and line endings), without per-line allocations.
/// Historical clone/heap measurements do not describe this representation. Independent of the
/// disabled disk quota; this is a deterministic proxy, never host RSS or GPU memory.
pub const READ_CACHE_BUDGET: crate::read_cache::RetentionBudget =
    crate::read_cache::RetentionBudget {
        max_total_bytes: Some(2 * 1024 * 1024),
        max_entry_bytes: Some(1024 * 1024),
    };

/// Zero-allocation deterministic UTF-8 + structural container weight proxy for optional
/// `SnapshotCache` retention accounting. Active views and in-flight buffers are outside this sum.
#[must_use]
pub fn snapshot_weight_bytes(snapshot: &Snapshot) -> usize {
    fn opt_str_len(s: &Option<String>) -> usize {
        s.as_ref().map_or(0, String::len)
    }
    fn author_bytes(author: &crate::contract::AuthorProjection) -> usize {
        use crate::contract::AuthorProjection;
        80usize.saturating_add(match author {
            AuthorProjection::Linked(author) => author
                .pen_name
                .len()
                .saturating_add(author.handle.len())
                .saturating_add(author.href.len()),
            AuthorProjection::Deleted { label } | AuthorProjection::Unavailable { label } => {
                label.len()
            }
            AuthorProjection::Unsupported => 0,
        })
    }
    fn public_tag_bytes(t: &crate::contract::PublicTagDto) -> usize {
        64usize
            .saturating_add(t.tag_id.len())
            .saturating_add(t.facet.len())
            .saturating_add(t.label.len())
            .saturating_add(t.slug.len())
    }
    fn tag_dto_bytes(t: &crate::contract::TagDto) -> usize {
        let succ = t.successor.as_ref().map_or(0, |s| {
            48usize
                .saturating_add(s.tag_id.len())
                .saturating_add(s.label.len())
                .saturating_add(s.slug.len())
        });
        128usize
            .saturating_add(t.tag_id.len())
            .saturating_add(t.facet.len())
            .saturating_add(t.label.len())
            .saturating_add(t.slug.len())
            .saturating_add(opt_str_len(&t.description))
            .saturating_add(t.status.len())
            .saturating_add(succ)
    }

    match snapshot {
        Snapshot::Works(list) => {
            let mut bytes = 64usize
                .saturating_add(opt_str_len(&list.filters.q))
                .saturating_add(opt_str_len(&list.next_cursor));
            for filter in &list.filters.tags {
                bytes = bytes
                    .saturating_add(64)
                    .saturating_add(filter.requested.len())
                    .saturating_add(filter.resolution.len())
                    .saturating_add(filter.tag.as_ref().map_or(0, tag_dto_bytes));
            }
            for card in &list.works {
                bytes = bytes
                    .saturating_add(128)
                    .saturating_add(card.work_id.len())
                    .saturating_add(card.title.len())
                    .saturating_add(opt_str_len(&card.excerpt))
                    .saturating_add(card.author_display_name.len())
                    .saturating_add(card.author.as_ref().map_or(0, author_bytes))
                    .saturating_add(card.published_at.len())
                    .saturating_add(opt_str_len(&card.first_published_at));
                for tag in &card.tags {
                    bytes = bytes.saturating_add(public_tag_bytes(tag));
                }
            }
            bytes
        }
        Snapshot::Publication(pub_) => {
            let mut bytes = 256usize
                .saturating_add(pub_.work_id.len())
                .saturating_add(pub_.revision_id.len())
                .saturating_add(pub_.title.len())
                .saturating_add(pub_.author_display_name.len())
                .saturating_add(pub_.author.as_ref().map_or(0, author_bytes))
                .saturating_add(pub_.published_at.len());
            for tag in &pub_.tags {
                bytes = bytes.saturating_add(public_tag_bytes(tag));
            }
            // One String plus allocator header; format is an inline enum, covered by the base.
            bytes
                .saturating_add(28)
                .saturating_add(pub_.document.source.len())
        }
        Snapshot::Control(_) => 0,
    }
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
        #[cfg(feature = "browser")]
        browser::check(&self.key, crate::freshness::Trigger::Manual);
        #[cfg(not(feature = "browser"))]
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
    #[cfg(feature = "browser")]
    browser::invalidate_withdrawn(work_id);
    #[cfg(not(feature = "browser"))]
    let _ = work_id;
}

/// Call after a confirmed publication change so observed publication/list views revalidate without
/// waiting for the 120s timer. Unlike [`invalidate_withdrawn`] no card is optimistically removed.
pub fn invalidate_publication(work_id: &str) {
    #[cfg(feature = "browser")]
    browser::invalidate_publication(work_id);
    #[cfg(not(feature = "browser"))]
    let _ = work_id;
}

/// Reserved for a policy-approved public-device-cache control; no private storage is touched.
/// The boolean acknowledges only durable completion. No UI claims a save/clear yet.
#[cfg(feature = "browser")]
pub async fn clear_device_cache() -> bool {
    browser::clear_device_cache().await
}

#[cfg(feature = "browser")]
pub fn invalidate_author_attribution() {
    browser::invalidate_author_attribution();
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
    #[cfg(feature = "browser")]
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
    #[cfg(not(feature = "browser"))]
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
#[cfg(feature = "browser")]
pub async fn load(key: PublicKey) -> Result<Snapshot, LoadError> {
    browser::load(key).await
}

#[cfg(any(feature = "browser", test))]
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
                    | crate::contract::ReaderResponse::AuthorDeleted { work_id }
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
        attribution_barrier: Millis,
    }

    impl Coordinator {
        pub fn new(policy: Policy) -> Self {
            Self::with_cache(
                policy,
                InMemoryCache::with_budget(super::READ_CACHE_BUDGET, super::snapshot_weight_bytes),
            )
        }
    }
    impl<C: SnapshotCache<PublicKey, Snapshot>> Coordinator<C> {
        pub fn with_cache(policy: Policy, cache: C) -> Self {
            Self {
                store: Store::with_cache(policy, cache, |snapshot| match snapshot {
                    Snapshot::Works(_) => true,
                    // On a new SPA Reader visit the server must confirm that the
                    // publication is still accessible before its body can appear.
                    Snapshot::Publication(_) => !cfg!(feature = "csr"),
                    Snapshot::Control(_) => false,
                }),
                waiters: BTreeMap::new(),
                withdrawn: BTreeMap::new(),
                next_waiter: 1,
                seq: 0,
                seed_barrier: Millis::NEG_INFINITY,
                attribution_barrier: Millis::NEG_INFINITY,
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
                    if self.store.observer_count(key) == 0 {
                        self.store.mark_stale(key);
                    }
                }
            }
        }

        #[cfg(any(test, not(feature = "csr")))]
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
            // Only an invalidation seen by this document fences a seed. A back/forward
            // document (`rendered_at` = -inf) must keep the attribution its HTML rendered,
            // otherwise hydration disagrees with the server markup.
            let mut sanitized =
                self.attribution_barrier.is_finite() && rendered_at <= self.attribution_barrier;
            if sanitized {
                initial.clear_attribution();
            }
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
                    sanitized |= list.works.len() != before;
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

        fn push_waiter_replies(&mut self, key: &PublicKey, reply: Reply, effects: &mut Effects) {
            if let Some(mut ids) = self.waiters.remove(key)
                && let Some(last) = ids.pop()
            {
                effects
                    .replies
                    .extend(ids.into_iter().map(|id| (id, reply.clone())));
                effects.replies.push((last, reply));
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
                    Load::Ready(snap) => self.push_waiter_replies(&key, Ok(snap), effects),
                    Load::Rejected(rej) => {
                        self.push_waiter_replies(&key, Err(LoadError::Invalid(rej)), effects);
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

        pub fn invalidate_author_attribution(&mut self, at: Millis, visible: bool) -> Effects {
            self.attribution_barrier = at;
            self.store.edit_matching(
                |_| true,
                |_, snapshot| {
                    snapshot.clear_attribution();
                    true
                },
            );
            self.store.mutation_confirmed_where(|_| true);
            let mut effects = Effects {
                notify_changed: true,
                reschedule: true,
                ..Effects::default()
            };
            self.wake_matching(|_| true, at, visible, &mut effects);
            effects
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
                    | ReaderResponse::AuthorDeleted { .. }
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
                self.push_waiter_replies(&target, Err(LoadError::NotFound), effects);
            }
            self.store.edit_matching(
                |key| matches!(key, PublicKey::Works(_)),
                |_, snapshot| {
                    let Snapshot::Works(list) = snapshot else {
                        return false;
                    };
                    let before = list.works.len();
                    list.works.retain(|card| card.work_id != work_id);
                    before != list.works.len()
                },
            );
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
            let control = matches!(&outcome, Outcome::Fresh(Snapshot::Control(_)));
            let outcome = match outcome {
                Outcome::Fresh(snapshot) if !valid_snapshot(&key, &snapshot) => {
                    Outcome::Temporary { retry_after: None }
                }
                other => other,
            };
            let had_waiter = self.waiters.get(&key).is_some_and(|ws| !ws.is_empty());
            let applied = self.store.complete(&key, ticket, outcome, at, random);
            match applied {
                Applied::Fresh { .. } => {
                    if let PublicKey::Reader(id) = &key {
                        if control {
                            self.apply_withdrawal(id, at, visible, false, &mut effects);
                        } else {
                            self.withdrawn.remove(id);
                        }
                    }
                    if had_waiter
                        && let Some(reply) = self.store.status(&key).and_then(|s| s.data).map(Ok)
                    {
                        self.push_waiter_replies(&key, reply, &mut effects);
                    }
                }
                Applied::Removed => {
                    self.push_waiter_replies(&key, Err(LoadError::NotFound), &mut effects);
                    if let PublicKey::Reader(id) = &key {
                        self.apply_withdrawal(id, at, visible, false, &mut effects);
                    }
                }
                Applied::Rejected(rejection) => {
                    self.push_waiter_replies(
                        &key,
                        Err(LoadError::Invalid(rejection)),
                        &mut effects,
                    );
                }
                Applied::Failed => {
                    self.push_waiter_replies(&key, Err(LoadError::Unavailable), &mut effects);
                }
                Applied::Discarded => {
                    if self.waiters.get(&key).is_some_and(|ws| !ws.is_empty()) {
                        match self.store.load(&key, at) {
                            Load::Ready(data) => {
                                self.push_waiter_replies(&key, Ok(data), &mut effects);
                            }
                            Load::Rejected(rejection) => {
                                self.push_waiter_replies(
                                    &key,
                                    Err(LoadError::Invalid(rejection)),
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
                self.push_waiter_replies(key, Ok(snapshot), &mut effects);
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

#[cfg(feature = "browser")]
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
        #[cfg(feature = "perf-probe")]
        fetch_starts: u64,
        #[cfg(feature = "perf-probe")]
        fetch_finishes: u64,
        timer: Option<i32>,
        tick: Option<Closure<dyn FnMut()>>,
        listener: Option<u64>,
        storage_lease: Option<crate::public_persistence::browser::Lease>,
    }

    thread_local! {
        static OWNER: RefCell<Owner> = RefCell::new(Owner {
            coord: Coordinator::new(Policy::V1),
            channels: BTreeMap::new(),
            #[cfg(feature = "perf-probe")]
            fetch_starts: 0,
            #[cfg(feature = "perf-probe")]
            fetch_finishes: 0,
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
    #[cfg(not(feature = "csr"))]
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
        resource: Ticket,
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
                    owner.coord.store.release_resource(&key, self.resource);
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
            let start = with(|owner| owner.coord.load(&key, at));
            if let LoadStart::Immediate(Err(error)) = start {
                return Err(error);
            }
            let resource = with(|owner| owner.coord.store.hold_resource(&key, at));
            let cancelled_key = key.clone();
            let pending = match &start {
                LoadStart::Immediate(_) => None,
                LoadStart::Wait { id, .. } => Some(*id),
            };
            on_cleanup(move || {
                with(|owner| {
                    if let Some(id) = pending {
                        owner.channels.remove(&id);
                        owner.coord.cancel_waiter(&cancelled_key, id);
                    }
                    owner.coord.store.release_resource(&cancelled_key, resource);
                });
            });
            match start {
                LoadStart::Immediate(reply) => return reply,
                LoadStart::Wait { id, effects } => {
                    with(|owner| owner.channels.insert(id, tx));
                    apply_effects(effects);
                    (
                        rx,
                        WaiterGuard {
                            key,
                            id,
                            resource,
                            settled: false,
                        },
                    )
                }
            }
        };
        let reply = receive.await.unwrap_or(Err(LoadError::Unavailable));
        if reply.is_err() {
            with(|owner| {
                owner
                    .coord
                    .store
                    .release_resource(&guard.key, guard.resource)
            });
        }
        guard.settled = true;
        reply
    }

    pub fn follow(key: PublicKey, initial: Snapshot) -> Memo<Option<Status<Snapshot>>> {
        let at = now();
        let has_linked_attribution = initial.carries_linked_attribution();
        with(|owner| {
            #[cfg(not(feature = "csr"))]
            owner
                .coord
                .follow_seed(key.clone(), initial, rendered_at(), at);
            #[cfg(feature = "csr")]
            owner.coord.store.observe(&key, at);
        });
        listen();
        // After hydration, never during it: a refresh flag set mid-hydration would disagree
        // with the server HTML.
        let mounted = key.clone();
        Effect::new(move |_| {
            // Only a link requires an immediate authority read on reentry. Unlinked
            // labels and tombstones follow the normal public snapshot schedule.
            let trigger = if has_linked_attribution && cached_unverified(&mounted) {
                Trigger::Manual
            } else {
                Trigger::Signal
            };
            check(&mounted, trigger);
        });
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

    pub fn invalidate_author_attribution() {
        let effects = with(|owner| owner.coord.invalidate_author_attribution(now(), visible()));
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

    /// Snapshot of the READ-02 owner. The lab bundle exports counts only, never keys or data.
    #[cfg(feature = "perf-probe")]
    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn public_read_counts() -> String {
        with(|owner| {
            let (observers, resources, flights) = owner.coord.store.live_counts();
            serde_json::json!({
                "observers": observers, "resource_tickets": resources, "flights": flights,
                "fetch_starts": owner.fetch_starts, "fetch_finishes": owner.fetch_finishes, "waiters": owner.channels.len(),
                "scheduler_timers": usize::from(owner.timer.is_some()),
                "read_event_subscriptions": crate::read_events::subscriptions(),
            })
            .to_string()
        })
    }

    fn spawn_fetch(key: PublicKey, ticket: Ticket) {
        #[cfg(feature = "perf-probe")]
        with(|owner| owner.fetch_starts += 1);
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
            let persist = (lease.is_some() && with(|o| o.coord.store.is_current(&key, ticket)))
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
        #[cfg(feature = "perf-probe")]
        with(|owner| owner.fetch_finishes += 1);
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

    /// Ordinary wake hints revalidate observed stale snapshots. A restored history document
    /// can have missed profile changes while frozen, so fence its attribution before refreshing
    /// even if the snapshot was fresh just before leaving. Install once for the tab's lifetime.
    fn listen() {
        with(|owner| {
            if owner.listener.is_some() {
                return;
            }
            crate::public_persistence::browser::listen(|| {
                leptos::task::spawn_local(synchronize_storage());
            });
            leptos::task::spawn_local(synchronize_storage());
            let signal = crate::read_events::register(|hint| {
                leptos::task::spawn_local(synchronize_storage());
                if hint == crate::read_events::Hint::HistoryRestored {
                    invalidate_author_attribution();
                } else {
                    let effects = with(|owner| owner.coord.signal_all(now(), visible()));
                    apply_effects(effects);
                }
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
            excerpt: None,
            author_display_name: "Author".into(),
            author: None,
            published_at: "2026-11-15T00:00:00Z".into(),
            first_published_at: None,
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

    #[test]
    fn author_contract_cache_counts_ref_bytes_and_fences_retained_attribution() {
        use crate::contract::{AuthorProjection, PublicAuthorRef};
        let key = PublicKey::Works(ExploreQuery::default());
        let mut stale = sample_list(&["w1"]);
        let legacy_size = super::snapshot_weight_bytes(&stale);
        assert!(!stale.carries_linked_attribution());
        let mut unavailable = stale.clone();
        let Snapshot::Works(list) = &mut unavailable else {
            unreachable!()
        };
        list.works[0].author = Some(AuthorProjection::Unavailable {
            label: "Bút danh cũ".into(),
        });
        assert!(!unavailable.carries_linked_attribution());
        let Snapshot::Works(list) = &mut stale else {
            unreachable!()
        };
        let author = PublicAuthorRef {
            pen_name: "Tên tác giả".into(),
            handle: "ten".into(),
            href: "/@ten".into(),
        };
        let reference_bytes = author.pen_name.len() + author.handle.len() + author.href.len();
        list.works[0].author = Some(AuthorProjection::Linked(author));
        assert!(stale.carries_linked_attribution());
        assert!(super::snapshot_weight_bytes(&stale) >= legacy_size + reference_bytes);

        let mut read = Coordinator::new(Policy::V1);
        read.follow_seed(key.clone(), stale.clone(), 1.0, 1.0);
        read.release(&key, 2.0);
        let ticket = read.check(&key, Trigger::Manual, 3.0, true).fetches[0].1;
        read.invalidate_author_attribution(4.0, true);
        read.finish(
            key.clone(),
            ticket,
            Outcome::Fresh(stale.clone()),
            5.0,
            0.0,
            true,
        );
        read.follow_seed(key.clone(), stale, 3.0, 6.0);
        let Some(Snapshot::Works(list)) = read.status(&key).and_then(|s| s.data) else {
            panic!("expected retained work data");
        };
        assert_eq!(list.works[0].author, Some(AuthorProjection::Unsupported));
        assert!(list.works[0].author_display_name.is_empty());
        assert_eq!(list.works[0].work_id, "w1");
    }

    #[test]
    fn back_forward_seed_keeps_attribution_until_this_document_invalidates() {
        use crate::contract::{AuthorProjection, PublicAuthorRef};
        let key = PublicKey::Works(ExploreQuery::default());
        let mut linked = sample_list(&["w1"]);
        let Snapshot::Works(list) = &mut linked else {
            unreachable!()
        };
        list.works[0].author = Some(AuthorProjection::Linked(PublicAuthorRef {
            pen_name: "Tên tác giả".into(),
            handle: "ten".into(),
            href: "/@ten".into(),
        }));
        let mut read = Coordinator::new(Policy::V1);
        read.follow_seed(key.clone(), linked.clone(), f64::NEG_INFINITY, 1.0);
        assert_eq!(read.status(&key).and_then(|s| s.data), Some(linked.clone()));

        let mut fenced = Coordinator::new(Policy::V1);
        fenced.invalidate_author_attribution(1.0, true);
        fenced.follow_seed(key.clone(), linked, f64::NEG_INFINITY, 2.0);
        let Some(Snapshot::Works(list)) = fenced.status(&key).and_then(|s| s.data) else {
            panic!("expected fenced work data");
        };
        assert_eq!(list.works[0].author, Some(AuthorProjection::Unsupported));
    }

    #[test]
    fn author_contract_history_restore_fences_even_fresh_observed_attribution() {
        use crate::contract::{AuthorProjection, PublicAuthorRef};
        let key = PublicKey::Reader("work".into());
        let mut linked = sample_pub("work", "revision");
        let Snapshot::Publication(publication) = &mut linked else {
            unreachable!()
        };
        publication.author = Some(AuthorProjection::Linked(PublicAuthorRef {
            pen_name: "Bút danh trước đó".into(),
            handle: "ten-cu".into(),
            href: "/@ten-cu".into(),
        }));
        let mut read = Coordinator::new(Policy::V1);
        read.follow_seed(key.clone(), linked.clone(), 1.0, 1.0);
        assert!(read.signal_all(2.0, true).fetches.is_empty());
        let effects = read.invalidate_author_attribution(3.0, true);
        assert!(effects.notify_changed);
        assert_eq!(effects.fetches.len(), 1);
        assert_eq!(effects.fetches[0].0, key);
        assert!(read.signal_all(4.0, true).fetches.is_empty());
        let Some(Snapshot::Publication(fenced)) = read.status(&key).and_then(|s| s.data) else {
            panic!("expected retained publication");
        };
        assert_eq!(fenced.revision_id, "revision");
        assert_eq!(fenced.author, Some(AuthorProjection::Unsupported));
        assert!(fenced.author_display_name.is_empty());

        let Snapshot::Publication(publication) = &mut linked else {
            unreachable!()
        };
        publication.author = Some(AuthorProjection::Deleted {
            label: "Tài khoản đã xóa".into(),
        });
        publication.author_display_name = "Tài khoản đã xóa".into();
        read.finish(
            key.clone(),
            effects.fetches[0].1,
            Outcome::Fresh(linked.clone()),
            5.0,
            0.0,
            true,
        );
        assert_eq!(read.status(&key).and_then(|s| s.data), Some(linked));
    }

    fn sample_pub(id: &str, rev: &str) -> Snapshot {
        Snapshot::Publication(Publication {
            work_id: id.into(),
            revision_id: rev.into(),
            title: format!("Title {id}"),
            author_display_name: "Author".into(),
            author: None,
            document: DocumentDto {
                format: crate::contract::DocumentFormatDto::Default,
                source: String::new(),
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
        conformance(InMemoryCache::with_budget(
            super::READ_CACHE_BUDGET,
            super::snapshot_weight_bytes,
        ));
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
        reopen(
            InMemoryCache::with_budget(super::READ_CACHE_BUDGET, super::snapshot_weight_bytes),
            true,
        );
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

    fn heavy_pub(id: &str, block_count: usize, line_bytes: usize) -> Snapshot {
        let line = "a".repeat(line_bytes);
        Snapshot::Publication(Publication {
            work_id: id.into(),
            revision_id: format!("rev-{id}"),
            title: id.into(),
            author_display_name: "Tác giả".into(),
            author: None,
            published_at: "2026-11-17T00:00:00Z".into(),
            document: crate::document::default_document(&vec![line; block_count].join("\n\n")),
            tags: Vec::new(),
        })
    }

    fn oversized_pub(id: &str) -> Snapshot {
        let Snapshot::Publication(mut publication) = heavy_pub(id, 0, 0) else {
            unreachable!()
        };
        // 200k NFC characters, 1.2 MB exact NFD source: valid under the canonical text budget,
        // but over the optional cache's 1 MiB entry limit.
        publication.document =
            crate::document::default_document(&"a\u{0306}\u{0301}\n".repeat(200_000));
        Snapshot::Publication(publication)
    }

    #[test]
    fn discovery_cache_weight_includes_the_excerpt_utf8_bytes() {
        let empty = sample_list(&["work"]);
        let Snapshot::Works(mut list) = empty.clone() else {
            unreachable!()
        };
        let excerpt = "Mưa trên phố cũ";
        list.works[0].excerpt = Some(excerpt.into());
        assert_eq!(
            super::snapshot_weight_bytes(&Snapshot::Works(list)),
            super::snapshot_weight_bytes(&empty) + excerpt.len()
        );
    }

    #[test]
    fn canonical_cache_weight_counts_exact_source_without_line_allocations() {
        let empty = heavy_pub("source", 0, 0);
        let Snapshot::Publication(mut publication) = empty.clone() else {
            unreachable!()
        };
        let raw = "Vie\u{0323}\u{0302}t\r\n\n \t\n";
        publication.document = crate::document::default_document(raw);
        assert_eq!(
            super::snapshot_weight_bytes(&Snapshot::Publication(publication)),
            super::snapshot_weight_bytes(&empty) + raw.len()
        );
        assert!(super::snapshot_weight_bytes(&heavy_pub("tiny", 30_000, 1)) < 100_000);
    }

    #[test]
    fn joined_resources_disposed_after_delivery_release_the_unmounted_body() {
        let mut c = Coordinator::new(Policy::V1);
        let key = PublicKey::Reader("huge-source".into());
        let LoadStart::Wait { id, effects } = c.load(&key, 0.0) else {
            panic!()
        };
        let first = c.store.hold_resource(&key, 0.0);
        let LoadStart::Wait { id: joined, .. } = c.load(&key, 0.1) else {
            panic!()
        };
        let second = c.store.hold_resource(&key, 0.1);
        let huge = oversized_pub("huge-source");
        let done = c.finish(
            key.clone(),
            effects.fetches[0].1,
            Outcome::Fresh(huge.clone()),
            1.0,
            0.0,
            true,
        );
        assert_eq!(
            done.replies,
            vec![(id, Ok(huge.clone())), (joined, Ok(huge.clone()))]
        );
        assert_eq!(c.pending_waiter_count(&key), 0);
        c.cancel_waiter(&key, id);
        c.store.release_resource(&key, first);
        assert_eq!(c.status(&key).unwrap().data, Some(huge));
        assert!(c.store.observed().is_empty());
        c.cancel_waiter(&key, joined);
        c.store.release_resource(&key, second);
        assert!(
            c.status(&key).unwrap().data.is_none(),
            "last disposed Resource must release an unmounted oversized reply"
        );
        assert_eq!(c.store.cache().oversized_skips(), 1);
        assert!(matches!(c.load(&key, 2.0), LoadStart::Wait { .. }));
    }

    #[test]
    fn disposed_resource_from_before_withdrawal_cannot_park_the_republication() {
        let mut c = Coordinator::new(Policy::V1);
        let key = PublicKey::Reader("w".into());
        let LoadStart::Wait { id, effects } = c.load(&key, 0.0) else {
            panic!()
        };
        let old_resource = c.store.hold_resource(&key, 0.0);
        assert_eq!(
            c.withdraw_publication("w", 1.0, true).replies,
            vec![(id, Err(LoadError::NotFound))]
        );
        let LoadStart::Wait {
            id: new,
            effects: later,
        } = c.load(&key, 2.0)
        else {
            panic!()
        };
        let current = c.store.hold_resource(&key, 2.0);
        c.store.release_resource(&key, old_resource);
        let obsolete = c.finish(
            key.clone(),
            effects.fetches[0].1,
            Outcome::Fresh(sample_pub("w", "old")),
            3.0,
            0.0,
            true,
        );
        assert!(obsolete.replies.is_empty());
        assert!(c.status(&key).is_none());
        let fresh = c.finish(
            key.clone(),
            later.fetches[0].1,
            Outcome::Fresh(sample_pub("w", "new")),
            4.0,
            0.0,
            true,
        );
        assert_eq!(fresh.replies, vec![(new, Ok(sample_pub("w", "new")))]);
        c.store.release_resource(&key, current);
        assert!(c.store.cache().contains_key(&key));
    }

    #[test]
    fn withdrawal_preserves_release_order_of_unrelated_readers() {
        let cache = InMemoryCache::with_budget(
            crate::read_cache::RetentionBudget {
                max_entry_bytes: None,
                max_total_bytes: Some(2),
            },
            |_| 1,
        );
        let mut c = Coordinator::with_cache(Policy::V1, cache);
        let key = |id: &str| PublicKey::Reader(id.into());
        for id in ["z", "a"] {
            c.follow_seed(key(id), sample_pub(id, "r1"), 0.0, 1.0);
            c.release(&key(id), 2.0);
        }
        c.withdraw_publication("unrelated", 3.0, true);
        c.follow_seed(key("b"), sample_pub("b", "r1"), 0.0, 4.0);
        c.release(&key("b"), 5.0);
        assert!(
            !c.store.cache().contains_key(&key("z")),
            "oldest released Reader must be evicted first"
        );
        assert!(c.store.cache().contains_key(&key("a")));
    }

    #[test]
    fn over_budget_snapshot_reads_cleanly_without_retention_or_live_view_or_cursor_loss() {
        use crate::read_lifecycle::{Intent, PageChain, ReadOwner};

        let huge = oversized_pub("huge-source");
        let huge_weight = super::snapshot_weight_bytes(&huge);
        assert!(huge_weight > super::READ_CACHE_BUDGET.max_entry_bytes.unwrap());

        // Typical 452 KiB contiguous doc fits within the 1 MiB per-entry budget.
        let fit_prose = heavy_pub("fit-prose", 1, 452_000);
        assert!(
            super::snapshot_weight_bytes(&fit_prose)
                <= super::READ_CACHE_BUDGET.max_entry_bytes.unwrap()
        );

        let mut c = Coordinator::new(Policy::V1);
        let active_key = PublicKey::Reader("active".into());
        let huge_key = PublicKey::Reader("huge-source".into());
        c.follow_seed(active_key.clone(), sample_pub("active", "r1"), 0.0, 0.0);

        // Over-budget work still loads, finishes, and renders while mounted.
        let LoadStart::Wait { id, effects } = c.load(&huge_key, 1.0) else {
            panic!("initial huge load must fetch");
        };
        let done = c.finish(
            huge_key.clone(),
            effects.fetches[0].1,
            Outcome::Fresh(huge.clone()),
            2.0,
            0.0,
            true,
        );
        let [(receiver, reply)]: [_; 1] = done.replies.try_into().unwrap();
        let Ok(delivered) = reply else {
            panic!("over-budget read must deliver its accepted response");
        };
        assert_eq!(receiver, id);
        assert_eq!(delivered, huge);
        c.follow_seed(huge_key.clone(), delivered, 0.0, 3.0);
        assert_eq!(c.status(&huge_key).unwrap().data, Some(huge));

        // Leaving releases huge_key; SnapshotCache skips retention without failing.
        c.release(&huge_key, 4.0);
        assert_eq!(c.store.cache().total_bytes(), 0);
        assert_eq!(c.store.cache().oversized_skips(), 1);
        assert!(c.status(&huge_key).unwrap().data.is_none());

        // Churning 6 x ~460 KiB works forces weighted evictions while active_key and cursor chain survive untouched.
        let mut chain = PageChain::default();
        let mut reads = ReadOwner::new("desk:scope:page");
        let t = reads.begin(Intent::Initial, None).unwrap();
        assert!(reads.accept_page(&t, &mut chain, vec!["w1".into()], Some("cur-1".into())));
        reads.finish(&t);

        for i in 0..6u32 {
            let k = PublicKey::Reader(format!("prose-{i}"));
            let snap = heavy_pub(&format!("prose-{i}"), 1, 460_000);
            c.follow_seed(k.clone(), snap, 0.0, 10.0 + f64::from(i) * 2.0);
            c.release(&k, 11.0 + f64::from(i) * 2.0);
        }
        assert!(c.store.cache().total_bytes() <= super::READ_CACHE_BUDGET.max_total_bytes.unwrap());
        assert!(c.store.cache().evictions() >= 2);
        assert_eq!(
            c.status(&active_key).unwrap().data,
            Some(sample_pub("active", "r1"))
        );
        assert_eq!(chain.next_cursor.as_deref(), Some("cur-1"));
        assert_eq!(chain.pages, 1);

        // Reopening the over-budget entry starts a clean network fetch rather than failing.
        assert!(matches!(c.load(&huge_key, 30.0), LoadStart::Wait { .. }));
    }

    fn drive_cache_visit<C: SnapshotCache<PublicKey, Snapshot>>(
        coord: &mut Coordinator<C>,
        key: &PublicKey,
        snap: &Snapshot,
        at: f64,
    ) -> bool {
        let start = coord.load(key, at);
        let resource = coord.store.hold_resource(key, at);
        let hit = match start {
            LoadStart::Immediate(Ok(s)) => {
                // Browser follow uses the document navigation start, never the cache-hit time.
                coord.follow_seed(key.clone(), s, 0.0, at + 0.1);
                true
            }
            LoadStart::Wait { id, effects } => {
                let (_, tk) = &effects.fetches[0];
                let completion = coord.finish(
                    key.clone(),
                    *tk,
                    Outcome::Fresh(snap.clone()),
                    at + 0.05,
                    0.0,
                    true,
                );
                assert_eq!(
                    completion.replies,
                    vec![(id, Ok(snap.clone()))],
                    "benchmark must use a response accepted by the production coordinator"
                );
                let (_, reply) = completion.replies.into_iter().next().unwrap();
                coord.follow_seed(key.clone(), reply.unwrap(), 0.0, at + 0.1);
                false
            }
            LoadStart::Immediate(Err(_)) => panic!("unexpected rejected/err load state"),
        };
        coord.release(key, at + 0.2);
        coord.store.release_resource(key, resource);
        hit
    }

    #[test]
    fn weighted_eviction_and_50_nav_loops_preserve_hot_list_and_bound_ram() {
        let mut coord_b = Coordinator::new(Policy::V1);
        let mut coord_c = Coordinator::with_cache(Policy::V1, NoOpCache);

        let list_key = PublicKey::Works(ExploreQuery::default());
        let list_snap = sample_list(&["w0", "w1", "w2", "w3"]);
        let w0 = heavy_pub("w0", 1, 460_000);
        let w1 = heavy_pub("w1", 1, 780_000);
        let w2 = heavy_pub("w2", 1, 780_000);
        let w3 = heavy_pub("w3", 1, 780_000);
        let works = [("w0", w0), ("w1", w1), ("w2", w2), ("w3", w3)];

        // Simulate 50 Discovery <-> Reader loops alternating across large 200k-scale documents.
        let mut list_hits_b = 0usize;
        let mut list_hits_c = 0usize;
        for step in 0..50usize {
            let t = (step * 4) as f64;
            if drive_cache_visit(&mut coord_b, &list_key, &list_snap, t) {
                list_hits_b += 1;
            }
            if drive_cache_visit(&mut coord_c, &list_key, &list_snap, t) {
                list_hits_c += 1;
            }

            let (id, snap) = &works[step % works.len()];
            let r_key = PublicKey::Reader((*id).into());
            drive_cache_visit(&mut coord_b, &r_key, snap, t + 1.0);
            drive_cache_visit(&mut coord_c, &r_key, snap, t + 1.0);

            assert!(
                coord_b.store.cache().total_bytes()
                    <= super::READ_CACHE_BUDGET.max_total_bytes.unwrap()
            );
        }
        assert_eq!(
            list_hits_b, 49,
            "hot Discovery list hits cache on all 49 subsequent loops"
        );
        assert_eq!(
            list_hits_c, 0,
            "NoOpCache misses Discovery list on every loop"
        );
        assert!(coord_b.store.cache().evictions() > 0);
    }

    fn assert_ac04_race_matrix<C: SnapshotCache<PublicKey, Snapshot>>(cache: C) {
        let mut c = Coordinator::with_cache(Policy::V1, cache);
        let removed_k = PublicKey::Reader("w-removed".into());
        let withdrawn_k = PublicKey::Reader("w-withdrawn".into());
        let control_k = PublicKey::Reader("w-control".into());
        let cancel_k = PublicKey::Reader("w-cancel-huge".into());
        let list_k = PublicKey::Works(ExploreQuery::default());

        c.follow_seed(
            list_k.clone(),
            sample_list(&["w-removed", "w-withdrawn", "keep"]),
            0.0,
            1.0,
        );
        c.release(&list_k, 2.0);

        // 1) Authoritative removal + stale-if-error suppression across cache eviction churn.
        let LoadStart::Wait { effects, .. } = c.load(&removed_k, 3.0) else {
            panic!();
        };
        let old_removed_ticket = effects.fetches[0].1;
        c.follow_seed(removed_k.clone(), sample_pub("w-removed", "r1"), 2.0, 4.0);
        let stale_err_ticket = c.invalidate_publication("w-removed", 5.0, true).fetches[0].1;
        c.finish(
            removed_k.clone(),
            stale_err_ticket,
            Outcome::Removed,
            6.0,
            0.0,
            true,
        );
        assert!(c.status(&removed_k).is_none());

        // Withdrawn publication while an old list fetch is in flight and list_k sits in cache.
        c.withdraw_publication("w-withdrawn", 7.0, true);

        // Concurrent distinct keys + over-budget eviction pressure.
        let distinct_k = PublicKey::Reader("distinct".into());
        let LoadStart::Wait { effects: d_eff, .. } = c.load(&distinct_k, 8.0) else {
            panic!();
        };
        let distinct_ticket = d_eff.fetches[0].1;
        for i in 0..5u32 {
            let churn = PublicKey::Reader(format!("churn-{i}"));
            c.follow_seed(
                churn.clone(),
                heavy_pub(&format!("churn-{i}"), 1, 700_000),
                8.0,
                9.0 + f64::from(i),
            );
            c.release(&churn, 9.5 + f64::from(i));
        }

        // Old seeds / restore / late Temporary error after removal NEVER resurrect or show stale error.
        assert!(
            c.store.begin_restore(&removed_k, 19.5).is_none(),
            "removed key must never begin offline restore"
        );
        c.follow_seed(
            removed_k.clone(),
            sample_pub("w-removed", "stale-ssr"),
            5.0,
            20.0,
        );
        c.release(&removed_k, 20.5);
        assert!(
            c.status(&removed_k).is_none(),
            "authoritative removal fence must hold after eviction"
        );
        c.finish(
            removed_k.clone(),
            old_removed_ticket,
            Outcome::Temporary { retry_after: None },
            21.0,
            0.0,
            true,
        );
        assert!(
            c.status(&removed_k).is_none(),
            "no stale-if-error after removal"
        );

        // Restore ticket started before withdrawal is superseded by withdrawal + eviction.
        let restore_k = PublicKey::Reader("w-restore-race".into());
        let restore_ticket = c
            .store
            .begin_restore(&restore_k, 21.2)
            .expect("restore ticket");
        c.withdraw_publication("w-restore-race", 21.4, true);
        let eff_restore = c.restore(
            &restore_k,
            restore_ticket,
            sample_pub("w-restore-race", "stale-restore"),
            100.0,
            Some(60_000.0),
            21.6,
        );
        assert!(eff_restore.replies.is_empty());
        assert!(
            c.status(&restore_k).is_none(),
            "withdrawn key must reject late restore"
        );

        c.follow_seed(
            withdrawn_k.clone(),
            sample_pub("w-withdrawn", "stale-ssr"),
            6.0,
            22.0,
        );
        c.release(&withdrawn_k, 22.5);
        assert!(
            c.status(&withdrawn_k).is_none(),
            "withdrawn fence must hold after eviction"
        );

        // Concurrent distinct key still commits cleanly.
        c.finish(
            distinct_k.clone(),
            distinct_ticket,
            Outcome::Fresh(sample_pub("distinct", "r-ok")),
            23.0,
            0.0,
            true,
        );
        assert_eq!(
            c.status(&distinct_k).unwrap().data,
            Some(sample_pub("distinct", "r-ok"))
        );

        // 2) Control response never retained or resurrected after churn.
        let LoadStart::Wait {
            effects: ctl_eff, ..
        } = c.load(&control_k, 24.0)
        else {
            panic!();
        };
        c.finish(
            control_k.clone(),
            ctl_eff.fetches[0].1,
            Outcome::Fresh(Snapshot::Control(
                crate::contract::ReaderResponse::ReviewWithdrawn {
                    work_id: "w-control".into(),
                    author_name: "Tác giả".into(),
                },
            )),
            25.0,
            0.0,
            true,
        );
        c.follow_seed(
            control_k.clone(),
            sample_pub("w-control", "stale"),
            10.0,
            26.0,
        );
        c.release(&control_k, 27.0);
        assert!(c.status(&control_k).unwrap().data.is_none());

        // Republishing clears removal cleanly when a fresh origin load answers 200.
        let LoadStart::Wait {
            effects: rep_eff, ..
        } = c.load(&withdrawn_k, 28.0)
        else {
            panic!();
        };
        c.finish(
            withdrawn_k.clone(),
            rep_eff.fetches[0].1,
            Outcome::Fresh(sample_pub("w-withdrawn", "r2-repub")),
            29.0,
            0.0,
            true,
        );
        assert_eq!(
            c.status(&withdrawn_k).unwrap().data,
            Some(sample_pub("w-withdrawn", "r2-repub"))
        );

        // 3) Waiter cancellation on an over-budget load does not trap the payload in Entry::data.
        let LoadStart::Wait {
            id: waiter_id,
            effects: cancel_eff,
        } = c.load(&cancel_k, 30.0)
        else {
            panic!();
        };
        c.cancel_waiter(&cancel_k, waiter_id);
        let huge = oversized_pub("w-cancel-huge");
        c.finish(
            cancel_k.clone(),
            cancel_eff.fetches[0].1,
            Outcome::Fresh(huge),
            31.0,
            0.0,
            true,
        );
        assert!(
            c.status(&cancel_k).unwrap().data.is_none(),
            "cancelled ticket must discard the over-budget response before storing any payload"
        );
    }

    #[test]
    fn eviction_races_never_resurrect_removed_control_or_leak_cancelled_waiters() {
        assert_ac04_race_matrix(InMemoryCache::with_budget(
            super::READ_CACHE_BUDGET,
            super::snapshot_weight_bytes,
        ));
        assert_ac04_race_matrix(NoOpCache);
    }

    #[test]
    #[should_panic(expected = "authoritative removal fence")]
    fn negative_control_dropping_removal_fence_must_fail() {
        // Bypassing `Coordinator::withdrawn`/`Store::removed` clears the removal fence; the same
        // AC-04 assertion must fail immediately rather than silently resurrecting stale content.
        let mut c = Coordinator::new(Policy::V1);
        let removed_k = PublicKey::Reader("w-removed".into());
        c.withdraw_publication("w-removed", 10.0, true);
        // Simulate a fence bug by clearing `Store::removed` and `Coordinator::withdrawn` on reset
        // while still re-checking the exact invariant.
        c.clear();
        c.follow_seed(
            removed_k.clone(),
            sample_pub("w-removed", "resurrected"),
            5.0,
            20.0,
        );
        c.release(&removed_k, 21.0);
        assert!(
            c.status(&removed_k).is_none(),
            "authoritative removal fence must hold after eviction"
        );
    }

    #[test]
    #[ignore = "AC-01/AC-02 #237 corpus and A/B/C retention measurement"]
    fn measure_237_corpus_and_retention_tradeoff() {
        use crate::contract::{DocumentDto, PublicTagDto, ReaderResponse, WorkCard, WorkFilters};
        use std::io::{self, Write};
        use std::time::Instant;
        use unicode_normalization::UnicodeNormalization;

        struct ByteCounter(usize);
        impl Write for ByteCounter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0 += buf.len();
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        fn snapshot_json_size(snap: &Snapshot) -> usize {
            let mut c = ByteCounter(0);
            match snap {
                Snapshot::Works(w) => serde_json::to_writer(&mut c, w).unwrap(),
                Snapshot::Publication(p) => serde_json::to_writer(&mut c, p).unwrap(),
                Snapshot::Control(ctrl) => serde_json::to_writer(&mut c, ctrl).unwrap(),
            }
            c.0
        }

        fn corpus_dto(shape: &str, scalars: usize) -> DocumentDto {
            let alphabet: Vec<char> = match shape {
                "ascii_repeat" => "a",
                "emoji" => "😀🙂🌧🌿🌙🖋📚🎶",
                "escaping" => "a\"\\\t<&>",
                "tiny_blocks" => "abcdefghijklmnop",
                _ => "aăâbcdđeêghiklmnoôơpqrstuưvxyáàảãạắằẳẵặấầẩẫậéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ",
            }
            .chars()
            .collect();
            let mut state: u64 = 229;
            let chars: Vec<char> = (0..scalars)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    alphabet[((state >> 32) as usize) % alphabet.len()]
                })
                .collect();
            let lines: Vec<String> = if shape == "poem_tabs" {
                chars
                    .chunks(64)
                    .map(|chunk| {
                        let mut s: String = chunk.iter().collect();
                        if chunk.len() > 1 {
                            s.replace_range(..s.chars().next().unwrap().len_utf8(), "\t");
                        }
                        s
                    })
                    .collect()
            } else if shape == "tiny_blocks" {
                chars.iter().map(char::to_string).collect()
            } else {
                vec![chars.iter().collect()]
            };
            let block_lines = if shape == "poem_tabs" { 4 } else { 1 };
            crate::document::default_document(
                &lines
                    .chunks(block_lines)
                    .map(|chunk| {
                        chunk
                            .iter()
                            .map(|s| {
                                if shape == "vi_nfd" {
                                    s.nfd().collect()
                                } else {
                                    s.clone()
                                }
                            })
                            .collect::<Vec<String>>()
                            .join("\n")
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        }

        fn wrap_pub(id: &str, doc: DocumentDto) -> Snapshot {
            Snapshot::Publication(Publication {
                work_id: id.into(),
                revision_id: "00000000-0000-4000-8000-000000000001".into(),
                title: format!("[Mẫu] Kiểm thử {id}"),
                author_display_name: "Tác giả kiểm thử".into(),
                author: None,
                published_at: "2026-11-17T00:00:00Z".into(),
                document: doc,
                tags: vec![
                    PublicTagDto {
                        tag_id: "00000000-0000-4000-8000-000000000010".into(),
                        facet: "form".into(),
                        label: "Thơ".into(),
                        slug: "tho".into(),
                    },
                    PublicTagDto {
                        tag_id: "00000000-0000-4000-8000-000000000011".into(),
                        facet: "theme".into(),
                        label: "Ký ức".into(),
                        slug: "ky-uc".into(),
                    },
                ],
            })
        }

        fn struct_stats(snap: &Snapshot) -> (usize, usize, usize, usize, usize) {
            match snap {
                Snapshot::Publication(p) => {
                    let meta = p.work_id.len()
                        + p.revision_id.len()
                        + p.title.len()
                        + p.author_display_name.len()
                        + p.published_at.len()
                        + p.tags
                            .iter()
                            .map(|t| t.tag_id.len() + t.facet.len() + t.label.len() + t.slug.len())
                            .sum::<usize>();
                    // Rendered block count is diagnostic only: the retained DTO is one string.
                    let blocks = crate::document::paragraphs(&p.document.source).len();
                    let doc_utf8 = p.document.source.len();
                    let utf8 = meta + doc_utf8;
                    let wasm32_proxy = utf8 + 256 + 28 + p.tags.len() * 64;
                    let native64_proxy = utf8 + 512 + 48 + p.tags.len() * 128;
                    let logical_proxy = utf8 + 220 + 28 + p.tags.len() * 80;
                    (utf8, logical_proxy, wasm32_proxy, native64_proxy, blocks)
                }
                Snapshot::Works(w) => {
                    let cards_utf8: usize = w
                        .works
                        .iter()
                        .map(|c| {
                            c.work_id.len()
                                + c.title.len()
                                + c.excerpt.as_ref().map_or(0, String::len)
                                + c.author_display_name.len()
                                + c.published_at.len()
                                + c.first_published_at.as_ref().map_or(0, String::len)
                                + c.tags
                                    .iter()
                                    .map(|t| {
                                        t.tag_id.len()
                                            + t.facet.len()
                                            + t.label.len()
                                            + t.slug.len()
                                    })
                                    .sum::<usize>()
                        })
                        .sum();
                    let utf8 = cards_utf8
                        + w.next_cursor.as_ref().map_or(0, String::len)
                        + w.filters.q.as_ref().map_or(0, String::len);
                    let logical = utf8 + 64 + w.works.len() * 96;
                    let wasm32 = utf8 + 128 + w.works.len() * 160;
                    let native64 = utf8 + 256 + w.works.len() * 320;
                    (utf8, logical, wasm32, native64, w.works.len())
                }
                Snapshot::Control(_) => (0, 0, 0, 0, 0),
            }
        }

        fn make_list(n: usize) -> Snapshot {
            Snapshot::Works(WorkList {
                works: (0..n)
                    .map(|i| WorkCard {
                        work_id: format!("00000000-0000-4000-8000-{i:012}"),
                        title: format!("[Mẫu #100] Kiểm thử {i}"),
                        excerpt: None,
                        author_display_name: "Tác giả kiểm thử".into(),
                        author: None,
                        published_at: "2026-11-17T00:00:00Z".into(),
                        first_published_at: Some("2026-11-16T00:00:00Z".into()),
                        tags: vec![
                            PublicTagDto {
                                tag_id: "00000000-0000-4000-8000-000000000010".into(),
                                facet: "form".into(),
                                label: "Thơ".into(),
                                slug: "tho".into(),
                            },
                            PublicTagDto {
                                tag_id: "00000000-0000-4000-8000-000000000011".into(),
                                facet: "theme".into(),
                                label: "Ký ức".into(),
                                slug: "ky-uc".into(),
                            },
                        ],
                    })
                    .collect(),
                filters: WorkFilters {
                    q: None,
                    tags: vec![],
                },
                next_cursor: Some("cursor-opaque-token".into()),
            })
        }

        println!(
            "case,scalars,blocks,utf8_B,json_B,logical_walk_B,wasm32_proxy_B,native64_proxy_B,production_weight_B,clone_us,walk_us,json_count_us,decode_us"
        );
        // #234 baseline rows
        let list12 = make_list(12);
        let list20 = make_list(20);
        let baseline_pub = wrap_pub(
            "00000000-0000-4000-8000-000000000001",
            crate::document::default_document("Một dòng công khai dùng riêng cho kiểm thử #100."),
        );
        // The canonical provider admits at most 2 MiB of exact source. Match the live
        // two-paragraph scratch fixture: its blank lines are preserved in transport.
        let first = "Phố cũ mưa giăng sợi mỏng,";
        let last = "đã theo mưa về rất lâu.";
        let source = format!(
            "{first}{}{last}",
            "\n".repeat(2_097_152 - first.len() - last.len())
        );
        let max_source = wrap_pub("max-raw-source", crate::document::default_document(&source));
        for (name, scalars, snap) in [
            ("list_12_cards", 0, list12),
            ("list_20_cards", 0, list20),
            ("pub_234_single_line", 0, baseline_pub),
            ("nfd_oversized", 200_000, oversized_pub("nfd")),
            (
                "max_raw_source",
                first.chars().count() + last.chars().count(),
                max_source,
            ),
        ] {
            let (utf8, logical, wasm32, native64, blocks) = struct_stats(&snap);
            let json_b = snapshot_json_size(&snap);
            let t0 = Instant::now();
            let cloned = std::hint::black_box(snap.clone());
            drop(cloned);
            let clone_us = t0.elapsed().as_secs_f64() * 1e6;
            let t1 = Instant::now();
            let production_weight =
                std::hint::black_box(super::snapshot_weight_bytes(std::hint::black_box(&snap)));
            let walk_us = t1.elapsed().as_secs_f64() * 1e6;
            let t2 = Instant::now();
            std::hint::black_box(snapshot_json_size(std::hint::black_box(&snap)));
            let json_us = t2.elapsed().as_secs_f64() * 1e6;
            let decode_us = match &snap {
                Snapshot::Publication(p) => {
                    let encoded =
                        serde_json::to_vec(&ReaderResponse::Published(p.clone())).unwrap();
                    let t3 = Instant::now();
                    let decoded: ReaderResponse =
                        serde_json::from_slice(std::hint::black_box(&encoded)).unwrap();
                    std::hint::black_box(decoded);
                    t3.elapsed().as_secs_f64() * 1e6
                }
                Snapshot::Works(w) => {
                    let encoded = serde_json::to_vec(w).unwrap();
                    let t3 = Instant::now();
                    let decoded: WorkList =
                        serde_json::from_slice(std::hint::black_box(&encoded)).unwrap();
                    std::hint::black_box(decoded);
                    t3.elapsed().as_secs_f64() * 1e6
                }
                Snapshot::Control(_) => 0.0,
            };
            println!(
                "{name},{scalars},{blocks},{utf8},{json_b},{logical},{wasm32},{native64},{production_weight},{clone_us:.1},{walk_us:.1},{json_us:.1},{decode_us:.1}"
            );
        }

        let shapes = [
            "ascii_repeat",
            "vi_nfc",
            "vi_nfd",
            "emoji",
            "poem_tabs",
            "tiny_blocks",
            "escaping",
        ];
        for scalars in [30_000usize, 100_000, 200_000] {
            for shape in shapes {
                let doc = corpus_dto(shape, scalars);
                let snap = wrap_pub(&format!("{shape}-{scalars}"), doc);
                let (utf8, logical, wasm32, native64, blocks) = struct_stats(&snap);
                let json_b = snapshot_json_size(&snap);
                let t0 = Instant::now();
                let cloned = std::hint::black_box(snap.clone());
                drop(cloned);
                let clone_us = t0.elapsed().as_secs_f64() * 1e6;
                let t1 = Instant::now();
                let production_weight =
                    std::hint::black_box(super::snapshot_weight_bytes(std::hint::black_box(&snap)));
                let walk_us = t1.elapsed().as_secs_f64() * 1e6;
                let t2 = Instant::now();
                std::hint::black_box(snapshot_json_size(std::hint::black_box(&snap)));
                let json_us = t2.elapsed().as_secs_f64() * 1e6;
                let Snapshot::Publication(p) = &snap else {
                    unreachable!()
                };
                let encoded = serde_json::to_vec(&ReaderResponse::Published(p.clone())).unwrap();
                let t3 = Instant::now();
                let decoded: ReaderResponse =
                    serde_json::from_slice(std::hint::black_box(&encoded)).unwrap();
                std::hint::black_box(decoded);
                let decode_us = t3.elapsed().as_secs_f64() * 1e6;
                println!(
                    "{shape}-{scalars},{scalars},{blocks},{utf8},{json_b},{logical},{wasm32},{native64},{production_weight},{clone_us:.1},{walk_us:.1},{json_us:.1},{decode_us:.1}"
                );
            }
        }
        // The production weight is reported separately from the exploratory structural proxies.
        println!("--- A/B/C 50-loop simulation (same corpus) ---");
        let work_list = make_list(12);
        let list_key = PublicKey::Works(ExploreQuery::default());
        let corpus_scenarios: [(&str, Vec<Snapshot>); 3] = [
            (
                "normal_234_16_works",
                (0..16)
                    .map(|i| {
                        wrap_pub(
                            &format!("234-{i}"),
                            crate::document::default_document(
                                "Một dòng công khai dùng riêng cho kiểm thử #100.",
                            ),
                        )
                    })
                    .collect(),
            ),
            (
                "mixed_229_200k_16_works",
                (0..16)
                    .map(|i| {
                        let shape = if i < 8 {
                            "vi_nfc"
                        } else if i < 12 {
                            "poem_tabs"
                        } else {
                            "emoji"
                        };
                        wrap_pub(&format!("mix-{i}"), corpus_dto(shape, 200_000))
                    })
                    .collect(),
            ),
            (
                "worst_tiny_blocks_200k_4_works",
                (0..4)
                    .map(|i| wrap_pub(&format!("tiny-{i}"), corpus_dto("tiny_blocks", 200_000)))
                    .collect(),
            ),
        ];

        for (scenario_name, items) in corpus_scenarios {
            let mut coord_a = Coordinator::with_cache(
                Policy::V1,
                InMemoryCache::with_budget(
                    crate::read_cache::RetentionBudget::UNBOUNDED,
                    super::snapshot_weight_bytes,
                ),
            );
            let mut coord_b = Coordinator::new(Policy::V1);
            let mut coord_c = Coordinator::with_cache(Policy::V1, NoOpCache);

            let mut hits = [0usize; 3];
            let mut misses = [0usize; 3];
            let mut peak_a = 0usize;
            let mut peak_b = 0usize;
            for step in 0..50usize {
                let t = (step * 2) as f64;
                for (idx_stat, hit) in [
                    drive_cache_visit(&mut coord_a, &list_key, &work_list, t),
                    drive_cache_visit(&mut coord_b, &list_key, &work_list, t),
                    drive_cache_visit(&mut coord_c, &list_key, &work_list, t),
                ]
                .into_iter()
                .enumerate()
                {
                    if hit {
                        hits[idx_stat] += 1;
                    } else {
                        misses[idx_stat] += 1;
                    }
                }

                let idx = step % items.len();
                let item = &items[idx];
                let Snapshot::Publication(publication) = item else {
                    unreachable!("reader corpus contains publications")
                };
                let rk = PublicKey::Reader(publication.work_id.clone());
                for (idx_stat, hit) in [
                    drive_cache_visit(&mut coord_a, &rk, item, t + 1.0),
                    drive_cache_visit(&mut coord_b, &rk, item, t + 1.0),
                    drive_cache_visit(&mut coord_c, &rk, item, t + 1.0),
                ]
                .into_iter()
                .enumerate()
                {
                    if hit {
                        hits[idx_stat] += 1;
                    } else {
                        misses[idx_stat] += 1;
                    }
                }

                peak_a = peak_a.max(coord_a.store.cache().total_bytes());
                peak_b = peak_b.max(coord_b.store.cache().total_bytes());
            }
            println!(
                "sim,{scenario_name},A_peak_B={peak_a},A_entries={},A_hits={},A_misses={},B_peak_B={peak_b},B_entries={},B_hits={},B_misses={},B_evictions={},B_skips={},C_peak_B=0,C_hits={},C_misses={}",
                coord_a.store.cache().len(),
                hits[0],
                misses[0],
                coord_b.store.cache().len(),
                hits[1],
                misses[1],
                coord_b.store.cache().evictions(),
                coord_b.store.cache().oversized_skips(),
                hits[2],
                misses[2]
            );
        }
    }
}
