//! Own profile commands and actor-free projections belong to Profile.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Serialize)]
pub struct ProfileWork {
    pub work_id: Uuid,
    pub title: String,
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
