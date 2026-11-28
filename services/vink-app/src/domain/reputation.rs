//! Điểm Vink (Reputation V1, epic #600) pure core: formula version 1 in exact integer fixed
//! point, entitlement and anchor effects, maturity and calendar periods, the Q41 community
//! credit machine per calendar and top-board admission/order, and what a source transaction owes
//! the outbox (`outbox`, #645). No I/O: persistence, worker and HTTP callers apply exactly the
//! effects returned here.
//!
//! Product rules: `VDR-REPUTATION@r1` (PRODUCT_CONFIRMED), architecture K1/K2 of
//! `VDR-REPUTATION-CONTRACT@r1` (Provisional); spec `docs/architecture/reputation.md`.
//!
//! ```compile_fail
//! use vink_app::domain::reputation::formula::ScoreMicros;
//! let score = ScoreMicros(1);
//! ```
//! ```compile_fail
//! use vink_app::domain::reputation::formula::ComponentCount;
//! let count: ComponentCount = 1_u64;
//! ```
//! Lawful twin: the same values come only through the checked constructors.
//! ```
//! use vink_app::domain::reputation::formula::{ComponentCount, ScoreMicros, score_v1};
//! let count = ComponentCount::new(1).unwrap();
//! assert_eq!(score_v1(count, ComponentCount::ZERO).display(), 1010);
//! assert_eq!(ScoreMicros::BASELINE.display(), 1000);
//! ```

pub mod board;
pub mod calendar;
pub mod credit;
pub mod entitlement;
pub mod formula;
pub mod outbox;

#[cfg(kani)]
mod proofs;
#[cfg(test)]
mod replay;
#[cfg(test)]
mod tests;
