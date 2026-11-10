//! Keyset paging for roots and replies (D2T-INV-09). Both lists use the chronological baseline
//! `created_at ASC, id ASC`: server-assigned, immutable keys with the ID as tiebreaker. Reply order
//! never follows reactions. Ranked root modes ("Nổi bật") need their own policy/epoch cursor.
//!
//! Keyset paging is not a snapshot: a comment approved later, or committed late with an earlier
//! timestamp, can land before a cursor already handed out. Clients refresh to see such comments;
//! nothing here promises a gap-free stream across requests.
//!
//! A client number is not a page size; only [`PageLimit::new`] bounds it:
//! ```compile_fail
//! use vink_app::domain::discussion::page::PageLimit;
//! let limit = PageLimit(10_000);
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::page::{MAX_PAGE, PageLimit};
//! assert!(PageLimit::new(Some(10_000)).is_err());
//! assert_eq!(PageLimit::new(Some(10)).unwrap().get(), 10);
//! assert!(PageLimit::DEFAULT.get() <= MAX_PAGE);
//! ```

use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::ids::CommentId;

pub const DEFAULT_PAGE: u16 = 20;
pub const MAX_PAGE: u16 = 50;
/// Replies previewed under each root of a root page; the replies page continues after them.
pub const PREVIEW_REPLIES: u16 = 3;

/// Server-owned page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageError {
    #[error("limit must be between 1 and {MAX_PAGE}")]
    Limit,
    #[error("cursor is malformed or from another version")]
    Cursor,
}

impl PageLimit {
    pub const DEFAULT: Self = Self(DEFAULT_PAGE);

    pub fn new(requested: Option<u32>) -> Result<Self, PageError> {
        match requested {
            None => Ok(Self::DEFAULT),
            Some(raw) if (1..=u32::from(MAX_PAGE)).contains(&raw) => {
                Ok(Self(u16::try_from(raw).map_err(|_| PageError::Limit)?))
            }
            Some(_) => Err(PageError::Limit),
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Position after the last item of a page. Opaque to clients: `<scope>1.<unix micros>.<uuid>`.
///
/// A cursor is a position, never a capability: every read re-applies its own visibility and
/// actor filters after it, so a forged position can only skip rows the reader could already
/// page through. The scope prefix keeps lists apart anyway, so a cursor handed out by one list
/// is refused by another instead of silently landing somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageCursor {
    created_at_micros: i64,
    id: CommentId,
}

/// Which list issued a cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorScope {
    /// Public thread lists: roots, replies and the moderation queue (unchanged `c1.` format).
    Thread,
    /// The viewer's private overlay of their own unpublished comments (#120 DISC-02/03).
    Own,
}

impl CursorScope {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Thread => "c1.",
            Self::Own => "o1.",
        }
    }
}

/// Longest cursor any list issues: prefix, at most 20 sign/digit bytes, `.`, 32 hex digits.
/// Longer input is refused before any parsing.
pub const MAX_CURSOR_BYTES: usize = 3 + 20 + 1 + 32;

impl PageCursor {
    /// Timestamps are kept at the database's microsecond precision.
    pub fn after(created_at: OffsetDateTime, id: CommentId) -> Self {
        let micros = created_at.unix_timestamp_nanos().div_euclid(1_000);
        Self {
            created_at_micros: i64::try_from(micros).unwrap_or(i64::MAX),
            id,
        }
    }

    pub fn created_at(self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(self.created_at_micros) * 1_000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    pub const fn id(self) -> CommentId {
        self.id
    }

    /// A thread-list cursor (`c1.`).
    pub fn encode(self) -> String {
        self.encode_in(CursorScope::Thread)
    }

    pub fn encode_in(self, scope: CursorScope) -> String {
        format!(
            "{}{}.{}",
            scope.prefix(),
            self.created_at_micros,
            self.id.as_uuid().simple()
        )
    }

    /// Decodes a thread-list cursor (`c1.`).
    pub fn decode(raw: &str) -> Result<Self, PageError> {
        Self::decode_in(raw, CursorScope::Thread)
    }

    /// Accepts exactly the strings [`Self::encode_in`] issues for `scope`: anything longer than
    /// [`MAX_CURSOR_BYTES`], of another scope or version, or a non-canonical spelling of a valid
    /// position (`+1`, leading zeros, `-0`, upper-case hex) is refused, never normalized.
    pub fn decode_in(raw: &str, scope: CursorScope) -> Result<Self, PageError> {
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(PageError::Cursor);
        }
        let rest = raw.strip_prefix(scope.prefix()).ok_or(PageError::Cursor)?;
        let (micros, id) = rest.split_once('.').ok_or(PageError::Cursor)?;
        if micros.len() > 20 || id.len() != 32 {
            return Err(PageError::Cursor);
        }
        let created_at_micros: i64 = micros.parse().map_err(|_| PageError::Cursor)?;
        let id = Uuid::try_parse(id).map_err(|_| PageError::Cursor)?;
        let cursor = Self::at_micros(created_at_micros, CommentId::from_uuid(id))?;
        if cursor.encode_in(scope) != raw {
            return Err(PageError::Cursor);
        }
        Ok(cursor)
    }

    /// The decoded position, if it is a representable timestamp. Positions that cannot round-trip
    /// through [`Self::created_at`] are rejected instead of clamped, so a cursor never lands on
    /// another key than the one it was issued for.
    pub(super) fn at_micros(created_at_micros: i64, id: CommentId) -> Result<Self, PageError> {
        let nanos = i128::from(created_at_micros) * 1_000;
        OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| PageError::Cursor)?;
        Ok(Self {
            created_at_micros,
            id,
        })
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn limits_are_server_owned_and_bounded() {
        assert_eq!(PageLimit::new(None), Ok(PageLimit::DEFAULT));
        assert_eq!(PageLimit::new(Some(1)).unwrap().get(), 1);
        assert_eq!(PageLimit::new(Some(50)).unwrap().get(), 50);
        for bad in [0, 51, u32::MAX] {
            assert_eq!(PageLimit::new(Some(bad)), Err(PageError::Limit));
        }
    }

