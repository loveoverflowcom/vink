use time::OffsetDateTime;

use super::document::{Document, Title};
use super::ids::WorkId;

/// Optimistic-concurrency version of a working draft. Starts at 1 and increases on every save.
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

/// The author's private, mutable working copy. It is never a public or submitted snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingDraft {
    pub work_id: WorkId,
    pub version: DraftVersion,
    pub title: Title,
    pub document: Document,
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
