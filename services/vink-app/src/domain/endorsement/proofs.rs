//! Kani 0.67.0, production source in place. No loops/collections: default unwind, assertions on.
//! Scalars cover all i64; IDs use three symbolic values (all equality/inequality relations).
//! Facts are server-origin assumptions, not proof of auth/SQL. No target text is modelled.
use super::*;

fn principal(n: u8) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(u128::from(n)))
}
fn target(n: u8) -> EndorsementTarget {
    EndorsementTarget::Work {
        work: WorkId::from_uuid(Uuid::from_u128(u128::from(n))),
        revision: RevisionId::from_uuid(Uuid::from_u128(10)),
    }
}
fn command(expected: i64, selected: bool) -> EndorsementCommand {
    EndorsementCommand {
        key: EndorsementCommandKey::from_uuid(Uuid::from_u128(50)),
        target: target(1),
        expected: AcceptedOrder::new(expected).unwrap(),
        intent: if selected {
            EndorsementIntent::Select
        } else {
            EndorsementIntent::Remove
        },
    }
}
fn facts(state: EndorsementState) -> EndorsementFacts {
    EndorsementFacts {
        can_endorse: true,
        current: Some(StoredEndorsement::restore(principal(1), target(1), state)),
        receipt: None,
        target: Some(EndorsementTargetFacts::work(
            target(1).work(),
            principal(2),
            Some(RevisionId::from_uuid(Uuid::from_u128(10))),
        )),
    }
}

#[kani::proof]
fn endorsement_scalars_and_count_arithmetic_are_total() {
    let raw: i64 = kani::any();
    assert_eq!(AcceptedOrder::new(raw).is_some(), raw >= 1);
    assert_eq!(EndorsementVersion::new(raw).is_some(), raw >= 1);
    assert_eq!(EndorsementCount::new(raw).is_some(), raw >= 0);
    if let Some(count) = EndorsementCount::new(raw) {
        assert_eq!(count.apply(EndorsementDelta::None), Ok(count));
        let add = count.apply(EndorsementDelta::Add);
        let remove = count.apply(EndorsementDelta::Remove);
        assert_eq!(add.is_err(), raw == i64::MAX);
        assert_eq!(remove.is_err(), raw == 0);
        if let Ok(next) = add {
            assert_eq!(next.get(), raw + 1);
        }
        if let Ok(next) = remove {
            assert_eq!(next.get(), raw - 1);
        }
        kani::cover!(add == Err(CountError::Overflow));
        kani::cover!(remove == Err(CountError::Underflow));
        kani::cover!(raw > 0 && raw < i64::MAX);
    }
}

#[kani::proof]
fn endorsement_acceptance_fences_every_new_intent_including_same_state() {
    let order: i64 = kani::any();
    let version: i64 = kani::any();
    let expected: i64 = kani::any();
    kani::assume(order >= 1 && version >= 1 && version <= order && expected >= 1);
    let before = EndorsementState::restore(kani::any(), version, order).unwrap();
    let c = command(expected, kani::any());
    let result = decide_endorsement(principal(1), c, facts(before));
    match result {
        EndorsementOutcome::Accepted(write) => {
            let after = write.next().state();
            assert_eq!(expected, order);
            assert_eq!(after.order().get(), order + 1);
            assert_eq!(after.selected(), c.intent.selected());
            let changed = before.selected() != after.selected();
            assert_eq!(after.version().get(), version + i64::from(changed));
            assert_eq!(write.delta() == EndorsementDelta::None, !changed);
            assert_eq!(write.receipt().command(), c);
            assert_eq!(write.receipt().committed(), after);
            assert_eq!(write.next().actor(), principal(1));
            assert_eq!(write.next().target(), c.target);
            kani::cover!(!changed);
            kani::cover!(changed && after.selected());
            kani::cover!(changed && !after.selected());
        }
        EndorsementOutcome::Conflict { observed } => {
            assert_ne!(expected, order);
            assert_eq!(observed, before);
        }
        EndorsementOutcome::Refused(EndorsementRefusal::TokenExhausted) => {
            assert_eq!(expected, order);
            assert_eq!(order, i64::MAX);
        }
        _ => panic!("valid public facts cannot produce this outcome"),
    }
    kani::cover!(matches!(result, EndorsementOutcome::Conflict { .. }));
    kani::cover!(matches!(
        result,
        EndorsementOutcome::Refused(EndorsementRefusal::TokenExhausted)
    ));
}

