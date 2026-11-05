//! Endorsement use-case wiring with a recording store: authentication precedes validation, raw
//! input never reaches storage, the store receives exactly the pure command, and every outcome
//! the store reports keeps a distinct result. The store here returns what a real adapter would
//! get from `decide_endorsement` on its facts; transactions, locks and SQL are covered by the
//! real PostgreSQL suite (`tests/postgres/endorsement.rs`), not by this fake.

use std::sync::Mutex;

use uuid::Uuid;

use super::Vink;
use super::endorsement::*;
use super::ports::{
    CommentSummaryFacts, EndorsementStore, EndorsementSummaryFacts, EndorsementWriteRecord,
    OwnCommentFacts, OwnEndorsementFacts, StoreError,
};
use crate::domain::discussion::lifecycle::{CommentState, ModerationVersion, Review};
use crate::domain::discussion::root_placement;
use crate::domain::endorsement::{
    AcceptedOrder, EndorsementCommand, EndorsementCommandKey, EndorsementCount, EndorsementFacts,
    EndorsementIntent, EndorsementOutcome, EndorsementState, EndorsementTarget,
    EndorsementTargetFacts, StoredEndorsement, decide_endorsement,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, ThreadId, WorkId};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Set(PrincipalId, EndorsementCommand),
    Summaries(WorkId, Vec<CommentId>),
    Own(PrincipalId, WorkId, Vec<CommentId>),
}

/// Answers a write by running the real pure decision on scripted facts.
#[derive(Default)]
struct Script {
    calls: Mutex<Vec<Call>>,
    facts: Option<EndorsementFacts>,
    count: i64,
    published: Option<RevisionId>,
    summaries: Option<EndorsementSummaryFacts>,
    own: Option<OwnEndorsementFacts>,
    error: Option<StoreError>,
}

impl Script {
    fn record(&self, call: Call) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push(call);
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl EndorsementStore for Script {
    async fn set_endorsement(
        &self,
        actor: PrincipalId,
        command: EndorsementCommand,
    ) -> Result<EndorsementWriteRecord, StoreError> {
        self.record(Call::Set(actor, command))?;
        let facts = self.facts.expect("this use case must not reach the store");
        Ok(EndorsementWriteRecord {
            facts,
            outcome: decide_endorsement(actor, command, facts),
            count: EndorsementCount::new(self.count).unwrap(),
            published: self.published,
        })
    }

    async fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<EndorsementSummaryFacts, StoreError> {
        self.record(Call::Summaries(work_id, comments.ids().to_vec()))?;
        Ok(self.summaries.clone().expect("not scripted"))
    }

    async fn own_endorsements(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<OwnEndorsementFacts, StoreError> {
        self.record(Call::Own(actor, work_id, comments.ids().to_vec()))?;
        Ok(self.own.clone().expect("not scripted"))
    }
}

fn calls(vink: &Vink<Script>) -> Vec<Call> {
    vink.store().calls.lock().unwrap().clone()
}

const AUTHOR: u128 = 1;
const READER: u128 = 2;
const KEY: &str = "00000000-0000-0000-0000-0000000000aa";

fn principal(id: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(id))
}

fn reader() -> Option<PrincipalId> {
    Some(principal(READER))
}

fn work() -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(50))
}

fn revision(id: u128) -> RevisionId {
    RevisionId::from_uuid(Uuid::from_u128(id))
}

fn comment(id: u128) -> CommentId {
    CommentId::from_uuid(Uuid::from_u128(id))
}

fn thread() -> ThreadId {
    ThreadId::from_uuid(Uuid::from_u128(60))
}

fn key() -> EndorsementCommandKey {
    EndorsementCommandKey::from_uuid(Uuid::parse_str(KEY).unwrap())
}

fn state(selected: bool, version: i64, order: i64) -> EndorsementState {
    EndorsementState::restore(selected, version, order).unwrap()
}

fn count(raw: i64) -> Option<EndorsementCount> {
    EndorsementCount::new(raw)
}

/// Work 50 by principal 1, currently publishing revision `published`.
fn work_facts(published: Option<u128>) -> EndorsementTargetFacts {
    EndorsementTargetFacts::work(work(), principal(AUTHOR), published.map(revision))
}

/// Root comment `id` of work 50, written by principal 1.
fn comment_facts(id: u128, review: Review) -> EndorsementTargetFacts {
    EndorsementTargetFacts::comment(
        work(),
        thread(),
        root_placement(thread(), comment(id)),
        principal(AUTHOR),
        CommentState::restore(review, None, ModerationVersion::FIRST),
        true,
    )
    .unwrap()
}

