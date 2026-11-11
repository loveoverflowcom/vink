//! Focused unit tests for the governance core (ADMIN-MODERATION-20261111): authority, cases,
//! appeals, automation mode and investigations. The M4 replay (`governance_replay`) covers the
//! model's interleavings; these pin each refusal and edge the model abstracts away.

use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::appeal::{
    AppealBook, AppealFacts, AppealRefusal, AppealState, AppealVerdict, CurrentDecision, DecidedBy,
    Filed, Notice,
};
use super::authority::{
    AuthorityBasis, AuthorityRefusal, DirectoryChange, DirectoryError, DirectoryFacts, GrantFacts,
    GrantOrigin, GrantScope, GrantSpec, Grantee, GroupFacts, ModerationDirectory, ReviewCapability,
    ReviewTarget,
};
use super::automation::{Automation, AutomationError, AutomationMode, AutomationVersion};
use super::case::{
    CaseEvent, CaseFacts, CaseOrigin, CaseRefusal, CaseState, CaseVerdict, CaseVersion,
    CloseReason, InternalNote, ReviewCase, ReviewSubject,
};
use super::investigation::{
    EvidenceSource, Finding, Investigation, InvestigationFacts, InvestigationRefusal,
    InvestigationState, ViolationBasis,
};
use super::*;
use crate::domain::ids::{AppealId, CaseId, GrantId, GroupId, InvestigationId};
use crate::domain::publication::tests::{owner, revision, save, snapshot, work};

const ADMIN: u128 = 4;
const OTHER_ADMIN: u128 = 6;
const MOD_A: u128 = 2;
const MOD_B: u128 = 3;
const WRITER: u128 = 5;

fn p(n: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(n))
}
fn gid(n: u128) -> GrantId {
    GrantId::from_uuid(Uuid::from_u128(0x6000 + n))
}
fn group() -> GroupId {
    GroupId::from_uuid(Uuid::from_u128(0x7000))
}
fn at(hours: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::hours(hours)
}
fn reason(text: &str) -> DecisionReason {
    DecisionReason::parse(text).unwrap()
}

fn grant(n: u128, grantee: Grantee, capability: ReviewCapability, scope: GrantScope) -> GrantFacts {
    GrantFacts {
        id: gid(n),
        grantee,
        capability,
        scope,
        expires_at: None,
        revoked: false,
        origin: GrantOrigin::Admin,
    }
}

fn facts(grants: Vec<GrantFacts>, members: Vec<PrincipalId>) -> DirectoryFacts {
    DirectoryFacts {
        admins: vec![p(ADMIN), p(OTHER_ADMIN)],
        suspended: vec![],
        grants,
        groups: vec![GroupFacts {
            id: group(),
            members,
        }],
    }
}

fn directory(grants: Vec<GrantFacts>) -> ModerationDirectory {
    ModerationDirectory::restore(facts(grants, vec![])).unwrap()
}

/// MOD_A holds global work review (grant 1) and MOD_B holds work review on work 1 (grant 2).
fn staffed() -> ModerationDirectory {
    directory(vec![
        grant(
            1,
            Grantee::Principal(p(MOD_A)),
            ReviewCapability::Works,
            GrantScope::Global,
        ),
        grant(
            2,
            Grantee::Principal(p(MOD_B)),
            ReviewCapability::Works,
            GrantScope::Work(work(1)),
        ),
    ])
}

fn work_target() -> ReviewTarget {
    ReviewTarget::work(work(1), owner())
}

// ---------------------------------------------------------------------------------------------
// authority

#[test]
fn directory_restore_refuses_inconsistent_rows() {
    let dup = grant(
        1,
        Grantee::Principal(p(MOD_A)),
        ReviewCapability::Works,
        GrantScope::Global,
    );
    assert_eq!(
        ModerationDirectory::restore(facts(vec![dup.clone(), dup], vec![])),
        Err(DirectoryError::InvalidFacts)
    );
    let unknown_group = grant(
        1,
        Grantee::Group(GroupId::from_uuid(Uuid::from_u128(0x7999))),
        ReviewCapability::Works,
        GrantScope::Global,
    );
    assert_eq!(
        ModerationDirectory::restore(facts(vec![unknown_group], vec![])),
        Err(DirectoryError::InvalidFacts)
    );
    assert_eq!(
        ModerationDirectory::restore(facts(vec![], vec![p(MOD_A), p(MOD_A)])),
        Err(DirectoryError::InvalidFacts)
    );
    let mut two_groups = facts(vec![], vec![]);
    two_groups.groups.push(two_groups.groups[0].clone());
    assert_eq!(
        ModerationDirectory::restore(two_groups),
        Err(DirectoryError::InvalidFacts)
    );
}

#[test]
fn an_inherited_comment_grant_is_only_global_comment_review_for_one_principal() {
    let inherited = |grantee, capability, scope| GrantFacts {
        origin: GrantOrigin::InheritedComment,
        ..grant(1, grantee, capability, scope)
    };
    for bad in [
        inherited(
            Grantee::Principal(p(MOD_A)),
            ReviewCapability::Works,
            GrantScope::Global,
        ),
        inherited(
            Grantee::Principal(p(MOD_A)),
            ReviewCapability::Comments,
            GrantScope::Work(work(1)),
        ),
        inherited(
            Grantee::Group(group()),
            ReviewCapability::Comments,
            GrantScope::Global,
        ),
    ] {
        assert_eq!(
            ModerationDirectory::restore(facts(vec![bad], vec![])),
            Err(DirectoryError::InvalidInheritance)
        );
    }

    let restored = directory(vec![inherited(
        Grantee::Principal(p(MOD_A)),
        ReviewCapability::Comments,
        GrantScope::Global,
    )]);
    let comment = ReviewTarget::comment(work(9), p(WRITER), owner());
    assert_eq!(
        restored.resolve(p(MOD_A), &comment, at(0)).unwrap().basis(),
        AuthorityBasis::Grant(gid(1))
    );
    assert_eq!(
        restored.resolve(p(MOD_A), &ReviewTarget::work(work(9), owner()), at(0)),
        Err(AuthorityRefusal::NotAuthorized),
        "an inherited comment grant never confers work review"
    );
}

