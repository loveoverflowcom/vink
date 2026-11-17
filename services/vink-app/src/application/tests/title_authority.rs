//! #263 title authority through `Vink::save_draft` → recording store → revision content.
//! Both corpora run the real application path; PostgreSQL CAS is covered by the postgres lane.
use super::*;
use crate::domain::document::{Document, TitledDocument};
use crate::domain::revision::{RevisionContent, RevisionError};
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultCase {
    id: String,
    title: String,
    source: String,
    expected_title: Option<String>,
    expected_error: Option<String>,
    expected_hash: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkdownCase {
    id: String,
    source: String,
    expected_projection: String,
    expected_validation_error: Option<String>,
}

fn title_error(raw: &str) -> TitleError {
    match raw {
        "too_long" => TitleError::TooLong,
        "forbidden_character" => TitleError::ForbiddenCharacter,
        _ => panic!("unknown fixture error: {raw}"),
    }
}

/// What the store received must rebuild into revision content: the stored boundary agrees with
/// the authoring boundary, and an empty effective title cannot satisfy `MissingTitle`.
fn assert_revision(
    id: &str,
    saved: &ObservedSave,
    expected_title: &str,
) -> Option<RevisionContent> {
    let document = Document::parse(saved.document.clone()).unwrap();
    let content = TitledDocument::from_stored(document, &saved.title).unwrap();
    let revision = RevisionContent::new(content);
    if expected_title.is_empty() {
        assert_eq!(revision, Err(RevisionError::MissingTitle), "{id}");
        None
    } else {
        let revision = revision.unwrap();
        assert_eq!(revision.title().as_str(), expected_title, "{id}");
        Some(revision)
    }
}

#[tokio::test]
async fn default_title_authority_corpus() {
    let cases: Vec<DefaultCase> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/title-authority/default.json"
    ))
    .unwrap();
    assert!(cases.len() >= 12);
    for case in cases {
        if case.id.contains("nfd") {
            // Guard against an editor or tool normalizing the fixture into a precomposed case.
            assert_ne!(
                case.title.nfc().collect::<String>(),
                case.title,
                "{} must keep non-NFC input",
                case.id
            );
        }
        let vink = app();
        let document = DocumentData {
            format: "default".into(),
            source: case.source.clone(),
        };
        let result = vink
            .save_draft(
                Some(actor()),
                SaveDraft {
                    title: Some(case.title),
                    document: document.clone(),
                    ..command()
                },
            )
            .await;
        let saves = vink.store().saves.lock().unwrap();
        if let Some(error) = case.expected_error {
            assert_eq!(
                result,
                Err(DraftError::Invalid(InvalidInput::Title(title_error(
                    &error
                )))),
                "{}",
                case.id
            );
            assert!(saves.is_empty(), "{} must not write", case.id);
            continue;
        }
        assert!(result.is_ok(), "{}: {result:?}", case.id);
        let saved = &saves[0];
        let expected_title = case.expected_title.expect("successful case needs a title");
        assert_eq!(saved.title, expected_title, "{}", case.id);
        // The body is literal and byte-identical: no Markdown sniffing, no NFC, no CRLF folding.
        assert_eq!(saved.document, document, "{}", case.id);
        let revision = assert_revision(&case.id, saved, &expected_title);
        if let (Some(revision), Some(hash)) = (revision, case.expected_hash) {
            assert_eq!(revision.hash().as_str(), hash, "{}", case.id);
        }
    }
}

#[tokio::test]
async fn markdown_title_authority_corpus() {
    let cases: Vec<MarkdownCase> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/title-authority/markdown.json"
    ))
    .unwrap();
    assert!(cases.len() >= 30);
    for case in cases {
        let vink = app();
        let document = DocumentData {
            format: "markdown".into(),
            source: case.source.clone(),
        };
        let result = vink
            .save_draft(
                Some(actor()),
                SaveDraft {
                    title: None,
                    document: document.clone(),
                    ..command()
                },
            )
            .await;
        let saves = vink.store().saves.lock().unwrap();
        if let Some(error) = case.expected_validation_error {
            assert_eq!(
                result,
                Err(DraftError::Invalid(InvalidInput::Title(title_error(
                    &error
                )))),
                "{}",
                case.id
            );
            assert!(saves.is_empty(), "{} must not write", case.id);
            continue;
        }
        assert!(result.is_ok(), "{}: {result:?}", case.id);
        let saved = &saves[0];
        assert_eq!(saved.title, case.expected_projection, "{}", case.id);
        assert_eq!(saved.document, document, "{} source is untouched", case.id);
        assert_revision(&case.id, saved, &case.expected_projection);
    }
}
