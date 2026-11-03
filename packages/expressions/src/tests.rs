use crate::{
    catalog,
    policy::{Admission, Phase, Policy, Reason},
};

fn eligible<const N: usize>(capacity: usize) -> Policy<N> {
    let mut policy = Policy::new(capacity, [true; N]);
    policy.set_environment(false, true, true);
    for index in 0..N {
        policy.set_visible(index, true);
    }
    policy
}

#[test]
fn exact_unicode_sequences_share_the_canonical_contract() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../shared/unicode-fixtures.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let source = case["source"].as_str().unwrap();
        assert_eq!(
            catalog::lookup(source).map(|a| a.id),
            case["asset_id"].as_str(),
            "{source:?}"
        );
    }
    assert_ne!(
        catalog::lookup("👍").unwrap().id,
        catalog::lookup("👍🏽").unwrap().id
    );
    assert!(catalog::lookup("👍🏻").is_none());
    assert_eq!(catalog::fallback(None, false), catalog::Fallback::Unicode);
    let asset = catalog::lookup("❤️‍🔥").unwrap();
    assert_eq!(
        catalog::fallback(Some(asset), true),
        catalog::Fallback::Unicode
    );
    assert_eq!(
        catalog::fallback(Some(asset), false),
        catalog::Fallback::Poster(asset.poster)
    );
    assert!(catalog::by_id("https://untrusted.invalid/asset.json").is_none());
}

#[test]
fn reservations_and_releases_are_idempotent_and_shared() {
    let mut policy = eligible::<3>(2);
    assert_eq!(policy.activate(0), Admission::Start(1));
    assert_eq!(policy.activate(0), Admission::AlreadyActive(1));
    assert_eq!(policy.activate(1), Admission::Start(1));
    assert_eq!(policy.activate(2), Admission::Static(Reason::Budget));
    assert_eq!(policy.active(), 2);
    policy.set_environment(false, true, true);
    policy.set_visible(2, true);
    assert_eq!(policy.activate(0), Admission::AlreadyActive(1));
    assert_eq!(
        policy.player(2).unwrap().reason(),
        Reason::Budget,
        "unchanged host facts must retain the denied activation explanation"
    );
    assert!(policy.ready(0, 1));
    assert!(!policy.ready(0, 1));
    assert!(policy.release(0, 1));
    assert!(!policy.release(0, 1));
    assert_eq!(policy.activate(2), Admission::Start(1));
    assert_eq!(policy.active(), 2);
    assert!(policy.fail(2, 1));
    assert_eq!(policy.player(2).unwrap().phase(), Phase::Error);
    assert_eq!(policy.active(), 1);
    assert_eq!(policy.activate(2), Admission::Start(2));
    assert!(!policy.fail(2, 1));
    assert!(!policy.release(2, 1));
    assert_eq!(policy.active(), 2);
}

#[test]
fn late_callbacks_cannot_resurrect_cancelled_or_disposed_players() {
    let mut policy = eligible::<1>(2);
    assert_eq!(policy.activate(0), Admission::Start(1));
    policy.cancel(0);
    assert!(!policy.ready(0, 1));
    assert_eq!(policy.activate(0), Admission::Start(2));
    assert!(!policy.ready(0, 1));
    policy.dispose(0);
    policy.dispose(0);
    assert!(!policy.ready(0, 2));
    assert!(!policy.release(0, 2));
    assert!(!policy.fail(0, 2));
    assert_eq!(policy.activate(0), Admission::Static(Reason::Disposed));
    policy.set_visible(0, true);
    policy.set_environment(false, true, true);
    assert_eq!(policy.player(0).unwrap().phase(), Phase::Disposed);
    assert_eq!(policy.active(), 0);
}

#[test]
fn host_facts_cancel_loading_and_playing_without_automatic_resume() {
    for playing in [false, true] {
        for guard in 0..4 {
            let mut policy = eligible::<1>(2);
            policy.activate(0);
            if playing {
                assert!(policy.ready(0, 1));
            }
            match guard {
                0 => policy.set_environment(true, true, true),
                1 => policy.set_environment(false, false, true),
                2 => policy.set_environment(false, true, false),
                _ => policy.set_visible(0, false),
            }
            assert_eq!(policy.active(), 0);
            assert!(!policy.ready(0, 1));
            assert!(matches!(policy.activate(0), Admission::Static(_)));
            policy.set_environment(false, true, true);
            policy.set_visible(0, true);
            assert_eq!(
                policy.active(),
                0,
                "facts recovering cannot consume a fresh activation"
            );
            assert_eq!(policy.activate(0), Admission::Start(2));
        }
    }
    let mut unsupported = Policy::new(2, [false]);
    unsupported.set_environment(false, true, true);
    unsupported.set_visible(0, true);
    assert_eq!(
        unsupported.activate(0),
        Admission::Static(Reason::Unsupported)
    );
    assert_eq!(
        eligible::<1>(0).activate(0),
        Admission::Static(Reason::Budget)
    );
    assert_eq!(eligible::<1>(usize::MAX).capacity(), 4);
}

