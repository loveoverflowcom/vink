//! Two-tier discussion decisions, owned by Vink (owner decision 03/11/2026,
//! `docs/architecture/discussion-ownership.md`). This file holds the structural seam (D2T-01):
//! roots, one flat list of replies and context normalization. The submodules hold the other pure
//! decisions: body text, lifecycle, versioned admission and post-review (#120), per-work comments
//! mode, permissions, viewer visibility, idempotent submission, reactions and paging. Nothing here
//! performs I/O; a successful decision
//! is not a durable write.
//!
//! The PostgreSQL adapter loads facts in a transaction and calls these same functions; the
//! database independently enforces identity uniqueness and the two tiers. [`DiscussionThread`] is
//! the in-memory aggregate that the Quint trace replay drives; it shares [`reply_placement`]
//! with the adapter, so both go through one normalization seam.
//!
//! A reply target is looked up in this thread, never accepted as a client-supplied root.
//! Replying to a reply preserves context but does not create another structural level.
//!
//! Validated structure has no deserialization bypass or public mutation:
//! ```compile_fail
//! use vink_app::domain::discussion::CommentPlacement;
//! let placement: CommentPlacement = serde_json::from_str("{}").unwrap();
//! ```
//! ```compile_fail
//! use vink_app::domain::discussion::RootCommentId;
//! use vink_app::domain::ids::CommentId;
//! let comment = CommentId::parse("11111111-1111-4111-8111-111111111111").unwrap();
//! let root: RootCommentId = comment;
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes): the same imports and
//! IDs compile, and a placement exists only after a thread decision.
//! ```
//! use vink_app::domain::discussion::{CommentPlacement, DiscussionThread, RootCommentId};
//! use vink_app::domain::ids::{CommentId, ThreadId};
//! let comment = CommentId::parse("11111111-1111-4111-8111-111111111111").unwrap();
//! let thread = ThreadId::parse("22222222-2222-4222-8222-222222222222").unwrap();
//! let _: serde_json::Value = serde_json::from_str("{}").unwrap();
//! let placement: CommentPlacement = DiscussionThread::new(thread).create_root(comment).unwrap();
//! let root: RootCommentId = placement.root_id();
//! assert_eq!(root.as_comment_id(), comment);
//! ```

pub mod admission;
#[cfg(test)]
mod admission_replay;
pub mod body;
pub mod lifecycle;
#[cfg(test)]
mod lifecycle_replay;
pub mod moderation;
pub mod outbox;
#[cfg(test)]
mod outbox_replay;
pub mod page;
pub mod policy;
#[cfg(kani)]
mod proofs;
pub mod reaction;
#[cfg(test)]
mod reaction_replay;
pub mod settings;
pub mod submission;
pub mod visibility;

use super::ids::{CommentId, ThreadId};

/// A root identity obtained from a valid structural decision, not a raw DTO assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootCommentId(CommentId);

impl RootCommentId {
    pub const fn as_comment_id(self) -> CommentId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Root,
    Reply {
        root_id: RootCommentId,
        // None is the ONLY encoding for a direct response to the root.
        reply_to: Option<CommentId>,
    },
}

/// Immutable placement only. This contains no publishable text or author/access facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentPlacement {
    id: CommentId,
    thread_id: ThreadId,
    position: Position,
}

impl CommentPlacement {
    pub const fn id(self) -> CommentId {
        self.id
    }

    pub const fn thread_id(self) -> ThreadId {
        self.thread_id
    }

    /// Roots return their own identity; replies always return a root, never their context.
    pub const fn root_id(self) -> RootCommentId {
        match self.position {
            Position::Root => RootCommentId(self.id),
            Position::Reply { root_id, .. } => root_id,
        }
    }

    /// None for roots and direct replies. Use `is_root` to distinguish those two positions.
    pub const fn reply_to(self) -> Option<CommentId> {
        match self.position {
            Position::Root => None,
            Position::Reply { reply_to, .. } => reply_to,
        }
    }

    pub const fn is_root(self) -> bool {
        matches!(self.position, Position::Root)
    }