#[test]
fn resolve_matches_capability_scope_liveness_and_membership_at_the_moment_of_the_check() {
    let mut expiring = grant(
        3,
        Grantee::Principal(p(WRITER)),
        ReviewCapability::Works,
        GrantScope::Global,
    );
    expiring.expires_at = Some(at(2));
    let mut revoked = grant(
        4,
        Grantee::Principal(p(MOD_B)),
        ReviewCapability::Comments,
        GrantScope::Global,
    );
    revoked.revoked = true;
    let via_group = grant(
        5,
        Grantee::Group(group()),
        ReviewCapability::Comments,
        GrantScope::Global,
    );
    let mut rows = staffed().facts().grants.clone();
    rows.extend([expiring, revoked, via_group]);
    let dir = ModerationDirectory::restore(facts(rows, vec![p(MOD_A)])).unwrap();
    let other_work = ReviewTarget::work(work(2), p(9));
    let comment = ReviewTarget::comment(work(1), p(WRITER), owner());

    let admin = dir.resolve(p(ADMIN), &work_target(), at(0)).unwrap();
    assert!(admin.is_admin());
    assert_eq!(admin.checked_at(), at(0));
    assert_eq!(
        dir.resolve(p(MOD_A), &other_work, at(0)).unwrap().basis(),
        AuthorityBasis::Grant(gid(1))
    );
    assert_eq!(
        dir.resolve(p(MOD_B), &work_target(), at(0))
            .unwrap()
            .basis(),
        AuthorityBasis::Grant(gid(2))
    );
    assert_eq!(
        dir.resolve(p(MOD_B), &other_work, at(0)),
        Err(AuthorityRefusal::NotAuthorized),
        "a work-scoped grant covers only its work"
    );
    assert_eq!(
        dir.resolve(p(MOD_B), &comment, at(0)),
        Err(AuthorityRefusal::NotAuthorized),
        "a revoked grant confers nothing; work review never confers comment review"
    );
    assert_eq!(
        dir.resolve(p(MOD_A), &comment, at(0)).unwrap().basis(),
        AuthorityBasis::Grant(gid(5)),
        "a group grant confers through current membership"
    );
    assert!(dir.resolve(p(WRITER), &other_work, at(1)).is_ok());
    assert_eq!(
        dir.resolve(p(WRITER), &other_work, at(2)),
        Err(AuthorityRefusal::NotAuthorized),
        "a grant expires at its deadline"
    );

    let seat = dir.require_admin(p(ADMIN), at(0)).unwrap();
    let (left, audit) = dir.set_membership(seat, group(), p(MOD_A), false).unwrap();
    assert_eq!(
        audit.change,
        DirectoryChange::Left {
            group: group(),
            member: p(MOD_A)
        }
    );
    assert_eq!(
        left.resolve(p(MOD_A), &comment, at(0)),
        Err(AuthorityRefusal::NotAuthorized)
    );
}

#[test]
fn suspension_and_recusal_override_every_basis() {
    let mut rows = facts(staffed().facts().grants.clone(), vec![]);
    rows.suspended = vec![p(MOD_A), p(OTHER_ADMIN)];
    rows.grants.push(grant(
        3,
        Grantee::Principal(p(MOD_B)),
        ReviewCapability::Comments,
        GrantScope::Global,
    ));
    let dir = ModerationDirectory::restore(rows).unwrap();
    assert_eq!(
        dir.resolve(p(MOD_A), &work_target(), at(0)),
        Err(AuthorityRefusal::Suspended)
    );
    assert_eq!(
        dir.require_admin(p(OTHER_ADMIN), at(0)),
        Err(AuthorityRefusal::Suspended)
    );
    assert_eq!(
        dir.require_admin(p(MOD_B), at(0)),
        Err(AuthorityRefusal::NotAdmin)
    );
    // An admin recuses from their own work; a comment moderator recuses from comments on theirs.
    assert_eq!(
        dir.resolve(p(ADMIN), &ReviewTarget::work(work(2), p(ADMIN)), at(0)),
        Err(AuthorityRefusal::Recused)
    );
    assert_eq!(
        dir.resolve(
            p(MOD_B),
            &ReviewTarget::comment(work(2), p(WRITER), p(MOD_B)),
            at(0)
        ),
        Err(AuthorityRefusal::Recused)
    );
    assert_eq!(
        dir.resolve(
            p(MOD_B),
            &ReviewTarget::comment(work(2), p(MOD_B), owner()),
            at(0)
        ),
        Err(AuthorityRefusal::Recused)
    );
}