fn write_facts(target: EndorsementTargetFacts) -> EndorsementFacts {
    EndorsementFacts {
        can_endorse: true,
        target: Some(target),
        current: None,
        receipt: None,
    }
}

fn set_work(expected_order: i64, endorsed: bool, revision: &str) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: work(),
        revision: revision.into(),
        command_key: KEY.into(),
        expected_order,
        endorsed,
    }
}

fn set_comment(id: u128, expected_order: i64, endorsed: bool) -> SetCommentEndorsement {
    SetCommentEndorsement {
        work_id: work(),
        comment_id: comment(id),
        command_key: KEY.into(),
        expected_order,
        endorsed,
    }
}

const R7: &str = "00000000-0000-0000-0000-000000000007";

#[tokio::test]
async fn authentication_precedes_validation_and_storage() {
    let vink = Vink::new(Script::default());
    assert_eq!(
        vink.set_work_endorsement(None, set_work(0, true, "x"))
            .await,
        Err(EndorsementError::Unauthenticated)
    );
    assert_eq!(
        vink.set_comment_endorsement(None, set_comment(9, 0, true))
            .await,
        Err(EndorsementError::Unauthenticated)
    );
    assert_eq!(
        vink.own_endorsements(None, work(), Some("not-an-id")).await,
        Err(EndorsementError::Unauthenticated),
        "a guest is never reported as an actor who has not endorsed"
    );
    assert!(calls(&vink).is_empty());
}

#[tokio::test]
async fn invalid_input_is_refused_before_storage_with_its_field() {
    let vink = Vink::new(Script::default());
    for (command, field) in [
        (set_work(0, true, R7), "expected_order"),
        (set_work(1, true, "bản 7"), "revision_id"),
        (
            SetWorkEndorsement {
                command_key: "k-1".into(),
                ..set_work(1, true, R7)
            },
            "command_key",
        ),
    ] {
        let Err(EndorsementError::Invalid(input)) =
            vink.set_work_endorsement(reader(), command).await
        else {
            panic!("{field} must be invalid");
        };
        assert_eq!(input.field(), field);
    }
    let too_many = (0..=MAX_BATCH_COMMENTS)
        .map(|n| comment(n as u128 + 100).to_string())
        .collect::<Vec<_>>()
        .join(",");
    for raw in [
        "x",
        "00000000-0000-0000-0000-000000000009,",
        ",",
        too_many.as_str(),
    ] {
        let Err(EndorsementError::Invalid(input)) =
            vink.endorsement_summaries(work(), Some(raw)).await
        else {
            panic!("{raw} must be invalid");
        };
        assert_eq!(input.field(), "comment_ids");
    }
    assert!(calls(&vink).is_empty());
}

#[test]
fn batches_are_bounded_and_deduplicated_in_first_seen_order() {
    let (a, b) = (comment(1).to_string(), comment(2).to_string());
    let batch = CommentBatch::parse(Some(&format!("{b},{a},{b},{a}"))).unwrap();
    assert_eq!(batch.ids(), &[comment(2), comment(1)]);
    assert_eq!(CommentBatch::parse(None).unwrap().ids(), &[]);
    let urns = (0..MAX_BATCH_COMMENTS)
        .map(|n| format!("urn:uuid:{}", comment(n as u128 + 100)))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        CommentBatch::parse(Some(&urns)).unwrap().ids().len(),
        MAX_BATCH_COMMENTS,
        "100 IDs in any accepted form fit"
    );
    let repeated = vec![a.as_str(); MAX_BATCH_COMMENTS + 1].join(",");
    assert_eq!(
        CommentBatch::parse(Some(&repeated)),
        Err(EndorsementInput::TooManyComments),
        "the bound counts requested items, duplicates included"
    );
}

#[tokio::test]
async fn the_store_receives_the_exact_command_and_the_server_actor() {
    let vink = Vink::new(Script {
        facts: Some(write_facts(work_facts(Some(7)))),
        count: 1,
        ..Script::default()
    });
    let outcome = vink
        .set_work_endorsement(reader(), set_work(1, true, R7))
        .await
        .unwrap();
    let target = EndorsementTarget::Work {
        work: work(),
        revision: revision(7),
    };
    assert_eq!(
        calls(&vink),
        [Call::Set(
            principal(READER),
            EndorsementCommand {
                key: key(),
                target,
                expected: AcceptedOrder::FIRST,
                intent: EndorsementIntent::Select,
            }
        )]
    );
    assert_eq!(
        outcome,
        EndorsementOutcomeView {
            target,
            result: EndorsementResult::Accepted(state(true, 2, 2)),
            summary: count(1),
        }
    );
}

