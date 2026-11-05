//! Endorsing a work ("Đánh giá cao", LVE-FR-01/04, LVE-UC-02/10/11).
//!
//! A work endorsement is anchored to the revision the reader read (CHOICE-02 C). The request
//! carries that observed revision; the decision accepts a selection only when it is still the
//! current publication, so a publication that changed between reading and clicking never
//! receives the endorsement silently ([`WorkDecision::RevisionChanged`]). Publishing a new
//! revision writes no endorsement: earlier selections stay stored and become
//! [`OwnView::EndorsedEarlierRevision`], uncounted, until the reader endorses the new text.
//! Republishing the same revision makes them effective again: it is the same text.
//! The anchor is the immutable revision identity, never the publication generation (which a
//! withdraw/republish may restart) or a moderation version.

use super::{Endorsement, EndorsementCount, EndorsementVersion, OwnView, Summary, Transition};
use crate::domain::ids::{PrincipalId, RevisionId, WorkId};

/// Facts the adapter reads in the write transaction (publication row locked FOR SHARE).
/// `work` is the identity of the row it loaded, never copied from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkFacts {
    pub work: WorkId,
    /// `works.owner_id`.
    pub author: PrincipalId,
    /// The current publication's revision; `None` when the work has no public publication
    /// (private, never published or withdrawn).
    pub published: Option<RevisionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkIntent {
    /// Endorse the text the reader read: `observed` is the revision their page rendered.
    Endorse { observed: RevisionId },
    /// Remove the actor's endorsement, whatever revision it selects.
    Clear,
}

/// A desired-state request; the actor and the work come from the server and the route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkRequest {
    pub expected: EndorsementVersion,
    pub intent: WorkIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDecision {
    /// Eligible: the version decides (see [`Endorsement::transition`]).
    Transition(Transition<RevisionId>),
    /// Missing, foreign or not public: indistinguishable on purpose.
    TargetUnavailable,
    /// Authors do not endorse their own works (CHOICE-08 A).
    OwnWork,
    /// The reader endorsed a revision that is no longer the public one. Nothing is written;
    /// the client shows the current text before the reader may endorse it.
    RevisionChanged { published: RevisionId },
}

/// The one production decision for setting or clearing a work endorsement. `current` is the
/// actor's stored endorsement of this work, or [`Endorsement::ABSENT`].
pub fn decide_work_endorsement(
    actor: PrincipalId,
    work: WorkId,
    request: WorkRequest,
    facts: Option<WorkFacts>,
    current: Endorsement<RevisionId>,
) -> WorkDecision {
    let Some((facts, published)) = public(work, facts) else {
        return WorkDecision::TargetUnavailable;
    };
    if actor == facts.author {
        return WorkDecision::OwnWork;
    }
    let desired = match request.intent {
        WorkIntent::Endorse { observed } if observed != published => {
            return WorkDecision::RevisionChanged { published };
        }
        WorkIntent::Endorse { .. } => Some(published),
        WorkIntent::Clear => None,
    };
    WorkDecision::Transition(current.transition(request.expected, desired))
}

/// The actor's own state, shown only to them.
pub fn own_work_view(
    work: WorkId,
    facts: Option<WorkFacts>,
    current: Endorsement<RevisionId>,
) -> OwnView {
    match (public(work, facts), current.selection()) {
        (None, _) => OwnView::Unavailable,
        (Some(_), None) => OwnView::NotEndorsed,
        (Some((_, published)), Some(anchor)) if anchor == published => OwnView::Endorsed,
        (Some(_), Some(_)) => OwnView::EndorsedEarlierRevision,
    }
}

/// The public aggregate: `published` is the count of selections at the current publication's
/// revision; selections of earlier revisions are not part of it.
pub fn work_summary(
    work: WorkId,
    facts: Option<WorkFacts>,
    published: EndorsementCount,
) -> Summary {
    match public(work, facts) {
        Some(_) => Summary::Count(published),
        None => Summary::Unavailable,
    }
}

