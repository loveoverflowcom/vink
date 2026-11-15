//! READ-02: freshness of the public snapshots one browser tab already holds.
//!
//! Pure policy and state machine, without a clock, timers, HTTP or Leptos. The browser owner
//! (`public_reads`) supplies a monotonic time (`performance.now()`), entropy for jitter and the
//! transport outcome; SSR never uses this store, so nothing is shared across requests.
//!
//! Three durations stay separate: `stale_after` only says when to check again (it never deletes
//! data), `retain_unobserved` bounds memory for snapshots no view shows, and `max_display_age`
//! bounds how old a snapshot may be to render instantly on a *new* visit. Data on screen is never
//! removed by a temporary failure, only by an authoritative removal (`Outcome::Removed`).

use crate::read_cache::{InMemoryCache, SnapshotCache};
use std::collections::BTreeMap;

/// Milliseconds on a monotonic process clock.
pub type Millis = f64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    /// Age after which an observed snapshot is checked again. Not a TTL.
    pub stale_after: Millis,
    /// Upper bound of the per-entry spread added to the periodic check, so entries validated
    /// together do not refresh in lockstep.
    pub jitter: Millis,
    /// Oldest validation that may still render instantly when a view mounts.
    pub max_display_age: Millis,
    /// How long a snapshot no view observes is kept.
    pub retain_unobserved: Millis,
    /// Upper bound on entries; only unobserved ones are evicted.
    pub max_entries: usize,
    /// First retry delay after a temporary failure; doubles per failure.
    pub retry_base: Millis,
    /// Largest retry delay, also the cap applied to a server `Retry-After`.
    pub retry_max: Millis,
}

impl Policy {
    /// v1, provisional: see `docs/development/public-read-freshness.md`.
    pub const V1: Self = Self {
        stale_after: 120_000.0,
        jitter: 15_000.0,
        max_display_age: 15.0 * 60_000.0,
        retain_unobserved: 5.0 * 60_000.0,
        max_entries: 16,
        retry_base: 5_000.0,
        retry_max: 120_000.0,
    };
}

/// Why a view asks for a check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// The scheduler's single timer fired: stale threshold plus this entry's spread.
    Due,
    /// Mount, focus, becoming visible or `online`: revalidate only if stale.
    Signal,
    /// The reader asked explicitly: skip the app's threshold and backoff (still single-flight).
    Manual,
}

/// Validated query/filter fields reported by discovery HTTP 422 responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InputField {
    Query,
    Tag,
    Limit,
    Parameter,
}

/// Semantic public-read rejection derived strictly from `status + error.code + error.field`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Rejection {
    /// Cursor malformed or scoped to another search: stop retrying it and restart from page 1.
    RestartCursor,
    /// Query string, tag filter, page limit, or query parameter rejected by validation.
    InvalidInput(InputField),
    /// Missing/malformed error envelope, unknown error code, or unmapped 4xx: finite fallback.
    Fallback,
}

/// Classifies a non-retryable public-read rejection from status, `error.code`, and
/// `error.field`, without ever inspecting `error.message`.
pub fn classify_rejection(status: u16, code: Option<&str>, field: Option<&str>) -> Rejection {
    match (status, code, field) {
        (422, Some("validation_failed"), Some("cursor")) => Rejection::RestartCursor,
        (422, Some("validation_failed"), Some("q")) => Rejection::InvalidInput(InputField::Query),
        (422, Some("validation_failed"), Some("tag")) => Rejection::InvalidInput(InputField::Tag),
        (422, Some("validation_failed"), Some("limit")) => {
            Rejection::InvalidInput(InputField::Limit)
        }
        (422, Some("validation_failed"), Some("query")) => {
            Rejection::InvalidInput(InputField::Parameter)
        }
        _ => Rejection::Fallback,
    }
}

/// What the transport established about one request.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome<V> {
    /// A representation confirmed by the origin (a 200, or a 304 the HTTP cache merged).
    Fresh(V),
    /// Authoritative absence (404): the snapshot must not be shown again.
    Removed,
    /// Non-retryable client/input/cursor rejection (`400`, `414`, `422`): stops polling.
    Rejected(Rejection),
    /// Timeout, network, 408/429/5xx or an unexpected response; `retry_after` from the server if any.
    Temporary { retry_after: Option<Millis> },
}

impl<V> Outcome<V> {
    pub fn map<W>(self, f: impl FnOnce(V) -> W) -> Outcome<W> {
        match self {
            Self::Fresh(value) => Outcome::Fresh(f(value)),
            Self::Removed => Outcome::Removed,
            Self::Rejected(rejection) => Outcome::Rejected(rejection),
            Self::Temporary { retry_after } => Outcome::Temporary { retry_after },
        }
    }
}

/// How the public-read transport treats a status the application received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicStatus {
    /// 2xx with a body (a 304 the browser HTTP cache merged arrives as 200).
    Body,
    /// A raw 304 reached the application: there is no body to use, so ask once unconditionally.
    Unconditional,
    /// Authoritative absence.
    Removed,
    /// Non-retryable client rejection (400, 414, 422).
    Rejected,
    /// Anything else (0, 408, 409, 429, 5xx), including a second raw 304: keep last success and back off.
    Temporary,
}

pub fn public_status(status: u16) -> PublicStatus {
    match status {
        200..=299 => PublicStatus::Body,
        304 => PublicStatus::Unconditional,
        404 => PublicStatus::Removed,
        400 | 414 | 422 => PublicStatus::Rejected,
        _ => PublicStatus::Temporary,
    }
}

/// `Retry-After` as delta-seconds; an HTTP-date or anything malformed is ignored (the
/// backoff still applies).
pub fn retry_after_ms(value: &str) -> Option<Millis> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= 9 && value.bytes().all(|b| b.is_ascii_digit()))
        .then(|| value.parse::<u32>().ok())
        .flatten()
        .map(|seconds| f64::from(seconds) * 1_000.0)
}

/// Identifies one physical request. Only the newest ticket of an entry may commit.
pub type Ticket = crate::read_lifecycle::RequestId;

