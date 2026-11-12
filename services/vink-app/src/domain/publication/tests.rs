use super::*;
use crate::domain::document::{Document, Title};

pub(crate) fn work(n: u128) -> WorkId {
    WorkId::from_uuid(uuid::Uuid::from_u128(n))
}
pub(crate) fn owner() -> PrincipalId {
    PrincipalId::from_uuid(uuid::Uuid::from_u128(1))
}
pub(crate) fn revision(n: u128) -> RevisionId {
    RevisionId::from_uuid(uuid::Uuid::from_u128(n))
}
pub(crate) fn generation(n: i64) -> IntentGeneration {
    IntentGeneration::new(n).unwrap()
}
pub(crate) fn snapshot(n: u128) -> Snapshot {
    Snapshot {
        work: work(1),
        revision: revision(n),
        content: RevisionContent::new(
            Title::parse(&format!("Bản {n}")).unwrap(),
            Document::empty(),
        )
        .unwrap(),
    }
}
pub(crate) fn save(state: &Publication, n: u128) -> Publication {
    let result = state
        .decide_author(owner(), AuthorCommand::SaveSnapshot(snapshot(n)))
        .unwrap();
    assert_eq!(result.outcome, AuthorOutcome::Saved);
    assert_eq!(result.effects, Effects::NONE);
    result.next
}
pub(super) fn request(state: &Publication, n: u128) -> AuthorTransition {
    state
        .decide_author(
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(n),
                expected: state.facts().generation,
            },
        )
        .unwrap()
}
pub(super) fn review(state: &Publication, n: u128, outcome: ReviewOutcome) -> ReviewTransition {
    state
        .record_review(ReviewedRevision {
            work: work(1),
            revision: revision(n),
            hash: snapshot(n).content.hash().clone(),
            expected: state.revision(revision(n)).unwrap().moderation,
            outcome,
        })
        .unwrap()
}
pub(super) fn private(state: &Publication) -> AuthorTransition {
    state
        .decide_author(
            owner(),
            AuthorCommand::SetPrivate {
                expected: state.facts().generation,
            },
        )
        .unwrap()
}

#[test]
fn private_saves_use_canonical_immutable_content_without_evaluation() {
    let state = save(&Publication::new(work(1), owner()), 1);
    assert_eq!(state.facts().audience, Audience::Private);
    assert_eq!(state.facts().generation, IntentGeneration::FIRST);
    assert_eq!(
        state.facts().revisions[0].snapshot.content.hash(),
        snapshot(1).content.hash()
    );
    assert_eq!(state.readable_revision(), None);
    assert_eq!(
        state.decide_author(owner(), AuthorCommand::SaveSnapshot(snapshot(1))),
        Err(PublicationError::DuplicateRevision)
    );
    let mut foreign = snapshot(2);
    foreign.work = work(2);
    assert_eq!(
        state.decide_author(owner(), AuthorCommand::SaveSnapshot(foreign)),
        Err(PublicationError::WrongWork)
    );
}

#[test]
fn r1_remains_readable_while_r2_is_pending_needs_admin_rejected_or_approved_without_current_intent()
{
    let state = save(&Publication::new(work(1), owner()), 1);
    let submitted = request(&state, 1);
    assert_eq!(submitted.outcome, AuthorOutcome::Submitted);
    assert_eq!(
        submitted.effects.evaluation,
        Some(EvaluationRequest {
            work: work(1),
            revision: revision(1),
            hash: snapshot(1).content.hash().clone()
        })
    );
    assert_eq!(submitted.next.readable_revision(), None);
    let published = review(&submitted.next, 1, ReviewOutcome::Approved).next;
    let r2 = save(&published, 2);
    let pending = request(&r2, 2).next;
    assert_eq!(pending.readable_revision(), Some(&snapshot(1)));
    assert!(!pending.promotion_eligible(pending.facts().intent.unwrap()));
    let waiting = request(&pending, 2);
    assert_eq!(waiting.outcome, AuthorOutcome::Waiting);
    assert_eq!(waiting.effects, Effects::NONE);
    let uncertain = review(&waiting.next, 2, ReviewOutcome::NeedsAdmin).next;
    assert_eq!(request(&uncertain, 2).effects, Effects::NONE);
    let rejected = review(&uncertain, 2, ReviewOutcome::Rejected);
    assert_eq!(rejected.next.readable_revision(), Some(&snapshot(1)));
    assert_eq!(rejected.effects, Effects::NONE);
    assert_eq!(
        rejected.next.decide_author(
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(2),
                expected: rejected.next.facts().generation
            }
        ),
        Err(PublicationError::RevisionRejected)
    );
    // Request r1 again; a later admitted approval of r2 must not promote r2 or rewrite r1.
    let r1_again = request(&rejected.next, 1).next;
    let late_r2 = review(&r1_again, 2, ReviewOutcome::Approved);
    assert_eq!(late_r2.next.readable_revision(), Some(&snapshot(1)));
    assert_eq!(late_r2.effects, Effects::NONE);
    let promote_r2 = request(&late_r2.next, 2);
    assert_eq!(promote_r2.outcome, AuthorOutcome::Published);
    assert_eq!(promote_r2.next.readable_revision(), Some(&snapshot(2)));
}

