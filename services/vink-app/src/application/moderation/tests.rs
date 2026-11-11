use super::*;
use crate::domain::moderation::tests::{admin, evaluation_id, reason};
use crate::domain::moderation::{
    AdminOutcome, AdminVerdict, AppliedEvaluation, DecisionVersion, EvaluationRefusal,
    EvaluationTarget, EvaluatorOutcome, NeedsAdminCause, PolicyReasonCode,
};
use crate::domain::publication::tests::{owner, revision, snapshot, work};
use crate::domain::publication::{AuthorOutcome, Lifecycle};

#[test]
fn unauthenticated_and_suspended_principals_are_refused_before_any_author_or_admin_plan() {
    let publication = Publication::new(work(1), owner());
    let moderation = WorkModeration::new(work(1));
    for (actor, expected) in [
        (None, ModerationPlanError::Unauthenticated),
        (
            Some(BoundPrincipal {
                principal: owner(),
                standing: Standing::Suspended,
            }),
            ModerationPlanError::Suspended,
        ),
    ] {
        assert_eq!(
            plan_author_publication(
                actor,
                &publication,
                &moderation,
                AuthorCommand::SaveSnapshot(snapshot(1)),
                None,
            ),
            Err(expected)
        );
        assert_eq!(
            plan_work_admin_decision(
                actor,
                Some(work(1)),
                &publication,
                &moderation,
                AdminDecision {
                    revision: revision(1),
                    digest: snapshot(1).content.hash().clone(),
                    expected: DecisionVersion::FIRST,
                    verdict: AdminVerdict::Approve,
                    reason: reason("Kiểm tra quyền"),
                },
            ),
            Err(expected)
        );
    }
}

