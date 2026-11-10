//! Discussion outbox lifecycle (DISC-04, #120): what a durable event owes each consumer, when a
//! worker may claim that obligation, whose result counts and what happens on failure. Pure: time
//! is an input (microseconds since the Unix epoch, read by the adapter from PostgreSQL), as is
//! jitter; nothing here reads a clock, SQL, the network or the environment.
//!
//! **Model.** A committed business transition writes one immutable event (its identity, type,
//! schema version and the source version it observed) and one *delivery* per consumer that the
//! event type owes (#120 DISC-04 CHOICE-01 A). Each delivery has its own state, attempts and
//! claim generation, so one consumer's success never stands for another's: there is no
//! event-wide "processed" flag.
//!
//! **Safety vs. liveness.** The claim *generation* is the fence: every claim advances it by one,
//! and a result counts only when the delivery is still leased under the generation the worker
//! holds ([`check_ownership`]). The adapter reads that pair under a row lock in the same
//! transaction that applies the effect, so a worker that lost its claim cannot write an effect,
//! an acknowledgement, a retry or a dead letter for the new owner. The lease is only a liveness
//! timer: after it expires another worker may reclaim, which advances the generation; an expired
//! but unreclaimed lease still lets its holder finish, because a reclaim needs the same row lock.
//!
//! **Bounds.** Attempts are counted when claimed, so a crash or a lost lease spends one; the
//! budget ([`OutboxPolicy::max_attempts`]) and the deadline from the event (or from the last
//! operator requeue) end in a dead letter with a reason code, never in success. Retries back off
//! exponentially with a cap and bounded jitter. Unsupported events end as dead letters too
//! (requeued by an operator after an upgrade); a consumer this binary does not know is never
//! claimed at all, so a rolled-back binary leaves a newer consumer's work for its owner.
//!
//! **Not exactly once.** Effects that live in the App database are applied in the fenced
//! transaction together with their outcome, so a delivery's effect and its terminal state commit
//! or roll back together. Nothing here makes an effect outside that transaction exactly-once.
//!
//! Generations and states are not writable fields; a stored row is restored only through
//! [`DeliveryFacts`], and only these decisions produce the next state:
//! ```compile_fail
//! use vink_app::domain::discussion::outbox::ClaimGeneration;
//! let generation = ClaimGeneration(7);
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::outbox::ClaimGeneration;
//! let generation = ClaimGeneration::restore(7).unwrap();
//! assert_eq!(generation.get(), 7);
//! assert_eq!(ClaimGeneration::restore(-1), None);
//! ```

use super::admission::{
    AdmissionDecision, AdmissionFacts, AdmissionPolicy, Standing, decide_initial_review,
};
use super::lifecycle::CommentState;
use super::submission::PayloadDigest;

/// Event types that the App produces. A new type is a new variant with its own schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// A comment was stored by the submission path (initial state from its admission).
    CommentSubmitted,
}

impl EventKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::CommentSubmitted => "comment.submitted",
        }
    }

    /// The schema version this binary writes for the type.
    pub const fn current_schema(self) -> i16 {
        match self {
            Self::CommentSubmitted => 1,
        }
    }

    /// The consumers an event of this type owes, each with its own delivery.
    pub const fn consumers(self) -> &'static [Consumer] {
        match self {
            Self::CommentSubmitted => &[Consumer::AdmissionShadow],
        }
    }

    /// A stored type and schema this binary can read; anything else is unsupported.
    pub fn supported(code: &str, schema: i16) -> Option<Self> {
        match (code, schema) {
            ("comment.submitted", 1) => Some(Self::CommentSubmitted),
            _ => None,
        }
    }
}

/// Production consumers. Their codes carry their own version: changing what a consumer does is a
/// new code, so stored deliveries keep the meaning they were written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consumer {
    /// Records what the publish-first candidate would have decided (shadow/audit only).
    AdmissionShadow,
}

impl Consumer {
    pub const ALL: [Self; 1] = [Self::AdmissionShadow];

