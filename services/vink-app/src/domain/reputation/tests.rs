use serde::Deserialize;
use time::macros::datetime;
use uuid::Uuid;

use super::board::{GrowthKey, MIN_DISTINCT_ACTORS, RatingKey, qualify_growth, rating_admits};
use super::calendar::{
    Calendar, PeriodIndex, first_full_period, is_mature, maturity_threshold, period_end, period_of,
    period_start,
};
use super::credit::{CreditInput, CreditRecord, CreditState, step};
use super::entitlement::{Cause, EntitlementFacts, EntitlementRecord, Periods, apply};
use super::formula::{
    COMPONENT_BOUND, ComponentCount, CurrentDelta, FIXED_ONE_FOR_TESTS, FormulaError,
    FormulaVersion, GrowthTenths, ScoreMicros, ln_fixed_for_tests, score_v1,
};
use crate::domain::ids::PrincipalId;

#[derive(Deserialize)]
struct FormulaVectors {
    format: String,
    formula_version: i32,
    columns: Vec<String>,
    vectors: Vec<[u64; 4]>,
}

fn count(raw: u64) -> ComponentCount {
    ComponentCount::new(raw).unwrap()
}

#[test]
fn formula_reproduces_every_sr01_golden_vector_exactly() {
    let data: FormulaVectors = serde_json::from_str(include_str!(
        "../../../behavior/reputation/formula-vectors.json"
    ))
    .unwrap();
    assert_eq!(data.format, "vink-reputation-formula-vectors-v1");
    assert_eq!(
        FormulaVersion::from_stored(data.formula_version),
        Some(FormulaVersion::V1)
    );
    assert_eq!(
        data.columns,
        ["impact", "contribution", "score_micros", "display"]
    );
    assert!(data.vectors.len() > 2_000, "the extended corpus is present");
    for [impact, contribution, micros, display] in data.vectors {
        let score = score_v1(count(impact), count(contribution));
        assert_eq!(
            score.get(),
            i64::try_from(micros).unwrap(),
            "({impact}, {contribution})"
        );
        assert_eq!(
            score.display(),
            i64::try_from(display).unwrap(),
            "({impact}, {contribution})"
        );
    }
}

#[test]
fn integer_logarithm_matches_known_values() {
    let one = FIXED_ONE_FOR_TESTS as f64;
    for n in [1_u64, 2, 10, 11, 1_000, 1 << 39, COMPONENT_BOUND + 9] {
        let fixed = ln_fixed_for_tests(n) as f64 / one;
        assert!((fixed - (n as f64).ln()).abs() < 1e-12, "ln {n}: {fixed}");
    }
    assert_eq!(ln_fixed_for_tests(1), 0);
    // 2^k is exact up to the error of ln 2 times k: monotone in n.
    let mut previous = 0;
    for n in 1..=5_000 {
        let value = ln_fixed_for_tests(n);
        assert!(n == 1 || value > previous, "ln is increasing at {n}");
        previous = value;
    }
}

#[test]
fn score_is_monotone_and_baseline_is_exact() {
    assert_eq!(
        score_v1(ComponentCount::ZERO, ComponentCount::ZERO),
        ScoreMicros::BASELINE
    );
    assert_eq!(ScoreMicros::BASELINE.display(), 1000);
    let mut previous = ScoreMicros::BASELINE;
    for i in 1..=3_000 {
        let score = score_v1(count(i), ComponentCount::ZERO);
        assert!(score > previous, "impact {i}");
        previous = score;
    }
    let mut previous = ScoreMicros::BASELINE;
    for c in 1..=3_000 {
        let score = score_v1(ComponentCount::ZERO, count(c));
        assert!(score > previous, "contribution {c}");
        previous = score;
    }
    // Exact integer ratings never come out one micro low.
    assert_eq!(score_v1(count(100), count(100)).get(), 1_400_000_000);
    assert_eq!(
        score_v1(count(1_200), count(13_300)).get(),
        1_000_000_000 + 480_000_000 + 480_000_000
    );
}

#[test]
fn display_rounds_half_up_once_from_the_canonical_score() {
    for (micros, display) in [
        (1_400_499_999, 1400),
        (1_400_500_000, 1401),
        (1_472_499_987, 1472),
        (1_460_500_004, 1461),
    ] {
        assert_eq!(ScoreMicros::from_stored(micros).unwrap().display(), display);
    }
    assert_eq!(
        ScoreMicros::from_stored(999_999_999),
        None,
        "below baseline is corrupt"
    );
}

