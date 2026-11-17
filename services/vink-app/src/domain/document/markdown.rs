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

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Flattened text of the first H1, before `Title::parse`. Empty when there is no H1 or the first
/// H1 has no text. Stops parsing at the end of that heading.
pub fn first_h1_projection(source: &str) -> String {
    let mut in_h1 = false;
    let mut text = String::new();
    for event in Parser::new_ext(source, Options::empty()) {
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

#[cfg(test)]
mod tests {
    use super::first_h1_projection;
    use crate::domain::document::{MAX_TITLE_CHARS, Title, TitleError};
    use serde::Deserialize;
    use unicode_normalization::UnicodeNormalization;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        id: String,
        source: String,
        expected_projection: String,
        expected_validation_error: Option<String>,
    }

    /// The shared corpus: each oracle was written from CommonMark 0.31.2, not from this code.
    #[test]
    fn markdown_title_corpus() {
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/title-authority/markdown.json"
        ))
        .unwrap();
        assert!(cases.len() >= 30);
        for case in cases {
            let projected = first_h1_projection(&case.source);
            let normalized: String = projected.trim().nfc().collect();
            assert_eq!(normalized, case.expected_projection, "{}", case.id);
            match case.expected_validation_error.as_deref() {
                None => {
                    let title = Title::parse(&projected).unwrap();
                    assert_eq!(title.as_str(), case.expected_projection, "{}", case.id);
                }
                Some("too_long") => {
                    assert_eq!(
                        Title::parse(&projected),
                        Err(TitleError::TooLong),
                        "{}",
                        case.id
                    )
                }
                Some("forbidden_character") => assert_eq!(
                    Title::parse(&projected),
                    Err(TitleError::ForbiddenCharacter),
                    "{}",
                    case.id
                ),
                Some(other) => panic!("{}: unknown expected error {other}", case.id),
            }
        }
    }

    #[test]
    fn first_h1_wins_and_recalculates_after_source_changes() {
        let source = "# Một\n\n# Hai\n";
        assert_eq!(first_h1_projection(source), "Một");
        let deleted_first = source.replacen("# Một\n\n", "", 1);
        assert_eq!(first_h1_projection(&deleted_first), "Hai");
        let renamed = source.replacen("Một", "Ba", 1);
        assert_eq!(first_h1_projection(&renamed), "Ba");
        assert_eq!(first_h1_projection(""), "");
    }

    #[test]
    fn nfd_source_projects_nfc_title_and_tabs_become_spaces() {
        let nfd = "# Vie\u{0323}\u{0302}t\tNam\n";
        let title = Title::parse(&first_h1_projection(nfd)).unwrap();
        assert_eq!(title.as_str(), "Việt Nam");
        assert_ne!(nfd.nfc().collect::<String>(), nfd, "fixture stays NFD");
    }

    #[test]
    fn grapheme_and_emoji_boundaries_count_scalars_after_nfc() {
        // ZWJ family emoji: 7 scalars. 28 of them = 196 scalars + "Mưa " (4) = 200: valid.
        let family = "👩‍👩‍👧‍👦".repeat(28);
        let at_limit = format!("# Mưa {family}");
        assert_eq!(
            Title::parse(&first_h1_projection(&at_limit))
                .unwrap()
                .as_str()
                .chars()
                .count(),
            MAX_TITLE_CHARS
        );
        let over = format!("# Mưa! {family}");
        assert_eq!(
            Title::parse(&first_h1_projection(&over)),
            Err(TitleError::TooLong)
        );
    }

    #[test]
    fn a_long_document_is_parsed_only_up_to_its_first_h1() {
        let mut long = String::from("# Tên\n\n");
        for i in 0..20_000 {
            long.push_str(&format!("Đoạn {i} có *nhấn* và `mã`.\n\n"));
        }
        assert_eq!(first_h1_projection(&long), "Tên");
        let mut tail = long.replacen("# Tên\n\n", "", 1);
        tail.push_str("Tên cuối\n===\n");
        assert_eq!(first_h1_projection(&tail), "Tên cuối");
    }
}
