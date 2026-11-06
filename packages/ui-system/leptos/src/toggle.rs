//! ToggleAction (shared/components.md): a compact on/off action that shows what its owner
//! **confirmed**, never an optimistic guess. A press emits the desired state (the opposite of the
//! confirmed value), so a double tap or a late answer cannot flip it twice; nothing is emitted
//! while pending, uncertain, blocked or unknown. An uncertain write offers Reconcile, never a
//! blind replay. A count is a fact: unavailable reads as a mark, never 0. Request identity,
//! ordering fences and stale acknowledgements stay with the caller's reducer; the component only
//! renders the projection (`toggle-contract-v1`, shared with CMP through `toggle_vectors`).

use leptos::prelude::*;

use crate::action::{Button, ButtonVariant};
use crate::feedback::LoadingIndicator;
use crate::icons::IconName;
use crate::media::{Icon, IconSize};
use crate::state::RecoveryIntent;

/// What the owner last confirmed for this viewer. `Unknown`: not read yet, failed to read or
/// not the viewer's to know; it never defaults to Off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToggleValue {
    Off,
    On,
    #[default]
    Unknown,
}

impl ToggleValue {
    pub const fn from_selected(selected: bool) -> Self {
        if selected { Self::On } else { Self::Off }
    }

    pub const fn known(self) -> Option<bool> {
        match self {
            Self::Off => Some(false),
            Self::On => Some(true),
            Self::Unknown => None,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Unknown => "unknown",
        }
    }
}

/// The caller's latest intent for this target. `Failed` is a known refusal (nothing changed);
/// a lost or timed-out answer is `Uncertain`, which blocks presses until reconciled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TogglePhase {
    #[default]
    Idle,
    Pending {
        desired: bool,
    },
    Uncertain {
        desired: bool,
    },
    Failed,
}

impl TogglePhase {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Pending { desired: true } => "pending:on",
            Self::Pending { desired: false } => "pending:off",
            Self::Uncertain { desired: true } => "uncertain:on",
            Self::Uncertain { desired: false } => "uncertain:off",
            Self::Failed => "failed",
        }
    }
}

/// An aggregate the caller may show. `Hidden`: no capability to show a count here.
/// `Unavailable`: the read failed or the target is not visible; never shown as 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CountFact {
    #[default]
    Hidden,
    Unavailable,
    Known(u64),
}

impl CountFact {
    /// A server count (`bigint ≥ 0`); a negative value is an integrity error, shown unavailable.
    pub fn from_i64(count: i64) -> Self {
        u64::try_from(count).map_or(Self::Unavailable, Self::Known)
    }

    /// The visible text: digits grouped with `.` (vi-VN), `–` when unavailable, nothing when
    /// hidden.
    pub fn text(self) -> Option<String> {
        match self {
            Self::Hidden => None,
            Self::Unavailable => Some("–".to_string()),
            Self::Known(n) => {
                let digits = n.to_string();
                let mut out = String::with_capacity(digits.len() + digits.len() / 3);
                for (i, c) in digits.chars().enumerate() {
                    if i > 0 && (digits.len() - i) % 3 == 0 {
                        out.push('.');
                    }
                    out.push(c);
                }
                Some(out)
            }
        }
    }
}

/// Which explanation sits under the control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToggleNote {
    PendingOn,
    PendingOff,
    Uncertain,
    Failed,
    Unknown,
    Blocked,
}

impl ToggleNote {
    pub const fn id(self) -> &'static str {
        match self {
            Self::PendingOn => "pending-on",
            Self::PendingOff => "pending-off",
            Self::Uncertain => "uncertain",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Blocked => "disabled",
        }
    }
}

/// The resolved projection of value × phase × availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToggleView {
    /// The confirmed state shown (`None` = unknown: no mark, no pressed claim).
    pub pressed: Option<bool>,
    pub busy: bool,
    /// The desired state a press emits; `None` ignores the press.
    pub dispatch: Option<bool>,
    pub note: Option<ToggleNote>,
    /// Offer Reconcile (check the current state) instead of a press.
    pub reconcile: bool,
}

impl ToggleView {
    /// Precedence: pending → uncertain → blocked → unknown → failed → idle.
    pub const fn resolve(value: ToggleValue, phase: TogglePhase, enabled: bool) -> Self {
        let pressed = value.known();
        let mut view = Self {
            pressed,
            busy: false,
            dispatch: None,
            note: None,
            reconcile: false,
        };
        match phase {
            TogglePhase::Pending { desired } => {
                view.busy = true;
                view.note = Some(if desired {
                    ToggleNote::PendingOn
                } else {
                    ToggleNote::PendingOff
                });
            }
            TogglePhase::Uncertain { .. } => {
                view.note = Some(ToggleNote::Uncertain);
                view.reconcile = true;
            }
            _ if !enabled => view.note = Some(ToggleNote::Blocked),
            _ => match pressed {
                None => {
                    view.note = Some(ToggleNote::Unknown);
                    view.reconcile = true;
                }
                Some(on) => {
                    view.dispatch = Some(!on);
                    if matches!(phase, TogglePhase::Failed) {
                        view.note = Some(ToggleNote::Failed);
                    }
                }
            },
        }
        view
    }
}

