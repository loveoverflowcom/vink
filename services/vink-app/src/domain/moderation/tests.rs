use super::*;
use crate::domain::discussion::submission::PayloadDigest;
use crate::domain::publication::PublicPointer;
use crate::domain::publication::tests::{generation, owner, revision, snapshot, work};

pub(crate) fn admin() -> PrincipalId {
    PrincipalId::from_uuid(uuid::Uuid::from_u128(3))
}

pub(crate) fn evaluation_id(n: u128) -> EvaluationId {
    EvaluationId::from_uuid(uuid::Uuid::from_u128(n))
}

pub(crate) fn comment(n: u128) -> CommentId {
    CommentId::from_uuid(uuid::Uuid::from_u128(n))
}

pub(crate) fn reason(text: &str) -> DecisionReason {
    DecisionReason::parse(text).unwrap()
}

fn save_and_submit(
    publication: &Publication,
    moderation: &WorkModeration,
    rev: u128,
    eval: u128,
) -> (Publication, WorkModeration, EvaluationTicket) {
    let saved = moderation
        .decide_author(
            publication,
            owner(),
            AuthorCommand::SaveSnapshot(snapshot(rev)),
            None,
        )
        .unwrap();
    assert_eq!(saved.outcome, AuthorOutcome::Saved);
    assert_eq!(saved.ticket, None);
    let submitted = saved
        .moderation
        .decide_author(
            &saved.publication,
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(rev),
                expected: saved.publication.facts().generation,
            },
            Some(evaluation_id(eval)),
        )
        .unwrap();
    assert_eq!(submitted.outcome, AuthorOutcome::Submitted);
    let ticket = submitted.ticket.expect("submission must issue a ticket");
    (submitted.publication, submitted.moderation, ticket)
}

#[test]
fn private_saves_open_no_evaluation_and_only_submitted_revisions_receive_tickets() {
    let publication = Publication::new(work(1), owner());
    let moderation = WorkModeration::new(work(1));
    let saved = moderation
        .decide_author(
            &publication,
            owner(),
            AuthorCommand::SaveSnapshot(snapshot(1)),
            None,
        )
        .unwrap();
    assert_eq!(saved.outcome, AuthorOutcome::Saved);
    assert_eq!(saved.ticket, None);
    assert!(saved.moderation.facts().jobs.is_empty());
    assert!(saved.moderation.facts().revisions.is_empty());

    // Submitting without an EvaluationId fails cleanly without mutating state.
    assert_eq!(
        saved.moderation.decide_author(
            &saved.publication,
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(1),
                expected: saved.publication.facts().generation,
            },
            None,
        ),
        Err(ModerationError::MissingEvaluationId)
    );

    let (pub_state, mod_state, ticket) = save_and_submit(&publication, &moderation, 1, 1);
    assert_eq!(ticket.id, evaluation_id(1));
    assert_eq!(ticket.policy, PolicyVersion::FIRST);
    assert_eq!(
        ticket.target,
        EvaluationTarget::WorkRevision {
            work: work(1),
            revision: revision(1),
            digest: snapshot(1).content.hash().clone(),
        }
    );
    assert_eq!(mod_state.facts().jobs.len(), 1);
    assert_eq!(mod_state.facts().jobs[0].state, JobState::Open);
    assert_eq!(
        mod_state.revision_decision(revision(1)).unwrap().version,
        DecisionVersion::FIRST
    );
    assert_eq!(pub_state.readable_revision(), None);
}

