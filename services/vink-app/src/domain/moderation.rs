//! Moderation evaluation contract and versioned decision authority core (#151).
//!
//! Pairs with [`super::publication`] (`#150`):
//! - Raw evaluator output is untrusted input (`MOD-01`); malformed, unknown-field or uncertain
//!   output classifies as [`EvaluatorOutcome::InvalidOutput`] / [`EvaluatorOutcome::Uncertain`]
//!   and routes to [`Moderation::NeedsAdmin`], never [`Moderation::Approved`].
//! - Only revisions explicitly submitted by the author (`AuthorOutcome::Submitted`) open an
//!   [`EvaluationTicket`]; private draft saves never open a job (`MOD-05`, `INV-08`).
//! - Timeouts and provider errors mark the [`EvaluationId`] attempt [`JobState::Failed`]
//!   (`retryable`) without mutating revision moderation or the public pointer (`MOD-03`, `MOD-08`).
//!   A retry must issue a new [`EvaluationId`] through [`WorkModeration::reevaluate`].
//! - Every accepted decision on a submitted revision advances [`DecisionVersion`] and delegates
//!   promotion/unpublish to [`Publication::record_review`] (`MOD-02`, `MOD-07`, `AUD-02`, `PUB-01`).
//! - Once an admin decision is recorded on a revision ([`DecisionBy::Admin`]), late AI callbacks
//!   for that revision are refused with [`EvaluationRefusal::Overridden`] (`MOD-09`, `INV-02`).
//!
//! ADMIN-MODERATION-20261111 adds the governance owners as submodules: [`authority`] (admins,
//! grants, groups, recusal), [`case`] (claims fenced by case and decision versions), [`appeal`]
//! (one appeal per adverse decision version of the evaluator or a moderator, admin final),
//! [`automation`] (pause/resume) and [`investigation`] (per-read audited access to a once-public
//! revision). Here: a moderator's decision ([`DecisionBy::Moderator`]) also wins over late AI
//! results; making a work private withdraws its open jobs ([`JobState::Withdrawn`]) and refuses
//! later human decisions ([`ModerationError::CaseClosed`]); republishing an approval of an older
//! policy re-reviews the exact snapshot (PUB-06); a paused automation reroutes jobs to humans.

pub mod appeal;
pub mod authority;
pub mod automation;
pub mod case;
pub mod investigation;

use self::appeal::{AppealConclusion, AppealVerdict};
use self::authority::{AuthorityBasis, ReviewCapability, VerifiedAuthority};
use self::automation::AutomationMode;
use self::case::{CaseConclusion, CaseVerdict, ReviewSubject};
use super::discussion::submission::PayloadDigest;
use super::ids::{CommentId, EvaluationId, GrantId, PrincipalId, RevisionId, WorkId};
use super::publication::{
    Audience, AuthorCommand, AuthorOutcome, Effects, EvaluationRequest, Moderation, PointerEffect,
    Publication, PublicationError, ReviewOutcome, ReviewedRevision,
};
use super::revision::ContentHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolicyVersion(i64);

impl PolicyVersion {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn next(self) -> Result<Self, ModerationError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ModerationError::PolicyVersionExhausted)
    }
}

/// Per-revision monotonic decision fence (#151, MOD-08, MOD-09).
///
/// Distinct from `IntentGeneration` (author audience intent), [`EvaluationId`] (one evaluation
/// attempt) and worker `ClaimGeneration` (outbox lease). Starts at [`Self::FIRST`] (`1`) when a
/// revision is submitted (`Pending`) and increments on every accepted AI or admin decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DecisionVersion(i64);

impl DecisionVersion {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn next(self) -> Result<Self, ModerationError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ModerationError::DecisionVersionExhausted)
    }
}

/// Closed set of actionable policy reason codes for evaluator rejections (MOD-01, D-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReasonCode {
    SafetyPolicy,
    Harassment,
    HateSpeech,
    SexualContent,
    IllegalContent,
    SpamOrAbuse,
}

impl PolicyReasonCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SafetyPolicy => "safety_policy",
            Self::Harassment => "harassment",
            Self::HateSpeech => "hate_speech",
            Self::SexualContent => "sexual_content",
            Self::IllegalContent => "illegal_content",
            Self::SpamOrAbuse => "spam_or_abuse",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "safety_policy" => Some(Self::SafetyPolicy),
            "harassment" => Some(Self::Harassment),
            "hate_speech" => Some(Self::HateSpeech),
            "sexual_content" => Some(Self::SexualContent),
            "illegal_content" => Some(Self::IllegalContent),
            "spam_or_abuse" => Some(Self::SpamOrAbuse),
            _ => None,
        }
    }
}

/// Validated non-empty human reason required for every admin override (AC-05, MOD-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionReason(String);

impl DecisionReason {
    pub const MAX_CHARS: usize = 240;

    pub fn parse(raw: &str) -> Result<Self, ModerationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > Self::MAX_CHARS
            || trimmed.chars().any(char::is_control)
        {
            return Err(ModerationError::ReasonRequired);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Subject scope of an evaluation target (CHOICE-01 A).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationSubject {
    Work(WorkId),
    Comment(CommentId),
}

impl EvaluationSubject {
    pub const fn matches(self, target: &EvaluationTarget) -> bool {
        match (self, target) {
            (Self::Work(expected), EvaluationTarget::WorkRevision { work, .. }) => {
                expected.as_uuid().as_u128() == work.as_uuid().as_u128()
            }
            (Self::Comment(expected), EvaluationTarget::CommentRevision { comment, .. }) => {
                expected.as_uuid().as_u128() == comment.as_uuid().as_u128()
            }
            _ => false,
        }
    }
}

/// Exact immutable target bound to an evaluation request or result (CHOICE-01 A, MOD-02, MOD-07).
///
/// Only `WorkRevision` is wired to a domain consumer ([`WorkModeration`] + [`Publication`]) in
/// `#151`; `CommentRevision` shares the envelope and binding validator for contract/script
/// compatibility while the comment-edit consumer remains deferred (`#154`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationTarget {
    WorkRevision {
        work: WorkId,
        revision: RevisionId,
        digest: ContentHash,
    },
    CommentRevision {
        comment: CommentId,
        revision: RevisionId,
        digest: PayloadDigest,
    },
}

impl EvaluationTarget {
    pub const fn subject(&self) -> EvaluationSubject {
        match self {
            Self::WorkRevision { work, .. } => EvaluationSubject::Work(*work),
            Self::CommentRevision { comment, .. } => EvaluationSubject::Comment(*comment),
        }
    }

    pub const fn revision(&self) -> RevisionId {
        match self {
            Self::WorkRevision { revision, .. } | Self::CommentRevision { revision, .. } => {
                *revision
            }
        }
    }

    pub fn same_subject(&self, other: &Self) -> bool {
        self.subject() == other.subject()
    }

    pub fn same_revision_and_digest(&self, other: &Self) -> bool {
        self == other
    }
}

/// Request ticket issued for one evaluation attempt (AC-01, MOD-05, MOD-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationTicket {
    pub id: EvaluationId,
    pub target: EvaluationTarget,
    pub policy: PolicyVersion,
}