/// Every string the control shows or speaks. `label` stays visible in every state;
/// `selected_name` is the accessible name once On is confirmed and must contain the label's
/// words (WCAG 2.5.3), e.g. "Đánh giá cao" / "Bỏ đánh giá cao".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleCopy {
    pub label: String,
    pub selected_name: String,
    /// Spoken after the number: "12 lượt đánh giá cao".
    pub count_unit: String,
    pub count_unavailable: String,
    pub pending_on: String,
    pub pending_off: String,
    pub uncertain: String,
    pub failed: String,
    pub unknown: String,
    pub reconcile: String,
}

impl ToggleCopy {
    /// Neutral Vietnamese defaults; features replace what their contract names.
    pub fn new(label: impl Into<String>, selected_name: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selected_name: selected_name.into(),
            count_unit: "lượt".into(),
            count_unavailable: "chưa có số liệu".into(),
            pending_on: "Đang gửi…".into(),
            pending_off: "Đang gửi…".into(),
            uncertain: "Chưa xác nhận được.".into(),
            failed: "Chưa thực hiện được. Bạn có thể thử lại.".into(),
            unknown: "Chưa rõ trạng thái.".into(),
            reconcile: "Kiểm tra lại".into(),
        }
    }

    /// The text for a note; `Blocked` uses the caller's reason.
    pub fn note(&self, note: ToggleNote, reason: Option<&str>) -> Option<String> {
        Some(match note {
            ToggleNote::PendingOn => self.pending_on.clone(),
            ToggleNote::PendingOff => self.pending_off.clone(),
            ToggleNote::Uncertain => self.uncertain.clone(),
            ToggleNote::Failed => self.failed.clone(),
            ToggleNote::Unknown => self.unknown.clone(),
            ToggleNote::Blocked => {
                return reason.filter(|r| !r.trim().is_empty()).map(str::to_string);
            }
        })
    }

    pub fn name(&self, pressed: Option<bool>) -> &str {
        if pressed == Some(true) {
            &self.selected_name
        } else {
            &self.label
        }
    }
}

