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

/// Position after the last item of a page. Opaque to clients: `c1.<unix micros>.<uuid>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageCursor {
    created_at_micros: i64,
    id: CommentId,
}

const PREFIX: &str = "c1.";

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

    pub fn encode(self) -> String {
        format!(
            "{PREFIX}{}.{}",
            self.created_at_micros,
            self.id.as_uuid().simple()
        )
    }

    pub fn decode(raw: &str) -> Result<Self, PageError> {
        let rest = raw.strip_prefix(PREFIX).ok_or(PageError::Cursor)?;
        let (micros, id) = rest.split_once('.').ok_or(PageError::Cursor)?;
        if micros.len() > 20 || id.len() != 32 {
            return Err(PageError::Cursor);
        }
        let created_at_micros: i64 = micros.parse().map_err(|_| PageError::Cursor)?;
        let id = Uuid::try_parse(id).map_err(|_| PageError::Cursor)?;
        Self::at_micros(created_at_micros, CommentId::from_uuid(id))
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
}
