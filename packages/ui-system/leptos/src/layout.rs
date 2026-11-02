//! Layout primitives: named spacing, measure and tone only — no per-screen numbers.

use leptos::prelude::*;

/// Spacing from `space.*`; `Density` follows the scope's `density.stack-gap`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Space {
    None,
    S1,
    S2,
    S3,
    #[default]
    S4,
    S6,
    S8,
    S12,
    Density,
}

impl Space {
    pub const fn var(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::S1 => "var(--vk-space-1)",
            Self::S2 => "var(--vk-space-2)",
            Self::S3 => "var(--vk-space-3)",
            Self::S4 => "var(--vk-space-4)",
            Self::S6 => "var(--vk-space-6)",
            Self::S8 => "var(--vk-space-8)",
            Self::S12 => "var(--vk-space-12)",
            Self::Density => "var(--vk-density-stack-gap)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SurfaceTone {
    #[default]
    Base,
    Low,
    Container,
    High,
    Highest,
    /// `primary-container` with its own content colour.
    Primary,
}

impl SurfaceTone {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Low => "low",
            Self::Container => "container",
            Self::High => "high",
            Self::Highest => "highest",
            Self::Primary => "primary",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Radius {
    None,
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
}

impl Radius {
    pub const fn var(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::Sm => "var(--vk-radius-sm)",
            Self::Md => "var(--vk-radius-md)",
            Self::Lg => "var(--vk-radius-lg)",
            Self::Xl => "var(--vk-radius-xl)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SurfaceTag {
    #[default]
    Div,
    Section,
    Article,
}

/// A toned area. It is not interactive; put a Link or Button inside instead of making it one.
#[component]
pub fn Surface(
    #[prop(optional)] tone: SurfaceTone,
    #[prop(optional)] radius: Radius,
    #[prop(optional)] padding: Space,
    #[prop(optional)] tag: SurfaceTag,
    #[prop(optional, into)] label: Option<String>,
    children: Children,
) -> impl IntoView {
    let class = format!("vk-ui-surface vk-ui-surface--{}", tone.id());
    let style = format!(
        "--vk-ui-radius: {}; --vk-ui-padding: {};",
        radius.var(),
        padding.var()
    );
    match tag {
        SurfaceTag::Div => {
            view! { <div class=class style=style aria-label=label>{children()}</div> }.into_any()
        }
        SurfaceTag::Section => {
            view! { <section class=class style=style aria-label=label>{children()}</section> }
                .into_any()
        }
        SurfaceTag::Article => {
            view! { <article class=class style=style aria-label=label>{children()}</article> }
                .into_any()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Measure {
    /// `measure` (42rem): long-form reading.
    #[default]
    Reading,
    /// `measure-wide` (70rem): page layouts.
    Wide,
    Full,
}

impl Measure {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Wide => "wide",
            Self::Full => "full",
        }
    }
}

/// Centres content at a named measure; it never adds side padding (the caller's layout does).
#[component]
pub fn Container(#[prop(optional)] measure: Measure, children: Children) -> impl IntoView {
    view! {
        <div class=format!("vk-ui-container vk-ui-container--{}", measure.id())>{children()}</div>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    #[default]
    Column,
    Row,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    #[default]
    Stretch,
    Start,
    Center,
    End,
    Baseline,
}

impl Align {
    pub const fn css(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Start => "flex-start",
            Self::Center => "center",
            Self::End => "flex-end",
            Self::Baseline => "baseline",
        }
    }
}

/// Children in DOM order with a named gap. Rows may wrap; reading order never changes.
#[component]
pub fn Stack(
    #[prop(optional)] direction: Direction,
    #[prop(optional)] gap: Space,
    #[prop(optional)] align: Align,
    #[prop(optional)] wrap: bool,
    children: Children,
) -> impl IntoView {
    let class = match direction {
        Direction::Column => "vk-ui-stack",
        Direction::Row => "vk-ui-stack vk-ui-stack--row",
    };
    let style = format!(
        "--vk-ui-gap: {}; --vk-ui-align: {};{}",
        gap.var(),
        align.css(),
        if wrap { " flex-wrap: wrap;" } else { "" }
    );
    view! { <div class=class style=style>{children()}</div> }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GridMin {
    Small,
    #[default]
    Medium,
}

impl GridMin {
    pub const fn var(self) -> &'static str {
        match self {
            Self::Small => "var(--vk-size-grid-min-sm)",
            Self::Medium => "var(--vk-size-grid-min-md)",
        }
    }
}

/// As many columns of at least `min` as fit; one column when even that does not.
#[component]
pub fn Grid(
    #[prop(optional)] min: GridMin,
    #[prop(optional)] gap: Space,
    children: Children,
) -> impl IntoView {
    let style = format!("--vk-ui-gap: {}; --vk-ui-min: {};", gap.var(), min.var());
    view! { <div class="vk-ui-grid" style=style>{children()}</div> }
}

/// `<hr>` separates content; a decorative divider is hidden from assistive technology.
#[component]
pub fn Divider(#[prop(optional)] decorative: bool) -> impl IntoView {
    if decorative {
        view! { <div class="vk-ui-divider" aria-hidden="true"></div> }.into_any()
    } else {
        view! { <hr class="vk-ui-divider" /> }.into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;

    #[test]
    fn layout_uses_named_tokens_only() {
        let html = render(|| {
            view! {
                <Stack direction=Direction::Row gap=Space::Density wrap=true>
                    <Surface tone=SurfaceTone::Primary radius=Radius::Lg padding=Space::S6>"x"</Surface>
                </Stack>
            }
            .into_any()
        });
        assert!(html.contains("var(--vk-density-stack-gap)"), "{html}");
        assert!(html.contains("flex-wrap: wrap"), "{html}");
        assert!(html.contains("vk-ui-surface--primary"), "{html}");
        assert!(html.contains("var(--vk-radius-lg)"), "{html}");
        let divider = render(|| view! { <Divider decorative=true /> }.into_any());
        assert!(divider.contains("aria-hidden=\"true\""), "{divider}");
        assert!(render(|| view! { <Divider /> }.into_any()).starts_with("<hr"));
    }
}
