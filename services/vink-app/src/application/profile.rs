//! Own profile commands and actor-free projections belong to Profile.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PublicWorkSummary;
use super::catalog::PublicTag;
use super::ports::StoreError;
use crate::domain::profile::ProfilePolicies;

#[derive(Debug, Serialize)]
pub struct OwnProfile {
    pub principal_id: Uuid,
    pub pen_name: String,
    pub handle: String,
    pub handle_setup_complete: bool,
    pub remaining_handle_changes: i16,
    pub profile_version: i64,
    pub bio: Option<String>,
    pub policies: ProfilePolicies,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveProfile {
    /// The principal of the originating form. The authenticated actor supplies authority.
    pub principal_id: Uuid,
    pub expected_version: i64,
    pub pen_name: String,
    /// Required key: `null` clears the Bio, an omitted key is invalid input rather than a
    /// silent deletion.
    #[serde(deserialize_with = "Option::deserialize")]
    pub bio: Option<String>,
    pub policies: ProfilePolicies,
}

/// One currently public work on an author's own profile (#587). The page names its author, so
/// no byline is repeated; no counter or private view count is projected. Optional facts are
/// omitted when the publication cannot establish them: a title-only reader keeps working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileWork {
    pub work_id: Uuid,
    pub title: String,
    /// Projected from the published revision only, as on a discovery card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    /// Present only for an exact first-publication fact (never a legacy pointer time).
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_published_at: Option<OffsetDateTime>,
    /// Active public tags of the bound metadata snapshot: form first, then themes.
    pub tags: Vec<PublicTag>,
}

impl From<PublicWorkSummary> for ProfileWork {
    /// Drops the author attribution and the pointer time: neither is part of the profile item.
    fn from(work: PublicWorkSummary) -> Self {
        Self {
            work_id: work.work_id.as_uuid(),
            title: work.title.as_str().to_owned(),
            excerpt: work.excerpt,
            first_published_at: work.first_published_at,
            tags: work.tags.iter().map(PublicTag::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProfileWorks {
    Page {
        works: Vec<ProfileWork>,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
    Restart,
}

#[derive(Debug)]
pub enum ProfileError {
    Store(StoreError),
    Missing,
    WrongOwner,
    Invalid(&'static str),
    DisclosurePending,
    Stale(i64),
}

impl From<StoreError> for ProfileError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

pub const DELETED_AUTHOR_LABEL: &str = "Tài khoản đã xóa";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorAttribution {
    Linked(super::handle::AuthorRef),
    Deleted,
    Unavailable(String),
}

impl AuthorAttribution {
    pub fn project(name: String, handle: Option<String>, retired: bool) -> Self {
        if retired {
            return Self::Deleted;
        }
        match handle {
            Some(handle) => Self::Linked(super::handle::AuthorRef::new(name, handle)),
            None => Self::Unavailable(name),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Linked(author) => &author.pen_name,
            Self::Deleted => DELETED_AUTHOR_LABEL,
            Self::Unavailable(label) => label,
        }
    }
}

impl From<String> for AuthorAttribution {
    fn from(label: String) -> Self {
        Self::Unavailable(label)
    }
}

impl From<&str> for AuthorAttribution {
    fn from(label: &str) -> Self {
        Self::Unavailable(label.to_owned())
    }
}