#[test]
fn component_counts_are_checked() {
    assert_eq!(
        ComponentCount::new(COMPONENT_BOUND),
        Err(FormulaError::OutOfBounds)
    );
    let top = count(COMPONENT_BOUND - 1);
    assert_eq!(top.apply(CurrentDelta::Add), Err(FormulaError::OutOfBounds));
    assert_eq!(
        ComponentCount::ZERO.apply(CurrentDelta::Remove),
        Err(FormulaError::Underflow)
    );
    assert_eq!(count(5).apply(CurrentDelta::Remove), Ok(count(4)));
    assert_eq!(count(5).apply(CurrentDelta::None), Ok(count(5)));
    assert_eq!(
        ComponentCount::from_stored(-1),
        Err(FormulaError::Underflow)
    );
}

#[test]
fn growth_is_fixed_units_in_tenths() {
    assert_eq!(GrowthTenths::of(1, 0).unwrap().get(), 24);
    assert_eq!(GrowthTenths::of(0, 1).unwrap().get(), 16);
    assert_eq!(GrowthTenths::of(-1, 0).unwrap().get(), -24);
    assert_eq!(GrowthTenths::of(10, 10).unwrap().get(), 400);
    assert_eq!(GrowthTenths::of(3, -2).unwrap().get(), 40);
    assert_eq!(GrowthTenths::of(i64::MAX, 0), Err(FormulaError::Overflow));
    assert_eq!(
        GrowthTenths::from_stored(i64::MAX).checked_add(GrowthTenths::of(0, 1).unwrap()),
        Err(FormulaError::Overflow)
    );
}

#[test]
fn weeks_start_monday_and_months_on_the_first_in_vietnam() {
    // Monday 2026-11-30 00:00 +07 = Sunday 17:00 UTC.
    let before = datetime!(2026-11-29 16:59:59.999999999 UTC);
    let boundary = datetime!(2026-11-29 17:00 UTC);
    let week = period_of(Calendar::Week, before);
    assert_eq!(
        period_of(Calendar::Week, boundary),
        week.next(),
        "end belongs to the next week"
    );
    assert_eq!(period_start(Calendar::Week, week.next()), Some(boundary));
    assert_eq!(period_end(Calendar::Week, week), Some(boundary));
    // December 2026 starts at 2026-11-30 17:00 UTC.
    let month = period_of(Calendar::Month, datetime!(2026-11-30 16:59 UTC));
    assert_eq!(
        period_of(Calendar::Month, datetime!(2026-11-30 17:00 UTC)),
        month.next()
    );
    assert_eq!(
        period_start(Calendar::Month, month.next()),
        Some(datetime!(2026-11-30 17:00 UTC))
    );
    // Leap February 2028 ends on the 29th; the year turns inside one week.
    let march = period_of(Calendar::Month, datetime!(2028-03-15 00:00 UTC));
    assert_eq!(
        period_start(Calendar::Month, march),
        Some(datetime!(2028-02-29 17:00 UTC))
    );
    assert_eq!(
        period_of(Calendar::Week, datetime!(2026-12-31 12:00 UTC)),
        period_of(Calendar::Week, datetime!(2027-01-02 12:00 UTC))
    );
    assert_eq!(
        period_of(Calendar::Month, datetime!(2026-12-31 18:00 UTC)).get(),
        2027 * 12
    );
    for at in [before, boundary, datetime!(2030-06-15 05:06 UTC)] {
        for calendar in Calendar::ALL {
            let p = period_of(calendar, at);
            let start = period_start(calendar, p).unwrap();
            let end = period_end(calendar, p).unwrap();
            assert!(start <= at && at < end, "{calendar:?} {at}");
        }
    }
}

#[test]
fn first_full_period_starts_at_or_after_activation() {
    let boundary = datetime!(2026-11-29 17:00 UTC);
    let week = period_of(Calendar::Week, boundary);
    assert_eq!(first_full_period(Calendar::Week, boundary), week);
    assert_eq!(
        first_full_period(Calendar::Week, datetime!(2026-11-29 17:00:01 UTC)),
        week.next()
    );
    assert_eq!(
        first_full_period(Calendar::Month, datetime!(2026-11-28 08:00 UTC)),
        period_of(Calendar::Month, datetime!(2026-12-15 00:00 UTC))
    );
}