#[test]
fn raw_evaluator_outputs_reject_unknown_fields_and_route_uncertainty_to_needs_admin() {
    assert_eq!(
        EvaluatorOutcome::from_raw_json(r#"{"verdict":"approve"}"#),
        EvaluatorOutcome::Approve
    );
    assert_eq!(
        EvaluatorOutcome::from_raw_json(r#"{"verdict":"reject","reason_code":"hate_speech"}"#),
        EvaluatorOutcome::Reject(PolicyReasonCode::HateSpeech)
    );
    assert_eq!(
        EvaluatorOutcome::from_raw_json(r#"{"verdict":"uncertain"}"#),
        EvaluatorOutcome::Uncertain
    );
    for invalid in [
        "",
        "{}",
        r#"{"verdict":"approve","reason_code":"safety_policy"}"#,
        r#"{"verdict":"reject"}"#,
        r#"{"verdict":"reject","reason_code":"made_up_code"}"#,
        r#"{"verdict":"approve","pointer":"publish_now"}"#,
        r#"{"verdict":"approve","override_admin":true}"#,
    ] {
        assert_eq!(
            EvaluatorOutcome::from_raw_json(invalid),
            EvaluatorOutcome::InvalidOutput,
            "raw payload {invalid} must classify as InvalidOutput"
        );
    }

    let (pub_state, mod_state, ticket) = save_and_submit(
        &Publication::new(work(1), owner()),
        &WorkModeration::new(work(1)),
        1,
        1,
    );
    let transition = mod_state
        .apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket.id,
                target: ticket.target,
                policy: ticket.policy,
                outcome: EvaluatorOutcome::from_raw_json(
                    r#"{"verdict":"approve","authority":"forged"}"#,
                ),
            },
        )
        .unwrap();
    assert_eq!(
        transition.outcome,
        AppliedEvaluation::NeedsAdmin {
            cause: NeedsAdminCause::InvalidOutput
        }
    );
    assert_eq!(
        transition.publication.facts().revisions[0].moderation,
        Moderation::NeedsAdmin
    );
    assert_eq!(transition.publication.readable_revision(), None);
}

#[test]
fn timeout_and_provider_error_consume_the_attempt_without_publishing_and_require_fresh_evaluation_id()
 {
    let (pub_state, mod_state, ticket1) = save_and_submit(
        &Publication::new(work(1), owner()),
        &WorkModeration::new(work(1)),
        1,
        1,
    );

    // While job 1 is still open, reevaluate refuses with JobAlreadyOpen ("not_needed").
    assert_eq!(
        mod_state.reevaluate(&pub_state, revision(1), evaluation_id(2)),
        Err(ReevaluateRefusal::JobAlreadyOpen.into())
    );

    // Timeout marks job 1 Failed without advancing DecisionVersion or changing Moderation.
    let timed_out = mod_state
        .apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket1.id,
                target: ticket1.target.clone(),
                policy: ticket1.policy,
                outcome: EvaluatorOutcome::Timeout,
            },
        )
        .unwrap();
    assert_eq!(timed_out.outcome, AppliedEvaluation::Retryable);
    assert_eq!(timed_out.publication, pub_state);
    assert_eq!(timed_out.moderation.facts().jobs[0].state, JobState::Failed);
    assert_eq!(
        timed_out
            .moderation
            .revision_decision(revision(1))
            .unwrap()
            .version,
        DecisionVersion::FIRST
    );

    // Late/duplicate callback on consumed job 1 is refused with Duplicate and mutates nothing.
    let before = timed_out.moderation.clone();
    assert_eq!(
        timed_out.moderation.apply_evaluator_result(
            &timed_out.publication,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket1.id,
                target: ticket1.target.clone(),
                policy: ticket1.policy,
                outcome: EvaluatorOutcome::Approve,
            },
        ),
        Err(EvaluationRefusal::Duplicate.into())
    );
    assert_eq!(timed_out.moderation, before);

    // Reusing the same EvaluationId for reevaluate is refused.
    assert_eq!(
        timed_out
            .moderation
            .reevaluate(&timed_out.publication, revision(1), ticket1.id),
        Err(ModerationError::DuplicateEvaluationId)
    );

    // Fresh EvaluationId opens job 2; ProviderError also consumes job 2 as Retryable.
    let (requeued_mod, ticket2) = timed_out
        .moderation
        .reevaluate(&timed_out.publication, revision(1), evaluation_id(2))
        .unwrap();
    let provider_failed = requeued_mod
        .apply_evaluator_result(
            &timed_out.publication,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket2.id,
                target: ticket2.target,
                policy: ticket2.policy,
                outcome: EvaluatorOutcome::ProviderError,
            },
        )
        .unwrap();
    assert_eq!(provider_failed.outcome, AppliedEvaluation::Retryable);
    assert_eq!(provider_failed.publication.readable_revision(), None);
}

