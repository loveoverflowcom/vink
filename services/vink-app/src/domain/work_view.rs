//! Work views ("Lượt xem", #341): the pure write rule of VIEW-01, 03..06, 08, 10 and 11 under
//! `VDR-WORK-VIEWS@r1` (docs/architecture/work-views.md). The production caller is the
//! `PgStore` transaction (`persistence/work_view.rs`), which loads the facts under locks, calls
//! [`decide_view`] and applies exactly the decision returned here; runtime status and evidence
//! live in docs/development/work-views.md. What the author is told from the stored totals (#342)
//! is the separate read rule in [`count`].
//!
//! **What is decided.** One event is one displayed public body in one tab (#343 emits it). Two
//! separate checks apply, in this order (VIEW-03):
//!
//! 1. *Idempotency of one event.* An event that already has a receipt is a replay: it reports the
//!    first decision and writes nothing, whatever the reader or the clock say now. A receipt of
//!    another work under the same event ID is a reuse and is refused.
//! 2. *Merging of independent opens.* A reader (member: `PrincipalId`; guest: a random
//!    first-party token) has at most one counted view per work per sliding [`VIEW_WINDOW`],
//!    anchored at the counted view. A blocked open neither counts nor extends the window. The
//!    window is still open at exactly 24 hours and closed one instant later ("đã quá 24 giờ").
//!
//! Between the two, the event must lie inside the acceptance horizon (its UUIDv7 time no more
//! than [`MAX_EVENT_AGE`] behind or [`MAX_EVENT_LEAD`] ahead of the decision clock), the work
//! must exist and be public at the decision (any revision: the key is the `WorkId`, VIEW-06), and
//! a member reader must not own the work (VIEW-05). Because a receipt is retained for
//! [`RECEIPT_RETENTION`] > `MAX_EVENT_AGE + MAX_EVENT_LEAD`, any replay that passes the horizon
//! still finds its receipt; a later replay is refused as outside the horizon, so a technical
//! duplicate never becomes a new view, even after retention removed its receipt.
//!
//! **What is stored** (VIEW-11): a total and a per-day total per work, the keyed digest of
//! (reader, work) for the open window, and per-event receipts without any reader. The digest is
//! an HMAC under a server key ([`ViewDigestKey`]) so a database copy alone cannot test whether a
//! known member read a work; it is bound to the work, so rows of one reader cannot be joined
//! across works. No IP address, fingerprint or reading history is an input.
//!
//! **Anti-inflation rule** (CHOICE-06 A; applied at HTTP by #343): input validation here, the two
//! checks above, [`INGEST_LIMIT`] per attributed network in process memory, and no count for a
//! speculative (prefetch/prerender) request ([`is_speculative`]). Bots that run JavaScript and
//! rotate guest tokens are not fully excluded; VIEW-02 promises no more.

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use time::macros::offset;
use time::{Date, Duration, OffsetDateTime, UtcOffset};
use uuid::{Uuid, Variant, Version};

use super::ids::{PrincipalId, RevisionId, WorkId};

pub mod count;
#[cfg(test)]
mod tests;

/// VIEW-03: one counted view per reader and work per sliding 24 hours. The value is shared with
/// the Web explanation through `vink-work-view-policy`; the Reader's foreground threshold is a
/// browser-only emission condition and is deliberately not an input here.
pub const VIEW_WINDOW: Duration = Duration::hours(vink_work_view_policy::VIEW_WINDOW_HOURS as i64);
/// Oldest event time (from its UUIDv7) still decided, relative to the decision clock.
pub const MAX_EVENT_AGE: Duration = Duration::hours(11);
/// Furthest an event time may lie ahead of the decision clock (client clock skew).
pub const MAX_EVENT_LEAD: Duration = Duration::hours(11);
/// Receipts expire with the window (VIEW-11). Must exceed the horizon width, checked below.
pub const RECEIPT_RETENTION: Duration = VIEW_WINDOW;
/// A guest token lives this long from issue and is never renewed (CHOICE-03: fixed expiry), so
/// no token outlives one window. The HTTP adapter sets it as the cookie lifetime.
pub const GUEST_TOKEN_TTL: Duration = VIEW_WINDOW;
/// The day of the per-day total: `Asia/Ho_Chi_Minh`, a fixed UTC+7 without daylight saving since
/// 1975, as the Web shows dates.
pub const VIEW_DAY_OFFSET: UtcOffset = offset!(+7);

