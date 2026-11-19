//! Application DTOs and error types for the Social Follow Graph (#294).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::handle::AuthorRef;
use super::ports::StoreError;
use crate::domain::follow::{FollowDirection, can_read_count, can_read_list};
use crate::domain::profile::ProfilePolicies;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PublicRelationships {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followers_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub following_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followers_href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub following_href: Option<String>,
}

impl PublicRelationships {
    pub fn is_empty(&self) -> bool {
        self.followers_count.is_none()
            && self.following_count.is_none()
            && self.followers_href.is_none()
            && self.following_href.is_none()
    }

    pub fn from_policies(
        policies: ProfilePolicies,
        handle: &str,
        followers_count: Option<i64>,
        following_count: Option<i64>,
    ) -> Self {
        Self {
            followers_count: can_read_count(policies, FollowDirection::Followers, false)
                .then_some(followers_count)
                .flatten(),
            following_count: can_read_count(policies, FollowDirection::Following, false)
                .then_some(following_count)
                .flatten(),
            followers_href: can_read_list(policies, FollowDirection::Followers, false)
                .then(|| format!("/@{handle}/{}", FollowDirection::Followers.web_segment())),
            following_href: can_read_list(policies, FollowDirection::Following, false)
                .then(|| format!("/@{handle}/{}", FollowDirection::Following.web_segment())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnRelationshipsSummary {
    pub followers_count: i64,
    pub following_count: i64,
    pub followers_href: String,
    pub following_href: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FollowToken {
    pub following: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FollowObserved {
    pub target_handle: String,
    pub following: bool,
    pub mutual: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetFollowInput {
    pub command_id: Uuid,
    pub expected_order: i64,
    pub following: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetFollowResponse {
    pub target_handle: String,
    pub outcome: &'static str,
    pub following: bool,
    pub mutual: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed: Option<FollowToken>,
    #[serde(skip_serializing_if = "PublicRelationships::is_empty")]
    pub relationships: PublicRelationships,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FollowReceiptView {
    pub command_id: Uuid,
    pub target_handle: String,
    pub expected_order: i64,
    pub desired_following: bool,
    pub committed: FollowToken,
    pub observed: FollowObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewerRelationship {
    #[serde(rename = "self")]
    SelfProfile {
        target_handle: String,
        can_follow: bool,
        own_relationships: OwnRelationshipsSummary,
    },
    Other {
        target_handle: String,
        can_follow: bool,
        following: bool,
        mutual: bool,
        pair_version: i64,
        accepted_order: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PublicRelationshipList {
    Page {
        author: AuthorRef,
        direction: FollowDirection,
        #[serde(skip_serializing_if = "Option::is_none")]
        count: Option<i64>,
        people: Vec<AuthorRef>,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
    Restart {
        author: AuthorRef,
        direction: FollowDirection,
    },
    Alias {
        href: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnRelationshipPerson {
    pub author: AuthorRef,
    pub mutual: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OwnRelationshipList {
    Page {
        author: AuthorRef,
        direction: FollowDirection,
        count: i64,
        people: Vec<OwnRelationshipPerson>,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
    Restart {
        author: AuthorRef,
        direction: FollowDirection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MembershipItem {
    pub handle: String,
    pub following: bool,
    pub mutual: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MembershipBatch {
    pub items: Vec<MembershipItem>,
}

#[derive(Debug)]
pub enum FollowError {
    Store(StoreError),
    Invalid(&'static str),
    NotFound,
    Gone,
    ActorUnavailable,
    TargetUnavailable,
    ListPrivate,
    SelfFollow,
    Conflict(FollowObserved),
    KeyReused,
}

impl From<StoreError> for FollowError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
