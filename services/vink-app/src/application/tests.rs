use std::sync::Mutex;

use time::macros::datetime;

use super::*;
use crate::domain::document::{MAX_DOCUMENT_CHARS, MAX_SOURCE_BYTES, MAX_TITLE_CHARS};

mod title_authority;

const AT: OffsetDateTime = datetime!(2026-11-02 12:00:00 UTC);

#[derive(Debug, PartialEq, Eq)]
struct ObservedSave {
    owner: PrincipalId,
    work_id: WorkId,
    expected_version: i64,
    title: String,
    document: DocumentData,
}

struct RecordingStore {
    saves: Mutex<Vec<ObservedSave>>,
    outcome: Result<SaveOutcome, StoreError>,
}

impl RecordingStore {
    fn new(outcome: Result<SaveOutcome, StoreError>) -> Self {
        Self {
            saves: Mutex::new(Vec::new()),
            outcome,
        }
    }
}

impl DraftStore for RecordingStore {
    async fn search_drafts(
        &self,
        _: PrincipalId,
        _: &crate::domain::draft_query::DraftSearch,
    ) -> Result<Vec<DraftSummary>, StoreError> {
        panic!("save must not list drafts")
    }

    async fn create_draft(&self, _: PrincipalId, _: WorkId) -> Result<WorkingDraft, StoreError> {
        panic!("save must not create a draft or revision")
    }

    async fn load_draft(
        &self,
        _: PrincipalId,
        _: WorkId,
    ) -> Result<Option<WorkingDraft>, StoreError> {
        panic!("save must not pre-read authority/version")
    }

    async fn save_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
        change: &DraftChange,
    ) -> Result<SaveOutcome, StoreError> {
        self.saves.lock().unwrap().push(ObservedSave {
            owner,
            work_id,
            expected_version: change.expected_version().get(),
            title: change.content().title().as_str().into(),
            document: change.content().document().to_data(),
        });
        self.outcome.clone()
    }
}

fn actor() -> PrincipalId {
    PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap()
}

fn work() -> WorkId {
    WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap()
}

fn data(format: &str, source: &str) -> DocumentData {
    DocumentData {
        format: format.into(),
        source: source.into(),
    }
}

/// NFD on purpose: the title projection is NFC, the source stays exactly as authored.
const BODY: &str = "Đe\u{0302}m xuống,\r\n  đợi trăng\n\n\n   \n\tđường xa.\n";

fn command() -> SaveDraft {
    SaveDraft {
        work_id: work(),
        expected_version: 1,
        title: Some("  Vie\u{0323}\u{0302}t Đ/đ  ".into()),
        document: data("default", BODY),
    }
}

fn app() -> Vink<RecordingStore> {
    Vink::new(RecordingStore::new(Ok(SaveOutcome::Saved {
        version: DraftVersion::new(2).unwrap(),
        updated_at: AT,
    })))
}

#[tokio::test]
async fn save_passes_one_validated_change_to_the_store() {
    let vink = app();
    assert_eq!(
        vink.save_draft(Some(actor()), command()).await,
        Ok(SavedDraft {
            work_id: work(),
            version: DraftVersion::new(2).unwrap(),
            updated_at: AT,
        })
    );
    assert_eq!(
        *vink.store().saves.lock().unwrap(),
        vec![ObservedSave {
            owner: actor(),
            work_id: work(),
            expected_version: 1,
            title: "Việt Đ/đ".into(),
            document: data("default", BODY),
        }]
    );
}

