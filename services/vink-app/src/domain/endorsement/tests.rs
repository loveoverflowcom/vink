use super::*;
use crate::domain::discussion::lifecycle::{ModerationVersion, Review};
use crate::domain::discussion::{reply_placement, root_placement};

pub(super) fn actor(n: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(n))
}
pub(super) fn work(n: u128) -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(n))
}
pub(super) fn revision(n: u128) -> RevisionId {
    RevisionId::from_uuid(Uuid::from_u128(n))
}
pub(super) fn target() -> EndorsementTarget {
    EndorsementTarget::Work {
        work: work(10),
        revision: revision(11),
    }
}
pub(super) fn cmd(key: u128, expected: i64, selected: bool) -> EndorsementCommand {
    EndorsementCommand {
        key: EndorsementCommandKey::from_uuid(Uuid::from_u128(key)),
        target: target(),
        expected: AcceptedOrder::new(expected).unwrap(),
        intent: if selected {
            EndorsementIntent::Select
        } else {
            EndorsementIntent::Remove
        },
    }
}
pub(super) fn facts() -> EndorsementFacts {
    EndorsementFacts {
        can_endorse: true,
        current: None,
        receipt: None,
        target: Some(EndorsementTargetFacts::work(
            work(10),
            actor(2),
            Some(revision(11)),
        )),
    }
}
pub(super) fn accepted(outcome: EndorsementOutcome) -> EndorsementWrite {
    let EndorsementOutcome::Accepted(write) = outcome else {
        panic!("expected acceptance, got {outcome:?}");
    };
    write
}

#[test]
fn off_noop_fences_the_older_on_even_when_off_arrives_first() {
    // A=ON@1 was created first; B=OFF@1 arrives first. B is a new accepted intent, not replay.
    let b = accepted(decide_endorsement(actor(1), cmd(2, 1, false), facts()));
    assert_eq!(b.delta(), EndorsementDelta::None);
    assert_eq!(b.next().state().version().get(), 1);
    assert_eq!(b.next().state().order().get(), 2);
    let f = EndorsementFacts {
        current: Some(b.next()),
        ..facts()
    };
    assert_eq!(
        decide_endorsement(actor(1), cmd(1, 1, true), f),
        EndorsementOutcome::Conflict {
            observed: b.next().state()
        }
    );
    // Same-state command with a different key is also stale, not a successful retry.
    assert!(matches!(
        decide_endorsement(actor(1), cmd(3, 1, false), f),
        EndorsementOutcome::Conflict { .. }
    ));
}

#[test]
fn replay_is_historical_and_never_resurrects_after_remove_or_a_fresh_key() {
    let on = accepted(decide_endorsement(actor(1), cmd(1, 1, true), facts()));
    let off = accepted(decide_endorsement(
        actor(1),
        cmd(2, 2, false),
        EndorsementFacts {
            current: Some(on.next()),
            ..facts()
        },
    ));
    let f = EndorsementFacts {
        current: Some(off.next()),
        receipt: Some(on.receipt()),
        ..facts()
    };
    assert_eq!(
        decide_endorsement(actor(1), cmd(1, 1, true), f),
        EndorsementOutcome::Replay {
            committed: on.next().state(),
            observed: off.next().state()
        }
    );
    assert!(!off.next().state().selected());
    assert_eq!(off.next().state().version().get(), 3);
    assert!(matches!(
        decide_endorsement(
            actor(1),
            cmd(3, 1, true),
            EndorsementFacts { receipt: None, ..f }
        ),
        EndorsementOutcome::Conflict { .. }
    ));
    assert_eq!(
        decide_endorsement(actor(1), cmd(1, 1, false), f),
        EndorsementOutcome::Refused(EndorsementRefusal::KeyReused)
    );
    let again = accepted(decide_endorsement(
        actor(1),
        cmd(3, 3, true),
        EndorsementFacts { receipt: None, ..f },
    ));
    assert_eq!(
        again.next().state().order().get(),
        4,
        "remove/recreate never resets order"
    );
}

#[test]
fn same_state_new_key_advances_order_but_not_mutation_version_or_count() {
    let on = accepted(decide_endorsement(actor(1), cmd(1, 1, true), facts()));
    let repeat = accepted(decide_endorsement(
        actor(1),
        cmd(2, 2, true),
        EndorsementFacts {
            current: Some(on.next()),
            ..facts()
        },
    ));
    assert_eq!(repeat.delta(), EndorsementDelta::None);
    assert_eq!(repeat.next().state().version().get(), 2);
    assert_eq!(repeat.next().state().order().get(), 3);
    assert_eq!(
        EndorsementCount::new(1).unwrap().apply(repeat.delta()),
        EndorsementCount::new(1).ok_or(CountError::Underflow)
    );
}