#[test]
fn withdrawn_intent_fences_late_approval_but_retains_the_decision() {
    let state = request(&save(&Publication::new(work(1), owner()), 1), 1).next;
    let old_intent = state.facts().intent.unwrap();
    let private = private(&state);
    assert_eq!(private.outcome, AuthorOutcome::MadePrivate);
    assert_eq!(private.effects.pointer, PointerEffect::Clear);
    assert_eq!(private.next.facts().intent, None);
    let approved = review(&private.next, 1, ReviewOutcome::Approved);
    assert_eq!(approved.next.readable_revision(), None);
    assert_eq!(
        approved.next.facts().revisions[0].moderation,
        Moderation::Approved
    );
    assert_eq!(approved.effects, Effects::NONE);
    assert!(!approved.next.promotion_eligible(old_intent));
    // Explicitly asking for that exact approved revision may publish it, without new evaluation.
    let republished = request(&approved.next, 1);
    assert_eq!(republished.outcome, AuthorOutcome::Published);
    assert_eq!(republished.effects.evaluation, None);
}

#[test]
fn owner_unknown_revision_stale_tabs_and_overflow_refuse_without_a_plan() {
    assert_eq!(IntentGeneration::new(0), None);
    assert_eq!(IntentGeneration::new(-1), None);
    let state = save(&Publication::new(work(1), owner()), 1);
    let before = state.clone();
    assert_eq!(
        state.decide_author(
            PrincipalId::from_uuid(uuid::Uuid::from_u128(2)),
            AuthorCommand::SetPrivate {
                expected: generation(1)
            }
        ),
        Err(PublicationError::NotFound)
    );
    assert_eq!(
        state.decide_author(
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(9),
                expected: generation(1)
            }
        ),
        Err(PublicationError::UnknownRevision)
    );
    let reaffirmed = private(&state);
    assert_eq!(reaffirmed.outcome, AuthorOutcome::Reaffirmed);
    assert_eq!(reaffirmed.next.facts().generation, generation(2));
    for command in [
        AuthorCommand::SetPrivate {
            expected: generation(1),
        },
        AuthorCommand::RequestPublic {
            revision: revision(1),
            expected: generation(1),
        },
    ] {
        assert_eq!(
            reaffirmed.next.decide_author(owner(), command),
            Err(PublicationError::Stale)
        );
    }
    assert_eq!(state, before);
    let mut facts = state.facts().clone();
    facts.generation = generation(i64::MAX);
    let last = Publication::restore(facts).unwrap();
    for command in [
        AuthorCommand::SetPrivate {
            expected: generation(i64::MAX),
        },
        AuthorCommand::RequestPublic {
            revision: revision(1),
            expected: generation(i64::MAX),
        },
    ] {
        assert_eq!(
            last.decide_author(owner(), command),
            Err(PublicationError::GenerationExhausted)
        );
    }
    assert_eq!(
        last.facts().revisions[0].moderation,
        Moderation::Unsubmitted
    );
}

