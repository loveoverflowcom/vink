//! Presentation rules for the loaded private draft list; no authority or manuscript reads.

use crate::contract::{ApiError, DraftCard};
pub use crate::search::search_key;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DraftSort {
    #[default]
    Recent,
    Title,
}

pub fn visible_drafts(drafts: &[DraftCard], query: &str, sort: DraftSort) -> Vec<DraftCard> {
    let query = search_key(query);
    let mut visible: Vec<_> = drafts
        .iter()
        .filter(|draft| search_key(&display_title(&draft.title)).contains(&query))
        .cloned()
        .collect();
    if sort == DraftSort::Title {
        visible.sort_by_cached_key(|draft| {
            (
                search_key(&display_title(&draft.title)),
                draft.work_id.clone(),
            )
        });
    }
    // Recent keeps the provider's updated_at DESC, work_id order (including equal timestamps).
    visible
}

pub fn display_title(title: &str) -> String {
    if title.is_empty() {
        "Chưa đặt tên".to_string()
    } else {
        title.to_string()
    }
}

/// API v1 timestamps are RFC 3339 UTC. Keep that zone explicit rather than imply local time.
pub fn updated_at_label(timestamp: &str) -> String {
    match (timestamp.get(0..10), timestamp.get(11..16)) {
        (Some(date), Some(clock))
            if timestamp.get(10..11) == Some("T") && timestamp.ends_with('Z') =>
        {
            let parts: Vec<_> = date.split('-').collect();
            if let [year, month, day] = parts.as_slice() {
                return format!("{day}/{month}/{year}, {clock} UTC");
            }
            timestamp.to_string()
        }
        _ => timestamp.to_string(),
    }
}

/// A failed non-idempotent POST must not turn into an automatic replay. Even 503/500 can
/// follow an uncertain transaction commit. Only explicit pre-write rejections are certain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CreateState {
    #[default]
    Ready,
    Pending,
    Rejected(ApiError),
    Uncertain,
    Reconciled,
}

impl CreateState {
    pub fn can_begin(&self) -> bool {
        matches!(
            self,
            Self::Ready | Self::Rejected(ApiError::Invalid { .. }) | Self::Reconciled
        )
    }

    pub fn begin(&mut self) -> bool {
        if !self.can_begin() {
            return false;
        }
        *self = Self::Pending;
        true
    }

    pub fn failed(&mut self, error: ApiError) {
        *self = match error {
            ApiError::Unauthenticated | ApiError::Forbidden | ApiError::Invalid { .. } => {
                Self::Rejected(error)
            }
            _ => Self::Uncertain,
        };
    }

    /// A successful GET permits a deliberate extra create; it cannot prove which POST wrote.
    pub fn reconciled(&mut self) {
        match self {
            Self::Uncertain => *self = Self::Reconciled,
            Self::Rejected(ApiError::Unauthenticated | ApiError::Forbidden) => *self = Self::Ready,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_normalization::UnicodeNormalization;

    fn card(id: &str, title: &str) -> DraftCard {
        DraftCard {
            work_id: id.into(),
            title: title.into(),
            version: 2,
            updated_at: "2026-11-01T08:30:00Z".into(),
        }
    }

    #[test]
    fn vietnamese_search_preserves_source_and_provider_order() {
        let drafts = vec![
            card("b", "Đường về có nắng"),
            card("a", "ĐƯỜNG VỀ CÓ MƯA".nfd().collect::<String>().as_str()),
            card("c", ""),
        ];
        let found = visible_drafts(&drafts, "  duong   ve co ", DraftSort::Recent);
        assert_eq!(
            found.iter().map(|d| d.work_id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(found[1].title, drafts[1].title);
        assert_eq!(
            visible_drafts(&drafts, "đường về có mưa", DraftSort::Title)[0].work_id,
            "a"
        );
        assert_eq!(
            visible_drafts(&drafts, "chua dat ten", DraftSort::Recent)[0].work_id,
            "c"
        );
        assert!(visible_drafts(&drafts, "biển", DraftSort::Recent).is_empty());
        assert_eq!(
            visible_drafts(&drafts, "", DraftSort::Title)[0].work_id,
            "c"
        );
    }

    #[test]
    fn uncertain_create_needs_reconciliation_and_a_new_intent() {
        for error in [
            ApiError::Network,
            ApiError::Unavailable,
            ApiError::Unexpected { status: 201 },
            ApiError::Unexpected { status: 500 },
        ] {
            let mut state = CreateState::Ready;
            assert!(state.begin());
            assert!(!state.begin());
            state.failed(error);
            assert_eq!(state, CreateState::Uncertain);
            assert!(!state.begin());
            state.reconciled();
            assert_eq!(state, CreateState::Reconciled);
            assert!(state.begin());
        }
    }

    #[test]
    fn explicit_pre_write_rejections_remain_distinct() {
        for error in [
            ApiError::Unauthenticated,
            ApiError::Forbidden,
            ApiError::Invalid { field: None },
        ] {
            let mut state = CreateState::Pending;
            state.failed(error.clone());
            assert_eq!(state, CreateState::Rejected(error));
            let invalid = matches!(state, CreateState::Rejected(ApiError::Invalid { .. }));
            assert_eq!(state.can_begin(), invalid);
            state.reconciled();
            assert!(state.can_begin());
        }
    }
}