const _: () = assert!(
    MAX_EVENT_AGE.whole_seconds() + MAX_EVENT_LEAD.whole_seconds()
        < RECEIPT_RETENTION.whole_seconds(),
    "a replay inside the horizon must always find its receipt"
);
const _: () = assert!(GUEST_TOKEN_TTL.whole_seconds() <= VIEW_WINDOW.whole_seconds());

/// The identity of one displayed body in one tab: a UUIDv7 minted by the tab when the body is
/// shown and reused for every retry (VIEW-10). Its embedded time bounds how late it is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewEventId(Uuid);

impl ViewEventId {
    /// Any textual UUID form; only an RFC 9562 version 7 UUID is an event ID.
    pub fn parse(raw: &str) -> Option<Self> {
        Self::from_uuid(Uuid::try_parse(raw).ok()?)
    }

    pub fn from_uuid(id: Uuid) -> Option<Self> {
        (id.get_version() == Some(Version::SortRand) && id.get_variant() == Variant::RFC4122)
            .then_some(Self(id))
    }

    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// The time in the UUID, `None` when it does not fit a calendar date (a forged ID).
    pub fn issued_at(&self) -> Option<OffsetDateTime> {
        let (seconds, nanos) = self.0.get_timestamp()?.to_unix();
        let seconds = i64::try_from(seconds).ok()?;
        OffsetDateTime::from_unix_timestamp(seconds)
            .ok()?
            .checked_add(Duration::nanoseconds(i64::from(nanos)))
    }

    /// Inside the acceptance horizon at `now`: at most [`MAX_EVENT_AGE`] old and at most
    /// [`MAX_EVENT_LEAD`] ahead, both bounds inclusive.
    pub fn within_horizon(&self, now: OffsetDateTime) -> bool {
        self.issued_at()
            .is_some_and(|at| now - at <= MAX_EVENT_AGE && at - now <= MAX_EVENT_LEAD)
    }
}

/// A random first-party guest token (VIEW-04): an RFC 9562 version 4 UUID. It names a browser,
/// not a person, and is used only to merge that browser's opens of a work. Never logged.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GuestToken(Uuid);

impl GuestToken {
    pub fn parse(raw: &str) -> Option<Self> {
        let id = Uuid::try_parse(raw).ok()?;
        (id.get_version() == Some(Version::Random) && id.get_variant() == Variant::RFC4122)
            .then_some(Self(id))
    }

    /// A fresh token for a browser that has none (the HTTP adapter issues it).
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// The token text the browser keeps.
    pub fn expose(&self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Debug for GuestToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GuestToken(..)")
    }
}

/// Who opened the body, as a measurement identity (not a real person). A member is the
/// session's principal; a guest is a browser token. The two never merge (E-30): a guest who
/// signs in during the window may be counted once more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewReader {
    Member(PrincipalId),
    Guest(GuestToken),
}

/// The server key of reader digests: 32 bytes from configuration, never stored in the database.
/// Replacing it starts every open window afresh, so each (reader, work) may count once more
/// within the next 24 hours; rotate only when the key leaked.
#[derive(Clone, PartialEq, Eq)]
pub struct ViewDigestKey([u8; 32]);

impl ViewDigestKey {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exactly 64 hexadecimal digits.
    pub fn parse_hex(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.len() != 64 || !raw.is_ascii() {
            return None;
        }
        let mut bytes = [0; 32];
        let (pairs, _) = raw.as_bytes().as_chunks::<2>();
        for (out, pair) in bytes.iter_mut().zip(pairs) {
            *out = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        Some(Self(bytes))
    }
}

impl std::fmt::Debug for ViewDigestKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ViewDigestKey(..)")
    }
}

/// The stored dedupe key of one (reader, work): HMAC-SHA-256 under [`ViewDigestKey`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReaderDigest([u8; 32]);

impl ReaderDigest {
    pub fn derive(key: &ViewDigestKey, reader: ViewReader, work: WorkId) -> Self {
        let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).expect("HMAC accepts a 32-byte key");
        mac.update(b"vink:work-view:reader:v1\0");
        match reader {
            ViewReader::Member(principal) => {
                mac.update(b"m");
                mac.update(principal.as_uuid().as_bytes());
            }
            ViewReader::Guest(token) => {
                mac.update(b"g");
                mac.update(token.0.as_bytes());
            }
        }
        mac.update(work.as_uuid().as_bytes());
        Self(mac.finalize().into_bytes().into())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for ReaderDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReaderDigest(..)")
    }
}