#[test]
fn scripted_evaluator_steps_all_nine_classes_and_never_falls_back_to_approve_when_empty() {
    assert_eq!(
        ScriptedEvaluator::LOCAL_TEST_ONLY_LABEL,
        "local_test_scripted_evaluator"
    );
    let author_actor = Some(BoundPrincipal {
        principal: owner(),
        standing: Standing::Active,
    });
    let admin_actor = Some(BoundPrincipal {
        principal: admin(),
        standing: Standing::Active,
    });

    let saved = plan_author_publication(
        author_actor,
        &Publication::new(work(1), owner()),
        &WorkModeration::new(work(1)),
        AuthorCommand::SaveSnapshot(snapshot(1)),
        None,
    )
    .unwrap();
    assert_eq!(saved.outcome, AuthorOutcome::Saved);

    let submitted = plan_author_publication(
        author_actor,
        &saved.publication,
        &saved.moderation,
        AuthorCommand::RequestPublic {
            revision: revision(1),
            expected: saved.publication.facts().generation,
        },
        Some(evaluation_id(1)),
    )
    .unwrap();
    let ticket1 = submitted.ticket.unwrap();

    // Unscripted call on an empty ScriptedEvaluator returns ProviderError (Retryable), never Approve.
    let mut empty_evaluator = ScriptedEvaluator::new([]);
    let (unscripted_res, unscripted_trans) = evaluate_and_apply_work_revision(
        &mut empty_evaluator,
        &submitted.publication,
        &submitted.moderation,
        &ticket1,
    )
    .unwrap();
    assert_eq!(unscripted_res.outcome, EvaluatorOutcome::ProviderError);
    assert_eq!(unscripted_trans.outcome, AppliedEvaluation::Retryable);
    assert_eq!(empty_evaluator.unscripted_calls(), 1);
    assert_eq!(unscripted_trans.publication.readable_revision(), None);

    // Exercise helper constructors for all 9 ScriptKind variants.
    let (mod_job2, ticket2) = plan_work_reevaluation(
        &unscripted_trans.publication,
        &unscripted_trans.moderation,
        revision(1),
        evaluation_id(2),
    )
    .unwrap();
    let mut scripted = ScriptedEvaluator::new([
        ScriptedStep::timeout(&ticket2),
        ScriptedStep::duplicate_result(&ticket2, EvaluatorOutcome::Approve),
    ]);
    let (_, timeout_trans) = evaluate_and_apply_work_revision(
        &mut scripted,
        &unscripted_trans.publication,
        &mod_job2,
        &ticket2,
    )
    .unwrap();
    assert_eq!(timeout_trans.outcome, AppliedEvaluation::Retryable);
    assert_eq!(
        evaluate_and_apply_work_revision(
            &mut scripted,
            &timeout_trans.publication,
            &timeout_trans.moderation,
            &ticket2,
        ),
        Err(EvaluationRefusal::Duplicate.into())
    );

    let (mod_job3, ticket3) = plan_work_reevaluation(
        &timeout_trans.publication,
        &timeout_trans.moderation,
        revision(1),
        evaluation_id(3),
    )
    .unwrap();
    scripted.push(ScriptedStep::wrong_revision(
        &ticket3,
        EvaluationTarget::WorkRevision {
            work: work(1),
            revision: revision(2),
            digest: snapshot(2).content.hash().clone(),
        },
        EvaluatorOutcome::Approve,
    ));
    assert_eq!(
        evaluate_and_apply_work_revision(
            &mut scripted,
            &timeout_trans.publication,
            &mod_job3,
            &ticket3,
        ),
        Err(EvaluationRefusal::WrongRevision.into())
    );

    scripted.push(ScriptedStep::provider_error(&ticket3));
    let (_, provider_err_trans) = evaluate_and_apply_work_revision(
        &mut scripted,
        &timeout_trans.publication,
        &mod_job3,
        &ticket3,
    )
    .unwrap();
    assert_eq!(provider_err_trans.outcome, AppliedEvaluation::Retryable);

    let (mod_job4, ticket4) = plan_work_reevaluation(
        &provider_err_trans.publication,
        &provider_err_trans.moderation,
        revision(1),
        evaluation_id(4),
    )
    .unwrap();
    scripted.push(ScriptedStep::approve(&ticket4));
    let (_, approved_r1) = evaluate_and_apply_work_revision(
        &mut scripted,
        &provider_err_trans.publication,
        &mod_job4,
        &ticket4,
    )
    .unwrap();
    assert_eq!(approved_r1.outcome, AppliedEvaluation::Published);

    // r2 -> Reject
    let saved_r2 = plan_author_publication(
        author_actor,
        &approved_r1.publication,
        &approved_r1.moderation,
        AuthorCommand::SaveSnapshot(snapshot(2)),
        None,
    )
    .unwrap();
    let sub_r2 = plan_author_publication(
        author_actor,
        &saved_r2.publication,
        &saved_r2.moderation,
        AuthorCommand::RequestPublic {
            revision: revision(2),
            expected: saved_r2.publication.facts().generation,
        },
        Some(evaluation_id(5)),
    )
    .unwrap();
    let ticket5 = sub_r2.ticket.unwrap();
    scripted.push(ScriptedStep::reject(
        &ticket5,
        PolicyReasonCode::SafetyPolicy,
    ));
    let (_, rejected_r2) = evaluate_and_apply_work_revision(
        &mut scripted,
        &sub_r2.publication,
        &sub_r2.moderation,
        &ticket5,
    )
    .unwrap();
    assert_eq!(
        rejected_r2.outcome,
        AppliedEvaluation::Rejected {
            reason: PolicyReasonCode::SafetyPolicy
        }
    );

    // r3 -> NeedsAdmin -> InvalidOutput (on r4) -> DelayedResult (after admin override)
    let saved_r3 = plan_author_publication(
        author_actor,
        &rejected_r2.publication,
        &rejected_r2.moderation,
        AuthorCommand::SaveSnapshot(snapshot(3)),
        None,
    )
    .unwrap();
    let sub_r3 = plan_author_publication(
        author_actor,
        &saved_r3.publication,
        &saved_r3.moderation,
        AuthorCommand::RequestPublic {
            revision: revision(3),
            expected: saved_r3.publication.facts().generation,
        },
        Some(evaluation_id(6)),
    )
    .unwrap();
    let ticket6 = sub_r3.ticket.unwrap();
    let bumped = sub_r3.moderation.bump_policy().unwrap();
    let (requeued_r3, ticket7) =
        plan_work_reevaluation(&sub_r3.publication, &bumped, revision(3), evaluation_id(7))
            .unwrap();
    scripted.push(ScriptedStep::needs_admin(&ticket7));
    let (_, needs_admin_r3) = evaluate_and_apply_work_revision(
        &mut scripted,
        &sub_r3.publication,
        &requeued_r3,
        &ticket7,
    )
    .unwrap();
    assert_eq!(
        needs_admin_r3.outcome,
        AppliedEvaluation::NeedsAdmin {
            cause: NeedsAdminCause::Uncertain
        }
    );

    let admin_override_r3 = plan_work_admin_decision(
        admin_actor,
        Some(work(1)),
        &needs_admin_r3.publication,
        &needs_admin_r3.moderation,
        AdminDecision {
            revision: revision(3),
            digest: snapshot(3).content.hash().clone(),
            expected: DecisionVersion::new(2).unwrap(),
            verdict: AdminVerdict::Approve,
            reason: reason("Duyệt thủ công r3"),
        },
    )
    .unwrap();
    assert_eq!(admin_override_r3.outcome, AdminOutcome::Published);

    scripted.push(ScriptedStep::delayed_result(
        &ticket6,
        EvaluatorOutcome::Reject(PolicyReasonCode::SafetyPolicy),
    ));
    assert_eq!(
        evaluate_and_apply_work_revision(
            &mut scripted,
            &admin_override_r3.publication,
            &admin_override_r3.moderation,
            &ticket6,
        ),
        Err(EvaluationRefusal::Overridden.into())
    );

    let saved_r4 = plan_author_publication(
        author_actor,
        &admin_override_r3.publication,
        &admin_override_r3.moderation,
        AuthorCommand::SaveSnapshot(snapshot(4)),
        None,
    )
    .unwrap();
    let sub_r4 = plan_author_publication(
        author_actor,
        &saved_r4.publication,
        &saved_r4.moderation,
        AuthorCommand::RequestPublic {
            revision: revision(4),
            expected: saved_r4.publication.facts().generation,
        },
        Some(evaluation_id(8)),
    )
    .unwrap();
    let ticket8 = sub_r4.ticket.unwrap();
    scripted.push(ScriptedStep::invalid_output(
        &ticket8,
        r#"{"verdict":"approve","extra":true}"#,
    ));
    let (_, invalid_r4) = evaluate_and_apply_work_revision(
        &mut scripted,
        &sub_r4.publication,
        &sub_r4.moderation,
        &ticket8,
    )
    .unwrap();
    assert_eq!(
        invalid_r4.outcome,
        AppliedEvaluation::NeedsAdmin {
            cause: NeedsAdminCause::InvalidOutput
        }
    );

    let executed: std::collections::BTreeSet<ScriptKind> =
        scripted.executed_kinds().iter().copied().collect();
    assert_eq!(executed, std::collections::BTreeSet::from(ScriptKind::ALL));
}

