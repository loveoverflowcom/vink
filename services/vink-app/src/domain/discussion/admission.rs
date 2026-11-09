//! Initial review of a new comment under a versioned admission policy (#120 DISC-01), and the
//! post-review that may withdraw a publish-first approval.
//!
//! COM-02/D-08 keep premoderation: every stored comment waits for a human. This module names that
//! rule and versions it, so a later policy delta changes one decision and its recorded provenance
//! instead of a constant spread over every layer. The adapter loads [`AdmissionFacts`] from server
//! authorities inside the write transaction and supplies the clock; nothing here performs I/O,
//! reads a clock or trusts a client field. No request DTO carries a tier, role or review.
//!
//! - Standing that is not known to be active refuses the submission. A pending row is never a way
//!   around authentication or suspension.
//! - [`AdmissionPolicy::PremoderationV1`] admits every comment as pending. It is the only policy a
//!   [`PolicySelection`] can make effective, and the rollback target of any later selection.
//! - [`AdmissionPolicy::TrustedCohortV1`] is a PROPOSED publish-first candidate (#120 CHOICE-01 A).
//!   It approves only a principal that the server's trust authority attested recently, on a
//!   standard thread, with a fresh deterministic-rules assessment of this exact payload and no
//!   flag from any source. Anything missing, stale, revoked, foreign or flagged stays pending. It
//!   runs in shadow: its decision is returned for measurement and never stored. Its thresholds are
//!   candidates awaiting evidence (`docs/architecture/discussion-admission.md`).
//! - An untrusted source (an AI classifier, MOD-05) can hold a comment but never clear one.
//!
//! A selection cannot make the candidate effective. Enabling it needs an accepted D-08 delta, a
//! trust authority with revocation, the post-review worker and a reviewed change of this type:
//! ```compile_fail
//! use vink_app::domain::discussion::admission::{AdmissionPolicy, PolicySelection};
//! let selection = PolicySelection { effective: AdmissionPolicy::TrustedCohortV1, shadow: None };
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::admission::{AdmissionPolicy, PolicySelection};
//! let selection = PolicySelection::shadowing(AdmissionPolicy::TrustedCohortV1);
//! assert_eq!(selection.effective(), AdmissionPolicy::PremoderationV1);
//! assert_eq!(selection.shadow(), Some(AdmissionPolicy::TrustedCohortV1));
//! ```

use super::lifecycle::{CommentState, LifecycleError, ModerationVersion, Review, Transition};
use super::submission::PayloadDigest;

/// One fixed admission rule. A changed rule is a new variant with a new code, never an edit of an
/// existing one, so stored provenance keeps its meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionPolicy {
    /// COM-02/D-08 baseline: every new comment waits for human review.
    PremoderationV1,
    /// PROPOSED publish-first candidate; shadow-only (see the module documentation).
    TrustedCohortV1,
}

impl AdmissionPolicy {
    /// Stable provenance code for storage, audit and contracts.
    pub const fn code(self) -> &'static str {
        match self {
            Self::PremoderationV1 => "premoderation.v1",
            Self::TrustedCohortV1 => "trusted-cohort.v1",
        }
    }
}

/// How long a trust attestation counts as current under `trusted-cohort.v1`: 24 hours.
/// CANDIDATE value for shadow measurement, not an accepted threshold.
pub const TRUST_MAX_AGE_MICROS: i64 = 24 * 60 * 60 * 1_000_000;
/// How long a rules assessment of a payload counts as current under `trusted-cohort.v1`:
/// 5 minutes. CANDIDATE value for shadow measurement, not an accepted threshold.
pub const RULES_MAX_AGE_MICROS: i64 = 5 * 60 * 1_000_000;

/// The principal's standing as the App reads it at the decision's commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Active,
    Suspended,
    /// Not readable or not established: never treated as active.
    Unknown,
}

/// What the server's trust authority says about the principal. No such authority exists yet, so
/// an adapter can only supply `Unknown`. A verified email address is not trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Unknown,
    NotTrusted,
    /// A trust attestation existed and was withdrawn.
    Revoked,
    /// Attested by the authority at this instant (microseconds since the Unix epoch).
    Trusted {
        attested_at_micros: i64,
    },
}