/// What a view should do on mount.
#[derive(Clone, Debug, PartialEq)]
pub enum Load<V> {
    /// A snapshot recent enough to render now; revalidate with [`Trigger::Signal`] afterwards.
    Ready(V),
    /// No displayable snapshot: this caller owns the request.
    Fetch(Ticket),
    /// No displayable snapshot and a request is already running: wait for it.
    Join,
    /// Terminal client/input/cursor rejection without retry loop; manual retry still clears it.
    Rejected(Rejection),
}

/// The effect of a completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// Committed: the snapshot was replaced (`changed`) or confirmed unchanged.
    Fresh { changed: bool },
    /// Committed: entry dropped because the origin says it no longer exists.
    Removed,
    /// Committed: non-retryable rejection recorded, automatic polling stopped.
    Rejected(Rejection),
    /// Committed: failure recorded, last success kept.
    Failed,
    /// A superseded ticket (invalidated, removed, cleared or replaced); nothing changed.
    Discarded,
}

/// The projection a view renders.
#[derive(Clone, Debug, PartialEq)]
pub struct Status<V> {
    pub data: Option<V>,
    /// A request is running while a snapshot is shown.
    pub refreshing: bool,
    /// The last attempt failed temporarily; `data` is the last success.
    pub failed: bool,
    /// Non-retryable rejection recorded for this key (e.g. background refresh hit 422 cursor).
    pub rejected: Option<Rejection>,
}

#[derive(Clone, Debug)]
struct Entry<V> {
    data: Option<V>,
    cached_unverified: bool,
    /// Start of the request that confirmed `data`: a conservative lower bound.
    validated_at: Millis,
    /// Cached snapshot may lag origin state (`mark_stale`): refreshes once due, without clearing backoff.
    stale: bool,
    /// Confirmed server write (`mutation_confirmed`): permits ONE immediate check even during backoff.
    immediate_retry: bool,
    /// Non-retryable client/cursor/query rejection (`422` etc.); suppresses automatic retries.
    rejected: Option<Rejection>,
    in_flight: Option<(Ticket, Millis)>,
    failures: u32,
    retry_at: Millis,
    spread: Millis,
    observers: u32,
    last_seen: Millis,
}

impl<V> Entry<V> {
    fn empty(now: Millis) -> Self {
        Self {
            data: None,
            cached_unverified: false,
            validated_at: Millis::NEG_INFINITY,
            stale: false,
            immediate_retry: false,
            rejected: None,
            in_flight: None,
            failures: 0,
            retry_at: Millis::NEG_INFINITY,
            spread: 0.0,
            observers: 0,
            last_seen: now,
        }
    }

    /// When a scheduled or signalled check becomes due, if this entry is refreshable at all.
    fn check_at(&self, policy: &Policy, with_spread: bool) -> Option<Millis> {
        if (self.data.is_none() && !self.immediate_retry)
            || self.in_flight.is_some()
            || self.rejected.is_some()
        {
            return None;
        }
        Some(if self.immediate_retry {
            Millis::NEG_INFINITY
        } else if self.failures > 0 {
            self.retry_at
        } else if self.stale {
            Millis::NEG_INFINITY
        } else {
            self.validated_at + policy.stale_after + if with_spread { self.spread } else { 0.0 }
        })
    }
}

#[derive(Clone, Debug)]
pub struct Store<K, V, C = InMemoryCache<K, V>> {
    cache: C,
    reusable: fn(&V) -> bool,
    policy: Policy,
    entries: BTreeMap<K, Entry<V>>,
    /// Timestamp of authoritative `404` / `remove`: rejects older `seed` calls without creating
    /// a permanent tombstone against a new origin `Fresh` response on re-publication.
    removed: BTreeMap<K, Millis>,
    high_water: Millis,
}

impl<K: Ord + Clone, V: Clone + PartialEq> Store<K, V> {
    pub fn new(policy: Policy) -> Self {
        Self::with_cache(policy, InMemoryCache::default(), |_| true)
    }
}
impl<K: Ord + Clone, V: Clone + PartialEq, C: SnapshotCache<K, V>> Store<K, V, C> {
    pub fn with_cache(policy: Policy, cache: C, reusable: fn(&V) -> bool) -> Self {
        Self {
            policy,
            cache,
            reusable,
            entries: BTreeMap::new(),
            removed: BTreeMap::new(),
            high_water: Millis::NEG_INFINITY,
        }
    }
    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn record_time(&mut self, now: Millis) {
        if now.is_finite() {
            self.high_water = self.high_water.max(now);
        }
    }

    fn trim_removed(&mut self) {
        while self.removed.len() > self.policy.max_entries {
            let Some(oldest) = self
                .removed
                .iter()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.removed.remove(&oldest);
        }
    }

    fn entry(&mut self, key: &K, now: Millis) -> &mut Entry<V> {
        self.entries
            .entry(key.clone())
            .or_insert_with(|| Entry::empty(now))
    }

    fn start(&mut self, key: &K, now: Millis) -> Ticket {
        let ticket = Ticket::allocate();
        let entry = self.entry(key, now);
        entry.immediate_retry = false;
        entry.rejected = None;
        entry.in_flight = Some((ticket, now));
        ticket
    }

    /// A view mounts on `key`. Reading never extends `validated_at`.
    pub fn load(&mut self, key: &K, now: Millis) -> Load<V> {
        self.record_time(now);
        self.collect(now);
        let max_age = self.policy.max_display_age;
        let cached = self.cache.get(key);
        let reusable = self.reusable;
        let entry = self.entry(key, now);
        if entry.data.is_none() {
            entry.data = cached;
        }
        entry.last_seen = now;
        if let Some(rejection) = entry.rejected
            && entry.data.is_none()
        {
            return Load::Rejected(rejection);
        }
        if let Some(data) = &entry.data
            && reusable(data)
            && now - entry.validated_at <= max_age
        {
            entry.cached_unverified = true;
            return Load::Ready(data.clone());
        }
        if entry.in_flight.is_some() {
            return Load::Join;
        }
        Load::Fetch(self.start(key, now))
    }