#[test]
fn resolve_preferring_keeps_a_live_preferred_basis_and_falls_back_otherwise() {
    let mut rows = staffed().facts().grants.clone();
    rows.push(grant(
        3,
        Grantee::Principal(p(MOD_B)),
        ReviewCapability::Works,
        GrantScope::Global,
    ));
    let dir = directory(rows);
    let preferred = Some(AuthorityBasis::Grant(gid(3)));
    assert_eq!(
        dir.resolve_preferring(p(MOD_B), &work_target(), at(0), preferred)
            .unwrap()
            .basis(),
        AuthorityBasis::Grant(gid(3))
    );
    assert_eq!(
        dir.resolve_preferring(
            p(MOD_B),
            &work_target(),
            at(0),
            Some(AuthorityBasis::Grant(gid(1)))
        )
        .unwrap()
        .basis(),
        AuthorityBasis::Grant(gid(2)),
        "a basis the actor does not hold falls back to stored order"
    );
}

#[test]
fn admin_directory_changes_recheck_the_seat_and_keep_rows_for_audit() {
    let dir = staffed();
    let seat = dir.require_admin(p(ADMIN), at(1)).unwrap();
    let spec = GrantSpec {
        grantee: Grantee::Group(group()),
        capability: ReviewCapability::Comments,
        scope: GrantScope::Global,
        expires_at: None,
    };

    let (granted, audit) = dir.grant(seat, gid(9), spec).unwrap();
    assert_eq!(audit.admin, p(ADMIN));
    assert_eq!(audit.at, at(1));
    assert_eq!(audit.change, DirectoryChange::Granted(gid(9)));
    assert_eq!(
        granted.facts().grants.last().unwrap().origin,
        GrantOrigin::Admin
    );
    assert_eq!(
        granted.grant(seat, gid(9), spec),
        Err(DirectoryError::DuplicateGrant)
    );
    assert_eq!(
        dir.grant(
            seat,
            gid(10),
            GrantSpec {
                grantee: Grantee::Group(GroupId::from_uuid(Uuid::from_u128(0x7999))),
                ..spec
            }
        ),
        Err(DirectoryError::UnknownGroup)
    );

    let (revoked, audit) = granted.revoke(seat, gid(9)).unwrap();
    assert_eq!(audit.change, DirectoryChange::Revoked(gid(9)));
    assert!(
        revoked
            .facts()
            .grants
            .iter()
            .any(|g| g.id == gid(9) && g.revoked)
    );
    assert_eq!(revoked.revoke(seat, gid(9)), Err(DirectoryError::Unchanged));
    assert_eq!(dir.revoke(seat, gid(42)), Err(DirectoryError::UnknownGrant));

    let (joined, _) = dir.set_membership(seat, group(), p(MOD_A), true).unwrap();
    assert_eq!(
        joined.set_membership(seat, group(), p(MOD_A), true),
        Err(DirectoryError::Unchanged)
    );
    assert_eq!(
        dir.set_membership(seat, group(), p(MOD_A), false),
        Err(DirectoryError::Unchanged)
    );
    assert_eq!(
        dir.set_membership(
            seat,
            GroupId::from_uuid(Uuid::from_u128(0x7999)),
            p(MOD_A),
            true
        ),
        Err(DirectoryError::UnknownGroup)
    );

    // A seat checked earlier is not standing authority: the directory re-checks it.
    let mut demoted = facts(dir.facts().grants.clone(), vec![]);
    demoted.admins.retain(|a| *a != p(ADMIN));
    let demoted = ModerationDirectory::restore(demoted).unwrap();
    assert_eq!(
        demoted.grant(seat, gid(11), spec),
        Err(DirectoryError::Refused(AuthorityRefusal::NotAdmin))
    );
}

// ---------------------------------------------------------------------------------------------
// cases

fn subject(decision: i64) -> ReviewSubject {
    ReviewSubject::WorkRevision {
        work: work(1),
        revision: revision(1),
        digest: snapshot(1).content.hash().clone(),
        decision: DecisionVersion::new(decision).unwrap(),
    }
}

fn queued() -> ReviewCase {
    ReviewCase::open(
        CaseId::from_uuid(Uuid::from_u128(0xCA5E)),
        subject(1),
        work_target(),
        CaseOrigin::EvaluatorUncertain,
    )
}

fn claimed_by(dir: &ModerationDirectory, actor: u128) -> ReviewCase {
    queued()
        .claim(dir, p(actor), CaseVersion::FIRST, at(0))
        .unwrap()
        .next
}

#[test]
fn internal_notes_are_bounded_plain_text() {
    assert_eq!(InternalNote::parse("   "), Err(CaseRefusal::NoteRequired));
    assert_eq!(
        InternalNote::parse(&"ă".repeat(InternalNote::MAX_CHARS + 1)),
        Err(CaseRefusal::NoteRequired)
    );
    assert_eq!(
        InternalNote::parse("xem\u{7}lại"),
        Err(CaseRefusal::NoteRequired)
    );
    let note = InternalNote::parse("  Đoạn hai\ncần đối chiếu  ").unwrap();
    assert_eq!(note.as_str(), "Đoạn hai\ncần đối chiếu");
    assert!(InternalNote::parse(&"ă".repeat(InternalNote::MAX_CHARS)).is_ok());
}

#[test]
fn case_restore_refuses_mismatched_subject_target_and_version() {
    let base = queued().facts().clone();
    assert!(ReviewCase::restore(base.clone()).is_ok());
    assert_eq!(
        ReviewCase::restore(CaseFacts {
            target: ReviewTarget::work(work(2), owner()),
            ..base.clone()
        }),
        Err(CaseRefusal::InvalidFacts)
    );
    assert_eq!(
        ReviewCase::restore(CaseFacts {
            target: ReviewTarget::comment(work(1), p(WRITER), owner()),
            ..base.clone()
        }),
        Err(CaseRefusal::InvalidFacts)
    );
    assert_eq!(
        ReviewCase::restore(CaseFacts {
            state: CaseState::Escalated,
            ..base
        }),
        Err(CaseRefusal::InvalidFacts),
        "version 1 is the recorded state"
    );
}