impl EvaluationTicket {
    pub fn for_work_revision(
        id: EvaluationId,
        request: &EvaluationRequest,
        policy: PolicyVersion,
    ) -> Self {
        Self {
            id,
            target: EvaluationTarget::WorkRevision {
                work: request.work,
                revision: request.revision,
                digest: request.hash.clone(),
            },
            policy,
        }
    }
}

/// Untrusted wire/provider payload before classification (MOD-01, AC-01, AC-02).
///
/// `#[serde(deny_unknown_fields)]` rejects any extra authority/pointer fields. Invalid combinations
/// or malformed JSON convert to [`EvaluatorOutcome::InvalidOutput`] without logging input text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEvaluatorOutput {
    pub verdict: String,
    #[serde(default)]
    pub reason_code: Option<String>,
}

/// Validated semantic outcome of one evaluation attempt (AC-02, AC-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluatorOutcome {
    Approve,
    Reject(PolicyReasonCode),
    Uncertain,
    Timeout,
    ProviderError,
    InvalidOutput,
}

impl EvaluatorOutcome {
    pub fn from_raw_json(raw: &str) -> Self {
        match serde_json::from_str::<RawEvaluatorOutput>(raw) {
            Ok(parsed) => Self::from_raw_output(parsed),
            Err(_) => Self::InvalidOutput,
        }
    }

    pub fn from_raw_output(raw: RawEvaluatorOutput) -> Self {
        match (raw.verdict.as_str(), raw.reason_code.as_deref()) {
            ("approve", None) => Self::Approve,
            ("reject", Some(code)) => match PolicyReasonCode::parse(code) {
                Some(reason) => Self::Reject(reason),
                None => Self::InvalidOutput,
            },
            ("uncertain" | "needs_admin", None) => Self::Uncertain,
            _ => Self::InvalidOutput,
        }
    }
}

/// Result returned by an evaluator for one [`EvaluationTicket`] (AC-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatorResult {
    pub evaluation_id: EvaluationId,
    pub target: EvaluationTarget,
    pub policy: PolicyVersion,
    pub outcome: EvaluatorOutcome,
}

/// Capability token representing the evaluator callback path, bound to the automation mode
/// read in the same commit (MOD-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluatorCapability(AutomationMode);

impl EvaluatorCapability {
    /// The evaluator path while automation is enabled. Until a durable automation store exists
    /// (`ADMIN-MODERATION-20261111` runtime gap) callers that have no mode use this.
    pub const fn evaluator() -> Self {
        Self(AutomationMode::Enabled)
    }

    /// The evaluator path under the mode read in the applying commit.
    pub const fn under(mode: AutomationMode) -> Self {
        Self(mode)
    }

    pub const fn mode(self) -> AutomationMode {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Open,
    Applied,
    Failed,
    /// The author made the work private while the job was open (PUB-06).
    Withdrawn,
    /// Automation was paused; the revision went to the human queue (MOD-14).
    Rerouted,
}

impl JobState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Applied => "applied",
            Self::Failed => "failed",
            Self::Withdrawn => "withdrawn",
            Self::Rerouted => "rerouted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationJobFacts {
    pub id: EvaluationId,
    pub target: EvaluationTarget,
    pub policy: PolicyVersion,
    pub state: JobState,
}

/// Refusal reasons when an [`EvaluatorResult`] cannot be applied, in exact guard precedence order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EvaluationRefusal {
    #[error("caller is not the moderation evaluator")]
    NotEvaluator,
    #[error("unknown evaluation job")]
    UnknownJob,
    #[error("the author withdrew the work while this job was open")]
    Withdrawn,
    #[error("automatic moderation is paused; the result has no effect")]
    AutomationPaused,
    #[error("evaluation job was already consumed")]
    Duplicate,
    #[error("evaluation result targets a different subject")]
    WrongTarget,
    #[error("evaluation result revision or content digest does not match")]
    WrongRevision,
    #[error("evaluation target is no longer active")]
    TargetGone,
    #[error("revision decision was already overridden by an admin")]
    Overridden,
    #[error("evaluation policy version is stale")]
    StalePolicy,
    #[error("revision was superseded and is no longer pending")]
    Superseded,
}

impl EvaluationRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotEvaluator => "not_evaluator",
            Self::UnknownJob => "unknown_job",
            Self::Withdrawn => "withdrawn",
            Self::AutomationPaused => "automation_paused",
            Self::Duplicate => "duplicate",
            Self::WrongTarget => "wrong_target",
            Self::WrongRevision => "wrong_revision",
            Self::TargetGone => "target_gone",
            Self::Overridden => "overridden",
            Self::StalePolicy => "stale_policy",
            Self::Superseded => "superseded",
        }
    }
}

/// Target-revision facts inspected by [`validate_evaluation_result`] (CHOICE-01 A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRevisionFacts {
    pub target: EvaluationTarget,
    pub target_active: bool,
    pub overridden_by_admin: bool,
    pub candidate_pending: bool,
}

/// Shared pure guard-precedence validator for work and comment evaluation results (CHOICE-01 A).
///
/// Enforces the exact refusal precedence from `publication.qnt` and `comment_edit.qnt`:
/// `NotEvaluator` -> `WrongTarget` (cross-subject consumer) -> `UnknownJob` -> `Withdrawn` ->
/// `AutomationPaused` -> `Duplicate` -> `WrongTarget` / `WrongRevision` -> `TargetGone` ->
/// `Overridden` (any human decision) -> `StalePolicy` -> `Superseded`.
pub fn validate_evaluation_result<'a>(
    capability: Option<EvaluatorCapability>,
    expected_subject: Option<EvaluationSubject>,
    jobs: &'a [EvaluationJobFacts],
    current_policy: PolicyVersion,
    result: &EvaluatorResult,
    lookup_revision: impl FnOnce(RevisionId) -> Option<TargetRevisionFacts>,
) -> Result<&'a EvaluationJobFacts, EvaluationRefusal> {
    let Some(capability) = capability else {
        return Err(EvaluationRefusal::NotEvaluator);
    };
    if let Some(subject) = expected_subject
        && !subject.matches(&result.target)
    {
        return Err(EvaluationRefusal::WrongTarget);
    }
    let job = jobs
        .iter()
        .find(|j| j.id == result.evaluation_id)
        .ok_or(EvaluationRefusal::UnknownJob)?;
    if job.state == JobState::Withdrawn {
        return Err(EvaluationRefusal::Withdrawn);
    }
    if job.state == JobState::Rerouted || capability.mode() == AutomationMode::Paused {
        return Err(EvaluationRefusal::AutomationPaused);
    }
    if job.state != JobState::Open {
        return Err(EvaluationRefusal::Duplicate);
    }
    if !job.target.same_subject(&result.target) {
        return Err(EvaluationRefusal::WrongTarget);
    }
    let Some(revision_facts) = lookup_revision(result.target.revision()) else {
        return Err(EvaluationRefusal::WrongRevision);
    };
    if !job.target.same_revision_and_digest(&result.target)
        || !job.target.same_revision_and_digest(&revision_facts.target)
    {
        return Err(EvaluationRefusal::WrongRevision);
    }
    if !revision_facts.target_active {
        return Err(EvaluationRefusal::TargetGone);
    }
    if revision_facts.overridden_by_admin {
        return Err(EvaluationRefusal::Overridden);
    }
    if job.policy != current_policy || result.policy != job.policy {
        return Err(EvaluationRefusal::StalePolicy);
    }
    if !revision_facts.candidate_pending {
        return Err(EvaluationRefusal::Superseded);
    }
    Ok(job)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionBy {
    Ai,
    Moderator,
    Admin,
    /// Waiting for a person after a policy change (PUB-06); no evaluator result decides it.
    PolicyRereview,
    Legacy,
}

