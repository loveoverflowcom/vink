//! Browser representations of the Notifications Gateway contract.
use serde::Deserialize;
#[cfg(feature = "browser")]
use serde::Serialize;

#[cfg(feature = "browser")]
pub(super) const INBOX: &str = "/api/v1/me/notifications";

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Count {
    pub freshness: Freshness,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum Freshness {
    Fresh {
        unseen_group_count: u32,
        count_is_lower_bound: bool,
    },
    Stale {
        last_known_count: u32,
        count_is_lower_bound: bool,
    },
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct Page {
    pub inbox_snapshot: String,
    pub items: Vec<Item>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Item {
    pub group_id: String,
    pub group_version: i64,
    pub is_read: bool,
    pub actor_count: usize,
    pub actors_preview: Vec<Author>,
    pub target: Target,
    pub summary: Summary,
    pub excerpt: Option<Excerpt>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Author {
    Linked { pen_name: String, handle: String },
    Deleted { label: String },
    Unavailable { label: String },
}

impl Author {
    pub fn label(&self) -> &str {
        match self {
            Self::Linked { pen_name, .. } => pen_name,
            Self::Deleted { label } | Self::Unavailable { label } => label,
        }
    }

    pub fn handle(&self) -> Option<&str> {
        match self {
            Self::Linked { handle, .. } => Some(handle),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Summary {
    pub kind: String,
    pub action_required: bool,
    pub reply_to_viewer: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Excerpt {
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Target {
    WorkEndorsement {
        work_id: String,
        revision_id: String,
        work_title: String,
    },
    WorkCommentThread {
        work_id: String,
        comment_id: String,
        work_title: String,
    },
    WorkCommentReply {
        work_id: String,
        comment_id: String,
        work_title: String,
    },
    WorkCommentEndorsement {
        work_id: String,
        comment_id: String,
        work_title: String,
    },
    WorkModerationDecision {
        work_id: String,
        revision_id: String,
        decision_version: i64,
        work_title: String,
        revision_created_at: Option<String>,
    },
    CommentModerationDecision {
        comment_id: String,
        moderation_version: i64,
        work_title: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

pub(super) fn segment(value: &str) -> String {
    const ID: percent_encoding::AsciiSet =
        percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_');
    percent_encoding::utf8_percent_encode(value, &ID).to_string()
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Reason {
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct CommentDecision {
    pub work_title: Option<String>,
    pub outcome: String,
    pub reason: Reason,
    pub is_current: bool,
    pub decided_at: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Endorsers {
    pub items: Vec<Endorser>,
    pub next_cursor: Option<String>,
    pub total_count: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Endorser {
    pub author: Author,
}

#[cfg(feature = "browser")]
#[derive(Clone, Debug, Serialize)]
pub(super) struct SnapshotCommand {
    pub command_key: String,
    pub up_to_snapshot: String,
}

#[cfg(feature = "browser")]
#[derive(Debug, Serialize)]
pub(super) struct ReadCommand {
    pub command_key: String,
    pub group_id: String,
    pub observed_group_version: i64,
}