/// Review requirement of the thread itself, e.g. an operator holding a thread under attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadReview {
    Standard,
    AllReviewed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Clear,
    Flagged,
}

/// One source's assessment: of which payload, when, and what it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assessment {
    Unassessed,
    Assessed {
        verdict: Verdict,
        payload: PayloadDigest,
        at_micros: i64,
    },
}

/// Facts the adapter reads from server authorities for one submission. The client supplies none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionFacts {
    pub standing: Standing,
    pub trust: Trust,
    pub thread: ThreadReview,
    /// Deterministic server rules (links, mentions, repetition) at a fixed version: the only
    /// source that can clear a payload. None is implemented yet.
    pub rules: Assessment,
    /// An untrusted classifier (AI provider, MOD-05): its flag holds, its clear counts for nothing.
    pub classifier: Assessment,
    /// Digest of the payload being submitted ([`PayloadDigest::of_submission`]).
    pub payload: PayloadDigest,
    /// The adapter's clock at the decision, in microseconds since the Unix epoch.
    pub now_micros: i64,
}

impl AdmissionFacts {
    /// Facts when only standing has a source, as in the pilot: no trust authority, no thread
    /// hold, nothing assessed. Every policy admits an active principal's comment as pending.
    pub const fn standing_only(
        standing: Standing,
        payload: PayloadDigest,
        now_micros: i64,
    ) -> Self {
        Self {
            standing,
            trust: Trust::Unknown,
            thread: ThreadReview::Standard,
            rules: Assessment::Unassessed,
            classifier: Assessment::Unassessed,
            payload,
            now_micros,
        }
    }
}

impl From<crate::domain::principal::Standing> for Standing {
    fn from(standing: crate::domain::principal::Standing) -> Self {
        match standing {
            crate::domain::principal::Standing::Active => Self::Active,
            crate::domain::principal::Standing::Suspended => Self::Suspended,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialReview {
    Pending,
    Approved,
}

/// Why an admission chose its review: provenance for audit and for the shadow comparison.
/// Under `trusted-cohort.v1` the first failing guard is reported, in the order listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionReason {
    /// `premoderation.v1`: every comment is reviewed.
    PolicyPremoderation,
    ThreadReviewed,
    /// A rules or classifier assessment, of any payload or age, flagged it.
    ContentFlagged,
    TrustUnknown,
    NotTrusted,
    TrustRevoked,
    /// The attestation is older than the policy allows, or dated in the future.
    TrustStale,
    ContentUnassessed,
    /// The rules assessed another payload.
    ContentMismatch,
    /// The rules assessment is older than the policy allows, or dated in the future.
    ContentStale,
    /// The only reason that approves.
    TrustedAndClear,
}

impl AdmissionReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::PolicyPremoderation => "policy_premoderation",
            Self::ThreadReviewed => "thread_reviewed",
            Self::ContentFlagged => "content_flagged",
            Self::TrustUnknown => "trust_unknown",
            Self::NotTrusted => "not_trusted",
            Self::TrustRevoked => "trust_revoked",
            Self::TrustStale => "trust_stale",
            Self::ContentUnassessed => "content_unassessed",
            Self::ContentMismatch => "content_mismatch",
            Self::ContentStale => "content_stale",
            Self::TrustedAndClear => "trusted_and_clear",
        }
    }
}

/// Why a submission is not admitted at all: nothing is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Suspended,
    StandingUnknown,
}

impl Refusal {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Suspended => "suspended",
            Self::StandingUnknown => "standing_unknown",
        }
    }
}

/// An admitted comment's initial review with its provenance. Only a decision creates one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Admission {
    policy: AdmissionPolicy,
    review: InitialReview,
    reason: AdmissionReason,
}

impl Admission {
    pub const fn policy(self) -> AdmissionPolicy {
        self.policy
    }

    pub const fn review(self) -> InitialReview {
        self.review
    }

    pub const fn reason(self) -> AdmissionReason {
        self.reason
    }

    /// The state to store: present at the first moderation version.
    pub const fn initial_state(self) -> CommentState {
        CommentState::admitted(match self.review {
            InitialReview::Pending => Review::Pending,
            InitialReview::Approved => Review::Approved,
        })
    }

