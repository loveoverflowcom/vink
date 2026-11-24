//! Browser-owned representations of the private moderation API.
use super::DocumentDto;
use super::publication::{AuthorPublicationState, Decision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Case {
    #[serde(default)]
    pub viewer_is_admin: bool,
    pub id: String,
    pub work_id: String,
    pub revision_id: Option<String>,
    pub decision_version: i64,
    pub version: i64,
    pub state: String,
    pub origin: String,
    pub claimed_by: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub document: Option<DocumentDto>,
    #[serde(default)]
    pub notes: Vec<Note>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    pub note: String,
}
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
pub struct AppealDetail {
    pub appeal: Appeal,
    pub title: String,
    pub document: DocumentDto,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Automation {
    pub mode: String,
    pub version: i64,
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

/// Queue envelope, distinct from the author's unpaged appeal history.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QueuePage<T> {
    pub authority_key: String,
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
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

#[cfg(test)]
mod queue_tests {
    use super::*;
    #[test]
    fn moderation_queue_provider_fixtures_and_legacy_arrays_are_distinct() {
        let p: QueuePage<Case> = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/moderation.works.page.json"
        ))
        .unwrap();
        assert_eq!(p.items.len(), 1);
        assert!(p.next_cursor.is_some());
        assert!(serde_json::from_str::<Vec<Case>>(include_str!("../../../../services/vink-app/contracts/http/v1/fixtures/moderation.works.page.json")).is_err());
        let end: QueuePage<Case> = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/moderation.works.end.json"
        ))
        .unwrap();
        assert!(end.next_cursor.is_none());
        assert_eq!(end.items[0].id, p.items[0].id);
        let empty: QueuePage<Case> = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/moderation.queue.empty.json"
        ))
        .unwrap();
        assert!(empty.items.is_empty() && empty.next_cursor.is_none());
        let appeals: QueuePage<Appeal> = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/moderation.appeals.page.json"
        ))
        .unwrap();
        assert_eq!(appeals.items[0].notice, "not_sent");
    }
}