#[test]
fn restore_rejects_inconsistent_bindings_and_accepts_old_pointer_with_new_intent() {
    let state = save(&Publication::new(work(1), owner()), 1);
    let pending = request(&state, 1).next;
    let public = review(&pending, 1, ReviewOutcome::Approved).next;
    let r2 = request(&save(&public, 2), 2).next;
    assert_eq!(Publication::restore(r2.facts().clone()), Ok(r2.clone()));
    let mut invalid = Vec::new();
    let base = r2.facts().clone();
    let mut f = base.clone();
    f.revisions[0].snapshot.work = work(2);
    invalid.push(f);
    let mut f = base.clone();
    f.revisions.push(f.revisions[0].clone());
    invalid.push(f);
    let mut f = base.clone();
    f.audience = Audience::Private;
    invalid.push(f);
    let mut f = base.clone();
    f.intent = None;
    invalid.push(f);
    let mut f = base.clone();
    f.intent.as_mut().unwrap().revision = revision(9);
    invalid.push(f);
    let mut f = base.clone();
    f.intent.as_mut().unwrap().generation = generation(1);
    invalid.push(f);
    let mut f = base.clone();
    f.revisions[1].moderation = Moderation::Unsubmitted;
    invalid.push(f);
    let mut f = base.clone();
    f.pointer.as_mut().unwrap().revision = revision(9);
    invalid.push(f);
    let mut f = base.clone();
    f.pointer.as_mut().unwrap().intent_generation = generation(i64::MAX);
    invalid.push(f);
    let mut f = base.clone();
    f.pointer.as_mut().unwrap().intent_generation = base.generation;
    invalid.push(f);
    let mut f = base.clone();
    f.revisions[0].moderation = Moderation::Rejected;
    invalid.push(f);
    for lifecycle in [
        Lifecycle::AuthorDeleted(DeletionProvenance::Hidden),
        Lifecycle::Removed,
    ] {
        let mut f = base.clone();
        f.lifecycle = lifecycle;
        invalid.push(f);
    }
    let mut f = state.facts().clone();
    f.revisions[0].moderation = Moderation::Pending;
    invalid.push(f);
    let mut f = private(&r2).next.facts().clone();
    f.audience = Audience::Public;
    f.lifecycle = Lifecycle::AuthorDeleted(DeletionProvenance::Hidden);
    invalid.push(f);
    for facts in invalid {
        assert_eq!(
            Publication::restore(facts),
            Err(PublicationError::InvalidFacts)
        );
    }
}

#[test]
fn inactive_facts_never_publish_or_resurrect_and_reviews_recheck_exact_binding() {
    let saved = save(&Publication::new(work(1), owner()), 1);
    assert_eq!(
        saved.record_review(ReviewedRevision {
            work: work(1),
            revision: revision(1),
            hash: snapshot(1).content.hash().clone(),
            expected: Moderation::Unsubmitted,
            outcome: ReviewOutcome::Approved
        }),
        Err(PublicationError::NotSubmitted)
    );
    let pending = request(&saved, 1).next;
    let good = ReviewedRevision {
        work: work(1),
        revision: revision(1),
        hash: snapshot(1).content.hash().clone(),
        expected: Moderation::Pending,
        outcome: ReviewOutcome::Approved,
    };
    let mut wrong = good.clone();
    wrong.hash = snapshot(2).content.hash().clone();
    assert_eq!(
        pending.record_review(wrong),
        Err(PublicationError::WrongHash)
    );
    let mut wrong = good.clone();
    wrong.work = work(2);
    assert_eq!(
        pending.record_review(wrong),
        Err(PublicationError::WrongWork)
    );
    let mut wrong = good.clone();
    wrong.revision = revision(2);
    assert_eq!(
        pending.record_review(wrong),
        Err(PublicationError::UnknownRevision)
    );
    let mut wrong = good.clone();
    wrong.expected = Moderation::Approved;
    assert_eq!(
        pending.record_review(wrong),
        Err(PublicationError::StaleModeration)
    );
    for lifecycle in [
        Lifecycle::AuthorDeleted(DeletionProvenance::Hidden),
        Lifecycle::Removed,
    ] {
        let mut facts = private(&pending).next.facts().clone();
        facts.lifecycle = lifecycle;
        let terminal = Publication::restore(facts).unwrap();
        assert_eq!(terminal.readable_revision(), None);
        assert!(!terminal.promotion_eligible(pending.facts().intent.unwrap()));
        for command in [
            AuthorCommand::SaveSnapshot(snapshot(2)),
            AuthorCommand::RequestPublic {
                revision: revision(1),
                expected: terminal.facts().generation,
            },
            AuthorCommand::SetPrivate {
                expected: terminal.facts().generation,
            },
        ] {
            assert_eq!(
                terminal.decide_author(owner(), command),
                Err(PublicationError::WorkUnavailable)
            );
        }
        assert_eq!(
            terminal.record_review(good.clone()),
            Err(PublicationError::WorkUnavailable)
        );
    }
}