    /// Records a server-rendered snapshot the page hydrated with. `rendered_at` must not be
    /// later than the server read (the page's navigation start is a safe bound). Never replaces
    /// a newer snapshot, a running request, or an authoritative removal at or after `rendered_at`.
    pub fn seed(&mut self, key: &K, data: V, rendered_at: Millis, now: Millis) {
        self.record_time(now);
        if let Some(&removed_at) = self.removed.get(key) {
            if rendered_at <= removed_at {
                return;
            }
            self.removed.remove(key);
        }
        let cached = self.cache.get(key);
        let entry = self.entry(key, now);
        if entry.data.is_none() {
            entry.data = cached;
        }
        if entry.in_flight.is_none()
            && entry.rejected.is_none()
            && (entry.validated_at == Millis::NEG_INFINITY || entry.validated_at < rendered_at)
        {
            entry.data = Some(data);
            entry.cached_unverified = false;
            entry.validated_at = rendered_at;
            entry.stale = false;
            entry.immediate_retry = false;
            entry.failures = 0;
            entry.retry_at = Millis::NEG_INFINITY;
        }
    }

    /// True when an authoritative removal was recorded at or after `rendered_at` and no newer
    /// fresh load has cleared it yet.
    pub fn is_removed_at(&self, key: &K, rendered_at: Millis) -> bool {
        self.removed
            .get(key)
            .is_some_and(|&removed_at| rendered_at <= removed_at)
    }

    /// An out-of-band origin read shares the entry's ticket fence and request-start clock.
    /// Its caller must complete it, including on cancellation.
    pub fn begin_external_read(&mut self, key: &K, now: Millis) -> Ticket {
        self.record_time(now);
        self.start(key, now)
    }

    pub fn is_current(&self, key: &K, ticket: Ticket) -> bool {
        self.entries
            .get(key)
            .and_then(|e| e.in_flight)
            .is_some_and(|(current, _)| current == ticket)
    }

    /// Starts a check if `trigger` warrants one; `None` coalesces with a running request or
    /// means the snapshot is fresh enough or backing off or rejected.
    pub fn begin(&mut self, key: &K, trigger: Trigger, now: Millis) -> Option<Ticket> {
        self.record_time(now);
        let policy = self.policy;
        let entry = self.entries.get(key)?;
        if entry.in_flight.is_some() {
            return None;
        }
        let due = match trigger {
            Trigger::Manual => true,
            Trigger::Due => entry.check_at(&policy, true).is_some_and(|at| now >= at),
            Trigger::Signal => entry.check_at(&policy, false).is_some_and(|at| now >= at),
        };
        due.then(|| self.start(key, now))
    }

    /// Commits `outcome` only for the entry's current ticket. `random` in `[0, 1)` spreads the
    /// next check or retry.
    pub fn complete(
        &mut self,
        key: &K,
        ticket: Ticket,
        outcome: Outcome<V>,
        now: Millis,
        random: f64,
    ) -> Applied {
        self.record_time(now);
        let policy = self.policy;
        let Some(entry) = self.entries.get_mut(key) else {
            return Applied::Discarded;
        };
        let Some((current, started_at)) = entry.in_flight else {
            return Applied::Discarded;
        };
        if current != ticket {
            return Applied::Discarded;
        }
        entry.in_flight = None;
        let random = random.clamp(0.0, 1.0);
        match outcome {
            Outcome::Fresh(data) => {
                self.removed.remove(key);
                self.cache.remove(key);
                let changed = entry.data.as_ref() != Some(&data);
                entry.data = Some(data);
                entry.cached_unverified = false;
                entry.validated_at = started_at;
                entry.stale = false;
                entry.immediate_retry = false;
                entry.rejected = None;
                entry.failures = 0;
                entry.retry_at = Millis::NEG_INFINITY;
                entry.spread = random * policy.jitter;
                Applied::Fresh { changed }
            }
            Outcome::Removed => {
                self.remove_at(key, now);
                Applied::Removed
            }
            Outcome::Rejected(rejection) => {
                self.cache.remove(key);
                entry.data = None;
                entry.stale = false;
                entry.immediate_retry = false;
                entry.failures = 0;
                entry.retry_at = Millis::NEG_INFINITY;
                entry.rejected = Some(rejection);
                Applied::Rejected(rejection)
            }
            Outcome::Temporary { retry_after } => {
                entry.immediate_retry = false;
                entry.failures = entry.failures.saturating_add(1);
                let exponent = entry.failures.saturating_sub(1).min(16) as i32;
                let backoff = (policy.retry_base * 2f64.powi(exponent)).min(policy.retry_max);
                // Equal jitter: at least 80 % of the backoff, never above it.
                let mut delay = backoff * (0.8 + 0.2 * random);
                if let Some(server) = retry_after {
                    delay = delay.max(server.clamp(0.0, policy.retry_max));
                }
                entry.retry_at = now + delay;
                Applied::Failed
            }
        }
    }