    pub const fn code(self) -> &'static str {
        match self {
            Self::AdmissionShadow => "admission_shadow.v1",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|consumer| consumer.code() == code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryState {
    /// Claimable once `available_at` has passed (first attempt or a scheduled retry).
    Ready,
    /// Held under the current generation until `available_at` (the lease expiry).
    Leased,
    Succeeded,
    /// Ended without an effect for a reason that is not a failure (stale or removed target).
    Skipped,
    /// Ended without an effect; waits for an operator. Never cleaned up while it waits.
    Dead,
}

impl DeliveryState {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Leased => "leased",
            Self::Succeeded => "succeeded",
            Self::Skipped => "skipped",
            Self::Dead => "dead",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::Ready,
            Self::Leased,
            Self::Succeeded,
            Self::Skipped,
            Self::Dead,
        ]
        .into_iter()
        .find(|state| state.code() == code)
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Skipped | Self::Dead)
    }
}

/// The claim fence. Starts at 0 (never claimed) and advances by one on every claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaimGeneration(i64);

impl ClaimGeneration {
    pub const UNCLAIMED: Self = Self(0);

    pub fn restore(raw: i64) -> Option<Self> {
        (raw >= 0).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// A delivery as stored, read under its row lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryFacts {
    pub state: DeliveryState,
    /// Claims so far, including the one in progress.
    pub attempts: u32,
    pub generation: ClaimGeneration,
    /// Ready: not claimable before this instant. Leased: the lease expiry.
    pub available_at_micros: i64,
    /// The event's creation, or the last operator requeue: the deadline counts from here.
    pub deadline_from_micros: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxPolicy {
    max_attempts: u32,
    lease_micros: i64,
    base_backoff_micros: i64,
    max_backoff_micros: i64,
    deadline_micros: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("max_attempts must be between 1 and 100")]
    Attempts,
    #[error("the lease, backoff and deadline must be positive and backoff <= cap <= deadline")]
    Durations,
}

impl OutboxPolicy {
    /// 8 attempts, 30 s lease, 5 s → 10 min backoff, 7 day deadline.
    pub const DEFAULT: Self = Self {
        max_attempts: 8,
        lease_micros: 30_000_000,
        base_backoff_micros: 5_000_000,
        max_backoff_micros: 600_000_000,
        deadline_micros: 7 * 24 * 3_600_000_000,
    };

    pub fn new(
        max_attempts: u32,
        lease_micros: i64,
        base_backoff_micros: i64,
        max_backoff_micros: i64,
        deadline_micros: i64,
    ) -> Result<Self, PolicyError> {
        if !(1..=100).contains(&max_attempts) {
            return Err(PolicyError::Attempts);
        }
        if lease_micros <= 0
            || base_backoff_micros <= 0
            || max_backoff_micros < base_backoff_micros
            || deadline_micros < max_backoff_micros
        {
            return Err(PolicyError::Durations);
        }
        Ok(Self {
            max_attempts,
            lease_micros,
            base_backoff_micros,
            max_backoff_micros,
            deadline_micros,
        })
    }

    pub const fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    pub const fn lease_micros(self) -> i64 {
        self.lease_micros
    }

    pub const fn base_backoff_micros(self) -> i64 {
        self.base_backoff_micros
    }

    pub const fn max_backoff_micros(self) -> i64 {
        self.max_backoff_micros
    }

    pub const fn deadline_micros(self) -> i64 {
        self.deadline_micros
    }

    /// `base · 2^(attempts−1)`, capped; attempt 0 or 1 waits `base`.
    pub fn backoff_micros(self, attempts: u32) -> i64 {
        let exponent = attempts.saturating_sub(1).min(32);
        self.base_backoff_micros
            .checked_mul(1_i64 << exponent)
            .map_or(self.max_backoff_micros, |delay| {
                delay.min(self.max_backoff_micros)
            })
    }

