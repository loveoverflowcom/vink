//! Markdown title authority (#263): the first H1 of the CommonMark AST, flattened to text.
//!
//! Dialect: CommonMark 0.31.2 core through `pulldown-cmark` with **no extensions** (no tables,
//! strikethrough, task lists, heading attributes, math or footnotes). The Markdown editor (#256)
//! and Reader (#264) must agree on this dialect and run the same corpus
//! (`tests/fixtures/title-authority/markdown.json`) before they render Markdown.
//!
//! Rules:
//! - Walk the whole AST in source order, including block quotes and list items; ATX and Setext
//!   H1 count. Fenced/indented code, raw HTML blocks (`<h1>`) and escaped `\#` are not headings.
//! - The **first** H1 decides. When it is empty, the projection is empty: a later H1 is content,
//!   never a fallback title.
//! - Flatten: text, code spans, link labels and image alt text in order; entities and escapes are
//!   already decoded by the parser; inline HTML tags are dropped (never executed or kept as
//!   markup); soft/hard breaks and tabs become one space.
//! - The caller applies `Title::parse` (trim, NFC, forbidden characters, 200-character limit).
//!   Nothing is truncated and the source is never rewritten.

pub mod submission;

pub use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};
use pulldown_cmark::{Options, Parser};

/// The canonical grammar for both title extraction and document rendering.
pub fn commonmark(source: &str) -> Parser<'_> {
    Parser::new_ext(source, Options::empty())
}

/// Flattened text of the first H1, before `Title::parse`. Empty when there is no H1 or the first
/// H1 has no text. Stops parsing at the end of that heading.
pub fn first_h1_projection(source: &str) -> String {
    let mut in_h1 = false;
    let mut text = String::new();
    for event in commonmark(source) {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => in_h1 = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => return text,
            Event::Text(part) | Event::Code(part) if in_h1 => {
                text.extend(part.chars().map(|c| if c == '\t' { ' ' } else { c }));
            }
            Event::SoftBreak | Event::HardBreak if in_h1 => text.push(' '),
            _ => {}
        }
    }
    text
}
