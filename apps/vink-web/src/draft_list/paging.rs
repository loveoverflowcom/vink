//! Bounded private-summary adapter for the shared read owner. No transport/cache/authority.
use crate::contract::author_works::{AuthorWorkCard as DraftCard, AuthorWorkList as DraftList};
use crate::read_lifecycle::{Intent, PageChain, ReadOwner, ReadTicket};

pub const PAGE_SIZE: usize = 24;
pub const PAGES_PER_PART: usize = 10;

#[derive(Clone, Debug, Default)]
pub struct DraftPages {
    pub drafts: Vec<DraftCard>,
    pub chain: PageChain,
    pub loaded: bool,
    pub part: usize,
}
impl DraftPages {
    pub fn next_part(&self) -> bool {
        self.chain.pages >= PAGES_PER_PART
    }
    /// Validate in a temporary chain. A failed page leaves both cursor and summaries intact.
    /// At the explicit part boundary, discard old IDs/cursors only with the accepted next page.
    pub fn accept<S: Clone + PartialEq>(
        &mut self,
        owner: &ReadOwner<S>,
        ticket: &ReadTicket<S>,
        page: DraftList,
    ) -> bool {
        let append = ticket.intent == Intent::Append;
        if page.works.len() > PAGE_SIZE
            || page
                .next_cursor
                .as_ref()
                .is_some_and(|c| c.is_empty() || c.len() > 6500)
            || (append && !self.loaded)
        {
            return false;
        }
        let next_part = append && self.next_part();
        let mut chain = if next_part {
            let mut fresh = PageChain::default();
            fresh.next_cursor = self.chain.next_cursor.clone();
            fresh.generation = self.chain.generation + 1;
            fresh
        } else {
            self.chain.clone()
        };
        if !owner.accept_page(
            ticket,
            &mut chain,
            page.works.iter().map(|d| d.work_id.clone()),
            page.next_cursor,
        ) {
            return false;
        }
        if !append || next_part {
            self.drafts = page.works;
            self.part = if next_part { self.part + 1 } else { 1 };
        } else {
            self.drafts.extend(page.works);
        }
        self.chain = chain;
        self.loaded = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(start: usize, more: bool) -> DraftList {
        DraftList {
            works: (start..start + PAGE_SIZE)
                .map(|id| DraftCard {
                    work_id: id.to_string(),
                    title: format!("Bản {id}"),
                    draft_version: Some(1),
                    publication: None,
                    views: None,
                    updated_at: "2026-11-15T00:00:00Z".into(),
                })
                .collect(),
            next_cursor: more.then(|| format!("cursor-{}", start + PAGE_SIZE)),
        }
    }
    #[test]
    fn bounded_parts_failures_and_head_refresh_commit_data_with_the_cursor() {
        let mut owner = ReadOwner::new("owner/1;q;recent;24");
        let mut pages = DraftPages::default();
        for n in 0..PAGES_PER_PART {
            let ticket = owner
                .begin(
                    if n == 0 {
                        Intent::Initial
                    } else {
                        Intent::Append
                    },
                    pages.chain.next_cursor.clone(),
                )
                .unwrap();
            assert!(pages.accept(&owner, &ticket, page(n * PAGE_SIZE, true)));
            owner.finish(&ticket);
        }
        assert_eq!(pages.drafts.len(), 240);
        let ticket = owner
            .begin(Intent::Append, pages.chain.next_cursor.clone())
            .unwrap();
        let old_cursor = pages.chain.next_cursor.clone();
        let mut invalid = page(240, true);
        invalid.next_cursor = old_cursor.clone();
        assert!(!pages.accept(&owner, &ticket, invalid));
        assert_eq!(pages.drafts.len(), 240);
        assert_eq!(pages.chain.next_cursor, old_cursor);
        assert!(pages.accept(&owner, &ticket, page(240, true)));
        owner.finish(&ticket);
        assert_eq!(pages.drafts.len(), PAGE_SIZE);
        assert_eq!(pages.chain.pages, 1);
        assert_eq!(pages.part, 2);
        let late = owner
            .begin(Intent::Append, pages.chain.next_cursor.clone())
            .unwrap();
        let head = owner.begin(Intent::Refresh, None).unwrap();
        assert!(!pages.accept(&owner, &late, page(264, true)));
        assert!(pages.accept(&owner, &head, page(0, true)));
        owner.finish(&head);
        assert_eq!(pages.part, 1);
        assert_eq!(pages.drafts[0].work_id, "0");
        assert_eq!(pages.chain.pages, 1);
    }
    #[test]
    fn query_sort_and_session_aba_cannot_commit_old_private_data() {
        for a in ["A/1;q1;recent;24", "A/1;q1;title;24", "A/3;q1;recent;24"] {
            let mut owner = ReadOwner::new(a);
            let old = owner.begin(Intent::Initial, None).unwrap();
            owner.change_scope("B/2;q2;recent;24");
            owner.change_scope(a);
            let current = owner.begin(Intent::Initial, None).unwrap();
            let mut pages = DraftPages::default();
            assert!(!pages.accept(&owner, &old, page(0, true)));
            assert!(!pages.loaded);
            assert!(pages.accept(&owner, &current, page(24, false)));
            owner.finish(&current);
            assert_eq!(pages.drafts[0].work_id, "24");
            assert_eq!(pages.chain.next_cursor, None);
        }
    }
}
