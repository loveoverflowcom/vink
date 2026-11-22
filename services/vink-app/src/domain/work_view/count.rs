//! The read rule of work views (#342, VIEW-06..09 and VIEW-12 under `VDR-WORK-VIEWS@r1`): what
//! the author may be told about one work from the stored facts. The production caller is the
//! private author read-model (`persistence/author_works.rs`); the HTTP contract maps the result
//! one to one (`http/author_works.rs`).
//!
//! A "0" is a measured fact, so an absent total is only zero when counting has started (the
//! `work_view_counting` mark, VIEW-09) and the work could have been counted, i.e. it has been
//! public at some point. Otherwise the answer names why there is no number: counting has not
//! started, or the work has never been public (L-01). The total is per `WorkId` and never
//! reset, whatever the current revision or audience (VIEW-06, VIEW-08): this rule does not look
//! at the publication state at all.
//!
//! "Tính từ" (VIEW-09, L-12): the total covers only time since counting started, so the answer
//! carries that instant whenever the work may have been public before it. Only an exact first
//! publication at or after the start drops it (L-13); a legacy or unknown first publication
//! predates the history table, hence the start, or cannot be proven later than it, and keeps the
//! label, which is true in every case.

use time::OffsetDateTime;

/// When the work was first public, as `work_first_publications` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPublic {
    /// `never_public`: no public pointer has ever existed.
    Never,
    /// `exact`: the instant the first public pointer was written.
    At(OffsetDateTime),
    /// `legacy`: public before first publications were recorded (before any counting).
    Legacy,
    /// `unknown`: possibly public before first publications were recorded.
    Unknown,
}

/// The stored facts of one work that the read rule uses, all from one read snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewCountFacts {
    /// `work_view_totals.views`; `None` when the work has no total row.
    pub total: Option<i64>,
    pub first_public: FirstPublic,
    /// `work_view_counting.started_at`; `None` before the first event was ever decided.
    pub counting_started_at: Option<OffsetDateTime>,
}

/// What the author may be told about the views of one work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewCount {
    /// A measured total, possibly zero. `since` is the counting start when the total does not
    /// cover the whole public life of the work ("Tính từ dd/mm/yyyy").
    Counted {
        views: u64,
        since: Option<OffsetDateTime>,
    },
    /// Nothing has been counted on this installation yet: no number, not "0".
    NotStarted,
    /// The work has never been public, so nothing could be counted: no number, not "0" (L-01).
    NeverPublic,
}

/// Stored facts that no write could have produced. The read reports the work's number as
/// unavailable rather than guessing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ViewCountAnomaly {
    #[error("a stored work view total is not positive")]
    NonPositiveTotal,
    #[error("a work has a view total but counting never started")]
    TotalBeforeStart,
    #[error("a never-public work has a view total")]
    TotalWithoutPublication,
}

/// The read rule. See the module documentation.
pub fn read_view_count(facts: ViewCountFacts) -> Result<ViewCount, ViewCountAnomaly> {
    match (facts.total, facts.first_public, facts.counting_started_at) {
        (Some(views), _, _) if views < 1 => Err(ViewCountAnomaly::NonPositiveTotal),
        (Some(_), FirstPublic::Never, _) => Err(ViewCountAnomaly::TotalWithoutPublication),
        (Some(_), _, None) => Err(ViewCountAnomaly::TotalBeforeStart),
        (None, FirstPublic::Never, _) => Ok(ViewCount::NeverPublic),
        (None, _, None) => Ok(ViewCount::NotStarted),
        (total, first_public, Some(start)) => Ok(ViewCount::Counted {
            views: total.map_or(0, i64::unsigned_abs),
            since: counted_since(first_public, start),
        }),
    }
}