    /// A publish-first approval owes a post-review; a pending comment already waits for a human.
    pub const fn post_review(self) -> Option<PostReviewTicket> {
        match self.review {
            InitialReview::Approved => Some(PostReviewTicket {
                policy: self.policy,
                version: ModerationVersion::FIRST,
            }),
            InitialReview::Pending => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionDecision {
    Admit(Admission),
    Refuse(Refusal),
}

/// The admission decision of one policy. Standing is checked first under every policy.
pub fn decide_initial_review(policy: AdmissionPolicy, facts: AdmissionFacts) -> AdmissionDecision {
    match facts.standing {
        Standing::Active => {}
        Standing::Suspended => return AdmissionDecision::Refuse(Refusal::Suspended),
        Standing::Unknown => return AdmissionDecision::Refuse(Refusal::StandingUnknown),
    }
    let (review, reason) = match policy {
        AdmissionPolicy::PremoderationV1 => {
            (InitialReview::Pending, AdmissionReason::PolicyPremoderation)
        }
        AdmissionPolicy::TrustedCohortV1 => match trusted_cohort_hold(facts) {
            Some(reason) => (InitialReview::Pending, reason),
            None => (InitialReview::Approved, AdmissionReason::TrustedAndClear),
        },
    };
    AdmissionDecision::Admit(Admission {
        policy,
        review,
        reason,
    })
}

/// The first guard of `trusted-cohort.v1` that holds the comment, or `None` when all pass.
fn trusted_cohort_hold(facts: AdmissionFacts) -> Option<AdmissionReason> {
    if facts.thread == ThreadReview::AllReviewed {
        return Some(AdmissionReason::ThreadReviewed);
    }
    if flagged(facts.rules) || flagged(facts.classifier) {
        return Some(AdmissionReason::ContentFlagged);
    }
    let trust_hold = match facts.trust {
        Trust::Unknown => Some(AdmissionReason::TrustUnknown),
        Trust::NotTrusted => Some(AdmissionReason::NotTrusted),
        Trust::Revoked => Some(AdmissionReason::TrustRevoked),
        Trust::Trusted { attested_at_micros } => {
            (!fresh(attested_at_micros, facts.now_micros, TRUST_MAX_AGE_MICROS))
                .then_some(AdmissionReason::TrustStale)
        }
    };
    if trust_hold.is_some() {
        return trust_hold;
    }
    match facts.rules {
        Assessment::Unassessed => Some(AdmissionReason::ContentUnassessed),
        Assessment::Assessed { payload, .. } if payload != facts.payload => {
            Some(AdmissionReason::ContentMismatch)
        }
        Assessment::Assessed { at_micros, .. }
            if !fresh(at_micros, facts.now_micros, RULES_MAX_AGE_MICROS) =>
        {
            Some(AdmissionReason::ContentStale)
        }
        // A flag returned above, so this assessment is clear, current and of this payload.
        Assessment::Assessed { .. } => None,
    }
}

const fn flagged(assessment: Assessment) -> bool {
    matches!(
        assessment,
        Assessment::Assessed {
            verdict: Verdict::Flagged,
            ..
        }
    )
}

/// A timestamp counts when it is not in the future and at most `max_age` old. An age that does
/// not fit in `i64` is too old; a future timestamp cannot be verified. Neither panics nor wraps.
fn fresh(at: i64, now: i64, max_age: i64) -> bool {
    at <= now && now.checked_sub(at).is_some_and(|age| age <= max_age)
}

/// Which policies run for a submission: `effective` decides what is stored; `shadow` is only
/// evaluated and returned for measurement (#120 CHOICE-01 A). Every constructible selection keeps
/// premoderation effective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicySelection {
    effective: AdmissionPolicy,
    shadow: Option<AdmissionPolicy>,
}

impl PolicySelection {
    /// The pilot and the rollback target: every comment waits for review.
    pub const PREMODERATION: Self = Self {
        effective: AdmissionPolicy::PremoderationV1,
        shadow: None,
    };

    /// The pilot plus a candidate evaluated in shadow.
    pub const fn shadowing(candidate: AdmissionPolicy) -> Self {
        Self {
            effective: AdmissionPolicy::PremoderationV1,
            shadow: Some(candidate),
        }
    }

    pub const fn effective(self) -> AdmissionPolicy {
        self.effective
    }

    pub const fn shadow(self) -> Option<AdmissionPolicy> {
        self.shadow
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionOutcome {
    /// Decides what is stored.
    pub effective: AdmissionDecision,
    /// Recorded for comparison only; never the comment's state.
    pub shadow: Option<AdmissionDecision>,
}

pub fn decide_admission(selection: PolicySelection, facts: AdmissionFacts) -> AdmissionOutcome {
    AdmissionOutcome {
        effective: decide_initial_review(selection.effective, facts),
        shadow: selection
            .shadow
            .map(|candidate| decide_initial_review(candidate, facts)),
    }
}

/// What a post-review job carries: the policy and the version its admission produced. It is
/// created with an approved [`Admission`]; a worker restores it from the job it stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostReviewTicket {
    policy: AdmissionPolicy,
    version: ModerationVersion,
}

impl PostReviewTicket {
    /// Rehydrates a ticket read from a stored job. Never build one from request data.
    pub const fn restore(policy: AdmissionPolicy, version: ModerationVersion) -> Self {
        Self { policy, version }
    }

    pub const fn policy(self) -> AdmissionPolicy {
        self.policy
    }

    pub const fn version(self) -> ModerationVersion {
        self.version
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostReviewFinding {
    Clear,
    Flagged,
    /// The review did not finish within its retries or deadline (MOD-03): fail closed.
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostReview {
    /// Nothing to withdraw; write nothing.
    Keep,
    /// Return the comment to the review queue; `Unchanged` when it is already hidden.
    Hold(Transition),
}

/// A post-review outcome for the admission in `ticket`. It never approves: a clear finding keeps
/// the comment as it is, and a flag or an unresolved review withdraws the approval only at the
/// observed version. A human decision since then (`Stale`) and a removal (`Removed`, terminal)
/// both win over the job.
pub fn decide_post_review(
    ticket: PostReviewTicket,
    current: CommentState,
    finding: PostReviewFinding,
) -> Result<PostReview, LifecycleError> {
    match finding {
        PostReviewFinding::Clear => Ok(PostReview::Keep),
        PostReviewFinding::Flagged | PostReviewFinding::Unresolved => current
            .hold_for_review(ticket.version)
            .map(PostReview::Hold),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::discussion::body::CommentBody;
    use crate::domain::discussion::lifecycle::Decision;
    use crate::domain::ids::WorkId;

    const NOW: i64 = 1_800_000_000_000_000;

    fn digest(text: &str) -> PayloadDigest {
        PayloadDigest::of_submission(
            WorkId::from_uuid(Uuid::from_u128(77)),
            None,
            &CommentBody::parse(text).unwrap(),
        )
    }

    fn assessed(verdict: Verdict, payload: PayloadDigest, at_micros: i64) -> Assessment {
        Assessment::Assessed {
            verdict,
            payload,
            at_micros,
        }
    }

    /// Facts under which `trusted-cohort.v1` approves; each test breaks one guard.
    fn passing() -> AdmissionFacts {
        AdmissionFacts {
            standing: Standing::Active,
            trust: Trust::Trusted {
                attested_at_micros: NOW - 1,
            },
            thread: ThreadReview::Standard,
            rules: assessed(Verdict::Clear, digest("Hay quá"), NOW),
            classifier: Assessment::Unassessed,
            payload: digest("Hay quá"),
            now_micros: NOW,
        }
    }

    fn admitted(decision: AdmissionDecision) -> Admission {
        match decision {
            AdmissionDecision::Admit(admission) => admission,
            other => panic!("expected an admission, got {other:?}"),
        }
    }

    fn reason(facts: AdmissionFacts) -> AdmissionReason {
        admitted(decide_initial_review(
            AdmissionPolicy::TrustedCohortV1,
            facts,
        ))
        .reason()
    }

    #[test]
    fn premoderation_pends_every_active_principal_and_refuses_the_rest() {
        let admission = admitted(decide_initial_review(
            AdmissionPolicy::PremoderationV1,
            passing(),
        ));
        assert_eq!(
            (admission.review(), admission.reason()),
            (InitialReview::Pending, AdmissionReason::PolicyPremoderation)
        );
        assert_eq!(admission.initial_state(), CommentState::SUBMITTED);
        assert_eq!(admission.post_review(), None);
        for policy in [
            AdmissionPolicy::PremoderationV1,
            AdmissionPolicy::TrustedCohortV1,
        ] {
            for (standing, refusal) in [
                (Standing::Suspended, Refusal::Suspended),
                (Standing::Unknown, Refusal::StandingUnknown),
            ] {
                let facts = AdmissionFacts {
                    standing,
                    ..passing()
                };
                assert_eq!(
                    decide_initial_review(policy, facts),
                    AdmissionDecision::Refuse(refusal)
                );
            }
        }
    }

    #[test]
    fn the_candidate_approves_only_when_every_guard_passes() {
        let admission = admitted(decide_initial_review(
            AdmissionPolicy::TrustedCohortV1,
            passing(),
        ));
        assert_eq!(
            (admission.review(), admission.reason()),
            (InitialReview::Approved, AdmissionReason::TrustedAndClear)
        );
        let state = admission.initial_state();
        assert!(state.is_public());
        assert_eq!(state.version(), ModerationVersion::FIRST);
        assert_eq!(
            admission.post_review(),
            Some(PostReviewTicket::restore(
                AdmissionPolicy::TrustedCohortV1,
                ModerationVersion::FIRST
            ))
        );

        let other = digest("Bài khác");
        let base = passing();
        for (facts, expected) in [
            (
                AdmissionFacts {
                    thread: ThreadReview::AllReviewed,
                    ..base
                },
                AdmissionReason::ThreadReviewed,
            ),
            (
                AdmissionFacts {
                    rules: assessed(Verdict::Flagged, base.payload, NOW),
                    ..base
                },
                AdmissionReason::ContentFlagged,
            ),
            (
                AdmissionFacts {
                    classifier: assessed(Verdict::Flagged, other, 0),
                    ..base
                },
                AdmissionReason::ContentFlagged,
            ),
            (
                AdmissionFacts {
                    trust: Trust::Unknown,
                    ..base
                },
                AdmissionReason::TrustUnknown,
            ),
            (
                AdmissionFacts {
                    trust: Trust::NotTrusted,
                    ..base
                },
                AdmissionReason::NotTrusted,
            ),
            (
                AdmissionFacts {
                    trust: Trust::Revoked,
                    ..base
                },
                AdmissionReason::TrustRevoked,
            ),
            (
                AdmissionFacts {
                    trust: Trust::Trusted {
                        attested_at_micros: NOW - TRUST_MAX_AGE_MICROS - 1,
                    },
                    ..base
                },
                AdmissionReason::TrustStale,
            ),
            (
                AdmissionFacts {
                    rules: Assessment::Unassessed,
                    ..base
                },
                AdmissionReason::ContentUnassessed,
            ),
            (
                AdmissionFacts {
                    rules: assessed(Verdict::Clear, other, NOW),
                    ..base
                },
                AdmissionReason::ContentMismatch,
            ),
            (
                AdmissionFacts {
                    rules: assessed(Verdict::Clear, base.payload, NOW + 1),
                    ..base
                },
                AdmissionReason::ContentStale,
            ),
        ] {
            let admission = admitted(decide_initial_review(
                AdmissionPolicy::TrustedCohortV1,
                facts,
            ));
            assert_eq!(admission.reason(), expected, "{facts:?}");
            assert_eq!(admission.review(), InitialReview::Pending);
            assert_eq!(admission.initial_state(), CommentState::SUBMITTED);
            assert_eq!(admission.post_review(), None);
        }
    }

    #[test]
    fn an_untrusted_classifier_can_hold_but_never_clear() {
        let base = passing();
        let clear_classifier = AdmissionFacts {
            rules: Assessment::Unassessed,
            classifier: assessed(Verdict::Clear, base.payload, NOW),
            ..base
        };
        assert_eq!(reason(clear_classifier), AdmissionReason::ContentUnassessed);
        let both_clear = AdmissionFacts {
            classifier: assessed(Verdict::Clear, base.payload, NOW),
            ..base
        };
        assert_eq!(reason(both_clear), AdmissionReason::TrustedAndClear);
    }

    #[test]
    fn freshness_is_inclusive_bounded_and_never_wraps() {
        let at = |trust_at, rules_at, now| AdmissionFacts {
            trust: Trust::Trusted {
                attested_at_micros: trust_at,
            },
            rules: assessed(Verdict::Clear, passing().payload, rules_at),
            now_micros: now,
            ..passing()
        };
        assert_eq!(
            reason(at(
                NOW - TRUST_MAX_AGE_MICROS,
                NOW - RULES_MAX_AGE_MICROS,
                NOW
            )),
            AdmissionReason::TrustedAndClear,
            "exactly the maximum age still counts"
        );
        assert_eq!(
            reason(at(NOW, NOW - RULES_MAX_AGE_MICROS - 1, NOW)),
            AdmissionReason::ContentStale
        );
        assert_eq!(
            reason(at(NOW + 1, NOW, NOW)),
            AdmissionReason::TrustStale,
            "a future attestation cannot be verified"
        );
        assert_eq!(
            reason(at(i64::MIN, i64::MAX, i64::MAX)),
            AdmissionReason::TrustStale,
            "an age beyond i64 is too old, not a wrap to a small age"
        );
        assert_eq!(
            reason(at(i64::MIN, i64::MIN, i64::MIN)),
            AdmissionReason::TrustedAndClear
        );
    }

    #[test]
    fn shadow_decisions_are_returned_but_never_effective() {
        let selection = PolicySelection::shadowing(AdmissionPolicy::TrustedCohortV1);
        let outcome = decide_admission(selection, passing());
        assert_eq!(
            outcome.effective,
            decide_initial_review(AdmissionPolicy::PremoderationV1, passing())
        );
        let shadow = admitted(outcome.shadow.unwrap());
        assert_eq!(shadow.review(), InitialReview::Approved);
        let stored = admitted(outcome.effective);
        assert_eq!(stored.initial_state(), CommentState::SUBMITTED);
        assert_eq!(stored.policy(), AdmissionPolicy::PremoderationV1);

        let pilot = decide_admission(PolicySelection::PREMODERATION, passing());
        assert_eq!(pilot.effective, outcome.effective);
        assert_eq!(pilot.shadow, None);
        assert_eq!(
            PolicySelection::PREMODERATION.effective(),
            AdmissionPolicy::PremoderationV1
        );
    }

    #[test]
    fn post_review_withdraws_only_the_observed_approval() {
        let admission = admitted(decide_initial_review(
            AdmissionPolicy::TrustedCohortV1,
            passing(),
        ));
        let ticket = admission.post_review().unwrap();
        let approved = admission.initial_state();
        assert_eq!(
            decide_post_review(ticket, approved, PostReviewFinding::Clear),
            Ok(PostReview::Keep)
        );
        for finding in [PostReviewFinding::Flagged, PostReviewFinding::Unresolved] {
            let Ok(PostReview::Hold(Transition::Changed(held))) =
                decide_post_review(ticket, approved, finding)
            else {
                panic!("{finding:?} must withdraw the approval");
            };
            assert_eq!(held.review(), Review::Pending);
            assert!(held.is_present() && !held.is_public());
        }
        // A moderator rejected and approved again: the job's observation is stale.
        let Ok(Transition::Changed(rejected)) =
            approved.decide(ModerationVersion::FIRST, Decision::Reject)
        else {
            panic!("reject");
        };
        let Ok(Transition::Changed(human)) = rejected.decide(rejected.version(), Decision::Approve)
        else {
            panic!("approve");
        };
        assert_eq!(
            decide_post_review(ticket, human, PostReviewFinding::Flagged),
            Err(LifecycleError::Stale {
                current: human.version()
            })
        );
        let Ok(Transition::Changed(removed)) = approved.remove_by_author() else {
            panic!("remove");
        };
        assert_eq!(
            decide_post_review(ticket, removed, PostReviewFinding::Unresolved),
            Err(LifecycleError::Removed)
        );
    }

    #[test]
    fn provenance_codes_are_distinct_and_stable() {
        let reasons = [
            AdmissionReason::PolicyPremoderation,
            AdmissionReason::ThreadReviewed,
            AdmissionReason::ContentFlagged,
            AdmissionReason::TrustUnknown,
            AdmissionReason::NotTrusted,
            AdmissionReason::TrustRevoked,
            AdmissionReason::TrustStale,
            AdmissionReason::ContentUnassessed,
            AdmissionReason::ContentMismatch,
            AdmissionReason::ContentStale,
            AdmissionReason::TrustedAndClear,
        ];
        let mut codes: Vec<&str> = reasons.iter().map(|reason| reason.code()).collect();
        codes.extend([Refusal::Suspended.code(), Refusal::StandingUnknown.code()]);
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), reasons.len() + 2);
        assert_eq!(AdmissionPolicy::PremoderationV1.code(), "premoderation.v1");
        assert_eq!(AdmissionPolicy::TrustedCohortV1.code(), "trusted-cohort.v1");
    }

    #[test]
    fn decisions_are_exhaustive_over_the_fact_space() {
        // Every standing × trust (with fresh/stale/future attestations) × thread × rules × classifier
        // × policy: exactly one decision, and an approval exactly when an independent predicate
        // over the raw facts holds. Guards precedence and the conservative default.
        let payload = digest("Hay quá");
        let other = digest("Khác");
        let times = [NOW, NOW - RULES_MAX_AGE_MICROS - 1, NOW + 1, i64::MIN];
        let mut assessments = vec![Assessment::Unassessed];
        for verdict in [Verdict::Clear, Verdict::Flagged] {
            for digest in [payload, other] {
                for at in times {
                    assessments.push(assessed(verdict, digest, at));
                }
            }
        }
        let mut trusts = vec![Trust::Unknown, Trust::NotTrusted, Trust::Revoked];
        for at in [NOW, NOW - TRUST_MAX_AGE_MICROS - 1, NOW + 1, i64::MIN] {
            trusts.push(Trust::Trusted {
                attested_at_micros: at,
            });
        }
        let (mut approvals, mut decisions) = (0, 0);
        for policy in [
            AdmissionPolicy::PremoderationV1,
            AdmissionPolicy::TrustedCohortV1,
        ] {
            for standing in [Standing::Active, Standing::Suspended, Standing::Unknown] {
                for trust in &trusts {
                    for thread in [ThreadReview::Standard, ThreadReview::AllReviewed] {
                        for rules in &assessments {
                            for classifier in &assessments {
                                let facts = AdmissionFacts {
                                    standing,
                                    trust: *trust,
                                    thread,
                                    rules: *rules,
                                    classifier: *classifier,
                                    payload,
                                    now_micros: NOW,
                                };
                                let decision = decide_initial_review(policy, facts);
                                decisions += 1;
                                let expected_approval = policy == AdmissionPolicy::TrustedCohortV1
                                    && standing == Standing::Active
                                    && thread == ThreadReview::Standard
                                    && matches!(trust, Trust::Trusted { attested_at_micros }
                                        if *attested_at_micros == NOW)
                                    && *rules == assessed(Verdict::Clear, payload, NOW)
                                    && !matches!(
                                        classifier,
                                        Assessment::Assessed {
                                            verdict: Verdict::Flagged,
                                            ..
                                        }
                                    );
                                match decision {
                                    AdmissionDecision::Refuse(_) => {
                                        assert_ne!(standing, Standing::Active);
                                    }
                                    AdmissionDecision::Admit(admission) => {
                                        assert_eq!(standing, Standing::Active);
                                        assert_eq!(admission.policy(), policy);
                                        let approved =
                                            admission.review() == InitialReview::Approved;
                                        assert_eq!(approved, expected_approval, "{facts:?}");
                                        assert_eq!(
                                            approved,
                                            admission.reason() == AdmissionReason::TrustedAndClear
                                        );
                                        approvals += usize::from(approved);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(approvals > 0 && approvals < decisions);
    }
}