#[test]
fn production_budget_is_bounded_for_adversarial_event_sequences() {
    // Enumerate all 9^6 interleavings; this is code enumeration, not Kani proof.
    for mut commands in 0..9_usize.pow(6) {
        let mut policy = eligible::<2>(1);
        for _ in 0..6 {
            let event = commands % 9;
            commands /= 9;
            match event {
                0 => {
                    policy.activate(0);
                }
                1 => {
                    policy.activate(1);
                }
                2 => {
                    policy.ready(0, 1);
                }
                3 => {
                    policy.ready(1, 1);
                }
                4 => {
                    policy.release(0, 1);
                }
                5 => {
                    policy.dispose(0);
                }
                6 => policy.set_environment(true, true, true),
                7 => policy.set_environment(false, true, true),
                _ => policy.cancel(1),
            }
            assert!(policy.active() <= policy.capacity());
        }
    }
}

fn apply_trace(
    policy: &mut Policy<3>,
    command: &serde_json::Value,
    facts: &mut (bool, bool, bool),
) -> &'static str {
    let index = command["id"].as_u64().unwrap() as usize;
    let generation = command["expected"].as_u64().unwrap() as u32;
    let flag = command["flag"].as_bool().unwrap();
    match command["op"].as_str().unwrap() {
        "activate" => match policy.activate(index) {
            Admission::Start(_) => "load",
            Admission::AlreadyActive(_) => "already_active",
            Admission::Static(Reason::Budget) => "budget",
            Admission::Static(_) => "static",
        },
        "ready" => {
            if policy.ready(index, generation) {
                "play"
            } else {
                "ignored"
            }
        }
        "complete" => {
            if policy.release(index, generation) {
                "complete"
            } else {
                "ignored"
            }
        }
        "error" => {
            if policy.fail(index, generation) {
                "error"
            } else {
                "ignored"
            }
        }
        "dispose" => {
            policy.dispose(index);
            "disposed"
        }
        "cancel" => {
            policy.cancel(index);
            "cancelled"
        }
        "visible" => {
            policy.set_visible(index, flag);
            "facts"
        }
        op @ ("reduced" | "foreground" | "runtime") => {
            match op {
                "reduced" => facts.0 = flag,
                "foreground" => facts.1 = flag,
                _ => facts.2 = flag,
            }
            policy.set_environment(facts.0, facts.1, facts.2);
            "facts"
        }
        _ => panic!("unknown model command"),
    }
}

fn assert_state(policy: &Policy<3>, step: &serde_json::Value) {
    assert_eq!(policy.active(), step["active"].as_u64().unwrap() as usize);
    for expected in step["players"].as_array().unwrap() {
        let player = policy
            .player(expected["id"].as_u64().unwrap() as usize)
            .unwrap();
        assert_eq!(player.phase().as_str(), expected["phase"].as_str().unwrap());
        assert_eq!(
            u64::from(player.generation()),
            expected["generation"].as_u64().unwrap()
        );
    }
}

#[test]
fn quint_inputs_replay_through_the_real_policy() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../behavior/fixtures.json")).unwrap();
    for scenario in fixture["scenarios"].as_array().unwrap() {
        let mut policy = eligible::<3>(2);
        let mut facts = (false, true, true);
        for step in scenario["steps"].as_array().unwrap() {
            assert_eq!(
                apply_trace(&mut policy, &step["command"], &mut facts),
                step["outcome"].as_str().unwrap()
            );
            assert_state(&policy, step);
        }
    }
}

#[test]
fn production_rejects_the_quint_missing_guard_counterexample() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../behavior/late-callback-counterexample.json"
    ))
    .unwrap();
    let steps = fixture["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    let mut policy = eligible::<3>(2);
    let mut facts = (false, true, true);
    for step in &steps[..2] {
        apply_trace(&mut policy, &step["command"], &mut facts);
        assert_state(&policy, step);
    }
    assert_eq!(
        apply_trace(&mut policy, &steps[2]["command"], &mut facts),
        "ignored"
    );
    assert_eq!(steps[2]["players"][0]["phase"], "playing");
    assert_eq!(policy.player(0).unwrap().phase(), Phase::Disposed);
    assert_eq!(policy.active(), 0);
}
