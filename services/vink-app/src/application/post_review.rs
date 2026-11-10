//! Operator-only re-review use cases. The database credential is the authority, as for catalog
//! governance; a bounded audit note must identify the operator and reason. No HTTP constructor.
use uuid::Uuid;

use super::Vink;
use super::ports::{PostReviewStore, StoreError};
use crate::domain::discussion::admission::PostReviewFinding;
use crate::domain::discussion::lifecycle::ModerationVersion;
use crate::domain::discussion::moderation::GrantNote;
use crate::domain::ids::CommentId;

#[derive(Debug, Clone, Copy)]
pub struct ReviewAuthority(());
impl ReviewAuthority {
    pub const fn operator() -> Self {
        Self(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestReview {
    pub key: Uuid,
    pub comment: CommentId,
    pub expected: ModerationVersion,
    pub note: GrantNote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewResult {
    pub ticket: Uuid,
    pub key: Uuid,
    pub finding: PostReviewFinding,
    pub note: GrantNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewWrite {
    Accepted(Uuid),
    Replayed(Uuid),
    Conflict,
    Refused,
    Disabled,
    NotFound,
}

impl<S: PostReviewStore> Vink<S> {
    pub async fn request_review(
        &self,
        authority: ReviewAuthority,
        enabled: bool,
        request: &RequestReview,
    ) -> Result<ReviewWrite, StoreError> {
        self.store.request_review(authority, enabled, request).await
    }
    pub async fn record_review_result(
        &self,
        authority: ReviewAuthority,
        result: &ReviewResult,
    ) -> Result<ReviewWrite, StoreError> {
        self.store.record_review_result(authority, result).await
    }
}
