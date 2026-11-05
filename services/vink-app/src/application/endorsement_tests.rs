//! Endorsement use-case wiring with a recording store: authentication precedes validation, raw
//! input never reaches storage, the store receives exactly the pure request, and every decision
//! the store reports keeps a distinct outcome. Transactions, locks and SQL are covered by the
//! real PostgreSQL suite (`tests/postgres/endorsement.rs`), not by this fake.

use std::sync::Mutex;

use uuid::Uuid;

use super::Vink;
use super::endorsement::*;
use super::ports::{
    CommentEndorsementWrite, CommentSummaryFacts, EndorsementStore, EndorsementSummaryFacts,
    OwnCommentFacts, OwnEndorsementFacts, StoreError, WorkEndorsementWrite,
};
use crate::domain::discussion::lifecycle::{CommentState, ModerationVersion, Review};
use crate::domain::discussion::settings::{CommentsMode, DiscussionSettings, SettingsGeneration};
use crate::domain::endorsement::comment::{
    CommentDecision, CommentFacts, CommentIntent, CommentRequest,
};
use crate::domain::endorsement::work::{WorkDecision, WorkFacts, WorkIntent, WorkRequest};
use crate::domain::endorsement::{
    CommentAnchor, Endorsement, EndorsementCount, EndorsementVersion, OwnView, Summary, Transition,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Work(PrincipalId, WorkId, WorkRequest),
    Comment(PrincipalId, WorkId, CommentId, CommentRequest),
    Summaries(WorkId, Vec<CommentId>),
    Own(PrincipalId, WorkId, Vec<CommentId>),
}

#[derive(Default)]
struct Script {
    calls: Mutex<Vec<Call>>,
    work: Option<WorkEndorsementWrite>,
    comment: Option<CommentEndorsementWrite>,
    summaries: Option<EndorsementSummaryFacts>,
    own: Option<OwnEndorsementFacts>,
    error: Option<StoreError>,
}

impl Script {
    fn record<T>(&self, call: Call, answer: Option<T>) -> Result<T, StoreError> {
        self.calls.lock().unwrap().push(call);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(answer.expect("this use case must not reach the store"))
    }
}

impl EndorsementStore for Script {
    async fn set_work_endorsement(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        request: WorkRequest,
    ) -> Result<WorkEndorsementWrite, StoreError> {
        self.record(Call::Work(actor, work_id, request), self.work)
    }

    async fn set_comment_endorsement(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comment_id: CommentId,
        request: CommentRequest,
    ) -> Result<CommentEndorsementWrite, StoreError> {
        self.record(
            Call::Comment(actor, work_id, comment_id, request),
            self.comment,
        )
    }

    async fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<EndorsementSummaryFacts, StoreError> {
        self.record(
            Call::Summaries(work_id, comments.ids().to_vec()),
            self.summaries.clone(),
        )
    }

