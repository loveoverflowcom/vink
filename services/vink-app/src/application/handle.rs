//! The account owner and the public resolver expose disjoint facts.
use super::ports::StoreError;
use crate::domain::handle::Intent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct OwnHandle {
    pub principal_id: Uuid,
    pub pen_name: String,
    pub handle: String,
    pub setup_complete: bool,
    pub remaining_changes: i16,
    pub version: i64,
    pub aliases: Vec<String>,
    pub has_public_works: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandleCommand {
    /// A precondition for a tab whose session cookie changed; never grants authority.
    pub principal_id: Uuid,
    pub command_id: Uuid,
    pub expected_version: i64,
    pub intent: Intent,
    pub handle: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct HandleReceipt {
    pub command_id: Uuid,
    pub accepted_version: i64,
    pub handle: String,
    pub remaining_changes: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorRef {
    pub pen_name: String,
    pub handle: String,
    pub href: String,
}

impl AuthorRef {
    pub(crate) fn new(pen_name: String, handle: String) -> Self {
        Self {
            href: Self::href_for(&handle),
            pen_name,
            handle,
        }
    }

    pub(crate) fn href_for(handle: &str) -> String {
        format!("/@{handle}")
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Resolution {
    Current {
        author: AuthorRef,
        #[serde(skip_serializing_if = "Option::is_none")]
        bio: Option<String>,
        publications: super::profile::ProfileWorks,
        has_public_works: bool,
        #[serde(
            default,
            skip_serializing_if = "super::follow::PublicRelationships::is_empty"
        )]
        relationships: Box<super::follow::PublicRelationships>,
    },
    Alias {
        href: String,
    },
    Gone,
    Missing,
}

#[derive(Debug)]
pub enum HandleError {
    Store(StoreError),
    Missing,
    WrongOwner,
    Invalid,
    Taken,
    OwnAlias,
    Exhausted,
    Stale(i64),
    KeyReused,
}
impl From<StoreError> for HandleError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
