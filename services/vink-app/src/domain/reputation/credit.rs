//! The Q41 community credit machine, one instance per entitlement and calendar.
//!
//! | State | Credit | ON (eligible) | OFF (eligible) | boundary |
//! | --- | --- | --- | --- | --- |
//! | `Unseen` | 0 | → `Active`, +1 | – | – |
//! | `Active` | 1 | – | → `WithdrawnPending(p)`, −1 | – |
//! | `WithdrawnPending(p)` | 0 | in p → `Active`, +1 | – | after p → `Burned` |
//! | `Burned` | 0 | 0 | 0 | terminal |
//!
//! Before the calendar's first full period (Q33) the machine is `Unstarted` and records nothing;
//! the first input at or after that period starts it from the entitlement's effective state at
//! the boundary: effective → `Burned` (baseline), otherwise `Unseen`. Inputs never move a
//! machine to an earlier period than one it already saw (monotone clamp), so a late or
//! out-of-order accepted time cannot reopen a closed withdrawal.

use super::calendar::PeriodIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreditState {
    Unstarted,
    Unseen,
    Active,
    WithdrawnPending(PeriodIndex),
    Burned,
}

impl CreditState {
    /// The credit this state holds (0 or 1, before weights).
    pub const fn credit(self) -> i64 {
        match self {
            Self::Active => 1,
            _ => 0,
        }
    }

    /// Stored code; `WithdrawnPending` keeps its period in a separate column.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unstarted => "unstarted",
            Self::Unseen => "unseen",
            Self::Active => "active",
            Self::WithdrawnPending(_) => "withdrawn",
            Self::Burned => "burned",
        }
    }

    pub const fn pending(self) -> Option<PeriodIndex> {
        match self {
            Self::WithdrawnPending(period) => Some(period),
            _ => None,
        }
    }

    /// Restores a stored state; a pending period exists exactly for `withdrawn`.
    pub fn restore(code: &str, pending: Option<PeriodIndex>) -> Option<Self> {
        match (code, pending) {
            ("unstarted", None) => Some(Self::Unstarted),
            ("unseen", None) => Some(Self::Unseen),
            ("active", None) => Some(Self::Active),
            ("withdrawn", Some(period)) => Some(Self::WithdrawnPending(period)),
            ("burned", None) => Some(Self::Burned),
            _ => None,
        }
    }
}

/// What happened to the entitlement, as far as credit is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreditInput {
    /// The actor's own action made the entitlement effective while eligible.
    On,
    /// The actor's own action made an effective entitlement ineffective while eligible.
    Off,
    /// The actor's own action dropped the last source selection while the entitlement was not
    /// eligible (parked): an `Active` credit retires without a debit.
    OffWhileParked,
    /// Terminal neutral event (DEL-05, author deletion, verified correction): retire, no debit.
    Retire,
    /// Held maturity or baseline: the entitlement counts currently but never as growth.
    Burn,
    /// Any other event: only the boundary roll and the start apply.
    Neutral,
}

/// One calendar's machine and the last period it saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreditRecord {
    state: CreditState,
    last: Option<PeriodIndex>,
}

impl CreditRecord {
    pub const NEW: Self = Self {
        state: CreditState::Unstarted,
        last: None,
    };

    pub const fn restore(state: CreditState, last: Option<PeriodIndex>) -> Self {
        Self { state, last }
    }

    pub const fn state(self) -> CreditState {
        self.state
    }

    pub const fn last(self) -> Option<PeriodIndex> {
        self.last
    }
}

/// Result of one input: the record after it, the period the input was attributed to and the
/// credit delta (−1, 0 or +1) in that period. `period` is `None` while the calendar has not
/// started (partial period before activation readiness).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreditStep {
    pub record: CreditRecord,
    pub period: Option<PeriodIndex>,
    pub delta: i64,
}

/// Applies one input. `at` is the period of the accepted time, `start` the calendar's first
/// full period, `effective_before` whether the entitlement was effective before this input
/// (used only to start the machine at the boundary).
pub fn step(
    record: CreditRecord,
    input: CreditInput,
    at: PeriodIndex,
    start: PeriodIndex,
    effective_before: bool,
) -> CreditStep {
    let period = match record.last {
        Some(last) if last > at => last,
        _ => at,
    };
    if period < start {
        return CreditStep {
            record: CreditRecord {
                state: record.state,
                last: Some(period),
            },
            period: None,
            delta: 0,
        };
    }
    let mut state = match record.state {
        CreditState::Unstarted if effective_before => CreditState::Burned,
        CreditState::Unstarted => CreditState::Unseen,
        CreditState::WithdrawnPending(p) if period > p => CreditState::Burned,
        other => other,
    };
    let mut delta = 0;
    state = match (state, input) {
        (CreditState::Unseen, CreditInput::On) => {
            delta = 1;
            CreditState::Active
        }
        (CreditState::Active, CreditInput::Off) => {
            delta = -1;
            CreditState::WithdrawnPending(period)
        }
        (CreditState::WithdrawnPending(p), CreditInput::On) if p == period => {
            delta = 1;
            CreditState::Active
        }
        (CreditState::Active, CreditInput::OffWhileParked | CreditInput::Retire) => {
            CreditState::Burned
        }
        (CreditState::Unseen | CreditState::WithdrawnPending(_), CreditInput::Retire) => {
            CreditState::Burned
        }
        // Only an entitlement that never held credit can become current-only.
        (CreditState::Unseen, CreditInput::Burn) => CreditState::Burned,
        (state, _) => state,
    };
    CreditStep {
        record: CreditRecord {
            state,
            last: Some(period),
        },
        period: Some(period),
        delta,
    }
}
