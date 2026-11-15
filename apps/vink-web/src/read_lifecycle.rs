//! Bounded read ownership, independent of HTTP, Leptos and snapshot storage.
//! Scope includes representation/query/filter/sort/limit and, for private reads, the
//! principal/session generation. Cursor is an input, never a progress acknowledgement.
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

/// Unique across owner disposal/remount as well as A → B → A within one owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(u64);
impl RequestId {
    pub fn allocate() -> Self {
        Self(NEXT_REQUEST.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Initial,
    Refresh,
    Append,
    Restart,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket<S> {
    pub scope: S,
    pub generation: u64,
    pub cursor: Option<String>,
    pub intent: Intent,
    request: RequestId,
}

/// Only one logical flight per owner. A head read supersedes an append, but concurrent
/// refresh signals join the existing head flight. Validation remains the adapter's job.
#[derive(Clone, Debug)]
pub struct ReadOwner<S> {
    scope: S,
    generation: u64,
    flight: Option<ReadTicket<S>>,
    disposed: bool,
}
impl<S: Clone + PartialEq> ReadOwner<S> {
    pub fn new(scope: S) -> Self {
        Self {
            scope,
            generation: 0,
            flight: None,
            disposed: false,
        }
    }
    pub fn change_scope(&mut self, scope: S) {
        self.invalidate();
        self.scope = scope;
    }
    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.flight = None;
    }
    pub fn dispose(&mut self) {
        self.invalidate();
        self.disposed = true;
    }
    pub fn in_flight_intent(&self) -> Option<Intent> {
        self.flight.as_ref().map(|f| f.intent)
    }
    pub fn begin(&mut self, intent: Intent, cursor: Option<String>) -> Option<ReadTicket<S>> {
        if self.disposed || (intent == Intent::Append) != cursor.is_some() {
            return None;
        }
        if let Some(flight) = &self.flight {
            if intent == Intent::Append || flight.intent != Intent::Append {
                return None;
            }
            self.invalidate();
        }
        let ticket = ReadTicket {
            scope: self.scope.clone(),
            generation: self.generation,
            cursor,
            intent,
            request: RequestId::allocate(),
        };
        self.flight = Some(ticket.clone());
        Some(ticket)
    }
    pub fn accepts(&self, ticket: &ReadTicket<S>) -> bool {
        !self.disposed
            && self.scope == ticket.scope
            && self.generation == ticket.generation
            && self.flight.as_ref() == Some(ticket)
    }
    /// Atomically accept the page's identity/cursor against this flight. Keep the ticket
    /// current until finish so an owner can perform its bounded follow-up lookups.
    pub fn accept_page(
        &self,
        ticket: &ReadTicket<S>,
        chain: &mut PageChain,
        ids: impl IntoIterator<Item = String>,
        next: Option<String>,
    ) -> bool {
        self.accepts(ticket) && chain.accept(ticket, ids, next)
    }
    pub fn finish(&mut self, ticket: &ReadTicket<S>) -> bool {
        if !self.accepts(ticket) {
            return false;
        }
        self.flight = None;
        true
    }
}

/// Cursor/identity validation before an adapter commits its DTO. A successful head replaces
/// the entire chain. Reject overlap instead of silently deduplicating a potentially gapped page.
#[derive(Clone, Debug, Default)]
pub struct PageChain {
    pub next_cursor: Option<String>,
    pub generation: u64,
    pub pages: usize,
    ids: BTreeSet<String>,
    cursors: BTreeSet<String>,
}
impl PageChain {
    pub fn accept<S>(
        &mut self,
        ticket: &ReadTicket<S>,
        ids: impl IntoIterator<Item = String>,
        next: Option<String>,
    ) -> bool {
        let append = ticket.intent == Intent::Append;
        if append && (ticket.cursor.is_none() || ticket.cursor != self.next_cursor) {
            return false;
        }
        let mut page = BTreeSet::new();
        for id in ids {
            if !page.insert(id.clone()) || (append && self.ids.contains(&id)) {
                return false;
            }
        }
        if let Some(cursor) = &next
            && (page.is_empty()
                || ticket.cursor.as_ref() == Some(cursor)
                || (append && self.cursors.contains(cursor)))
        {
            return false;
        }
        if !append {
            self.ids.clear();
            self.cursors.clear();
            self.generation += 1;
            self.pages = 0;
        }
        self.pages += 1;
        self.ids.extend(page);
        if let Some(cursor) = &next {
            self.cursors.insert(cursor.clone());
        }
        self.next_cursor = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aba_session_disposal_and_reverse_completion_are_fenced() {
        let mut owner = ReadOwner::new("a/0");
        let old = owner.begin(Intent::Initial, None).unwrap();
        owner.change_scope("b/1");
        owner.change_scope("a/0");
        let new = owner.begin(Intent::Initial, None).unwrap();
        assert!(!owner.finish(&old));
        assert!(owner.accepts(&new));
        owner.dispose();
        assert!(!owner.finish(&new));
        let mut remount = ReadOwner::new("a/0");
        remount.begin(Intent::Initial, None).unwrap();
        assert!(!remount.accepts(&old));
    }
    #[test]
    fn rejected_append_keeps_cursor_and_refresh_replaces_chain() {
        let mut owner = ReadOwner::new("query");
        let mut chain = PageChain::default();
        let head = owner.begin(Intent::Initial, None).unwrap();
        assert!(chain.accept(&head, ["a".into()], Some("c1".into())));
        owner.finish(&head);
        let tail = owner
            .begin(Intent::Append, chain.next_cursor.clone())
            .unwrap();
        assert!(!chain.accept(&tail, ["a".into()], Some("c2".into())));
        assert_eq!(chain.next_cursor.as_deref(), Some("c1"));
        let fresh = owner.begin(Intent::Refresh, None).unwrap();
        assert!(!owner.accepts(&tail));
        assert!(owner.begin(Intent::Refresh, None).is_none());
        assert!(chain.accept(&fresh, ["b".into()], Some("new".into())));
        owner.finish(&fresh);
        let next = owner
            .begin(Intent::Append, chain.next_cursor.clone())
            .unwrap();
        assert!(
            chain.accept(&next, ["a".into()], None),
            "old head/tail IDs are discarded"
        );
    }
}
