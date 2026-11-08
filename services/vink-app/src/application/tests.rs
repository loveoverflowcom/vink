use std::sync::Mutex;

use time::macros::datetime;

use super::*;
use crate::domain::document::{BlockData, MAX_DOCUMENT_CHARS, MAX_TITLE_CHARS, SCHEMA_VERSION};

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
    async fn list_drafts(&self, _: PrincipalId) -> Result<Vec<DraftSummary>, StoreError> {
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
            title: change.title().as_str().into(),
            document: change.document().to_data(),
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

fn data(blocks: &[&[&str]]) -> DocumentData {
    DocumentData {
        schema_version: SCHEMA_VERSION,
        blocks: blocks
            .iter()
            .map(|lines| BlockData {
                lines: lines.iter().map(|line| (*line).into()).collect(),
            })
            .collect(),
    }
}

fn command() -> SaveDraft {
    SaveDraft {
        work_id: work(),
        expected_version: 1,
        title: "  Vie\u{0323}\u{0302}t Đ/đ  ".into(),
        document: data(&[&["Đe\u{0302}m xuống,", "  đợi trăng"], &["đường xa."]]),
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
            document: data(&[&["Đêm xuống,", "  đợi trăng"], &["đường xa."]]),
        }]
    );
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
        input.title = raw;
        cases.push((input, InvalidInput::Title(error)));
    }
    for (raw, error) in [
        (data(&[&[]]), DocumentError::EmptyBlock { block: 0 }),
        (
            data(&[&["Đêm", "  "]]),
            DocumentError::BlankLine { block: 0, line: 1 },
        ),
        (
            data(&[&["Đêm\nxuống"]]),
            DocumentError::ForbiddenCharacter { block: 0, line: 0 },
        ),
        (
            data(&[&[&"ư".repeat(MAX_DOCUMENT_CHARS + 1)]]),
            DocumentError::TooLong,
        ),
        (
            // The untrusted wire shape deserializes successfully; that does not validate it.
            serde_json::from_str(r#"{"schema_version":2,"blocks":[]}"#).unwrap(),
            DocumentError::UnsupportedSchemaVersion(2),
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
    input.title = "a\nb".into();
    input.document.schema_version = 2;
    assert_eq!(
        vink.save_draft(None, input.clone()).await,
        Err(DraftError::Unauthenticated)
    );
    assert_eq!(
        vink.save_draft(Some(actor()), input.clone()).await,
        Err(DraftError::Invalid(InvalidInput::ExpectedVersion))
    );
    input.expected_version = 1;
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
