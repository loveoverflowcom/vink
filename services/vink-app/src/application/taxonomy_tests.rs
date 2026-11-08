//! Application wiring for draft tags. `FactStore` behaves like the contract of `WorkTagStore`:
//! it holds deterministic facts and calls the REAL `decide_tag_change`, so the policy is never
//! mocked; `NoStore` panics on any call. Neither establishes SQL locking or atomicity (#96).
use std::sync::Mutex;

use uuid::Uuid;

use super::ports::{StoreError, StoredWorkTags, WorkTagStore};
use super::taxonomy::{SaveWorkTags, TagError, TagInput, TagRejection};
use super::*;
use crate::domain::ids::TagId;
use crate::domain::principal::Standing;
use crate::domain::taxonomy::{
    CatalogTag, MetadataVersion, SelectionError, TagChange, TagFacet, TagLabel, TagOutcome,
    TagStatus, WorkTagFacts, WorkTags, decide_tag_change,
};

fn tag_id(n: u128) -> TagId {
    TagId::from_uuid(Uuid::from_u128(n))
}

fn raw(n: u128) -> String {
    tag_id(n).to_string()
}

fn row(n: u128, facet: TagFacet, label: &str, status: TagStatus) -> CatalogTag {
    let label = TagLabel::parse(label).unwrap();
    CatalogTag::restore(tag_id(n), facet, label.clone(), label.slug(), None, status).unwrap()
}

const TAN_VAN: u128 = 1;
const KY_UC: u128 = 11;
const GIA_DINH: u128 = 12;
const DO_THI: u128 = 13;
const MUA: u128 = 14;

fn catalog() -> Vec<CatalogTag> {
    vec![
        row(TAN_VAN, TagFacet::Form, "Tản văn", TagStatus::Active),
        row(KY_UC, TagFacet::Theme, "Ký ức", TagStatus::Active),
        row(GIA_DINH, TagFacet::Theme, "Gia đình", TagStatus::Active),
        row(
            DO_THI,
            TagFacet::Theme,
            "Đô thị",
            TagStatus::Retired {
                successor: Some(tag_id(MUA)),
            },
        ),
        row(MUA, TagFacet::Theme, "Mùa", TagStatus::Active),
    ]
}

fn owner() -> PrincipalId {
    PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap()
}

fn stranger() -> PrincipalId {
    PrincipalId::parse("22222222-2222-4222-8222-222222222222").unwrap()
}