/// The stored key of an event receipt: SHA-256 of the event ID, so the receipt keeps neither the
/// client's clock nor an ID that HTTP logs could match.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventDigest([u8; 32]);

impl EventDigest {
    pub fn of(event: ViewEventId) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"vink:work-view:event:v1\0");
        hasher.update(event.0.as_bytes());
        Self(hasher.finalize().into())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hex, the text of the event's advisory lock key.
    pub fn hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl std::fmt::Debug for EventDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventDigest({})", &self.hex()[..8])
    }
}

/// A validated view event: which work, which displayed body, who.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewEvent {
    pub work: WorkId,
    pub id: ViewEventId,
    pub reader: ViewReader,
}

/// Why a decided event added nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewSkip {
    /// The work has no public pointer at the decision: private, hidden, withdrawn after review,
    /// never public (VIEW-01, 08; E-12, E-13, E-34).
    NotPublic,
    /// The member reader owns the work (VIEW-05; E-22).
    OwnWork,
    /// The reader already has a counted view of this work in the open window (VIEW-03).
    WithinWindow,
}

/// The durable decision of one event, kept in its receipt and reported by every replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewVerdict {
    Counted,
    Skipped(ViewSkip),
}

impl ViewVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Counted => "counted",
            Self::Skipped(ViewSkip::WithinWindow) => "within_window",
            Self::Skipped(ViewSkip::OwnWork) => "own_work",
            Self::Skipped(ViewSkip::NotPublic) => "not_public",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "counted" => Self::Counted,
            "within_window" => Self::Skipped(ViewSkip::WithinWindow),
            "own_work" => Self::Skipped(ViewSkip::OwnWork),
            "not_public" => Self::Skipped(ViewSkip::NotPublic),
            _ => return None,
        })
    }
}

/// The receipt of one decided event: its work, its verdict and the public revision the decision
/// saw (the publication basis), `None` exactly when the work was not public.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewReceipt {
    work: WorkId,
    verdict: ViewVerdict,
    basis: Option<RevisionId>,
}

impl ViewReceipt {
    /// `None` for a combination no decision produces.
    pub fn restore(work: WorkId, verdict: ViewVerdict, basis: Option<RevisionId>) -> Option<Self> {
        let not_public = verdict == ViewVerdict::Skipped(ViewSkip::NotPublic);
        (not_public == basis.is_none()).then_some(Self {
            work,
            verdict,
            basis,
        })
    }

    pub const fn work(&self) -> WorkId {
        self.work
    }

    pub const fn verdict(&self) -> ViewVerdict {
        self.verdict
    }

    pub const fn basis(&self) -> Option<RevisionId> {
        self.basis
    }
}

/// Why an event was not decided at all. Nothing is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewRefusal {
    /// The event time is outside the acceptance horizon (or unreadable): a stale replay, a
    /// skewed clock, a forged ID.
    OutsideHorizon,
    /// The event ID already has a receipt for another work.
    EventReused,
    /// No such work. The HTTP adapter must answer it exactly like [`ViewSkip::NotPublic`], so the
    /// ingest reveals nothing about private works.
    WorkNotFound,
}

/// The facts of a work at the decision, read under the publication share lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkFacts {
    pub owner: PrincipalId,
    /// The current public pointer; `None` when the work is not public.
    pub published: Option<RevisionId>,
}

/// Everything [`decide_view`] reads, loaded by one transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewFacts {
    /// `None`: the work does not exist.
    pub work: Option<WorkFacts>,
    /// The receipt of this event ID, if any (any work, any age).
    pub receipt: Option<ViewReceipt>,
    /// When the reader's last counted view of this work was counted, if a window row exists
    /// (open or not yet removed).
    pub window: Option<OffsetDateTime>,
    /// The decision clock.
    pub now: OffsetDateTime,
}

/// A counted view: the window to claim, the totals to bump and the receipt to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountWrite {
    /// The expired window row this view replaces (`counted_at` it held), or `None` to insert one.
    pub replaces: Option<OffsetDateTime>,
    pub counted_at: OffsetDateTime,
    /// The bucket of the per-day total.
    pub day: Date,
    pub receipt: ViewReceipt,
}