#[test]
fn of_two_racing_claims_exactly_one_applies() {
    let dir = staffed();
    let case = queued();
    let first = case
        .claim(&dir, p(MOD_A), CaseVersion::FIRST, at(0))
        .unwrap();
    assert_eq!(first.event, CaseEvent::Claimed);
    assert_eq!(first.next.version().get(), 2);
    assert_eq!(
        first.next.state(),
        CaseState::Claimed {
            by: p(MOD_A),
            basis: AuthorityBasis::Grant(gid(1))
        }
    );
    assert_eq!(
        first.next.claim(&dir, p(MOD_B), CaseVersion::FIRST, at(0)),
        Err(CaseRefusal::StaleCase),
        "the loser carries the old version"
    );
    assert_eq!(
        first
            .next
            .claim(&dir, p(MOD_B), first.next.version(), at(0)),
        Err(CaseRefusal::AlreadyClaimed)
    );
    assert_eq!(
        case.claim(&dir, p(WRITER), CaseVersion::FIRST, at(0)),
        Err(CaseRefusal::Authority(AuthorityRefusal::NotAuthorized))
    );
}

#[test]
fn release_note_and_escalate_belong_to_the_claimant() {
    let dir = staffed();
    let case = claimed_by(&dir, MOD_A);
    let v = case.version();
    let note = InternalNote::parse("Đã đọc khổ ba").unwrap();

    assert_eq!(case.release(p(MOD_B), v), Err(CaseRefusal::NotClaimant));
    assert_eq!(
        case.note(&dir, p(MOD_B), v, note.clone(), at(0)),
        Err(CaseRefusal::NotClaimant)
    );
    assert_eq!(case.escalate(p(MOD_B), v), Err(CaseRefusal::NotClaimant));

    let noted = case.note(&dir, p(MOD_A), v, note.clone(), at(0)).unwrap();
    assert_eq!(noted.event, CaseEvent::Noted);
    assert_eq!(noted.next.state(), case.state(), "a note keeps the holder");
    let by_admin = noted
        .next
        .note(&dir, p(ADMIN), noted.next.version(), note, at(0))
        .unwrap();
    assert_eq!(by_admin.next.facts().notes.len(), 2);
    assert_eq!(by_admin.next.facts().notes[1].author, p(ADMIN));

    let released = case.release(p(MOD_A), v).unwrap();
    assert_eq!(released.event, CaseEvent::Released);
    assert_eq!(released.next.state(), CaseState::Queued);
    assert_eq!(released.next.version().get(), v.get() + 1);

    let escalated = case.escalate(p(MOD_A), v).unwrap().next;
    assert_eq!(escalated.state(), CaseState::Escalated);
    assert_eq!(
        escalated.claim(&dir, p(MOD_B), escalated.version(), at(0)),
        Err(CaseRefusal::Escalated)
    );
}

#[test]
fn only_an_admin_with_a_reason_takes_a_case_over() {
    let dir = staffed();
    let case = claimed_by(&dir, MOD_A);
    let v = case.version();
    let why = reason("Ca quá hạn xử lý");

    assert_eq!(
        case.take_over(&dir, p(ADMIN), v, None, at(0)),
        Err(CaseRefusal::ReasonRequired)
    );
    assert_eq!(
        case.take_over(&dir, p(MOD_B), v, Some(&why), at(0)),
        Err(CaseRefusal::Authority(AuthorityRefusal::NotAdmin))
    );
    let taken = case
        .take_over(&dir, p(ADMIN), v, Some(&why), at(0))
        .unwrap();
    assert_eq!(taken.event, CaseEvent::TakenOver);
    assert_eq!(
        taken.next.state(),
        CaseState::Claimed {
            by: p(ADMIN),
            basis: AuthorityBasis::Admin
        }
    );
    assert_eq!(
        taken
            .next
            .take_over(&dir, p(ADMIN), taken.next.version(), Some(&why), at(0)),
        Err(CaseRefusal::AlreadyClaimed)
    );
    assert_eq!(
        case.take_over(&dir, p(ADMIN), CaseVersion::FIRST, Some(&why), at(0)),
        Err(CaseRefusal::StaleCase)
    );
}

