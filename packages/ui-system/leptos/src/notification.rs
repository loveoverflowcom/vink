//! Notification primitives (#485, LayoutSpec `docs/design/notifications.md`): the bell's badge
//! and the expressive shapes of system marks. DTO-free: the host maps the Gateway's `freshness`
//! to [`BadgeState`] and never passes a number it could not verify, so no state here can show
//! the digit "0".

use leptos::prelude::*;

use crate::icons::{IconName, VIEW_BOX};

/// Largest number shown on a badge; anything above reads "99+".
pub const BADGE_CAP: u32 = 99;

/// What the bell may say about unseen groups.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BadgeState {
    /// A verified zero, a guest, or nothing known yet: no badge at all.
    #[default]
    Hidden,
    /// A verified count (`fresh`). Zero behaves as [`BadgeState::Hidden`].
    Count(u32),
    /// The Gateway supplied a lower bound for a fresh count.
    LowerBound,
    /// The last verified count while the source cannot be checked (`stale`): outlined; a stale
    /// zero is an empty outline, never "0".
    Stale(u32),
    /// The last known count was a lower bound.
    StaleLowerBound,
    /// The count could not be verified at all (`503`): an empty ring.
    Unknown,
}

impl BadgeState {
    /// The digits on the pill, if any: "1".."99", "99+".
    pub fn digits(self) -> Option<String> {
        match self {
            Self::LowerBound | Self::StaleLowerBound => Some(format!("{BADGE_CAP}+")),
            Self::Count(n) | Self::Stale(n) if n > BADGE_CAP => Some(format!("{BADGE_CAP}+")),
            Self::Count(n) | Self::Stale(n) if n > 0 => Some(n.to_string()),
            _ => None,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Hidden | Self::Count(0) => "hidden",
            Self::Count(_) | Self::LowerBound => "count",
            Self::Stale(_) | Self::StaleLowerBound => "stale",
            Self::Unknown => "unknown",
        }
    }

    /// The trigger's accessible name (LayoutSpec, Accessibility); the badge itself is hidden from
    /// assistive technology so the number is read once, with the bell.
    pub fn trigger_name(self, base: &str) -> String {
        match self {
            Self::Hidden | Self::Count(0) => base.to_string(),
            Self::LowerBound => format!("{base}, hơn {BADGE_CAP} mục mới"),
            Self::StaleLowerBound => {
                format!("{base}, hơn {BADGE_CAP} mục mới, có thể chưa cập nhật")
            }
            Self::Count(n) if n > BADGE_CAP => format!("{base}, hơn {BADGE_CAP} mục mới"),
            Self::Count(n) => format!("{base}, {n} mục mới"),
            Self::Stale(0) => format!("{base}, có thể chưa cập nhật"),
            Self::Stale(n) if n > BADGE_CAP => {
                format!("{base}, hơn {BADGE_CAP} mục mới, có thể chưa cập nhật")
            }
            Self::Stale(n) => format!("{base}, {n} mục mới, có thể chưa cập nhật"),
            Self::Unknown => format!("{base}, chưa kiểm tra được mục mới"),
        }
    }
}

/// The badge of a bell. Decorative: the trigger carries [`BadgeState::trigger_name`].
#[component]
pub fn NotificationBadge(#[prop(into)] state: Signal<BadgeState>) -> impl IntoView {
    view! {
        <span
            class="vk-ui-badge"
            data-state=move || state.get().id()
            hidden=move || state.get().id() == "hidden"
            aria-hidden="true"
        >
            {move || state.get().digits()}
        </span>
    }
}

/// M3 Expressive shapes used by notification marks (Thông báo R4 §4): a cookie of 8
/// lobes at 7 % amplitude and a soft burst of 12 lobes at 12 %, in a 100×100 box, rebuilt from
/// those parameters (not copied from the concept file).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExpressiveShape {
    Cookie,
    Burst,
}

impl ExpressiveShape {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cookie => "cookie",
            Self::Burst => "burst",
        }
    }

    pub const fn path(self) -> &'static str {
        match self {
            Self::Cookie => COOKIE,
            Self::Burst => BURST,
        }
    }
}