fn public(work: WorkId, facts: Option<WorkFacts>) -> Option<(WorkFacts, RevisionId)> {
    let facts = facts.filter(|facts| facts.work == work)?;
    facts.published.map(|published| (facts, published))
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn principal(id: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(id))
    }

    fn revision(id: u128) -> RevisionId {
        RevisionId::from_uuid(Uuid::from_u128(id))
    }

    fn version(raw: i64) -> EndorsementVersion {
        EndorsementVersion::new(raw).unwrap()
    }

    const WORK: u128 = 50;

    fn work() -> WorkId {
        WorkId::from_uuid(Uuid::from_u128(WORK))
    }

    /// Work 50 by principal 1, publishing revision `published`.
    fn facts(published: Option<u128>) -> Option<WorkFacts> {
        Some(WorkFacts {
            work: work(),
            author: principal(1),
            published: published.map(revision),
        })
    }

    fn endorse(expected: i64, observed: u128) -> WorkRequest {
        WorkRequest {
            expected: version(expected),
            intent: WorkIntent::Endorse {
                observed: revision(observed),
            },
        }
    }

    fn clear(expected: i64) -> WorkRequest {
        WorkRequest {
            expected: version(expected),
            intent: WorkIntent::Clear,
        }
    }

    fn written(decision: WorkDecision) -> Endorsement<RevisionId> {
        match decision {
            WorkDecision::Transition(transition) => transition.write().unwrap(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_reader_endorses_the_revision_they_read() {
        let next = written(decide_work_endorsement(
            principal(2),
            work(),
            endorse(1, 7),
            facts(Some(7)),
            Endorsement::ABSENT,
        ));
        assert_eq!(next, Endorsement::restore(Some(revision(7)), version(2)));
        assert_eq!(
            own_work_view(work(), facts(Some(7)), next),
            OwnView::Endorsed
        );
    }

    #[test]
    fn a_publication_that_changed_after_reading_never_receives_the_endorsement() {
        // The reader saw revision 7; revision 8 was published before the request committed.
        assert_eq!(
            decide_work_endorsement(
                principal(2),
                work(),
                endorse(1, 7),
                facts(Some(8)),
                Endorsement::ABSENT
            ),
            WorkDecision::RevisionChanged {
                published: revision(8)
            }
        );
    }

    #[test]
    fn a_new_publication_leaves_earlier_endorsements_uncounted_until_reconfirmed() {
        let on_seven = Endorsement::restore(Some(revision(7)), version(2));
        assert_eq!(
            own_work_view(work(), facts(Some(8)), on_seven),
            OwnView::EndorsedEarlierRevision
        );
        // Retrying the old request is not a reconfirmation.
        assert_eq!(
            decide_work_endorsement(
                principal(2),
                work(),
                endorse(2, 7),
                facts(Some(8)),
                on_seven
            ),
            WorkDecision::RevisionChanged {
                published: revision(8)
            }
        );
        let WorkDecision::Transition(Transition::Changed { next, delta }) = decide_work_endorsement(
            principal(2),
            work(),
            endorse(2, 8),
            facts(Some(8)),
            on_seven,
        ) else {
            panic!("reconfirming the new text is a change");
        };
        assert_eq!(next.selection(), Some(revision(8)));
        assert_eq!(
            (delta.removed(), delta.added()),
            (Some(revision(7)), Some(revision(8)))
        );
        // Republishing revision 7 makes the old selection effective again: same text.
        assert_eq!(
            own_work_view(work(), facts(Some(7)), on_seven),
            OwnView::Endorsed
        );
        // Clearing never needs the current revision.
        let cleared = written(decide_work_endorsement(
            principal(2),
            work(),
            clear(2),
            facts(Some(8)),
            on_seven,
        ));
        assert_eq!(cleared, Endorsement::restore(None, version(3)));
    }

    #[test]
    fn hidden_missing_or_foreign_works_refuse_everything_and_show_nothing() {
        let on = Endorsement::restore(Some(revision(7)), version(2));
        let other = WorkId::from_uuid(Uuid::from_u128(WORK + 1));
        let foreign = Some(WorkFacts {
            work: other,
            ..facts(Some(7)).unwrap()
        });
        for unavailable in [facts(None), None, foreign] {
            for request in [endorse(2, 7), clear(2)] {
                assert_eq!(
                    decide_work_endorsement(principal(2), work(), request, unavailable, on),
                    WorkDecision::TargetUnavailable,
                    "{unavailable:?} {request:?}"
                );
            }
            assert_eq!(own_work_view(work(), unavailable, on), OwnView::Unavailable);
            assert_eq!(
                work_summary(work(), unavailable, EndorsementCount::restore(4)),
                Summary::Unavailable,
                "unavailable is not zero"
            );
        }
        assert_eq!(
            work_summary(work(), facts(Some(7)), EndorsementCount::ZERO),
            Summary::Count(EndorsementCount::ZERO)
        );
    }

    #[test]
    fn authors_cannot_endorse_or_touch_their_own_work() {
        for request in [endorse(1, 7), clear(1)] {
            assert_eq!(
                decide_work_endorsement(
                    principal(1),
                    work(),
                    request,
                    facts(Some(7)),
                    Endorsement::ABSENT
                ),
                WorkDecision::OwnWork
            );
        }
    }
}
