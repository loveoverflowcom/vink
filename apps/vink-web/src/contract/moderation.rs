//! Browser-owned representations of the private moderation API.
use super::DocumentDto;
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
#[derive(Clone, Debug, Deserialize, Serialize)]
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