#[test]
fn plan_work_admin_removal_enforces_active_standing_and_admin_authority() {
    let author_actor = BoundPrincipal {
        principal: owner(),
        standing: Standing::Active,
    };
    let admin_actor = BoundPrincipal {
        principal: admin(),
        standing: Standing::Active,
    };
    let initial_pub = Publication::new(work(1), owner());
    let initial_mod = WorkModeration::new(work(1));

    let saved = plan_author_publication(
        Some(author_actor),
        &initial_pub,
        &initial_mod,
        AuthorCommand::SaveSnapshot(snapshot(1)),
        None,
    )
    .unwrap();

    // 1. Unauthenticated fails
    assert_eq!(
        plan_work_admin_removal(None, Some(work(1)), &saved.publication, &saved.moderation),
        Err(ModerationPlanError::Unauthenticated)
    );

    // 2. Suspended fails
    assert_eq!(
        plan_work_admin_removal(
            Some(BoundPrincipal {
                standing: Standing::Suspended,
                ..admin_actor
            }),
            Some(work(1)),
            &saved.publication,
            &saved.moderation
        ),
        Err(ModerationPlanError::Suspended)
    );

    // 3. No admin grant fails NotAdmin
    assert_eq!(
        plan_work_admin_removal(
            Some(admin_actor),
            None,
            &saved.publication,
            &saved.moderation
        ),
        Err(ModerationPlanError::Rejected(ModerationError::NotAdmin))
    );

    // 4. Successful removal
    let removed = plan_work_admin_removal(
        Some(admin_actor),
        Some(work(1)),
        &saved.publication,
        &saved.moderation,
    )
    .unwrap();
    assert_eq!(removed.publication.facts().lifecycle, Lifecycle::Removed);

    // 5. Calling again on already removed work fails TargetGone
    assert_eq!(
        plan_work_admin_removal(
            Some(admin_actor),
            Some(work(1)),
            &removed.publication,
            &removed.moderation
        ),
        Err(ModerationPlanError::Rejected(ModerationError::TargetGone))
    );
}
