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

#[test]
fn read_planner_composes_access_and_discussion_visibility_truth_table() {
    let author_actor = BoundPrincipal {
        principal: PrincipalId::from_uuid(uuid::Uuid::from_u128(1)),
        standing: Standing::Active,
    };
    let reader = PrincipalId::from_uuid(uuid::Uuid::from_u128(2));
    let stranger = PrincipalId::from_uuid(uuid::Uuid::from_u128(3));
    let work = WorkId::from_uuid(uuid::Uuid::from_u128(1));
    let r1 = RevisionId::from_uuid(uuid::Uuid::from_u128(1));
    let content = RevisionContent::new(Title::parse("Mưa").unwrap(), Document::empty()).unwrap();

    let initial = Publication::new(work, author_actor.principal);
    let saved = plan_publication(
        Some(author_actor),
        &initial,
        AuthorCommand::SaveSnapshot(Snapshot {
            work,
            revision: r1,
            content,
        }),
    )
    .unwrap()
    .next;

    // 1. Unpublished work
    assert_eq!(plan_work_access(&saved), WorkAccessPlan::Unavailable);
    assert_eq!(
        check_comment_submission(&saved),
        Err(TombstoneWriteError::WorkUnavailable)
    );
    let comment_state = CommentState::SUBMITTED;
    assert_eq!(
        plan_comment_view(&saved, reader, comment_state, None),
        CommentView::Unavailable
    );
    assert_eq!(
        plan_root_listing(&saved, reader, comment_state, 0, None),
        None
    );
    assert_eq!(
        plan_reply_context(&saved, false, None),
        ReplyContext::Unavailable
    );
    assert_eq!(
        plan_own_overlay(&saved, reader, comment_state, Some(reader)),
        None
    );

    // 2. Public work
    let requested = plan_publication(
        Some(author_actor),
        &saved,
        AuthorCommand::RequestPublic {
            revision: r1,
            expected: saved.facts().generation,
        },
    )
    .unwrap()
    .next;
    let public = requested
        .record_review(ReviewedRevision {
            work,
            revision: r1,
            hash: requested.facts().revisions[0]
                .snapshot
                .content
                .hash()
                .clone(),
            expected: Moderation::Pending,
            outcome: ReviewOutcome::Approved,
        })
        .unwrap()
        .next;

    assert!(matches!(
        plan_work_access(&public),
        WorkAccessPlan::PublicBody(_)
    ));
    assert_eq!(check_comment_submission(&public), Ok(()));

    // For public comment:
    let approved_comment = match comment_state
        .decide(
            comment_state.version(),
            crate::domain::discussion::lifecycle::Decision::Approve,
        )
        .unwrap()
    {
        crate::domain::discussion::lifecycle::Transition::Changed(s)
        | crate::domain::discussion::lifecycle::Transition::Unchanged(s) => s,
    };
    assert_eq!(
        plan_comment_view(&public, reader, approved_comment, None),
        CommentView::Public
    );
    assert_eq!(
        plan_root_listing(&public, reader, approved_comment, 0, None),
        Some(RootListing::Shown(CommentView::Public))
    );
    assert_eq!(plan_reply_context(&public, false, None), ReplyContext::Root);

    // For pending comment: author of comment sees OwnPending, stranger sees Unavailable
    assert_eq!(
        plan_comment_view(&public, reader, comment_state, Some(reader)),
        CommentView::OwnPending
    );
    assert_eq!(
        plan_comment_view(&public, reader, comment_state, Some(stranger)),
        CommentView::Unavailable
    );
    assert_eq!(
        plan_comment_view(&public, reader, comment_state, None),
        CommentView::Unavailable
    );

    // 3. Author-deleted public work -> Tombstone
    let deleted = plan_publication(
        Some(author_actor),
        &public,
        AuthorCommand::Delete {
            expected: Some(public.facts().generation),
        },
    )
    .unwrap()
    .next;

    assert!(matches!(
        plan_work_access(&deleted),
        WorkAccessPlan::AuthorTombstone { .. }
    ));
    // Submission is refused as tombstone read-only; own removal stays lawful
    assert_eq!(
        check_comment_submission(&deleted),
        Err(TombstoneWriteError::TombstoneReadOnly)
    );
    assert_eq!(check_own_comment_removal(&public), Ok(()));
    assert_eq!(check_own_comment_removal(&deleted), Ok(()));
    assert_eq!(
        check_own_comment_removal(&saved),
        Err(TombstoneWriteError::WorkUnavailable)
    );
    // Valid approved comments are still visible
    assert_eq!(
        plan_comment_view(&deleted, reader, approved_comment, None),
        CommentView::Public
    );
    // Own pending comments are visible only to the writer
    assert_eq!(
        plan_comment_view(&deleted, reader, comment_state, Some(reader)),
        CommentView::OwnPending
    );
    assert_eq!(
        plan_comment_view(&deleted, reader, comment_state, None),
        CommentView::Unavailable
    );

    // 4. Public work made private -> hidden page with its thread (PUB-07); deleted from there
    //    -> the author tombstone keeps the thread (ADMIN-MODERATION-20261111).
    let private = plan_publication(
        Some(author_actor),
        &public,
        AuthorCommand::SetPrivate {
            expected: public.facts().generation,
        },
    )
    .unwrap()
    .next;
    let priv_deleted = plan_publication(
        Some(author_actor),
        &private,
        AuthorCommand::Delete { expected: None },
    )
    .unwrap()
    .next;
    assert!(matches!(
        plan_work_access(&private),
        WorkAccessPlan::AuthorHidden { .. }
    ));
    assert_eq!(
        plan_comment_view(&private, reader, approved_comment, None),
        CommentView::Public
    );
    assert_eq!(
        check_comment_submission(&private),
        Err(TombstoneWriteError::HiddenReadOnly)
    );
    assert_eq!(check_own_comment_removal(&private), Ok(()));
    assert_eq!(
        plan_comment_review_context(&private),
        CommentReviewContext::CommentOnly
    );
    assert!(matches!(
        plan_work_access(&priv_deleted),
        WorkAccessPlan::AuthorTombstone { .. }
    ));
    assert_eq!(
        plan_comment_view(&priv_deleted, reader, approved_comment, None),
        CommentView::Public
    );
}
