use super::*;
use crate::domain::discussion::lifecycle::ModerationVersion;
use crate::domain::discussion::outbox::{ClaimGeneration, DeliveryState};

fn uuid(raw: &str) -> Uuid {
    Uuid::parse_str(raw).unwrap()
}

fn principal(n: u8) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_bytes([n; 16]))
}

fn state(review: Review, removal: Option<Removal>, version: i64) -> CommentState {
    CommentState::restore(review, removal, ModerationVersion::new(version).unwrap())
}

fn stamp(epoch: i64, readable: bool) -> NotificationStamp {
    NotificationStamp {
        epoch: PublicEpoch::new(epoch).unwrap(),
        readable,
    }
}

const WORK: Uuid = Uuid::from_u128(0xaaaa_aaaa_aaaa_4aaa_8aaa_aaaa_aaaa_aaaa);
const ROOT: Uuid = Uuid::from_u128(0xbbbb_bbbb_bbbb_4bbb_8bbb_bbbb_bbbb_bbbb);
const TARGET: Uuid = Uuid::from_u128(0xcccc_cccc_cccc_4ccc_8ccc_cccc_cccc_cccc);
const REPLY: Uuid = Uuid::from_u128(0xdddd_dddd_dddd_4ddd_8ddd_dddd_dddd_dddd);

fn at(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &Rfc3339).unwrap()
}

/// Contract §3 example 1 (reply) and its published digest, byte for byte.
#[test]
fn the_contract_reply_example_has_its_published_digest() {
    let a = PrincipalId::from_uuid(uuid("11111111-1111-4111-8111-11111111111a"));
    let b = PrincipalId::from_uuid(uuid("22222222-2222-4222-8222-22222222222b"));
    let c = PrincipalId::from_uuid(uuid("33333333-3333-4333-8333-33333333333c"));
    let owed = comment_moderated(CommentModerationFacts {
        work: WorkId::from_uuid(WORK),
        work_owner: principal(9),
        comment: CommentId::from_uuid(REPLY),
        writer: a,
        placement: PlacementFacts::Reply {
            root: CommentId::from_uuid(ROOT),
            root_author: c,
            target: ReplyTarget::Reply {
                id: CommentId::from_uuid(TARGET),
                author: b,
            },
        },
        before: state(Review::Pending, None, 1),
        after: state(Review::Approved, None, 2),
        stamp: Some(stamp(1, true)),
    });
    assert_eq!(owed.len(), 1);
    let envelope = Envelope::new(
        uuid("019a9b10-0001-7000-8000-000000000101"),
        1,
        at("2026-11-20T02:10:00Z"),
        owed[0].clone(),
    )
    .unwrap();
    assert_eq!(
        envelope.digest(),
        "e6f89afc584f2f9e4464294c5db85d852c4840a8cde79c891bfb038348ed26b6"
    );
    let wire = wire(&envelope.canonical(), &envelope.digest()).unwrap();
    assert_eq!(
        verify_wire(&wire),
        Some((envelope.event_id(), envelope.digest(), true))
    );
    // A changed byte is detected by the receiver's recomputation.
    let tampered = wire.replace("\"moderation_version\":2", "\"moderation_version\":3");
    assert!(matches!(verify_wire(&tampered), Some((_, _, false))));
}

/// Contract §3 example 1b (Thấm remove on a hidden work) and its published digest.
#[test]
fn the_contract_tham_example_has_its_published_digest() {
    let notice = tham_changed(ThamFacts {
        work: WorkId::from_uuid(WORK),
        revision: RevisionId::from_uuid(uuid("eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee")),
        owner: PrincipalId::from_uuid(uuid("55555555-5555-4555-8555-55555555555e")),
        actor: PrincipalId::from_uuid(uuid("44444444-4444-4444-8444-44444444444d")),
        delta: EndorsementDelta::Remove,
        version_after: 3,
        accepted_order: 4,
        stamp: stamp(1, false),
    })
    .unwrap();
    let envelope = Envelope::new(
        uuid("019a9b10-0002-7000-8000-000000000102"),
        1,
        at("2026-11-20T02:11:12Z"),
        notice,
    )
    .unwrap();
    assert_eq!(
        envelope.digest(),
        "c922b2f3355c071bf1cf9fca5ac1ac73031b34579808a0aa87ce438e35859140"
    );
}