fn work() -> WorkId {
    WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedSave {
    actor: PrincipalId,
    work_id: WorkId,
    change: TagChange,
}

/// The stored draft metadata of one work, its owner's standing and the catalog.
#[derive(Debug, Clone)]
struct Stored {
    owner: PrincipalId,
    standing: Standing,
    version: MetadataVersion,
    tags: WorkTags,
    catalog: Vec<CatalogTag>,
}

struct FactStore {
    state: Mutex<Result<Stored, StoreError>>,
    saves: Mutex<Vec<ObservedSave>>,
}

impl FactStore {
    fn new(state: Result<Stored, StoreError>) -> Self {
        Self {
            state: Mutex::new(state),
            saves: Mutex::new(Vec::new()),
        }
    }

    fn at(version: i64, tags: WorkTags) -> Self {
        Self::new(Ok(Stored {
            owner: owner(),
            standing: Standing::Active,
            version: MetadataVersion::new(version).unwrap(),
            tags,
            catalog: catalog(),
        }))
    }

    fn stored(&self) -> Stored {
        self.state.lock().unwrap().clone().unwrap()
    }
}

impl WorkTagStore for FactStore {
    async fn work_tags(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> Result<Option<StoredWorkTags>, StoreError> {
        let state = self.state.lock().unwrap().clone()?;
        if work_id != work() || owner != state.owner {
            return Ok(None);
        }
        let catalog = state
            .catalog
            .iter()
            .filter(|t| state.tags.contains(t.id()))
            .cloned()
            .collect();
        Ok(Some(StoredWorkTags {
            version: state.version,
            tags: state.tags,
            catalog,
        }))
    }

    async fn save_work_tags(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        change: &TagChange,
    ) -> Result<TagOutcome, StoreError> {
        self.saves.lock().unwrap().push(ObservedSave {
            actor,
            work_id,
            change: change.clone(),
        });
        let mut guard = self.state.lock().unwrap();
        let state = guard.as_mut().map_err(|e| e.clone())?;
        let facts = (work_id == work()).then(|| WorkTagFacts {
            owner: state.owner,
            // The actor's standing; only the owner's is scripted here.
            standing: if actor == state.owner {
                state.standing
            } else {
                Standing::Active
            },
            version: state.version,
            current: state.tags.clone(),
            catalog: state.catalog.iter().map(CatalogTag::facts).collect(),
        });
        let outcome = decide_tag_change(actor, change, facts.as_ref());
        if let TagOutcome::Accepted(write) = &outcome {
            state.version = write.version();
            state.tags = write.tags().clone();
        }
        Ok(outcome)
    }
}

/// Any call is a failure: input that must be refused before I/O.
struct NoStore;

impl WorkTagStore for NoStore {
    async fn work_tags(
        &self,
        _: PrincipalId,
        _: WorkId,
    ) -> Result<Option<StoredWorkTags>, StoreError> {
        panic!("unauthenticated reads must not reach the store")
    }

    async fn save_work_tags(
        &self,
        _: PrincipalId,
        _: WorkId,
        _: &TagChange,
    ) -> Result<TagOutcome, StoreError> {
        panic!("unauthenticated or invalid saves must not reach the store")
    }
}

fn save(expected: i64, form: Option<u128>, themes: &[u128]) -> SaveWorkTags {
    SaveWorkTags {
        work_id: work(),
        expected_version: expected,
        form: form.map(raw),
        themes: themes.iter().map(|n| raw(*n)).collect(),
    }
}

fn tags(form: Option<u128>, themes: &[u128]) -> WorkTags {
    WorkTags::restore(
        form.map(tag_id),
        themes.iter().map(|n| tag_id(*n)).collect(),
    )
    .unwrap()
}

#[tokio::test]
async fn unauthenticated_and_invalid_input_never_reach_the_store() {
    let vink = Vink::new(NoStore);
    let mut everything_wrong = save(0, None, &[]);
    everything_wrong.form = Some("thơ".into());
    assert_eq!(
        vink.save_work_tags(None, everything_wrong.clone()).await,
        Err(TagError::Unauthenticated)
    );
    assert_eq!(
        vink.work_tags(None, work()).await,
        Err(TagError::Unauthenticated)
    );
    let mut cases = vec![(everything_wrong, TagInput::ExpectedVersion)];
    let mut bad_form = save(1, None, &[]);
    bad_form.form = Some("thơ".into());
    cases.push((bad_form, TagInput::Selection(SelectionError::Form)));
    let mut bad_theme = save(1, None, &[KY_UC]);
    bad_theme.themes.push("ký-ức".into());
    cases.push((
        bad_theme,
        TagInput::Selection(SelectionError::Theme { index: 1 }),
    ));
    let mut too_many = save(1, None, &[]);
    too_many.themes = vec!["not-even-parsed".into(); 7];
    cases.push((
        too_many,
        TagInput::Selection(SelectionError::TooManyThemeRefs),
    ));
    for (input, expected) in cases {
        let field = expected.field();
        assert_eq!(
            vink.save_work_tags(Some(owner()), input).await,
            Err(TagError::Invalid(expected)),
            "{field}"
        );
    }
}

#[tokio::test]
async fn accepted_save_passes_one_validated_change_and_reports_the_policy_result() {
    let vink = Vink::new(FactStore::at(1, WorkTags::EMPTY));
    // "Ký ức" sent twice (picked directly and through an alias) is one theme.
    let saved = vink
        .save_work_tags(
            Some(owner()),
            save(1, Some(TAN_VAN), &[KY_UC, GIA_DINH, KY_UC]),
        )
        .await
        .unwrap();
    assert_eq!(saved.version.get(), 2);
    assert!(saved.changed);
    assert_eq!(saved.tags, tags(Some(TAN_VAN), &[KY_UC, GIA_DINH]));
    let saves = vink.store().saves.lock().unwrap().clone();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].actor, owner());
    assert_eq!(saves[0].change.expected(), MetadataVersion::FIRST);
    assert_eq!(
        saves[0].change.selection().themes(),
        [tag_id(KY_UC), tag_id(GIA_DINH)]
    );
    let view = vink.work_tags(Some(owner()), work()).await.unwrap();
    let labels: Vec<&str> = view.shown.iter().map(|t| t.label().as_str()).collect();
    assert_eq!(labels, ["Tản văn", "Ký ức", "Gia đình"]);
}