#[test]
fn maturity_is_seven_days_of_elapsed_time() {
    let created = datetime!(2026-11-20 10:00 UTC);
    assert_eq!(maturity_threshold(created), datetime!(2026-11-27 10:00 UTC));
    assert!(!is_mature(
        created,
        datetime!(2026-11-27 09:59:59.999999999 UTC)
    ));
    assert!(is_mature(created, datetime!(2026-11-27 10:00 UTC)));
}

const P1: PeriodIndex = PeriodIndex::new(1);

fn periods(week: i64, month: i64) -> Periods {
    Periods {
        week: PeriodIndex::new(week),
        month: PeriodIndex::new(month),
        week_start: P1,
        month_start: P1,
    }
}

fn facts(selected: u32, valid: u32, public: bool, mature: bool) -> EntitlementFacts {
    EntitlementFacts {
        selected_anchors: selected,
        valid_selected_anchors: valid,
        target_public: public,
        actor_mature: mature,
    }
}

struct Run {
    record: EntitlementRecord,
    current: i64,
    week: std::collections::BTreeMap<i64, i64>,
    month: std::collections::BTreeMap<i64, i64>,
}

impl Run {
    fn new() -> Self {
        Self {
            record: EntitlementRecord::NEW,
            current: 0,
            week: Default::default(),
            month: Default::default(),
        }
    }
    fn go(&mut self, cause: Cause, facts: EntitlementFacts, at: Periods) -> (i64, i64, i64) {
        let applied = apply(self.record, cause, facts, at);
        self.record = applied.record;
        self.current += applied.current.get();
        for (net, step) in [
            (&mut self.week, applied.week),
            (&mut self.month, applied.month),
        ] {
            if let Some(p) = step.period {
                *net.entry(p.get()).or_insert(0) += step.delta;
            }
        }
        (
            applied.current.get(),
            applied.week.delta,
            applied.month.delta,
        )
    }
}

#[test]
fn q41_oracle_weeks_and_month_compensation() {
    let mut run = Run::new();
    let on = facts(1, 1, true, true);
    let off = facts(0, 0, true, true);
    assert_eq!(run.go(Cause::Community, on, periods(1, 1)), (1, 1, 1));
    assert_eq!(run.go(Cause::Community, off, periods(2, 1)), (-1, -1, -1));
    assert_eq!(
        run.go(Cause::Community, on, periods(3, 1)),
        (1, 0, 1),
        "relike week 3: 0"
    );
    assert_eq!(
        run.go(Cause::Community, off, periods(4, 1)),
        (-1, 0, -1),
        "unlike week 4: 0"
    );
    assert_eq!(run.week.values().sum::<i64>(), 0);
    assert_eq!(run.week.get(&1), Some(&1));
    assert_eq!(run.week.get(&2), Some(&-1));
    assert_eq!(
        run.month.get(&1),
        Some(&0),
        "OFF/ON in different weeks of one month: 0"
    );
    // Next month the pending withdrawal is burned: a relike earns nothing.
    assert_eq!(run.go(Cause::Community, on, periods(5, 2)), (1, 0, 0));
    assert_eq!(run.record.month().state(), CreditState::Burned);
}

#[test]
fn same_period_loops_never_exceed_one_credit() {
    let mut run = Run::new();
    let on = facts(1, 1, true, true);
    let off = facts(0, 0, true, true);
    for _ in 0..5 {
        run.go(Cause::Community, on, periods(1, 1));
        run.go(Cause::Community, off, periods(1, 1));
    }
    assert_eq!(run.week.get(&1), Some(&0));
    run.go(Cause::Community, on, periods(1, 1));
    assert_eq!(run.week.get(&1), Some(&1));
    assert_eq!(run.current, 1);
}