fn reply_recipients(a: u8, b: u8, c: u8, target: Option<bool>) -> Vec<PrincipalId> {
    let placement = PlacementFacts::Reply {
        root: CommentId::from_uuid(ROOT),
        root_author: principal(c),
        target: match target {
            None => ReplyTarget::Root,
            Some(true) => ReplyTarget::Reply {
                id: CommentId::from_uuid(TARGET),
                author: principal(b),
            },
            Some(false) => ReplyTarget::Purged,
        },
    };
    comment_moderated(CommentModerationFacts {
        work: WorkId::from_uuid(WORK),
        work_owner: principal(99),
        comment: CommentId::from_uuid(REPLY),
        writer: principal(a),
        placement,
        before: state(Review::Pending, None, 1),
        after: state(Review::Approved, None, 2),
        stamp: Some(stamp(1, true)),
    })
    .into_iter()
    .flat_map(|notice| {
        assert_eq!(notice.event_type(), EventType::CommentReplyApproved);
        notice.recipients().to_vec()
    })
    .collect()
}

/// §4.3 B: A replies to B's reply under C's root (one flat tier), with every role overlap.
#[test]
fn reply_recipients_follow_p01b_with_self_exclusion_and_dedup() {
    let (a, b, c) = (1, 2, 3);
    assert_eq!(
        reply_recipients(a, b, c, Some(true)),
        vec![principal(b), principal(c)]
    );
    assert_eq!(reply_recipients(a, b, b, Some(true)), vec![principal(b)]);
    assert_eq!(reply_recipients(a, b, a, Some(true)), vec![principal(b)]);
    assert_eq!(reply_recipients(a, a, c, Some(true)), vec![principal(c)]);
    assert_eq!(reply_recipients(a, a, a, Some(true)), vec![]);
    assert_eq!(reply_recipients(a, b, c, None), vec![principal(c)]);
    assert_eq!(reply_recipients(a, b, a, None), vec![]);
    // The direct target's row was purged: only the root writer is still known.
    assert_eq!(reply_recipients(a, b, c, Some(false)), vec![principal(c)]);
    // The work owner is never a reply recipient by ownership alone.
    assert!(!reply_recipients(a, b, c, Some(true)).contains(&principal(99)));
}

fn root_facts(
    writer: u8,
    before: CommentState,
    after: CommentState,
    at: Option<NotificationStamp>,
) -> CommentModerationFacts {
    CommentModerationFacts {
        work: WorkId::from_uuid(WORK),
        work_owner: principal(7),
        comment: CommentId::from_uuid(ROOT),
        writer: principal(writer),
        placement: PlacementFacts::Root,
        before,
        after,
        stamp: at,
    }
}

#[test]
fn a_root_approval_reaches_only_the_work_owner_while_readable() {
    let pending = state(Review::Pending, None, 1);
    let approved = state(Review::Approved, None, 2);
    let owed = comment_moderated(root_facts(1, pending, approved, Some(stamp(2, true))));
    assert_eq!(owed.len(), 1);
    let notice = &owed[0];
    assert_eq!(notice.event_type(), EventType::CommentRootApproved);
    assert_eq!(notice.recipients(), &[principal(7)]);
    assert_eq!(notice.actor(), Some(principal(1)));
    assert_eq!(notice.stamp(), Some(stamp(2, true)));
    assert_eq!(notice.source_order(), 2);
    // Self: the owner's own root owes nothing.
    assert!(comment_moderated(root_facts(7, pending, approved, Some(stamp(1, true)))).is_empty());
    // COM-11: approved while hidden owes nothing, and nothing is owed later either.
    assert!(comment_moderated(root_facts(1, pending, approved, Some(stamp(1, false)))).is_empty());
    // A stamp that was not loaded never notifies.
    assert!(comment_moderated(root_facts(1, pending, approved, None)).is_empty());
    // Approve → approve (a version-advancing reaffirmation) is not a new approval.
    let again = state(Review::Approved, None, 3);
    assert!(comment_moderated(root_facts(1, approved, again, Some(stamp(1, true)))).is_empty());
    // Nothing changed at all (a stale or unchanged transition).
    assert!(comment_moderated(root_facts(1, pending, pending, Some(stamp(1, true)))).is_empty());
}

