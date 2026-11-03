//! Submitting a root or reply: the single production decision for D2T-INV-01/02/03/06.
//!
//! The adapter loads [`SubmissionFacts`] inside the write transaction, under locks that serialize
//! it with mode changes, moderation and withdrawal, then calls [`decide_submission`] and performs
//! exactly the write it returns. The client supplies only intent: an optional target ID, text and
//! a command key. Actor, thread, root, context and comment ID come from the server.
//!
//! Idempotency (D2T-INV-06): a receipt binds (actor, operation, key) to the digest of the
//! normalized payload. The same key and payload replays the first result even if the thread was
//! closed meanwhile; the same key with another payload is refused. A timeout is an uncertain
//! outcome: clients retry with the same key, never with a fresh one.

use sha2::{Digest, Sha256};

use super::body::CommentBody;
use super::lifecycle::CommentState;
use super::settings::DiscussionSettings;
use super::visibility::can_reply_to;
use super::{CommentPlacement, StructureError, reply_placement, root_placement};
use crate::domain::ids::{CommentId, ThreadId, WorkId};

pub const MIN_KEY_CHARS: usize = 8;
pub const MAX_KEY_CHARS: usize = 128;

/// Client-chosen idempotency key, scoped by the server to the acting principal and operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "command key must be {MIN_KEY_CHARS}-{MAX_KEY_CHARS} characters of A-Z, a-z, 0-9, '-' or '_'"
)]
pub struct CommandKeyError;

impl CommandKey {
    pub fn parse(raw: &str) -> Result<Self, CommandKeyError> {
        let valid = (MIN_KEY_CHARS..=MAX_KEY_CHARS).contains(&raw.len())
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if valid {
            Ok(Self(raw.to_owned()))
        } else {
            Err(CommandKeyError)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// SHA-256 of a command's canonical payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadDigest([u8; 32]);

impl PayloadDigest {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok().map(Self)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Canonical, unambiguous encoding: a versioned tag then length-delimited fields. The body is
    /// already NFC-normalized, so NFC and NFD spellings of the same text share a digest.
    pub fn of_submission(work_id: WorkId, target: Option<CommentId>, body: &CommentBody) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"vink.discussion.submit.v1\0");
        hasher.update(work_id.as_uuid().as_bytes());
        match target {
            None => hasher.update([0]),
            Some(target) => {
                hasher.update([1]);
                hasher.update(target.as_uuid().as_bytes());
            }
        }
        let text = body.as_str().as_bytes();
        hasher.update((text.len() as u64).to_be_bytes());
        hasher.update(text);
        Self(hasher.finalize().into())
    }
}

/// A stored receipt of an earlier, committed submission with the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    pub digest: PayloadDigest,
    pub comment_id: CommentId,
}

/// The requested target as loaded by the server inside the submitting thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFacts {
    pub placement: CommentPlacement,
    pub state: CommentState,
}

/// Facts the adapter reads in the write transaction. `target` is `None` when the request named
/// no target or the target is not a comment of this work's thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmissionFacts {
    pub receipt: Option<Receipt>,
    pub work_public: bool,
    pub settings: DiscussionSettings,
    pub target: Option<TargetFacts>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionDecision {
    /// Write this placement as a new pending comment together with the receipt.
    Insert(CommentPlacement),
    /// Same key and payload already committed: return that comment, write nothing.
    Replay(CommentId),
    /// Same key was used for a different payload: write nothing.
    KeyReused,
    /// The work has no current public publication.
    WorkNotPublic,
    /// The work's author closed comments.
    CommentsClosed,
    /// The target is missing, foreign or not public.
    TargetUnavailable,
    /// The server-chosen identity collides with the target; a server defect, never client input.
    IdentityCollision,
}

