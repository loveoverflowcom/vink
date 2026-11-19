//! HTTP v1 wire contracts for the Social Follow Graph (#294).

use serde::{Deserialize, Serialize};

use super::PublicAuthorRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowDirection {
    Followers,
    Following,
}

impl FollowDirection {
    pub const fn web_segment(self) -> &'static str {
        match self {
            Self::Followers => "nguoi-theo-doi",
            Self::Following => "dang-theo-doi",
        }
    }

    pub const fn api_segment(self) -> &'static str {
        match self {
            Self::Followers => "followers",
            Self::Following => "following",
        }
    }

    pub fn parse_web(raw: &str) -> Option<Self> {
        match raw {
            "nguoi-theo-doi" => Some(Self::Followers),
            "dang-theo-doi" => Some(Self::Following),
            _ => None,
        }
    }
}

pub fn valid_canonical_handle(raw: &str) -> bool {
    !raw.is_empty()
        && raw.len() <= 40
        && raw.split('-').all(|group| {
            !group.is_empty()
                && group
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn valid_relation_href<'a>(raw: &'a str, segment: &str) -> Option<&'a str> {
    let rest = raw.strip_prefix("/@")?;
    let (handle, suffix) = rest.split_once('/')?;
    (suffix == segment && valid_canonical_handle(handle)).then_some(raw)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicRelationships {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub followers_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub following_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub followers_href: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub following_href: Option<String>,
}

impl PublicRelationships {
    pub fn is_empty(&self) -> bool {
        self.followers_count.is_none()
            && self.following_count.is_none()
            && self.followers_href.is_none()
            && self.following_href.is_none()
    }

    pub fn valid_followers_href(&self) -> Option<&str> {
        self.followers_href
            .as_deref()
            .and_then(|href| valid_relation_href(href, FollowDirection::Followers.web_segment()))
    }

    pub fn valid_following_href(&self) -> Option<&str> {
        self.following_href
            .as_deref()
            .and_then(|href| valid_relation_href(href, FollowDirection::Following.web_segment()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnRelationshipsSummary {
    pub followers_count: i64,
    pub following_count: i64,
    pub followers_href: String,
    pub following_href: String,
}

impl OwnRelationshipsSummary {
    pub fn valid_followers_href(&self) -> Option<&str> {
        valid_relation_href(
            &self.followers_href,
            FollowDirection::Followers.web_segment(),
        )
    }

    pub fn valid_following_href(&self) -> Option<&str> {
        valid_relation_href(
            &self.following_href,
            FollowDirection::Following.web_segment(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowToken {
    pub following: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowObserved {
    pub target_handle: String,
    pub following: bool,
    pub mutual: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetFollowRequest {
    pub command_id: String,
    pub expected_order: i64,
    pub following: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetFollowResponse {
    pub target_handle: String,
    pub outcome: String,
    pub following: bool,
    pub mutual: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<FollowToken>,
    #[serde(default, skip_serializing_if = "PublicRelationships::is_empty")]
    pub relationships: PublicRelationships,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowReceipt {
    pub command_id: String,
    pub target_handle: String,
    pub expected_order: i64,
    pub desired_following: bool,
    pub committed: FollowToken,
    pub observed: FollowObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PublicRelationshipList {
    Page {
        author: PublicAuthorRef,
        direction: FollowDirection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<i64>,
        people: Vec<PublicAuthorRef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
    Restart {
        author: PublicAuthorRef,
        direction: FollowDirection,
    },
    Alias {
        href: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnRelationshipPerson {
    pub author: PublicAuthorRef,
    pub mutual: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OwnRelationshipList {
    Page {
        author: PublicAuthorRef,
        direction: FollowDirection,
        count: i64,
        people: Vec<OwnRelationshipPerson>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
    Restart {
        author: PublicAuthorRef,
        direction: FollowDirection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipItem {
    pub handle: String,
    pub following: bool,
    pub mutual: bool,
    pub pair_version: i64,
    pub accepted_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipBatch {
    pub items: Vec<MembershipItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorRelationshipsLoad {
    Page {
        author: PublicAuthorRef,
        direction: FollowDirection,
        count: Option<i64>,
        people: Vec<PublicAuthorRef>,
        next_cursor: Option<String>,
    },
    Restart {
        author: PublicAuthorRef,
        direction: FollowDirection,
    },
    Alias {
        href: String,
    },
    Private,
    Gone,
    Missing,
}

impl From<PublicRelationshipList> for AuthorRelationshipsLoad {
    fn from(list: PublicRelationshipList) -> Self {
        match list {
            PublicRelationshipList::Page {
                author,
                direction,
                count,
                people,
                next_cursor,
            } => Self::Page {
                author,
                direction,
                count,
                people,
                next_cursor,
            },
            PublicRelationshipList::Restart { author, direction } => {
                Self::Restart { author, direction }
            }
            PublicRelationshipList::Alias { href } => Self::Alias { href },
        }
    }
}