#[test]
fn adverse_decisions_reach_the_writer_and_invalidate_public_contributions() {
    let pending = state(Review::Pending, None, 1);
    let approved = state(Review::Approved, None, 2);
    let rejected = state(Review::Rejected, None, 3);
    let kinds = |owed: &[Notice]| -> Vec<(EventType, SemanticKind, Vec<PrincipalId>)> {
        owed.iter()
            .map(|n| (n.event_type(), n.semantic(), n.recipients().to_vec()))
            .collect()
    };
    // Rejecting a pending comment: the writer is told; nothing public to invalidate.
    let owed = comment_moderated(root_facts(
        1,
        pending,
        state(Review::Rejected, None, 2),
        None,
    ));
    assert_eq!(
        kinds(&owed),
        vec![(
            EventType::CommentModerationAdverse,
            SemanticKind::Rejected,
            vec![principal(1)]
        )]
    );
    assert_eq!(owed[0].actor(), None);
    assert!(!owed[0].action_required());
    // Rejecting an approved comment also invalidates its contribution (no recipients).
    let owed = comment_moderated(root_facts(1, approved, rejected, Some(stamp(1, true))));
    assert_eq!(
        kinds(&owed),
        vec![
            (
                EventType::CommentModerationAdverse,
                SemanticKind::Rejected,
                vec![principal(1)]
            ),
            (EventType::Invalidation, SemanticKind::Invalidate, vec![]),
        ]
    );
    // Removal by moderation of an approved comment.
    let removed = state(Review::Approved, Some(Removal::ByModeration), 3);
    let owed = comment_moderated(root_facts(1, approved, removed, None));
    assert_eq!(owed[0].semantic(), SemanticKind::Removed);
    assert_eq!(owed[1].event_type(), EventType::Invalidation);
    // Removal of a rejected comment is a new adverse decision; nothing was public.
    let owed = comment_moderated(root_facts(
        1,
        rejected,
        state(Review::Rejected, Some(Removal::ByModeration), 4),
        None,
    ));
    assert_eq!(kinds(&owed).len(), 1);
    assert_eq!(owed[0].semantic(), SemanticKind::Removed);
    // Rejected → rejected (version advanced) owes nothing new.
    assert!(
        comment_moderated(root_facts(
            1,
            rejected,
            state(Review::Rejected, None, 4),
            None
        ))
        .is_empty()
    );
    // Rejected → approved → owes a new approval (re-approval after an adverse decision).
    let reapproved = comment_moderated(root_facts(
        1,
        rejected,
        state(Review::Approved, None, 4),
        Some(stamp(1, true)),
    ));
    assert_eq!(reapproved[0].event_type(), EventType::CommentRootApproved);
}

#[test]
fn the_writers_own_removal_and_a_hold_only_invalidate() {
    let approved = state(Review::Approved, None, 2);
    let by_author = state(Review::Approved, Some(Removal::ByAuthor), 3);
    let held = state(Review::Pending, None, 3);
    for after in [by_author, held] {
        let notice = comment_left_public(
            WorkId::from_uuid(WORK),
            CommentId::from_uuid(ROOT),
            approved,
            after,
        )
        .unwrap();
        assert_eq!(notice.event_type(), EventType::Invalidation);
        assert!(notice.recipients().is_empty());
        assert_eq!(notice.source_order(), 3);
        assert_eq!(
            notice.dedup_key(),
            format!("notification.invalidation/comment/{ROOT}/3")
        );
    }
    let pending = state(Review::Pending, None, 1);
    assert_eq!(
        comment_left_public(
            WorkId::from_uuid(WORK),
            CommentId::from_uuid(ROOT),
            pending,
            state(Review::Pending, Some(Removal::ByAuthor), 2)
        ),
        None
    );
}

fn decision(moderation: Moderation, prior: Option<Conclusion>, version: i64) -> Option<Notice> {
    work_decided(WorkDecisionFacts {
        work: WorkId::from_uuid(WORK),
        revision: RevisionId::from_uuid(TARGET),
        owner: principal(7),
        decision_version: version,
        moderation,
        prior,
    })
}

/// §4.3 C and P-11a/b.
#[test]
fn work_decisions_notify_new_conclusions_only() {
    let approved = decision(Moderation::Approved, None, 2).unwrap();
    assert_eq!(approved.semantic(), SemanticKind::Approved);
    assert!(!approved.action_required());
    assert_eq!(approved.recipients(), &[principal(7)]);
    assert_eq!(approved.actor(), None);
    let rejected = decision(Moderation::Rejected, Some(Conclusion::Approved), 3).unwrap();
    assert_eq!(rejected.semantic(), SemanticKind::Rejected);
    assert!(rejected.action_required());
    let reversed = decision(Moderation::Approved, Some(Conclusion::Rejected), 4).unwrap();
    assert_eq!(reversed.source_order(), 4);
    assert_ne!(reversed.dedup_key(), approved.dedup_key());
    // Same verdict again (uphold, reaffirmation, re-review that approves again): nothing.
    assert_eq!(
        decision(Moderation::Approved, Some(Conclusion::Approved), 5),
        None
    );
    assert_eq!(
        decision(Moderation::Rejected, Some(Conclusion::Rejected), 5),
        None
    );
    // Not a conclusion: pending, needs-admin (uncertain/invalid/exhausted), unsubmitted.
    for open in [
        Moderation::Pending,
        Moderation::NeedsAdmin,
        Moderation::Unsubmitted,
    ] {
        assert_eq!(decision(open, None, 2), None);
        assert_eq!(decision(open, Some(Conclusion::Approved), 2), None);
    }
}

