//! Endorsing a root comment or a flat reply ("Hữu ích", LVE-FR-01, LVE-UC-03/11/12).
//!
//! Roots and replies follow the same rule: the endorsement belongs to that comment only, never
//! cascades to its root or to the work, and creates no comment, so the two tiers are untouched.
//! A reply under a hidden root stays endorsable while the reply itself is public (the root shows
//! as an anchor without text). Closing comments (COM-07) stops new comment endorsements but
//! never clearing or reaffirming one (CHOICE-09 B); it hides nothing and the work's own
//! endorsement is not governed by it. Reopening restores endorsing without any data change.

use super::{
    CommentAnchor, Endorsement, EndorsementCount, EndorsementVersion, OwnView, Summary, Transition,
};
use crate::domain::discussion::lifecycle::CommentState;
use crate::domain::discussion::settings::DiscussionSettings;
use crate::domain::ids::{CommentId, PrincipalId, WorkId};

/// Facts the adapter reads in the write transaction, under the discussion lock order
/// (publication → thread → comment). `work` and `comment` are the identities of the rows it
/// loaded (the comment's thread's work), never copied from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentFacts {
    pub work: WorkId,
    pub comment: CommentId,
    /// The work has a current public publication.
    pub work_public: bool,
    pub writer: PrincipalId,
    pub state: CommentState,
    pub settings: DiscussionSettings,
}