    /// Marks cached data stale without clearing any existing retry backoff (`failures`/`retry_at`).
    /// Supersedes any running request.
    pub fn mark_stale(&mut self, key: &K) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.stale = true;
            entry.in_flight = None;
        }
    }

    /// Generic invalidation over a scope: marks matching entries stale and supersedes running
    /// requests while preserving active retry backoff deadlines.
    pub fn mark_stale_where(&mut self, mut predicate: impl FnMut(&K) -> bool) {
        for (key, entry) in &mut self.entries {
            if predicate(key) {
                entry.stale = true;
                entry.in_flight = None;
            }
        }
    }

    /// Confirmed server mutation: supersedes any running request and grants ONE immediate check
    /// (consumed when that attempt starts). If that attempt fails temporarily, backoff resumes.
    pub fn mutation_confirmed(&mut self, key: &K) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.stale = true;
            entry.immediate_retry = true;
            entry.in_flight = None;
        }
    }

    /// Confirmed server mutation across a matching scope.
    pub fn mutation_confirmed_where(&mut self, mut predicate: impl FnMut(&K) -> bool) {
        for (key, entry) in &mut self.entries {
            if predicate(key) {
                entry.stale = true;
                entry.immediate_retry = true;
                entry.in_flight = None;
            }
        }
    }

    /// Drop `key`, supersede its running request, and record removal time `now` to reject older
    /// `seed` calls without blocking new origin loads on future re-publication.
    pub fn remove_at(&mut self, key: &K, now: Millis) {
        self.record_time(now);
        self.entries.remove(key);
        self.cache.remove(key);
        let fence = if self.high_water.is_finite() {
            self.high_water.max(now)
        } else {
            now
        };
        self.removed.insert(key.clone(), fence);
        self.trim_removed();
    }

    /// Drop `key` and supersede its running request using the store's high-water clock.
    pub fn remove(&mut self, key: &K) {
        let at = if self.high_water.is_finite() {
            self.high_water
        } else {
            0.0
        };
        self.remove_at(key, at);
    }

    /// Scope reset (for example another account): every snapshot and request is superseded.
    /// Tickets keep increasing, so no earlier request can match a later entry.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cache.clear();
        self.removed.clear();
    }

    /// Edits a held snapshot without confirming it (a derived preview of an authoritative
    /// removal elsewhere). Existing backoff deadlines remain intact.
    pub fn edit(&mut self, key: &K, edit: impl FnOnce(&mut V)) {
        if let Some(entry) = self.entries.get_mut(key) {
            if entry.data.is_none() {
                entry.data = self.cache.get(key);
            }
            self.cache.remove(key);
            if let Some(data) = &mut entry.data {
                edit(data);
            }
        }
    }

    /// Update every retained derived snapshot, including unobserved searches. Preserves existing
    /// retry backoff deadlines while marking changed entries stale.
    pub fn edit_all(&mut self, mut edit: impl FnMut(&K, &mut V)) {
        for (key, entry) in &mut self.entries {
            if entry.data.is_none() {
                entry.data = self.cache.get(key);
            }
            self.cache.remove(key);
            if let Some(data) = &mut entry.data {
                let before = data.clone();
                edit(key, data);
                if *data != before {
                    entry.stale = true;
                    entry.in_flight = None;
                }
            }
        }
    }

    pub fn observer_count(&self, key: &K) -> u32 {
        self.entries.get(key).map_or(0, |e| e.observers)
    }

    pub fn observe(&mut self, key: &K, now: Millis) {
        self.record_time(now);
        let entry = self.entry(key, now);
        entry.observers += 1;
        entry.last_seen = now;
    }

    /// End an observation and fence its abandoned flight. Snapshot retention is optional;
    /// the coordinator's rejection, age and removal facts do not belong to the cache.
    pub fn release(&mut self, key: &K, now: Millis) {
        self.record_time(now);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.observers = entry.observers.saturating_sub(1);
            entry.last_seen = now;
            if entry.observers == 0 {
                entry.in_flight = None;
                if let Some(data) = entry.data.take() {
                    if (self.reusable)(&data) {
                        self.cache.put(key.clone(), data);
                    } else {
                        self.cache.remove(key);
                    }
                }
            }
        }
        self.collect(now);
    }

    pub fn cached_unverified(&self, key: &K) -> bool {
        self.entries.get(key).is_some_and(|e| e.cached_unverified)
    }

    pub fn has_in_flight(&self, key: &K) -> bool {
        self.entries.get(key).is_some_and(|e| e.in_flight.is_some())
    }

    pub fn needs_fetch_when_waiting(&self, key: &K) -> bool {
        self.entries
            .get(key)
            .is_none_or(|e| e.data.is_none() && e.in_flight.is_none() && e.rejected.is_none())
    }

    pub fn matching_observed(&self, mut predicate: impl FnMut(&K) -> bool) -> Vec<K> {
        self.entries
            .iter()
            .filter(|(k, e)| e.observers > 0 && predicate(k))
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn status(&self, key: &K) -> Option<Status<V>> {
        self.entries.get(key).map(|entry| Status {
            refreshing: entry.data.is_some() && entry.in_flight.is_some(),
            failed: entry.data.is_some() && entry.failures > 0,
            rejected: entry.rejected,
            data: entry.data.clone().or_else(|| self.cache.get(key)),
        })
    }

    /// Observed keys whose scheduled check is due. The owner calls this only while visible.
    pub fn due(&self, now: Millis) -> Vec<K> {
        self.entries
            .iter()
            .filter(|(_, e)| e.observers > 0)
            .filter(|(_, e)| e.check_at(&self.policy, true).is_some_and(|at| now >= at))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Observed keys a lifecycle signal should revalidate.
    pub fn observed(&self) -> Vec<K> {
        self.entries
            .iter()
            .filter(|(_, e)| e.observers > 0)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// When the single scheduler timer should fire next; `None` when nothing observed can be
    /// refreshed (no snapshot, rejected, or every request is running).
    pub fn next_wake(&self, now: Millis) -> Option<Millis> {
        self.entries
            .values()
            .filter(|e| e.observers > 0)
            .filter_map(|e| e.check_at(&self.policy, true))
            .map(|at| at.max(now))
            .min_by(|a, b| a.total_cmp(b))
    }

    /// Drop unobserved, idle entries past retention, then the least recently seen unobserved
    /// ones above `max_entries`. Also expires and bounds removal fences.
    pub fn collect(&mut self, now: Millis) {
        self.record_time(now);
        let retain = self.policy.retain_unobserved;
        self.removed.retain(|_, at| now - *at < retain);
        self.trim_removed();
        self.entries.retain(|key, e| {
            let keep = e.observers > 0 || e.in_flight.is_some() || now - e.last_seen < retain;
            if !keep {
                self.cache.remove(key);
            }
            keep
        });
        while self.entries.len() > self.policy.max_entries {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, e)| e.observers == 0 && e.in_flight.is_none())
                .min_by(|a, b| a.1.last_seen.total_cmp(&b.1.last_seen))
                .map(|(k, _)| k.clone());
            match oldest {
                Some(key) => {
                    self.entries.remove(&key);
                    self.cache.remove(&key);
                }
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Millis = 1_000.0;

    fn store() -> Store<&'static str, u32> {
        Store::new(Policy::V1)
    }

    fn fetched(store: &mut Store<&'static str, u32>, key: &'static str, value: u32, now: Millis) {
        let Load::Fetch(ticket) = store.load(&key, now) else {
            panic!("expected a fetch");
        };
        assert_eq!(
            store.complete(&key, ticket, Outcome::Fresh(value), now, 0.0),
            Applied::Fresh { changed: true }
        );
    }

    #[test]
    fn stale_threshold_is_a_check_point_not_a_ttl_and_reads_do_not_extend_it() {
        let mut s = store();
        s.observe(&"works", 0.0);
        fetched(&mut s, "works", 1, 0.0);
        assert_eq!(s.begin(&"works", Trigger::Signal, 119.0 * S), None);
        // Reading repeatedly does not move validated_at.
        for t in [30.0, 60.0, 119.9] {
            assert_eq!(s.load(&"works", t * S), Load::Ready(1));
        }
        assert!(s.begin(&"works", Trigger::Signal, 120.0 * S).is_some());
        // Stale data is still shown while it is checked.
        assert_eq!(
            s.status(&"works"),
            Some(Status {
                data: Some(1),
                refreshing: true,
                failed: false,
                rejected: None,
            })
        );
        assert_eq!(s.load(&"works", 180.0 * S), Load::Ready(1));
    }

    #[test]
    fn scheduled_check_adds_bounded_spread_and_one_wake_covers_all_entries() {
        let mut s = store();
        s.observe(&"a", 0.0);
        s.observe(&"b", 0.0);
        for (key, random) in [("a", 0.0), ("b", 0.99)] {
            let Load::Fetch(t) = s.load(&key, 0.0) else {
                panic!()
            };
            s.complete(&key, t, Outcome::Fresh(1), 0.0, random);
        }
        assert_eq!(s.next_wake(0.0), Some(120.0 * S));
        assert_eq!(s.due(120.0 * S), vec!["a"]);
        assert!(s.begin(&"a", Trigger::Due, 120.0 * S).is_some());
        // "a" is in flight: the next wake is "b" at 120 s + 0.99 × 15 s.
        let wake = s.next_wake(120.0 * S).unwrap();
        assert!(wake > 134.0 * S && wake < 135.0 * S, "{wake}");
        assert_eq!(s.begin(&"b", Trigger::Due, 121.0 * S), None);
        // A lifecycle signal uses the plain threshold.
        assert!(s.begin(&"b", Trigger::Signal, 121.0 * S).is_some());
    }

    #[test]
    fn concurrent_triggers_share_one_request() {
        let mut s = store();
        s.observe(&"works", 0.0);
        s.observe(&"works", 0.0);
        let Load::Fetch(first) = s.load(&"works", 0.0) else {
            panic!()
        };
        assert_eq!(s.load(&"works", 1.0), Load::Join);
        for trigger in [Trigger::Due, Trigger::Signal, Trigger::Manual] {
            assert_eq!(s.begin(&"works", trigger, 2.0), None, "{trigger:?}");
        }
        s.complete(&"works", first, Outcome::Fresh(1), 3.0, 0.0);
        let at = 200.0 * S;
        let focus = s.begin(&"works", Trigger::Signal, at);
        assert!(focus.is_some());
        assert_eq!(s.begin(&"works", Trigger::Due, at), None);
        assert_eq!(s.begin(&"works", Trigger::Signal, at), None);
        assert_eq!(s.due(at), Vec::<&str>::new());
        assert_eq!(s.next_wake(at), None);
    }

    #[test]
    fn manual_refresh_and_invalidation_are_not_swallowed_by_a_fresh_snapshot() {
        let mut s = store();
        s.observe(&"works", 0.0);
        fetched(&mut s, "works", 1, 0.0);
        assert!(s.begin(&"works", Trigger::Manual, 1.0).is_some());
        let mut s = store();
        s.observe(&"works", 0.0);
        fetched(&mut s, "works", 1, 0.0);
        s.mark_stale(&"works");
        assert_eq!(s.next_wake(1.0), Some(1.0));
        assert!(s.begin(&"works", Trigger::Signal, 1.0).is_some());
    }

    #[test]
    fn temporary_failure_keeps_last_success_and_backs_off_without_piling_up() {
        let mut s = store();
        s.observe(&"p", 0.0);
        fetched(&mut s, "p", 7, 0.0);
        let mut now = 120.0 * S;
        let mut delays = vec![];
        for _ in 0..7 {
            let t = s.begin(&"p", Trigger::Due, now).expect("due");
            assert_eq!(s.begin(&"p", Trigger::Due, now), None, "never two at once");
            assert_eq!(
                s.complete(&"p", t, Outcome::Temporary { retry_after: None }, now, 1.0),
                Applied::Failed
            );
            assert_eq!(
                s.status(&"p"),
                Some(Status {
                    data: Some(7),
                    refreshing: false,
                    failed: true,
                    rejected: None,
                })
            );
            let wake = s.next_wake(now).unwrap();
            assert_eq!(
                s.begin(&"p", Trigger::Signal, wake - 1.0),
                None,
                "backoff respected"
            );
            delays.push((wake - now) / S);
            now = wake;
        }
        assert_eq!(delays, [5.0, 10.0, 20.0, 40.0, 80.0, 120.0, 120.0]);
        // Server Retry-After raises the delay, capped by retry_max.
        let t = s.begin(&"p", Trigger::Due, now).unwrap();
        s.complete(
            &"p",
            t,
            Outcome::Temporary {
                retry_after: Some(900.0 * S),
            },
            now,
            1.0,
        );
        assert_eq!(s.next_wake(now), Some(now + 120.0 * S));
        // Success resets the backoff and the snapshot.
        let t = s.begin(&"p", Trigger::Manual, now).unwrap();
        assert_eq!(
            s.complete(&"p", t, Outcome::Fresh(7), now, 0.0),
            Applied::Fresh { changed: false }
        );
        assert!(!s.status(&"p").unwrap().failed);
    }

    #[test]
    fn jittered_retry_stays_within_bounds() {
        for random in [0.0, 0.5, 0.999] {
            let mut s = store();
            s.observe(&"p", 0.0);
            fetched(&mut s, "p", 1, 0.0);
            let t = s.begin(&"p", Trigger::Manual, 0.0).unwrap();
            s.complete(
                &"p",
                t,
                Outcome::Temporary { retry_after: None },
                0.0,
                random,
            );
            let wake = s.next_wake(0.0).unwrap();
            assert!((4.0 * S..=5.0 * S).contains(&wake), "{random}: {wake}");
        }
    }

    #[test]
    fn authoritative_removal_deletes_and_late_responses_cannot_resurrect() {
        let mut s = store();
        s.observe(&"p", 0.0);
        fetched(&mut s, "p", 1, 0.0);
        let early = s.begin(&"p", Trigger::Manual, 1.0).unwrap();
        // An invalidation supersedes the running request …
        s.mark_stale(&"p");
        let late = s.begin(&"p", Trigger::Signal, 2.0).unwrap();
        assert_eq!(
            s.complete(&"p", early, Outcome::Fresh(99), 3.0, 0.0),
            Applied::Discarded
        );
        // … and a 404 deletes the entry, so neither response may bring it back.
        assert_eq!(
            s.complete(&"p", late, Outcome::Removed, 4.0, 0.0),
            Applied::Removed
        );
        assert_eq!(s.status(&"p"), None);
        assert_eq!(
            s.complete(&"p", late, Outcome::Fresh(1), 5.0, 0.0),
            Applied::Discarded
        );
        assert_eq!(
            s.complete(&"p", early, Outcome::Fresh(1), 5.0, 0.0),
            Applied::Discarded
        );
        // A new visit fetches again instead of rendering the removed snapshot.
        assert!(matches!(s.load(&"p", 6.0), Load::Fetch(_)));
    }

    #[test]
    fn scope_reset_and_remove_supersede_running_requests() {
        let mut s = store();
        let Load::Fetch(old) = s.load(&"works", 0.0) else {
            panic!()
        };
        s.clear();
        let Load::Fetch(new) = s.load(&"works", 1.0) else {
            panic!()
        };
        assert_ne!(old, new);
        assert_eq!(
            s.complete(&"works", old, Outcome::Fresh(1), 2.0, 0.0),
            Applied::Discarded
        );
        assert_eq!(s.load(&"works", 2.0), Load::Join);
        s.remove(&"works");
        assert_eq!(
            s.complete(&"works", new, Outcome::Fresh(2), 3.0, 0.0),
            Applied::Discarded
        );
        assert!(s.is_empty());
    }

    #[test]
    fn freshness_dates_from_request_start_not_completion() {
        let mut s = store();
        s.observe(&"works", 0.0);
        let Load::Fetch(t) = s.load(&"works", 0.0) else {
            panic!()
        };
        s.complete(&"works", t, Outcome::Fresh(1), 30.0 * S, 0.0);
        assert!(s.begin(&"works", Trigger::Signal, 120.0 * S).is_some());
    }

    #[test]
    fn max_display_age_limits_instant_render_on_a_new_visit_only() {
        let mut s = store();
        s.observe(&"p", 0.0);
        fetched(&mut s, "p", 1, 0.0);
        // Offline: every check fails, yet the snapshot on screen stays.
        let mut now = 0.0;
        while now < 20.0 * 60.0 * S {
            now = s.next_wake(now).unwrap();
            let t = s.begin(&"p", Trigger::Due, now).unwrap();
            s.complete(&"p", t, Outcome::Temporary { retry_after: None }, now, 0.5);
        }
        assert_eq!(s.status(&"p").unwrap().data, Some(1));
        // Leaving and coming back: too old to render instantly, so the visit fetches.
        s.release(&"p", now);
        assert!(matches!(s.load(&"p", now + 1.0), Load::Fetch(_)));
    }

    #[test]
    fn seed_never_overrides_newer_data_or_running_requests() {
        let mut s = store();
        s.seed(&"works", 1, 0.0, 0.0);
        assert_eq!(s.load(&"works", 1.0), Load::Ready(1));
        fetched_after(&mut s, 2, 10.0);
        s.seed(&"works", 3, 5.0, 11.0);
        assert_eq!(s.status(&"works").unwrap().data, Some(2));
        s.observe(&"works", 11.0);
        let t = s.begin(&"works", Trigger::Manual, 12.0).unwrap();
        s.seed(&"works", 4, 13.0, 13.0);
        s.complete(&"works", t, Outcome::Fresh(5), 14.0, 0.0);
        assert_eq!(s.status(&"works").unwrap().data, Some(5));

        fn fetched_after(s: &mut Store<&'static str, u32>, v: u32, now: Millis) {
            let t = s.begin(&"works", Trigger::Manual, now).unwrap();
            s.complete(&"works", t, Outcome::Fresh(v), now, 0.0);
        }
    }

    #[test]
    fn unobserved_snapshots_are_retained_then_collected_and_bounded() {
        let mut s = store();
        s.observe(&"works", 0.0);
        fetched(&mut s, "works", 1, 0.0);
        s.release(&"works", 10.0 * S);
        assert_eq!(s.load(&"works", 60.0 * S), Load::Ready(1));
        s.collect(60.0 * S + Policy::V1.retain_unobserved);
        assert!(s.is_empty());

        let mut s: Store<u32, u32> = Store::new(Policy::V1);
        s.observe(&999, 0.0);
        for key in 0..40u32 {
            let Load::Fetch(t) = s.load(&key, f64::from(key)) else {
                panic!()
            };
            s.complete(&key, t, Outcome::Fresh(key), f64::from(key), 0.0);
            s.collect(f64::from(key));
        }
        assert_eq!(s.len(), Policy::V1.max_entries);
        assert!(
            s.status(&999).is_some(),
            "observed entries are never evicted"
        );
        assert!(s.status(&0).is_none() && s.status(&39).is_some());
    }

    #[test]
    fn transport_never_parses_a_raw_304_and_reads_only_delta_seconds() {
        assert_eq!(public_status(200), PublicStatus::Body);
        assert_eq!(public_status(304), PublicStatus::Unconditional);
        assert_eq!(public_status(404), PublicStatus::Removed);
        for status in [400, 414, 422] {
            assert_eq!(public_status(status), PublicStatus::Rejected, "{status}");
        }
        for status in [0, 401, 403, 408, 409, 429, 500, 503] {
            assert_eq!(public_status(status), PublicStatus::Temporary, "{status}");
        }
        assert_eq!(retry_after_ms(" 30 "), Some(30_000.0));
        for value in [
            "",
            "-1",
            "1.5",
            "Wed, 21 Oct 2026 07:28:00 GMT",
            "9999999999",
        ] {
            assert_eq!(retry_after_ms(value), None, "{value}");
        }
    }

    #[test]
    fn classify_rejection_maps_known_422_fields_without_reading_message() {
        assert_eq!(
            classify_rejection(422, Some("validation_failed"), Some("cursor")),
            Rejection::RestartCursor
        );
        assert_eq!(
            classify_rejection(422, Some("validation_failed"), Some("q")),
            Rejection::InvalidInput(InputField::Query)
        );
        assert_eq!(
            classify_rejection(422, Some("validation_failed"), Some("tag")),
            Rejection::InvalidInput(InputField::Tag)
        );
        assert_eq!(
            classify_rejection(422, Some("validation_failed"), Some("limit")),
            Rejection::InvalidInput(InputField::Limit)
        );
        assert_eq!(
            classify_rejection(422, Some("validation_failed"), Some("query")),
            Rejection::InvalidInput(InputField::Parameter)
        );
        // Unknown codes (even with field="cursor") and unknown/missing envelopes never trigger cursor restart.
        for case in [
            (422, Some("unknown_future_code"), Some("cursor")),
            (422, Some("validation_failed"), Some("unknown_field")),
            (422, Some("validation_failed"), None),
            (422, None, None),
            (400, Some("validation_failed"), Some("cursor")),
            (414, None, None),
        ] {
            assert_eq!(
                classify_rejection(case.0, case.1, case.2),
                Rejection::Fallback,
                "{case:?}"
            );
        }
    }

    #[test]
    fn rejected_outcome_stops_retries_and_only_manual_resets() {
        let mut s = store();
        s.observe(&"p2", 0.0);
        let Load::Fetch(first) = s.load(&"p2", 0.0) else {
            panic!()
        };
        assert_eq!(
            s.complete(
                &"p2",
                first,
                Outcome::Rejected(Rejection::RestartCursor),
                10.0,
                0.0
            ),
            Applied::Rejected(Rejection::RestartCursor)
        );
        assert_eq!(s.next_wake(10.0), None);
        assert!(s.due(500.0 * S).is_empty());
        assert_eq!(s.begin(&"p2", Trigger::Signal, 20.0), None);
        assert_eq!(s.begin(&"p2", Trigger::Due, 500.0 * S), None);
        assert_eq!(
            s.load(&"p2", 30.0),
            Load::Rejected(Rejection::RestartCursor)
        );
        s.mark_stale(&"p2");
        s.mutation_confirmed(&"p2");
        assert_eq!(s.next_wake(30.0), None);
        assert_eq!(s.begin(&"p2", Trigger::Signal, 40.0), None);
        assert_eq!(
            s.status(&"p2"),
            Some(Status {
                data: None,
                refreshing: false,
                failed: false,
                rejected: Some(Rejection::RestartCursor),
            })
        );
        // Background 422 on an already-rendered observed entry stops polling immediately.
        let mut bg = store();
        bg.observe(&"page2", 0.0);
        fetched(&mut bg, "page2", 1, 0.0);
        let t = bg.begin(&"page2", Trigger::Due, 120.0 * S).unwrap();
        assert_eq!(
            bg.complete(
                &"page2",
                t,
                Outcome::Rejected(Rejection::InvalidInput(InputField::Query)),
                121.0 * S,
                0.0,
            ),
            Applied::Rejected(Rejection::InvalidInput(InputField::Query))
        );
        assert_eq!(bg.next_wake(121.0 * S), None);
        assert_eq!(bg.begin(&"page2", Trigger::Signal, 200.0 * S), None);
        // Only an explicit manual retry starts a new flight.
        assert!(bg.begin(&"page2", Trigger::Manual, 201.0 * S).is_some());
    }

    #[test]
    fn mark_stale_and_mutation_ack_preserve_backoff_after_temporary_failure() {
        let mut s = store();
        s.observe(&"w", 0.0);
        fetched(&mut s, "w", 1, 0.0);

        // Repro: fresh -> mark_stale -> begin -> Temporary { retry_after: 30_000 }
        s.mark_stale(&"w");
        let t1 = s
            .begin(&"w", Trigger::Signal, 1_000.0)
            .expect("first check");
        assert_eq!(
            s.complete(
                &"w",
                t1,
                Outcome::Temporary {
                    retry_after: Some(30.0 * S),
                },
                1_100.0,
                0.0,
            ),
            Applied::Failed
        );
        // Retry deadline stays respected (never NEG_INFINITY / 1_100).
        assert_eq!(s.next_wake(1_100.0), Some(31_100.0));
        assert!(s.due(1_500.0).is_empty());
        assert_eq!(s.begin(&"w", Trigger::Signal, 1_500.0), None);
        assert_eq!(s.begin(&"w", Trigger::Due, 31_099.0), None);

        // Reading status, calling mark_stale again, or no-op edit_all cannot clear backoff.
        let _ = s.status(&"w");
        s.mark_stale(&"w");
        s.edit_all(|_, _| {});
        assert_eq!(s.next_wake(2_000.0), Some(31_100.0));
        assert_eq!(s.begin(&"w", Trigger::Signal, 2_000.0), None);

        // At retry_at, one flight fires; another temporary failure doubles backoff.
        let t2 = s.begin(&"w", Trigger::Due, 31_100.0).expect("retry_at due");
        assert_eq!(
            s.complete(
                &"w",
                t2,
                Outcome::Temporary { retry_after: None },
                31_200.0,
                1.0
            ),
            Applied::Failed
        );
        let wake2 = s.next_wake(31_200.0).expect("second backoff");
        assert_eq!(wake2, 31_200.0 + 10.0 * S);
        assert_eq!(s.begin(&"w", Trigger::Signal, wake2 - 1.0), None);

        // Confirmed mutation grants ONE immediate check even while backing off; if that attempt
        // also temporarily fails, backoff resumes from the accumulated failure count.
        s.mutation_confirmed(&"w");
        assert_eq!(s.next_wake(32_000.0), Some(32_000.0));
        let t3 = s
            .begin(&"w", Trigger::Signal, 32_000.0)
            .expect("mutation wake");
        assert_eq!(
            s.complete(
                &"w",
                t3,
                Outcome::Temporary { retry_after: None },
                32_100.0,
                1.0
            ),
            Applied::Failed
        );
        assert_eq!(s.next_wake(32_100.0), Some(32_100.0 + 20.0 * S));
        assert_eq!(s.begin(&"w", Trigger::Signal, 32_200.0), None);
    }

    #[test]
    fn removal_blocks_older_seeds_without_preventing_republication() {
        let mut s = store();
        s.observe(&"p", 0.0);
        s.seed(&"p", 10, 0.0, 10.0);
        assert_eq!(s.status(&"p").unwrap().data, Some(10));

        // Authoritative removal at t = 100.
        s.remove_at(&"p", 100.0);
        assert_eq!(s.status(&"p"), None);

        // Stale SSR/navigation seed rendered before or at removal cannot resurrect it.
        s.seed(&"p", 10, 0.0, 120.0);
        assert_eq!(s.status(&"p"), None);
        s.seed(&"p", 10, 100.0, 120.0);
        assert_eq!(s.status(&"p"), None);

        // Re-publication in the same tab queries origin and recovers cleanly on 200 Fresh.
        let Load::Fetch(repub) = s.load(&"p", 150.0) else {
            panic!("removed must not be a permanent tombstone")
        };
        assert_eq!(
            s.complete(&"p", repub, Outcome::Fresh(20), 160.0, 0.0),
            Applied::Fresh { changed: true }
        );
        assert_eq!(s.status(&"p").unwrap().data, Some(20));
    }

    #[test]
    fn unobserved_entries_are_never_polled() {
        let mut s = store();
        s.observe(&"works", 0.0);
        fetched(&mut s, "works", 1, 0.0);
        s.release(&"works", 1.0);
        assert_eq!(s.next_wake(500.0 * S), None);
        assert!(s.due(500.0 * S).is_empty());
        assert!(s.observed().is_empty());
    }

    #[test]
    fn edit_changes_a_preview_without_confirming_it() {
        let mut s = store();
        s.observe(&"works", 0.0);
        fetched(&mut s, "works", 10, 0.0);
        s.edit(&"works", |v| *v -= 1);
        assert_eq!(s.status(&"works").unwrap().data, Some(9));
        assert!(s.begin(&"works", Trigger::Signal, 120.0 * S).is_some());
    }

    /// Adversarial sequences (deterministic PRNG): at most one request per key, no superseded
    /// ticket commits, removal is never undone by an older ticket, and only a commit moves
    /// freshness.
    #[test]
    fn random_interleavings_keep_single_flight_and_generation_safety() {
        let mut seed: u64 = 0x5eed_cafe;
        let mut next = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for _ in 0..300 {
            let mut s: Store<u8, u32> = Store::new(Policy::V1);
            let mut tickets: Vec<(u8, Ticket)> = vec![];
            let mut dead: Vec<(u8, Ticket)> = vec![];
            let mut now = 0.0;
            let mut value = 0;
            for _ in 0..80 {
                now += (next(200) as f64) * S;
                let key = next(2) as u8;
                match next(9) {
                    0 => {
                        if let Load::Fetch(t) = s.load(&key, now) {
                            tickets.push((key, t));
                        }
                    }
                    1 => s.observe(&key, now),
                    2 => s.release(&key, now),
                    3 => {
                        let trigger =
                            [Trigger::Due, Trigger::Signal, Trigger::Manual][next(3) as usize];
                        if let Some(t) = s.begin(&key, trigger, now) {
                            tickets.push((key, t));
                        }
                    }
                    4 | 5 if !tickets.is_empty() => {
                        let (k, t) = tickets.remove(next(tickets.len() as u64) as usize);
                        value += 1;
                        let outcome = match next(5) {
                            0 => Outcome::Removed,
                            1 => Outcome::Rejected(Rejection::RestartCursor),
                            2 => Outcome::Temporary { retry_after: None },
                            _ => Outcome::Fresh(value),
                        };
                        let before = s.status(&k);
                        let applied = s.complete(&k, t, outcome, now, 0.5);
                        if dead.contains(&(k, t)) {
                            assert_eq!(applied, Applied::Discarded);
                        }
                        if applied == Applied::Discarded {
                            assert_eq!(s.status(&k), before, "discard changes nothing");
                        }
                        dead.push((k, t));
                    }
                    6 => {
                        if next(2) == 0 {
                            s.mark_stale(&key);
                        } else {
                            s.mutation_confirmed(&key);
                        }
                        dead.extend(tickets.iter().filter(|(k, _)| *k == key));
                    }
                    7 => {
                        s.remove_at(&key, now);
                        dead.extend(tickets.iter().filter(|(k, _)| *k == key));
                    }
                    _ => {
                        if next(10) == 0 {
                            s.clear();
                            dead.extend(tickets.iter());
                        }
                    }
                }
                for key in 0..2u8 {
                    let live = s
                        .entries
                        .get(&key)
                        .and_then(|e| e.in_flight)
                        .map(|(t, _)| t);
                    let alive: Vec<_> = tickets
                        .iter()
                        .filter(|(k, t)| *k == key && !dead.contains(&(*k, *t)) && Some(*t) == live)
                        .collect();
                    assert!(alive.len() <= 1, "single flight per key");
                }
            }
        }
    }
}
