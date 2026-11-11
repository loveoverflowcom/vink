//! Tombstone scenario and negative control Quint trace replay (#157).
//!
//! Replays all 3 scenarios from `tombstone-vectors.json` (30 steps across `author_deleted_public_work`,
//! `private_then_deleted`, `moderation_removed`) against production [`Publication`], [`WorkModeration`],
//! [`plan_work_access`], and discussion visibility planning. Also replays the `tombstone-private-as-deleted-counterexample.json`
//! negative control to ensure private works never project as tombstones.

use super::*;
use crate::domain::discussion::lifecycle::{CommentState, Decision as CommentDecision, Transition};
use crate::domain::discussion::visibility::CommentView;
use crate::domain::ids::PrincipalId;
use crate::domain::moderation::{AdminAuthority, ModerationError, WorkModeration};
use crate::domain::publication::tests::{owner, revision, save, snapshot, work};
use crate::domain::publication::{
    Audience, AuthorCommand, AuthorOutcome, DeletionProvenance, Lifecycle, Moderation, Publication,
    PublicationError, ReviewOutcome, ReviewedRevision, WorkAccessPlan,
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
                            Err(TombstoneWriteError::TombstoneReadOnly) => "tombstone_read_only",
                            Err(TombstoneWriteError::WorkUnavailable) => "work_unavailable",
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
                        match comment.decide(comment.version(), decision) {
                            Ok(Transition::Changed(next)) | Ok(Transition::Unchanged(next)) => {
                                *comment = next;
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

            let expected_status = match &step.state.status as &str {
                "unpublished" => "unpublished",
                "public" => "public",
                "private" => "private",
                "author_deleted" => "author_deleted",
                "deleted_hidden" => "deleted_hidden",
                "moderation_removed" => "moderation_removed",
                other => panic!("unknown model status {other}"),
            };
            let actual_status = match pub_state.facts().lifecycle {
                Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(_)) => {
                    "author_deleted"
                }
                Lifecycle::AuthorDeleted(DeletionProvenance::Hidden) => "deleted_hidden",
                Lifecycle::Removed => "moderation_removed",
                Lifecycle::Active => {
                    if pub_state.facts().audience == Audience::Public {
                        "public"
                    } else if ever_public {
                        "private"
                    } else {
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

    assert_eq!(total_steps, 30);
}

#[test]
fn the_private_as_deleted_counterexample_is_refused_by_the_production_access_plan() {
    let vectors: TombstoneCounterexample = serde_json::from_str(include_str!(
        "../../../behavior/publication/tombstone-private-as-deleted-counterexample.json"
    ))
    .unwrap();
    let scenario = &vectors.violation[0];
    assert_eq!(scenario.steps.len(), 4);
    // The production calls below are bound to this exact trace; a regenerated counterexample
    // with other inputs must fail here instead of passing against a stale hand-written replay.
    let inputs: Vec<(&str, i64, u128, &str)> = scenario
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
        .collect();
    assert_eq!(
        inputs,
        [
            ("publish", 1, 0, "none"),
            ("submit", 2, 1, "none"),
            ("review", 9, 1, "approved"),
            ("make_private", 1, 0, "none"),
        ]
    );

    let mut pub_state = Publication::new(work(1), owner());
    pub_state = save(&pub_state, 1);

    // Step 1: publish
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
    let r = t
        .next
        .record_review(ReviewedRevision {
            work: work(1),
            revision: revision(1),
            hash: snapshot(1).content.hash().clone(),
            expected: t.next.facts().revisions[0].moderation,
            outcome: ReviewOutcome::Approved,
        })
        .unwrap();
    pub_state = r.next;

    // Step 2: submit comment 1 by writer 2
    let writer2 = PrincipalId::from_uuid(uuid::Uuid::from_u128(2));
    assert!(check_comment_submission(&pub_state).is_ok());
    let mut comment1 = CommentState::SUBMITTED;

    // Step 3: review comment 1 approved
    let t = comment1
        .decide(comment1.version(), CommentDecision::Approve)
        .unwrap();
    comment1 = match t {
        Transition::Changed(s) | Transition::Unchanged(s) => s,
    };

    // Step 4: author makes work private
    let t = pub_state
        .decide_author(
            owner(),
            AuthorCommand::SetPrivate {
                expected: pub_state.facts().generation,
            },
        )
        .unwrap();
    pub_state = t.next;

    // The mutant counterexample asserts that a guest (viewer 0) sees a tombstone page with comment 1:
    let mutant_last = &scenario.steps[3];
    assert_eq!(mutant_last.state.status, "private");
    assert_eq!(mutant_last.state.views[&0].page, "tombstone");
    assert_eq!(mutant_last.state.views[&0].comments, vec![1]);

    // Production refuses: access_plan is Unavailable, not tombstone:
    let access = plan_work_access(&pub_state);
    assert_eq!(access, WorkAccessPlan::Unavailable);
    assert_eq!(access.page_kind(), "not_found");
    assert!(!access.thread_accessible());

    // Guests and other viewers get CommentView::Unavailable:
    let guest_view = plan_comment_view(&pub_state, writer2, comment1, None);
    assert_eq!(
        guest_view,
        crate::domain::discussion::visibility::CommentView::Unavailable
    );

    let writer2_view = plan_comment_view(&pub_state, writer2, comment1, Some(writer2));
    assert_eq!(
        writer2_view,
        crate::domain::discussion::visibility::CommentView::Unavailable
    );
}