#[tokio::test]
async fn markdown_title_is_derived_by_the_server_and_never_written_by_the_client() {
    let vink = app();
    let source = "Lời đề.\n\n# *Mưa* trên phố\n\n# Tên sau\n";
    let markdown = SaveDraft {
        title: None,
        document: data("markdown", source),
        ..command()
    };
    assert!(
        vink.save_draft(Some(actor()), markdown.clone())
            .await
            .is_ok()
    );
    {
        let saves = vink.store().saves.lock().unwrap();
        assert_eq!(saves[0].title, "Mưa trên phố");
        assert_eq!(saves[0].document, data("markdown", source));
    }

    let vink = app();
    for title in [Some("Mưa trên phố".to_string()), Some(String::new())] {
        assert_eq!(
            vink.save_draft(
                Some(actor()),
                SaveDraft {
                    title,
                    ..markdown.clone()
                }
            )
            .await,
            Err(DraftError::Invalid(InvalidInput::TitleAuthority(
                TitleAuthorityError::TitleNotWritable
            )))
        );
    }
    assert_eq!(
        vink.save_draft(
            Some(actor()),
            SaveDraft {
                title: None,
                ..command()
            }
        )
        .await,
        Err(DraftError::Invalid(InvalidInput::TitleAuthority(
            TitleAuthorityError::MissingTitle
        )))
    );
    // An overlong first H1 is refused, not truncated; the editor keeps the source.
    assert_eq!(
        vink.save_draft(
            Some(actor()),
            SaveDraft {
                title: None,
                document: data(
                    "markdown",
                    &format!("# {}", "ơ".repeat(MAX_TITLE_CHARS + 1))
                ),
                ..command()
            }
        )
        .await,
        Err(DraftError::Invalid(InvalidInput::TitleAuthority(
            TitleAuthorityError::MarkdownHeading(TitleError::TooLong)
        )))
    );
    assert!(vink.store().saves.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invalid_raw_input_never_reaches_the_store() {
    let mut cases = Vec::new();
    for version in [0, -1] {
        let mut input = command();
        input.expected_version = version;
        cases.push((input, InvalidInput::ExpectedVersion));
    }
    for (raw, error) in [
        ("a\nb".into(), TitleError::ForbiddenCharacter),
        ("ư".repeat(MAX_TITLE_CHARS + 1), TitleError::TooLong),
    ] {
        let mut input = command();
        input.title = Some(raw);
        cases.push((input, InvalidInput::Title(error)));
    }
    for (raw, error) in [
        (
            data("default", "Đêm\nxuống\u{0}"),
            DocumentError::ForbiddenCharacter { line: 1 },
        ),
        (
            data("default", &"ư".repeat(MAX_DOCUMENT_CHARS + 1)),
            DocumentError::TooLong,
        ),
        (
            data("default", &" ".repeat(MAX_SOURCE_BYTES + 1)),
            DocumentError::TooManyBytes,
        ),
        (
            data("asciidoc", "= Tên"),
            DocumentError::DeferredFormat("asciidoc".into()),
        ),
        (
            // The untrusted wire shape deserializes successfully; that does not validate it.
            serde_json::from_str(r#"{"format":"html","source":"<b>x</b>"}"#).unwrap(),
            DocumentError::UnsupportedFormat("html".into()),
        ),
    ] {
        let mut input = command();
        input.document = raw;
        cases.push((input, InvalidInput::Document(error)));
    }
    let vink = app();
    for (input, expected) in cases {
        assert_eq!(
            vink.save_draft(Some(actor()), input).await,
            Err(DraftError::Invalid(expected))
        );
    }
    assert!(vink.store().saves.lock().unwrap().is_empty());
}

#[tokio::test]
async fn authentication_and_validation_precedence_are_unchanged() {
    let vink = app();
    let mut input = command();
    input.expected_version = 0;
    input.title = Some("a\nb".into());
    input.document.format = "markdown".into();
    assert_eq!(
        vink.save_draft(None, input.clone()).await,
        Err(DraftError::Unauthenticated)
    );
    assert_eq!(
        vink.save_draft(Some(actor()), input.clone()).await,
        Err(DraftError::Invalid(InvalidInput::ExpectedVersion))
    );
    input.expected_version = 1;
    // The format decides title authority, so the document is validated before the title.
    assert_eq!(
        vink.save_draft(Some(actor()), input.clone()).await,
        Err(DraftError::Invalid(InvalidInput::TitleAuthority(
            TitleAuthorityError::TitleNotWritable
        )))
    );
    input.document.format = "default".into();
    assert_eq!(
        vink.save_draft(Some(actor()), input).await,
        Err(DraftError::Invalid(InvalidInput::Title(
            TitleError::ForbiddenCharacter
        )))
    );
    assert!(vink.store().saves.lock().unwrap().is_empty());
}

#[tokio::test]
async fn store_outcomes_remain_distinct() {
    for (outcome, expected) in [
        (Ok(SaveOutcome::NotFound), DraftError::NotFound),
        (
            Ok(SaveOutcome::Conflict {
                current: DraftVersion::new(7).unwrap(),
            }),
            DraftError::VersionConflict {
                current: DraftVersion::new(7).unwrap(),
            },
        ),
        (
            Err(StoreError::UnknownPrincipal),
            DraftError::Unauthenticated,
        ),
        (
            Err(StoreError::Unavailable("test outage".into())),
            DraftError::Unavailable,
        ),
        (
            Err(StoreError::Integrity("test corruption".into())),
            DraftError::Internal,
        ),
    ] {
        let vink = Vink::new(RecordingStore::new(outcome));
        assert_eq!(
            vink.save_draft(Some(actor()), command()).await,
            Err(expected)
        );
        assert_eq!(vink.store().saves.lock().unwrap().len(), 1);
    }
}