fn tham(delta: EndorsementDelta, version_after: i64, order: i64, actor: u8) -> Option<Notice> {
    tham_changed(ThamFacts {
        work: WorkId::from_uuid(WORK),
        revision: RevisionId::from_uuid(TARGET),
        owner: principal(7),
        actor: principal(actor),
        delta,
        version_after,
        accepted_order: order,
        stamp: stamp(1, true),
    })
}

/// §4.3 D: select, same-state fence, remove, select again.
#[test]
fn tham_owes_every_real_change_but_only_the_first_selection_may_signal() {
    let first = tham(EndorsementDelta::Add, 2, 2, 1).unwrap();
    assert_eq!(first.first_selection(), Some(true));
    assert_eq!(first.recipients(), &[principal(7)]);
    assert_eq!(tham(EndorsementDelta::None, 2, 3, 1), None);
    let removed = tham(EndorsementDelta::Remove, 3, 4, 1).unwrap();
    assert_eq!(removed.first_selection(), None);
    assert_eq!(removed.semantic(), SemanticKind::Remove);
    let relike = tham(EndorsementDelta::Add, 4, 5, 1).unwrap();
    assert_eq!(relike.first_selection(), Some(false));
    assert_eq!(relike.source_order(), 5);
    // Each actor and order is its own logical event.
    assert_ne!(
        first.dedup_key(),
        tham(EndorsementDelta::Add, 2, 2, 2).unwrap().dedup_key()
    );
    assert_ne!(first.dedup_key(), relike.dedup_key());
    // The owner cannot endorse their own work; if facts said so, nothing is owed.
    assert_eq!(tham(EndorsementDelta::Add, 2, 2, 7), None);
}

#[test]
fn envelopes_carry_identifiers_versions_and_codes_only() {
    let allowed = [
        "action_required",
        "activation_epoch",
        "actor_id",
        "event_id",
        "event_type",
        "first_selection",
        "occurred_at",
        "publication_stamp",
        "recipients",
        "schema_version",
        "semantic_kind",
        "source_order",
        "source_owner",
        "target",
    ];
    let notices = [
        tham(EndorsementDelta::Add, 2, 2, 1).unwrap(),
        decision(Moderation::Rejected, None, 2).unwrap(),
        work_unreadable(WorkId::from_uuid(WORK), PublicEpoch::new(3).unwrap()),
        principal_standing_changed(principal(5), 1),
    ];
    for notice in notices {
        let envelope =
            Envelope::new(Uuid::now_v7(), 2, at("2026-11-20T02:11:12+07:00"), notice).unwrap();
        let Value::Object(object) = envelope.json() else {
            panic!("object");
        };
        for key in object.keys() {
            assert!(allowed.contains(&key.as_str()), "{key}");
        }
        assert_eq!(object["occurred_at"], "2026-11-19T19:11:12Z");
        assert_eq!(object["schema_version"], 1);
        assert_eq!(object["activation_epoch"], 2);
        // Canonical text is stable and its keys are sorted.
        let text = envelope.canonical();
        assert!(text.starts_with("{\"action_required\":"));
        assert!(!text.contains(' '));
    }
    assert!(
        Envelope::new(
            Uuid::now_v7(),
            0,
            at("2026-11-20T02:11:12Z"),
            work_unreadable(WorkId::from_uuid(WORK), PublicEpoch::new(1).unwrap())
        )
        .is_none()
    );
}

#[test]
fn codes_are_closed_and_the_admin_type_has_no_producer() {
    for kind in EventType::ALL {
        assert_eq!(EventType::from_code(kind.code()), Some(kind));
        assert_eq!(kind.produced(), kind != EventType::AdminNoticeReserved);
    }
    assert_eq!(EventType::from_code("comment.useful.changed"), None);
    for status in IngestStatus::ALL {
        assert_eq!(IngestStatus::from_code(status.code()), Some(status));
    }
    assert_eq!(IngestStatus::from_code("ok"), None);
}

