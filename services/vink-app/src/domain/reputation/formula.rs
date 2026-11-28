//! `formula_version 1` of Điểm Vink (VDR-REPUTATION@r1 Q25..Q28, contract K2).
//!
//! ```text
//! f(x)  = ln(1 + x/10) / ln 11
//! R     = 1000 + 400 × (0.6 f(I) + 0.4 f(C)) = 1000 + (240 ln a + 160 ln b) / ln 11
//!         with a = (I + 10)/10, b = (C + 10)/10
//! score_micros = floor(R × 10^6),  display = floor((score_micros + 500 000) / 10^6)
//! ```
//!
//! The canonical path is integer only. `ln` of an integer is computed in unsigned fixed point
//! with 96 fractional bits (`ln n = k ln 2 + 2 atanh((n − 2^k)/(n + 2^k))`), so the absolute
//! error of R stays far below 10^-18 over the whole domain, while SR-01 measured the smallest
//! distance of R × 10^6 to an integer as 4.8 × 10^-7 on a 10^6-pair grid. R × 10^6 is an
//! integer only when f(I) and f(C) are both integers (I + 10 and C + 10 are 10 × 11^k); that case
//! is computed exactly instead of through the series, so `floor` never lands one micro low.
//! Golden vectors: `services/vink-app/behavior/reputation/formula-vectors.json`.

/// Exclusive upper bound of I and C (Q27: no business cap, checked numeric bound).
pub const COMPONENT_BOUND: u64 = 1 << 40;

const MICROS: i64 = 1_000_000;
const BASE_MICROS: i64 = 1_000 * MICROS;
/// 400 × 0.6 and 400 × 0.4.
const IMPACT_SCALE: u128 = 240;
const CONTRIBUTION_SCALE: u128 = 160;
/// Growth units per net credit, in tenths: 2.4 and 1.6 (Q28).
const IMPACT_TENTHS: i64 = 24;
const CONTRIBUTION_TENTHS: i64 = 16;

const FRACTION_BITS: u32 = 96;
const ONE: u128 = 1 << FRACTION_BITS;
const LOW_64: u128 = u64::MAX as u128;

/// The formula a stored score was computed with. A new formula is a new variant and never
/// creates growth by itself (Q41 neutral events).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormulaVersion {
    V1,
}

impl FormulaVersion {
    pub const CURRENT: Self = Self::V1;

    pub const fn get(self) -> i32 {
        match self {
            Self::V1 => 1,
        }
    }