#[test]
fn current_revision_is_checked_even_when_selected_is_already_true() {
    let on = accepted(decide_endorsement(actor(1), cmd(1, 1, true), facts()));
    let f = EndorsementFacts {
        current: Some(on.next()),
        target: Some(EndorsementTargetFacts::work(
            work(10),
            actor(2),
            Some(revision(12)),
        )),
        ..facts()
    };
    assert_eq!(
        decide_endorsement(actor(1), cmd(2, 2, true), f),
        EndorsementOutcome::Refused(EndorsementRefusal::TargetUnavailable)
    );
    assert_eq!(
        public_count(target(), f.target, EndorsementCount::new(1).unwrap()),
        None
    );
    assert_eq!(target().anchor(), EndorsementAnchor::Revision(revision(11)));
    // Removing one's historical anchor remains possible without reading its content.
    assert_eq!(
        accepted(decide_endorsement(actor(1), cmd(2, 2, false), f)).delta(),
        EndorsementDelta::Remove
    );
}

#[test]
fn hidden_withdrawn_missing_only_allow_scoped_private_own_removal() {
    let on = accepted(decide_endorsement(actor(1), cmd(1, 1, true), facts()));
    for hidden in [
        None,
        Some(EndorsementTargetFacts::work(work(10), actor(2), None)),
    ] {
        let f = EndorsementFacts {
            target: hidden,
            current: Some(on.next()),
            ..facts()
        };
        let off = accepted(decide_endorsement(actor(1), cmd(2, 2, false), f));
        assert_eq!(off.delta(), EndorsementDelta::Remove);
        assert_eq!(
            public_count(target(), hidden, EndorsementCount::new(1).unwrap()),
            None
        );
        assert_eq!(
            decide_endorsement(actor(1), cmd(2, 2, true), f),
            EndorsementOutcome::Refused(EndorsementRefusal::TargetUnavailable)
        );
        assert_eq!(
            decide_endorsement(
                actor(1),
                cmd(2, 1, false),
                EndorsementFacts { current: None, ..f }
            ),
            EndorsementOutcome::Refused(EndorsementRefusal::TargetUnavailable)
        );
        assert_eq!(
            decide_endorsement(actor(3), cmd(2, 2, false), f),
            EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
        );
    }
}

#[test]
fn work_root_reply_have_independent_targets_and_no_self_endorsement() {
    let thread = ThreadId::from_uuid(Uuid::from_u128(20));
    let root = root_placement(thread, CommentId::from_uuid(Uuid::from_u128(21)));
    let reply = reply_placement(thread, CommentId::from_uuid(Uuid::from_u128(22)), root).unwrap();
    let public = CommentState::restore(Review::Approved, None, ModerationVersion::FIRST);
    for placement in [root, reply] {
        let target = EndorsementTarget::Comment {
            work: work(10),
            comment: placement.id(),
        };
        let f = EndorsementFacts {
            target: EndorsementTargetFacts::comment(
                work(10),
                thread,
                placement,
                actor(2),
                public,
                true,
            ),
            ..facts()
        };
        let c = EndorsementCommand {
            target,
            ..cmd(1, 1, true)
        };
        assert_eq!(
            accepted(decide_endorsement(actor(1), c, f)).next().target(),
            target
        );
        assert_eq!(
            decide_endorsement(actor(2), c, f),
            EndorsementOutcome::Refused(EndorsementRefusal::OwnContent)
        );
        assert_eq!(
            decide_endorsement(
                actor(1),
                c,
                EndorsementFacts {
                    target: EndorsementTargetFacts::comment(
                        work(10),
                        thread,
                        placement,
                        actor(2),
                        CommentState::SUBMITTED,
                        true
                    ),
                    ..f
                }
            ),
            EndorsementOutcome::Refused(EndorsementRefusal::TargetUnavailable)
        );
        assert_eq!(
            EndorsementTargetFacts::comment(
                work(10),
                ThreadId::from_uuid(Uuid::from_u128(30)),
                placement,
                actor(2),
                public,
                true
            ),
            None
        );
    }
    assert_eq!(
        decide_endorsement(actor(2), cmd(1, 1, true), facts()),
        EndorsementOutcome::Refused(EndorsementRefusal::OwnContent)
    );
}