const COOKIE: &str = "M50.00 0.00 L52.60 0.35 L55.11 1.35 L57.47 2.85 L59.64 4.63 L61.67 6.44 L63.62 8.07 L65.60 9.36 L67.71 10.23 L70.01 10.72 L72.55 10.95 L75.26 11.10 L78.06 11.38 L80.78 11.98 L83.27 13.05 L85.36 14.64 L86.95 16.73 L88.02 19.22 L88.62 21.94 L88.90 24.74 L89.05 27.45 L89.28 29.99 L89.77 32.29 L90.64 34.40 L91.93 36.38 L93.56 38.33 L95.37 40.36 L97.15 42.53 L98.65 44.89 L99.65 47.40 L100.00 50.00 L99.65 52.60 L98.65 55.11 L97.15 57.47 L95.37 59.64 L93.56 61.67 L91.93 63.62 L90.64 65.60 L89.77 67.71 L89.28 70.01 L89.05 72.55 L88.90 75.26 L88.62 78.06 L88.02 80.78 L86.95 83.27 L85.36 85.36 L83.27 86.95 L80.78 88.02 L78.06 88.62 L75.26 88.90 L72.55 89.05 L70.01 89.28 L67.71 89.77 L65.60 90.64 L63.62 91.93 L61.67 93.56 L59.64 95.37 L57.47 97.15 L55.11 98.65 L52.60 99.65 L50.00 100.00 L47.40 99.65 L44.89 98.65 L42.53 97.15 L40.36 95.37 L38.33 93.56 L36.38 91.93 L34.40 90.64 L32.29 89.77 L29.99 89.28 L27.45 89.05 L24.74 88.90 L21.94 88.62 L19.22 88.02 L16.73 86.95 L14.64 85.36 L13.05 83.27 L11.98 80.78 L11.38 78.06 L11.10 75.26 L10.95 72.55 L10.72 70.01 L10.23 67.71 L9.36 65.60 L8.07 63.62 L6.44 61.67 L4.63 59.64 L2.85 57.47 L1.35 55.11 L0.35 52.60 L0.00 50.00 L0.35 47.40 L1.35 44.89 L2.85 42.53 L4.63 40.36 L6.44 38.33 L8.07 36.38 L9.36 34.40 L10.23 32.29 L10.72 29.99 L10.95 27.45 L11.10 24.74 L11.38 21.94 L11.98 19.22 L13.05 16.73 L14.64 14.64 L16.73 13.05 L19.22 11.98 L21.94 11.38 L24.74 11.10 L27.45 10.95 L29.99 10.72 L32.29 10.23 L34.40 9.36 L36.38 8.07 L38.33 6.44 L40.36 4.63 L42.53 2.85 L44.89 1.35 L47.40 0.35Z";
const BURST: &str = "M50.00 0.00 L52.56 1.09 L54.84 3.96 L56.72 7.54 L58.38 10.57 L60.17 12.05 L62.46 11.66 L65.41 9.87 L68.83 7.70 L72.24 6.36 L75.00 6.70 L76.67 8.92 L77.21 12.54 L77.05 16.59 L76.97 20.04 L77.78 22.22 L79.96 23.03 L83.41 22.95 L87.46 22.79 L91.08 23.33 L93.30 25.00 L93.64 27.76 L92.30 31.17 L90.13 34.59 L88.34 37.54 L87.95 39.83 L89.43 41.62 L92.46 43.28 L96.04 45.16 L98.91 47.44 L100.00 50.00 L98.91 52.56 L96.04 54.84 L92.46 56.72 L89.43 58.38 L87.95 60.17 L88.34 62.46 L90.13 65.41 L92.30 68.83 L93.64 72.24 L93.30 75.00 L91.08 76.67 L87.46 77.21 L83.41 77.05 L79.96 76.97 L77.78 77.78 L76.97 79.96 L77.05 83.41 L77.21 87.46 L76.67 91.08 L75.00 93.30 L72.24 93.64 L68.83 92.30 L65.41 90.13 L62.46 88.34 L60.17 87.95 L58.38 89.43 L56.72 92.46 L54.84 96.04 L52.56 98.91 L50.00 100.00 L47.44 98.91 L45.16 96.04 L43.28 92.46 L41.62 89.43 L39.83 87.95 L37.54 88.34 L34.59 90.13 L31.17 92.30 L27.76 93.64 L25.00 93.30 L23.33 91.08 L22.79 87.46 L22.95 83.41 L23.03 79.96 L22.22 77.78 L20.04 76.97 L16.59 77.05 L12.54 77.21 L8.92 76.67 L6.70 75.00 L6.36 72.24 L7.70 68.83 L9.87 65.41 L11.66 62.46 L12.05 60.17 L10.57 58.38 L7.54 56.72 L3.96 54.84 L1.09 52.56 L0.00 50.00 L1.09 47.44 L3.96 45.16 L7.54 43.28 L10.57 41.62 L12.05 39.83 L11.66 37.54 L9.87 34.59 L7.70 31.17 L6.36 27.76 L6.70 25.00 L8.92 23.33 L12.54 22.79 L16.59 22.95 L20.04 23.03 L22.22 22.22 L23.03 20.04 L22.95 16.59 L22.79 12.54 L23.33 8.92 L25.00 6.70 L27.76 6.36 L31.17 7.70 L34.59 9.87 L37.54 11.66 L39.83 12.05 L41.62 10.57 L43.28 7.54 L45.16 3.96 L47.44 1.09Z";