#[tokio::test]
async fn replays_conflicts_and_refusals_keep_distinct_results() {
    // Replay: the store found this key's receipt; history and the current state both return.
    let root = comment_facts(9, Review::Approved);
    let first = decide_endorsement(
        principal(READER),
        EndorsementCommand {
            key: key(),
            target: EndorsementTarget::Comment {
                work: work(),
                comment: comment(9),
            },
            expected: AcceptedOrder::FIRST,
            intent: EndorsementIntent::Select,
        },
        write_facts(root),
    );
    let EndorsementOutcome::Accepted(write) = first else {
        panic!("{first:?}")
    };
    let later = state(false, 3, 3);
    let replayed = Vink::new(Script {
        facts: Some(EndorsementFacts {
            current: Some(StoredEndorsement::restore(
                principal(READER),
                write.next().target(),
                later,
            )),
            receipt: Some(write.receipt()),
            ..write_facts(root)
        }),
        count: 4,
        ..Script::default()
    })
    .set_comment_endorsement(reader(), set_comment(9, 1, true))
    .await
    .unwrap();
    assert_eq!(
        replayed.result,
        EndorsementResult::Replayed {
            committed: state(true, 2, 2),
            observed: later,
        },
        "a replay never reports the historical state as current"
    );
    assert_eq!(replayed.summary, count(4));

    // The same key with another payload is refused.
    let reused = Vink::new(Script {
        facts: Some(EndorsementFacts {
            current: Some(write.next()),
            receipt: Some(write.receipt()),
            ..write_facts(root)
        }),
        ..Script::default()
    })
    .set_comment_endorsement(reader(), set_comment(9, 2, false))
    .await;
    assert_eq!(reused, Err(EndorsementError::KeyReused));

    for (facts, published, actor, command, expected) in [
        // Another order than the current one: a conflict with the observed state.
        (
            write_facts(work_facts(Some(7))),
            Some(7),
            READER,
            set_work(3, true, R7),
            EndorsementError::Conflict {
                observed: EndorsementState::INITIAL,
            },
        ),
        (
            write_facts(work_facts(Some(7))),
            Some(7),
            AUTHOR,
            set_work(1, true, R7),
            EndorsementError::SelfEndorsement,
        ),
        (
            EndorsementFacts {
                can_endorse: false,
                ..write_facts(work_facts(Some(7)))
            },
            Some(7),
            READER,
            set_work(1, true, R7),
            EndorsementError::NotPermitted,
        ),
        // Selecting an earlier revision of a public work names the current one.
        (
            write_facts(work_facts(Some(8))),
            Some(8),
            READER,
            set_work(1, true, R7),
            EndorsementError::RevisionChanged {
                published: revision(8),
            },
        ),
        // Withdrawn: nothing to name.
        (
            write_facts(work_facts(None)),
            None,
            READER,
            set_work(1, true, R7),
            EndorsementError::NotFound,
        ),
        // Removing without a retained record of a hidden target is unavailable.
        (
            write_facts(work_facts(Some(8))),
            Some(8),
            READER,
            set_work(1, false, R7),
            EndorsementError::NotFound,
        ),
    ] {
        assert_eq!(
            Vink::new(Script {
                facts: Some(facts),
                published: published.map(revision),
                ..Script::default()
            })
            .set_work_endorsement(Some(principal(actor)), command.clone())
            .await,
            Err(expected),
            "{command:?}"
        );
    }

    for (error, expected) in [
        (
            StoreError::Unavailable("down".into()),
            EndorsementError::Unavailable,
        ),
        (
            StoreError::UnknownPrincipal,
            EndorsementError::Unauthenticated,
        ),
        (
            StoreError::Integrity("bad".into()),
            EndorsementError::Internal,
        ),
    ] {
        let vink = Vink::new(Script {
            error: Some(error),
            ..Script::default()
        });
        assert_eq!(
            vink.set_comment_endorsement(reader(), set_comment(9, 1, true))
                .await,
            Err(expected)
        );
        assert_eq!(
            vink.endorsement_summaries(work(), None).await,
            Err(expected),
            "an outage is never a zero count"
        );
    }
}

