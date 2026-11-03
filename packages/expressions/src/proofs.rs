//! Kani 0.67.0 / bundled nightly-2025-11-21. Bounds are explicit in the harnesses.
use crate::policy::{Admission, Phase, Policy};

#[kani::proof]
#[kani::unwind(12)]
fn budget_and_terminal_dispose() {
    let capacity: u8 = kani::any();
    kani::assume(capacity <= 3);
    let mut policy = Policy::new(capacity as usize, [true; 3]);
    policy.set_environment(false, true, true);
    for i in 0..3 {
        policy.set_visible(i, true);
    }
    for _ in 0..6 {
        let index: u8 = kani::any();
        kani::assume(index < 3);
        let index = index as usize;
        let generation: u32 = kani::any();
        let was_disposed = policy.player(index).unwrap().phase() == Phase::Disposed;
        match kani::any::<u8>() % 8 {
            0 => {
                policy.activate(index);
            }
            1 => {
                policy.ready(index, generation);
            }
            2 => {
                policy.release(index, generation);
            }
            3 => {
                policy.fail(index, generation);
            }
            4 => policy.cancel(index),
            5 => policy.dispose(index),
            6 => policy.set_environment(kani::any(), kani::any(), kani::any()),
            _ => policy.set_visible(index, kani::any()),
        }
        assert!(policy.active() <= policy.capacity());
        if was_disposed {
            assert_eq!(policy.player(index).unwrap().phase(), Phase::Disposed);
        }
    }
    kani::cover!(policy.active() == 3, "full budget is reachable");
    kani::cover!(
        policy.player(0).unwrap().phase() == Phase::Disposed,
        "terminal guard is exercised"
    );
}

#[kani::proof]
#[kani::unwind(8)]
fn stale_callback_and_motion_guards() {
    let mut policy = Policy::new(1, [true]);
    policy.set_environment(false, true, true);
    policy.set_visible(0, true);
    assert_eq!(policy.activate(0), Admission::Start(1));
    policy.cancel(0);
    assert_eq!(policy.activate(0), Admission::Start(2));
    assert!(!policy.ready(0, 1));
    assert!(!policy.release(0, 1));
    assert!(!policy.fail(0, 1));
    assert_eq!(policy.active(), 1);
    let reduced: bool = kani::any();
    let foreground: bool = kani::any();
    let visible: bool = kani::any();
    policy.set_environment(reduced, foreground, true);
    policy.set_visible(0, visible);
    assert_eq!(policy.ready(0, 2), !reduced && foreground && visible);
    policy.dispose(0);
    assert!(!policy.ready(0, 2));
    assert!(!policy.release(0, 2));
    assert_eq!(policy.active(), 0);
}

#[kani::proof]
#[kani::unwind(40)]
fn bounded_exact_sequence_lookup() {
    let choice: u8 = kani::any();
    kani::assume(choice < 4);
    let (source, expected) = match choice {
        0 => ("👍🏽", Some("noto-thumbs-up-medium")),
        1 => ("❤️‍🔥", Some("noto-heart-fire")),
        2 => ("👍🏻", None),
        _ => ("🇻🇳", None),
    };
    assert_eq!(crate::catalog::lookup(source).map(|a| a.id), expected);
}