    async fn own_endorsements(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> Result<OwnEndorsementFacts, StoreError> {
        self.record(
            Call::Own(actor, work_id, comments.ids().to_vec()),
            self.own.clone(),
        )
    }
}

fn vink(script: Script) -> Vink<Script> {
    Vink::new(script)
}

fn calls(vink: &Vink<Script>) -> Vec<Call> {
    vink.store().calls.lock().unwrap().clone()
}

const AUTHOR: u128 = 1;
const READER: u128 = 2;

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

fn version(raw: i64) -> EndorsementVersion {
    EndorsementVersion::new(raw).unwrap()
}

fn count(raw: u32) -> EndorsementCount {
    EndorsementCount::restore(raw)
}

/// Work 50 by principal 1, currently publishing revision `published`.
fn work_facts(published: Option<u128>) -> Option<WorkFacts> {
    Some(WorkFacts {
        work: work(),
        author: principal(AUTHOR),
        published: published.map(revision),
    })
}

fn state(review: Review) -> CommentState {
    CommentState::restore(review, None, ModerationVersion::FIRST)
}

fn settings(mode: CommentsMode) -> DiscussionSettings {
    DiscussionSettings::restore(mode, SettingsGeneration::new(1).unwrap())
}

/// Comment `id` of work 50, written by principal 1.
fn comment_facts(id: u128, review: Review, mode: CommentsMode) -> CommentFacts {
    CommentFacts {
        work: work(),
        comment: comment(id),
        work_public: true,
        writer: principal(AUTHOR),
        state: state(review),
        settings: settings(mode),
    }
}

fn set_work(expected_version: i64, endorsed: bool, observed: Option<&str>) -> SetWorkEndorsement {
    SetWorkEndorsement {
        work_id: work(),
        expected_version,
        endorsed,
        observed_revision: observed.map(str::to_owned),
    }
}

fn set_comment(id: u128, expected_version: i64, endorsed: bool) -> SetCommentEndorsement {
    SetCommentEndorsement {
        work_id: work(),
        comment_id: comment(id),
        expected_version,
        endorsed,
    }
}

const R7: &str = "00000000-0000-0000-0000-000000000007";

fn work_write(decision: WorkDecision, published: Option<u128>, n: u32) -> Script {
    Script {
        work: Some(WorkEndorsementWrite {
            facts: work_facts(published),
            decision,
            count: count(n),
        }),
        ..Script::default()
    }
}

#[tokio::test]
async fn authentication_precedes_validation_and_storage() {
    let vink = vink(Script::default());
    assert_eq!(
        vink.set_work_endorsement(None, set_work(0, true, Some("x")))
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
    let vink = vink(Script::default());
    for (command, field) in [
        (set_work(0, true, Some(R7)), "expected_version"),
        (set_work(1, true, None), "observed_revision_id"),
        (set_work(1, true, Some("bản 7")), "observed_revision_id"),
        (set_work(1, false, Some(R7)), "observed_revision_id"),
    ] {
        let Err(EndorsementError::Invalid(input)) =
            vink.set_work_endorsement(reader(), command).await
        else {
            panic!("{field} must be invalid");
        };
        assert_eq!(input.field(), field);
    }
    assert_eq!(
        vink.set_comment_endorsement(reader(), set_comment(9, -1, false))
            .await,
        Err(EndorsementError::Invalid(EndorsementInput::ExpectedVersion))
    );
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
        assert_eq!(
            vink.own_endorsements(reader(), work(), Some(raw))
                .await
                .err()
                .map(|e| matches!(e, EndorsementError::Invalid(_))),
            Some(true)
        );
    }
    assert!(calls(&vink).is_empty());
}

#[test]
fn batches_are_bounded_and_deduplicated_in_first_seen_order() {
    let (a, b) = (comment(1).to_string(), comment(2).to_string());
    let batch = CommentBatch::parse(Some(&format!("{b},{a},{b},{a}"))).unwrap();
    assert_eq!(batch.ids(), &[comment(2), comment(1)]);
    assert_eq!(CommentBatch::parse(None).unwrap().ids(), &[]);
    assert_eq!(CommentBatch::parse(Some("")).unwrap().ids(), &[]);
    let full = (0..MAX_BATCH_COMMENTS)
        .map(|n| comment(n as u128 + 100).to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        CommentBatch::parse(Some(&full)).unwrap().ids().len(),
        MAX_BATCH_COMMENTS
    );
    let urns = (0..MAX_BATCH_COMMENTS)
        .map(|n| format!("urn:uuid:{}", comment(n as u128 + 100)))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        CommentBatch::parse(Some(&urns)).unwrap().ids().len(),
        MAX_BATCH_COMMENTS,
        "100 IDs in any accepted form fit"
    );
    // The bound counts requested items, duplicates included.
    let repeated = vec![a.as_str(); MAX_BATCH_COMMENTS + 1].join(",");
    assert_eq!(
        CommentBatch::parse(Some(&repeated)),
        Err(EndorsementInput::TooManyComments)
    );
}

#[tokio::test]
async fn the_store_receives_the_exact_pure_request_and_the_server_actor() {
    let accepted = WorkDecision::Transition(Transition::Changed {
        next: Endorsement::restore(Some(revision(7)), version(2)),
        delta: changed_delta(),
    });
    let endorsed = vink(work_write(accepted, Some(7), 1));
    endorsed
        .set_work_endorsement(reader(), set_work(1, true, Some(R7)))
        .await
        .unwrap();
    let cleared = vink(Script {
        comment: Some(CommentEndorsementWrite {
            facts: Some(comment_facts(9, Review::Approved, CommentsMode::Open)),
            decision: CommentDecision::Transition(Transition::Reaffirmed {
                next: Endorsement::restore(None, version(4)),
            }),
            count: count(0),
        }),
        ..Script::default()
    });
    cleared
        .set_comment_endorsement(reader(), set_comment(9, 3, false))
        .await
        .unwrap();
    assert_eq!(
        calls(&endorsed),
        [Call::Work(
            principal(READER),
            work(),
            WorkRequest {
                expected: version(1),
                intent: WorkIntent::Endorse {
                    observed: revision(7)
                },
            }
        )]
    );
    assert_eq!(
        calls(&cleared),
        [Call::Comment(
            principal(READER),
            work(),
            comment(9),
            CommentRequest {
                expected: version(3),
                intent: CommentIntent::Clear,
            }
        )]
    );
}