/// What the transaction must do, and the only thing it may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDecision {
    /// +1 to the total and to `day`, claim the window, keep the receipt.
    Count(CountWrite),
    /// +0; keep the receipt so a retry replays it.
    Skip(ViewReceipt),
    /// +0; the event was decided before. Nothing is written.
    Replay(ViewReceipt),
    /// +0; not decided. Nothing is written.
    Refuse(ViewRefusal),
}

impl ViewDecision {
    /// The change this decision makes to the work's total.
    pub const fn delta(&self) -> i64 {
        match self {
            Self::Count(_) => 1,
            _ => 0,
        }
    }

    /// The receipt this decision writes, if it writes one.
    pub const fn new_receipt(&self) -> Option<ViewReceipt> {
        match self {
            Self::Count(write) => Some(write.receipt),
            Self::Skip(receipt) => Some(*receipt),
            Self::Replay(_) | Self::Refuse(_) => None,
        }
    }
}

/// `true` while a view counted at `counted_at` still blocks a new one at `now`: up to and
/// including exactly [`VIEW_WINDOW`] later. A clock that went backwards keeps it open.
pub fn window_open(counted_at: OffsetDateTime, now: OffsetDateTime) -> bool {
    now - counted_at <= VIEW_WINDOW
}

/// The per-day bucket of a view decided at `at`.
pub fn view_day(at: OffsetDateTime) -> Date {
    at.to_offset(VIEW_DAY_OFFSET).date()
}

/// The write rule. See the module documentation for the order of the checks.
pub fn decide_view(event: &ViewEvent, facts: ViewFacts) -> ViewDecision {
    if let Some(receipt) = facts.receipt {
        return if receipt.work == event.work {
            ViewDecision::Replay(receipt)
        } else {
            ViewDecision::Refuse(ViewRefusal::EventReused)
        };
    }
    if !event.id.within_horizon(facts.now) {
        return ViewDecision::Refuse(ViewRefusal::OutsideHorizon);
    }
    let Some(work) = facts.work else {
        return ViewDecision::Refuse(ViewRefusal::WorkNotFound);
    };
    let skip = |skip: ViewSkip, basis: Option<RevisionId>| {
        ViewDecision::Skip(ViewReceipt {
            work: event.work,
            verdict: ViewVerdict::Skipped(skip),
            basis,
        })
    };
    let Some(basis) = work.published else {
        return skip(ViewSkip::NotPublic, None);
    };
    if matches!(event.reader, ViewReader::Member(reader) if reader == work.owner) {
        return skip(ViewSkip::OwnWork, Some(basis));
    }
    if facts
        .window
        .is_some_and(|counted_at| window_open(counted_at, facts.now))
    {
        return skip(ViewSkip::WithinWindow, Some(basis));
    }
    ViewDecision::Count(CountWrite {
        replaces: facts.window,
        counted_at: facts.now,
        day: view_day(facts.now),
        receipt: ViewReceipt {
            work: event.work,
            verdict: ViewVerdict::Counted,
            basis: Some(basis),
        },
    })
}

/// The per-network ingest budget (CHOICE-06 A), applied by the HTTP adapter to the network that
/// `abuse::client_address` attributes. It is kept in process memory only: no network key of view
/// traffic is stored. A refused request adds nothing and never blocks reading; a shared network
/// (school, office, mobile carrier) over budget may lose views, which VIEW-10 accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestLimit {
    pub requests: u32,
    pub window_seconds: u32,
}

impl IngestLimit {
    /// `true` when one more request fits after `seen` requests in the current window.
    pub const fn admits(&self, seen: u32) -> bool {
        seen < self.requests
    }
}

/// Generous for people: one reader opening a work every few seconds with bounded retries stays
/// well below it; a script replaying events from one network does not.
pub const INGEST_LIMIT: IngestLimit = IngestLimit {
    requests: 120,
    window_seconds: 600,
};

/// `true` for a speculative request: any `prefetch` or `prerender` token in a `Sec-Purpose`,
/// `Purpose` or `X-Purpose` header value. A prerendered page runs JavaScript before anyone
/// navigates to it; its request must add nothing (VIEW-02, E-08).
pub fn is_speculative<'a>(purpose_values: impl IntoIterator<Item = &'a str>) -> bool {
    purpose_values.into_iter().any(|value| {
        value.split([';', ',', ' ']).map(str::trim).any(|token| {
            token.eq_ignore_ascii_case("prefetch") || token.eq_ignore_ascii_case("prerender")
        })
    })
}
