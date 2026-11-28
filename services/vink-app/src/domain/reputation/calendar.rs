//! Calendar periods and the actor maturity threshold (Q04, Q30, Q33).
//!
//! Weeks run Monday 00:00 to the next Monday 00:00 and months from the 1st 00:00 to the next
//! 1st 00:00, both in Asia/Ho_Chi_Minh, half-open [start, end): an event exactly at `end` belongs
//! to the next period. Asia/Ho_Chi_Minh has used a fixed UTC+07:00 with no daylight saving since
//! 1975, so a fixed offset is exact for every timestamp Vink can hold. Weeks and months are
//! independent calendars; nothing here derives one from the other.

use time::{Date, Duration, Month, OffsetDateTime, Time, UtcOffset};

/// UTC+07:00, Asia/Ho_Chi_Minh.
const LOCAL_OFFSET_SECONDS: i32 = 7 * 3600;
/// Julian day of Monday 1970-01-05, the origin of week indices.
const MONDAY_ORIGIN_JULIAN_DAY: i32 = 2_440_592;
/// Q30: 7 × 24 hours of elapsed time.
pub const MATURITY: Duration = Duration::hours(7 * 24);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Calendar {
    Week,
    Month,
}

impl Calendar {
    pub const ALL: [Self; 2] = [Self::Week, Self::Month];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            _ => None,
        }
    }
}

/// Ordinal of a period within one calendar: weeks since the week of 1970-01-05, months since
/// January of year 0. Consecutive periods differ by exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeriodIndex(i64);

impl PeriodIndex {
    pub const fn new(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

fn local_offset() -> UtcOffset {
    UtcOffset::from_whole_seconds(LOCAL_OFFSET_SECONDS).expect("UTC+07:00 is a valid offset")
}

/// The period of `calendar` containing the instant `at`.
pub fn period_of(calendar: Calendar, at: OffsetDateTime) -> PeriodIndex {
    let local = at.to_offset(local_offset()).date();
    match calendar {
        Calendar::Week => {
            let days = i64::from(local.to_julian_day() - MONDAY_ORIGIN_JULIAN_DAY);
            PeriodIndex(days.div_euclid(7))
        }
        Calendar::Month => {
            PeriodIndex(i64::from(local.year()) * 12 + i64::from(u8::from(local.month())) - 1)
        }
    }
}

/// First instant of a period, in UTC. `None` outside the dates `time` can represent.
pub fn period_start(calendar: Calendar, period: PeriodIndex) -> Option<OffsetDateTime> {
    let date = match calendar {
        Calendar::Week => {
            let day = period
                .0
                .checked_mul(7)?
                .checked_add(i64::from(MONDAY_ORIGIN_JULIAN_DAY))?;
            Date::from_julian_day(i32::try_from(day).ok()?).ok()?
        }
        Calendar::Month => {
            let year = i32::try_from(period.0.div_euclid(12)).ok()?;
            let month = Month::try_from(u8::try_from(period.0.rem_euclid(12) + 1).ok()?).ok()?;
            Date::from_calendar_date(year, month, 1).ok()?
        }
    };
    Some(
        date.with_time(Time::MIDNIGHT)
            .assume_offset(local_offset())
            .to_offset(UtcOffset::UTC),
    )
}

/// End of a period: the start of the next one.
pub fn period_end(calendar: Calendar, period: PeriodIndex) -> Option<OffsetDateTime> {
    period_start(calendar, period.next())
}

/// Q33: the first full period of `calendar` once activation is ready — the period whose start
/// is the first boundary at or after `ready_at`.
pub fn first_full_period(calendar: Calendar, ready_at: OffsetDateTime) -> PeriodIndex {
    let containing = period_of(calendar, ready_at);
    match period_start(calendar, containing) {
        Some(start) if start == ready_at => containing,
        _ => containing.next(),
    }
}

/// Q30: the instant an actor's interactions start to count.
pub fn maturity_threshold(principal_created_at: OffsetDateTime) -> OffsetDateTime {
    principal_created_at.saturating_add(MATURITY)
}

/// Whether an actor created at `principal_created_at` is mature at `at` (elapsed time, UTC).
pub fn is_mature(principal_created_at: OffsetDateTime, at: OffsetDateTime) -> bool {
    at >= maturity_threshold(principal_created_at)
}