    pub const fn from_stored(raw: i32) -> Option<Self> {
        match raw {
            1 => Some(Self::V1),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FormulaError {
    #[error("component count outside [0, 2^40)")]
    OutOfBounds,
    #[error("component count would become negative")]
    Underflow,
    #[error("growth units overflow")]
    Overflow,
}

/// Number of effective entitlements of one component (I or C) of one recipient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ComponentCount(u64);

impl ComponentCount {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Result<Self, FormulaError> {
        if raw < COMPONENT_BOUND {
            Ok(Self(raw))
        } else {
            Err(FormulaError::OutOfBounds)
        }
    }

    /// Restores a stored count (`bigint` columns are signed).
    pub fn from_stored(raw: i64) -> Result<Self, FormulaError> {
        u64::try_from(raw)
            .map_err(|_| FormulaError::Underflow)
            .and_then(Self::new)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    /// Applies one current delta (−1, 0 or +1) with checked bounds.
    pub fn apply(self, delta: CurrentDelta) -> Result<Self, FormulaError> {
        match delta {
            CurrentDelta::None => Ok(self),
            CurrentDelta::Add => Self::new(self.0 + 1),
            CurrentDelta::Remove => self
                .0
                .checked_sub(1)
                .map(Self)
                .ok_or(FormulaError::Underflow),
        }
    }
}

/// Change of one effective entitlement to its recipient's I or C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurrentDelta {
    None,
    Add,
    Remove,
}

impl CurrentDelta {
    pub const fn between(before: bool, after: bool) -> Self {
        match (before, after) {
            (false, true) => Self::Add,
            (true, false) => Self::Remove,
            _ => Self::None,
        }
    }

    pub const fn get(self) -> i64 {
        match self {
            Self::None => 0,
            Self::Add => 1,
            Self::Remove => -1,
        }
    }
}

/// Canonical score: floor(R × 10^6). Ranking uses this value, never the display integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScoreMicros(i64);

impl ScoreMicros {
    /// The score of a principal with no recognized entitlement (Q27). Distinct from
    /// "unavailable", which has no score at all.
    pub const BASELINE: Self = Self(BASE_MICROS);

    pub const fn get(self) -> i64 {
        self.0
    }

    /// Restores a stored canonical score; it can never be below the baseline.
    pub const fn from_stored(raw: i64) -> Option<Self> {
        if raw >= BASE_MICROS {
            Some(Self(raw))
        } else {
            None
        }
    }

    /// The public integer: half-up of R, rounded exactly once (no double rounding, see module).
    pub const fn display(self) -> i64 {
        (self.0 + MICROS / 2) / MICROS
    }
}

/// Q28 growth in tenths of a unit: 24 per net Thấm credit, 16 per net Ưng credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct GrowthTenths(i64);

impl GrowthTenths {
    pub const ZERO: Self = Self(0);

    pub const fn get(self) -> i64 {
        self.0
    }

    pub const fn from_stored(raw: i64) -> Self {
        Self(raw)
    }

    pub fn of(net_impact: i64, net_contribution: i64) -> Result<Self, FormulaError> {
        net_impact
            .checked_mul(IMPACT_TENTHS)
            .zip(net_contribution.checked_mul(CONTRIBUTION_TENTHS))
            .and_then(|(i, c)| i.checked_add(c))
            .map(Self)
            .ok_or(FormulaError::Overflow)
    }

    pub fn checked_add(self, other: Self) -> Result<Self, FormulaError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(FormulaError::Overflow)
    }
}

/// R × 10^6 floored, for formula version 1.
pub fn score_v1(impact: ComponentCount, contribution: ComponentCount) -> ScoreMicros {
    let i = impact.get();
    let c = contribution.get();
    if let (Some(a), Some(b)) = (power_of_eleven(i), power_of_eleven(c)) {
        // Both f values are integers: R is an integer, computed without the series.
        let units = IMPACT_SCALE * u128::from(a) + CONTRIBUTION_SCALE * u128::from(b);
        return ScoreMicros(BASE_MICROS + micros_of_units(units));
    }
    let ln10 = ln_fixed(10);
    // ln(x + 10) >= ln 10 for x >= 0; the same input gives the same value, so x = 0 is exact 0.
    let impact_ln = ln_fixed(i + 10).saturating_sub(ln10);
    let contribution_ln = ln_fixed(c + 10).saturating_sub(ln10);
    let numerator = IMPACT_SCALE * impact_ln + CONTRIBUTION_SCALE * contribution_ln;
    // numerator < 400 × 29 × 2^96 < 2^110 and × 10^6 < 2^130: split the division to stay in u128.
    let whole = numerator / LN_11;
    let remainder = numerator % LN_11;
    // remainder < LN_11 < 2^99, so remainder × 10^6 < 2^119.
    let fraction = remainder * (MICROS as u128) / LN_11;
    let micros = whole * (MICROS as u128) + fraction;
    ScoreMicros(BASE_MICROS + i64::try_from(micros).expect("bounded: R < 5300 for I, C < 2^40"))
}

fn micros_of_units(units: u128) -> i64 {
    i64::try_from(units * MICROS as u128).expect("bounded: f <= 11 for I, C < 2^40")
}

/// `Some(k)` when x + 10 = 10 × 11^k, i.e. f(x) = k exactly.
fn power_of_eleven(x: u64) -> Option<u32> {
    let a = x + 10;
    if !a.is_multiple_of(10) {
        return None;
    }
    let mut m = a / 10;
    let mut k = 0;
    while m.is_multiple_of(11) {
        m /= 11;
        k += 1;
    }
    (m == 1).then_some(k)
}

/// floor(a × b / 2^96) for operands whose product is below 2^224 (every use here: < 2^192).
const fn mul_fixed(a: u128, b: u128) -> u128 {
    let (a_hi, a_lo) = (a >> 64, a & LOW_64);
    let (b_hi, b_lo) = (b >> 64, b & LOW_64);
    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;
    let mid = (ll >> 64) + (lh & LOW_64) + (hl & LOW_64);
    let low = (ll & LOW_64) | ((mid & LOW_64) << 64);
    let high = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);
    (high << (128 - FRACTION_BITS)) | (low >> FRACTION_BITS)
}

/// atanh(t) × 2^96 for 0 <= t <= 1/3 (fixed point), by its odd power series. Each term shrinks
/// by at least 9×, so the loop ends after at most 31 terms; the bound only guards termination.
const fn atanh_fixed(t: u128) -> u128 {
    let square = mul_fixed(t, t);
    let mut power = t;
    let mut sum = t;
    let mut divisor: u128 = 3;
    while divisor < 128 {
        power = mul_fixed(power, square);
        if power == 0 {
            break;
        }
        sum += power / divisor;
        divisor += 2;
    }
    sum
}

/// floor(r × 2^96 / d) for r < d < 2^43, without overflowing u128.
const fn ratio_fixed(r: u128, d: u128) -> u128 {
    let high = (r << 48) / d;
    let rest = (r << 48) % d;
    (high << 48) + (rest << 48) / d
}

/// ln 2 = 2 atanh(1/3).
const LN_2: u128 = 2 * atanh_fixed(ONE / 3);
/// ln 11 through the same integer path as every other logarithm.
const LN_11: u128 = ln_fixed_const(11);

const fn ln_fixed_const(n: u64) -> u128 {
    let k = 63 - n.leading_zeros();
    let base = 1u128 << k;
    let n = n as u128;
    let t = ratio_fixed(n - base, n + base);
    (k as u128) * LN_2 + 2 * atanh_fixed(t)
}

/// ln(n) × 2^96 for 1 <= n <= 2^40 + 10.
fn ln_fixed(n: u64) -> u128 {
    debug_assert!((1..COMPONENT_BOUND + 10).contains(&n));
    ln_fixed_const(n)
}

#[cfg(test)]
pub(super) fn ln_fixed_for_tests(n: u64) -> u128 {
    ln_fixed(n)
}

#[cfg(test)]
pub(super) const FIXED_ONE_FOR_TESTS: u128 = ONE;
