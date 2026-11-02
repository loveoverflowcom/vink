//! Value types the generated `tokens` and `catalog` modules are written in.

/// An sRGB colour with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: f64) -> Self {
        Self { r, g, b, a }
    }

    /// `#rrggbb` when opaque, otherwise `rgb(r g b / a)` — the form `tokens.css` uses.
    pub fn css(self) -> String {
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("rgb({} {} {} / {})", self.r, self.g, self.b, self.a)
        }
    }

    /// `self` drawn over an opaque `below`.
    pub fn over(self, below: Rgba) -> Rgba {
        let mix = |top: u8, bottom: u8| {
            (f64::from(top) * self.a + f64::from(bottom) * (1.0 - self.a)).round() as u8
        };
        Rgba::new(
            mix(self.r, below.r),
            mix(self.g, below.g),
            mix(self.b, below.b),
            1.0,
        )
    }

    fn luminance(self) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// WCAG 2.x contrast ratio of `self` (composited over `bg`) on an opaque `bg`. This is the
    /// arithmetic for declared token pairs, not a certification of a rendered screen.
    pub fn contrast(self, bg: Rgba) -> f64 {
        let (a, b) = (self.over(bg).luminance(), bg.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebUnit {
    Rem,
    Px,
}

/// A length in CSS px (Web) / dp (CMP). The Web prints it in `web` units; rem assumes the
/// 16px browser default, so user text scaling still applies once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dimension {
    pub token: &'static str,
    pub css_var: &'static str,
    pub value: f64,
    /// Upper bound of a fluid Web size (`clamp`); CMP uses `value`.
    pub max: Option<f64>,
    /// Value under compact density, when the role depends on density.
    pub compact: Option<f64>,
    pub web: WebUnit,
}

impl Dimension {
    /// `var(--vk-…)`, so scopes and density apply.
    pub fn var(self) -> String {
        format!("var({})", self.css_var)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFamily {
    SansSerif,
    Serif,
    Monospace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontFamily {
    pub token: &'static str,
    pub css_var: &'static str,
    pub stack: &'static [&'static str],
    pub native: NativeFamily,
}

/// A self-hosted face: `weight` is a single weight or a variable range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontFace {
    pub family: &'static str,
    pub file: &'static str,
    pub weight: (u16, u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextSize {
    Fixed(f64),
    Fluid { min: f64, max: f64 },
}

/// A text role. CSS reads `{css_prefix}-{font,size,weight,line-height,tracking}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRole {
    pub token: &'static str,
    pub css_prefix: &'static str,
    pub family: FontFamily,
    pub size: TextSize,
    pub weight: u16,
    /// Unitless ratio of the font size.
    pub line_height: f64,
    pub tracking_em: f64,
}

impl TextRole {
    /// Inline declarations applying the role through its custom properties.
    pub fn style(self) -> String {
        let p = self.css_prefix;
        format!(
            "font-family: var({p}-font); font-size: var({p}-size); font-weight: var({p}-weight); \
             line-height: var({p}-line-height); letter-spacing: var({p}-tracking);"
        )
    }
}

/// A finite transition duration; `reduced_ms` is its reduced-motion value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    pub token: &'static str,
    pub css_var: &'static str,
    pub ms: u32,
    pub reduced_ms: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Easing {
    pub token: &'static str,
    pub css_var: &'static str,
    pub points: [f64; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContrastPair {
    pub fg: crate::tokens::ColorRole,
    pub bg: crate::tokens::ColorRole,
    pub min: f64,
    pub usage: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub id: &'static str,
    pub label: &'static str,
}

/// A preview registration. `leptos`/`cmp` are statuses from `catalog::STATUSES`; registration
/// never implies runtime, accessibility or design acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub family: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub scenarios: &'static [Scenario],
    pub leptos: &'static str,
    pub cmp: &'static str,
}

#[cfg(test)]
mod tests {
    use crate::tokens::{CONTRAST, ColorRole};

    #[test]
    fn every_declared_pair_meets_its_threshold_in_both_themes() {
        for pair in CONTRAST {
            for (fg, bg) in [
                (pair.fg.light(), pair.bg.light()),
                (pair.fg.dark(), pair.bg.dark()),
            ] {
                let ratio = fg.contrast(bg);
                assert!(ratio >= pair.min, "{pair:?}: {ratio:.2}");
            }
        }
    }

    #[test]
    fn recipes_resolve_to_their_semantic_role() {
        for role in ColorRole::ALL {
            if let Some(target) = role.alias() {
                assert_eq!(role.light(), target.light(), "{role:?}");
                assert_eq!(role.dark(), target.dark(), "{role:?}");
                assert_eq!(target.alias(), None, "recipes alias semantic roles");
            }
        }
    }

    #[test]
    fn css_forms_match_the_generated_stylesheet() {
        assert_eq!(ColorRole::Surface.light().css(), "#fffaff");
        assert_eq!(
            ColorRole::StateHover.light().css(),
            "rgb(104 70 165 / 0.08)"
        );
        let css = include_str!("../style/tokens.css");
        for role in ColorRole::ALL {
            assert!(
                css.contains(&format!("{}:", role.css_var())),
                "{role:?} has no custom property"
            );
        }
    }
}