/// A real delta for `Changed` (its fields are private to the core).
fn changed_delta() -> crate::domain::endorsement::EndorsementDelta<RevisionId> {
    let Transition::Changed { delta, .. } =
        Endorsement::<RevisionId>::ABSENT.transition(version(1), Some(revision(7)))
    else {
        unreachable!()
    };
    delta
}

#[tokio::test]
async fn accepted_and_already_current_are_distinct_successes_with_the_committed_state() {
    let on = Endorsement::restore(Some(revision(7)), version(2));
    for (decision, accepted) in [
        (
            WorkDecision::Transition(Transition::Changed {
                next: on,
                delta: changed_delta(),
            }),
            true,
        ),
        (
            WorkDecision::Transition(Transition::Reaffirmed { next: on }),
            true,
        ),
        (
            WorkDecision::Transition(Transition::AlreadyCurrent(on)),
            false,
        ),
    ] {
        let outcome = vink(work_write(decision, Some(7), 3))
            .set_work_endorsement(reader(), set_work(1, true, Some(R7)))
            .await
            .unwrap();
        assert_eq!(
            outcome,
            EndorsementOutcome {
                work_id: work(),
                comment_id: None,
                accepted,
                own: OwnEndorsement {
                    view: OwnView::Endorsed,
                    version: Some(version(2)),
                    revision: Some(revision(7)),
                    capability: Capability::Allowed,
                },
                summary: Summary::Count(count(3)),
            },
            "{decision:?}"
        );
    }
}

