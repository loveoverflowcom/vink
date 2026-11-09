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

/// What the transport established about one request.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome<V> {
    /// A representation confirmed by the origin (a 200, or a 304 the HTTP cache merged).
    Fresh(V),
    /// Authoritative absence (404): the snapshot must not be shown again.
    Removed,
    /// Timeout, network, 5xx or an unexpected response; `retry_after` from the server if any.
    Temporary { retry_after: Option<Millis> },
}

impl<V> Outcome<V> {
    pub fn map<W>(self, f: impl FnOnce(V) -> W) -> Outcome<W> {
        match self {
            Self::Fresh(value) => Outcome::Fresh(f(value)),
            Self::Removed => Outcome::Removed,
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
    /// Anything else, including a second raw 304: keep the last success and back off.
    Temporary,
}

pub fn public_status(status: u16) -> PublicStatus {
    match status {
        200..=299 => PublicStatus::Body,
        304 => PublicStatus::Unconditional,
        404 => PublicStatus::Removed,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);

/// What a view should do on mount.
#[derive(Clone, Debug, PartialEq)]
pub enum Load<V> {
    /// A snapshot recent enough to render now; revalidate with [`Trigger::Signal`] afterwards.
    Ready(V),
    /// No displayable snapshot: this caller owns the request.
    Fetch(Ticket),
    /// No displayable snapshot and a request is already running: wait for it.
    Join,
}

/// The effect of a completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// Committed: the snapshot was replaced (`changed`) or confirmed unchanged.
    Fresh { changed: bool },
    /// Committed: entry dropped because the origin says it no longer exists.
    Removed,
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
}

#[derive(Clone, Debug)]
struct Entry<V> {
    data: Option<V>,
    /// Start of the request that confirmed `data`: a conservative lower bound.
    validated_at: Millis,
    invalidated: bool,
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
            validated_at: Millis::NEG_INFINITY,
            invalidated: false,
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
        if self.data.is_none() || self.in_flight.is_some() {
            return None;
        }
        Some(if self.invalidated {
            Millis::NEG_INFINITY
        } else if self.failures > 0 {
            self.retry_at
        } else {
            self.validated_at + policy.stale_after + if with_spread { self.spread } else { 0.0 }
        })
    }
}

#[derive(Clone, Debug)]
pub struct Store<K, V> {
    policy: Policy,
    entries: BTreeMap<K, Entry<V>>,
    next_ticket: u64,
}

impl<K: Ord + Clone, V: Clone + PartialEq> Store<K, V> {
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            entries: BTreeMap::new(),
            next_ticket: 1,
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

    fn entry(&mut self, key: &K, now: Millis) -> &mut Entry<V> {
        self.entries
            .entry(key.clone())
            .or_insert_with(|| Entry::empty(now))
    }

    fn start(&mut self, key: &K, now: Millis) -> Ticket {
        let ticket = Ticket(self.next_ticket);
        self.next_ticket += 1;
        self.entry(key, now).in_flight = Some((ticket, now));
        ticket
    }

    /// A view mounts on `key`. Reading never extends `validated_at`.
    pub fn load(&mut self, key: &K, now: Millis) -> Load<V> {
        self.collect(now);
        let max_age = self.policy.max_display_age;
        let entry = self.entry(key, now);
        entry.last_seen = now;
        if let Some(data) = &entry.data
            && now - entry.validated_at <= max_age
        {
            return Load::Ready(data.clone());
        }
        if entry.in_flight.is_some() {
            return Load::Join;
        }
        Load::Fetch(self.start(key, now))
    }

    /// Records a server-rendered snapshot the page hydrated with. `rendered_at` must not be
    /// later than the server read (the page's navigation start is a safe bound). Never replaces
    /// a newer snapshot or a request already running.
    pub fn seed(&mut self, key: &K, data: V, rendered_at: Millis, now: Millis) {
        let entry = self.entry(key, now);
        if entry.in_flight.is_none() && (entry.data.is_none() || entry.validated_at < rendered_at) {
            entry.data = Some(data);
            entry.validated_at = rendered_at;
            entry.invalidated = false;
            entry.failures = 0;
        }
    }

