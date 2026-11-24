use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Everyone,
    #[serde(rename = "self")]
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policies {
    pub pen_name: Visibility,
    pub avatar: Visibility,
    pub bio: Visibility,
    pub date_of_birth: Visibility,
    pub followers_list: Visibility,
    pub followers_count: Visibility,
    pub following_list: Visibility,
    pub following_count: Visibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnProfile {
    pub principal_id: String,
    pub pen_name: String,
    pub handle: String,
    pub handle_setup_complete: bool,
    pub remaining_handle_changes: i16,
    pub profile_version: i64,
    pub bio: Option<String>,
    pub policies: Policies,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SaveProfile {
    pub principal_id: String,
    pub expected_version: i64,
    pub pen_name: String,
    pub bio: Option<String>,
    pub policies: Policies,
}

impl SaveProfile {
    /// On a lost answer report what was observed, without attributing a concurrent save
    /// to this tab. An independently advanced version is handled as a conflict.
    pub fn matches_successor(&self, stored: &OwnProfile) -> bool {
        self.principal_id == stored.principal_id
            && self.expected_version.checked_add(1) == Some(stored.profile_version)
            && self.pen_name == stored.pen_name
            && self.bio == stored.bio
            && self.policies == stored.policies
    }
}

/// Private Handle projection. Its version and quota belong to a different command from SaveProfile.
#[cfg(feature = "browser")]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OwnHandle {
    pub principal_id: String,
    pub handle: String,
    pub setup_complete: bool,
    pub remaining_changes: i16,
    pub version: i64,
    pub aliases: Vec<String>,
}

#[cfg(feature = "browser")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HandleIntent {
    Setup,
    Rename,
}

/// This wire shape also reads commands left in sessionStorage by the former Handle page.
#[cfg(feature = "browser")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HandleCommand {
    pub principal_id: String,
    pub command_id: String,
    pub expected_version: i64,
    pub intent: HandleIntent,
    pub handle: String,
}

#[cfg(feature = "browser")]
#[derive(Debug, Deserialize)]
pub(crate) struct HandleReceipt {
    pub command_id: String,
    pub accepted_version: i64,
    pub handle: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleRefusal {
    Taken,
    OwnAlias,
    Exhausted,
    KeyReused,
}