#[kani::proof]
fn endorsement_noop_then_delayed_on_and_remove_recreate_cannot_reset_order() {
    let off = EndorsementCommand {
        key: EndorsementCommandKey::from_uuid(Uuid::from_u128(51)),
        ..command(1, false)
    };
    let EndorsementOutcome::Accepted(write) =
        decide_endorsement(principal(1), off, facts(EndorsementState::INITIAL))
    else {
        panic!("OFF accepted");
    };
    let retained = write.next().state();
    let late = EndorsementCommand {
        key: EndorsementCommandKey::from_uuid(Uuid::from_u128(52)),
        ..command(1, true)
    };
    assert!(matches!(
        decide_endorsement(principal(1), late, facts(retained)),
        EndorsementOutcome::Conflict { .. }
    ));
    let fresh = EndorsementCommand {
        key: EndorsementCommandKey::from_uuid(Uuid::from_u128(53)),
        ..command(2, true)
    };
    let EndorsementOutcome::Accepted(on) = decide_endorsement(principal(1), fresh, facts(retained))
    else {
        panic!("fresh ON accepted");
    };
    let EndorsementOutcome::Accepted(off) =
        decide_endorsement(principal(1), command(3, false), facts(on.next().state()))
    else {
        panic!("remove accepted");
    };
    assert_eq!(off.next().state().order().get(), 4);
    assert!(matches!(
        decide_endorsement(principal(1), late, facts(off.next().state())),
        EndorsementOutcome::Conflict { .. }
    ));
    kani::cover!(!off.next().state().selected());
}

#[kani::proof]
fn endorsement_eligibility_and_scoped_private_removal_fail_closed() {
    let selected: bool = kani::any();
    let c = command(2, selected);
    let state = EndorsementState::restore(true, 2, 2).unwrap();
    let mut f = facts(state);
    f.can_endorse = kani::any();
    let visible: bool = kani::any();
    let own: bool = kani::any();
    let foreign: bool = kani::any();
    f.target = Some(EndorsementTargetFacts::work(
        target(if foreign { 2 } else { 1 }).work(),
        principal(if own { 1 } else { 2 }),
        if visible {
            Some(RevisionId::from_uuid(Uuid::from_u128(10)))
        } else {
            None
        },
    ));
    let outcome = decide_endorsement(principal(1), c, f);
    if !f.can_endorse {
        assert_eq!(
            outcome,
            EndorsementOutcome::Refused(EndorsementRefusal::AuthorityUnavailable)
        );
    } else if foreign {
        assert_eq!(
            outcome,
            EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
        );
    } else if !selected {
        assert!(matches!(outcome, EndorsementOutcome::Accepted(_)));
    } else if !visible {
        assert_eq!(
            outcome,
            EndorsementOutcome::Refused(EndorsementRefusal::TargetUnavailable)
        );
    } else if own {
        assert_eq!(
            outcome,
            EndorsementOutcome::Refused(EndorsementRefusal::OwnContent)
        );
    } else {
        assert!(matches!(outcome, EndorsementOutcome::Accepted(_)));
    }
    assert_eq!(
        public_count(c.target, f.target, EndorsementCount::ZERO).is_some(),
        visible && !foreign
    );
    kani::cover!(!selected && !visible && matches!(outcome, EndorsementOutcome::Accepted(_)));
    kani::cover!(selected && own && visible && !foreign && f.can_endorse);
    // A record belonging to someone else can never be cleared.
    let foreign_record = EndorsementFacts {
        current: Some(StoredEndorsement::restore(principal(3), target(1), state)),
        ..facts(state)
    };
    assert_eq!(
        decide_endorsement(principal(1), command(2, false), foreign_record),
        EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
    );
}

