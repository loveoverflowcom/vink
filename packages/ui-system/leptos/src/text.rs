//! Text and Heading: a type role chooses the look, the tag or level chooses the meaning.

use leptos::prelude::*;

use crate::tokens::text;
use crate::values::TextRole;

/// Shared text roles from `tokens.json` (`type.*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TypeRole {
    Display,
    Headline,
    Title,
    #[default]
    Body,
    Label,
    Caption,
    Reading,
    Code,
}

impl TypeRole {
    pub const ALL: [TypeRole; 8] = [
        Self::Display,
        Self::Headline,
        Self::Title,
        Self::Body,
        Self::Label,
        Self::Caption,
        Self::Reading,
        Self::Code,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Headline => "headline",
            Self::Title => "title",
            Self::Body => "body",
            Self::Label => "label",
            Self::Caption => "caption",
            Self::Reading => "reading",
            Self::Code => "code",
        }
    }

    pub const fn text_role(self) -> TextRole {
        match self {
            Self::Display => text::DISPLAY,
            Self::Headline => text::HEADLINE,
            Self::Title => text::TITLE,
            Self::Body => text::BODY,
            Self::Label => text::LABEL,
            Self::Caption => text::CAPTION,
            Self::Reading => text::READING,
            Self::Code => text::CODE,
        }
    }
}

/// Text colour roles that keep 4.5:1 on surfaces (`color.success` is non-text only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextTone {
    #[default]
    Default,
    Muted,
    Primary,
    Error,
    Warning,
}

impl TextTone {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Muted => "muted",
            Self::Primary => "primary",
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextTag {
    #[default]
    P,
    Span,
    Div,
    Strong,
}

fn class(role: TypeRole, tone: TextTone) -> String {
    format!(
        "vk-ui-text vk-ui-type-{} vk-ui-tone-{}",
        role.id(),
        tone.id()
    )
}

/// Text in a type role. Line breaks in `Reading` text are preserved (`white-space: pre-line`).
#[component]
pub fn Text(
    #[prop(optional)] role: TypeRole,
    #[prop(optional)] tone: TextTone,
    #[prop(optional)] tag: TextTag,
    #[prop(optional, into)] id: Option<String>,
    children: Children,
) -> impl IntoView {
    let class = class(role, tone);
    match tag {
        TextTag::P => view! { <p class=class id=id>{children()}</p> }.into_any(),
        TextTag::Span => view! { <span class=class id=id>{children()}</span> }.into_any(),
        TextTag::Div => view! { <div class=class id=id>{children()}</div> }.into_any(),
        TextTag::Strong => view! { <strong class=class id=id>{children()}</strong> }.into_any(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingLevel {
    /// The look a level gets when the caller does not choose one.
    pub const fn default_role(self) -> TypeRole {
        match self {
            Self::H1 => TypeRole::Display,
            Self::H2 => TypeRole::Headline,
            Self::H3 => TypeRole::Title,
            _ => TypeRole::Label,
        }
    }
}

/// A document heading; `role` changes only its look, never the outline level.
#[component]
pub fn Heading(
    level: HeadingLevel,
    #[prop(optional)] role: Option<TypeRole>,
    #[prop(optional)] tone: TextTone,
    #[prop(optional, into)] id: Option<String>,
    children: Children,
) -> impl IntoView {
    let class = class(role.unwrap_or(level.default_role()), tone) + " vk-ui-heading";
    match level {
        HeadingLevel::H1 => view! { <h1 class=class id=id>{children()}</h1> }.into_any(),
        HeadingLevel::H2 => view! { <h2 class=class id=id>{children()}</h2> }.into_any(),
        HeadingLevel::H3 => view! { <h3 class=class id=id>{children()}</h3> }.into_any(),
        HeadingLevel::H4 => view! { <h4 class=class id=id>{children()}</h4> }.into_any(),
        HeadingLevel::H5 => view! { <h5 class=class id=id>{children()}</h5> }.into_any(),
        HeadingLevel::H6 => view! { <h6 class=class id=id>{children()}</h6> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;

    #[test]
    fn heading_level_is_independent_of_its_look() {
        let html = render(|| {
            view! { <Heading level=HeadingLevel::H2 role=TypeRole::Caption>"Mục"</Heading> }
                .into_any()
        });
        assert!(html.starts_with("<h2"), "{html}");
        assert!(html.contains("vk-ui-type-caption"), "{html}");
        let html =
            render(|| view! { <Text role=TypeRole::Title tag=TextTag::Span>"x"</Text> }.into_any());
        assert!(
            html.starts_with("<span") && html.contains("vk-ui-type-title"),
            "{html}"
        );
    }

    #[test]
    fn every_type_role_has_a_token() {
        for role in TypeRole::ALL {
            assert!(role.text_role().token.ends_with(role.id()), "{role:?}");
        }
    }
}