#[test]
fn unknown_duplicate_cross_target_wrong_revision_wrong_digest_stale_policy_and_superseded_refuse_without_mutation()
 {
    let (pub_state, mod_state, ticket) = save_and_submit(
        &Publication::new(work(1), owner()),
        &WorkModeration::new(work(1)),
        1,
        1,
    );

    let valid = EvaluatorResult {
        evaluation_id: ticket.id,
        target: ticket.target.clone(),
        policy: ticket.policy,
        outcome: EvaluatorOutcome::Approve,
    };

    // 1. NotEvaluator
    assert_eq!(
        mod_state.apply_evaluator_result(&pub_state, None, &valid),
        Err(EvaluationRefusal::NotEvaluator.into())
    );

    // 2. Cross-target (CommentRevision and foreign WorkId)
    let comment_target = EvaluatorResult {
        target: EvaluationTarget::CommentRevision {
            comment: comment(1),
            revision: revision(1),
            digest: PayloadDigest::from_bytes(&[1u8; 32]).unwrap(),
        },
        ..valid.clone()
    };
    assert_eq!(
        mod_state.apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &comment_target
        ),
        Err(EvaluationRefusal::WrongTarget.into())
    );
    let foreign_work = EvaluatorResult {
        target: EvaluationTarget::WorkRevision {
            work: work(2),
            revision: revision(1),
            digest: snapshot(1).content.hash().clone(),
        },
        ..valid.clone()
    };
    assert_eq!(
        mod_state.apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &foreign_work
        ),
        Err(EvaluationRefusal::WrongTarget.into())
    );

    // 3. UnknownJob
    assert_eq!(
        mod_state.apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: evaluation_id(99),
                ..valid.clone()
            }
        ),
        Err(EvaluationRefusal::UnknownJob.into())
    );

    // 4. WrongRevision and wrong ContentHash digest
    assert_eq!(
        mod_state.apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                target: EvaluationTarget::WorkRevision {
                    work: work(1),
                    revision: revision(2),
                    digest: snapshot(2).content.hash().clone(),
                },
                ..valid.clone()
            }
        ),
        Err(EvaluationRefusal::WrongRevision.into())
    );
    assert_eq!(
        mod_state.apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                target: EvaluationTarget::WorkRevision {
                    work: work(1),
                    revision: revision(1),
                    digest: snapshot(2).content.hash().clone(),
                },
                ..valid.clone()
            }
        ),
        Err(EvaluationRefusal::WrongRevision.into())
    );

    // 5. StalePolicy
    let bumped = mod_state.bump_policy().unwrap();
    assert_eq!(bumped.facts().policy, PolicyVersion::new(2).unwrap());
    assert_eq!(
        bumped.apply_evaluator_result(&pub_state, Some(EvaluatorCapability::evaluator()), &valid),
        Err(EvaluationRefusal::StalePolicy.into())
    );

    // 6. Superseded: job 2 at policy 2 resolves r1 to NeedsAdmin; a third open job (or a comment
    // candidate that moved on) sees candidate_pending = false -> Superseded.
    let (requeued, ticket2) = bumped
        .reevaluate(&pub_state, revision(1), evaluation_id(2))
        .unwrap();
    let needs_admin = requeued
        .apply_evaluator_result(
            &pub_state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket2.id,
                target: ticket2.target.clone(),
                policy: ticket2.policy,
                outcome: EvaluatorOutcome::Uncertain,
            },
        )
        .unwrap();
    assert_eq!(
        validate_evaluation_result(
            Some(EvaluatorCapability::evaluator()),
            Some(EvaluationSubject::Work(work(1))),
            &[EvaluationJobFacts {
                id: evaluation_id(3),
                target: ticket2.target.clone(),
                policy: ticket2.policy,
                state: JobState::Open,
            }],
            ticket2.policy,
            &EvaluatorResult {
                evaluation_id: evaluation_id(3),
                target: ticket2.target,
                policy: ticket2.policy,
                outcome: EvaluatorOutcome::Approve,
            },
            |_| {
                Some(TargetRevisionFacts {
                    target: valid.target.clone(),
                    target_active: true,
                    overridden_by_admin: false,
                    candidate_pending: needs_admin.publication.facts().revisions[0].moderation
                        == Moderation::Pending,
                })
            },
        ),
        Err(EvaluationRefusal::Superseded)
    );
}