#[test]
fn several_anchors_are_one_entitlement() {
    let mut run = Run::new();
    assert_eq!(
        run.go(Cause::Community, facts(1, 1, true, true), periods(1, 1)),
        (1, 1, 1)
    );
    assert_eq!(
        run.go(Cause::Community, facts(2, 2, true, true), periods(1, 1)),
        (0, 0, 0)
    );
    assert_eq!(
        run.go(Cause::Community, facts(1, 1, true, true), periods(1, 1)),
        (0, 0, 0)
    );
    // The remaining anchor is hidden: neutral, current only.
    assert_eq!(
        run.go(Cause::Neutral, facts(1, 0, true, true), periods(1, 1)),
        (-1, 0, 0)
    );
    assert_eq!(
        run.go(Cause::Neutral, facts(1, 1, true, true), periods(1, 1)),
        (1, 0, 0)
    );
    assert_eq!(
        run.go(Cause::Community, facts(0, 0, true, true), periods(2, 1)),
        (-1, -1, -1)
    );
}

#[test]
fn parked_credit_retires_without_debit_and_never_returns() {
    let mut run = Run::new();
    run.go(Cause::Community, facts(1, 1, true, true), periods(1, 1));
    // Private: parked.
    assert_eq!(
        run.go(Cause::Neutral, facts(1, 1, false, true), periods(1, 1)),
        (-1, 0, 0)
    );
    assert_eq!(run.record.week().state(), CreditState::Active);
    // OFF while parked: retire, no debit.
    assert_eq!(
        run.go(Cause::Community, facts(0, 0, false, true), periods(2, 1)),
        (0, 0, 0)
    );
    assert_eq!(run.record.week().state(), CreditState::Burned);
    run.go(Cause::Neutral, facts(0, 0, true, true), periods(2, 1));
    assert_eq!(
        run.go(Cause::Community, facts(1, 1, true, true), periods(2, 1)),
        (1, 0, 0)
    );
    assert_eq!(run.week.values().sum::<i64>(), 1, "lifetime net stays 1");
}

#[test]
fn terminal_events_retire_for_good() {
    let mut run = Run::new();
    run.go(Cause::Community, facts(1, 1, true, true), periods(1, 1));
    assert_eq!(
        run.go(Cause::Terminal, facts(0, 0, true, true), periods(1, 1)),
        (-1, 0, 0)
    );
    assert!(run.record.terminal());
    let frozen = run.record;
    for cause in [
        Cause::Community,
        Cause::Neutral,
        Cause::Maturity,
        Cause::Terminal,
    ] {
        assert_eq!(
            run.go(cause, facts(1, 1, true, true), periods(3, 2)),
            (0, 0, 0)
        );
        assert_eq!(
            run.record, frozen,
            "{cause:?} resurrected a retired entitlement"
        );
    }
}

#[test]
fn held_maturity_is_current_only_and_off_before_threshold_allows_a_later_credit() {
    let mut held = Run::new();
    assert_eq!(
        held.go(Cause::Community, facts(1, 1, true, false), periods(1, 1)),
        (0, 0, 0)
    );
    assert_eq!(
        held.go(Cause::Maturity, facts(1, 1, true, true), periods(1, 1)),
        (1, 0, 0)
    );
    assert_eq!(held.record.week().state(), CreditState::Burned);
    assert_eq!(held.record.month().state(), CreditState::Burned);
    assert_eq!(
        held.go(Cause::Community, facts(0, 0, true, true), periods(2, 1)),
        (-1, 0, 0)
    );

    let mut later = Run::new();
    later.go(Cause::Community, facts(1, 1, true, false), periods(1, 1));
    later.go(Cause::Community, facts(0, 0, true, false), periods(1, 1));
    assert_eq!(
        later.go(Cause::Maturity, facts(0, 0, true, true), periods(1, 1)),
        (0, 0, 0)
    );
    assert_eq!(
        later.go(Cause::Community, facts(1, 1, true, true), periods(1, 1)),
        (1, 1, 1)
    );
}

#[test]
fn activation_burns_what_is_effective_at_each_first_boundary() {
    let mut run = Run::new();
    let at = Periods {
        week: PeriodIndex::new(0),
        month: PeriodIndex::new(0),
        week_start: P1,
        month_start: PeriodIndex::new(2),
    };
    // Partial period: current only, nothing recorded as growth.
    assert_eq!(
        run.go(Cause::Community, facts(1, 1, true, true), at),
        (1, 0, 0)
    );
    assert_eq!(run.record.week().state(), CreditState::Unstarted);
    // Week starts at 1 with the entitlement effective: baseline burned. Month still partial.
    let week1 = Periods { week: P1, ..at };
    assert_eq!(
        run.go(Cause::Community, facts(0, 0, true, true), week1),
        (-1, 0, 0)
    );
    assert_eq!(run.record.week().state(), CreditState::Burned);
    assert_eq!(run.record.month().state(), CreditState::Unstarted);
    // Month 2 starts with the entitlement not effective: unseen, then a real ON credits.
    let month2 = Periods {
        week: PeriodIndex::new(5),
        month: PeriodIndex::new(2),
        ..at
    };
    assert_eq!(
        run.go(Cause::Community, facts(1, 1, true, true), month2),
        (1, 0, 1)
    );
}

