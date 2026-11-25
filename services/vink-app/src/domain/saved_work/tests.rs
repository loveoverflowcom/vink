use super::*;
use crate::domain::publication::{IntentGeneration, PublicPointer, tests::snapshot};
use time::macros::datetime;

const FIRST_SAVE: OffsetDateTime = datetime!(2026-11-25 08:30 UTC);
const LATER: OffsetDateTime = datetime!(2026-11-26 11:00 UTC);

fn key() -> SavedWorkKey {
    SavedWorkKey {
        principal: PrincipalId::from_uuid(uuid::Uuid::from_u128(1)),
        work: WorkId::from_uuid(uuid::Uuid::from_u128(1)),
    }
}

fn public(revision: u128) -> SavedWorkAccess {
    SavedWorkAccess::Public {
        work: key().work,
        revision: RevisionId::from_uuid(uuid::Uuid::from_u128(revision)),
    }
}

fn generation(raw: i64) -> SavedGeneration {
    SavedGeneration::new(raw).unwrap()
}

fn command(desired: SaveIntent, expected: i64) -> SetSaved {
    SetSaved {
        key: key(),
        desired,
        expected_generation: generation(expected),
    }
}

fn initial() -> SavedWork {
    SavedWork::initial(key())
}

fn saved() -> SavedWork {
    let result = initial().decide(
        command(SaveIntent::Saved, 1),
        Standing::Active,
        public(10),
        0,
        FIRST_SAVE,
    );
    let Ok(SavedChange::Applied(state)) = result else {
        panic!("expected first save, got {result:?}")
    };
    assert_eq!(state.generation(), generation(2));
    assert_eq!(state.saved_at(), Some(FIRST_SAVE));
    state
}

#[test]
fn retry_reaffirm_and_aba_across_unsave() {
    use SaveIntent::{NotSaved, Saved};
    let first = saved();
    let apply = |state: SavedWork, intent, fence| {
        state.decide(
            command(intent, fence),
            Standing::Active,
            public(10),
            1,
            LATER,
        )
    };
    // Lost ACK. Same inputs at the old fence leave both date and generation untouched.
    assert_eq!(
        apply(first, Saved, 1),
        Ok(SavedChange::AlreadyCurrent(first))
    );
    let Ok(SavedChange::Reaffirmed(again)) = apply(first, Saved, 2) else {
        panic!("fresh same-state intent must advance")
    };
    assert_eq!(
        (again.generation(), again.saved_at()),
        (generation(3), Some(FIRST_SAVE))
    );
    assert_eq!(
        apply(again, NotSaved, 2),
        Err(SaveError::Stale {
            current: generation(3)
        })
    );
    let Ok(SavedChange::Applied(cleared)) = apply(again, NotSaved, 3) else {
        panic!("fresh clear")
    };
    // Reload the logical state, even though it has no membership or saved_at.
    let reloaded = SavedWork::restore(key(), generation(4), None).unwrap();
    assert_eq!(cleared, reloaded);
    assert_eq!(
        apply(reloaded, NotSaved, 3),
        Ok(SavedChange::AlreadyCurrent(reloaded))
    );
    assert_eq!(
        apply(reloaded, Saved, 1),
        Err(SaveError::Stale {
            current: generation(4)
        })
    );
    let Ok(SavedChange::Reaffirmed(still_clear)) = apply(reloaded, NotSaved, 4) else {
        panic!("fresh clear on absent membership")
    };
    assert_eq!(
        apply(still_clear, Saved, 4),
        Err(SaveError::Stale {
            current: generation(5)
        })
    );
    let Ok(SavedChange::Applied(resaved)) = apply(still_clear, Saved, 5) else {
        panic!("later save")
    };
    assert_eq!(
        (resaved.generation(), resaved.saved_at()),
        (generation(6), Some(LATER))
    );
    assert_eq!(
        apply(resaved, NotSaved, 3),
        Err(SaveError::Stale {
            current: generation(6)
        })
    );
    assert_eq!(
        apply(resaved, Saved, 1),
        Ok(SavedChange::AlreadyCurrent(resaved))
    );
}

