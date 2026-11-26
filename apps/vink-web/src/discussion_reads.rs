//! Private read reconciliation for one Reader, never serialized, cached or persisted.
use crate::contract::{ApiError, CommentStatus, OwnComment, OwnCommentList};
use crate::discussion_state::Scope;
use std::collections::{BTreeMap, BTreeSet};

pub const LOOKUP_BATCH: usize = 20;

pub type ReadTicket = crate::read_lifecycle::ReadTicket<Scope>;
use crate::read_lifecycle::{Intent, PageChain, ReadOwner};

#[derive(Clone, Debug)]
pub struct OwnState {
    pub scope: Scope,
    reads: ReadOwner<Scope>,
    chain: PageChain,
    pub loading: bool,
    pub comments: BTreeMap<String, OwnComment>,
    /// Listing order belongs to the own cursor, independently of public pagination.
    pub listed: Vec<String>,
    pub next_cursor: Option<String>,
    pub error: Option<ApiError>,
    pub author: bool,
    pub capability_error: Option<ApiError>,
    pub capability_loading: bool,
    /// The one own comment whose removal waits for an explicit “Gỡ” (VDR-DISCUSSION-R4@r1).
    confirming: Option<String>,
}

/// The chip “Bạn đã gửi” shows for one own comment (R4 image 06): text plus icon, never colour
/// alone. Placement and the writer-only rejection reason stay separate facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnChip {
    Pending,
    Rejected,
    /// Approved, yet not in the public pages loaded so far.
    ApprovedOffPage,
    Unavailable,
}

impl OwnChip {
    pub const fn of(status: CommentStatus) -> Self {
        match status {
            CommentStatus::Pending => Self::Pending,
            CommentStatus::Rejected => Self::Rejected,
            CommentStatus::Approved => Self::ApprovedOffPage,
            CommentStatus::Removed => Self::Unavailable,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "Chờ duyệt · Chưa công khai",
            Self::Rejected => "Không được duyệt · Chưa công khai",
            Self::ApprovedOffPage => "Đã được duyệt · Ngoài phần công khai đang tải",
            Self::Unavailable => "Nhận xét không còn hiển thị",
        }
    }

    pub const fn tone(self) -> crate::discussion_state::NoticeTone {
        use crate::discussion_state::NoticeTone;
        match self {
            Self::Pending => NoticeTone::Waiting,
            Self::Rejected => NoticeTone::Adverse,
            Self::ApprovedOffPage => NoticeTone::Success,
            Self::Unavailable => NoticeTone::Neutral,
        }
    }
}

