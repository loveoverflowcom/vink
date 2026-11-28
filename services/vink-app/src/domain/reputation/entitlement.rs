//! One entitlement — (actor, WorkId) for Thấm or (actor, CommentId) for Ưng — and how a change
//! of its facts moves the recipient's current count and the two credit machines (Q02, Q13, Q14,
//! Q19, Q20, Q23, Q41, Q42).
//!
//! The decision compares the STORED effective state with the effective state of the facts
//! re-read after the change, so a replayed or stale input that changes nothing yields nothing.
//! Callers apply maturity (`Cause::Maturity`, as of the threshold) before any later input of the
//! same actor; then a held ON that is still selected at the threshold is current-only, and a
//! later selection of another anchor of the same entitlement is not a new ON.

use super::calendar::PeriodIndex;
use super::credit::{CreditInput, CreditRecord, CreditStep, step};
use super::formula::CurrentDelta;

/// Which component of the recipient an entitlement feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Component {
    /// Ảnh hưởng: Thấm received on works, deduplicated by (actor, WorkId).
    Impact,
    /// Đóng góp: Ưng received on comments, by (actor, CommentId).
    Contribution,
}

impl Component {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Impact => "impact",
            Self::Contribution => "contribution",
        }
    }
}

/// Authoritative facts of one entitlement after a change, re-read by the caller under the
/// source's own order. Anchor counts are bounded by revisions per work (Impact) or 1 (Ưng).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntitlementFacts {
    /// Anchors the actor currently selects at the source (Thấm revisions, or the comment).
    pub selected_anchors: u32,
    /// Selected anchors that are valid: not a hidden revision, not a verified violation.
    pub valid_selected_anchors: u32,
    /// The work is public and active; for Ưng also the comment is approved and present.
    pub target_public: bool,
    /// The actor is past `principals.created_at + 7 × 24h` as of the change.
    pub actor_mature: bool,
}

impl EntitlementFacts {
    pub const fn effective(self) -> bool {
        self.target_public && self.actor_mature && self.valid_selected_anchors > 0
    }
}

/// Why the facts changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cause {
    /// The actor selected or removed an anchor (the source version changed).
    Community,
    /// Lifecycle or anchor validity changed without the actor (privacy, withdrawal, hiding,
    /// comment removal or review, republication, restoration).
    Neutral,
    /// DEL-05 actor deletion, author deletion or a verified correction: retire for good.
    Terminal,
    /// The actor reached the maturity threshold; facts are as of the threshold.
    Maturity,
}

/// Stored state of one entitlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntitlementRecord {
    effective: bool,
    terminal: bool,
    week: CreditRecord,
    month: CreditRecord,
}

impl EntitlementRecord {
    pub const NEW: Self = Self {
        effective: false,
        terminal: false,
        week: CreditRecord::NEW,
        month: CreditRecord::NEW,
    };

    pub const fn restore(
        effective: bool,
        terminal: bool,
        week: CreditRecord,
        month: CreditRecord,
    ) -> Option<Self> {
        if terminal && effective {
            return None;
        }
        Some(Self {
            effective,
            terminal,
            week,
            month,
        })
    }

    pub const fn effective(self) -> bool {
        self.effective
    }

    pub const fn terminal(self) -> bool {
        self.terminal
    }

    pub const fn week(self) -> CreditRecord {
        self.week
    }

    pub const fn month(self) -> CreditRecord {
        self.month
    }
}

/// The periods of the accepted time and each calendar's first full period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Periods {
    pub week: PeriodIndex,
    pub month: PeriodIndex,
    pub week_start: PeriodIndex,
    pub month_start: PeriodIndex,
}

/// Effects of one change. `current` applies to the recipient's count of `Component`; each
/// credit step carries its own period and −1/0/+1 delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Applied {
    pub record: EntitlementRecord,
    pub current: CurrentDelta,
    pub week: CreditStep,
    pub month: CreditStep,
}

impl Applied {
    /// Whether anything must be written.
    pub fn changed(&self, before: EntitlementRecord) -> bool {
        self.record != before || self.current != CurrentDelta::None
    }
}

pub fn apply(
    record: EntitlementRecord,
    cause: Cause,
    facts: EntitlementFacts,
    periods: Periods,
) -> Applied {
    if record.terminal {
        // No resurrection: nothing a later input says can bring a retired entitlement back.
        let idle = |credit: CreditRecord| CreditStep {
            record: credit,
            period: None,
            delta: 0,
        };
        return Applied {
            record,
            current: CurrentDelta::None,
            week: idle(record.week),
            month: idle(record.month),
        };
    }
    let before = record.effective;
    let after = match cause {
        Cause::Terminal => false,
        _ => facts.effective(),
    };
    let input = match (cause, before, after) {
        (Cause::Terminal, _, _) => CreditInput::Retire,
        (Cause::Community, false, true) => CreditInput::On,
        (Cause::Community, true, false) => CreditInput::Off,
        (Cause::Community, false, false) if facts.selected_anchors == 0 => {
            CreditInput::OffWhileParked
        }
        (Cause::Maturity, false, true) => CreditInput::Burn,
        _ => CreditInput::Neutral,
    };
    let week = step(record.week, input, periods.week, periods.week_start, before);
    let month = step(
        record.month,
        input,
        periods.month,
        periods.month_start,
        before,
    );
    Applied {
        record: EntitlementRecord {
            effective: after,
            terminal: cause == Cause::Terminal,
            week: week.record,
            month: month.record,
        },
        current: CurrentDelta::between(before, after),
        week,
        month,
    }
}
