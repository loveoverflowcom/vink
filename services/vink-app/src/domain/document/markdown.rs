//! Server validation of the shared Markdown title projection.

pub use vink_markdown_text::first_h1_projection;

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