#[test]
fn author_delete_and_admin_remove_transitions_and_access_plans() {
    let saved = save(&Publication::new(work(1), owner()), 1);
    let requested = request(&saved, 1);
    let public = review(&requested.next, 1, ReviewOutcome::Approved).next;

    // 1. Delete on a public work produces a PublicTombstone
    let deleted_pub = public
        .decide_author(
            owner(),
            AuthorCommand::Delete {
                expected: Some(public.facts().generation),
            },
        )
        .unwrap();
    assert_eq!(deleted_pub.outcome, AuthorOutcome::Deleted);
    assert_eq!(deleted_pub.effects.pointer, PointerEffect::Clear);
    assert_eq!(deleted_pub.next.facts().audience, Audience::Private);
    assert_eq!(deleted_pub.next.facts().generation.get(), 3);
    assert_eq!(deleted_pub.next.readable_revision(), None);
    let pointer = PublicPointer {
        revision: revision(1),
        intent_generation: generation(2),
    };
    assert_eq!(
        deleted_pub.next.facts().lifecycle,
        Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(pointer))
    );
    assert_eq!(
        deleted_pub.next.access_plan(),
        WorkAccessPlan::AuthorTombstone {
            last_pointer: pointer
        }
    );
    assert!(!deleted_pub.next.access_plan().is_listed());
    assert!(!deleted_pub.next.access_plan().allows_snippet());
    assert!(deleted_pub.next.access_plan().thread_accessible());

    // 2. A public work made private shows the hidden page; deleting it from there keeps the
    //    once-public pointer as the tombstone (PUB-07, ADMIN-MODERATION-20261111).
    let private_work = private(&public).next;
    assert_eq!(
        private_work.access_plan(),
        WorkAccessPlan::AuthorHidden {
            last_pointer: pointer
        }
    );
    assert!(!private_work.access_plan().is_listed());
    assert!(!private_work.access_plan().allows_snippet());
    assert!(private_work.access_plan().thread_accessible());
    assert!(!private_work.access_plan().accepts_new_writes());
    let deleted_priv = private_work
        .decide_author(owner(), AuthorCommand::Delete { expected: None })
        .unwrap();
    assert_eq!(deleted_priv.outcome, AuthorOutcome::Deleted);
    assert_eq!(
        deleted_priv.next.facts().lifecycle,
        Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(pointer))
    );
    assert!(deleted_priv.next.access_plan().thread_accessible());

    // 3. Delete on never-public work produces Hidden provenance
    let never_public = save(&Publication::new(work(1), owner()), 1);
    let deleted_never = never_public
        .decide_author(owner(), AuthorCommand::Delete { expected: None })
        .unwrap();
    assert_eq!(
        deleted_never.next.facts().lifecycle,
        Lifecycle::AuthorDeleted(DeletionProvenance::Hidden)
    );
    assert_eq!(
        deleted_never.next.access_plan(),
        WorkAccessPlan::Unavailable
    );

    // 4. Admin remove clears pointer, does not advance generation, keeps audience
    let (removed, effects) = public.admin_remove().unwrap();
    assert_eq!(effects.pointer, PointerEffect::Clear);
    assert_eq!(removed.facts().lifecycle, Lifecycle::Removed);
    assert_eq!(removed.facts().audience, Audience::Public);
    assert_eq!(removed.facts().generation, public.facts().generation);
    assert_eq!(removed.access_plan(), WorkAccessPlan::Unavailable);

    // 5. Subsequent mutations on deleted or removed fail with WorkUnavailable
    for terminal in [&deleted_pub.next, &removed] {
        assert_eq!(
            terminal.decide_author(owner(), AuthorCommand::Delete { expected: None }),
            Err(PublicationError::WorkUnavailable)
        );
        assert_eq!(
            terminal.admin_remove(),
            Err(PublicationError::WorkUnavailable)
        );
        assert_eq!(
            terminal.decide_author(
                owner(),
                AuthorCommand::RequestPublic {
                    revision: revision(1),
                    expected: terminal.facts().generation,
                }
            ),
            Err(PublicationError::WorkUnavailable)
        );
    }

    // 6. Stale generation on Delete fails with Stale
    assert_eq!(
        public.decide_author(
            owner(),
            AuthorCommand::Delete {
                expected: Some(generation(99))
            }
        ),
        Err(PublicationError::Stale)
    );
}