/// The "Tính từ" instant: the counting start unless the work was first public at or after it.
fn counted_since(first_public: FirstPublic, start: OffsetDateTime) -> Option<OffsetDateTime> {
    match first_public {
        FirstPublic::At(first) if first >= start => None,
        _ => Some(start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    const START: OffsetDateTime = datetime!(2026-12-01 01:00 UTC);

    fn facts(
        total: Option<i64>,
        first: FirstPublic,
        start: Option<OffsetDateTime>,
    ) -> ViewCountFacts {
        ViewCountFacts {
            total,
            first_public: first,
            counting_started_at: start,
        }
    }

    /// The lifecycle rows of the specification (docs/architecture/work-views.md §7) that the
    /// read side decides, plus every stored shape no write produces.
    #[test]
    fn read_rule_distinguishes_measured_zero_from_no_number() {
        let before = FirstPublic::At(datetime!(2026-11-20 08:00 UTC));
        let after = FirstPublic::At(datetime!(2026-12-01 02:00 UTC));
        let counted = |views, since| Ok(ViewCount::Counted { views, since });
        for (label, input, expected) in [
            // L-01: a draft never public has no number, before and after counting starts.
            (
                "L-01 before start",
                facts(None, FirstPublic::Never, None),
                Ok(ViewCount::NeverPublic),
            ),
            (
                "L-01 after start",
                facts(None, FirstPublic::Never, Some(START)),
                Ok(ViewCount::NeverPublic),
            ),
            // VIEW-09: nothing counted on the installation yet is not "0".
            (
                "not started, public",
                facts(None, after, None),
                Ok(ViewCount::NotStarted),
            ),
            (
                "not started, legacy",
                facts(None, FirstPublic::Legacy, None),
                Ok(ViewCount::NotStarted),
            ),
            // L-02: first public after the start, nobody counted yet: a real 0 without a label.
            ("L-02", facts(None, after, Some(START)), counted(0, None)),
            // L-03/L-13: a total of a work first public after the start.
            (
                "L-03",
                facts(Some(1_234), after, Some(START)),
                counted(1_234, None),
            ),
            // L-12: first public before the start: "Tính từ" the start, also for a zero.
            (
                "L-12",
                facts(Some(3), before, Some(START)),
                counted(3, Some(START)),
            ),
            (
                "L-12 zero",
                facts(None, before, Some(START)),
                counted(0, Some(START)),
            ),
            (
                "legacy",
                facts(Some(2), FirstPublic::Legacy, Some(START)),
                counted(2, Some(START)),
            ),
            (
                "unknown",
                facts(None, FirstPublic::Unknown, Some(START)),
                counted(0, Some(START)),
            ),
            // First public at the very instant counting started: nothing was missed.
            (
                "same instant",
                facts(Some(1), FirstPublic::At(START), Some(START)),
                counted(1, None),
            ),
            // Shapes no write produces are anomalies, never a guessed number.
            (
                "zero row",
                facts(Some(0), after, Some(START)),
                Err(ViewCountAnomaly::NonPositiveTotal),
            ),
            (
                "negative row",
                facts(Some(-1), after, Some(START)),
                Err(ViewCountAnomaly::NonPositiveTotal),
            ),
            (
                "total, no start",
                facts(Some(4), after, None),
                Err(ViewCountAnomaly::TotalBeforeStart),
            ),
            (
                "total, never public",
                facts(Some(4), FirstPublic::Never, Some(START)),
                Err(ViewCountAnomaly::TotalWithoutPublication),
            ),
        ] {
            assert_eq!(read_view_count(input), expected, "{label}");
        }
    }

    /// VIEW-06/VIEW-08: the rule has no publication-state input, so a revision change, a private
    /// or withdrawn work and a republication all read the same stored total.
    #[test]
    fn the_total_is_the_stored_total_whatever_the_current_state() {
        for total in [1_i64, 5, 8, i64::MAX] {
            assert_eq!(
                read_view_count(facts(Some(total), FirstPublic::Legacy, Some(START))),
                Ok(ViewCount::Counted {
                    views: total.unsigned_abs(),
                    since: Some(START)
                })
            );
        }
    }
}