impl DecisionBy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ai => "ai",
            Self::Moderator => "moderator",
            Self::Admin => "admin",
            Self::PolicyRereview => "rereview",
            Self::Legacy => "legacy",
        }
    }

    /// A person decided (or must decide): no evaluator result may change it (MOD-09).
    pub const fn is_human(self) -> bool {
        matches!(self, Self::Moderator | Self::Admin | Self::PolicyRereview)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeedsAdminCause {
    Uncertain,
    InvalidOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminVerdict {
    Approve,
    Reject,
}

/// Narrow capability token proving that `actor` may record an admin moderation decision on `work`
/// (#151, CHOICE-02 B, MOD-09).
///
/// Fields are private and `Deserialize` is not implemented so untrusted payloads cannot forge
/// authority. No global boolean or environment-variable constructor exists.
///
/// ```compile_fail
/// use vink_app::domain::moderation::AdminAuthority;
/// let authority: AdminAuthority = serde_json::from_str("{}").unwrap();
/// ```
/// Lawful twin (stable rustdoc ignores compile-fail error codes):
/// ```
/// use vink_app::domain::{
///     ids::{PrincipalId, WorkId},
///     moderation::AdminAuthority,
/// };
/// let raw: serde_json::Value = serde_json::from_str("{}").unwrap();
/// let actor = PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap();
/// let work = WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap();
/// let authority = AdminAuthority::scoped(actor, work);
/// assert_eq!(authority.actor(), actor);
/// assert_eq!(authority.work(), work);
/// assert!(raw.is_object());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminAuthority {
    actor: PrincipalId,
    work: WorkId,
}

impl AdminAuthority {
    pub const fn scoped(actor: PrincipalId, work: WorkId) -> Self {
        Self { actor, work }
    }

    pub const fn actor(self) -> PrincipalId {
        self.actor
    }

    pub const fn work(self) -> WorkId {
        self.work
    }
}

/// Validated admin override command for one submitted work revision (AC-05, MOD-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminDecision {
    pub revision: RevisionId,
    pub digest: ContentHash,
    pub expected: DecisionVersion,
    pub verdict: AdminVerdict,
    pub reason: DecisionReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionProvenance {
    AiApproved,
    AiRejected(PolicyReasonCode),
    AiNeedsAdmin(NeedsAdminCause),
    Admin {
        actor: PrincipalId,
        verdict: AdminVerdict,
        reason: DecisionReason,
    },
    /// A moderator decided under the named grant (MOD-11, MOD-13).
    Moderator {
        actor: PrincipalId,
        grant: GrantId,
        verdict: AdminVerdict,
        reason: DecisionReason,
    },
    /// A human approval of an older policy went back to the human queue (PUB-06).
    PolicyRereview,
    LegacyBootstrap,
}

impl DecisionProvenance {
    pub const fn decision_by(&self) -> DecisionBy {
        match self {
            Self::AiApproved | Self::AiRejected(_) | Self::AiNeedsAdmin(_) => DecisionBy::Ai,
            Self::Admin { .. } => DecisionBy::Admin,
            Self::Moderator { .. } => DecisionBy::Moderator,
            Self::PolicyRereview => DecisionBy::PolicyRereview,
            Self::LegacyBootstrap => DecisionBy::Legacy,
        }
    }

    const fn human_verdict(&self) -> Option<AdminVerdict> {
        match self {
            Self::Admin { verdict, .. } | Self::Moderator { verdict, .. } => Some(*verdict),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionDecisionFacts {
    pub revision: RevisionId,
    pub digest: ContentHash,
    pub version: DecisionVersion,
    /// Policy in force when the current decision (or submission, while pending) was made.
    pub policy: PolicyVersion,
    pub provenance: Option<DecisionProvenance>,
}

impl RevisionDecisionFacts {
    pub const fn decision_by(&self) -> Option<DecisionBy> {
        match &self.provenance {
            Some(provenance) => Some(provenance.decision_by()),
            None => None,
        }
    }
}

/// Untrusted row-loading shape for work moderation state; validated via [`WorkModeration::restore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkModerationFacts {
    pub work: WorkId,
    pub policy: PolicyVersion,
    pub revisions: Vec<RevisionDecisionFacts>,
    pub jobs: Vec<EvaluationJobFacts>,
}

/// Validated moderation aggregate paired with [`Publication`] (#151, CHOICE-02 B).
///
/// ```compile_fail
/// use vink_app::domain::moderation::WorkModeration;
/// let state: WorkModeration = serde_json::from_str("{}").unwrap();
/// ```
/// Lawful twin (stable rustdoc ignores compile-fail error codes):
/// ```
/// use vink_app::domain::{
///     ids::{PrincipalId, WorkId},
///     moderation::WorkModeration,
///     publication::Publication,
/// };
/// let raw: serde_json::Value = serde_json::from_str("{}").unwrap();
/// let work = WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap();
/// let owner = PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap();
/// let publication = Publication::new(work, owner);
/// let moderation = WorkModeration::new(work);
/// assert_eq!(
///     WorkModeration::restore(&publication, moderation.facts().clone()),
///     Ok(moderation)
/// );
/// assert!(raw.is_object());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkModeration(WorkModerationFacts);

/// How a submitted revision is routed by an author command (PUB-06, MOD-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRequest {
    /// Nothing to review (save, private, delete, waiting on an open job or a human case).
    None,
    /// First submission of an unsubmitted revision.
    Fresh,
    /// A pending revision whose job was withdrawn (or is stale) is reviewed again.
    Resubmitted,
    /// An approval of an older policy is reviewed again for the exact snapshot.
    PolicyRereview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorModerationTransition {
    pub publication: Publication,
    pub moderation: WorkModeration,
    pub outcome: AuthorOutcome,
    pub effects: Effects,
    /// Evaluator ticket, when automation is enabled and a review was requested.
    pub ticket: Option<EvaluationTicket>,
    pub review: ReviewRequest,
    /// Revision that must be in the human queue (paused automation, needs-admin, re-review of
    /// a human decision). The case owner opens or keeps its case.
    pub human_review: Option<RevisionId>,
    /// The author made the work private: the caller closes the work's cases and appeals.
    pub withdrawn_reviews: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedEvaluation {
    Published,
    ApprovedUnpublished,
    Rejected { reason: PolicyReasonCode },
    NeedsAdmin { cause: NeedsAdminCause },
    Retryable,
}

impl AppliedEvaluation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::ApprovedUnpublished => "approved_unpublished",
            Self::Rejected { .. } => "rejected",
            Self::NeedsAdmin { .. } => "needs_admin",
            Self::Retryable => "retryable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationTransition {
    pub publication: Publication,
    pub moderation: WorkModeration,
    pub outcome: AppliedEvaluation,
    pub effects: Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminOutcome {
    Published,
    ApprovedUnpublished,
    Rejected,
    Unpublished,
}

impl AdminOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::ApprovedUnpublished => "approved_unpublished",
            Self::Rejected => "rejected",
            Self::Unpublished => "unpublished",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminTransition {
    pub publication: Publication,
    pub moderation: WorkModeration,
    pub outcome: AdminOutcome,
    pub effects: Effects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminRemovalTransition {
    pub publication: Publication,
    pub moderation: WorkModeration,
    pub effects: Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReevaluateRefusal {
    #[error("work is not active")]
    WorkUnavailable,
    #[error("the work is private; its reviews were withdrawn")]
    Withdrawn,
    #[error("unknown revision")]
    UnknownRevision,
    #[error("revision is not pending evaluation")]
    NotPending,
    #[error("an open evaluation job already exists for this revision and policy")]
    JobAlreadyOpen,
}

impl ReevaluateRefusal {
    pub const fn as_quint_outcome(self) -> &'static str {
        "not_needed"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ModerationError {
    #[error("stored moderation facts are inconsistent with publication state")]
    InvalidFacts,
    #[error("policy version cannot advance")]
    PolicyVersionExhausted,
    #[error("decision version cannot advance")]
    DecisionVersionExhausted,
    #[error("evaluation ID already exists")]
    DuplicateEvaluationId,
    #[error("missing evaluation ID for submitted revision")]
    MissingEvaluationId,
    #[error("admin decision reason must be non-empty and free of control characters")]
    ReasonRequired,
    #[error("caller lacks narrow admin authority for this work")]
    NotAdmin,
    #[error("unknown revision")]
    UnknownRevision,
    #[error("target work is no longer active")]
    TargetGone,
    #[error("revision has not been submitted for moderation")]
    NotSubmitted,
    #[error("revision content hash does not match")]
    WrongHash,
    #[error("decision version changed")]
    StaleDecision,
    #[error("the author made the work private; its review cases are closed")]
    CaseClosed,
    #[error("authority does not cover work review of this work")]
    WrongAuthority,
    #[error(transparent)]
    Refused(#[from] EvaluationRefusal),
    #[error(transparent)]
    Reevaluate(#[from] ReevaluateRefusal),
    #[error(transparent)]
    Publication(#[from] PublicationError),
}

impl WorkModeration {
    pub fn new(work: WorkId) -> Self {
        Self(WorkModerationFacts {
            work,
            policy: PolicyVersion::FIRST,
            revisions: vec![],
            jobs: vec![],
        })
    }

    pub fn restore(
        publication: &Publication,
        facts: WorkModerationFacts,
    ) -> Result<Self, ModerationError> {
        let pub_facts = publication.facts();
        if facts.work != pub_facts.work {
            return Err(ModerationError::InvalidFacts);
        }
        for (i, rev) in facts.revisions.iter().enumerate() {
            if facts.revisions[..i]
                .iter()
                .any(|prior| prior.revision == rev.revision)
            {
                return Err(ModerationError::InvalidFacts);
            }
            let Some(pub_rev) = pub_facts
                .revisions
                .iter()
                .find(|r| r.snapshot.revision == rev.revision)
            else {
                return Err(ModerationError::InvalidFacts);
            };
            if &rev.digest != pub_rev.snapshot.content.hash() {
                return Err(ModerationError::InvalidFacts);
            }
            if rev.policy > facts.policy {
                return Err(ModerationError::InvalidFacts);
            }
            let human = rev
                .provenance
                .as_ref()
                .and_then(DecisionProvenance::human_verdict);
            match (pub_rev.moderation, &rev.provenance) {
                (Moderation::Unsubmitted, _) => return Err(ModerationError::InvalidFacts),
                // A first submission, or an AI re-review after a policy change (PUB-06).
                (Moderation::Pending, None) => {}
                (
                    Moderation::Approved,
                    Some(DecisionProvenance::AiApproved | DecisionProvenance::LegacyBootstrap),
                ) if rev.version > DecisionVersion::FIRST => {}
                (Moderation::Approved, Some(_))
                    if human == Some(AdminVerdict::Approve)
                        && rev.version > DecisionVersion::FIRST => {}
                (Moderation::Rejected, Some(DecisionProvenance::AiRejected(_)))
                    if rev.version > DecisionVersion::FIRST => {}
                (Moderation::Rejected, Some(_))
                    if human == Some(AdminVerdict::Reject)
                        && rev.version > DecisionVersion::FIRST => {}
                (
                    Moderation::NeedsAdmin,
                    Some(DecisionProvenance::AiNeedsAdmin(_) | DecisionProvenance::PolicyRereview),
                ) if rev.version > DecisionVersion::FIRST => {}
                _ => return Err(ModerationError::InvalidFacts),
            }
        }
        for pub_rev in &pub_facts.revisions {
            if pub_rev.moderation != Moderation::Unsubmitted
                && !facts
                    .revisions
                    .iter()
                    .any(|r| r.revision == pub_rev.snapshot.revision)
            {
                return Err(ModerationError::InvalidFacts);
            }
        }
        for (i, job) in facts.jobs.iter().enumerate() {
            if facts.jobs[..i].iter().any(|prior| prior.id == job.id) || job.policy > facts.policy {
                return Err(ModerationError::InvalidFacts);
            }
            let EvaluationTarget::WorkRevision {
                work,
                revision,
                digest,
            } = &job.target
            else {
                return Err(ModerationError::InvalidFacts);
            };
            if *work != facts.work {
                return Err(ModerationError::InvalidFacts);
            }
            let Some(pub_rev) = pub_facts
                .revisions
                .iter()
                .find(|r| r.snapshot.revision == *revision)
            else {
                return Err(ModerationError::InvalidFacts);
            };
            if pub_rev.moderation == Moderation::Unsubmitted
                || digest != pub_rev.snapshot.content.hash()
            {
                return Err(ModerationError::InvalidFacts);
            }
            if job.state == JobState::Open
                && facts.jobs[..i].iter().any(|prior| {
                    prior.state == JobState::Open
                        && prior.policy == job.policy
                        && prior.target.revision() == *revision
                })
            {
                return Err(ModerationError::InvalidFacts);
            }
        }
        Ok(Self(facts))
    }

    pub fn facts(&self) -> &WorkModerationFacts {
        &self.0
    }

    pub fn revision_decision(&self, revision: RevisionId) -> Option<&RevisionDecisionFacts> {
        self.0.revisions.iter().find(|r| r.revision == revision)
    }

    /// Execute an author command with automation enabled. See [`Self::decide_author_with`].
    pub fn decide_author(
        &self,
        publication: &Publication,
        actor: PrincipalId,
        command: AuthorCommand,
        new_evaluation_id: Option<EvaluationId>,
    ) -> Result<AuthorModerationTransition, ModerationError> {
        self.decide_author_with(
            publication,
            actor,
            command,
            new_evaluation_id,
            AutomationMode::Enabled,
        )
    }

    fn open_job(
        &mut self,
        request: &EvaluationRequest,
        evaluation_id: Option<EvaluationId>,
    ) -> Result<EvaluationTicket, ModerationError> {
        let evaluation_id = evaluation_id.ok_or(ModerationError::MissingEvaluationId)?;
        if self.0.jobs.iter().any(|j| j.id == evaluation_id) {
            return Err(ModerationError::DuplicateEvaluationId);
        }
        let ticket = EvaluationTicket::for_work_revision(evaluation_id, request, self.0.policy);
        self.0.jobs.push(EvaluationJobFacts {
            id: ticket.id,
            target: ticket.target.clone(),
            policy: ticket.policy,
            state: JobState::Open,
        });
        Ok(ticket)
    }

    fn has_open_job(&self, revision: RevisionId) -> bool {
        self.0.jobs.iter().any(|j| {
            j.state == JobState::Open
                && j.policy == self.0.policy
                && j.target.revision() == revision
        })
    }

    /// Execute an author command against [`Publication`] and route review (PUB-06, MOD-05,
    /// MOD-14, INV-08):
    /// - a private save opens nothing;
    /// - a first submission, or a pending revision whose job was withdrawn, goes to the
    ///   evaluator (or to the human queue while automation is paused);
    /// - republishing an approval of an older policy re-reviews the exact snapshot: an AI (or
    ///   legacy) approval goes back to the evaluator, a human approval to the human queue;
    /// - making the work private withdraws every open job and tells the caller to close the
    ///   work's cases and appeals; decisions already made are kept.
    pub fn decide_author_with(
        &self,
        publication: &Publication,
        actor: PrincipalId,
        command: AuthorCommand,
        new_evaluation_id: Option<EvaluationId>,
        automation: AutomationMode,
    ) -> Result<AuthorModerationTransition, ModerationError> {
        if self.0.work != publication.facts().work {
            return Err(ModerationError::InvalidFacts);
        }
        let mut next = self.clone();
        let mut source = publication.clone();
        let mut review = ReviewRequest::None;
        let mut human_review = None;

        // PUB-06: an approval of an older policy is not reused for a new public intent.
        if let AuthorCommand::RequestPublic { revision, expected } = &command
            && actor == publication.facts().owner
            && publication.facts().lifecycle.is_active()
            && *expected == publication.facts().generation
            && publication
                .facts()
                .revisions
                .iter()
                .any(|r| r.snapshot.revision == *revision && r.moderation == Moderation::Approved)
            && publication.readable_revision().map(|s| s.revision) != Some(*revision)
            && let Some(decision) = self.revision_decision(*revision)
            && decision.policy < self.0.policy
        {
            let human = decision
                .provenance
                .as_ref()
                .is_some_and(|p| p.decision_by().is_human());
            let to = if human {
                Moderation::NeedsAdmin
            } else {
                Moderation::Pending
            };
            source = publication.reopen_review(*revision, to)?;
            let slot = next
                .0
                .revisions
                .iter_mut()
                .find(|r| r.revision == *revision)
                .ok_or(ModerationError::InvalidFacts)?;
            slot.version = slot.version.next()?;
            slot.policy = self.0.policy;
            slot.provenance = human.then_some(DecisionProvenance::PolicyRereview);
            review = ReviewRequest::PolicyRereview;
        }

        let transition = source.decide_author(actor, command.clone())?;

        match (&command, transition.outcome) {
            (AuthorCommand::SetPrivate { .. }, _) => {
                for job in &mut next.0.jobs {
                    if matches!(job.state, JobState::Open | JobState::Rerouted) {
                        job.state = JobState::Withdrawn;
                    }
                }
            }
            (AuthorCommand::RequestPublic { revision, .. }, outcome) => {
                let moderation = transition
                    .next
                    .facts()
                    .revisions
                    .iter()
                    .find(|r| r.snapshot.revision == *revision)
                    .map(|r| r.moderation);
                if outcome == AuthorOutcome::Submitted {
                    next.0.revisions.push(RevisionDecisionFacts {
                        revision: *revision,
                        digest: transition
                            .effects
                            .evaluation
                            .as_ref()
                            .map(|e| e.hash.clone())
                            .ok_or(ModerationError::InvalidFacts)?,
                        version: DecisionVersion::FIRST,
                        policy: self.0.policy,
                        provenance: None,
                    });
                    review = ReviewRequest::Fresh;
                } else if review == ReviewRequest::None
                    && moderation == Some(Moderation::Pending)
                    && !next.has_open_job(*revision)
                {
                    review = ReviewRequest::Resubmitted;
                }
                if moderation == Some(Moderation::NeedsAdmin) {
                    human_review = Some(*revision);
                }
            }
            _ => {}
        }

        let ticket = if matches!(
            review,
            ReviewRequest::Fresh | ReviewRequest::Resubmitted | ReviewRequest::PolicyRereview
        ) && human_review.is_none()
        {
            let AuthorCommand::RequestPublic { revision, .. } = &command else {
                return Err(ModerationError::InvalidFacts);
            };
            if automation == AutomationMode::Paused {
                human_review = Some(*revision);
                None
            } else {
                let request = match &transition.effects.evaluation {
                    Some(request) => request.clone(),
                    None => EvaluationRequest {
                        work: self.0.work,
                        revision: *revision,
                        hash: transition
                            .next
                            .facts()
                            .revisions
                            .iter()
                            .find(|r| r.snapshot.revision == *revision)
                            .map(|r| r.snapshot.content.hash().clone())
                            .ok_or(ModerationError::InvalidFacts)?,
                    },
                };
                Some(next.open_job(&request, new_evaluation_id)?)
            }
        } else {
            None
        };

        Ok(AuthorModerationTransition {
            publication: transition.next,
            moderation: next,
            outcome: transition.outcome,
            effects: transition.effects,
            ticket,
            review,
            human_review,
            withdrawn_reviews: matches!(command, AuthorCommand::SetPrivate { .. }),
        })
    }

    /// Open a fresh [`EvaluationTicket`] for a still-pending revision after a retryable failure or
    /// policy bump (`MOD-08`). Never reuses a consumed [`EvaluationId`].
    pub fn reevaluate(
        &self,
        publication: &Publication,
        revision: RevisionId,
        evaluation_id: EvaluationId,
    ) -> Result<(Self, EvaluationTicket), ModerationError> {
        if self.0.work != publication.facts().work {
            return Err(ModerationError::InvalidFacts);
        }
        if !publication.facts().lifecycle.is_active() {
            return Err(ReevaluateRefusal::WorkUnavailable.into());
        }
        if publication.facts().audience == Audience::Private {
            return Err(ReevaluateRefusal::Withdrawn.into());
        }
        let Some(pub_rev) = publication
            .facts()
            .revisions
            .iter()
            .find(|r| r.snapshot.revision == revision)
        else {
            return Err(ReevaluateRefusal::UnknownRevision.into());
        };
        if pub_rev.moderation != Moderation::Pending {
            return Err(ReevaluateRefusal::NotPending.into());
        }
        if self.has_open_job(revision) {
            return Err(ReevaluateRefusal::JobAlreadyOpen.into());
        }
        let mut next = self.clone();
        let ticket = next.open_job(
            &EvaluationRequest {
                work: self.0.work,
                revision,
                hash: pub_rev.snapshot.content.hash().clone(),
            },
            Some(evaluation_id),
        )?;
        Ok((next, ticket))
    }

    pub fn bump_policy(&self) -> Result<Self, ModerationError> {
        let mut next = self.clone();
        next.0.policy = self.0.policy.next()?;
        Ok(next)
    }

    /// Pause reroute (MOD-14): every open job leaves the evaluator; the returned revisions are
    /// those still waiting for a decision, which go to the human queue (a revision a person
    /// already decided keeps that decision). Results that still arrive are refused
    /// `AutomationPaused`.
    pub fn reroute_to_humans(&self) -> (Self, Vec<RevisionId>) {
        let mut next = self.clone();
        let mut revisions = Vec::new();
        for job in &mut next.0.jobs {
            if job.state == JobState::Open {
                job.state = JobState::Rerouted;
                let revision = job.target.revision();
                let undecided = self
                    .revision_decision(revision)
                    .is_some_and(|d| d.provenance.is_none());
                if undecided && !revisions.contains(&revision) {
                    revisions.push(revision);
                }
            }
        }
        (next, revisions)
    }

    /// Validate and apply an [`EvaluatorResult`] to [`WorkModeration`] and [`Publication`].
    ///
    /// - Refused results (`Err`) never mutate job state, decision state or publication state.
    /// - `Timeout` and `ProviderError` consume the job (`Failed` / `Retryable`) without changing
    ///   revision moderation or the public pointer.
    /// - Decided outcomes advance [`DecisionVersion`] and call [`Publication::record_review`].
    pub fn apply_evaluator_result(
        &self,
        publication: &Publication,
        capability: Option<EvaluatorCapability>,
        result: &EvaluatorResult,
    ) -> Result<EvaluationTransition, ModerationError> {
        if self.0.work != publication.facts().work {
            return Err(ModerationError::InvalidFacts);
        }
        let pub_facts = publication.facts();
        let job = validate_evaluation_result(
            capability,
            Some(EvaluationSubject::Work(self.0.work)),
            &self.0.jobs,
            self.0.policy,
            result,
            |revision| {
                let pub_rev = pub_facts
                    .revisions
                    .iter()
                    .find(|r| r.snapshot.revision == revision)?;
                let rev_decision = self.revision_decision(revision);
                Some(TargetRevisionFacts {
                    target: EvaluationTarget::WorkRevision {
                        work: pub_facts.work,
                        revision,
                        digest: pub_rev.snapshot.content.hash().clone(),
                    },
                    target_active: pub_facts.lifecycle.is_active(),
                    overridden_by_admin: rev_decision
                        .and_then(RevisionDecisionFacts::decision_by)
                        .is_some_and(DecisionBy::is_human),
                    candidate_pending: pub_rev.moderation == Moderation::Pending,
                })
            },
        )?;
        let job_policy = job.policy;

        let revision = result.target.revision();
        let mut next = self.clone();
        let job_slot = next
            .0
            .jobs
            .iter_mut()
            .find(|j| j.id == result.evaluation_id)
            .ok_or(EvaluationRefusal::UnknownJob)?;

        let (review_outcome, provenance, needs_admin_cause, reject_reason) = match result.outcome {
            EvaluatorOutcome::Timeout | EvaluatorOutcome::ProviderError => {
                job_slot.state = JobState::Failed;
                return Ok(EvaluationTransition {
                    publication: publication.clone(),
                    moderation: next,
                    outcome: AppliedEvaluation::Retryable,
                    effects: Effects {
                        evaluation: None,
                        pointer: PointerEffect::Unchanged,
                    },
                });
            }
            EvaluatorOutcome::Approve => (
                ReviewOutcome::Approved,
                DecisionProvenance::AiApproved,
                None,
                None,
            ),
            EvaluatorOutcome::Reject(reason) => (
                ReviewOutcome::Rejected,
                DecisionProvenance::AiRejected(reason),
                None,
                Some(reason),
            ),
            EvaluatorOutcome::Uncertain => (
                ReviewOutcome::NeedsAdmin,
                DecisionProvenance::AiNeedsAdmin(NeedsAdminCause::Uncertain),
                Some(NeedsAdminCause::Uncertain),
                None,
            ),
            EvaluatorOutcome::InvalidOutput => (
                ReviewOutcome::NeedsAdmin,
                DecisionProvenance::AiNeedsAdmin(NeedsAdminCause::InvalidOutput),
                Some(NeedsAdminCause::InvalidOutput),
                None,
            ),
        };

        job_slot.state = JobState::Applied;
        let rev_slot = next
            .0
            .revisions
            .iter_mut()
            .find(|r| r.revision == revision)
            .ok_or(ModerationError::InvalidFacts)?;
        rev_slot.version = rev_slot.version.next()?;
        rev_slot.policy = job_policy;
        rev_slot.provenance = Some(provenance);

        let review = publication.record_review(ReviewedRevision {
            work: self.0.work,
            revision,
            hash: rev_slot.digest.clone(),
            expected: Moderation::Pending,
            outcome: review_outcome,
        })?;

        let outcome = match review_outcome {
            ReviewOutcome::Approved => {
                if matches!(review.effects.pointer, PointerEffect::Set(_)) {
                    AppliedEvaluation::Published
                } else {
                    AppliedEvaluation::ApprovedUnpublished
                }
            }
            ReviewOutcome::Rejected => AppliedEvaluation::Rejected {
                reason: reject_reason.unwrap_or(PolicyReasonCode::SafetyPolicy),
            },
            ReviewOutcome::NeedsAdmin => AppliedEvaluation::NeedsAdmin {
                cause: needs_admin_cause.unwrap_or(NeedsAdminCause::Uncertain),
            },
        };

        Ok(EvaluationTransition {
            publication: review.next,
            moderation: next,
            outcome,
            effects: review.effects,
        })
    }

    /// Shared human decision path (admin or moderator). Checks lifecycle, private withdrawal,
    /// submission, digest and the decision-version fence, then records `provenance` and runs the
    /// publication guard.
    fn apply_human(
        &self,
        publication: &Publication,
        decision: AdminDecision,
        provenance: DecisionProvenance,
    ) -> Result<AdminTransition, ModerationError> {
        let pub_facts = publication.facts();
        let Some(pub_rev) = pub_facts
            .revisions
            .iter()
            .find(|r| r.snapshot.revision == decision.revision)
        else {
            return Err(ModerationError::UnknownRevision);
        };
        if !pub_facts.lifecycle.is_active() {
            return Err(ModerationError::TargetGone);
        }
        if pub_rev.moderation == Moderation::Unsubmitted {
            return Err(ModerationError::NotSubmitted);
        }
        // PUB-06: making the work private closed its review cases and appeals.
        if pub_facts.audience == Audience::Private {
            return Err(ModerationError::CaseClosed);
        }
        if &decision.digest != pub_rev.snapshot.content.hash() {
            return Err(ModerationError::WrongHash);
        }
        let Some(current_decision) = self.revision_decision(decision.revision) else {
            return Err(ModerationError::InvalidFacts);
        };
        if decision.expected != current_decision.version {
            return Err(ModerationError::StaleDecision);
        }

        let mut next = self.clone();
        let rev_slot = next
            .0
            .revisions
            .iter_mut()
            .find(|r| r.revision == decision.revision)
            .ok_or(ModerationError::InvalidFacts)?;
        rev_slot.version = rev_slot.version.next()?;
        rev_slot.policy = self.0.policy;
        rev_slot.provenance = Some(provenance);

        let review_outcome = match decision.verdict {
            AdminVerdict::Approve => ReviewOutcome::Approved,
            AdminVerdict::Reject => ReviewOutcome::Rejected,
        };
        let review = publication.record_review(ReviewedRevision {
            work: self.0.work,
            revision: decision.revision,
            hash: decision.digest,
            expected: pub_rev.moderation,
            outcome: review_outcome,
        })?;

        let outcome = match decision.verdict {
            AdminVerdict::Approve => {
                if matches!(review.effects.pointer, PointerEffect::Set(_))
                    || review
                        .next
                        .facts()
                        .pointer
                        .is_some_and(|p| p.revision == decision.revision)
                {
                    AdminOutcome::Published
                } else {
                    AdminOutcome::ApprovedUnpublished
                }
            }
            AdminVerdict::Reject => {
                if matches!(review.effects.pointer, PointerEffect::Clear) {
                    AdminOutcome::Unpublished
                } else {
                    AdminOutcome::Rejected
                }
            }
        };

        Ok(AdminTransition {
            publication: review.next,
            moderation: next,
            outcome,
            effects: review.effects,
        })
    }

    /// Validate and apply a narrow-authority admin decision (`AC-05`, `MOD-09`, `INV-02`).
    ///
    /// Requires an [`AdminAuthority`] scoped to `self.work` whose actor is not the work's author,
    /// an existing submitted revision (`!= Unsubmitted`), an active, non-private work, matching
    /// content digest, and matching `expected: DecisionVersion`. Delegates pointer promotion or
    /// unpublication to [`Publication::record_review`].
    pub fn apply_admin_decision(
        &self,
        publication: &Publication,
        authority: Option<AdminAuthority>,
        decision: AdminDecision,
    ) -> Result<AdminTransition, ModerationError> {
        if self.0.work != publication.facts().work {
            return Err(ModerationError::InvalidFacts);
        }
        let Some(authority) = authority else {
            return Err(ModerationError::NotAdmin);
        };
        if authority.work() != self.0.work || authority.actor() == publication.facts().owner {
            return Err(ModerationError::NotAdmin);
        }
        let provenance = DecisionProvenance::Admin {
            actor: authority.actor(),
            verdict: decision.verdict,
            reason: decision.reason.clone(),
        };
        self.apply_human(publication, decision, provenance)
    }

    /// A human decision under a freshly [`VerifiedAuthority`] of work review for this work
    /// (MOD-11, MOD-13): a moderator's grant or an admin seat, recorded in the provenance.
    pub fn apply_human_decision(
        &self,
        publication: &Publication,
        authority: &VerifiedAuthority,
        decision: AdminDecision,
    ) -> Result<AdminTransition, ModerationError> {
        if self.0.work != publication.facts().work {
            return Err(ModerationError::InvalidFacts);
        }
        let target = authority.target();
        if target.capability != ReviewCapability::Works
            || target.work != self.0.work
            || target.content_owner != publication.facts().owner
        {
            return Err(ModerationError::WrongAuthority);
        }
        let provenance = match authority.basis() {
            AuthorityBasis::Admin => DecisionProvenance::Admin {
                actor: authority.actor(),
                verdict: decision.verdict,
                reason: decision.reason.clone(),
            },
            AuthorityBasis::Grant(grant) => DecisionProvenance::Moderator {
                actor: authority.actor(),
                grant,
                verdict: decision.verdict,
                reason: decision.reason.clone(),
            },
        };
        self.apply_human(publication, decision, provenance)
    }

    /// Apply a concluded case on a work revision (MOD-12). The case already fenced its own
    /// version and the decision version it saw; this re-fences the decision version here.
    pub fn apply_case_conclusion(
        &self,
        publication: &Publication,
        conclusion: &CaseConclusion,
    ) -> Result<AdminTransition, ModerationError> {
        let ReviewSubject::WorkRevision {
            work,
            revision,
            digest,
            decision,
        } = &conclusion.subject
        else {
            return Err(ModerationError::WrongAuthority);
        };
        if *work != self.0.work {
            return Err(ModerationError::InvalidFacts);
        }
        let reason = match &conclusion.reason {
            Some(reason) => reason.clone(),
            None => DecisionReason::parse("approved on review")?,
        };
        self.apply_human_decision(
            publication,
            &conclusion.authority,
            AdminDecision {
                revision: *revision,
                digest: digest.clone(),
                expected: *decision,
                verdict: match conclusion.verdict {
                    CaseVerdict::Approve => AdminVerdict::Approve,
                    CaseVerdict::Reject => AdminVerdict::Reject,
                },
                reason,
            },
        )
    }

    /// Apply an admin's appeal conclusion (MOD-10): uphold keeps the verdict, reverse flips it;
    /// both create a new admin decision version. Publishing still needs the author's current
    /// intent on that exact revision.
    pub fn apply_appeal_conclusion(
        &self,
        publication: &Publication,
        conclusion: &AppealConclusion,
    ) -> Result<AdminTransition, ModerationError> {
        let ReviewSubject::WorkRevision {
            work,
            revision,
            digest,
            decision,
        } = &conclusion.subject
        else {
            return Err(ModerationError::WrongAuthority);
        };
        if *work != self.0.work || conclusion.admin.actor() == publication.facts().owner {
            return Err(ModerationError::NotAdmin);
        }
        let current = publication
            .facts()
            .revisions
            .iter()
            .find(|r| r.snapshot.revision == *revision)
            .ok_or(ModerationError::UnknownRevision)?
            .moderation;
        let appealed = match current {
            Moderation::Approved => AdminVerdict::Approve,
            Moderation::Rejected => AdminVerdict::Reject,
            _ => return Err(ModerationError::StaleDecision),
        };
        let verdict = match (conclusion.verdict, appealed) {
            (AppealVerdict::Uphold, v) => v,
            (AppealVerdict::Reverse, AdminVerdict::Approve) => AdminVerdict::Reject,
            (AppealVerdict::Reverse, AdminVerdict::Reject) => AdminVerdict::Approve,
        };
        let provenance = DecisionProvenance::Admin {
            actor: conclusion.admin.actor(),
            verdict,
            reason: conclusion.reason.clone(),
        };
        self.apply_human(
            publication,
            AdminDecision {
                revision: *revision,
                digest: digest.clone(),
                expected: *decision,
                verdict,
                reason: conclusion.reason.clone(),
            },
            provenance,
        )
    }

    /// Apply an admin removal to an active work (#157, AC-01, AC-05).
    ///
    /// Same narrow authority as [`Self::apply_admin_decision`]: scoped to `self.work`, and the
    /// work's author recuses (an author deletes through `AuthorCommand::Delete`, which keeps a
    /// lawful tombstone; removal would hide valid comments instead).
    pub fn apply_admin_remove(
        &self,
        publication: &Publication,
        authority: Option<AdminAuthority>,
    ) -> Result<AdminRemovalTransition, ModerationError> {
        if self.0.work != publication.facts().work {
            return Err(ModerationError::InvalidFacts);
        }
        let authority = authority.ok_or(ModerationError::NotAdmin)?;
        if authority.work() != self.0.work || authority.actor() == publication.facts().owner {
            return Err(ModerationError::NotAdmin);
        }
        if !publication.facts().lifecycle.is_active() {
            return Err(ModerationError::TargetGone);
        }
        let (next_pub, effects) = publication.admin_remove()?;
        Ok(AdminRemovalTransition {
            publication: next_pub,
            moderation: self.clone(),
            effects,
        })
    }
}

/// Synchronous evaluator contract for one [`EvaluationTicket`] (#151, AC-01, AC-02).
pub trait ModerationEvaluator {
    fn evaluate(&mut self, ticket: &EvaluationTicket) -> EvaluatorResult;
}

/// The nine canonical mock outcome classes from `evaluator-vectors.json` (#151, AC-02, CHOICE-03 A).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum ScriptKind {
    Approve,
    Reject,
    NeedsAdmin,
    Timeout,
    ProviderError,
    InvalidOutput,
    DelayedResult,
    DuplicateResult,
    WrongRevision,
}

impl ScriptKind {
    pub const ALL: [Self; 9] = [
        Self::Approve,
        Self::Reject,
        Self::NeedsAdmin,
        Self::Timeout,
        Self::ProviderError,
        Self::InvalidOutput,
        Self::DelayedResult,
        Self::DuplicateResult,
        Self::WrongRevision,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedStep {
    pub kind: ScriptKind,
    pub result: EvaluatorResult,
}

impl ScriptedStep {
    pub fn approve(ticket: &EvaluationTicket) -> Self {
        Self {
            kind: ScriptKind::Approve,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome: EvaluatorOutcome::Approve,
            },
        }
    }

    pub fn reject(ticket: &EvaluationTicket, reason: PolicyReasonCode) -> Self {
        Self {
            kind: ScriptKind::Reject,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome: EvaluatorOutcome::Reject(reason),
            },
        }
    }

    pub fn needs_admin(ticket: &EvaluationTicket) -> Self {
        Self {
            kind: ScriptKind::NeedsAdmin,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome: EvaluatorOutcome::Uncertain,
            },
        }
    }

    pub fn timeout(ticket: &EvaluationTicket) -> Self {
        Self {
            kind: ScriptKind::Timeout,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome: EvaluatorOutcome::Timeout,
            },
        }
    }

    pub fn provider_error(ticket: &EvaluationTicket) -> Self {
        Self {
            kind: ScriptKind::ProviderError,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome: EvaluatorOutcome::ProviderError,
            },
        }
    }

    pub fn invalid_output(ticket: &EvaluationTicket, raw_json: &str) -> Self {
        let parsed = EvaluatorOutcome::from_raw_json(raw_json);
        let outcome = if matches!(parsed, EvaluatorOutcome::InvalidOutput) {
            parsed
        } else {
            EvaluatorOutcome::InvalidOutput
        };
        Self {
            kind: ScriptKind::InvalidOutput,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome,
            },
        }
    }

    pub fn wrong_revision(
        ticket: &EvaluationTicket,
        wrong_target: EvaluationTarget,
        outcome: EvaluatorOutcome,
    ) -> Self {
        Self {
            kind: ScriptKind::WrongRevision,
            result: EvaluatorResult {
                evaluation_id: ticket.id,
                target: wrong_target,
                policy: ticket.policy,
                outcome,
            },
        }
    }

    pub fn duplicate_result(consumed_ticket: &EvaluationTicket, outcome: EvaluatorOutcome) -> Self {
        Self {
            kind: ScriptKind::DuplicateResult,
            result: EvaluatorResult {
                evaluation_id: consumed_ticket.id,
                target: consumed_ticket.target.clone(),
                policy: consumed_ticket.policy,
                outcome,
            },
        }
    }

    pub fn delayed_result(stale_ticket: &EvaluationTicket, outcome: EvaluatorOutcome) -> Self {
        Self {
            kind: ScriptKind::DelayedResult,
            result: EvaluatorResult {
                evaluation_id: stale_ticket.id,
                target: stale_ticket.target.clone(),
                policy: stale_ticket.policy,
                outcome,
            },
        }
    }
}

/// Canonical JSON representation of `services/vink-app/behavior/target/evaluator-vectors.json`
/// (`vink-moderation-evaluator-script-v1`, CHOICE-03 A).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalEvaluatorVectors {
    pub format: String,
    pub status: String,
    pub quint_version: String,
    pub note: String,
    pub scripts: Vec<CanonicalEvaluatorScript>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalEvaluatorScript {
    pub name: String,
    pub steps: Vec<CanonicalScriptStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalScriptStep {
    pub model: String,
    pub scenario: String,
    pub step: usize,
    pub script: ScriptKind,
    pub result: CanonicalScriptResult,
    pub applied: String,
    pub public_revision: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalScriptResult {
    pub job: u128,
    pub revision: u128,
    pub outcome: String,
}

impl CanonicalScriptResult {
    pub fn parse_outcome(&self) -> Option<EvaluatorOutcome> {
        match self.outcome.as_str() {
            "approve" => Some(EvaluatorOutcome::Approve),
            "reject" => Some(EvaluatorOutcome::Reject(PolicyReasonCode::SafetyPolicy)),
            "uncertain" => Some(EvaluatorOutcome::Uncertain),
            "timeout" => Some(EvaluatorOutcome::Timeout),
            "provider_error" => Some(EvaluatorOutcome::ProviderError),
            "invalid_output" => Some(EvaluatorOutcome::from_raw_json(
                r#"{"verdict":"invalid_output","authority":"forged"}"#,
            )),
            _ => None,
        }
    }
}

/// Deterministic local/test-only scripted evaluator (#151, CHOICE-03 A, AC-02).
///
/// Steps through an explicit queue of [`ScriptedStep`]s with no network, API key, clock/sleep or
/// manuscript logging. Calling [`Self::evaluate`] when the script queue is exhausted returns
/// [`EvaluatorOutcome::ProviderError`] and increments [`Self::unscripted_calls`]; it never falls
/// back to [`EvaluatorOutcome::Approve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedEvaluator {
    queue: std::collections::VecDeque<ScriptedStep>,
    executed: Vec<ScriptKind>,
    unscripted_calls: usize,
}

impl ScriptedEvaluator {
    pub const LOCAL_TEST_ONLY_LABEL: &'static str = "local_test_scripted_evaluator";

    pub fn new(steps: impl IntoIterator<Item = ScriptedStep>) -> Self {
        Self {
            queue: steps.into_iter().collect(),
            executed: Vec::new(),
            unscripted_calls: 0,
        }
    }

    pub fn push(&mut self, step: ScriptedStep) {
        self.queue.push_back(step);
    }

    pub fn remaining(&self) -> usize {
        self.queue.len()
    }

    pub fn executed_kinds(&self) -> &[ScriptKind] {
        &self.executed
    }

    pub const fn unscripted_calls(&self) -> usize {
        self.unscripted_calls
    }
}

impl ModerationEvaluator for ScriptedEvaluator {
    fn evaluate(&mut self, ticket: &EvaluationTicket) -> EvaluatorResult {
        if let Some(step) = self.queue.pop_front() {
            self.executed.push(step.kind);
            step.result
        } else {
            self.unscripted_calls += 1;
            EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target.clone(),
                policy: ticket.policy,
                outcome: EvaluatorOutcome::ProviderError,
            }
        }
    }
}

#[cfg(test)]
mod governance_replay;
#[cfg(test)]
mod governance_tests;
#[cfg(test)]
mod replay;
#[cfg(test)]
pub(crate) mod tests;