#[kani::proof]
fn endorsement_receipt_replays_history_separately_and_rejects_changed_payload() {
    let EndorsementOutcome::Accepted(on) = decide_endorsement(
        principal(1),
        command(1, true),
        facts(EndorsementState::INITIAL),
    ) else {
        panic!("ON accepted");
    };
    let after = EndorsementState::restore(false, 3, 3).unwrap();
    let mut f = facts(after);
    f.receipt = Some(on.receipt());
    let replay = decide_endorsement(principal(1), command(1, true), f);
    assert_eq!(
        replay,
        EndorsementOutcome::Replay {
            committed: on.next().state(),
            observed: after
        }
    );
    assert_eq!(
        decide_endorsement(principal(1), command(1, false), f),
        EndorsementOutcome::Refused(EndorsementRefusal::KeyReused)
    );
    let other_actor = EndorsementFacts {
        receipt: EndorsementReceipt::restore(principal(3), command(1, true), on.next().state()),
        ..f
    };
    assert_eq!(
        decide_endorsement(principal(1), command(1, true), other_actor),
        EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
    );
    kani::cover!(
        matches!(replay, EndorsementOutcome::Replay { committed, observed } if committed.selected() && !observed.selected())
    );
}

#[kani::proof]
fn endorsement_comment_scope_and_visibility_reuse_two_tier_facts() {
    use crate::domain::discussion::lifecycle::{ModerationVersion, Removal, Review};
    use crate::domain::discussion::{reply_placement, root_placement};
    let thread = ThreadId::from_uuid(Uuid::from_u128(20));
    let root = root_placement(thread, CommentId::from_uuid(Uuid::from_u128(21)));
    let reply = reply_placement(thread, CommentId::from_uuid(Uuid::from_u128(22)), root).unwrap();
    let is_root: bool = kani::any();
    let placement = if is_root { root } else { reply };
    let approved: bool = kani::any();
    let removed: bool = kani::any();
    let work_public: bool = kani::any();
    let own: bool = kani::any();
    let state = CommentState::restore(
        if approved {
            Review::Approved
        } else {
            Review::Pending
        },
        if removed {
            Some(Removal::ByAuthor)
        } else {
            None
        },
        ModerationVersion::FIRST,
    );
    let t = EndorsementTarget::Comment {
        work: target(1).work(),
        comment: placement.id(),
    };
    let tf = EndorsementTargetFacts::comment(
        t.work(),
        thread,
        placement,
        principal(if own { 1 } else { 2 }),
        state,
        work_public,
    )
    .unwrap();
    let visible = approved && !removed && work_public;
    assert_eq!(tf.visible(t), visible);
    let f = EndorsementFacts {
        current: None,
        target: Some(tf),
        ..facts(EndorsementState::INITIAL)
    };
    let outcome = decide_endorsement(
        principal(1),
        EndorsementCommand {
            target: t,
            ..command(1, true)
        },
        f,
    );
    if !visible {
        assert_eq!(
            outcome,
            EndorsementOutcome::Refused(EndorsementRefusal::TargetUnavailable)
        );
    } else if own {
        assert_eq!(
            outcome,
            EndorsementOutcome::Refused(EndorsementRefusal::OwnContent)
        );
    } else {
        assert!(matches!(outcome, EndorsementOutcome::Accepted(_)));
    }
    let wrong = ThreadId::from_uuid(Uuid::from_u128(30));
    assert!(
        EndorsementTargetFacts::comment(
            t.work(),
            wrong,
            placement,
            principal(2),
            state,
            work_public
        )
        .is_none()
    );
    let other_comment = EndorsementTarget::Comment {
        work: t.work(),
        comment: CommentId::from_uuid(Uuid::from_u128(99)),
    };
    assert_eq!(
        decide_endorsement(
            principal(1),
            EndorsementCommand {
                target: other_comment,
                ..command(1, true)
            },
            f
        ),
        EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
    );
    kani::cover!(is_root && visible && !own);
    kani::cover!(!is_root && visible && !own);
}