#[test]
fn revalidation_requeues_only_a_claimant_without_any_live_basis() {
    let mut rows = staffed().facts().grants.clone();
    rows.push(grant(
        3,
        Grantee::Principal(p(MOD_B)),
        ReviewCapability::Works,
        GrantScope::Global,
    ));
    let dir = directory(rows);
    let case = claimed_by(&dir, MOD_B);
    assert_eq!(
        case.state(),
        CaseState::Claimed {
            by: p(MOD_B),
            basis: AuthorityBasis::Grant(gid(2))
        }
    );
    let seat = dir.require_admin(p(ADMIN), at(0)).unwrap();

    let same = case.revalidate(&dir, at(0)).unwrap();
    assert_eq!(same.event, CaseEvent::AuthorityKept);
    assert_eq!(same.next, case, "nothing changed, nothing advanced");

    let (without_scoped, _) = dir.revoke(seat, gid(2)).unwrap();
    let kept = case.revalidate(&without_scoped, at(0)).unwrap();
    assert_eq!(kept.event, CaseEvent::AuthorityKept);
    assert_eq!(
        kept.next.state(),
        CaseState::Claimed {
            by: p(MOD_B),
            basis: AuthorityBasis::Grant(gid(3))
        }
    );
    assert_eq!(kept.next.version().get(), case.version().get() + 1);

    let (without_any, _) = without_scoped.revoke(seat, gid(3)).unwrap();
    let lost = case.revalidate(&without_any, at(0)).unwrap();
    assert_eq!(lost.event, CaseEvent::AuthorityLost);
    assert_eq!(lost.next.state(), CaseState::Queued);
    assert_eq!(
        case.conclude(
            &without_any,
            p(MOD_B),
            case.version(),
            &subject(1),
            CaseVerdict::Approve,
            None,
            false,
            at(0)
        )
        .map(|_| ()),
        Err(CaseRefusal::Authority(AuthorityRefusal::NotAuthorized)),
        "a claimant who lost every basis cannot conclude"
    );
    assert_eq!(
        queued().revalidate(&dir, at(0)),
        Err(CaseRefusal::NotClaimant)
    );
}

#[test]
fn a_conclusion_fences_the_subject_decision_reason_and_open_appeal() {
    let dir = staffed();
    let case = claimed_by(&dir, MOD_A);
    let v = case.version();
    let conclude = |actor: u128, current: &ReviewSubject, verdict, why, appeal_open| {
        case.conclude(&dir, p(actor), v, current, verdict, why, appeal_open, at(0))
    };

    assert_eq!(
        conclude(MOD_B, &subject(1), CaseVerdict::Approve, None, false).map(|_| ()),
        Err(CaseRefusal::NotClaimant)
    );
    let other = ReviewSubject::WorkRevision {
        work: work(1),
        revision: revision(2),
        digest: snapshot(2).content.hash().clone(),
        decision: DecisionVersion::FIRST,
    };
    assert_eq!(
        conclude(MOD_A, &other, CaseVerdict::Approve, None, false).map(|_| ()),
        Err(CaseRefusal::WrongSubject)
    );
    assert_eq!(
        conclude(MOD_A, &subject(2), CaseVerdict::Approve, None, false).map(|_| ()),
        Err(CaseRefusal::StaleDecision)
    );
    assert_eq!(
        conclude(MOD_A, &subject(1), CaseVerdict::Approve, None, true).map(|_| ()),
        Err(CaseRefusal::AppealOpen)
    );
    assert_eq!(
        conclude(MOD_A, &subject(1), CaseVerdict::Reject, None, false).map(|_| ()),
        Err(CaseRefusal::ReasonRequired)
    );

    let why = reason("Vi phạm quy tắc 3");
    let (transition, conclusion) = conclude(
        MOD_A,
        &subject(1),
        CaseVerdict::Reject,
        Some(why.clone()),
        false,
    )
    .unwrap();
    assert_eq!(transition.event, CaseEvent::Concluded);
    assert_eq!(conclusion.subject, subject(1));
    assert_eq!(conclusion.authority.basis(), AuthorityBasis::Grant(gid(1)));
    assert_eq!(conclusion.reason, Some(why));

    // While an appeal is open only an admin concludes.
    let admin_case = queued()
        .take_over(
            &dir,
            p(ADMIN),
            CaseVersion::FIRST,
            Some(&reason("Phản đối")),
            at(0),
        )
        .unwrap()
        .next;
    assert!(
        admin_case
            .conclude(
                &dir,
                p(ADMIN),
                admin_case.version(),
                &subject(1),
                CaseVerdict::Approve,
                None,
                true,
                at(0)
            )
            .is_ok()
    );
}

#[test]
fn closing_ends_an_open_case_once_and_concluded_cases_stay_as_they_are() {
    let dir = staffed();
    let case = claimed_by(&dir, MOD_A);
    let closed = case.close(CloseReason::Withdrawn).unwrap();
    assert_eq!(closed.event, CaseEvent::Closed(CloseReason::Withdrawn));
    assert_eq!(
        closed.next.state(),
        CaseState::Closed(CloseReason::Withdrawn)
    );
    assert_eq!(
        closed.next.close(CloseReason::TargetGone),
        Err(CaseRefusal::CaseClosed)
    );
    assert_eq!(
        closed.next.release(p(MOD_A), closed.next.version()),
        Err(CaseRefusal::CaseClosed)
    );

    let (concluded, _) = case
        .conclude(
            &dir,
            p(MOD_A),
            case.version(),
            &subject(1),
            CaseVerdict::Approve,
            None,
            false,
            at(0),
        )
        .unwrap();
    assert_eq!(
        concluded.next.close(CloseReason::Withdrawn),
        Err(CaseRefusal::CaseClosed)
    );
}

// ---------------------------------------------------------------------------------------------
// appeals

fn appeal_id(n: u128) -> AppealId {
    AppealId::from_uuid(Uuid::from_u128(0xA000 + n))
}

fn decision(fence: i64, decided_by: Option<DecidedBy>, adverse: bool) -> CurrentDecision {
    CurrentDecision {
        subject: subject(fence),
        owner: owner(),
        decided_by,
        adverse,
        withdrawn: false,
    }
}

fn rejected_by_moderator() -> CurrentDecision {
    decision(2, Some(DecidedBy::Moderator(p(MOD_A))), true)
}

