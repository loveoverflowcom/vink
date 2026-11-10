//! Operator re-review is a new authority, independent of admission (DISC-POST-REVIEW-RUNTIME).
//! The adapter verifies the immutable envelope and loads these facts under its commit locks.
//! No result can approve, revive a removal, or certify a newer human decision as reviewed.

use super::admission::PostReviewFinding;
use super::lifecycle::{CommentState, LifecycleError, ModerationVersion, Transition};
use super::submission::PayloadDigest;
use sha2::{Digest, Sha256};

/// Digest of the stored immutable comment text, separate from admission/submission identity.
pub fn payload_digest(body: &str) -> PayloadDigest {
    let mut hash = Sha256::new();
    hash.update(b"vink.comment-review.v1\0");
    hash.update((body.len() as u64).to_be_bytes());
    hash.update(body.as_bytes());
    PayloadDigest::from_bytes(&hash.finalize()).expect("SHA-256 is 32 bytes")
}

pub const CONSUMER: &str = "operator_post_review.v1";
pub const EVENT: &str = "comment.rereview_requested";
pub const POLICY: &str = "operator-rereview.v1";
/// Operational bound for this explicit operator workflow; not an automatic moderation SLA.
pub const DEADLINE_SECONDS: i64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewBinding {
    pub version: ModerationVersion,
    pub payload: PayloadDigest,
    pub deadline_micros: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    AwaitResult,
    Keep,
    Hold(CommentState),
    Superseded,
    Removed,
    /// The adapter must quarantine the debt, without guessing a target or declaring it done.
    InvalidBinding,
}

pub fn decide_review(
    ticket: ReviewBinding,
    current: CommentState,
    payload: PayloadDigest,
    result: Option<PostReviewFinding>,
    now_micros: i64,
    exhausted: bool,
) -> Result<ReviewDecision, LifecycleError> {
    if ticket.payload != payload {
        return Ok(ReviewDecision::InvalidBinding);
    }
    if !current.is_present() {
        return Ok(ReviewDecision::Removed);
    }
    if ticket.version != current.version() || !current.is_public() {
        return Ok(ReviewDecision::Superseded);
    }
    let finding = if exhausted || now_micros >= ticket.deadline_micros {
        PostReviewFinding::Unresolved
    } else if let Some(result) = result {
        result
    } else {
        return Ok(ReviewDecision::AwaitResult);
    };
    match finding {
        PostReviewFinding::Clear => Ok(ReviewDecision::Keep),
        PostReviewFinding::Flagged | PostReviewFinding::Unresolved => {
            match current.hold_for_review(ticket.version)? {
                Transition::Changed(next) => Ok(ReviewDecision::Hold(next)),
                Transition::Unchanged(_) => Ok(ReviewDecision::Superseded),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::lifecycle::{Decision, Review};
    use super::*;

    fn binding() -> ReviewBinding {
        ReviewBinding {
            version: ModerationVersion::new(2).unwrap(),
            payload: PayloadDigest::from_bytes(&[7; 32]).unwrap(),
            deadline_micros: 100,
        }
    }

    #[test]
    fn clear_is_bound_and_exhaustion_never_means_completed_without_a_hold() {
        let ticket = binding();
        let current = CommentState::restore(Review::Approved, None, ticket.version);
        assert_eq!(
            decide_review(ticket, current, ticket.payload, None, 99, false),
            Ok(ReviewDecision::AwaitResult)
        );
        assert_eq!(
            decide_review(
                ticket,
                current,
                ticket.payload,
                Some(PostReviewFinding::Clear),
                99,
                false
            ),
            Ok(ReviewDecision::Keep)
        );
        for (now, exhausted) in [(100, false), (99, true)] {
            let Ok(ReviewDecision::Hold(held)) = decide_review(
                ticket,
                current,
                ticket.payload,
                Some(PostReviewFinding::Clear),
                now,
                exhausted,
            ) else {
                panic!("must hold");
            };
            assert_eq!(held.review(), Review::Pending);
            assert_eq!(held.version().get(), 3);
        }
        let Ok(Transition::Changed(human)) = current.decide(ticket.version, Decision::Approve)
        else {
            panic!("accepted reaffirmation must fence");
        };
        for result in [
            None,
            Some(PostReviewFinding::Clear),
            Some(PostReviewFinding::Flagged),
        ] {
            assert_eq!(
                decide_review(ticket, human, ticket.payload, result, 99, false),
                Ok(ReviewDecision::Superseded)
            );
        }
        let Ok(Transition::Changed(removed)) = current.remove_by_author() else {
            panic!("remove");
        };
        assert_eq!(
            decide_review(ticket, removed, ticket.payload, None, 100, true),
            Ok(ReviewDecision::Removed)
        );
        assert_eq!(
            decide_review(
                ticket,
                current,
                PayloadDigest::from_bytes(&[8; 32]).unwrap(),
                None,
                100,
                true
            ),
            Ok(ReviewDecision::InvalidBinding)
        );
    }
}