#[tokio::test]
async fn summaries_hide_unavailable_targets_and_never_turn_them_into_zero() {
    let facts = EndorsementSummaryFacts {
        work: Some(work_facts(Some(7))),
        published: Some(revision(7)),
        work_count: EndorsementCount::ZERO,
        comments: vec![
            CommentSummaryFacts {
                comment_id: comment(1),
                facts: comment_facts(1, Review::Approved),
                count: EndorsementCount::new(4).unwrap(),
            },
            CommentSummaryFacts {
                comment_id: comment(2),
                facts: comment_facts(2, Review::Pending),
                count: EndorsementCount::new(9).unwrap(),
            },
        ],
    };
    let vink = Vink::new(Script {
        summaries: Some(facts.clone()),
        ..Script::default()
    });
    let raw = [comment(3), comment(2), comment(1), comment(3)]
        .map(|id| id.to_string())
        .join(",");
    let summaries = vink
        .endorsement_summaries(work(), Some(&raw))
        .await
        .unwrap();
    assert_eq!(
        calls(&vink),
        [Call::Summaries(
            work(),
            vec![comment(3), comment(2), comment(1)]
        )]
    );
    assert_eq!(
        (summaries.revision, summaries.work),
        (revision(7), EndorsementCount::ZERO),
        "zero after a successful read"
    );
    assert_eq!(
        summaries
            .comments
            .iter()
            .map(|entry| (entry.comment_id, entry.summary))
            .collect::<Vec<_>>(),
        [
            (comment(3), None),
            (comment(2), None),
            (comment(1), count(4))
        ]
    );
    let withdrawn = Vink::new(Script {
        summaries: Some(EndorsementSummaryFacts {
            work: Some(work_facts(None)),
            published: None,
            ..facts
        }),
        ..Script::default()
    });
    assert_eq!(
        withdrawn.endorsement_summaries(work(), None).await,
        Err(EndorsementError::NotFound)
    );
}

#[tokio::test]
async fn own_state_is_actor_scoped_with_tokens_only_for_visible_targets() {
    let facts = OwnEndorsementFacts {
        can_endorse: true,
        work: Some(work_facts(Some(7))),
        published: Some(revision(7)),
        work_state: Some(state(true, 2, 3)),
        comments: vec![
            OwnCommentFacts {
                comment_id: comment(1),
                facts: comment_facts(1, Review::Approved),
                state: None,
            },
            OwnCommentFacts {
                comment_id: comment(2),
                facts: comment_facts(2, Review::Rejected),
                state: Some(state(true, 2, 2)),
            },
        ],
    };
    let vink = Vink::new(Script {
        own: Some(facts.clone()),
        ..Script::default()
    });
    let raw = [comment(1), comment(2), comment(3)]
        .map(|id| id.to_string())
        .join(",");
    let own = vink
        .own_endorsements(reader(), work(), Some(&raw))
        .await
        .unwrap();
    assert_eq!(
        calls(&vink),
        [Call::Own(
            principal(READER),
            work(),
            vec![comment(1), comment(2), comment(3)]
        )]
    );
    assert_eq!(
        own.work,
        OwnEndorsement {
            state: Some(state(true, 2, 3)),
            capability: Capability::Allowed,
        }
    );
    assert_eq!(
        own.comments.iter().map(|c| c.own).collect::<Vec<_>>(),
        [
            OwnEndorsement {
                state: Some(EndorsementState::INITIAL),
                capability: Capability::Allowed,
            },
            // A rejected comment exposes no stored state, not even its order.
            OwnEndorsement::UNAVAILABLE,
            OwnEndorsement::UNAVAILABLE,
        ]
    );
    let author = Vink::new(Script {
        own: Some(OwnEndorsementFacts {
            work_state: None,
            ..facts.clone()
        }),
        ..Script::default()
    })
    .own_endorsements(Some(principal(AUTHOR)), work(), None)
    .await
    .unwrap();
    assert_eq!(
        author.work,
        OwnEndorsement {
            state: Some(EndorsementState::INITIAL),
            capability: Capability::OwnContent,
        }
    );
    let barred = Vink::new(Script {
        own: Some(OwnEndorsementFacts {
            can_endorse: false,
            ..facts.clone()
        }),
        ..Script::default()
    })
    .own_endorsements(reader(), work(), None)
    .await
    .unwrap();
    assert_eq!(barred.work.capability, Capability::NotPermitted);
    let withdrawn = Vink::new(Script {
        own: Some(OwnEndorsementFacts {
            work: Some(work_facts(None)),
            published: None,
            ..facts
        }),
        ..Script::default()
    });
    assert_eq!(
        withdrawn.own_endorsements(reader(), work(), None).await,
        Err(EndorsementError::NotFound)
    );
}
