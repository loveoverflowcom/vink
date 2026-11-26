//! One in-memory inbox per active surface and confirmed browser session.
#[cfg(feature = "browser")]
use super::contract::SnapshotCommand;
use super::contract::{Freshness, Item, Page};
use vink_ui::BadgeState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Load {
    #[default]
    Idle,
    Loading,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Inbox {
    pub snapshot: Option<String>,
    pub items: Vec<Item>,
    pub next: Option<String>,
    /// The request cursors for all pages successfully rendered.
    pub pages: Vec<Option<String>>,
    pub first: Load,
    pub more: Load,
    pub mark: Load,
    #[cfg(feature = "browser")]
    pub mark_command: Option<SnapshotCommand>,
    #[cfg(feature = "browser")]
    pub seen_command: Option<SnapshotCommand>,
}

impl Inbox {
    pub fn append(&mut self, cursor: Option<String>, page: Page) {
        if cursor.is_none() {
            self.snapshot = Some(page.inbox_snapshot);
            self.items.clear();
            self.pages.clear();
        }
        self.next = page
            .next_cursor
            .filter(|next| Some(next) != cursor.as_ref());
        self.pages.push(cursor);
        for item in page.items {
            if !self
                .items
                .iter()
                .any(|shown| shown.group_id == item.group_id)
            {
                self.items.push(item);
            }
        }
    }
}

impl Freshness {
    pub fn badge(&self) -> BadgeState {
        match self {
            Self::Fresh {
                count_is_lower_bound: true,
                ..
            } => BadgeState::LowerBound,
            Self::Stale {
                count_is_lower_bound: true,
                ..
            } => BadgeState::StaleLowerBound,
            Self::Fresh {
                unseen_group_count, ..
            } => BadgeState::Count(*unseen_group_count),
            Self::Stale {
                last_known_count, ..
            } => BadgeState::Stale(*last_known_count),
        }
    }
}

pub(super) fn stale(badge: BadgeState) -> BadgeState {
    match badge {
        BadgeState::Count(n) => BadgeState::Stale(n),
        BadgeState::LowerBound => BadgeState::StaleLowerBound,
        other => other,
    }
}

pub(super) fn poll_delay(failures: u32, entropy: f64) -> u64 {
    if failures == 0 {
        30_000 + (entropy * 30_000.0) as u64
    } else {
        (30_000_u64 << (failures - 1).min(3)) + (entropy * 15_000.0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_contract_maps_verified_stale_and_lower_bound_without_a_fake_zero() {
        use super::super::contract::Count;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/notifications-examples.json"
        ))
        .unwrap();
        let verified: Count = serde_json::from_value(fixture["counts"][0].clone()).unwrap();
        let cached: Count = serde_json::from_value(fixture["counts"][1].clone()).unwrap();
        assert_eq!(verified.freshness.badge(), BadgeState::Count(3));
        assert_eq!(cached.freshness.badge(), BadgeState::Stale(3));
        assert_eq!(
            Freshness::Stale {
                last_known_count: 0,
                count_is_lower_bound: false
            }
            .badge()
            .digits(),
            None
        );
    }

    #[test]
    fn healthy_poll_is_jittered_and_failure_backoff_is_bounded() {
        assert_eq!(poll_delay(0, 0.0), 30_000);
        assert_eq!(poll_delay(0, 0.999), 59_970);
        assert_eq!(poll_delay(2, 0.0), 60_000);
        assert_eq!(poll_delay(3, 0.5), 127_500);
        assert_eq!(poll_delay(20, 0.5), 247_500);
    }

    #[test]
    fn filtered_page_continues_and_cannot_move_the_seen_watermark() {
        let mut inbox = Inbox::default();
        inbox.append(
            None,
            Page {
                inbox_snapshot: "first".into(),
                next_cursor: Some("next".into()),
                ..Page::default()
            },
        );
        assert_eq!(inbox.next.as_deref(), Some("next"));
        inbox.append(
            Some("next".into()),
            Page {
                inbox_snapshot: "future".into(),
                ..Page::default()
            },
        );
        assert_eq!(inbox.snapshot.as_deref(), Some("first"));
        assert!(inbox.next.is_none());
    }
}