/// Fill of a shape mark, mapped to colour roles in CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShapeTone {
    /// `primary` / `on-primary`: type glyph on an avatar.
    #[default]
    Primary,
    /// `primary-fixed` / `on-primary-fixed`: approval, empty state.
    PrimaryFixed,
    /// `tertiary` / `on-tertiary`: rejection that may need action.
    Tertiary,
    /// `surface-container-highest` / `on-surface`: removed comment.
    Neutral,
}

impl ShapeTone {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::PrimaryFixed => "primary-fixed",
            Self::Tertiary => "tertiary",
            Self::Neutral => "neutral",
        }
    }
}

/// An icon on an expressive shape. Decorative: the row or panel names what it means.
#[component]
pub fn ShapeMark(
    shape: ExpressiveShape,
    icon: IconName,
    #[prop(optional)] tone: ShapeTone,
    /// Render as the 34 px glyph on a person avatar.
    #[prop(optional)]
    glyph: bool,
) -> impl IntoView {
    view! {
        <span
            class="vk-ui-shape-mark"
            data-shape=shape.id()
            data-tone=tone.id()
            class:vk-ui-shape-mark--glyph=glyph
            aria-hidden="true"
        >
            <svg class="vk-ui-shape-mark__shape" viewBox="0 0 100 100" focusable="false">
                <path d=shape.path() fill="currentColor" vector-effect="non-scaling-stroke"></path>
            </svg>
            <svg class="vk-ui-shape-mark__icon" viewBox=VIEW_BOX focusable="false">
                <path d=icon.path() fill="currentColor"></path>
            </svg>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_badge_state_shows_the_digit_zero() {
        for state in [
            BadgeState::Hidden,
            BadgeState::Count(0),
            BadgeState::Stale(0),
            BadgeState::Unknown,
        ] {
            assert_eq!(state.digits(), None, "{state:?}");
        }
        assert_eq!(BadgeState::Count(0).id(), "hidden");
        assert_eq!(BadgeState::Stale(0).id(), "stale");
        assert_eq!(BadgeState::Unknown.id(), "unknown");
    }

    #[test]
    fn counts_cap_at_ninety_nine_plus() {
        assert_eq!(BadgeState::Count(7).digits().as_deref(), Some("7"));
        assert_eq!(BadgeState::Count(99).digits().as_deref(), Some("99"));
        assert_eq!(BadgeState::Count(100).digits().as_deref(), Some("99+"));
        assert_eq!(BadgeState::Stale(250).digits().as_deref(), Some("99+"));
        assert_eq!(BadgeState::LowerBound.digits().as_deref(), Some("99+"));
        assert_eq!(BadgeState::StaleLowerBound.digits().as_deref(), Some("99+"));
    }

    #[test]
    fn trigger_name_follows_the_layout_spec() {
        let name = |state: BadgeState| state.trigger_name("Thông báo");
        assert_eq!(name(BadgeState::Hidden), "Thông báo");
        assert_eq!(name(BadgeState::Count(0)), "Thông báo");
        assert_eq!(name(BadgeState::Count(3)), "Thông báo, 3 mục mới");
        assert_eq!(name(BadgeState::Count(120)), "Thông báo, hơn 99 mục mới");
        assert_eq!(name(BadgeState::LowerBound), "Thông báo, hơn 99 mục mới");
        assert_eq!(
            name(BadgeState::Stale(3)),
            "Thông báo, 3 mục mới, có thể chưa cập nhật"
        );
        assert_eq!(
            name(BadgeState::Stale(0)),
            "Thông báo, có thể chưa cập nhật"
        );
        assert_eq!(
            name(BadgeState::StaleLowerBound),
            "Thông báo, hơn 99 mục mới, có thể chưa cập nhật"
        );
        assert_eq!(
            name(BadgeState::Unknown),
            "Thông báo, chưa kiểm tra được mục mới"
        );
    }

    #[test]
    fn hidden_badge_renders_without_digits() {
        let html = Owner::new().with(|| {
            view! { <NotificationBadge state=Signal::stored(BadgeState::Count(0)) /> }.to_html()
        });
        assert!(html.contains("data-state=\"hidden\""), "{html}");
        assert!(html.contains(" hidden"), "{html}");
        assert!(!html.contains(">0<"), "{html}");
    }
}
