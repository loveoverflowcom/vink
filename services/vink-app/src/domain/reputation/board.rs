//! Top Rating and Top Growth admission and canonical order (Q17, Q29, Q43).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::formula::{ComponentCount, FormulaError, GrowthTenths, ScoreMicros};
use crate::domain::ids::PrincipalId;

/// Entries per board (Q29).
pub const BOARD_SIZE: usize = 100;
/// Extra closed-period candidates kept beyond the board (Q44); an optimization, never a guarantee.
pub const CLOSED_RESERVE: usize = 50;
pub const PAGE_DEFAULT: usize = 20;
pub const PAGE_MAX: usize = 100;
/// Q43: distinct actors with positive weighted net credit a Top Growth entry needs.
pub const MIN_DISTINCT_ACTORS: u32 = 3;

/// Top Rating admits recognized members only: baseline-only (I = C = 0) stays out (Q17).
/// Comment-only members are admitted (Q11).
pub const fn rating_admits(impact: ComponentCount, contribution: ComponentCount) -> bool {
    impact.get() > 0 || contribution.get() > 0
}

/// A recipient's growth in one period, from the weighted net credit of each actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrowthQualification {
    pub total: GrowthTenths,
    pub positive_actors: u32,
}

impl GrowthQualification {
    /// Q43: G > 0 and at least three distinct actors with positive weighted net credit.
    pub const fn admits(self) -> bool {
        self.total.get() > 0 && self.positive_actors >= MIN_DISTINCT_ACTORS
    }
}

/// Sums weighted net credit per actor first, so several entitlements of one actor count as one
/// actor, then counts actors whose net is positive. Unlike-only or zero actors never count.
pub fn qualify_growth(
    nets: impl IntoIterator<Item = (PrincipalId, GrowthTenths)>,
) -> Result<GrowthQualification, FormulaError> {
    let mut per_actor: BTreeMap<PrincipalId, GrowthTenths> = BTreeMap::new();
    for (actor, net) in nets {
        let entry = per_actor.entry(actor).or_insert(GrowthTenths::ZERO);
        *entry = entry.checked_add(net)?;
    }
    let mut total = GrowthTenths::ZERO;
    let mut positive_actors = 0u32;
    for net in per_actor.values() {
        total = total.checked_add(*net)?;
        if net.get() > 0 {
            positive_actors = positive_actors.saturating_add(1);
        }
    }
    Ok(GrowthQualification {
        total,
        positive_actors,
    })
}

/// Canonical Top Rating order: unrounded score descending, then PrincipalId ascending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RatingKey {
    pub score: ScoreMicros,
    pub principal: PrincipalId,
}

impl Ord for RatingKey {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .score
            .cmp(&self.score)
            .then_with(|| self.principal.cmp(&other.principal))
    }
}

impl PartialOrd for RatingKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Canonical Top Growth order: growth descending, then PrincipalId ascending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrowthKey {
    pub growth: GrowthTenths,
    pub principal: PrincipalId,
}

impl Ord for GrowthKey {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .growth
            .cmp(&self.growth)
            .then_with(|| self.principal.cmp(&other.principal))
    }
}

impl PartialOrd for GrowthKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