#[test]
fn foreign_facts_and_corrupt_receipt_fail_closed_before_writing() {
    let on = accepted(decide_endorsement(actor(1), cmd(1, 1, true), facts()));
    for wrong in [
        EndorsementFacts {
            current: Some(on.next()),
            ..facts()
        },
        EndorsementFacts {
            target: Some(EndorsementTargetFacts::work(
                work(99),
                actor(2),
                Some(revision(11)),
            )),
            ..facts()
        },
    ] {
        let command = EndorsementCommand {
            target: EndorsementTarget::Work {
                work: work(99),
                revision: revision(11),
            },
            ..cmd(2, 2, false)
        };
        // First variant: retained record belongs to work10. Second: command is work10, facts work99.
        let command = if wrong.current.is_some() {
            command
        } else {
            cmd(2, 2, false)
        };
        assert_eq!(
            decide_endorsement(actor(1), command, wrong),
            EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
        );
    }
    assert_eq!(
        decide_endorsement(
            actor(1),
            cmd(1, 1, true),
            EndorsementFacts {
                receipt: Some(on.receipt()),
                ..facts()
            }
        ),
        EndorsementOutcome::Refused(EndorsementRefusal::InvalidFacts)
    );
    assert!(EndorsementReceipt::restore(actor(1), cmd(1, 1, false), on.next().state()).is_none());
    assert_eq!(
        decide_endorsement(
            actor(1),
            cmd(1, 1, true),
            EndorsementFacts {
                can_endorse: false,
                ..facts()
            }
        ),
        EndorsementOutcome::Refused(EndorsementRefusal::AuthorityUnavailable)
    );
}

#[test]
fn exhausted_tokens_and_counts_refuse_without_partial_writes() {
    // i64::MAX is odd, so the only reachable state there is OFF; ON sits one version below.
    for (selected, version) in [(false, i64::MAX), (true, i64::MAX - 1)] {
        let state = EndorsementState::restore(selected, version, i64::MAX).unwrap();
        let f = EndorsementFacts {
            current: Some(StoredEndorsement::restore(actor(1), target(), state)),
            ..facts()
        };
        for wanted in [false, true] {
            assert_eq!(
                decide_endorsement(actor(1), cmd(1, i64::MAX, wanted), f),
                EndorsementOutcome::Refused(EndorsementRefusal::TokenExhausted)
            );
        }
    }
    assert_eq!(EndorsementState::restore(false, 2, 1), None);
    assert_eq!(EndorsementState::restore(false, 0, 1), None);
    assert_eq!(EndorsementCount::new(-1), None);
    assert_eq!(
        EndorsementCount::ZERO.apply(EndorsementDelta::Remove),
        Err(CountError::Underflow)
    );
    let max = EndorsementCount::new(i64::MAX).unwrap();
    assert_eq!(max.apply(EndorsementDelta::Add), Err(CountError::Overflow));
    assert_eq!(max.get(), i64::MAX);
}

/// Every state the reducer accepts is one storage can restore, and storage restores no state the
/// reducer cannot reach: ON exactly at an even version. Exhaustive over every intent sequence of
/// length <= 8 from the absent state, each command at the current order.
#[test]
fn accepted_states_keep_selection_parity_and_storage_restores_only_reachable_states() {
    let mut frontier = vec![EndorsementState::INITIAL];
    let mut seen = 0;
    for _ in 0..8 {
        let mut next_frontier = Vec::new();
        for state in frontier {
            for wanted in [false, true] {
                let f = EndorsementFacts {
                    current: (state != EndorsementState::INITIAL)
                        .then(|| StoredEndorsement::restore(actor(1), target(), state)),
                    ..facts()
                };
                let next = accepted(decide_endorsement(
                    actor(1),
                    cmd(1, state.order().get(), wanted),
                    f,
                ))
                .next()
                .state();
                let (version, order) = (next.version().get(), next.order().get());
                assert_eq!(next.selected(), version % 2 == 0, "{next:?}");
                assert_eq!(
                    EndorsementState::restore(next.selected(), version, order),
                    Some(next)
                );
                assert_eq!(
                    EndorsementState::restore(!next.selected(), version, order),
                    None,
                    "the opposite selection at {version}/{order} is unreachable"
                );
                next_frontier.push(next);
                seen += 1;
            }
        }
        frontier = next_frontier;
    }
    assert_eq!(seen, 510);
    // Storage shapes the database also refuses (selected without a version step, and back).
    assert_eq!(EndorsementState::restore(true, 1, 2), None);
    assert_eq!(EndorsementState::restore(false, 2, 2), None);
    assert_eq!(EndorsementState::restore(false, 2, 3), None);
}