impl OwnState {
    pub fn new(scope: Scope) -> Self {
        Self {
            reads: ReadOwner::new(scope.clone()),
            chain: PageChain::default(),
            scope,
            loading: false,
            comments: BTreeMap::new(),
            listed: vec![],
            next_cursor: None,
            error: None,
            author: false,
            capability_error: None,
            capability_loading: false,
            confirming: None,
        }
    }
    pub fn change_scope(&mut self, scope: Scope) {
        self.reads.change_scope(scope.clone());
        let reads = self.reads.clone();
        *self = Self::new(scope);
        self.reads = reads;
    }
    /// Explicit reconciliation supersedes a preceding read; event bursts use begin_page.
    pub fn begin(&mut self) -> Option<ReadTicket> {
        self.reads.invalidate();
        self.begin_page(false)
    }
    pub fn begin_page(&mut self, append: bool) -> Option<ReadTicket> {
        self.scope.session.as_ref()?;
        let ticket = self.reads.begin(
            if append {
                Intent::Append
            } else {
                Intent::Refresh
            },
            if append {
                self.next_cursor.clone()
            } else {
                None
            },
        )?;
        self.loading = true;
        self.error = None;
        // Reloading what the writer sees cancels a removal they had not confirmed yet.
        self.confirming = None;
        Some(ticket)
    }
    pub fn invalidate_reads(&mut self) {
        self.reads.invalidate();
        self.loading = false;
    }
    pub fn accepts(&self, ticket: &ReadTicket) -> bool {
        self.reads.accepts(ticket)
    }
    pub fn apply_list(&mut self, ticket: &ReadTicket, page: OwnCommentList, append: bool) -> bool {
        if !self.accepts(ticket)
            || page.work_id != self.scope.work_id
            || append != (ticket.intent == Intent::Append)
            || !self.reads.accept_page(
                ticket,
                &mut self.chain,
                page.comments.iter().map(|c| c.comment_id.clone()),
                page.next_cursor.clone(),
            )
        {
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
        self.reads.finish(ticket);
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
    pub fn has_loaded_page(&self) -> bool {
        self.chain.pages > 0
    }

    pub fn can_remove(&self, id: &str) -> bool {
        self.error.is_none() && !self.loading && self.comments.get(id).is_some_and(|c| c.can_remove)
    }
    /// “Gỡ nhận xét của tôi” opens a confirmation at that item; nothing is sent yet. Asking for
    /// another item moves the single confirmation there.
    pub fn ask_remove(&mut self, id: &str) -> bool {
        let allowed = self.can_remove(id);
        self.confirming = allowed.then(|| id.to_owned());
        allowed
    }
    pub fn cancel_remove(&mut self) {
        self.confirming = None;
    }
    /// Shown only while removal is still allowed: a lost capability hides the confirmation.
    pub fn confirming(&self, id: &str) -> bool {
        self.confirming.as_deref() == Some(id) && self.can_remove(id)
    }
    /// Consumes the confirmation. Only a confirmed, still removable comment may be sent.
    pub fn take_confirmed(&mut self, id: &str) -> bool {
        let confirmed = self.confirming(id);
        if confirmed {
            self.confirming = None;
        }
        confirmed
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

    #[test]
    fn r4_removal_waits_for_one_confirmation_that_any_reload_or_lost_capability_cancels() {
        let mut s = OwnState::new(scope("a/0"));
        let t = s.begin().unwrap();
        let mut page = list();
        for c in &mut page.comments {
            c.can_remove = true;
        }
        let first = page.comments[0].comment_id.clone();
        assert!(s.apply_list(&t, page.clone(), false));
        assert!(
            !s.ask_remove(&first),
            "nothing is removable while a read is open"
        );
        s.finish(&t, None);
        assert!(!s.take_confirmed(&first), "no confirmation, no DELETE");
        assert!(s.ask_remove(&first) && s.confirming(&first));
        assert!(
            !s.ask_remove("unknown-id") && !s.confirming(&first),
            "one at a time"
        );
        assert!(s.ask_remove(&first));
        s.cancel_remove();
        assert!(!s.take_confirmed(&first));
        assert!(s.ask_remove(&first));
        assert!(s.take_confirmed(&first));
        assert!(
            !s.take_confirmed(&first),
            "consumed: a double press sends once"
        );
        // A reload cancels; so does a failed read that revokes every capability.
        assert!(s.ask_remove(&first));
        let reload = s.begin().unwrap();
        assert!(!s.confirming(&first));
        assert!(s.apply_list(&reload, page.clone(), false));
        s.finish(&reload, None);
        assert!(
            !s.confirming(&first),
            "a finished reload does not resurrect it"
        );
        assert!(s.ask_remove(&first));
        let failing = s.begin_page(false).unwrap();
        s.finish(&failing, Some(ApiError::Unavailable));
        assert!(!s.confirming(&first) && !s.take_confirmed(&first));
        // Another principal starts without any pending confirmation.
        s.change_scope(scope("b/1"));
        assert!(!s.confirming(&first));
    }

    #[test]
    fn r4_own_chips_name_each_status_in_words_with_their_own_tone() {
        use crate::discussion_state::NoticeTone;
        let chips = [
            CommentStatus::Pending,
            CommentStatus::Rejected,
            CommentStatus::Approved,
            CommentStatus::Removed,
        ]
        .map(OwnChip::of);
        assert_eq!(
            chips,
            [
                OwnChip::Pending,
                OwnChip::Rejected,
                OwnChip::ApprovedOffPage,
                OwnChip::Unavailable
            ]
        );
        assert_eq!(OwnChip::Pending.tone(), NoticeTone::Waiting);
        assert_eq!(OwnChip::Rejected.tone(), NoticeTone::Adverse);
        assert_eq!(OwnChip::ApprovedOffPage.tone(), NoticeTone::Success);
        assert_eq!(OwnChip::Unavailable.tone(), NoticeTone::Neutral);
        let labels = chips.map(OwnChip::label);
        assert!(labels.iter().all(|l| !l.is_empty()));
        assert_eq!(
            labels.iter().collect::<BTreeSet<_>>().len(),
            4,
            "distinct words, not colour alone"
        );
        assert!(!OwnChip::Unavailable.label().contains("xóa"));
    }
}