#[tokio::test]
async fn refusals_and_conflicts_keep_distinct_errors() {
    let earlier = Endorsement::restore(Some(revision(6)), version(5));
    for (decision, expected) in [
        (WorkDecision::TargetUnavailable, EndorsementError::NotFound),
        (WorkDecision::OwnWork, EndorsementError::SelfEndorsement),
        (
            WorkDecision::RevisionChanged {
                published: revision(7),
            },
            EndorsementError::RevisionChanged {
                published: revision(7),
            },
        ),
        (
            WorkDecision::Transition(Transition::Stale { current: earlier }),
            // The stored selection is of an earlier revision: shown as such, with its version.
            EndorsementError::Conflict {
                current: OwnEndorsement {
                    view: OwnView::EndorsedEarlierRevision,
                    version: Some(version(5)),
                    revision: Some(revision(6)),
                    capability: Capability::Allowed,
                },
            },
        ),
        (
            WorkDecision::Transition(Transition::VersionExhausted),
            EndorsementError::Internal,
        ),
    ] {
        assert_eq!(
            vink(work_write(decision, Some(7), 0))
                .set_work_endorsement(reader(), set_work(1, true, Some(R7)))
                .await,
            Err(expected),
            "{decision:?}"
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
        let vink = vink(Script {
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
async fn comment_outcomes_report_closed_comments_and_advisory_capabilities() {
    let closed = comment_facts(9, Review::Approved, CommentsMode::Closed);
    let answer = |decision| Script {
        comment: Some(CommentEndorsementWrite {
            facts: Some(closed),
            decision,
            count: count(2),
        }),
        ..Script::default()
    };
    assert_eq!(
        vink(answer(CommentDecision::CommentsClosed))
            .set_comment_endorsement(reader(), set_comment(9, 1, true))
            .await,
        Err(EndorsementError::CommentsClosed)
    );
    assert_eq!(
        vink(answer(CommentDecision::OwnComment))
            .set_comment_endorsement(Some(principal(AUTHOR)), set_comment(9, 1, true))
            .await,
        Err(EndorsementError::SelfEndorsement)
    );
    // Clearing while comments are closed is accepted; endorsing again is then blocked.
    let cleared = vink(answer(CommentDecision::Transition(Transition::Changed {
        next: Endorsement::restore(None, version(3)),
        delta: comment_delta(),
    })))
    .set_comment_endorsement(reader(), set_comment(9, 2, false))
    .await
    .unwrap();
    assert!(cleared.accepted);
    assert_eq!(cleared.comment_id, Some(comment(9)));
    assert_eq!(
        cleared.own,
        OwnEndorsement {
            view: OwnView::NotEndorsed,
            version: Some(version(3)),
            revision: None,
            capability: Capability::CommentsClosed,
        }
    );
    assert_eq!(cleared.summary, Summary::Count(count(2)));
}

fn comment_delta() -> crate::domain::endorsement::EndorsementDelta<CommentAnchor> {
    let on = Endorsement::restore(Some(CommentAnchor), version(2));
    let Transition::Changed { delta, .. } = on.transition(version(2), None) else {
        unreachable!()
    };
    delta
}

#[tokio::test]
async fn summaries_hide_unavailable_targets_and_never_turn_them_into_zero() {
    let public = comment_facts(1, Review::Approved, CommentsMode::Open);
    let pending = comment_facts(2, Review::Pending, CommentsMode::Open);
    let facts = EndorsementSummaryFacts {
        work: work_facts(Some(7)),
        work_count: count(0),
        comments: vec![
            CommentSummaryFacts {
                facts: public,
                count: count(4),
            },
            CommentSummaryFacts {
                facts: pending,
                count: count(9),
            },
        ],
    };
    let vink = vink(Script {
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
    assert_eq!(summaries.revision, revision(7));
    assert_eq!(
        summaries.work,
        Summary::Count(count(0)),
        "zero after a successful read"
    );
    assert_eq!(
        summaries
            .comments
            .iter()
            .map(|entry| (entry.comment_id, entry.summary))
            .collect::<Vec<_>>(),
        [
            (comment(3), Summary::Unavailable),
            (comment(2), Summary::Unavailable),
            (comment(1), Summary::Count(count(4))),
        ]
    );
    for work in [work_facts(None), None] {
        let vink = self::vink(Script {
            summaries: Some(EndorsementSummaryFacts {
                work,
                ..facts.clone()
            }),
            ..Script::default()
        });
        assert_eq!(
            vink.endorsement_summaries(self::work(), None).await,
            Err(EndorsementError::NotFound)
        );
    }
}

#[tokio::test]
async fn own_state_is_actor_scoped_with_versions_only_for_available_targets() {
    let on_six = Endorsement::restore(Some(revision(6)), version(3));
    let facts = OwnEndorsementFacts {
        work: work_facts(Some(7)),
        endorsement: on_six,
        comments: vec![
            OwnCommentFacts {
                facts: comment_facts(1, Review::Approved, CommentsMode::Closed),
                endorsement: Endorsement::restore(Some(CommentAnchor), version(2)),
            },
            OwnCommentFacts {
                facts: comment_facts(2, Review::Rejected, CommentsMode::Open),
                endorsement: Endorsement::restore(Some(CommentAnchor), version(4)),
            },
        ],
    };
    let vink = vink(Script {
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
            view: OwnView::EndorsedEarlierRevision,
            version: Some(version(3)),
            revision: Some(revision(6)),
            capability: Capability::Allowed,
        }
    );
    assert_eq!(
        own.comments.iter().map(|c| c.own).collect::<Vec<_>>(),
        [
            // Reaffirming an existing selection stays possible while comments are closed.
            OwnEndorsement {
                view: OwnView::Endorsed,
                version: Some(version(2)),
                revision: None,
                capability: Capability::Allowed,
            },
            // A rejected comment exposes no stored state, not even its version.
            OwnEndorsement::UNAVAILABLE,
            OwnEndorsement::UNAVAILABLE,
        ]
    );

    // The author sees their own content as not endorsable, never as a refusal at click time.
    let author = self::vink(Script {
        own: Some(OwnEndorsementFacts {
            endorsement: Endorsement::ABSENT,
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
            view: OwnView::NotEndorsed,
            version: Some(EndorsementVersion::FIRST),
            revision: None,
            capability: Capability::OwnContent,
        }
    );
    let withdrawn = self::vink(Script {
        own: Some(OwnEndorsementFacts {
            work: work_facts(None),
            ..facts
        }),
        ..Script::default()
    });
    assert_eq!(
        withdrawn.own_endorsements(reader(), work(), None).await,
        Err(EndorsementError::NotFound)
    );
}