#[test]
fn the_owner_files_one_appeal_per_adverse_non_admin_decision_version() {
    let book = AppealBook::default();
    let current = rejected_by_moderator();
    let seen = subject(2);

    assert_eq!(
        book.file(p(WRITER), &current, &seen, appeal_id(1)),
        Err(AppealRefusal::NotFound)
    );
    assert_eq!(
        book.file(owner(), &current, &subject(1), appeal_id(1)),
        Err(AppealRefusal::StaleDecision)
    );
    assert_eq!(
        book.file(
            owner(),
            &decision(2, Some(DecidedBy::Admin(p(ADMIN))), true),
            &seen,
            appeal_id(1)
        ),
        Err(AppealRefusal::FinalDecision)
    );
    assert_eq!(
        book.file(
            owner(),
            &decision(2, Some(DecidedBy::Evaluator), false),
            &seen,
            appeal_id(1)
        ),
        Err(AppealRefusal::NothingToAppeal)
    );
    assert_eq!(
        book.file(owner(), &decision(2, None, true), &seen, appeal_id(1)),
        Err(AppealRefusal::NothingToAppeal)
    );
    assert_eq!(
        book.file(
            owner(),
            &CurrentDecision {
                withdrawn: true,
                ..current.clone()
            },
            &seen,
            appeal_id(1)
        ),
        Err(AppealRefusal::Withdrawn)
    );

    let (filed, outcome) = book.file(owner(), &current, &seen, appeal_id(1)).unwrap();
    assert_eq!(outcome, Filed::Recorded(appeal_id(1)));
    let recorded = filed.get(appeal_id(1)).unwrap();
    assert_eq!(recorded.state, AppealState::Open);
    assert_eq!(recorded.notice, Notice::NotSent, "recorded, never sent");
    assert_eq!(recorded.decided_by, DecidedBy::Moderator(p(MOD_A)));
    assert!(filed.open_on(&subject(2)));
    assert!(!filed.open_on(&subject(3)));

    let (again, outcome) = filed.file(owner(), &current, &seen, appeal_id(2)).unwrap();
    assert_eq!(
        outcome,
        Filed::Existing(appeal_id(1)),
        "any key returns the one appeal"
    );
    assert_eq!(again, filed);
}

#[test]
fn only_an_admin_concludes_an_open_appeal_with_a_reason_on_the_version_it_names() {
    let dir = staffed();
    let current = rejected_by_moderator();
    let (book, _) = AppealBook::default()
        .file(owner(), &current, &subject(2), appeal_id(1))
        .unwrap();
    let why = || Some(reason("Đọc lại đủ ngữ cảnh"));
    let conclude = |book: &AppealBook, admin: u128, current: &CurrentDecision, why| {
        book.conclude(
            &dir,
            p(admin),
            appeal_id(1),
            current,
            AppealVerdict::Reverse,
            why,
            at(0),
        )
    };

    assert_eq!(
        conclude(&book, MOD_A, &current, why()).map(|_| ()),
        Err(AppealRefusal::Authority(AuthorityRefusal::NotAdmin))
    );
    assert_eq!(
        book.conclude(
            &dir,
            p(ADMIN),
            appeal_id(9),
            &current,
            AppealVerdict::Uphold,
            why(),
            at(0)
        )
        .map(|_| ()),
        Err(AppealRefusal::UnknownAppeal)
    );
    assert_eq!(
        conclude(&book, ADMIN, &current, None).map(|_| ()),
        Err(AppealRefusal::ReasonRequired)
    );
    assert_eq!(
        conclude(&book, ADMIN, &decision(3, None, false), why()).map(|_| ()),
        Err(AppealRefusal::StaleDecision)
    );
    assert_eq!(
        conclude(
            &book,
            ADMIN,
            &CurrentDecision {
                withdrawn: true,
                ..current.clone()
            },
            why()
        )
        .map(|_| ()),
        Err(AppealRefusal::Withdrawn)
    );
    assert_eq!(
        conclude(
            &book,
            ADMIN,
            &CurrentDecision {
                owner: p(ADMIN),
                ..current.clone()
            },
            why()
        )
        .map(|_| ()),
        Err(AppealRefusal::Recused)
    );

    let (concluded, conclusion) = conclude(&book, ADMIN, &current, why()).unwrap();
    assert_eq!(conclusion.admin.actor(), p(ADMIN));
    assert_eq!(conclusion.subject, subject(2));
    assert_eq!(conclusion.verdict, AppealVerdict::Reverse);
    assert_eq!(
        concluded.get(appeal_id(1)).unwrap().state,
        AppealState::Reversed
    );
    assert!(!concluded.open_on(&subject(2)));
    assert_eq!(
        conclude(&concluded, OTHER_ADMIN, &current, why()).map(|_| ()),
        Err(AppealRefusal::AlreadyResolved)
    );
}

#[test]
fn appeal_restore_and_close_keep_one_row_per_decision_version() {
    let row = |n: u128, fence: i64, decided_by| AppealFacts {
        id: appeal_id(n),
        subject: subject(fence),
        appellant: owner(),
        decided_by,
        state: AppealState::Open,
        notice: Notice::NotSent,
    };
    assert_eq!(
        AppealBook::restore(vec![
            row(1, 2, DecidedBy::Evaluator),
            row(2, 2, DecidedBy::Evaluator)
        ]),
        Err(AppealRefusal::InvalidFacts)
    );
    assert_eq!(
        AppealBook::restore(vec![row(1, 2, DecidedBy::Admin(p(ADMIN)))]),
        Err(AppealRefusal::InvalidFacts)
    );
    let mut upheld = row(1, 2, DecidedBy::Evaluator);
    upheld.state = AppealState::Upheld;
    let book = AppealBook::restore(vec![upheld, row(2, 3, DecidedBy::Evaluator)]).unwrap();
    let closed = book.close_open(CloseReason::Withdrawn);
    assert_eq!(
        closed.get(appeal_id(1)).unwrap().state,
        AppealState::Upheld,
        "a concluded appeal is never reopened or closed"
    );
    assert_eq!(
        closed.get(appeal_id(2)).unwrap().state,
        AppealState::Closed(CloseReason::Withdrawn)
    );
    assert_eq!(closed.appeals().len(), 2);
}

