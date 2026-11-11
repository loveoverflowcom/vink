//! Tombstone / hidden page scenario and negative control Quint trace replay (#157,
//! ADMIN-MODERATION-20261111).
//!
//! Replays all 4 scenarios from `tombstone-vectors.json` (41 steps across
//! `author_deleted_public_work`, `hidden_then_public_then_deleted`, `never_public`,
//! `moderation_removed`) against production [`Publication`], [`WorkModeration`],
//! [`plan_work_access`], discussion visibility planning, the comment review context and the
//! author-notification fence. Also replays both negative controls: an admin removal is never
//! the hidden page, and an approval made while hidden never notifies after republication.

use super::*;
use crate::domain::discussion::lifecycle::{CommentState, Decision as CommentDecision, Transition};
use crate::domain::discussion::visibility::CommentView;
use crate::domain::ids::PrincipalId;
use crate::domain::moderation::{AdminAuthority, ModerationError, WorkModeration};
use crate::domain::publication::tests::{owner, revision, save, snapshot, work};
use crate::domain::publication::{
    Audience, AuthorCommand, AuthorOutcome, DeletionProvenance, Lifecycle, Moderation,
    NotificationStamp, Publication, PublicationError, ReviewOutcome, ReviewedRevision,
    WorkAccessPlan,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct TombstoneVectors {
    scenarios: Vec<TombstoneScenario>,
}

#[derive(Deserialize)]
struct TombstoneScenario {
    name: String,
    steps: Vec<TombstoneStep>,
}

#[derive(Deserialize)]
struct TombstoneStep {
    command: TombstoneCommand,
    outcome: String,
    state: TombstoneState,
}

#[derive(Deserialize)]
struct TombstoneCommand {
    actor: i64,
    id: u128,
    op: String,
    value: String,
}

#[derive(Deserialize)]
struct TombstoneState {
    status: String,
    #[serde(rename = "everPublic")]
    ever_public: bool,
    comments: BTreeMap<u128, TombstoneComment>,
    views: BTreeMap<i64, TombstoneView>,
    notified: Vec<u128>,
}

#[derive(Deserialize)]
struct TombstoneComment {
    removed: bool,
    review: String,
    writer: i64,
}

#[derive(Deserialize)]
struct TombstoneView {
    comments: Vec<u128>,
    listed: bool,
    page: String,
    snippet: bool,
}

#[derive(Deserialize)]
struct TombstoneCounterexample {
    violation: Vec<TombstoneScenario>,
}

#[test]
fn quint_tombstone_scenarios_replay_against_production_access_and_discussion() {
    let vectors: TombstoneVectors = serde_json::from_str(include_str!(
        "../../../behavior/publication/tombstone-vectors.json"
    ))
    .unwrap();

    let mut total_steps = 0;
    for scenario in vectors.scenarios {
        let mut pub_state = Publication::new(work(1), owner());
        let mut mod_state = WorkModeration::new(work(1));
        pub_state = save(&pub_state, 1);
        let mut comments: BTreeMap<u128, (PrincipalId, CommentState)> = BTreeMap::new();
        let mut ever_public = false;
        // Every approval with the stamp of the moment it happened, and those that notified.
        let mut approvals: Vec<(u128, NotificationStamp)> = Vec::new();
        let mut notified: Vec<u128> = Vec::new();

        for (index, step) in scenario.steps.iter().enumerate() {
            let c = &step.command;
            let outcome: &'static str = match c.op.as_str() {
                // Model shell `publish` = author request + an admitted approval of revision 1
                // (authority assumption: the evaluator/admin path is M1's, not M3's).
                "publish" => {
                    let before = pub_state.clone();
                    match pub_state.decide_author(
                        owner(),
                        AuthorCommand::RequestPublic {
                            revision: revision(1),
                            expected: pub_state.facts().generation,
                        },
                    ) {
                        Ok(t) => {
                            pub_state = match t.outcome {
                                AuthorOutcome::Published => t.next,
                                AuthorOutcome::Submitted => {
                                    t.next
                                        .record_review(ReviewedRevision {
                                            work: work(1),
                                            revision: revision(1),
                                            hash: snapshot(1).content.hash().clone(),
                                            expected: Moderation::Pending,
                                            outcome: ReviewOutcome::Approved,
                                        })
                                        .unwrap()
                                        .next
                                }
                                other => panic!("unexpected publish outcome {other:?}"),
                            };
                            ever_public = true;
                            "published"
                        }
                        Err(PublicationError::WorkUnavailable) => {
                            assert_eq!(pub_state, before);
                            "work_unavailable"
                        }
                        Err(other) => panic!("unexpected publish refusal {other:?}"),
                    }
                }
                "make_private" => {
                    let before = pub_state.clone();
                    match pub_state.decide_author(
                        owner(),
                        AuthorCommand::SetPrivate {
                            expected: pub_state.facts().generation,
                        },
                    ) {
                        Ok(t) => {
                            let outcome = match t.outcome {
                                AuthorOutcome::MadePrivate => "made_private",
                                AuthorOutcome::Reaffirmed => "unchanged",
                                other => panic!("unexpected make_private outcome {other:?}"),
                            };
                            pub_state = t.next;
                            outcome
                        }
                        Err(PublicationError::WorkUnavailable) => {
                            assert_eq!(pub_state, before);
                            "work_unavailable"
                        }
                        Err(other) => panic!("unexpected make_private refusal {other:?}"),
                    }
                }
                "author_delete" => {
                    let before = pub_state.clone();
                    match pub_state.decide_author(
                        owner(),
                        AuthorCommand::Delete {
                            expected: Some(pub_state.facts().generation),
                        },
                    ) {
                        Ok(t) => {
                            assert_eq!(t.outcome, AuthorOutcome::Deleted);
                            pub_state = t.next;
                            // The production provenance, not a harness pre-check, names the outcome.
                            match pub_state.facts().lifecycle {
                                Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(
                                    _,
                                )) => "deleted_with_tombstone",
                                Lifecycle::AuthorDeleted(DeletionProvenance::Hidden) => "deleted",
                                other => panic!("delete produced {other:?}"),
                            }
                        }
                        Err(PublicationError::WorkUnavailable) => {
                            assert_eq!(pub_state, before);
                            "work_unavailable"
                        }
                        Err(other) => panic!("unexpected author_delete refusal {other:?}"),
                    }
                }
                "moderation_remove" => {
                    let before = (pub_state.clone(), mod_state.clone());
                    match mod_state.apply_admin_remove(
                        &pub_state,
                        Some(AdminAuthority::scoped(
                            PrincipalId::from_uuid(uuid::Uuid::from_u128(c.actor as u128)),
                            work(1),
                        )),
                    ) {
                        Ok(t) => {
                            pub_state = t.publication;
                            mod_state = t.moderation;
                            "removed_by_moderation"
                        }
                        Err(ModerationError::TargetGone) => {
                            assert_eq!((pub_state.clone(), mod_state.clone()), before);
                            "work_unavailable"
                        }
                        Err(other) => panic!("unexpected moderation_remove refusal {other:?}"),
                    }
                }
                "submit" => {
                    let writer = PrincipalId::from_uuid(uuid::Uuid::from_u128(c.actor as u128));
                    if let std::collections::btree_map::Entry::Vacant(e) = comments.entry(c.id) {
                        match check_comment_submission(&pub_state) {
                            Ok(()) => {
                                e.insert((writer, CommentState::SUBMITTED));
                                "created"
                            }
                            Err(error) => error.as_str(),
                        }
                    } else {
                        "duplicate"
                    }
                }
                "review" => {
                    let Some((_, comment)) = comments.get_mut(&c.id) else {
                        panic!("review on unknown comment {}", c.id);
                    };
                    if !comment.is_present() {
                        "comment_unavailable"
                    } else {
                        let decision = match c.value.as_str() {
                            "approved" => CommentDecision::Approve,
                            "rejected" => CommentDecision::Reject,
                            other => panic!("unknown review value {other}"),
                        };
                        // COM-11: a reviewer of a comment on a non-public work sees only the comment.
                        assert_eq!(
                            matches!(
                                plan_comment_review_context(&pub_state),
                                CommentReviewContext::WithWork(_)
                            ),
                            pub_state.readable_revision().is_some()
                        );
                        match comment.decide(comment.version(), decision) {
                            Ok(Transition::Changed(next)) | Ok(Transition::Unchanged(next)) => {
                                *comment = next;
                                if decision == CommentDecision::Approve {
                                    let stamp = pub_state.notification_stamp();
                                    approvals.push((c.id, stamp));
                                    if pub_state.should_notify_author(stamp) {
                                        notified.push(c.id);
                                    }
                                }
                                "reviewed"
                            }
                            Err(_) => "invalid_transition",
                        }
                    }
                }
                "remove_own" => {
                    let Some((writer, comment)) = comments.get_mut(&c.id) else {
                        panic!("remove_own on unknown comment {}", c.id);
                    };
                    let actor = PrincipalId::from_uuid(uuid::Uuid::from_u128(c.actor as u128));
                    if *writer != actor
                        || !comment.is_present()
                        || check_own_comment_removal(&pub_state).is_err()
                    {
                        "comment_unavailable"
                    } else {
                        match comment.remove_by_author() {
                            Ok(Transition::Changed(next)) | Ok(Transition::Unchanged(next)) => {
                                *comment = next;
                                "comment_removed"
                            }
                            Err(_) => "invalid_transition",
                        }
                    }
                }
                other => panic!("unknown command op {other}"),
            };

            assert_eq!(
                outcome, step.outcome,
                "scenario {} step {} op {} outcome mismatch",
                scenario.name, index, c.op
            );

            // Verify state projections match the model step state
            assert_eq!(
                ever_public, step.state.ever_public,
                "scenario {} step {} ever_public mismatch",
                scenario.name, index
            );

            // An approval that did not notify when it happened never becomes deliverable later,
            // not even after republication (no backlog of the hidden period).
            for (id, stamp) in approvals.iter().filter(|(id, _)| !notified.contains(id)) {
                assert!(
                    !pub_state.should_notify_author(*stamp),
                    "scenario {} step {}: an old approval of comment {id} would notify",
                    scenario.name,
                    index
                );
            }
            assert_eq!(
                notified, step.state.notified,
                "{} step {}",
                scenario.name, index
            );

            let expected_status = step.state.status.as_str();
            let actual_status = match pub_state.facts().lifecycle {
                Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(_)) => {
                    "author_deleted"
                }
                Lifecycle::AuthorDeleted(DeletionProvenance::Hidden) => "deleted_hidden",
                Lifecycle::Removed => "moderation_removed",
                Lifecycle::Active => {
                    if pub_state.facts().audience == Audience::Public {
                        "public"
                    } else if pub_state.facts().hidden.is_some() {
                        "hidden"
                    } else {
                        assert!(!ever_public || pub_state.facts().ever_public.is_empty());
                        "unpublished"
                    }
                }
            };
            assert_eq!(
                actual_status, expected_status,
                "scenario {} step {} status mismatch",
                scenario.name, index
            );

            assert_eq!(comments.len(), step.state.comments.len());
            for (&comment_id, (writer, comment_state)) in &comments {
                let expected_comment = &step.state.comments[&comment_id];
                assert_eq!(expected_comment.removed, !comment_state.is_present());
                assert_eq!(expected_comment.writer, writer.as_uuid().as_u128() as i64);
                let expected_review = match comment_state.review() {
                    crate::domain::discussion::lifecycle::Review::Pending => "pending",
                    crate::domain::discussion::lifecycle::Review::Approved => "approved",
                    crate::domain::discussion::lifecycle::Review::Rejected => "rejected",
                };
                assert_eq!(expected_comment.review, expected_review);
            }

            // Verify access plan projection for each viewer
            let access = plan_work_access(&pub_state);
            for (&viewer_id, view) in &step.state.views {
                let viewer = if viewer_id == 0 {
                    None
                } else {
                    Some(PrincipalId::from_uuid(uuid::Uuid::from_u128(
                        viewer_id as u128,
                    )))
                };

                let page_kind = access.page_kind();
                assert_eq!(
                    page_kind, view.page,
                    "scenario {} step {} viewer {} page mismatch",
                    scenario.name, index, viewer_id
                );

                assert_eq!(
                    access.is_listed(),
                    view.listed,
                    "scenario {} step {} viewer {} listed mismatch",
                    scenario.name,
                    index,
                    viewer_id
                );

                assert_eq!(
                    access.allows_snippet(),
                    view.snippet,
                    "scenario {} step {} viewer {} snippet mismatch",
                    scenario.name,
                    index,
                    viewer_id
                );

                // Verify visible comments match the view
                let visible_comment_ids: Vec<u128> = comments
                    .iter()
                    .filter(|(_, (writer, state))| {
                        let comment_view = plan_comment_view(&pub_state, *writer, *state, viewer);
                        comment_view != CommentView::Unavailable
                    })
                    .map(|(&id, _)| id)
                    .collect();

                assert_eq!(
                    visible_comment_ids, view.comments,
                    "scenario {} step {} viewer {} visible comments mismatch",
                    scenario.name, index, viewer_id
                );
            }

            total_steps += 1;
        }
    }

    assert_eq!(total_steps, 41);
}

