//! Executable legacy guards for #263. Markdown integration awaits source contract #255.
use super::*;
use crate::domain::revision::{RevisionContent, RevisionError};
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCase {
    id: String,
    title: String,
    blocks: Vec<Vec<String>>,
    expected_title: Option<String>,
    expected_error: Option<String>,
    expected_v1_hash: Option<String>,
}

#[tokio::test]
async fn legacy_v1_title_authority_corpus() {
    let cases: Vec<LegacyCase> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/title-authority/legacy-v1.json"
    ))
    .unwrap();
    for case in cases {
        let vink = app();
        let document = DocumentData {
            schema_version: SCHEMA_VERSION,
            blocks: case
                .blocks
                .iter()
                .map(|lines| BlockData {
                    lines: lines.clone(),
                })
                .collect(),
        };
        let result = vink
            .save_draft(
                Some(actor()),
                SaveDraft {
                    title: case.title,
                    document,
                    ..command()
                },
            )
            .await;
        let saves = vink.store().saves.lock().unwrap();
        if let Some(error) = case.expected_error {
            let error = match error.as_str() {
                "too_long" => TitleError::TooLong,
                "forbidden_character" => TitleError::ForbiddenCharacter,
                _ => panic!("unknown fixture error: {error}"),
            };
            assert_eq!(
                result,
                Err(DraftError::Invalid(InvalidInput::Title(error))),
                "{}",
                case.id
            );
            assert!(saves.is_empty(), "{} must not write", case.id);
            assert!(case.expected_title.is_none());
            continue;
        }
        assert!(result.is_ok(), "{}: {result:?}", case.id);
        assert_eq!(saves.len(), 1, "{}", case.id);
        let saved = &saves[0];
        let expected_title = case.expected_title.expect("successful case needs a title");
        assert_eq!(saved.title, expected_title, "{}", case.id);
        assert_eq!(saved.owner, actor());
        assert_eq!(saved.work_id, work());
        assert_eq!(saved.expected_version, 1);
        let expected_body = DocumentData {
            schema_version: SCHEMA_VERSION,
            blocks: case
                .blocks
                .iter()
                .map(|lines| BlockData {
                    lines: lines.iter().map(|line| line.nfc().collect()).collect(),
                })
                .collect(),
        };
        assert_eq!(saved.document, expected_body, "{}", case.id);
        let revision = RevisionContent::new(
            Title::parse(&saved.title).unwrap(),
            Document::parse(saved.document.clone()).unwrap(),
        );
        if expected_title.is_empty() {
            // A body heading cannot bypass publication's separate title requirement.
            assert_eq!(revision, Err(RevisionError::MissingTitle), "{}", case.id);
        } else {
            let revision = revision.unwrap();
            assert_eq!(revision.title().as_str(), expected_title, "{}", case.id);
            if let Some(hash) = case.expected_v1_hash {
                assert_eq!(revision.hash().as_str(), hash, "{}", case.id);
            }
        }
    }
}