// ---------------------------------------------------------------------------------------------
// automation

#[test]
fn automation_mode_changes_are_admin_only_versioned_and_reasoned() {
    let dir = staffed();
    let start = Automation::INITIAL;
    let why = || Some(reason("Bộ đánh giá trả kết quả lạ"));
    assert_eq!(start.mode(), AutomationMode::Enabled);

    assert_eq!(
        start.set(
            &dir,
            p(MOD_A),
            start.version(),
            AutomationMode::Paused,
            why(),
            at(0)
        ),
        Err(AutomationError::Authority(AuthorityRefusal::NotAdmin))
    );
    assert_eq!(
        start.set(
            &dir,
            p(ADMIN),
            AutomationVersion::new(2).unwrap(),
            AutomationMode::Paused,
            why(),
            at(0)
        ),
        Err(AutomationError::Stale)
    );
    assert_eq!(
        start.set(
            &dir,
            p(ADMIN),
            start.version(),
            AutomationMode::Enabled,
            why(),
            at(0)
        ),
        Err(AutomationError::Unchanged)
    );
    assert_eq!(
        start.set(
            &dir,
            p(ADMIN),
            start.version(),
            AutomationMode::Paused,
            None,
            at(0)
        ),
        Err(AutomationError::ReasonRequired)
    );

    let (paused, audit) = start
        .set(
            &dir,
            p(ADMIN),
            start.version(),
            AutomationMode::Paused,
            why(),
            at(1),
        )
        .unwrap();
    assert_eq!(paused.mode(), AutomationMode::Paused);
    assert_eq!(paused.version().get(), 2);
    assert_eq!(audit.admin, p(ADMIN));
    assert_eq!(audit.version, paused.version());
    assert_eq!(audit.at, at(1));
    let (resumed, _) = paused
        .set(
            &dir,
            p(OTHER_ADMIN),
            paused.version(),
            AutomationMode::Enabled,
            why(),
            at(2),
        )
        .unwrap();
    assert_eq!(
        resumed,
        Automation::restore(AutomationMode::Enabled, AutomationVersion::new(3).unwrap())
    );
    assert_eq!(
        Automation::restore(
            AutomationMode::Paused,
            AutomationVersion::new(i64::MAX).unwrap()
        )
        .set(
            &dir,
            p(ADMIN),
            AutomationVersion::new(i64::MAX).unwrap(),
            AutomationMode::Enabled,
            why(),
            at(3)
        ),
        Err(AutomationError::VersionExhausted)
    );
}

// ---------------------------------------------------------------------------------------------
// investigations

/// Work 1 by `owner()`: revision 1 published through production, then made private.
fn once_public_now_private() -> (Publication, WorkModeration) {
    let publication = save(&Publication::new(work(1), owner()), 1);
    let submitted = WorkModeration::new(work(1))
        .decide_author(
            &publication,
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(1),
                expected: publication.facts().generation,
            },
            Some(EvaluationId::from_uuid(Uuid::from_u128(0xF1))),
        )
        .unwrap();
    let ticket = submitted.ticket.clone().unwrap();
    let approved = submitted
        .moderation
        .apply_evaluator_result(
            &submitted.publication,
            Some(EvaluatorCapability::evaluator()),
            &ScriptedStep::approve(&ticket).result,
        )
        .unwrap();
    assert_eq!(approved.outcome, AppliedEvaluation::Published);
    let private = approved
        .moderation
        .decide_author_with(
            &approved.publication,
            owner(),
            AuthorCommand::SetPrivate {
                expected: approved.publication.facts().generation,
            },
            None,
            AutomationMode::Enabled,
        )
        .unwrap();
    assert_eq!(private.outcome, AuthorOutcome::MadePrivate);
    (private.publication, private.moderation)
}

fn open_file(publication: &Publication, expires_at: Option<OffsetDateTime>) -> Investigation {
    Investigation::open(
        &staffed(),
        p(ADMIN),
        publication,
        InvestigationId::from_uuid(Uuid::from_u128(0x1A)),
        revision(1),
        ViolationBasis::parse("Đạo văn từ tác phẩm đã xuất bản").unwrap(),
        EvidenceSource::parse("Báo cáo của độc giả").unwrap(),
        expires_at,
        at(0),
    )
    .unwrap()
}

#[test]
fn investigation_basis_and_evidence_are_bounded_plain_text() {
    assert_eq!(
        ViolationBasis::parse("  "),
        Err(InvestigationRefusal::BasisRequired)
    );
    assert_eq!(
        ViolationBasis::parse(&"x".repeat(501)),
        Err(InvestigationRefusal::BasisRequired)
    );
    assert_eq!(
        EvidenceSource::parse("a\u{0}b"),
        Err(InvestigationRefusal::EvidenceRequired)
    );
    assert_eq!(EvidenceSource::parse(" Ca 12 ").unwrap().as_str(), "Ca 12");
}