#[test]
fn late_accepted_times_never_reopen_an_earlier_period() {
    let start = P1;
    let on = step(
        CreditRecord::NEW,
        CreditInput::On,
        PeriodIndex::new(3),
        start,
        false,
    );
    let off = step(
        on.record,
        CreditInput::Off,
        PeriodIndex::new(4),
        start,
        true,
    );
    // An out-of-order accepted time in week 3 is attributed to week 4 (monotone clamp).
    let late = step(
        off.record,
        CreditInput::On,
        PeriodIndex::new(3),
        start,
        false,
    );
    assert_eq!(late.period, Some(PeriodIndex::new(4)));
    assert_eq!(
        late.delta, 1,
        "same-period compensation in the clamped period"
    );
    assert_eq!(late.record.last(), Some(PeriodIndex::new(4)));
}

#[test]
fn no_op_and_stale_inputs_change_nothing() {
    let mut run = Run::new();
    run.go(Cause::Community, facts(1, 1, true, true), periods(1, 1));
    let before = run.record;
    let applied = apply(
        before,
        Cause::Community,
        facts(1, 1, true, true),
        periods(1, 1),
    );
    assert_eq!(applied.current, CurrentDelta::None);
    assert!(!applied.changed(before));
    assert_eq!(
        EntitlementRecord::restore(true, true, CreditRecord::NEW, CreditRecord::NEW),
        None
    );
}

fn principal(n: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(n))
}

#[test]
fn top_growth_needs_three_distinct_positive_actors() {
    let unit = GrowthTenths::of(1, 0).unwrap();
    let big = GrowthTenths::of(50, 50).unwrap();
    let two = qualify_growth([(principal(1), big), (principal(2), big)]).unwrap();
    assert!(!two.admits(), "two actors, however active, stay out");
    let same_actor = qualify_growth([
        (principal(1), unit),
        (principal(1), unit),
        (principal(1), unit),
        (principal(2), unit),
    ])
    .unwrap();
    assert_eq!(same_actor.positive_actors, 2, "one actor counts once");
    let unlike = GrowthTenths::of(-1, 0).unwrap();
    let mixed = qualify_growth([
        (principal(1), unit),
        (principal(2), unit),
        (principal(3), unlike),
    ])
    .unwrap();
    assert_eq!(mixed.positive_actors, 2, "unlike-only actors never count");
    let three = qualify_growth([
        (principal(1), unit),
        (principal(2), unit),
        (principal(3), unit),
    ])
    .unwrap();
    assert_eq!(three.positive_actors, MIN_DISTINCT_ACTORS);
    assert!(three.admits());
    let negative_total = qualify_growth([
        (principal(1), unit),
        (principal(2), unit),
        (principal(3), unit),
        (principal(4), GrowthTenths::of(-5, 0).unwrap()),
    ])
    .unwrap();
    assert!(!negative_total.admits(), "G must be positive");
}

#[test]
fn boards_order_by_score_then_principal() {
    assert!(!rating_admits(ComponentCount::ZERO, ComponentCount::ZERO));
    assert!(
        rating_admits(ComponentCount::ZERO, count(1)),
        "comment-only members are admitted"
    );
    let low = score_v1(count(1), ComponentCount::ZERO);
    let high = score_v1(count(2), ComponentCount::ZERO);
    let mut keys = [
        RatingKey {
            score: low,
            principal: principal(1),
        },
        RatingKey {
            score: high,
            principal: principal(9),
        },
        RatingKey {
            score: low,
            principal: principal(0),
        },
    ];
    keys.sort();
    assert_eq!(
        keys.iter().map(|k| k.principal).collect::<Vec<_>>(),
        [principal(9), principal(0), principal(1)]
    );
    let mut growth = [
        GrowthKey {
            growth: GrowthTenths::of(1, 0).unwrap(),
            principal: principal(2),
        },
        GrowthKey {
            growth: GrowthTenths::of(1, 0).unwrap(),
            principal: principal(1),
        },
        GrowthKey {
            growth: GrowthTenths::of(2, 0).unwrap(),
            principal: principal(3),
        },
    ];
    growth.sort();
    assert_eq!(
        growth.iter().map(|k| k.principal).collect::<Vec<_>>(),
        [principal(3), principal(1), principal(2)]
    );
}