impl CommentFacts {
    /// Public to every reader: an approved, present comment of a public work.
    pub fn visible(&self) -> bool {
        self.work_public && self.state.is_public()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentIntent {
    Endorse,
    Clear,
}

/// A desired-state request; the actor and the target come from the server and the route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentRequest {
    pub expected: EndorsementVersion,
    pub intent: CommentIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentDecision {
    /// Eligible: the version decides (see [`Endorsement::transition`]).
    Transition(Transition<CommentAnchor>),
    /// Missing, in another work's thread, or not public: indistinguishable on purpose.
    TargetUnavailable,
    /// Writers do not endorse their own comments (CHOICE-08 A).
    OwnComment,
    /// The work's author closed comments: no new endorsement; clearing stays possible.
    CommentsClosed,
}

/// The one production decision for setting or clearing a comment endorsement. `current` is
/// the actor's stored endorsement of this comment, or [`Endorsement::ABSENT`].
pub fn decide_comment_endorsement(
    actor: PrincipalId,
    work: WorkId,
    comment: CommentId,
    request: CommentRequest,
    facts: Option<CommentFacts>,
    current: Endorsement<CommentAnchor>,
) -> CommentDecision {
    let Some(facts) = visible(work, comment, facts) else {
        return CommentDecision::TargetUnavailable;
    };
    if actor == facts.writer {
        return CommentDecision::OwnComment;
    }
    let desired = match request.intent {
        CommentIntent::Endorse => Some(CommentAnchor),
        CommentIntent::Clear => None,
    };
    if desired.is_some() && desired != current.selection() && !facts.settings.accepts_new_comments()
    {
        return CommentDecision::CommentsClosed;
    }
    CommentDecision::Transition(current.transition(request.expected, desired))
}

/// The actor's own state, shown only to them.
pub fn own_comment_view(
    work: WorkId,
    comment: CommentId,
    facts: Option<CommentFacts>,
    current: Endorsement<CommentAnchor>,
) -> OwnView {
    match (visible(work, comment, facts), current.selection()) {
        (None, _) => OwnView::Unavailable,
        (Some(_), None) => OwnView::NotEndorsed,
        (Some(_), Some(CommentAnchor)) => OwnView::Endorsed,
    }
}

/// The public aggregate of one comment; a hidden comment exposes no count.
pub fn comment_summary(
    work: WorkId,
    comment: CommentId,
    facts: Option<CommentFacts>,
    count: EndorsementCount,
) -> Summary {
    match visible(work, comment, facts) {
        Some(_) => Summary::Count(count),
        None => Summary::Unavailable,
    }
}

fn visible(work: WorkId, comment: CommentId, facts: Option<CommentFacts>) -> Option<CommentFacts> {
    facts.filter(|facts| facts.work == work && facts.comment == comment && facts.visible())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::discussion::lifecycle::{ModerationVersion, Removal, Review};
    use crate::domain::discussion::settings::{CommentsMode, SettingsGeneration};

    fn principal(id: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(id))
    }

    fn version(raw: i64) -> EndorsementVersion {
        EndorsementVersion::new(raw).unwrap()
    }

    fn work(id: u128) -> WorkId {
        WorkId::from_uuid(Uuid::from_u128(id))
    }

    fn comment(id: u128) -> CommentId {
        CommentId::from_uuid(Uuid::from_u128(id))
    }

    fn public() -> CommentState {
        CommentState::restore(Review::Approved, None, ModerationVersion::FIRST)
    }

    fn closed() -> DiscussionSettings {
        DiscussionSettings::restore(CommentsMode::Closed, SettingsGeneration::new(2).unwrap())
    }

    /// Comment 9 by principal 2 in work 50's thread, public and open.
    fn facts() -> CommentFacts {
        CommentFacts {
            work: work(50),
            comment: comment(9),
            work_public: true,
            writer: principal(2),
            state: public(),
            settings: DiscussionSettings::INITIAL,
        }
    }

    fn request(expected: i64, intent: CommentIntent) -> CommentRequest {
        CommentRequest {
            expected: version(expected),
            intent,
        }
    }

    fn decide(
        actor: u128,
        request: CommentRequest,
        facts: CommentFacts,
        current: Endorsement<CommentAnchor>,
    ) -> CommentDecision {
        decide_comment_endorsement(
            principal(actor),
            work(50),
            comment(9),
            request,
            Some(facts),
            current,
        )
    }

    #[test]
    fn a_reader_endorses_and_clears_a_comment() {
        let CommentDecision::Transition(Transition::Changed { next, delta }) = decide(
            3,
            request(1, CommentIntent::Endorse),
            facts(),
            Endorsement::ABSENT,
        ) else {
            panic!("endorse");
        };
        assert_eq!(next, Endorsement::restore(Some(CommentAnchor), version(2)));
        assert_eq!(delta.added(), Some(CommentAnchor));
        assert_eq!(
            own_comment_view(work(50), comment(9), Some(facts()), next),
            OwnView::Endorsed
        );
        let CommentDecision::Transition(Transition::Changed { next, .. }) =
            decide(3, request(2, CommentIntent::Clear), facts(), next)
        else {
            panic!("clear");
        };
        assert_eq!(next, Endorsement::restore(None, version(3)));
    }

    #[test]
    fn closing_comments_stops_new_endorsements_only() {
        let shut = CommentFacts {
            settings: closed(),
            ..facts()
        };
        assert_eq!(
            decide(
                3,
                request(1, CommentIntent::Endorse),
                shut,
                Endorsement::ABSENT
            ),
            CommentDecision::CommentsClosed
        );
        let on = Endorsement::restore(Some(CommentAnchor), version(2));
        assert!(matches!(
            decide(3, request(2, CommentIntent::Endorse), shut, on),
            CommentDecision::Transition(Transition::Reaffirmed { .. })
        ));
        assert_eq!(
            decide(3, request(1, CommentIntent::Endorse), shut, on),
            CommentDecision::Transition(Transition::AlreadyCurrent(on)),
            "a lost-response retry after closing still answers"
        );
        assert!(matches!(
            decide(3, request(2, CommentIntent::Clear), shut, on),
            CommentDecision::Transition(Transition::Changed { .. })
        ));
        assert_eq!(
            comment_summary(
                work(50),
                comment(9),
                Some(shut),
                EndorsementCount::restore(2)
            ),
            Summary::Count(EndorsementCount::restore(2)),
            "closing hides nothing"
        );
    }

    #[test]
    fn writers_cannot_endorse_or_touch_their_own_comment() {
        for intent in [CommentIntent::Endorse, CommentIntent::Clear] {
            assert_eq!(
                decide(2, request(1, intent), facts(), Endorsement::ABSENT),
                CommentDecision::OwnComment
            );
        }
    }

    #[test]
    fn hidden_or_misbound_comments_refuse_everything_and_show_nothing() {
        let on = Endorsement::restore(Some(CommentAnchor), version(2));
        let mut unavailable = vec![
            CommentFacts {
                work_public: false,
                ..facts()
            },
            CommentFacts {
                work: work(51),
                ..facts()
            },
            CommentFacts {
                comment: comment(10),
                ..facts()
            },
        ];
        for review in [Review::Pending, Review::Rejected] {
            unavailable.push(CommentFacts {
                state: CommentState::restore(review, None, ModerationVersion::FIRST),
                ..facts()
            });
        }
        for removal in [Removal::ByAuthor, Removal::ByModeration] {
            unavailable.push(CommentFacts {
                state: CommentState::restore(
                    Review::Approved,
                    Some(removal),
                    ModerationVersion::new(2).unwrap(),
                ),
                ..facts()
            });
        }
        for hidden in unavailable.into_iter().map(Some).chain([None]) {
            for intent in [CommentIntent::Endorse, CommentIntent::Clear] {
                assert_eq!(
                    decide_comment_endorsement(
                        principal(3),
                        work(50),
                        comment(9),
                        request(2, intent),
                        hidden,
                        on
                    ),
                    CommentDecision::TargetUnavailable,
                    "{hidden:?} {intent:?}"
                );
            }
            assert_eq!(
                own_comment_view(work(50), comment(9), hidden, on),
                OwnView::Unavailable
            );
            assert_eq!(
                comment_summary(work(50), comment(9), hidden, EndorsementCount::restore(1)),
                Summary::Unavailable
            );
        }
    }
}