#[test]
fn first_unsave_fences_delayed_save_and_future_fence_never_replays() {
    let remove = command(SaveIntent::NotSaved, 1);
    let Ok(SavedChange::Reaffirmed(clear)) =
        initial().decide(remove, Standing::Active, SavedWorkAccess::Gone, 0, LATER)
    else {
        panic!("first unsave must persist a fence")
    };
    assert_eq!(clear.saved_at(), None);
    assert_eq!(clear.generation(), generation(2));
    for (intent, expected) in [
        (SaveIntent::Saved, 1),
        (SaveIntent::NotSaved, 3),
        (SaveIntent::Saved, 3),
    ] {
        assert_eq!(
            clear.decide(
                command(intent, expected),
                Standing::Active,
                public(10),
                0,
                LATER
            ),
            Err(SaveError::Stale {
                current: generation(2)
            })
        );
    }
}

#[test]
fn cap_counts_only_active_memberships_and_only_blocks_new_saves() {
    use SaveIntent::{NotSaved, Saved};
    let fence_only = SavedWork::restore(key(), generation(8), None).unwrap();
    let add = command(Saved, 8);
    assert_eq!(
        fence_only.decide(add, Standing::Active, public(10), 10_000, LATER),
        Err(SaveError::LimitReached)
    );
    let Ok(SavedChange::Applied(last_slot)) =
        fence_only.decide(add, Standing::Active, public(10), 9_999, FIRST_SAVE)
    else {
        panic!("last slot")
    };
    assert_eq!(last_slot.generation(), generation(9));
    assert_eq!(
        last_slot.decide(add, Standing::Active, public(11), MAX_SAVED_WORKS, LATER),
        Ok(SavedChange::AlreadyCurrent(last_slot))
    );
    let Ok(SavedChange::Reaffirmed(full)) = last_slot.decide(
        command(Saved, 9),
        Standing::Active,
        public(11),
        MAX_SAVED_WORKS,
        LATER,
    ) else {
        panic!("reaffirmation consumes no slot")
    };
    assert_eq!(full.saved_at(), Some(FIRST_SAVE));
    let removed = full.decide(
        command(NotSaved, 10),
        Standing::Active,
        SavedWorkAccess::Gone,
        MAX_SAVED_WORKS,
        LATER,
    );
    let Ok(SavedChange::Applied(next)) = removed else {
        panic!("unsave frees a slot")
    };
    assert_eq!(next.saved_at(), None);
}

#[test]
fn commit_checks_current_access_even_for_retries_and_allows_removal() {
    let state = saved();
    for access in [
        SavedWorkAccess::Hidden,
        SavedWorkAccess::AuthorDeleted,
        SavedWorkAccess::Gone,
    ] {
        for expected in [1, 2] {
            assert_eq!(
                state.decide(
                    command(SaveIntent::Saved, expected),
                    Standing::Active,
                    access,
                    1,
                    LATER
                ),
                Err(SaveError::NotReadable)
            );
        }
        assert_eq!(
            initial().decide(
                command(SaveIntent::Saved, 1),
                Standing::Active,
                access,
                0,
                LATER
            ),
            Err(SaveError::NotReadable)
        );
        let Ok(SavedChange::Applied(cleared)) = state.decide(
            command(SaveIntent::NotSaved, 2),
            Standing::Active,
            access,
            1,
            LATER,
        ) else {
            panic!("unsave must work for {access:?}")
        };
        assert_eq!(
            (cleared.saved_at(), cleared.generation()),
            (None, generation(3))
        );
    }
}

#[test]
fn commands_and_projections_are_owner_scoped_and_require_active_standing() {
    let state = saved();
    let other = PrincipalId::from_uuid(uuid::Uuid::from_u128(2));
    let other_publication = SavedWorkAccess::Public {
        work: WorkId::from_uuid(uuid::Uuid::from_u128(2)),
        revision: RevisionId::from_uuid(uuid::Uuid::from_u128(10)),
    };
    assert_eq!(
        state.project(key().principal, Standing::Active, other_publication),
        None
    );
    assert_eq!(
        state.decide(
            command(SaveIntent::Saved, 2),
            Standing::Active,
            other_publication,
            1,
            LATER
        ),
        Err(SaveError::NotReadable)
    );
    for intent in [SaveIntent::Saved, SaveIntent::NotSaved] {
        let cmd = command(intent, 2);
        assert_eq!(
            state.decide(cmd, Standing::Suspended, public(10), 1, LATER),
            Err(SaveError::Suspended)
        );
        for wrong in [
            SavedWorkKey {
                principal: other,
                ..key()
            },
            SavedWorkKey {
                work: WorkId::from_uuid(uuid::Uuid::from_u128(2)),
                ..key()
            },
        ] {
            assert_eq!(
                state.decide(
                    SetSaved { key: wrong, ..cmd },
                    Standing::Active,
                    public(10),
                    1,
                    LATER
                ),
                Err(SaveError::WrongKey)
            );
        }
    }
    assert_eq!(state.project(other, Standing::Active, public(10)), None);
    assert_eq!(
        state.project(key().principal, Standing::Suspended, public(10)),
        None
    );
    assert_eq!(
        initial().project(key().principal, Standing::Active, public(10)),
        None
    );
}

