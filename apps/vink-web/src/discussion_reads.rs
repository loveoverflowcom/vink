//! Private read reconciliation for one Reader, never serialized, cached or persisted.
use crate::contract::{ApiError, CommentStatus, OwnComment, OwnCommentList};
use crate::discussion_state::Scope;
use std::collections::{BTreeMap, BTreeSet};

pub const LOOKUP_BATCH: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    pub scope: Scope,
    pub generation: u64,
}

#[derive(Clone, Debug)]
pub struct OwnState {
    pub scope: Scope,
    generation: u64,
    pub loading: bool,
    pub comments: BTreeMap<String, OwnComment>,
    /// Listing order belongs to the own cursor, independently of public pagination.
    pub listed: Vec<String>,
    pub next_cursor: Option<String>,
    pub error: Option<ApiError>,
    pub author: bool,
    pub capability_error: Option<ApiError>,
}
impl OwnState {
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            generation: 0,
            loading: false,
            comments: BTreeMap::new(),
            listed: vec![],
            next_cursor: None,
            error: None,
            author: false,
            capability_error: None,
        }
    }
    pub fn change_scope(&mut self, scope: Scope) {
        let generation = self.generation + 1;
        *self = Self::new(scope);
        self.generation = generation;
    }
    /// Supersede, never queue behind an obsolete read. Only this ticket can settle loading.
    pub fn begin(&mut self) -> Option<ReadTicket> {
        self.scope.session.as_ref()?;
        self.generation += 1;
        self.loading = true;
        self.error = None;
        Some(ReadTicket {
            scope: self.scope.clone(),
            generation: self.generation,
        })
    }
    pub fn invalidate_reads(&mut self) {
        self.generation += 1;
        self.loading = false;
    }
    pub fn accepts(&self, ticket: &ReadTicket) -> bool {
        self.scope == ticket.scope && self.generation == ticket.generation
    }
    pub fn apply_list(&mut self, ticket: &ReadTicket, page: OwnCommentList, append: bool) -> bool {
        if !self.accepts(ticket) || page.work_id != self.scope.work_id {
            return false;
        }
        if !append {
            self.listed.clear();
        }
        for c in page.comments {
            if !self.listed.contains(&c.comment_id) {
                self.listed.push(c.comment_id.clone());
            }
            self.comments.entry(c.comment_id.clone()).or_insert(c);
        }
        self.next_cursor = page.next_cursor;
        true
    }
    /// Exact lookup, performed after the list, wins over its older snapshot. Absence is no
    /// evidence of removal: keep the last fact but revoke that item's unconfirmed capability.
    pub fn apply_lookup(
        &mut self,
        ticket: &ReadTicket,
        ids: &[String],
        page: OwnCommentList,
    ) -> bool {
        if !self.accepts(ticket) || page.work_id != self.scope.work_id {
            return false;
        }
        for id in ids {
            if let Some(c) = self.comments.get_mut(id) {
                c.can_remove = false;
            }
        }
        for c in page.comments {
            if ids.contains(&c.comment_id) {
                self.comments.insert(c.comment_id.clone(), c);
            }
        }
        true
    }
    pub fn finish(&mut self, ticket: &ReadTicket, error: Option<ApiError>) {
        if !self.accepts(ticket) {
            return;
        }
        self.loading = false;
        if let Some(error) = error {
            self.author = false;
            for c in self.comments.values_mut() {
                c.can_remove = false;
            }
            if matches!(
                error,
                ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound
            ) {
                self.comments.clear();
                self.listed.clear();
                self.next_cursor = None;
            }
            self.error = Some(error);
        }
    }
    pub fn can_remove(&self, id: &str) -> bool {
        self.error.is_none() && !self.loading && self.comments.get(id).is_some_and(|c| c.can_remove)
    }
    pub fn visible_items(&self, public: &BTreeSet<String>) -> Vec<OwnComment> {
        // Approved off-page is still recoverable after an ACK/lookup; never duplicate a
        // public row and never display a body/reason for removed items.
        let mut ids = self.listed.clone();
        for id in self.comments.keys() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        ids.into_iter()
            .filter_map(|id| self.comments.get(&id))
            .filter(|c| c.status != CommentStatus::Approved || !public.contains(&c.comment_id))
            .cloned()
            .collect()
    }
}