#[test]
fn admin_override_requires_narrow_authority_reason_and_expected_version_and_blocks_late_ai_callbacks()
 {
    let publication = Publication::new(work(1), owner());
    let moderation = WorkModeration::new(work(1));
    let saved = moderation
        .decide_author(
            &publication,
            owner(),
            AuthorCommand::SaveSnapshot(snapshot(1)),
            None,
        )
        .unwrap();

    // Admin cannot decide an unsubmitted private snapshot.
    let decision_v1 = AdminDecision {
        revision: revision(1),
        digest: snapshot(1).content.hash().clone(),
        expected: DecisionVersion::FIRST,
        verdict: AdminVerdict::Approve,
        reason: reason("Bản thảo đạt chuẩn biên tập"),
    };
    assert_eq!(
        saved.moderation.apply_admin_decision(
            &saved.publication,
            Some(AdminAuthority::scoped(admin(), work(1))),
            decision_v1.clone(),
        ),
        Err(ModerationError::NotSubmitted)
    );

    // Reason validation rejects empty, whitespace and control characters.
    for bad_reason in ["", "   ", "lí do\ncó newline", &"a".repeat(241)] {
        assert_eq!(
            DecisionReason::parse(bad_reason),
            Err(ModerationError::ReasonRequired)
        );
    }

    let (pub_state, mod_state, ticket) = save_and_submit(&publication, &moderation, 1, 1);

    // Missing authority, wrong work scope or author self-approving is refused with NotAdmin.
    for invalid_authority in [
        None,
        Some(AdminAuthority::scoped(admin(), work(2))),
        Some(AdminAuthority::scoped(owner(), work(1))),
    ] {
        assert_eq!(
            mod_state.apply_admin_decision(&pub_state, invalid_authority, decision_v1.clone()),
            Err(ModerationError::NotAdmin)
        );
    }

    // Wrong hash or stale expected DecisionVersion is refused without mutation.
    assert_eq!(
        mod_state.apply_admin_decision(
            &pub_state,
            Some(AdminAuthority::scoped(admin(), work(1))),
            AdminDecision {
                digest: snapshot(2).content.hash().clone(),
                ..decision_v1.clone()
            },
        ),
        Err(ModerationError::WrongHash)
    );
    assert_eq!(
        mod_state.apply_admin_decision(
            &pub_state,
            Some(AdminAuthority::scoped(admin(), work(1))),
            AdminDecision {
                expected: DecisionVersion::new(2).unwrap(),
                ..decision_v1.clone()
            },
        ),
        Err(ModerationError::StaleDecision)
    );

    // Valid admin approval publishes r1 (because author intent is public on r1) and advances DecisionVersion to 2.
    let approved = mod_state
        .apply_admin_decision(
            &pub_state,
            Some(AdminAuthority::scoped(admin(), work(1))),
            decision_v1.clone(),
        )
        .unwrap();
    assert_eq!(approved.outcome, AdminOutcome::Published);
    assert_eq!(approved.publication.readable_revision(), Some(&snapshot(1)));
    assert_eq!(
        approved
            .moderation
            .revision_decision(revision(1))
            .unwrap()
            .version,
        DecisionVersion::new(2).unwrap()
    );

    // Replaying the old v1 admin command now fails with StaleDecision (ABA fence).
    assert_eq!(
        approved.moderation.apply_admin_decision(
            &approved.publication,
            Some(AdminAuthority::scoped(admin(), work(1))),
            decision_v1,
        ),
        Err(ModerationError::StaleDecision)
    );

    // Late AI callback on open job 1 is refused with Overridden and keeps r1 public.
    let late_ai = approved.moderation.apply_evaluator_result(
        &approved.publication,
        Some(EvaluatorCapability::evaluator()),
        &EvaluatorResult {
            evaluation_id: ticket.id,
            target: ticket.target,
            policy: ticket.policy,
            outcome: EvaluatorOutcome::Reject(PolicyReasonCode::SafetyPolicy),
        },
    );
    assert_eq!(late_ai, Err(EvaluationRefusal::Overridden.into()));
    assert_eq!(approved.publication.readable_revision(), Some(&snapshot(1)));
}