#[test]
fn author_can_save_own_work_only_while_its_public_body_is_readable() {
    use crate::domain::publication::{AuthorCommand, Publication, ReviewOutcome, ReviewedRevision};
    let owner = key().principal;
    let work = key().work;
    let snapshot = snapshot(10);
    let revision = snapshot.revision;
    let hash = snapshot.content.hash().clone();
    let draft = Publication::new(work, owner)
        .decide_author(owner, AuthorCommand::SaveSnapshot(snapshot))
        .unwrap()
        .next;
    let save = |publication: &Publication| {
        initial().decide(
            command(SaveIntent::Saved, 1),
            Standing::Active,
            (&publication.access_plan()).into(),
            0,
            FIRST_SAVE,
        )
    };
    assert_eq!(save(&draft), Err(SaveError::NotReadable));
    let pending = draft
        .decide_author(
            owner,
            AuthorCommand::RequestPublic {
                revision,
                expected: draft.facts().generation,
            },
        )
        .unwrap()
        .next;
    assert_eq!(save(&pending), Err(SaveError::NotReadable));
    let published = pending
        .record_review(ReviewedRevision {
            work,
            revision,
            hash,
            expected: pending.facts().revisions[0].moderation,
            outcome: ReviewOutcome::Approved,
        })
        .unwrap()
        .next;
    assert_eq!(save(&published), Ok(SavedChange::Applied(saved())));
}

#[test]
fn lifecycle_reprojects_current_publication_and_retains_original_save_time() {
    use WorkAccessPlan::{AuthorHidden, AuthorTombstone, PublicBody, ReviewWithdrawn, Unavailable};
    let state = saved();
    let work = key().work;
    let generation = state.generation();
    let first = snapshot(10);
    let later = snapshot(11);
    let last_pointer = PublicPointer {
        revision: first.revision,
        intent_generation: IntentGeneration::FIRST,
    };
    let readable = |revision| {
        Some(SavedRow::Readable {
            work,
            generation,
            revision,
            saved_at: FIRST_SAVE,
        })
    };
    for (plan, expected) in [
        (PublicBody(&first), readable(first.revision)),
        (
            AuthorHidden { last_pointer },
            Some(SavedRow::Hidden { work, generation }),
        ),
        // Both work deletion and author account deletion use the author's tombstone.
        (
            AuthorTombstone { last_pointer },
            Some(SavedRow::AuthorDeleted { work, generation }),
        ),
        (ReviewWithdrawn { last_pointer }, None),
        (Unavailable, None), // Removed, never public or missing.
        (PublicBody(&later), readable(later.revision)),
    ] {
        assert_eq!(
            state.project(key().principal, Standing::Active, (&plan).into()),
            expected
        );
    }
}

#[test]
fn invalid_or_exhausted_generation_never_wraps_or_resurrects_membership() {
    assert!(SavedGeneration::new(0).is_none());
    assert!(SavedGeneration::new(-1).is_none());
    assert!(SavedWork::restore(key(), SavedGeneration::FIRST, Some(FIRST_SAVE)).is_none());
    let state = SavedWork::restore(key(), generation(i64::MAX), None).unwrap();
    for intent in [SaveIntent::Saved, SaveIntent::NotSaved] {
        assert_eq!(
            state.decide(
                command(intent, i64::MAX),
                Standing::Active,
                public(10),
                0,
                LATER
            ),
            Err(SaveError::GenerationExhausted)
        );
    }
}