#[test]
fn an_investigation_opens_only_on_a_once_public_revision_of_a_now_private_work() {
    let dir = staffed();
    let (private, _) = once_public_now_private();
    let open = |dir: &ModerationDirectory, admin: u128, publication: &Publication, rev, exp| {
        Investigation::open(
            dir,
            p(admin),
            publication,
            InvestigationId::from_uuid(Uuid::from_u128(0x1A)),
            revision(rev),
            ViolationBasis::parse("Căn cứ").unwrap(),
            EvidenceSource::parse("Nguồn").unwrap(),
            exp,
            at(1),
        )
    };

    assert_eq!(
        open(&dir, MOD_A, &private, 1, None).map(|_| ()),
        Err(InvestigationRefusal::Authority(AuthorityRefusal::NotAdmin))
    );
    let mut self_admin = facts(vec![], vec![]);
    self_admin.admins.push(owner());
    let self_admin = ModerationDirectory::restore(self_admin).unwrap();
    assert_eq!(
        open(&self_admin, 1, &private, 1, None).map(|_| ()),
        Err(InvestigationRefusal::Conflict)
    );
    let never_public = save(&save(&Publication::new(work(1), owner()), 1), 2);
    assert_eq!(
        open(&dir, ADMIN, &never_public, 1, None).map(|_| ()),
        Err(InvestigationRefusal::NeverPublic)
    );
    assert_eq!(
        open(&dir, ADMIN, &private, 2, None).map(|_| ()),
        Err(InvestigationRefusal::NeverPublic)
    );
    assert_eq!(
        open(&dir, ADMIN, &private, 1, Some(at(1))).map(|_| ()),
        Err(InvestigationRefusal::Expired)
    );
    assert!(open(&dir, ADMIN, &private, 1, Some(at(2))).is_ok());
}

#[test]
fn every_read_rechecks_the_file_and_returns_the_audit_row_with_the_exact_snapshot() {
    let dir = staffed();
    let (private, moderation) = once_public_now_private();
    let file = open_file(&private, Some(at(5)));
    let read = |file: &Investigation, admin: u128, publication: &Publication, rev, hour| {
        file.read(&dir, p(admin), publication, revision(rev), at(hour))
            .map(|r| (r.audit, r.snapshot.revision))
    };

    let (audit, snapshot_revision) = read(&file, ADMIN, &private, 1, 1).unwrap();
    assert_eq!(snapshot_revision, revision(1));
    assert_eq!(audit.admin, p(ADMIN));
    assert_eq!(audit.revision, revision(1));
    assert_eq!(audit.investigation, file.facts().id);
    assert_eq!(audit.at, at(1));

    assert_eq!(
        read(&file, OTHER_ADMIN, &private, 1, 1),
        Err(InvestigationRefusal::NotInvestigator)
    );
    assert_eq!(
        read(&file, ADMIN, &private, 2, 1),
        Err(InvestigationRefusal::WrongRevision)
    );
    assert_eq!(
        read(&file, ADMIN, &private, 1, 5),
        Err(InvestigationRefusal::Expired)
    );
    let elsewhere = Publication::new(work(2), owner());
    assert_eq!(
        read(&file, ADMIN, &elsewhere, 1, 1),
        Err(InvestigationRefusal::WrongWork)
    );
    let mut demoted = facts(vec![], vec![]);
    demoted.admins.retain(|a| *a != p(ADMIN));
    let demoted = ModerationDirectory::restore(demoted).unwrap();
    assert_eq!(
        file.read(&demoted, p(ADMIN), &private, revision(1), at(1))
            .map(|_| ()),
        Err(InvestigationRefusal::Authority(AuthorityRefusal::NotAdmin))
    );

    // The right lapses once the author makes the work public again.
    let public = moderation
        .decide_author_with(
            &private,
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(1),
                expected: private.facts().generation,
            },
            None,
            AutomationMode::Enabled,
        )
        .unwrap();
    assert_eq!(public.outcome, AuthorOutcome::Published);
    assert_eq!(
        read(&file, ADMIN, &public.publication, 1, 1),
        Err(InvestigationRefusal::ConditionLapsed)
    );
}

#[test]
fn only_the_opening_admin_closes_a_file_once_and_a_closed_file_reads_nothing() {
    let dir = staffed();
    let (private, _) = once_public_now_private();
    let file = open_file(&private, None);
    let basis = || ViolationBasis::parse("Không thấy vi phạm").unwrap();

    assert_eq!(
        file.close(&dir, p(OTHER_ADMIN), Finding::NoViolation, basis(), at(1)),
        Err(InvestigationRefusal::NotInvestigator)
    );
    let closed = file
        .close(&dir, p(ADMIN), Finding::NoViolation, basis(), at(1))
        .unwrap();
    assert_eq!(
        closed.facts().state,
        InvestigationState::Closed {
            finding: Finding::NoViolation,
            basis: basis()
        }
    );
    assert_eq!(
        closed.close(&dir, p(ADMIN), Finding::Violation, basis(), at(2)),
        Err(InvestigationRefusal::Closed)
    );
    assert_eq!(
        closed
            .read(&dir, p(ADMIN), &private, revision(1), at(2))
            .map(|_| ()),
        Err(InvestigationRefusal::Closed)
    );
    let restored = Investigation::restore(InvestigationFacts {
        ..closed.facts().clone()
    });
    assert_eq!(restored, closed);
}