    #[test]
    fn cursors_round_trip_at_microsecond_precision() {
        let at = datetime!(2026-11-03 09:15:42.123456789 UTC);
        let id = CommentId::from_uuid(Uuid::from_u128(0x0192_aaaa_bbbb_7ccc_8ddd_eeee_ffff_0000));
        let cursor = PageCursor::after(at, id);
        let decoded = PageCursor::decode(&cursor.encode()).unwrap();
        assert_eq!(decoded, cursor);
        assert_eq!(
            decoded.created_at(),
            datetime!(2026-11-03 09:15:42.123456 UTC)
        );
        assert_eq!(decoded.id(), id);
        let before_epoch = PageCursor::after(datetime!(1969-12-31 23:59:59.5 UTC), id);
        assert_eq!(PageCursor::decode(&before_epoch.encode()), Ok(before_epoch));
    }

    /// -9999-01-01T00:00:00Z and 9999-12-31T23:59:59.999999Z, the `time` range without
    /// `large-dates`, in microseconds.
    const MIN_MICROS: i64 = -377_705_116_800_000_000;
    const MAX_MICROS: i64 = 253_402_300_799_999_999;

    #[test]
    fn the_extreme_representable_positions_round_trip() {
        let id = CommentId::from_uuid(Uuid::from_u128(1));
        for micros in [MIN_MICROS, -1, 0, MAX_MICROS] {
            let cursor = PageCursor::at_micros(micros, id).unwrap();
            assert_eq!(PageCursor::decode(&cursor.encode()), Ok(cursor));
            assert_eq!(PageCursor::after(cursor.created_at(), id), cursor);
        }
    }

    #[test]
    fn foreign_or_forged_cursors_are_rejected() {
        let id = "0192aaaabbbb7ccc8dddeeeeffff0000";
        for raw in [
            "",
            "c1.",
            "c2.1.0192aaaabbbb7ccc8dddeeeeffff0000",
            "c1.abc.0192aaaabbbb7ccc8dddeeeeffff0000",
            "c1.1.not-a-uuid",
            "c1.1.0192aaaa-bbbb-7ccc-8ddd-eeeeffff0000",
            "c1.99999999999999999999.0192aaaabbbb7ccc8dddeeeeffff0000",
            &format!("c1.{}.{id}", i64::MAX),
            &format!("c1.{}.{id}", MAX_MICROS + 1),
            &format!("c1.{}.{id}", MIN_MICROS - 1),
            &format!("c1.1.{id}.extra"),
        ] {
            assert_eq!(PageCursor::decode(raw), Err(PageError::Cursor), "{raw}");
        }
    }

    /// Each of these names a valid position in a spelling the server never issues; accepting
    /// them would give one position many cursors and let a client probe the parser.
    #[test]
    fn non_canonical_spellings_are_rejected_not_normalized() {
        let id = "0192aaaabbbb7ccc8dddeeeeffff0000";
        let canonical = format!("c1.1.{id}");
        assert!(PageCursor::decode(&canonical).is_ok());
        for raw in [
            format!("c1.+1.{id}"),
            format!("c1.01.{id}"),
            format!("c1.-0.{id}"),
            format!("c1.00.{id}"),
            format!("c1.1.{}", id.to_uppercase()),
            format!("C1.1.{id}"),
            format!(" c1.1.{id}"),
            format!("c1.1.{id} "),
            format!("c1.1.{id}\n"),
            format!("c1.1\u{0}.{id}"),
            format!("c1.１.{id}"),
        ] {
            assert_eq!(PageCursor::decode(&raw), Err(PageError::Cursor), "{raw:?}");
        }
    }

    #[test]
    fn oversized_input_is_refused_before_parsing() {
        let longest =
            PageCursor::at_micros(-377_705_116_800_000_000, CommentId::from_uuid(Uuid::max()))
                .unwrap()
                .encode();
        assert!(longest.len() <= MAX_CURSOR_BYTES, "{longest}");
        assert!(PageCursor::decode(&longest).is_ok());
        for len in [MAX_CURSOR_BYTES + 1, 4096, 1 << 20] {
            let raw = format!("c1.{}", "9".repeat(len));
            assert_eq!(PageCursor::decode(&raw), Err(PageError::Cursor));
        }
        // Multi-byte input counts bytes, not characters.
        let wide = format!("c1.1.{}", "ệ".repeat(20));
        assert_eq!(PageCursor::decode(&wide), Err(PageError::Cursor));
    }

    #[test]
    fn scopes_do_not_accept_each_others_cursors() {
        let cursor = PageCursor::at_micros(1, CommentId::from_uuid(Uuid::from_u128(9))).unwrap();
        let thread = cursor.encode();
        let own = cursor.encode_in(CursorScope::Own);
        assert!(thread.starts_with("c1.") && own.starts_with("o1."));
        assert_eq!(PageCursor::decode_in(&own, CursorScope::Own), Ok(cursor));
        assert_eq!(
            PageCursor::decode_in(&thread, CursorScope::Thread),
            Ok(cursor)
        );
        assert_eq!(
            PageCursor::decode_in(&thread, CursorScope::Own),
            Err(PageError::Cursor)
        );
        assert_eq!(
            PageCursor::decode_in(&own, CursorScope::Thread),
            Err(PageError::Cursor)
        );
    }
}
