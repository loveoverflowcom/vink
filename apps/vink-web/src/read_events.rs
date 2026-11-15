//! One lifecycle listener set per browser tab; only mounted owners subscribe.
//! This dispatches read hints, never commands or cache scans.
#[cfg(feature = "hydrate")]
mod browser {
    use leptos::prelude::*;
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
    use wasm_bindgen::{JsCast, closure::Closure};
    type Hook = Rc<dyn Fn()>;
    struct Events {
        next: u64,
        hooks: BTreeMap<u64, Hook>,
        listener: Option<Closure<dyn FnMut()>>,
    }
    thread_local! {
        static EVENTS: RefCell<Events> = RefCell::new(Events { next: 0, hooks: BTreeMap::new(), listener: None });
    }
    pub fn now() -> f64 {
        window().performance().map(|p| p.now()).unwrap_or(0.0)
    }
    pub fn observe(hook: impl Fn() + 'static) {
        let owner = Owner::current().map(|o| o.downgrade());
        let id = register(move || {
            if let Some(owner) = owner.as_ref().and_then(|o| o.upgrade()) {
                owner.with(&hook);
            }
        });
        on_cleanup(move || {
            EVENTS.with_borrow_mut(|e| {
                e.hooks.remove(&id);
            })
        });
    }
    pub fn register(hook: impl Fn() + 'static) -> u64 {
        EVENTS.with_borrow_mut(|e| {
            e.next += 1;
            let id = e.next;
            e.hooks.insert(id, Rc::new(hook));
            if e.listener.is_none() {
                let listener = Closure::<dyn FnMut()>::new(|| {
                    let hooks =
                        EVENTS.with_borrow(|e| e.hooks.values().cloned().collect::<Vec<_>>());
                    for hook in hooks {
                        hook();
                    }
                });
                let callback = listener.as_ref().unchecked_ref();
                let _ = document().add_event_listener_with_callback("visibilitychange", callback);
                for event in ["online", "focus", "pageshow"] {
                    let _ = window().add_event_listener_with_callback(event, callback);
                }
                e.listener = Some(listener);
            }
            id
        })
    }
    pub fn visible() -> bool {
        document().visibility_state() != web_sys::VisibilityState::Hidden
    }
}
#[cfg(feature = "hydrate")]
pub use browser::{now, observe, register, visible};

/// Signal throttling/backoff for no-store owners. Initial/manual reads bypass the deadline;
/// automatic hints coalesce, including when a read fails before the next event in a burst.
#[derive(Clone, Debug, Default)]
pub struct WakePolicy {
    next_at: f64,
    failures: u32,
}
impl WakePolicy {
    pub fn confirmed(now: f64) -> Self {
        let mut policy = Self::default();
        policy.settled(now, true);
        policy
    }
    pub fn due(&self, now: f64) -> bool {
        now >= self.next_at
    }
    pub fn started(&mut self) {
        self.next_at = f64::INFINITY;
    }
    pub fn settled(&mut self, now: f64, success: bool) {
        if success {
            self.failures = 0;
            self.next_at = now + crate::freshness::Policy::V1.stale_after;
        } else {
            self.failures = self.failures.saturating_add(1);
            let p = crate::freshness::Policy::V1;
            self.next_at = now
                + (p.retry_base * 2f64.powi(self.failures.saturating_sub(1).min(16) as i32))
                    .min(p.retry_max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_burst_failure_and_hidden_pause_do_not_reset_retry_deadlines() {
        let mut wake = WakePolicy::default();
        assert!(wake.due(0.0));
        wake.started();
        for time in [0.0, 1.0, 120_000.0] {
            assert!(!wake.due(time));
        }
        wake.settled(120_000.0, false);
        assert!(!wake.due(124_999.0));
        assert!(wake.due(125_000.0));
        wake.started();
        wake.settled(125_000.0, false);
        assert!(!wake.due(134_999.0));
        assert!(wake.due(135_000.0));
        wake.started();
        wake.settled(135_000.0, true);
        assert!(!wake.due(254_999.0));
        assert!(wake.due(255_000.0));
    }
}