pub fn decide_submission(
    thread_id: ThreadId,
    comment_id: CommentId,
    digest: PayloadDigest,
    requested_target: Option<CommentId>,
    facts: SubmissionFacts,
) -> SubmissionDecision {
    if let Some(receipt) = facts.receipt {
        return if receipt.digest == digest {
            SubmissionDecision::Replay(receipt.comment_id)
        } else {
            SubmissionDecision::KeyReused
        };
    }
    if !facts.work_public {
        return SubmissionDecision::WorkNotPublic;
    }
    if !facts.settings.accepts_new_comments() {
        return SubmissionDecision::CommentsClosed;
    }
    let Some(requested) = requested_target else {
        return SubmissionDecision::Insert(root_placement(thread_id, comment_id));
    };
    let Some(target) = facts
        .target
        .filter(|target| target.placement.id() == requested && can_reply_to(target.state))
    else {
        return SubmissionDecision::TargetUnavailable;
    };
    match reply_placement(thread_id, comment_id, target.placement) {
        Ok(placement) => SubmissionDecision::Insert(placement),
        Err(StructureError::TargetNotFound) => SubmissionDecision::TargetUnavailable,
        Err(StructureError::CommentExists) => SubmissionDecision::IdentityCollision,
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::discussion::lifecycle::{Decision, ModerationVersion, Review, Transition};
    use crate::domain::discussion::settings::{CommentsMode, SettingsGeneration};

    fn comment(id: u128) -> CommentId {
        CommentId::from_uuid(Uuid::from_u128(id))
    }

    fn thread(id: u128) -> ThreadId {
        ThreadId::from_uuid(Uuid::from_u128(id))
    }

    fn work() -> WorkId {
        WorkId::from_uuid(Uuid::from_u128(77))
    }

    fn body(text: &str) -> CommentBody {
        CommentBody::parse(text).unwrap()
    }

    fn approved() -> CommentState {
        match CommentState::SUBMITTED.decide(ModerationVersion::FIRST, Decision::Approve) {
            Ok(Transition::Changed(state)) => state,
            other => panic!("{other:?}"),
        }
    }

    fn open() -> SubmissionFacts {
        SubmissionFacts {
            receipt: None,
            work_public: true,
            settings: DiscussionSettings::INITIAL,
            target: None,
        }
    }

    fn digest() -> PayloadDigest {
        PayloadDigest::of_submission(work(), None, &body("Hay quá"))
    }

    #[test]
    fn keys_are_bounded_ascii_tokens() {
        assert!(CommandKey::parse("01J9Z-abc_def").is_ok());
        assert!(CommandKey::parse(&"k".repeat(MAX_KEY_CHARS)).is_ok());
        for bad in [
            "short",
            "has space x",
            "dấu-tiếng-việt",
            "../../etc",
            &"k".repeat(MAX_KEY_CHARS + 1),
        ] {
            assert_eq!(CommandKey::parse(bad), Err(CommandKeyError), "{bad}");
        }
    }

    #[test]
    fn digests_distinguish_every_payload_field_but_not_normalization() {
        let base = PayloadDigest::of_submission(work(), None, &body("Hay quá"));
        assert_eq!(
            base,
            PayloadDigest::of_submission(work(), None, &body("Hay qua\u{0301}"))
        );
        let other_work = WorkId::from_uuid(Uuid::from_u128(78));
        for different in [
            PayloadDigest::of_submission(other_work, None, &body("Hay quá")),
            PayloadDigest::of_submission(work(), Some(comment(1)), &body("Hay quá")),
            PayloadDigest::of_submission(work(), None, &body("Hay quá!")),
        ] {
            assert_ne!(base, different);
        }
        assert_eq!(PayloadDigest::from_bytes(base.as_bytes()), Some(base));
        assert_eq!(PayloadDigest::from_bytes(&[0; 31]), None);
    }

    #[test]
    fn a_receipt_replays_the_same_payload_even_after_comments_close() {
        let closed =
            DiscussionSettings::restore(CommentsMode::Closed, SettingsGeneration::new(2).unwrap());
        let facts = SubmissionFacts {
            receipt: Some(Receipt {
                digest: digest(),
                comment_id: comment(5),
            }),
            work_public: false,
            settings: closed,
            target: None,
        };
        assert_eq!(
            decide_submission(thread(1), comment(9), digest(), None, facts),
            SubmissionDecision::Replay(comment(5))
        );
        let other = PayloadDigest::of_submission(work(), None, &body("Khác"));
        assert_eq!(
            decide_submission(thread(1), comment(9), other, None, facts),
            SubmissionDecision::KeyReused
        );
    }

    #[test]
    fn withdrawn_works_and_closed_threads_accept_nothing_new() {
        let not_public = SubmissionFacts {
            work_public: false,
            ..open()
        };
        assert_eq!(
            decide_submission(thread(1), comment(9), digest(), None, not_public),
            SubmissionDecision::WorkNotPublic
        );
        let closed = SubmissionFacts {
            settings: DiscussionSettings::restore(
                CommentsMode::Closed,
                SettingsGeneration::new(2).unwrap(),
            ),
            ..open()
        };
        assert_eq!(
            decide_submission(thread(1), comment(9), digest(), None, closed),
            SubmissionDecision::CommentsClosed
        );
    }

    #[test]
    fn replies_normalize_to_the_targets_root_and_need_a_public_target() {
        let root = root_placement(thread(1), comment(1));
        let reply = reply_placement(thread(1), comment(2), root).unwrap();
        let with_target = |placement, state| SubmissionFacts {
            target: Some(TargetFacts { placement, state }),
            ..open()
        };

        let decision = decide_submission(
            thread(1),
            comment(9),
            digest(),
            Some(reply.id()),
            with_target(reply, approved()),
        );
        let SubmissionDecision::Insert(placement) = decision else {
            panic!("{decision:?}")
        };
        assert_eq!(placement.root_id(), root.root_id());
        assert_eq!(placement.reply_to(), Some(reply.id()));

        for state in [
            CommentState::SUBMITTED,
            CommentState::restore(Review::Rejected, None, ModerationVersion::FIRST),
            match approved().remove_by_author() {
                Ok(Transition::Changed(state)) => state,
                other => panic!("{other:?}"),
            },
        ] {
            assert_eq!(
                decide_submission(
                    thread(1),
                    comment(9),
                    digest(),
                    Some(reply.id()),
                    with_target(reply, state)
                ),
                SubmissionDecision::TargetUnavailable
            );
        }
        // Missing target, a target fact for another ID, and a target from another thread.
        assert_eq!(
            decide_submission(thread(1), comment(9), digest(), Some(comment(4)), open()),
            SubmissionDecision::TargetUnavailable
        );
        assert_eq!(
            decide_submission(
                thread(1),
                comment(9),
                digest(),
                Some(comment(4)),
                with_target(reply, approved())
            ),
            SubmissionDecision::TargetUnavailable
        );
        let foreign = root_placement(thread(2), comment(4));
        assert_eq!(
            decide_submission(
                thread(1),
                comment(9),
                digest(),
                Some(foreign.id()),
                with_target(foreign, approved())
            ),
            SubmissionDecision::TargetUnavailable
        );
        assert_eq!(
            decide_submission(
                thread(1),
                root.id(),
                digest(),
                Some(root.id()),
                with_target(root, approved())
            ),
            SubmissionDecision::IdentityCollision
        );
    }

    #[test]
    fn decisions_are_exhaustive_over_the_fact_space() {
        // Every combination of receipt × publication × mode × target presence/state: exactly
        // one decision, and an Insert only when all gates pass. Guards the precedence order.
        let root = root_placement(thread(1), comment(1));
        let states = [CommentState::SUBMITTED, approved()];
        let modes = [
            DiscussionSettings::INITIAL,
            DiscussionSettings::restore(CommentsMode::Closed, SettingsGeneration::new(2).unwrap()),
        ];
        let mut inserts = 0;
        for receipt in [
            None,
            Some(Receipt {
                digest: digest(),
                comment_id: comment(5),
            }),
        ] {
            for work_public in [false, true] {
                for settings in modes {
                    for target in [None, Some(states[0]), Some(states[1])] {
                        for requested in [None, Some(root.id())] {
                            let facts = SubmissionFacts {
                                receipt,
                                work_public,
                                settings,
                                target: target.map(|state| TargetFacts {
                                    placement: root,
                                    state,
                                }),
                            };
                            let decision = decide_submission(
                                thread(1),
                                comment(9),
                                digest(),
                                requested,
                                facts,
                            );
                            let gates_pass = receipt.is_none()
                                && work_public
                                && settings.accepts_new_comments()
                                && (requested.is_none()
                                    || target.is_some_and(|state| state.is_public()));
                            assert_eq!(
                                matches!(decision, SubmissionDecision::Insert(_)),
                                gates_pass,
                                "{facts:?} {requested:?} -> {decision:?}"
                            );
                            inserts += usize::from(gates_pass);
                        }
                    }
                }
            }
        }
        assert!(inserts > 0);
    }
}