    /// Starts a check if `trigger` warrants one; `None` coalesces with a running request or
    /// means the snapshot is fresh enough.
    pub fn begin(&mut self, key: &K, trigger: Trigger, now: Millis) -> Option<Ticket> {
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
                let changed = entry.data.as_ref() != Some(&data);
                entry.data = Some(data);
                entry.validated_at = started_at;
                entry.invalidated = false;
                entry.failures = 0;
                entry.retry_at = Millis::NEG_INFINITY;
                entry.spread = random * policy.jitter;
                Applied::Fresh { changed }
            }
            Outcome::Removed => {
                self.entries.remove(key);
                Applied::Removed
            }
            Outcome::Temporary { retry_after } => {
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

    /// A successful mutation changed what `key` represents: refetch it next, keep showing the
    /// current snapshot meanwhile, and make any running request unable to commit.
    pub fn invalidate(&mut self, key: &K) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.invalidated = true;
            entry.in_flight = None;
        }
    }

    /// Drop `key` and supersede its running request.
    pub fn remove(&mut self, key: &K) {
        self.entries.remove(key);
    }

    /// Scope reset (for example another account): every snapshot and request is superseded.
    /// Tickets keep increasing, so no earlier request can match a later entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Edits a held snapshot without confirming it (a derived preview of an authoritative
    /// removal elsewhere). Freshness is unchanged.
    pub fn edit(&mut self, key: &K, edit: impl FnOnce(&mut V)) {
        if let Some(data) = self.entries.get_mut(key).and_then(|e| e.data.as_mut()) {
            edit(data);
        }
    }

    /// Supersede requests in every matching scope, including entries still awaiting first data.
    pub fn invalidate_where(&mut self, mut predicate: impl FnMut(&K) -> bool) {
        for (key, entry) in &mut self.entries {
            if predicate(key) {
                entry.invalidated = true;
                entry.in_flight = None;
            }
        }
    }

    /// Update every retained derived snapshot, including unobserved searches.
    pub fn edit_all(&mut self, mut edit: impl FnMut(&K, &mut V)) {
        for (key, entry) in &mut self.entries {
            if let Some(data) = &mut entry.data {
                let before = data.clone();
                edit(key, data);
                if *data != before {
                    entry.invalidated = true;
                    entry.in_flight = None;
                }
            }
        }
    }

    pub fn observe(&mut self, key: &K, now: Millis) {
        let entry = self.entry(key, now);
        entry.observers += 1;
        entry.last_seen = now;
    }

    pub fn release(&mut self, key: &K, now: Millis) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.observers = entry.observers.saturating_sub(1);
            entry.last_seen = now;
        }
        self.collect(now);
    }

    pub fn status(&self, key: &K) -> Option<Status<V>> {
        self.entries.get(key).map(|entry| Status {
            refreshing: entry.data.is_some() && entry.in_flight.is_some(),
            failed: entry.data.is_some() && entry.failures > 0,
            data: entry.data.clone(),
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
    /// refreshed (no snapshot, or every request is running).
    pub fn next_wake(&self, now: Millis) -> Option<Millis> {
        self.entries
            .values()
            .filter(|e| e.observers > 0)
            .filter_map(|e| e.check_at(&self.policy, true))
            .map(|at| at.max(now))
            .min_by(|a, b| a.total_cmp(b))
    }

    /// Drop unobserved, idle entries past retention, then the least recently seen unobserved
    /// ones above `max_entries`.
    pub fn collect(&mut self, now: Millis) {
        let retain = self.policy.retain_unobserved;
        self.entries
            .retain(|_, e| e.observers > 0 || e.in_flight.is_some() || now - e.last_seen < retain);
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
                failed: false
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
        s.invalidate(&"works");
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
                    failed: true
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
        s.invalidate(&"p");
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
        for status in [0, 401, 403, 409, 500, 503] {
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
                        let outcome = match next(4) {
                            0 => Outcome::Removed,
                            1 => Outcome::Temporary { retry_after: None },
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
                        s.invalidate(&key);
                        dead.extend(tickets.iter().filter(|(k, _)| *k == key));
                    }
                    7 => {
                        s.remove(&key);
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