/// Stable dedup before batching. One task awaits each batch: concurrency = 1, not N+1.
pub fn lookup_batches(ids: impl IntoIterator<Item = String>) -> Vec<Vec<String>> {
    ids.into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(LOOKUP_BATCH)
        .map(<[String]>::to_vec)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(s: &str) -> Scope {
        Scope {
            work_id: "33333333-3333-4333-8333-333333333333".into(),
            session: Some(s.into()),
        }
    }
    fn list() -> OwnCommentList {
        serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/discussion.me.get.json"
        ))
        .unwrap()
    }
    fn lookup() -> OwnCommentList {
        serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/discussion.me.ids.json"
        ))
        .unwrap()
    }
    #[test]
    fn delayed_reads_never_block_or_settle_a_new_principal_or_generation() {
        for sequence in [
            ["a/0", "b/1", "a/2"],
            ["a/0", "guest/1", "a/2"],
            ["a/0", "guest/1", "b/2"],
        ] {
            let mut s = OwnState::new(scope(sequence[0]));
            let old = s.begin().unwrap();
            assert!(s.apply_list(&old, list(), false));
            s.author = true;
            for actor in &sequence[1..] {
                s.change_scope(scope(actor));
                assert!(s.comments.is_empty() && !s.author && !s.loading);
                let next = s.begin().unwrap();
                assert!(!s.apply_list(&old, list(), false));
                s.finish(&old, Some(ApiError::Unavailable));
                assert!(s.loading, "old completion cannot clear new loading");
                assert!(s.apply_list(&next, list(), false));
                s.finish(&next, None);
                assert!(!s.loading);
            }
        }
    }
    #[test]
    fn lookup_wins_over_list_and_missing_id_is_never_invented_as_removed() {
        let mut s = OwnState::new(scope("a/0"));
        let t = s.begin().unwrap();
        let mut page = list();
        let id = page.comments[0].comment_id.clone();
        s.apply_list(&t, page.clone(), false);
        let mut approved = page.comments[0].clone();
        approved.status = CommentStatus::Approved;
        s.apply_lookup(
            &t,
            std::slice::from_ref(&id),
            OwnCommentList {
                work_id: t.scope.work_id.clone(),
                comments: vec![approved.clone()],
                next_cursor: None,
            },
        );
        s.apply_list(&t, page.clone(), false); // older listing snapshot cannot downgrade lookup
        assert_eq!(s.comments[&id].status, CommentStatus::Approved);
        assert!(
            s.visible_items(&BTreeSet::from([id.clone()]))
                .iter()
                .all(|c| c.comment_id != id)
        );
        assert!(
            s.visible_items(&BTreeSet::new())
                .iter()
                .any(|c| c.comment_id == id)
        );
        s.apply_lookup(
            &t,
            std::slice::from_ref(&id),
            OwnCommentList {
                work_id: t.scope.work_id.clone(),
                comments: vec![],
                next_cursor: None,
            },
        );
        assert_eq!(s.comments[&id].status, CommentStatus::Approved);
        assert!(!s.comments[&id].can_remove);
        page.comments[0].status = CommentStatus::Removed;
        page.comments[0].body = None;
        s.apply_lookup(&t, std::slice::from_ref(&id), page);
        assert_eq!(s.comments[&id].body, None);
        let old = t;
        let new = s.begin().unwrap();
        assert!(!s.apply_lookup(
            &old,
            std::slice::from_ref(&id),
            OwnCommentList {
                work_id: old.scope.work_id.clone(),
                comments: vec![approved],
                next_cursor: None
            }
        ));
        s.finish(&new, None);
        assert_eq!(s.comments[&id].status, CommentStatus::Removed);
    }
    #[test]
    fn authority_failure_revokes_private_facts_but_temporary_failure_preserves_text() {
        let mut s = OwnState::new(scope("a/0"));
        let t = s.begin().unwrap();
        s.apply_list(&t, list(), false);
        s.finish(&t, Some(ApiError::Unavailable));
        assert!(!s.comments.is_empty());
        assert!(s.comments.values().all(|c| !c.can_remove));
        for error in [
            ApiError::Unauthenticated,
            ApiError::Forbidden,
            ApiError::NotFound,
        ] {
            let t = s.begin().unwrap();
            s.apply_list(&t, list(), false);
            s.finish(&t, Some(error));
            assert!(s.comments.is_empty() && s.listed.is_empty() && !s.author && !s.loading);
        }
    }
    #[test]
    fn fixtures_include_unlisted_roots_rejections_and_removed_without_body() {
        let list = list();
        assert!(list.comments.iter().any(|c| matches!(
            c.placement,
            crate::contract::OwnPlacement::Reply {
                root_listed: false,
                ..
            }
        )));
        assert!(
            list.comments
                .iter()
                .any(|c| c.status == CommentStatus::Rejected && c.rejection_reason.is_some())
        );
        assert!(
            lookup()
                .comments
                .iter()
                .any(|c| c.status == CommentStatus::Removed && c.body.is_none() && !c.can_remove)
        );
        let ids = (0..81).chain(0..81).map(|n| format!("id-{n}"));
        let batches = lookup_batches(ids);
        assert_eq!(batches.len(), 5);
        assert!(batches.iter().all(|b| b.len() <= 20));
        assert_eq!(
            batches.into_iter().flatten().collect::<BTreeSet<_>>().len(),
            81
        );
    }
}
