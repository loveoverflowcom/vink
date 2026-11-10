use super::*;
use crate::domain::{
    document::{Document, Title},
    ids::{PrincipalId, RevisionId, WorkId},
    publication::{
        AuthorOutcome, IntentGeneration, Moderation, PointerEffect, ReviewOutcome,
        ReviewedRevision, Snapshot,
    },
    revision::RevisionContent,
};

#[test]
fn use_case_calls_the_real_core_for_r1_r2_withdrawal_and_stale_commands() {
    let actor = BoundPrincipal {
        principal: PrincipalId::from_uuid(uuid::Uuid::from_u128(1)),
        standing: Standing::Active,
    };
    let work = WorkId::from_uuid(uuid::Uuid::from_u128(1));
    let r1 = RevisionId::from_uuid(uuid::Uuid::from_u128(1));
    let r2 = RevisionId::from_uuid(uuid::Uuid::from_u128(2));
    let content = RevisionContent::new(Title::parse("Mưa").unwrap(), Document::empty()).unwrap();
    let initial = Publication::new(work, actor.principal);
    let save = |id| {
        AuthorCommand::SaveSnapshot(Snapshot {
            work,
            revision: id,
            content: content.clone(),
        })
    };
    assert_eq!(
        plan_publication(None, &initial, save(r1)),
        Err(PublicationPlanError::Unauthenticated)
    );
    assert_eq!(
        plan_publication(
            Some(BoundPrincipal {
                standing: Standing::Suspended,
                ..actor
            }),
            &initial,
            save(r1)
        ),
        Err(PublicationPlanError::Suspended)
    );
    let stranger = BoundPrincipal {
        principal: PrincipalId::from_uuid(uuid::Uuid::from_u128(2)),
        ..actor
    };
    assert_eq!(
        plan_publication(Some(stranger), &initial, save(r1)),
        Err(PublicationPlanError::Rejected(PublicationError::NotFound))
    );
    let saved = plan_publication(Some(actor), &initial, save(r1)).unwrap();
    assert_eq!(saved.effects.evaluation, None);
    let public = |state: &Publication, id| {
        plan_publication(
            Some(actor),
            state,
            AuthorCommand::RequestPublic {
                revision: id,
                expected: state.facts().generation,
            },
        )
        .unwrap()
    };
    let pending = public(&saved.next, r1);
    assert_eq!(pending.outcome, AuthorOutcome::Submitted);
    let approved = pending
        .next
        .record_review(ReviewedRevision {
            work,
            revision: r1,
            hash: content.hash().clone(),
            expected: Moderation::Pending,
            outcome: ReviewOutcome::Approved,
        })
        .unwrap()
        .next;
    let saved2 = plan_publication(Some(actor), &approved, save(r2))
        .unwrap()
        .next;
    let pending2 = public(&saved2, r2).next;
    assert_eq!(pending2.readable_revision().unwrap().revision, r1);
    let rejected = pending2
        .record_review(ReviewedRevision {
            work,
            revision: r2,
            hash: content.hash().clone(),
            expected: Moderation::Pending,
            outcome: ReviewOutcome::Rejected,
        })
        .unwrap()
        .next;
    assert_eq!(rejected.readable_revision().unwrap().revision, r1);
    let private = plan_publication(
        Some(actor),
        &rejected,
        AuthorCommand::SetPrivate {
            expected: rejected.facts().generation,
        },
    )
    .unwrap();
    assert_eq!(private.effects.pointer, PointerEffect::Clear);
    assert_eq!(private.next.readable_revision(), None);
    assert_eq!(
        plan_publication(
            Some(actor),
            &private.next,
            AuthorCommand::RequestPublic {
                revision: r1,
                expected: IntentGeneration::FIRST
            }
        ),
        Err(PublicationPlanError::Rejected(PublicationError::Stale))
    );
    let republished = public(&private.next, r1);
    assert_eq!(republished.outcome, AuthorOutcome::Published);
}