#[test]
fn restore_validates_deletion_provenance_and_fails_closed_on_corruption() {
    let saved = save(&Publication::new(work(1), owner()), 1);
    let requested = request(&saved, 1);
    let public = review(&requested.next, 1, ReviewOutcome::Approved).next;
    let deleted = public
        .decide_author(owner(), AuthorCommand::Delete { expected: None })
        .unwrap()
        .next;

    // Valid restore
    assert_eq!(
        Publication::restore(deleted.facts().clone()),
        Ok(deleted.clone())
    );

    // A once-public revision may later be rejected by a moderator; the tombstone stays valid.
    let mut rejected_later = deleted.facts().clone();
    rejected_later.revisions[0].moderation = Moderation::Rejected;
    assert!(Publication::restore(rejected_later).is_ok());

    // Incompatible: PublicTombstone of a revision that was never public
    let mut corrupted = deleted.facts().clone();
    corrupted.ever_public.clear();
    corrupted.public_epoch = PublicEpoch::NEVER;
    assert_eq!(
        Publication::restore(corrupted),
        Err(PublicationError::InvalidFacts)
    );

    // Incompatible: PublicTombstone with cross-work revision
    let mut corrupted = deleted.facts().clone();
    corrupted.lifecycle =
        Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(PublicPointer {
            revision: revision(999),
            intent_generation: generation(2),
        }));
    assert_eq!(
        Publication::restore(corrupted),
        Err(PublicationError::InvalidFacts)
    );

    // Incompatible: PublicTombstone with future intent_generation
    let mut corrupted = deleted.facts().clone();
    corrupted.lifecycle =
        Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(PublicPointer {
            revision: revision(1),
            intent_generation: generation(99),
        }));
    assert_eq!(
        Publication::restore(corrupted),
        Err(PublicationError::InvalidFacts)
    );

    // Incompatible: AuthorDeleted with Audience::Public
    let mut corrupted = deleted.facts().clone();
    corrupted.audience = Audience::Public;
    assert_eq!(
        Publication::restore(corrupted),
        Err(PublicationError::InvalidFacts)
    );
}

#[test]
fn rejecting_currently_public_revision_sets_review_withdrawn_access_plan() {
    let saved = save(&Publication::new(work(1), owner()), 1);
    let requested = request(&saved, 1);
    let public = review(&requested.next, 1, ReviewOutcome::Approved).next;

    assert!(public.readable_revision().is_some());
    assert_eq!(public.facts().review_withdrawn, None);

    // Rejecting currently public revision r1
    let rejected = review(&public, 1, ReviewOutcome::Rejected);
    let withdrawn_state = rejected.next;

    assert_eq!(withdrawn_state.readable_revision(), None);
    assert_eq!(rejected.effects.pointer, PointerEffect::Clear);
    assert_eq!(
        withdrawn_state.facts().review_withdrawn,
        Some(PublicPointer {
            revision: revision(1),
            intent_generation: generation(2),
        })
    );

    // Access plan classification
    let plan = withdrawn_state.access_plan();
    assert_eq!(
        plan,
        WorkAccessPlan::ReviewWithdrawn {
            last_pointer: PublicPointer {
                revision: revision(1),
                intent_generation: generation(2),
            }
        }
    );
    assert_eq!(plan.page_kind(), "review_withdrawn");
    assert!(!plan.is_listed());
    assert!(!plan.allows_snippet());
    assert!(plan.thread_accessible());
    assert!(!plan.accepts_new_writes());

    // Restore validates review_withdrawn correctly
    assert_eq!(
        Publication::restore(withdrawn_state.facts().clone()),
        Ok(withdrawn_state.clone())
    );

    // Making work private clears review_withdrawn
    let private = withdrawn_state
        .decide_author(
            owner(),
            AuthorCommand::SetPrivate {
                expected: withdrawn_state.facts().generation,
            },
        )
        .unwrap()
        .next;
    assert_eq!(private.facts().review_withdrawn, None);
    assert_eq!(private.access_plan(), WorkAccessPlan::Unavailable);
}
