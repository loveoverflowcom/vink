//! Work views (#341): record one view event and prune what VIEW-11 lets expire.
//!
//! The HTTP ingest of #343 resolves who is reading (the session's principal, or the browser's
//! guest token), applies the per-network budget and the speculative-request rule of
//! `domain::work_view`, then calls [`Vink::record_work_view`] with the decision clock and the
//! server digest key. This use case validates the untrusted fields, derives the stored digests
//! and hands ONE [`WorkViewStore`] transaction the event: it loads the facts under locks, calls
//! `decide_view` and commits before answering. Nothing here restates a counting rule.
//!
//! Every result is a success for the reader: a skipped, replayed or ignored event is a normal
//! answer, and a failure is only ever reported to the ingest, which must never block reading
//! (VIEW-10). `Ignored(WorkNotFound)` and `Skipped(NotPublic)` must look the same over HTTP.

use time::OffsetDateTime;

use super::Vink;
use super::ports::{StoreError, ViewPrune, ViewWrite, WorkViewStore};
use crate::domain::ids::{PrincipalId, RevisionId, WorkId};
use crate::domain::work_view::{
    EventDigest, GuestToken, ReaderDigest, ViewDecision, ViewDigestKey, ViewEvent, ViewEventId,
    ViewReader, ViewRefusal, ViewSkip, ViewVerdict,
};

/// Most prune rounds per call; each round removes up to `limit` windows and `limit` receipts.
pub const MAX_PRUNE_ROUNDS: usize = 100;

/// Untrusted fields of one view event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordWorkView {
    pub work_id: WorkId,
    /// The UUIDv7 the tab minted when the body was shown; the same text on every retry.
    pub event_id: String,
}

/// Who is reading, as the HTTP adapter resolved it. A valid session wins over a guest token; a
/// missing, invalid or expired session is a guest. Suspension does not matter: reading is not an
/// authority-bearing write, so it takes no standing fence and a suspended member counts like
/// any reader.
#[derive(Clone, PartialEq, Eq)]
pub enum Viewer {
    Member(PrincipalId),
    /// The guest token text from the browser, as received.
    Guest(String),
}

impl std::fmt::Debug for Viewer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Member(principal) => f.debug_tuple("Member").field(principal).finish(),
            Self::Guest(_) => f.write_str("Guest(..)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkViewInput {
    #[error("event_id must be a UUIDv7")]
    EventId,
    #[error("the guest token must be a random UUID")]
    GuestToken,
}

impl WorkViewInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::EventId => "event_id",
            Self::GuestToken => "guest_token",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkViewError {
    #[error("invalid input: {0}")]
    Invalid(#[from] WorkViewInput),
    /// Nothing was committed; a retry with the same event may succeed.
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for WorkViewError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            // No principal or standing is checked on this path; anything else is a broken
            // invariant, never reported as a counted or skipped view.
            StoreError::UnknownPrincipal
            | StoreError::PrincipalSuspended
            | StoreError::Integrity(_) => Self::Internal,
        }
    }
}

/// What a committed event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkViewResult {
    /// +1 to the work's total and to the day of `decided_at`.
    Counted,
    /// +0; the decision is kept so a retry replays it.
    Skipped(ViewSkip),
    /// +0; this event was decided before, with this verdict. Nothing was written.
    Replayed(ViewVerdict),
    /// +0; not decided and nothing written.
    Ignored(ViewRefusal),
}

/// The durable answer to one event, reported only after the store committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkViewRecord {
    pub work_id: WorkId,
    pub event_id: ViewEventId,
    pub result: WorkViewResult,
    /// The public revision the (first) decision saw: the publication basis. `None` when the work
    /// was not public or the event was ignored.
    pub basis: Option<RevisionId>,
    /// The decision clock of this request (microsecond precision, as stored).
    pub decided_at: OffsetDateTime,
}

impl WorkViewRecord {
    /// The change this request made to the work's total.
    pub fn delta(&self) -> i64 {
        i64::from(self.result == WorkViewResult::Counted)
    }
}

/// What one prune call removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkViewPrune {
    pub windows: u64,
    pub receipts: u64,
    /// `false` when [`MAX_PRUNE_ROUNDS`] ran out before the backlog did.
    pub drained: bool,
}

/// The clock as PostgreSQL stores it, so a decision and its stored instant are equal.
fn microseconds(now: OffsetDateTime) -> OffsetDateTime {
    now.replace_nanosecond(now.nanosecond() / 1_000 * 1_000)
        .expect("truncating nanoseconds stays in range")
}

fn reader(viewer: &Viewer) -> Result<ViewReader, WorkViewInput> {
    match viewer {
        Viewer::Member(principal) => Ok(ViewReader::Member(*principal)),
        Viewer::Guest(raw) => GuestToken::parse(raw)
            .map(ViewReader::Guest)
            .ok_or(WorkViewInput::GuestToken),
    }
}

impl<S> Vink<S>
where
    S: WorkViewStore,
{
    /// Records one view event decided at `now` (the HTTP adapter passes its clock). The digest
    /// `key` comes from configuration and never reaches storage.
    pub async fn record_work_view(
        &self,
        key: &ViewDigestKey,
        viewer: Viewer,
        input: RecordWorkView,
        now: OffsetDateTime,
    ) -> Result<WorkViewRecord, WorkViewError> {
        let event_id = ViewEventId::parse(&input.event_id).ok_or(WorkViewInput::EventId)?;
        let reader = reader(&viewer)?;
        let event = ViewEvent {
            work: input.work_id,
            id: event_id,
            reader,
        };
        let decided_at = microseconds(now);
        let decision = self
            .store()
            .record_view(ViewWrite {
                event,
                reader: ReaderDigest::derive(key, reader, input.work_id),
                event_digest: EventDigest::of(event_id),
                now: decided_at,
            })
            .await?;
        let (result, basis) = match decision {
            ViewDecision::Count(write) => (WorkViewResult::Counted, write.receipt.basis()),
            ViewDecision::Skip(receipt) => match receipt.verdict() {
                ViewVerdict::Skipped(skip) => (WorkViewResult::Skipped(skip), receipt.basis()),
                ViewVerdict::Counted => return Err(WorkViewError::Internal),
            },
            ViewDecision::Replay(receipt) => {
                (WorkViewResult::Replayed(receipt.verdict()), receipt.basis())
            }
            ViewDecision::Refuse(refusal) => (WorkViewResult::Ignored(refusal), None),
        };
        Ok(WorkViewRecord {
            work_id: input.work_id,
            event_id,
            result,
            basis,
            decided_at,
        })
    }

    /// Removes windows and receipts older than their retention at `now`, in rounds of at most
    /// `limit` rows each, until a round finds less than a full batch or [`MAX_PRUNE_ROUNDS`] ran.
    /// Totals and per-day totals are never pruned (VIEW-11).
    pub async fn prune_work_views(
        &self,
        now: OffsetDateTime,
        limit: i64,
    ) -> Result<WorkViewPrune, StoreError> {
        let full = u64::try_from(limit.max(1)).unwrap_or(u64::MAX);
        let mut total = WorkViewPrune::default();
        for _ in 0..MAX_PRUNE_ROUNDS {
            let ViewPrune { windows, receipts } = self
                .store()
                .prune_views(microseconds(now), limit.max(1))
                .await?;
            total.windows += windows;
            total.receipts += receipts;
            if windows < full && receipts < full {
                total.drained = true;
                break;
            }
        }
        Ok(total)
    }
}