#[test]
fn calendar_names_round_trip() {
    for calendar in Calendar::ALL {
        assert_eq!(Calendar::parse(calendar.as_str()), Some(calendar));
    }
    assert_eq!(Calendar::parse("day"), None);
}

#[test]
fn activation_epochs_advance_only_on_activation() {
    use super::outbox::{Activation, ActivationChange};
    let off = Activation::restore(false, 0).unwrap();
    assert_eq!(off.producing(), None);
    let ActivationChange::Changed(on) = off.change(true) else {
        panic!("activation changes");
    };
    assert_eq!(
        (on.active(), on.epoch(), on.producing()),
        (true, 1, Some(1))
    );
    assert_eq!(on.change(true), ActivationChange::Unchanged(on));
    let ActivationChange::Changed(paused) = on.change(false) else {
        panic!("deactivation changes");
    };
    assert_eq!((paused.active(), paused.epoch()), (false, 1));
    assert_eq!(Activation::restore(true, 0), None, "active needs an epoch");
    assert_eq!(
        Activation::restore(false, i64::MAX).unwrap().change(true),
        ActivationChange::Exhausted
    );
}

#[test]
fn only_a_changed_endorsement_owes_a_community_event() {
    use super::outbox::{CommunityContext, CommunityEvent};
    use crate::domain::endorsement::{EndorsementDelta, EndorsementState, EndorsementTarget};
    use crate::domain::ids::{CommentId, RevisionId, WorkId};
    let work = WorkId::from_uuid(Uuid::from_u128(10));
    let revision = RevisionId::from_uuid(Uuid::from_u128(11));
    let target = EndorsementTarget::Work { work, revision };
    let on = EndorsementState::restore(true, 2, 2).unwrap();
    let context = CommunityContext::Work {
        work_public: true,
        hidden_revision: None,
    };
    let event = CommunityEvent::of(
        principal(1),
        principal(2),
        target,
        EndorsementDelta::Add,
        on,
        context,
    );
    assert!(matches!(
        event,
        Some(CommunityEvent::Tham {
            work_public: true,
            ..
        })
    ));
    assert_eq!(event.unwrap().kind(), "tham");
    assert_eq!(
        CommunityEvent::of(
            principal(1),
            principal(2),
            target,
            EndorsementDelta::None,
            on,
            context
        ),
        None,
        "same-state intent: only the order advanced"
    );
    let comment = EndorsementTarget::Comment {
        comment: CommentId::from_uuid(Uuid::from_u128(12)),
        work,
    };
    assert_eq!(
        CommunityEvent::of(
            principal(1),
            principal(2),
            comment,
            EndorsementDelta::Add,
            on,
            context
        ),
        None,
        "a comment target needs a comment context"
    );
    let ung = CommunityEvent::of(
        principal(1),
        principal(2),
        comment,
        EndorsementDelta::Remove,
        EndorsementState::restore(false, 3, 4).unwrap(),
        CommunityContext::Comment {
            work_public: false,
            comment_public: true,
        },
    );
    assert!(matches!(
        ung,
        Some(CommunityEvent::Ung {
            work_public: false,
            comment_public: true,
            ..
        })
    ));
}

#[test]
fn credit_states_round_trip_through_storage() {
    for state in [
        CreditState::Unstarted,
        CreditState::Unseen,
        CreditState::Active,
        CreditState::WithdrawnPending(PeriodIndex::new(7)),
        CreditState::Burned,
    ] {
        assert_eq!(
            CreditState::restore(state.code(), state.pending()),
            Some(state)
        );
    }
    assert_eq!(CreditState::restore("withdrawn", None), None);
    assert_eq!(
        CreditState::restore("active", Some(PeriodIndex::new(1))),
        None
    );
    assert_eq!(CreditState::restore("gone", None), None);
}
