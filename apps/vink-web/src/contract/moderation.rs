//! Browser-owned representations of the private moderation API.
use super::publication::{AuthorPublicationState, Decision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Appeal {
    pub id: String,
    pub work_id: String,
    pub revision_id: String,
    pub decision_version: i64,
    pub state: String,
    pub notice: String,
    pub resolution_reason: Option<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Filed {
    pub appeal: Appeal,
    pub replayed: bool,
}

/// The owner's outcome and permitted reason for one concluded decision.
#[derive(Clone, Debug, Deserialize)]
pub struct AuthorWorkDecision {
    pub revision_id: String,
    pub decision_version: i64,
    pub outcome: Decision,
    pub is_current: bool,
    #[serde(default)]
    pub action_required: bool,
    pub can_appeal: bool,
    pub reason: Option<String>,
    pub decided_at: String,
}

/// The Desk detail reads these facts together from one snapshot.
#[derive(Clone, Debug, Deserialize)]
pub struct AuthorWorkReview {
    pub state: AuthorPublicationState,
    pub appeals: Vec<Appeal>,
    pub decisions: Vec<AuthorWorkDecision>,
}

#[cfg(test)]
#[test]
fn decision_history_preserves_the_previous_outcome() {
    let review: AuthorWorkReview = serde_json::from_str(include_str!(
        "../../../../services/vink-app/contracts/http/v1/fixtures/publication.review.json"
    ))
    .unwrap();
    let [current, previous] = review.decisions.as_slice() else {
        panic!("expected two decision versions")
    };
    assert!(current.is_current && !previous.is_current);
    assert_eq!(current.revision_id, previous.revision_id);
    assert_eq!(
        (current.decision_version, current.outcome),
        (3, Decision::Approved)
    );
    assert_eq!(
        (previous.decision_version, previous.outcome),
        (2, Decision::Rejected)
    );
    assert_eq!(previous.reason.as_deref(), Some("Cần xem lại chính sách"));
    assert_eq!(review.state.revisions[0].decision_version, Some(3));
    assert_eq!(
        review.appeals[0].decision_version,
        previous.decision_version
    );
    assert_eq!(review.appeals[0].state, "reversed");
}