#[tokio::test]
async fn refusals_stay_distinct_and_write_nothing() {
    let start = tags(None, &[DO_THI]);
    for (input, actor, expected) in [
        (
            save(4, None, &[999]),
            owner(),
            TagError::Rejected(TagRejection::UnknownTag(tag_id(999))),
        ),
        (
            save(4, None, &[MUA, GIA_DINH, KY_UC, DO_THI]),
            owner(),
            TagError::Rejected(TagRejection::TooManyThemes),
        ),
        (
            save(4, Some(KY_UC), &[]),
            owner(),
            TagError::Rejected(TagRejection::WrongFacet {
                tag: tag_id(KY_UC),
                slot: TagFacet::Form,
            }),
        ),
        (save(4, None, &[KY_UC]), stranger(), TagError::NotFound),
        (
            SaveWorkTags {
                work_id: WorkId::parse("44444444-4444-4444-8444-444444444444").unwrap(),
                ..save(4, None, &[KY_UC])
            },
            owner(),
            TagError::NotFound,
        ),
    ] {
        let vink = Vink::new(FactStore::at(4, start.clone()));
        assert_eq!(vink.save_work_tags(Some(actor), input).await, Err(expected));
        let stored = vink.store().stored();
        assert_eq!((stored.version.get(), stored.tags), (4, start.clone()));
        assert_eq!(vink.store().saves.lock().unwrap().len(), 1);
    }
    // Retired: kept when already on the draft, refused when newly chosen (successor hint kept).
    let vink = Vink::new(FactStore::at(4, start.clone()));
    assert!(
        vink.save_work_tags(Some(owner()), save(4, None, &[DO_THI, KY_UC]))
            .await
            .is_ok()
    );
    let vink = Vink::new(FactStore::at(4, WorkTags::EMPTY));
    assert_eq!(
        vink.save_work_tags(Some(owner()), save(4, None, &[DO_THI]))
            .await,
        Err(TagError::Rejected(TagRejection::RetiredTag {
            tag: tag_id(DO_THI),
            successor: Some(tag_id(MUA)),
        }))
    );
    let suspended = FactStore::at(4, start);
    suspended.state.lock().unwrap().as_mut().unwrap().standing = Standing::Suspended;
    let vink = Vink::new(suspended);
    assert_eq!(
        vink.save_work_tags(Some(owner()), save(4, None, &[KY_UC]))
            .await,
        Err(TagError::Suspended)
    );
}

#[tokio::test]
async fn stale_tab_conflicts_with_observed_tags_and_lost_ack_is_reconciled_by_state() {
    let vink = Vink::new(FactStore::at(4, tags(None, &[KY_UC])));
    let tab_a = save(4, None, &[KY_UC, GIA_DINH]);
    let tab_b = save(4, None, &[MUA]);
    vink.save_work_tags(Some(owner()), tab_a.clone())
        .await
        .unwrap();
    let conflict = vink.save_work_tags(Some(owner()), tab_b).await;
    assert_eq!(
        conflict,
        Err(TagError::VersionConflict {
            current: MetadataVersion::new(5).unwrap(),
            observed: tags(None, &[KY_UC, GIA_DINH]),
        })
    );
    assert_eq!(vink.store().stored().tags, tags(None, &[KY_UC, GIA_DINH]));
    // Tab A never saw its acknowledgement and resends: a conflict whose observed state shows its
    // desired tags hold. The application does not turn that into "saved by this command".
    let Err(TagError::VersionConflict { observed, .. }) =
        vink.save_work_tags(Some(owner()), tab_a.clone()).await
    else {
        panic!("a resent command must conflict");
    };
    let desired = crate::domain::taxonomy::TagSelection::parse(None, &tab_a.themes).unwrap();
    assert!(observed.matches(&desired));
    assert_eq!(vink.store().stored().version.get(), 5);
}

#[tokio::test]
async fn store_failures_are_not_turned_into_success_or_empty_tags() {
    for (error, expected) in [
        (
            StoreError::Unavailable("outage".into()),
            TagError::Unavailable,
        ),
        (StoreError::UnknownPrincipal, TagError::Unauthenticated),
        (StoreError::Integrity("corrupt".into()), TagError::Internal),
    ] {
        let vink = Vink::new(FactStore::new(Err(error)));
        assert_eq!(
            vink.save_work_tags(Some(owner()), save(1, None, &[KY_UC]))
                .await,
            Err(expected.clone())
        );
        assert_eq!(vink.work_tags(Some(owner()), work()).await, Err(expected));
        assert_eq!(vink.store().saves.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn author_view_shows_retired_tags_hides_foreign_drafts_and_fails_closed() {
    let vink = Vink::new(FactStore::at(2, tags(Some(TAN_VAN), &[DO_THI])));
    let view = vink.work_tags(Some(owner()), work()).await.unwrap();
    assert_eq!(view.version.get(), 2);
    assert_eq!(view.shown.len(), 2);
    assert!(!view.shown[1].status().is_active());
    assert_eq!(
        vink.work_tags(Some(stranger()), work()).await,
        Err(TagError::NotFound)
    );
    // A stored tag without its catalog row is corruption, not an empty list.
    let broken = FactStore::at(2, tags(None, &[KY_UC]));
    broken
        .state
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .catalog
        .clear();
    assert_eq!(
        Vink::new(broken).work_tags(Some(owner()), work()).await,
        Err(TagError::Internal)
    );
}