fn trace_inputs(scenario: &TombstoneScenario) -> Vec<(&str, i64, u128, &str)> {
    scenario
        .steps
        .iter()
        .map(|s| {
            (
                s.command.op.as_str(),
                s.command.actor,
                s.command.id,
                s.command.value.as_str(),
            )
        })
        .collect()
}

fn published(pub_state: &Publication) -> Publication {
    let expected = pub_state.facts().generation;
    let t = pub_state
        .decide_author(
            owner(),
            AuthorCommand::RequestPublic {
                revision: revision(1),
                expected,
            },
        )
        .unwrap();
    if t.outcome == AuthorOutcome::Published {
        return t.next;
    }
    t.next
        .record_review(ReviewedRevision {
            work: work(1),
            revision: revision(1),
            hash: snapshot(1).content.hash().clone(),
            expected: t.next.facts().revisions[0].moderation,
            outcome: ReviewOutcome::Approved,
        })
        .unwrap()
        .next
}

fn approved_comment() -> CommentState {
    match CommentState::SUBMITTED
        .decide(CommentState::SUBMITTED.version(), CommentDecision::Approve)
    {
        Ok(Transition::Changed(s)) => s,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_removed_as_hidden_counterexample_is_refused_by_the_production_access_plan() {
    let vectors: TombstoneCounterexample = serde_json::from_str(include_str!(
        "../../../behavior/publication/tombstone-removed-as-hidden-counterexample.json"
    ))
    .unwrap();
    let scenario = &vectors.violation[0];
    // The production calls below are bound to this exact trace; a regenerated counterexample
    // with other inputs must fail here instead of passing against a stale hand-written replay.
    assert_eq!(
        trace_inputs(scenario),
        [
            ("publish", 1, 0, "none"),
            ("submit", 2, 1, "none"),
            ("review", 9, 1, "approved"),
            ("moderation_remove", 9, 0, "none"),
        ]
    );
    let mutant_last = &scenario.steps[3];
    assert_eq!(mutant_last.state.status, "moderation_removed");
    assert_eq!(mutant_last.state.views[&0].page, "hidden");
    assert_eq!(mutant_last.state.views[&0].comments, vec![1]);

    let pub_state = published(&save(&Publication::new(work(1), owner()), 1));
    let writer2 = PrincipalId::from_uuid(uuid::Uuid::from_u128(2));
    assert!(check_comment_submission(&pub_state).is_ok());
    let comment1 = approved_comment();
    let removed = WorkModeration::new(work(1))
        .apply_admin_remove(
            &pub_state,
            Some(AdminAuthority::scoped(
                PrincipalId::from_uuid(uuid::Uuid::from_u128(9)),
                work(1),
            )),
        )
        .unwrap()
        .publication;

    // Production refuses the mutant: an admin removal is not found, with no thread.
    let access = plan_work_access(&removed);
    assert_eq!(access, WorkAccessPlan::Unavailable);
    assert_eq!(access.page_kind(), "not_found");
    assert!(!access.thread_accessible());
    assert_eq!(removed.facts().hidden, None);
    for viewer in [None, Some(writer2)] {
        assert_eq!(
            plan_comment_view(&removed, writer2, comment1, viewer),
            CommentView::Unavailable
        );
    }
}

#[test]
fn the_hidden_notice_counterexample_is_refused_by_the_production_notification_fence() {
    let vectors: TombstoneCounterexample = serde_json::from_str(include_str!(
        "../../../behavior/publication/tombstone-hidden-notice-counterexample.json"
    ))
    .unwrap();
    let scenario = &vectors.violation[0];
    assert_eq!(
        trace_inputs(scenario),
        [
            ("publish", 1, 0, "none"),
            ("submit", 2, 1, "none"),
            ("make_private", 1, 0, "none"),
            ("review", 9, 1, "approved"),
            ("publish", 1, 0, "none"),
        ]
    );
    let mutant_last = &scenario.steps[4];
    assert_eq!(mutant_last.state.notified, vec![1]);

    let public = published(&save(&Publication::new(work(1), owner()), 1));
    assert!(check_comment_submission(&public).is_ok());
    let hidden = public
        .decide_author(
            owner(),
            AuthorCommand::SetPrivate {
                expected: public.facts().generation,
            },
        )
        .unwrap()
        .next;
    assert!(matches!(
        plan_work_access(&hidden),
        WorkAccessPlan::AuthorHidden { .. }
    ));
    // The approval happens on the hidden page: its stamp is of a non-readable moment.
    let stamp = hidden.notification_stamp();
    assert!(!hidden.should_notify_author(stamp));
    let again = published(&hidden);
    assert!(matches!(
        plan_work_access(&again),
        WorkAccessPlan::PublicBody(_)
    ));
    // Production refuses the mutant: republication delivers nothing from the hidden period.
    assert!(!again.should_notify_author(stamp));
    assert!(again.should_notify_author(again.notification_stamp()));
    assert!(again.facts().public_epoch > public.facts().public_epoch);
}
