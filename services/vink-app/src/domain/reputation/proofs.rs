//! Kani 0.67.0, production source in place. Fixed-length input sequences only (no collections):
//! symbolic inputs, symbolic period steps and symbolic facts. The fixed-point logarithm is
//! covered by golden vectors, not here.
use super::calendar::PeriodIndex;
use super::credit::{CreditInput, CreditRecord, CreditState, step};
use super::entitlement::{Cause, EntitlementFacts, EntitlementRecord, Periods, apply};
use super::formula::{COMPONENT_BOUND, ComponentCount, CurrentDelta, FormulaError, GrowthTenths};

const STEPS: usize = 4;

fn any_input() -> CreditInput {
    match kani::any::<u8>() % 6 {
        0 => CreditInput::On,
        1 => CreditInput::Off,
        2 => CreditInput::OffWhileParked,
        3 => CreditInput::Retire,
        4 => CreditInput::Burn,
        _ => CreditInput::Neutral,
    }
}

fn any_cause() -> Cause {
    match kani::any::<u8>() % 4 {
        0 => Cause::Community,
        1 => Cause::Neutral,
        2 => Cause::Terminal,
        _ => Cause::Maturity,
    }
}

fn any_facts() -> EntitlementFacts {
    let selected: u32 = kani::any();
    let valid: u32 = kani::any();
    kani::assume(selected <= 2 && valid <= selected);
    EntitlementFacts {
        selected_anchors: selected,
        valid_selected_anchors: valid,
        target_public: kani::any(),
        actor_mature: kani::any(),
    }
}

/// Q41: per-period net in {−1, 0, +1}, lifetime net in {0, 1}, `Burned` terminal, no credit
/// before the first full period — for every input sequence and period walk.
#[kani::proof]
fn credit_machine_bounds_hold_for_every_sequence() {
    let start = PeriodIndex::new(1);
    let mut record = CreditRecord::NEW;
    let mut period: i64 = 0;
    let mut nets = [0i64; STEPS + 1];
    let mut lifetime = 0i64;
    let mut burned = false;
    for _ in 0..STEPS {
        let advance: bool = kani::any();
        if advance {
            period += 1;
        }
        let result = step(
            record,
            any_input(),
            PeriodIndex::new(period),
            start,
            kani::any(),
        );
        assert!(result.delta >= -1 && result.delta <= 1);
        match result.period {
            None => assert_eq!(result.delta, 0),
            Some(p) => {
                assert!(p.get() >= start.get());
                nets[p.get() as usize] += result.delta;
            }
        }
        lifetime += result.delta;
        assert!(lifetime == 0 || lifetime == 1);
        if burned {
            assert_eq!(result.record.state(), CreditState::Burned);
        }
        burned = result.record.state() == CreditState::Burned;
        assert_eq!(
            result.record.state().credit(),
            i64::from(result.record.state() == CreditState::Active)
        );
        record = result.record;
    }
    assert_eq!(
        nets[0], 0,
        "nothing is recorded before the first full period"
    );
    for net in nets {
        assert!(net >= -1 && net <= 1);
    }
    kani::cover!(lifetime == 1);
    kani::cover!(nets.iter().any(|net| *net == -1));
}

/// The recipient's count from one entitlement equals its stored effective state; a terminal
/// record stays terminal and ineffective; current deltas are exact transitions.
#[kani::proof]
fn entitlement_current_tracks_effective_and_terminal_is_final() {
    let mut record = EntitlementRecord::NEW;
    let mut current = 0i64;
    let mut period: i64 = 1;
    for _ in 0..3 {
        if kani::any::<bool>() {
            period += 1;
        }
        let at = Periods {
            week: PeriodIndex::new(period),
            month: PeriodIndex::new(period),
            week_start: PeriodIndex::new(1),
            month_start: PeriodIndex::new(1),
        };
        let was_terminal = record.terminal();
        let applied = apply(record, any_cause(), any_facts(), at);
        current += applied.current.get();
        assert_eq!(current, i64::from(applied.record.effective()));
        assert!(!(applied.record.terminal() && applied.record.effective()));
        if was_terminal {
            assert_eq!(applied.record, record);
            assert_eq!(applied.current, CurrentDelta::None);
        }
        record = applied.record;
    }
    kani::cover!(record.terminal());
}

#[kani::proof]
fn component_counts_and_growth_are_checked() {
    let raw: u64 = kani::any();
    let count = ComponentCount::new(raw);
    assert_eq!(count.is_ok(), raw < COMPONENT_BOUND);
    if let Ok(count) = count {
        let add = count.apply(CurrentDelta::Add);
        let remove = count.apply(CurrentDelta::Remove);
        assert_eq!(add.is_err(), raw == COMPONENT_BOUND - 1);
        assert_eq!(remove == Err(FormulaError::Underflow), raw == 0);
        if let Ok(next) = add {
            assert_eq!(next.get(), raw + 1);
        }
    }
    let impact: i64 = kani::any();
    let contribution: i64 = kani::any();
    kani::assume(impact.unsigned_abs() < 1 << 50 && contribution.unsigned_abs() < 1 << 50);
    let growth = GrowthTenths::of(impact, contribution).unwrap();
    assert_eq!(growth.get(), 24 * impact + 16 * contribution);
}