    /// Rehydrates a placement the adapter READ from storage. Never call it with request data:
    /// clients name a target, not a position. It rejects shapes that are impossible in any thread
    /// (self root, self/root context); that `root_id` really is a root of `thread_id` and that a
    /// context is a reply of the same root are enforced by the database's composite keys, which
    /// this row already passed. Run the PostgreSQL suite for that boundary.
    pub fn from_stored(
        id: CommentId,
        thread_id: ThreadId,
        position: StoredPosition,
    ) -> Result<Self, StoredPlacementError> {
        let position = match position {
            StoredPosition::Root => Position::Root,
            StoredPosition::Reply { root_id, reply_to } => {
                if root_id == id || reply_to == Some(id) || reply_to == Some(root_id) {
                    return Err(StoredPlacementError);
                }
                Position::Reply {
                    root_id: RootCommentId(root_id),
                    reply_to,
                }
            }
        };
        Ok(Self {
            id,
            thread_id,
            position,
        })
    }
}

/// Raw position columns of a stored comment, before [`CommentPlacement::from_stored`] checks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredPosition {
    Root,
    Reply {
        root_id: CommentId,
        reply_to: Option<CommentId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("stored comment placement is not a valid two-tier position")]
pub struct StoredPlacementError;

/// A new root of `thread_id`. Roots have no context.
pub const fn root_placement(thread_id: ThreadId, id: CommentId) -> CommentPlacement {
    CommentPlacement {
        id,
        thread_id,
        position: Position::Root,
    }
}