#[test]
fn approval_with_withdrawn_intent_or_inactive_lifecycle_never_publishes_and_r1_stays_public_while_r2_fails()
 {
    let (pub_r1, mod_r1, ticket1) = save_and_submit(
        &Publication::new(work(1), owner()),
        &WorkModeration::new(work(1)),
        1,
        1,
    );
    let published_r1 = mod_r1
        .apply_evaluator_result(
            &pub_r1,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket1.id,
                target: ticket1.target,
                policy: ticket1.policy,
                outcome: EvaluatorOutcome::Approve,
            },
        )
        .unwrap();
    assert_eq!(published_r1.outcome, AppliedEvaluation::Published);
    assert_eq!(
        published_r1.publication.readable_revision(),
        Some(&snapshot(1))
    );

    // Submit r2 while r1 is public: r1 remains readable while r2 is pending and then rejected.
    let (pub_r2, mod_r2, ticket2) =
        save_and_submit(&published_r1.publication, &published_r1.moderation, 2, 2);
    assert_eq!(pub_r2.readable_revision(), Some(&snapshot(1)));
    let rejected_r2 = mod_r2
        .apply_evaluator_result(
            &pub_r2,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: ticket2.id,
                target: ticket2.target,
                policy: ticket2.policy,
                outcome: EvaluatorOutcome::Reject(PolicyReasonCode::Harassment),
            },
        )
        .unwrap();
    assert_eq!(
        rejected_r2.outcome,
        AppliedEvaluation::Rejected {
            reason: PolicyReasonCode::Harassment
        }
    );
    assert_eq!(
        rejected_r2.publication.readable_revision(),
        Some(&snapshot(1))
    );

    // Author withdraws to Private; admin overrides r2 to Approve -> ApprovedUnpublished, pointer stays None!
    let made_private = rejected_r2
        .moderation
        .decide_author(
            &rejected_r2.publication,
            owner(),
            AuthorCommand::SetPrivate {
                expected: rejected_r2.publication.facts().generation,
            },
            None,
        )
        .unwrap();
    assert_eq!(made_private.publication.readable_revision(), None);
    let admin_approved_private = made_private
        .moderation
        .apply_admin_decision(
            &made_private.publication,
            Some(AdminAuthority::scoped(admin(), work(1))),
            AdminDecision {
                revision: revision(2),
                digest: snapshot(2).content.hash().clone(),
                expected: DecisionVersion::new(2).unwrap(),
                verdict: AdminVerdict::Approve,
                reason: reason("Chấp thuận sau khi xem xét ngữ cảnh văn học"),
            },
        )
        .unwrap();
    assert_eq!(
        admin_approved_private.outcome,
        AdminOutcome::ApprovedUnpublished
    );
    assert_eq!(admin_approved_private.publication.readable_revision(), None);
    assert_eq!(
        admin_approved_private.effects.pointer,
        PointerEffect::Unchanged
    );
}

#[test]
fn restore_rejects_inconsistent_moderation_facts_and_version_exhaustion_refuses_cleanly() {
    assert_eq!(PolicyVersion::new(0), None);
    assert_eq!(DecisionVersion::new(0), None);
    assert_eq!(
        PolicyVersion::new(i64::MAX).unwrap().next(),
        Err(ModerationError::PolicyVersionExhausted)
    );
    assert_eq!(
        DecisionVersion::new(i64::MAX).unwrap().next(),
        Err(ModerationError::DecisionVersionExhausted)
    );

    let (pub_state, mod_state, _) = save_and_submit(
        &Publication::new(work(1), owner()),
        &WorkModeration::new(work(1)),
        1,
        1,
    );
    assert_eq!(
        WorkModeration::restore(&pub_state, mod_state.facts().clone()),
        Ok(mod_state.clone())
    );

    let base = mod_state.facts().clone();
    let mut wrong_work = base.clone();
    wrong_work.work = work(2);
    assert_eq!(
        WorkModeration::restore(&pub_state, wrong_work),
        Err(ModerationError::InvalidFacts)
    );

    let mut missing_rev = base.clone();
    missing_rev.revisions.clear();
    assert_eq!(
        WorkModeration::restore(&pub_state, missing_rev),
        Err(ModerationError::InvalidFacts)
    );

    let mut duplicate_job = base.clone();
    duplicate_job.jobs.push(base.jobs[0].clone());
    assert_eq!(
        WorkModeration::restore(&pub_state, duplicate_job),
        Err(ModerationError::InvalidFacts)
    );

    let mut future_job_policy = base;
    future_job_policy.jobs[0].policy = PolicyVersion::new(2).unwrap();
    assert_eq!(
        WorkModeration::restore(&pub_state, future_job_policy),
        Err(ModerationError::InvalidFacts)
    );

    // Public pointer remains valid across restoration after approval.
    let mut pub_approved_facts = pub_state.facts().clone();
    pub_approved_facts.revisions[0].moderation = Moderation::Approved;
    pub_approved_facts.pointer = Some(PublicPointer {
        revision: revision(1),
        intent_generation: generation(2),
    });
    let pub_approved = Publication::restore(pub_approved_facts).unwrap();
    let mut mod_approved_facts = mod_state.facts().clone();
    mod_approved_facts.revisions[0].version = DecisionVersion::new(2).unwrap();
    mod_approved_facts.revisions[0].provenance = Some(DecisionProvenance::AiApproved);
    mod_approved_facts.jobs[0].state = JobState::Applied;
    assert!(WorkModeration::restore(&pub_approved, mod_approved_facts.clone()).is_ok());

    mod_approved_facts.revisions[0].provenance = Some(DecisionProvenance::LegacyBootstrap);
    let restored = WorkModeration::restore(&pub_approved, mod_approved_facts).unwrap();
    assert_eq!(
        restored.facts().revisions[0].decision_by(),
        Some(crate::domain::moderation::DecisionBy::Legacy)
    );
}
