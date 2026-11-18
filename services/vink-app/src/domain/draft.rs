use time::OffsetDateTime;

use super::document::{Title, TitledDocument};
use super::ids::WorkId;

/// Optimistic-concurrency version of a working draft. Starts at 1 and increases on every save.
/// ```compile_fail
/// use vink_app::domain::draft::DraftVersion;
/// let version: DraftVersion = serde_json::from_str("0").unwrap();
/// ```
/// Lawful twin (stable rustdoc does not check `compile_fail` error codes):
/// ```
/// use vink_app::domain::draft::DraftVersion;
/// let raw: i64 = serde_json::from_str("0").unwrap();
/// assert_eq!(DraftVersion::new(raw), None);
/// let _first: DraftVersion = DraftVersion::new(1).unwrap();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DraftVersion(i64);

impl DraftVersion {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Validated replacement content and the version it intends to replace.
///
/// This is neither a submitted revision nor authorization to write. The application validates
/// untrusted input into a [`TitledDocument`] (format-aware title authority) before constructing
/// it; the store must still check owner and version at the durable write. It deliberately has no
/// `Deserialize` implementation.
///
/// Raw input cannot stand in for validated values:
/// ```compile_fail
/// use vink_app::domain::document::DocumentData;
/// use vink_app::domain::draft::DraftChange;
/// let change = DraftChange::new(0, DocumentData {
///     format: "default".into(),
///     source: String::new(),
/// });
/// ```
/// Nor can deserialization bypass those constructors:
/// ```compile_fail
/// use vink_app::domain::draft::DraftChange;
/// let change: DraftChange = serde_json::from_str("{}").unwrap();
/// ```
/// Once constructed, callers cannot replace its fields:
/// ```compile_fail
/// use vink_app::domain::document::{Document, DocumentFormat, TitledDocument};
/// use vink_app::domain::draft::{DraftChange, DraftVersion};
/// let content = TitledDocument::author(Document::empty(DocumentFormat::Default), Some("")).unwrap();
/// let mut change = DraftChange::new(DraftVersion::FIRST, content);
/// change.expected_version = DraftVersion::FIRST;
/// ```
/// Lawful twin of the three blocks above: the same imports, raw shape and constructor compile
/// when validated values go in, so each block fails only at its own bypass.
/// ```
/// use vink_app::domain::document::{Document, DocumentData, DocumentFormat, TitledDocument};
/// use vink_app::domain::draft::{DraftChange, DraftVersion};
/// let data = DocumentData {
///     format: "default".into(),
///     source: String::new(),
/// };
/// let content = TitledDocument::author(Document::parse(data).unwrap(), Some("")).unwrap();
/// let change = DraftChange::new(DraftVersion::FIRST, content);
/// assert_eq!(change.expected_version(), DraftVersion::FIRST);
/// let _: serde_json::Value = serde_json::from_str("{}").unwrap();
/// let _ = DocumentFormat::Default;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftChange {
    expected_version: DraftVersion,
    content: TitledDocument,
}

impl DraftChange {
    pub fn new(expected_version: DraftVersion, content: TitledDocument) -> Self {
        Self {
            expected_version,
            content,
        }
    }

    pub fn expected_version(&self) -> DraftVersion {
        self.expected_version
    }

    pub fn content(&self) -> &TitledDocument {
        &self.content
    }
}

/// The author's private, mutable working copy. It is never a public or submitted snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingDraft {
    pub work_id: WorkId,
    pub version: DraftVersion,
    pub content: TitledDocument,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSummary {
    pub work_id: WorkId,
    pub version: DraftVersion,
    pub title: Title,
    pub updated_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::DraftVersion;

    #[test]
    fn versions_start_at_one() {
        assert_eq!(DraftVersion::new(0), None);
        assert_eq!(DraftVersion::new(-3), None);
        assert_eq!(DraftVersion::new(1), Some(DraftVersion::FIRST));
    }
}