/// The one normalization seam for a reply. `target` must have been looked up by the server in
/// the server-selected thread (the aggregate below or the adapter under a row lock); a placement
/// from another thread is `TargetNotFound`. Replying to a reply keeps that reply as context and
/// its root as the structural parent, so no third tier exists. A direct reply to a root encodes
/// its context as `None`, never as the root ID.
pub fn reply_placement(
    thread_id: ThreadId,
    id: CommentId,
    target: CommentPlacement,
) -> Result<CommentPlacement, StructureError> {
    if target.thread_id != thread_id {
        return Err(StructureError::TargetNotFound);
    }
    // A fresh identity equal to the target or to its root would make a self-referential
    // placement; it is a collision, never a valid reply.
    if target.id == id || target.root_id().0 == id {
        return Err(StructureError::CommentExists);
    }
    Ok(CommentPlacement {
        id,
        thread_id,
        position: Position::Reply {
            root_id: target.root_id(),
            reply_to: (!target.is_root()).then_some(target.id),
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StructureError {
    #[error("comment identity already exists in this thread")]
    CommentExists,
    #[error("reply target does not exist in this thread")]
    TargetNotFound,
}

/// Pure, append-only structural state and the production creation seam for this slice.
///
/// It deliberately offers no raw-record restore, edit, removal or public-list method. Future
/// adapters need separately verified durable constraints, moderation, reads and rehydration.
/// Linear membership checks serve this creation-only slice; this is no scaling claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscussionThread {
    id: ThreadId,
    comments: Vec<CommentPlacement>,
}

impl DiscussionThread {
    pub const fn new(id: ThreadId) -> Self {
        Self {
            id,
            comments: Vec::new(),
        }
    }

    pub const fn id(&self) -> ThreadId {
        self.id
    }

    /// Lookup within the server-selected thread. This is not a visibility/access check.
    pub fn comment(&self, id: CommentId) -> Option<CommentPlacement> {
        self.comments
            .iter()
            .find(|comment| comment.id == id)
            .copied()
    }

    pub fn create_root(&mut self, id: CommentId) -> Result<CommentPlacement, StructureError> {
        self.ensure_new(id)?;
        let placement = root_placement(self.id, id);
        self.comments.push(placement);
        Ok(placement)
    }

    /// Normalize ONLY a target already present in this thread. The caller cannot supply
    /// `root_id` or a placement from another thread. Failure leaves all structural state intact.
    pub fn create_reply(
        &mut self,
        id: CommentId,
        target_id: CommentId,
    ) -> Result<CommentPlacement, StructureError> {
        self.ensure_new(id)?;
        let target = self
            .comment(target_id)
            .ok_or(StructureError::TargetNotFound)?;
        let placement = reply_placement(self.id, id, target)?;
        self.comments.push(placement);
        Ok(placement)
    }

    fn ensure_new(&self, id: CommentId) -> Result<(), StructureError> {
        if self.comment(id).is_some() {
            Err(StructureError::CommentExists)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use uuid::Uuid;

    fn comment(id: u128) -> CommentId {
        CommentId::from_uuid(Uuid::from_u128(id))
    }

    fn thread(id: u128) -> DiscussionThread {
        DiscussionThread::new(ThreadId::from_uuid(Uuid::from_u128(id)))
    }

    #[test]
    fn reply_to_reply_keeps_one_root_and_flat_structure() {
        let mut state = thread(10);
        let root = state.create_root(comment(1)).unwrap();
        let first = state.create_reply(comment(2), root.id()).unwrap();
        let second = state.create_reply(comment(3), first.id()).unwrap();
        let third = state.create_reply(comment(4), second.id()).unwrap();

        assert!(root.is_root());
        assert_eq!(root.root_id().as_comment_id(), root.id());
        assert_eq!(root.reply_to(), None);
        for reply in [first, second, third] {
            assert!(!reply.is_root());
            assert_eq!(reply.thread_id(), state.id());
            assert_eq!(reply.root_id(), root.root_id());
        }
        assert_eq!(first.reply_to(), None);
        assert_eq!(second.reply_to(), Some(first.id()));
        assert_eq!(third.reply_to(), Some(second.id()));
        assert_structure(&state);
    }

    #[test]
    fn target_selects_the_root_without_a_client_root_argument() {
        let mut state = thread(10);
        let first_root = state.create_root(comment(1)).unwrap();
        let other_root = state.create_root(comment(2)).unwrap();
        let other_reply = state.create_reply(comment(3), other_root.id()).unwrap();
        let reply = state.create_reply(comment(4), other_reply.id()).unwrap();

        assert_eq!(reply.root_id(), other_root.root_id());
        assert_ne!(reply.root_id(), first_root.root_id());
        assert_eq!(reply.reply_to(), Some(other_reply.id()));
    }

    #[test]
    fn a_foreign_or_missing_target_does_not_change_this_thread() {
        let mut state = thread(10);
        state.create_root(comment(1)).unwrap();
        let mut foreign = thread(20);
        foreign.create_root(comment(2)).unwrap();
        let foreign_reply = foreign.create_reply(comment(3), comment(2)).unwrap();
        let before = state.clone();

        for target in [comment(2), foreign_reply.id(), comment(99)] {
            assert_eq!(
                state.create_reply(comment(4), target),
                Err(StructureError::TargetNotFound)
            );
            assert_eq!(state, before);
        }
    }

    #[test]
    fn identities_cannot_be_reused_reparented_or_made_self_referential() {
        let mut state = thread(10);
        let root = state.create_root(comment(1)).unwrap();
        let reply = state.create_reply(comment(2), root.id()).unwrap();
        let before = state.clone();

        assert_eq!(
            state.create_root(reply.id()),
            Err(StructureError::CommentExists)
        );
        for (id, target) in [(root.id(), reply.id()), (reply.id(), reply.id())] {
            assert_eq!(
                state.create_reply(id, target),
                Err(StructureError::CommentExists)
            );
        }
        assert_eq!(
            state.create_reply(comment(3), comment(3)),
            Err(StructureError::TargetNotFound)
        );
        assert_eq!(state, before);
        assert_eq!(state.comment(root.id()), Some(root));
        assert_eq!(state.comment(reply.id()), Some(reply));
    }

    #[test]
    fn the_shared_reply_seam_rejects_foreign_targets_and_self_targets() {
        let mut state = thread(10);
        let root = state.create_root(comment(1)).unwrap();
        let reply = state.create_reply(comment(2), root.id()).unwrap();
        let foreign = ThreadId::from_uuid(Uuid::from_u128(20));

        assert_eq!(
            reply_placement(foreign, comment(3), reply),
            Err(StructureError::TargetNotFound)
        );
        assert_eq!(
            reply_placement(state.id(), reply.id(), reply),
            Err(StructureError::CommentExists)
        );
        let contextual = reply_placement(state.id(), comment(3), reply).unwrap();
        assert_eq!(contextual.root_id(), root.root_id());
        assert_eq!(contextual.reply_to(), Some(reply.id()));
        assert_eq!(
            reply_placement(state.id(), root.id(), reply),
            Err(StructureError::CommentExists),
            "an identity equal to the target's root would make the reply its own root"
        );
        let direct = reply_placement(state.id(), comment(4), root).unwrap();
        assert_eq!(
            direct.reply_to(),
            None,
            "a root target is never encoded as context"
        );
    }

    #[test]
    fn stored_positions_round_trip_and_impossible_shapes_are_rejected() {
        let mut state = thread(10);
        let root = state.create_root(comment(1)).unwrap();
        let first = state.create_reply(comment(2), root.id()).unwrap();
        let second = state.create_reply(comment(3), first.id()).unwrap();
        for placement in [root, first, second] {
            let stored = if placement.is_root() {
                StoredPosition::Root
            } else {
                StoredPosition::Reply {
                    root_id: placement.root_id().as_comment_id(),
                    reply_to: placement.reply_to(),
                }
            };
            assert_eq!(
                CommentPlacement::from_stored(placement.id(), state.id(), stored),
                Ok(placement)
            );
        }
        for (id, root_id, reply_to) in [
            (comment(5), comment(5), None),
            (comment(5), comment(1), Some(comment(5))),
            (comment(5), comment(1), Some(comment(1))),
        ] {
            assert_eq!(
                CommentPlacement::from_stored(
                    id,
                    state.id(),
                    StoredPosition::Reply { root_id, reply_to }
                ),
                Err(StoredPlacementError)
            );
        }
    }

    fn assert_structure(state: &DiscussionThread) {
        for (index, placement) in state.comments.iter().copied().enumerate() {
            assert_eq!(placement.thread_id(), state.id());
            assert_eq!(state.comment(placement.id()), Some(placement));
            assert!(
                state.comments[..index]
                    .iter()
                    .all(|prior| prior.id() != placement.id())
            );
            let root = state.comment(placement.root_id().as_comment_id()).unwrap();
            assert!(root.is_root(), "structural parent must be a root");
            assert_eq!(root.root_id(), placement.root_id());
            if placement.is_root() {
                assert_eq!(root, placement);
                assert_eq!(placement.reply_to(), None);
            } else {
                assert_ne!(placement.id(), root.id());
                assert!(state.comments[..index].contains(&root));
                if let Some(context_id) = placement.reply_to() {
                    let context = state.comment(context_id).unwrap();
                    assert!(!context.is_root(), "direct root context must use None");
                    assert_eq!(context.root_id(), placement.root_id());
                    // Every context points backwards to an existing peer, so even contextual
                    // references cannot form a self-edge or cycle in this creation-only slice.
                    assert!(state.comments[..index].contains(&context));
                }
            }
        }
    }

    #[derive(Default)]
    struct Coverage {
        root: usize,
        direct: usize,
        contextual: usize,
        duplicate: usize,
        missing: usize,
        longest_context_chain: usize,
    }

    fn enumerate(state: &DiscussionThread, remaining: usize, coverage: &mut Coverage) {
        assert_structure(state);
        if remaining == 0 {
            return;
        }
        for id in 1..=4 {
            // 0 means create a root; 1..4 are reply targets. This enumerates all 20
            // commands at each step, including rejected attempts, not just valid paths.
            for target in 0..=4 {
                let mut next = state.clone();
                let outcome = if target == 0 {
                    next.create_root(comment(id))
                } else {
                    next.create_reply(comment(id), comment(target))
                };
                match outcome {
                    Ok(placement) => {
                        assert_eq!(next.comments.len(), state.comments.len() + 1);
                        assert_eq!(
                            &next.comments[..state.comments.len()],
                            state.comments.as_slice()
                        );
                        if placement.is_root() {
                            coverage.root += 1;
                        } else if placement.reply_to().is_none() {
                            coverage.direct += 1;
                        } else {
                            coverage.contextual += 1;
                        }
                        let mut depth = 0;
                        let mut cursor = placement;
                        while let Some(context) = cursor.reply_to() {
                            depth += 1;
                            cursor = next.comment(context).unwrap();
                        }
                        coverage.longest_context_chain = coverage.longest_context_chain.max(depth);
                    }
                    Err(error) => {
                        assert_eq!(&next, state, "rejection must be atomic");
                        match error {
                            StructureError::CommentExists => coverage.duplicate += 1,
                            StructureError::TargetNotFound => coverage.missing += 1,
                        }
                    }
                }
                enumerate(&next, remaining - 1, coverage);
            }
        }
    }

    #[test]
    fn every_four_step_sequence_preserves_two_tiers_and_immutable_placement() {
        // 4 identities, 20 commands per step, 20^4 complete sequences (168,420 edges).
        // This is finite enumeration of real Rust code, not Kani, Quint or a database proof.
        let mut coverage = Coverage::default();
        enumerate(&thread(10), 4, &mut coverage);
        assert!(coverage.root > 0);
        assert!(coverage.direct > 0);
        assert!(coverage.contextual > 0);
        assert!(coverage.duplicate > 0);
        assert!(coverage.missing > 0);
        assert_eq!(coverage.longest_context_chain, 2);
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Conformance {
        format: String,
        quint_version: String,
        scenarios: Vec<Scenario>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Scenario {
        name: String,
        steps: Vec<Step>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Step {
        command: Command,
        outcome: ExpectedOutcome,
        state: Vec<ExpectedPlacement>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Command {
        op: Operation,
        id: u128,
        target: u128,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Operation {
        Root,
        Reply,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    enum ExpectedOutcome {
        Created,
        Duplicate,
        Missing,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct ExpectedPlacement {
        id: u128,
        root: u128,
        reply_to: Option<u128>,
    }

    #[test]
    fn quint_generated_inputs_replay_against_the_production_creation_seam() {
        let fixture: Conformance =
            serde_json::from_str(include_str!("../../behavior/discussion/fixtures.json")).unwrap();
        let tool: serde_json::Value =
            serde_json::from_str(include_str!("../../behavior/discussion/package.json")).unwrap();
        assert_eq!(fixture.format, "vink-discussion-conformance-v1");
        assert_eq!(
            fixture.quint_version,
            tool["dependencies"]["@informalsystems/quint"]
        );
        assert!(!fixture.scenarios.is_empty());
        let mut seen = [false; 5];
        for scenario in fixture.scenarios {
            assert!(!scenario.name.is_empty());
            assert!(!scenario.steps.is_empty());
            let mut state = thread(10);
            for (index, step) in scenario.steps.into_iter().enumerate() {
                // Only model INPUTS go into the production seam. The generated state is used
                // solely as an oracle below; no expected placement initializes Rust state.
                let result = match step.command.op {
                    Operation::Root => {
                        assert_eq!(step.command.target, 0);
                        state.create_root(comment(step.command.id))
                    }
                    Operation::Reply => {
                        state.create_reply(comment(step.command.id), comment(step.command.target))
                    }
                };
                let outcome = match result {
                    Ok(placement) => {
                        let kind = if placement.is_root() {
                            0
                        } else if placement.reply_to().is_none() {
                            1
                        } else {
                            2
                        };
                        seen[kind] = true;
                        ExpectedOutcome::Created
                    }
                    Err(StructureError::CommentExists) => {
                        seen[3] = true;
                        ExpectedOutcome::Duplicate
                    }
                    Err(StructureError::TargetNotFound) => {
                        seen[4] = true;
                        ExpectedOutcome::Missing
                    }
                };
                assert_eq!(outcome, step.outcome, "{} step {index}", scenario.name);
                let mut observed: Vec<_> = state
                    .comments
                    .iter()
                    .map(|placement| ExpectedPlacement {
                        id: placement.id().as_uuid().as_u128(),
                        root: placement.root_id().as_comment_id().as_uuid().as_u128(),
                        reply_to: placement.reply_to().map(|id| id.as_uuid().as_u128()),
                    })
                    .collect();
                observed.sort_by_key(|placement| placement.id);
                assert_eq!(observed, step.state, "{} step {index}", scenario.name);
                assert_structure(&state);
            }
        }
        assert!(
            seen.into_iter().all(|covered| covered),
            "non-vacuous replay coverage"
        );
    }
}