    /// Whether the deadline counted from `from` has passed at `now`. An age that cannot be
    /// computed (overflow) counts as past: it fails closed into a dead letter.
    pub fn past_deadline(self, from_micros: i64, now_micros: i64) -> bool {
        now_micros
            .checked_sub(from_micros)
            .is_none_or(|age| age > self.deadline_micros)
    }
}

/// Why a delivery ended in the dead-letter state. Codes are stored and logged; none carries data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadReason {
    RetryExhausted,
    DeadlineExceeded,
    /// The event's type or schema version is not readable by this binary.
    UnsupportedEvent,
    /// The event's fields do not satisfy its schema (a defect or manual edit).
    InvalidPayload,
    /// The claim fence cannot advance any further.
    GenerationExhausted,
}

impl DeadReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RetryExhausted => "retry_exhausted",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::UnsupportedEvent => "unsupported_event",
            Self::InvalidPayload => "invalid_payload",
            Self::GenerationExhausted => "generation_exhausted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimDecision {
    /// Terminal, not due yet, or leased under a live lease.
    NotEligible,
    Claim {
        generation: ClaimGeneration,
        attempts: u32,
        lease_until_micros: i64,
        /// The previous holder's lease expired without a result (crash, stall or lost commit).
        reclaimed: bool,
    },
    /// The claim would exceed the budget or the deadline: end the delivery instead.
    DeadLetter { reason: DeadReason, reclaimed: bool },
}

/// Whether a worker may claim `facts` at `now`, and the claim it records.
pub fn decide_claim(facts: DeliveryFacts, policy: OutboxPolicy, now_micros: i64) -> ClaimDecision {
    let due = facts.available_at_micros <= now_micros;
    let reclaimed = match facts.state {
        DeliveryState::Ready if due => false,
        DeliveryState::Leased if due => true,
        _ => return ClaimDecision::NotEligible,
    };
    if facts.attempts >= policy.max_attempts {
        return ClaimDecision::DeadLetter {
            reason: DeadReason::RetryExhausted,
            reclaimed,
        };
    }
    if policy.past_deadline(facts.deadline_from_micros, now_micros) {
        return ClaimDecision::DeadLetter {
            reason: DeadReason::DeadlineExceeded,
            reclaimed,
        };
    }
    match facts.generation.next() {
        Some(generation) => ClaimDecision::Claim {
            generation,
            attempts: facts.attempts + 1,
            lease_until_micros: now_micros.saturating_add(policy.lease_micros),
            reclaimed,
        },
        None => ClaimDecision::DeadLetter {
            reason: DeadReason::GenerationExhausted,
            reclaimed,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReason {
    /// The delivery is no longer leased (finished, retried or dead-lettered).
    NotLeased,
    /// Another claim owns it now.
    Reclaimed,
}

impl StaleReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotLeased => "not_leased",
            Self::Reclaimed => "reclaimed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    Owner,
    Stale(StaleReason),
}

/// The fence every post-claim write passes, under the delivery's row lock.
pub fn check_ownership(facts: DeliveryFacts, held: ClaimGeneration) -> Ownership {
    if facts.state != DeliveryState::Leased {
        Ownership::Stale(StaleReason::NotLeased)
    } else if facts.generation != held {
        Ownership::Stale(StaleReason::Reclaimed)
    } else {
        Ownership::Owner
    }
}

/// Why an attempt failed in a way that may pass on retry. Codes only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transient {
    /// The database refused or lost the effect transaction.
    Database,
    /// The handler did not finish within its bound.
    Timeout,
    /// The handler panicked; its transaction rolled back.
    Panic,
}

impl Transient {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Timeout => "timeout",
            Self::Panic => "panic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The comment the event names no longer exists.
    TargetMissing,
    /// The comment was removed; there is nothing left to observe.
    TargetRemoved,
}

impl SkipReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TargetMissing => "target_missing",
            Self::TargetRemoved => "target_removed",
        }
    }
}

/// What a handler reports for one attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    /// The effect was written in the fenced transaction.
    Applied,
    /// The effect's own receipt already existed; nothing new was written.
    AlreadyApplied,
    Skipped(SkipReason),
    Retry(Transient),
    Fail(DeadReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Succeed {
        already_applied: bool,
    },
    Skip(SkipReason),
    Retry {
        available_at_micros: i64,
        error: Transient,
    },
    DeadLetter(DeadReason),
    /// Write nothing: another claim (or none) owns the delivery.
    Stale(StaleReason),
}