#[test]
fn activation_starts_a_new_epoch_and_deactivation_keeps_it() {
    let never = Activation::restore(false, 0).unwrap();
    assert_eq!(never.producing(), None);
    let ActivationChange::Changed(first) = never.change(true) else {
        panic!("activate");
    };
    assert_eq!(
        (first.active(), first.epoch(), first.producing()),
        (true, 1, Some(1))
    );
    assert_eq!(first.change(true), ActivationChange::Unchanged(first));
    let ActivationChange::Changed(paused) = first.change(false) else {
        panic!("deactivate");
    };
    assert_eq!((paused.epoch(), paused.producing()), (1, None));
    let ActivationChange::Changed(second) = paused.change(true) else {
        panic!("reactivate");
    };
    assert_eq!(second.producing(), Some(2));
    assert_eq!(Activation::restore(true, 0), None);
    assert_eq!(Activation::restore(false, -1), None);
    assert_eq!(
        Activation::restore(false, i64::MAX).unwrap().change(true),
        ActivationChange::Exhausted
    );
}

const NOW: i64 = 1_800_000_000_000_000;

fn held(attempts: u32, generation: i64) -> DeliveryFacts {
    DeliveryFacts {
        state: DeliveryState::Leased,
        attempts,
        generation: ClaimGeneration::restore(generation).unwrap(),
        available_at_micros: NOW + 10,
        deadline_from_micros: NOW - 1_000,
    }
}

#[test]
fn relay_results_are_recorded_only_by_the_current_claim() {
    let policy = OutboxPolicy::new(3, 30_000_000, 1_000_000, 10_000_000, 3_600_000_000).unwrap();
    let generation = |raw| ClaimGeneration::restore(raw).unwrap();
    let decide = |facts, held_gen, result| {
        decide_relay_completion(facts, generation(held_gen), result, policy, NOW, 0)
    };
    assert_eq!(
        decide(held(1, 2), 2, RelayResult::Ingested(IngestStatus::Applied)),
        RelayCompletion::Acked {
            already_applied: false
        }
    );
    assert_eq!(
        decide(
            held(1, 2),
            2,
            RelayResult::Ingested(IngestStatus::AlreadyApplied)
        ),
        RelayCompletion::Acked {
            already_applied: true
        }
    );
    // A late ACK from a reclaimed worker, or after the delivery finished, writes nothing.
    assert_eq!(
        decide(held(2, 3), 2, RelayResult::Ingested(IngestStatus::Applied)),
        RelayCompletion::Stale(StaleReason::Reclaimed)
    );
    let finished = DeliveryFacts {
        state: DeliveryState::Succeeded,
        ..held(1, 2)
    };
    assert_eq!(
        decide(
            finished,
            2,
            RelayResult::Ingested(IngestStatus::AlreadyApplied)
        ),
        RelayCompletion::Stale(StaleReason::NotLeased)
    );
    // Mismatch and invalid are never replays: dead letters for an operator.
    assert_eq!(
        decide(
            held(1, 2),
            2,
            RelayResult::Ingested(IngestStatus::PayloadMismatch)
        ),
        RelayCompletion::DeadLetter(RelayDeadReason::PayloadMismatch)
    );
    assert_eq!(
        decide(
            held(1, 2),
            2,
            RelayResult::Ingested(IngestStatus::RejectedInvalid)
        ),
        RelayCompletion::DeadLetter(RelayDeadReason::RejectedInvalid)
    );
    // Transient answers and transport failures back off, bounded by budget and deadline.
    for result in [
        RelayResult::Ingested(IngestStatus::TransientUnavailable),
        RelayResult::Failed(RelayFailure::Timeout),
        RelayResult::Failed(RelayFailure::Transport),
        RelayResult::Failed(RelayFailure::MissingResult),
    ] {
        assert!(matches!(
            decide(held(1, 2), 2, result),
            RelayCompletion::Retry { available_at_micros, .. } if available_at_micros == NOW + 1_000_000
        ));
        assert_eq!(
            decide(held(3, 4), 4, result),
            RelayCompletion::DeadLetter(RelayDeadReason::RetryExhausted)
        );
        let late = DeliveryFacts {
            deadline_from_micros: NOW - policy.deadline_micros() - 1,
            ..held(1, 2)
        };
        assert_eq!(
            decide(late, 2, result),
            RelayCompletion::DeadLetter(RelayDeadReason::DeadlineExceeded)
        );
    }
    assert_eq!(
        decide(
            held(1, 2),
            2,
            RelayResult::Failed(RelayFailure::InvalidStored)
        ),
        RelayCompletion::DeadLetter(RelayDeadReason::InvalidStored)
    );
}
