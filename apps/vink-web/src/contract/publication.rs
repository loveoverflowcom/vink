//! Browser-owned representations of the author's publication API (PUB-AUTHOR-WEB-01): the one
//! place the web app decodes `GET …/publication/state` and encodes the three commands. The
//! provider fixtures in `services/vink-app/contracts/http/v1/fixtures` are decoded by the tests
//! below, so a provider change fails this consumer.
//!
//! Enum fields keep an `Unknown` variant. A value this build does not know is never read as
//! "private", "approved" or "empty": the view reports a safe message and reads again.
use serde::{Deserialize, Serialize};

/// `audience` of the author's state: exactly the two audiences of AUD-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    Private,
    Public,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Active,
    AuthorDeleted,
    Removed,
    #[serde(other)]
    Unknown,
}

/// The decision a revision carries. `Unsubmitted` is a captured snapshot nobody asked to
/// publish; `NeedsAdmin` is a hand-over to people, neither "AI is reviewing" nor a rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Unsubmitted,
    Pending,
    Approved,
    Rejected,
    NeedsAdmin,
    #[serde(other)]
    Unknown,
}

/// `GET /api/v1/works/{work_id}/publication/state`: the author's own, `no-store` view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorPublicationState {
    pub work_id: String,
    pub audience: Audience,
    pub lifecycle: Lifecycle,
    /// The fence of `request-public` and `set-private` (`expected_generation`).
    pub generation: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<PublishIntent>,
    /// The revision readers see now; absent when the work is not public.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<PublicPointer>,
    /// Oldest first (`created_at`, then id). The page never depends on the order.
    pub revisions: Vec<AuthorRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_ticket: Option<OpenTicket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishIntent {
    pub revision_id: String,
    pub generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicPointer {
    pub revision_id: String,
    pub intent_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorRevision {
    pub can_appeal: bool,
    pub decision_reason: Option<String>,
    pub revision_id: String,
    pub title: String,
    pub content_hash: String,
    pub moderation: Decision,
    /// The version an appeal must quote; the appeal page reads it again from the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_snapshot_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenTicket {
    pub evaluation_id: String,
    pub revision_id: String,
    pub policy_version: i64,
}

/// `POST …/snapshots`: freezes the saved draft and its saved metadata as one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSnapshotRequest {
    pub command_key: String,
    pub expected_draft_version: i64,
    pub expected_metadata_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedSnapshot {
    pub outcome: String,
    pub revision_id: String,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_snapshot_id: Option<String>,
    pub draft_version: i64,
    pub metadata_version: i64,
    /// `true`: this answer replays the first one for the same key; nothing new was written.
    pub replayed: bool,
}

/// `POST …/publication/request-public`: asks to publish exactly one captured revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPublicRequest {
    pub command_key: String,
    pub revision_id: String,
    pub expected_generation: i64,
}

/// The receipt of one command. It is history, not the work's current state: a replay may omit
/// `ticket_id`, and the page always reads the state again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPublicReceipt {
    pub outcome: String,
    pub revision_id: String,
    pub generation: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket_id: Option<String>,
    pub replayed: bool,
}

/// `POST …/publication/set-private`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetPrivateRequest {
    pub command_key: String,
    pub expected_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetPrivateReceipt {
    pub outcome: String,
    pub generation: i64,
    pub replayed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    fn fixture(name: &str) -> serde_json::Value {
        let path = format!(
            "{}/../../services/vink-app/contracts/http/v1/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        serde_json::from_str(&raw).unwrap()
    }

    /// Decodes a provider fixture and checks that this representation keeps every field.
    fn round_trip<T: DeserializeOwned + Serialize>(name: &str) -> T {
        let json = fixture(name);
        let value: T = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&value).unwrap(), json, "{name}");
        value
    }

    #[test]
    fn the_whole_author_state_is_read() {
        let state: AuthorPublicationState =
            round_trip("publication.state.public_r1_pending_r2.json");
        assert_eq!(
            (state.audience, state.lifecycle),
            (Audience::Public, Lifecycle::Active)
        );
        assert_eq!(state.generation, 4);
        assert_eq!(state.intent.as_ref().map(|i| i.generation), Some(4));
        assert_eq!(
            state.pointer.as_ref().map(|p| p.revision_id.as_str()),
            Some("0192f3a1-0000-7000-8000-0000000000b1")
        );
        assert_eq!(state.revisions.len(), 2);
        assert_eq!(state.revisions[0].moderation, Decision::Approved);
        assert_eq!(state.revisions[1].moderation, Decision::Pending);
        assert!(state.open_ticket.is_some());
        let rejected: AuthorPublicationState = round_trip("publication.state.public_rejected.json");
        assert!(rejected.pointer.is_none() && rejected.open_ticket.is_none());
        assert!(rejected.revisions[0].can_appeal);
        assert_eq!(rejected.revisions[0].decision_version, Some(2));
        let private: AuthorPublicationState = round_trip("publication.state.private.json");
        assert_eq!(private.audience, Audience::Private);
        assert!(private.revisions.is_empty());
    }

    #[test]
    fn commands_and_receipts_keep_every_field() {
        let _: CaptureSnapshotRequest = round_trip("snapshot.capture.request.json");
        let captured: CapturedSnapshot = round_trip("snapshot.capture.response.json");
        assert!(!captured.replayed);
        let _: RequestPublicRequest = round_trip("publication.request_public.request.json");
        let first: RequestPublicReceipt = round_trip("publication.request_public.response.json");
        assert!(first.ticket_id.is_some());
        // A replay carries no ticket: that is an answer, not a malformed one.
        let replay: RequestPublicReceipt =
            round_trip("publication.request_public.response.replayed.json");
        assert!(replay.replayed && replay.ticket_id.is_none());
        let _: SetPrivateRequest = round_trip("publication.set_private.request.json");
        let receipt: SetPrivateReceipt = round_trip("publication.set_private.response.json");
        assert_eq!(receipt.generation, 5);
    }

    #[test]
    fn a_value_this_build_does_not_know_is_unknown_not_a_known_state() {
        let mut state = fixture("publication.state.public_r1_pending_r2.json");
        state["audience"] = "unlisted".into();
        state["lifecycle"] = "archived".into();
        state["revisions"][1]["moderation"] = "escalated".into();
        let state: AuthorPublicationState = serde_json::from_value(state).unwrap();
        assert_eq!(state.audience, Audience::Unknown);
        assert_eq!(state.lifecycle, Lifecycle::Unknown);
        assert_eq!(state.revisions[1].moderation, Decision::Unknown);
    }
}