impl Completion {
    /// The state this completion records, if any.
    pub const fn state(self) -> Option<DeliveryState> {
        match self {
            Self::Succeed { .. } => Some(DeliveryState::Succeeded),
            Self::Skip(_) => Some(DeliveryState::Skipped),
            Self::Retry { .. } => Some(DeliveryState::Ready),
            Self::DeadLetter(_) => Some(DeliveryState::Dead),
            Self::Stale(_) => None,
        }
    }

    /// The stored outcome code of a terminal completion.
    pub const fn outcome(self) -> Option<&'static str> {
        match self {
            Self::Succeed {
                already_applied: false,
            } => Some("applied"),
            Self::Succeed {
                already_applied: true,
            } => Some("already_applied"),
            Self::Skip(reason) => Some(reason.code()),
            Self::DeadLetter(reason) => Some(reason.code()),
            Self::Retry { .. } | Self::Stale(_) => None,
        }
    }
}

/// What the holder of `held` records for `result`. `facts.attempts` already counts this attempt.
/// `jitter_micros` is capped at the base backoff, so a hostile or broken source cannot push a
/// retry past the cap by more than that.
pub fn decide_completion(
    facts: DeliveryFacts,
    held: ClaimGeneration,
    result: HandlerResult,
    policy: OutboxPolicy,
    now_micros: i64,
    jitter_micros: i64,
) -> Completion {
    if let Ownership::Stale(reason) = check_ownership(facts, held) {
        return Completion::Stale(reason);
    }
    match result {
        HandlerResult::Applied => Completion::Succeed {
            already_applied: false,
        },
        HandlerResult::AlreadyApplied => Completion::Succeed {
            already_applied: true,
        },
        HandlerResult::Skipped(reason) => Completion::Skip(reason),
        HandlerResult::Fail(reason) => Completion::DeadLetter(reason),
        HandlerResult::Retry(_) if facts.attempts >= policy.max_attempts => {
            Completion::DeadLetter(DeadReason::RetryExhausted)
        }
        HandlerResult::Retry(_) if policy.past_deadline(facts.deadline_from_micros, now_micros) => {
            Completion::DeadLetter(DeadReason::DeadlineExceeded)
        }
        HandlerResult::Retry(error) => Completion::Retry {
            available_at_micros: now_micros
                .saturating_add(policy.backoff_micros(facts.attempts))
                .saturating_add(jitter_micros.clamp(0, policy.base_backoff_micros)),
            error,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requeue {
    /// Back to ready now with a fresh budget and deadline; generation and history kept.
    Requeued,
    /// Only a dead letter can be requeued: a finished effect is never run again, and work in
    /// progress keeps its owner.
    Refused(DeliveryState),
}

pub const fn decide_requeue(state: DeliveryState) -> Requeue {
    match state {
        DeliveryState::Dead => Requeue::Requeued,
        other => Requeue::Refused(other),
    }
}

/// The publish-first candidate that the shadow consumer evaluates. Its decision is recorded
/// beside the comment and never written to it, so the effective policy stays premoderation
/// whatever this constant names (the stored comment's own provenance says which one admitted it).
pub const SHADOW_CANDIDATE: AdmissionPolicy = AdmissionPolicy::TrustedCohortV1;

/// Facts the shadow consumer reads from the App at evaluation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowTarget {
    /// The comment's current state (a human decision may already be recorded).
    pub state: CommentState,
    /// Its writer's current standing.
    pub standing: Standing,
    /// The digest its submission receipt holds.
    pub payload: PayloadDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowObservation {
    Skip(SkipReason),
    /// Record what the candidate decides; the comment's own state is never written.
    Record {
        candidate: AdmissionPolicy,
        decision: AdmissionDecision,
        observed: CommentState,
    },
}

/// The admission-shadow consumer's decision. Trust, thread holds, rules and classifier have no
/// source in the pilot, so they are `Unknown`/`Standard`/`Unassessed` here (honest facts, never
/// invented ones) and the candidate can only hold or refuse. Slow sources (a rules engine, an
/// untrusted classifier) belong here, outside the submission transaction, once they exist.
pub fn observe_shadow(target: Option<ShadowTarget>, now_micros: i64) -> ShadowObservation {
    let Some(target) = target else {
        return ShadowObservation::Skip(SkipReason::TargetMissing);
    };
    if target.state.removal().is_some() {
        return ShadowObservation::Skip(SkipReason::TargetRemoved);
    }
    let facts = AdmissionFacts::standing_only(target.standing, target.payload, now_micros);
    ShadowObservation::Record {
        candidate: SHADOW_CANDIDATE,
        decision: decide_initial_review(SHADOW_CANDIDATE, facts),
        observed: target.state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::discussion::admission::{AdmissionReason, InitialReview, Refusal};
    use crate::domain::discussion::lifecycle::{Decision, ModerationVersion, Transition};

    const NOW: i64 = 1_800_000_000_000_000;
    const POLICY: OutboxPolicy = OutboxPolicy::DEFAULT;

    fn facts(
        state: DeliveryState,
        attempts: u32,
        generation: i64,
        available: i64,
    ) -> DeliveryFacts {
        DeliveryFacts {
            state,
            attempts,
            generation: ClaimGeneration::restore(generation).unwrap(),
            available_at_micros: available,
            deadline_from_micros: NOW - 1_000,
        }
    }

    fn generation(raw: i64) -> ClaimGeneration {
        ClaimGeneration::restore(raw).unwrap()
    }

    #[test]
    fn claims_only_due_ready_or_expired_leases_and_advance_the_fence() {
        let ready = facts(DeliveryState::Ready, 0, 0, NOW);
        assert_eq!(
            decide_claim(ready, POLICY, NOW),
            ClaimDecision::Claim {
                generation: generation(1),
                attempts: 1,
                lease_until_micros: NOW + POLICY.lease_micros(),
                reclaimed: false,
            }
        );
        let expired = facts(DeliveryState::Leased, 3, 3, NOW - 1);
        assert_eq!(
            decide_claim(expired, POLICY, NOW),
            ClaimDecision::Claim {
                generation: generation(4),
                attempts: 4,
                lease_until_micros: NOW + POLICY.lease_micros(),
                reclaimed: true,
            }
        );
        for not_eligible in [
            facts(DeliveryState::Ready, 0, 0, NOW + 1),
            facts(DeliveryState::Leased, 1, 1, NOW + 1),
            facts(DeliveryState::Succeeded, 1, 1, NOW - 1),
            facts(DeliveryState::Skipped, 1, 1, NOW - 1),
            facts(DeliveryState::Dead, 8, 8, NOW - 1),
        ] {
            assert_eq!(
                decide_claim(not_eligible, POLICY, NOW),
                ClaimDecision::NotEligible,
                "{not_eligible:?}"
            );
        }
    }

    #[test]
    fn budget_deadline_and_fence_exhaustion_dead_letter_instead_of_claiming() {
        let spent = facts(DeliveryState::Leased, 8, 8, NOW - 1);
        assert_eq!(
            decide_claim(spent, POLICY, NOW),
            ClaimDecision::DeadLetter {
                reason: DeadReason::RetryExhausted,
                reclaimed: true,
            }
        );
        let old = DeliveryFacts {
            deadline_from_micros: NOW - POLICY.deadline_micros() - 1,
            ..facts(DeliveryState::Ready, 1, 1, NOW)
        };
        assert_eq!(
            decide_claim(old, POLICY, NOW),
            ClaimDecision::DeadLetter {
                reason: DeadReason::DeadlineExceeded,
                reclaimed: false,
            }
        );
        let overflowing = DeliveryFacts {
            deadline_from_micros: i64::MAX,
            ..facts(DeliveryState::Ready, 0, 0, i64::MIN)
        };
        assert!(matches!(
            decide_claim(overflowing, POLICY, i64::MIN),
            ClaimDecision::DeadLetter {
                reason: DeadReason::DeadlineExceeded,
                ..
            }
        ));
        let exhausted = facts(DeliveryState::Ready, 0, i64::MAX, NOW);
        assert_eq!(
            decide_claim(exhausted, POLICY, NOW),
            ClaimDecision::DeadLetter {
                reason: DeadReason::GenerationExhausted,
                reclaimed: false,
            }
        );
    }

    #[test]
    fn only_the_current_holder_records_anything() {
        let held = facts(DeliveryState::Leased, 2, 5, NOW + 10);
        for result in [
            HandlerResult::Applied,
            HandlerResult::AlreadyApplied,
            HandlerResult::Skipped(SkipReason::TargetRemoved),
            HandlerResult::Retry(Transient::Database),
            HandlerResult::Fail(DeadReason::UnsupportedEvent),
        ] {
            assert_eq!(
                decide_completion(held, generation(4), result, POLICY, NOW, 0),
                Completion::Stale(StaleReason::Reclaimed)
            );
            for state in [
                DeliveryState::Ready,
                DeliveryState::Succeeded,
                DeliveryState::Skipped,
                DeliveryState::Dead,
            ] {
                let gone = DeliveryFacts { state, ..held };
                assert_eq!(
                    decide_completion(gone, generation(5), result, POLICY, NOW, 0),
                    Completion::Stale(StaleReason::NotLeased)
                );
            }
        }
        // An expired lease that nobody reclaimed still belongs to its holder.
        let expired = facts(DeliveryState::Leased, 2, 5, NOW - 10);
        assert_eq!(
            decide_completion(
                expired,
                generation(5),
                HandlerResult::Applied,
                POLICY,
                NOW,
                0
            ),
            Completion::Succeed {
                already_applied: false
            }
        );
    }

    #[test]
    fn retries_back_off_with_a_cap_and_end_as_dead_letters() {
        let held = |attempts| facts(DeliveryState::Leased, attempts, 1, NOW + 10);
        let retry = HandlerResult::Retry(Transient::Timeout);
        let at = |attempts, jitter| match decide_completion(
            held(attempts),
            generation(1),
            retry,
            POLICY,
            NOW,
            jitter,
        ) {
            Completion::Retry {
                available_at_micros,
                error,
            } => {
                assert_eq!(error, Transient::Timeout);
                available_at_micros - NOW
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(at(1, 0), 5_000_000);
        assert_eq!(at(2, 0), 10_000_000);
        assert_eq!(at(7, 0), 320_000_000);
        // Jitter is capped at the base and never negative.
        assert_eq!(at(1, i64::MAX), 10_000_000);
        assert_eq!(at(1, -5), 5_000_000);
        assert_eq!(
            decide_completion(held(8), generation(1), retry, POLICY, NOW, 0),
            Completion::DeadLetter(DeadReason::RetryExhausted)
        );
        let policy = OutboxPolicy::new(40, 1, 1, 1_000, 10_000).unwrap();
        assert_eq!(policy.backoff_micros(39), 1_000);
        assert_eq!(policy.backoff_micros(0), 1);
        let late = DeliveryFacts {
            deadline_from_micros: NOW - POLICY.deadline_micros() - 1,
            ..held(1)
        };
        assert_eq!(
            decide_completion(late, generation(1), retry, POLICY, NOW, 0),
            Completion::DeadLetter(DeadReason::DeadlineExceeded)
        );
        // A permanent failure is never retried and never counts as success.
        assert_eq!(
            decide_completion(
                held(1),
                generation(1),
                HandlerResult::Fail(DeadReason::InvalidPayload),
                POLICY,
                NOW,
                0
            ),
            Completion::DeadLetter(DeadReason::InvalidPayload)
        );
    }

    #[test]
    fn policies_reject_unusable_bounds() {
        assert_eq!(OutboxPolicy::new(0, 1, 1, 1, 1), Err(PolicyError::Attempts));
        assert_eq!(
            OutboxPolicy::new(101, 1, 1, 1, 1),
            Err(PolicyError::Attempts)
        );
        assert_eq!(
            OutboxPolicy::new(1, 0, 1, 1, 1),
            Err(PolicyError::Durations)
        );
        assert_eq!(
            OutboxPolicy::new(1, 1, 2, 1, 2),
            Err(PolicyError::Durations)
        );
        assert_eq!(
            OutboxPolicy::new(1, 1, 1, 2, 1),
            Err(PolicyError::Durations)
        );
        let default = OutboxPolicy::DEFAULT;
        assert_eq!(
            OutboxPolicy::new(
                default.max_attempts(),
                default.lease_micros(),
                default.base_backoff_micros(),
                default.max_backoff_micros(),
                default.deadline_micros()
            ),
            Ok(default)
        );
    }

    #[test]
    fn only_dead_letters_are_requeued() {
        assert_eq!(decide_requeue(DeliveryState::Dead), Requeue::Requeued);
        for state in [
            DeliveryState::Ready,
            DeliveryState::Leased,
            DeliveryState::Succeeded,
            DeliveryState::Skipped,
        ] {
            assert_eq!(decide_requeue(state), Requeue::Refused(state));
        }
    }

    #[test]
    fn codes_round_trip_and_unknown_events_are_unsupported() {
        for state in [
            DeliveryState::Ready,
            DeliveryState::Leased,
            DeliveryState::Succeeded,
            DeliveryState::Skipped,
            DeliveryState::Dead,
        ] {
            assert_eq!(DeliveryState::from_code(state.code()), Some(state));
        }
        assert_eq!(DeliveryState::from_code("processed"), None);
        for consumer in Consumer::ALL {
            assert_eq!(Consumer::from_code(consumer.code()), Some(consumer));
        }
        assert_eq!(Consumer::from_code("admission_shadow.v2"), None);
        let kind = EventKind::CommentSubmitted;
        assert_eq!(
            EventKind::supported(kind.code(), kind.current_schema()),
            Some(kind)
        );
        assert_eq!(EventKind::supported(kind.code(), 2), None);
        assert_eq!(EventKind::supported("comment.removed", 1), None);
        assert_eq!(kind.consumers(), &[Consumer::AdmissionShadow]);
    }

    #[test]
    fn the_shadow_consumer_observes_without_approving_or_writing() {
        let payload = PayloadDigest::from_bytes(&[7; 32]).unwrap();
        let pending = CommentState::SUBMITTED;
        let active = ShadowTarget {
            state: pending,
            standing: Standing::Active,
            payload,
        };
        let ShadowObservation::Record {
            candidate,
            decision,
            observed,
        } = observe_shadow(Some(active), NOW)
        else {
            panic!("an active writer's comment is observed");
        };
        assert_eq!(
            (candidate, observed),
            (AdmissionPolicy::TrustedCohortV1, pending)
        );
        let AdmissionDecision::Admit(admission) = decision else {
            panic!("active standing is admitted");
        };
        // No trust authority exists: the candidate holds, it never approves.
        assert_eq!(admission.review(), InitialReview::Pending);
        assert_eq!(admission.reason(), AdmissionReason::TrustUnknown);

        let suspended = ShadowTarget {
            standing: Standing::Suspended,
            ..active
        };
        assert!(matches!(
            observe_shadow(Some(suspended), NOW),
            ShadowObservation::Record {
                decision: AdmissionDecision::Refuse(Refusal::Suspended),
                ..
            }
        ));
        // A human decision is observed, not overwritten.
        let Ok(Transition::Changed(approved)) =
            pending.decide(ModerationVersion::FIRST, Decision::Approve)
        else {
            panic!("approve");
        };
        assert!(matches!(
            observe_shadow(Some(ShadowTarget { state: approved, ..active }), NOW),
            ShadowObservation::Record { observed, .. } if observed == approved
        ));
        let Ok(Transition::Changed(removed)) = approved.remove_by_author() else {
            panic!("remove");
        };
        assert_eq!(
            observe_shadow(
                Some(ShadowTarget {
                    state: removed,
                    ..active
                }),
                NOW
            ),
            ShadowObservation::Skip(SkipReason::TargetRemoved)
        );
        assert_eq!(
            observe_shadow(None, NOW),
            ShadowObservation::Skip(SkipReason::TargetMissing)
        );
    }
}