/// A confirmed on/off action with an optional count. One `<button>` target; the accessible name
/// follows the confirmed value, the count and note are its description, the note is a polite
/// live region, and Reconcile is a separate Text button that appears only when needed.
/// `on_set` must move the caller's phase to `Pending` before it returns, so a second click on
/// the same frame is ignored.
#[component]
pub fn ToggleAction(
    /// Stable, unique: `{id}-count` and `{id}-note` are derived from it for SSR/hydration.
    #[prop(into)]
    id: String,
    copy: ToggleCopy,
    #[prop(into)] value: Signal<ToggleValue>,
    #[prop(optional, into)] phase: Signal<TogglePhase>,
    #[prop(optional, into)] count: Signal<CountFact>,
    /// `Some(reason)` blocks presses and shows the reason (never hover-only).
    #[prop(optional, into)]
    blocked: Signal<Option<String>>,
    /// Leading glyph from the registry (kept in every state); the check mark is separate.
    #[prop(optional)]
    icon: Option<IconName>,
    #[prop(into)] on_set: Callback<bool>,
    #[prop(into)] on_recover: Callback<RecoveryIntent>,
) -> impl IntoView {
    let copy = StoredValue::new(copy);
    let view = Memo::new(move |_| {
        ToggleView::resolve(value.get(), phase.get(), blocked.with(Option::is_none))
    });
    let count_id = format!("{id}-count");
    let note_id = format!("{id}-note");
    let note_text = Memo::new(move |_| {
        let note = view.get().note?;
        blocked.with(|reason| copy.with_value(|c| c.note(note, reason.as_deref())))
    });
    let described = {
        let (count_id, note_id) = (count_id.clone(), note_id.clone());
        move || {
            let mut ids = Vec::new();
            if count.with(|c| *c != CountFact::Hidden) {
                ids.push(count_id.clone());
            }
            if note_text.with(Option::is_some) {
                ids.push(note_id.clone());
            }
            (!ids.is_empty()).then(|| ids.join(" "))
        }
    };
    // `held`: uncertain or unknown; the confirmed look stays, presses wait for Reconcile.
    let state = move || {
        let v = view.get();
        match (v.busy, v.dispatch, v.note) {
            (true, ..) => "pending",
            (_, Some(_), _) => "enabled",
            (_, None, Some(ToggleNote::Blocked)) => "disabled",
            _ => "held",
        }
    };
    let pressed = move || match view.get().pressed {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    };
    view! {
        <span class="vk-ui-action vk-ui-toggle" data-phase=move || phase.get().id()>
            <button
                type="button"
                id=id
                class="vk-ui-toggle__control"
                aria-label=move || copy.with_value(|c| c.name(view.get().pressed).to_string())
                aria-describedby=described
                aria-disabled=move || view.get().dispatch.is_none().then_some("true")
                aria-busy=move || view.get().busy.then_some("true")
                data-state=state
                data-pressed=pressed
                on:click=move |ev| match view.get_untracked().dispatch {
                    Some(desired) => on_set.run(desired),
                    None => ev.prevent_default(),
                }
            >
                {icon.map(|name| view! {
                    <span class="vk-ui-toggle__glyph" aria-hidden="true">
                        <Icon name=name size=IconSize::Small />
                    </span>
                })}
                <span class="vk-ui-toggle__label">{copy.with_value(|c| c.label.clone())}</span>
                {move || {
                    let fact = count.get();
                    fact.text().map(|shown| {
                        let unit = copy.with_value(|c| match fact {
                            CountFact::Unavailable => format!("{}: {}", c.count_unit, c.count_unavailable),
                            _ => format!(" {}", c.count_unit),
                        });
                        let unavailable = fact == CountFact::Unavailable;
                        let kind = if unavailable { "unavailable" } else { "known" };
                        view! {
                            <span class="vk-ui-toggle__count" id=count_id.clone() data-count=kind>
                                <span aria-hidden=unavailable.then_some("true")>{shown}</span>
                                <span class="vk-ui-visually-hidden">{unit}</span>
                            </span>
                        }
                    })
                }}
                // One reserved trailing slot, like Button's status slot: MicroArc while pending,
                // the check mark once On is confirmed; the label never moves.
                <span class="vk-ui-toggle__mark" aria-hidden="true">
                    {move || {
                        let v = view.get();
                        if v.busy {
                            Some(view! { <LoadingIndicator /> }.into_any())
                        } else {
                            (v.pressed == Some(true)).then(|| {
                                view! { <Icon name=IconName::Check size=IconSize::Small /> }.into_any()
                            })
                        }
                    }}
                </span>
            </button>
            <span
                class="vk-ui-toggle__feedback"
                aria-live="polite"
                data-active=move || (note_text.with(Option::is_some) || view.get().reconcile).then_some("true")
            >
                {move || note_text.get().map(|text| view! {
                    <span class="vk-ui-action__note" id=note_id.clone()>{text}</span>
                })}
                {move || view.get().reconcile.then(|| view! {
                    <Button
                        variant=ButtonVariant::Text
                        on_press=move |_| on_recover.run(RecoveryIntent::Reconcile)
                    >
                        {copy.with_value(|c| c.reconcile.clone())}
                    </Button>
                })}
            </span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;
    use crate::toggle_vectors as v;

    fn value(id: &str) -> ToggleValue {
        match id {
            "off" => ToggleValue::Off,
            "on" => ToggleValue::On,
            "unknown" => ToggleValue::Unknown,
            other => panic!("value {other}"),
        }
    }

    fn phase(id: &str) -> TogglePhase {
        match id {
            "idle" => TogglePhase::Idle,
            "pending:on" => TogglePhase::Pending { desired: true },
            "pending:off" => TogglePhase::Pending { desired: false },
            "uncertain:on" => TogglePhase::Uncertain { desired: true },
            "uncertain:off" => TogglePhase::Uncertain { desired: false },
            "failed" => TogglePhase::Failed,
            other => panic!("phase {other}"),
        }
    }

    fn known(id: &str) -> Option<bool> {
        match id {
            "on" => Some(true),
            "off" => Some(false),
            "-" => None,
            other => panic!("state {other}"),
        }
    }

    #[test]
    fn every_shared_vector_resolves_the_same() {
        assert_eq!(v::VERSION, "toggle-contract-v1");
        assert_eq!(v::VIEWS.len(), 36);
        for &(val, ph, avail, pressed, busy, dispatch, note, recover) in v::VIEWS {
            let got = ToggleView::resolve(value(val), phase(ph), avail == "enabled");
            let case = format!("{val} × {ph} × {avail}");
            assert_eq!(value(val).id(), val, "{case}");
            assert_eq!(phase(ph).id(), ph, "{case}");
            assert_eq!(got.pressed, known(pressed), "pressed {case}");
            assert_eq!(got.busy, busy, "busy {case}");
            assert_eq!(got.dispatch, known(dispatch), "dispatch {case}");
            assert_eq!(got.note.map_or("-", ToggleNote::id), note, "note {case}");
            assert_eq!(got.reconcile, recover == "reconcile", "recover {case}");
        }
    }

    #[test]
    fn counts_read_as_facts_and_unavailable_never_becomes_zero() {
        for &(fact, shown) in v::COUNTS {
            let parsed = match fact {
                "hidden" => CountFact::Hidden,
                "unavailable" => CountFact::Unavailable,
                known => CountFact::Known(known.trim_start_matches("known:").parse().unwrap()),
            };
            assert_eq!(parsed.text().as_deref(), shown, "{fact}");
        }
        assert_eq!(CountFact::from_i64(-1), CountFact::Unavailable);
        assert_eq!(CountFact::from_i64(0), CountFact::Known(0));
        assert_eq!(ToggleValue::default(), ToggleValue::Unknown);
    }

    fn sample(
        value: ToggleValue,
        phase: TogglePhase,
        count: CountFact,
        blocked: Option<&str>,
    ) -> String {
        let blocked = blocked.map(str::to_string);
        render(move || {
            view! {
                <ToggleAction
                    id="work-1"
                    copy=ToggleCopy::new("Đánh giá cao", "Bỏ đánh giá cao")
                    value=value
                    phase=phase
                    count=count
                    blocked=blocked
                    on_set=|_: bool| {}
                    on_recover=|_: RecoveryIntent| {}
                />
            }
            .into_any()
        })
    }

    fn button(html: &str) -> &str {
        html.split("<button")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap()
    }

    #[test]
    fn the_name_follows_the_confirmed_value_and_the_count_describes_it() {
        let html = sample(
            ToggleValue::Off,
            TogglePhase::Idle,
            CountFact::Known(1234),
            None,
        );
        let b = button(&html);
        for needle in [
            "type=\"button\"",
            "aria-label=\"Đánh giá cao\"",
            "aria-describedby=\"work-1-count\"",
            "data-pressed=\"false\"",
            "data-state=\"enabled\"",
        ] {
            assert!(b.contains(needle), "{needle} in {b}");
        }
        assert!(
            !b.contains("aria-pressed"),
            "action-named, not a pressed toggle: {b}"
        );
        assert!(html.contains("1.234") && html.contains("lượt"), "{html}");

        let html = sample(
            ToggleValue::On,
            TogglePhase::Idle,
            CountFact::Known(0),
            None,
        );
        assert!(
            button(&html).contains("aria-label=\"Bỏ đánh giá cao\""),
            "{html}"
        );
        assert!(
            html.contains("data-pressed=\"true\"") && html.contains("<svg"),
            "check mark: {html}"
        );
        assert!(html.contains(">0<"), "a known zero is shown: {html}");
    }

    #[test]
    fn pending_uncertain_blocked_and_unavailable_are_visible_without_colour() {
        let html = sample(
            ToggleValue::Off,
            TogglePhase::Pending { desired: true },
            CountFact::Known(3),
            None,
        );
        let b = button(&html);
        assert!(
            b.contains("aria-busy=\"true\"") && b.contains("aria-disabled=\"true\""),
            "{b}"
        );
        assert!(
            b.contains("aria-describedby=\"work-1-count work-1-note\""),
            "{b}"
        );
        assert!(
            html.contains("vk-ui-arc") && html.contains("Đang gửi…"),
            "{html}"
        );

        let html = sample(
            ToggleValue::On,
            TogglePhase::Uncertain { desired: false },
            CountFact::Unavailable,
            None,
        );
        assert!(
            html.contains("Chưa xác nhận được.") && html.contains("Kiểm tra lại"),
            "{html}"
        );
        assert!(
            html.contains("data-count=\"unavailable\"") && html.contains("chưa có số liệu"),
            "{html}"
        );
        assert!(!html.contains(">0<"), "unavailable is not zero: {html}");

        let html = sample(
            ToggleValue::Off,
            TogglePhase::Idle,
            CountFact::Hidden,
            Some("Bạn không thể đánh giá tác phẩm của mình."),
        );
        let b = button(&html);
        assert!(
            b.contains("aria-disabled=\"true\"") && b.contains("aria-describedby=\"work-1-note\""),
            "{b}"
        );
        assert!(
            html.contains("Bạn không thể đánh giá tác phẩm của mình."),
            "{html}"
        );
        assert!(
            !html.contains("work-1-count"),
            "hidden count has no element: {html}"
        );

        let html = sample(
            ToggleValue::Unknown,
            TogglePhase::Idle,
            CountFact::Known(2),
            None,
        );
        assert!(button(&html).contains("data-pressed=\"unknown\""), "{html}");
        assert!(
            html.contains("Chưa rõ trạng thái.") && html.contains("Kiểm tra lại"),
            "{html}"
        );
    }
}
