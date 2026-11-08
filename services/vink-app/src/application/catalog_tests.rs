//! Application wiring for the curated catalog (TAG-PR-03, #97). `Facts` returns a fixed catalog
//! snapshot and counts reads; `Curated` behaves like the contract of `CatalogStore`: it loads the
//! facts the adapter would, calls the REAL `decide_catalog_change` and applies only an accepted
//! write, recording every authority and command; `Scripted` returns a fixed outcome; `NoStore`
//! panics on any call. None of them establishes SQL locking, snapshots, audit atomicity or
//! constraints: the PostgreSQL tests do.
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use uuid::Uuid;

use super::catalog::{
    CatalogError, CatalogInput, CatalogOperation, CatalogQueryInput, CatalogReadError,
    CatalogRejection, CatalogRequest, ChangeCatalog, CuratorAuthority, MAX_LISTED_TAGS, SearchTags,
    TagResolution,
};
use super::ports::{
    ActiveCatalog, CatalogAuditRecord, CatalogListing, CatalogReadStore, CatalogStore, SlugFacts,
    StoreError,
};
use super::*;
use crate::domain::ids::TagId;
use crate::domain::taxonomy::{
    CatalogChange, CatalogCommand, CatalogFacts, CatalogOutcome, CatalogRefusal, CatalogTag,
    CatalogVersion, DescriptionError, LabelError, MatchTier, QueryError, SlugHolder, SlugKind,
    TagAlias, TagFacet, TagLabel, TagSlug, TagStatus, decide_catalog_change,
};

fn tag_id(n: u128) -> TagId {
    TagId::from_uuid(Uuid::from_u128(n))
}

fn row(n: u128, facet: TagFacet, label: &str, status: TagStatus) -> CatalogTag {
    let label = TagLabel::parse(label).unwrap();
    CatalogTag::restore(tag_id(n), facet, label.clone(), label.slug(), None, status).unwrap()
}

fn active(n: u128, facet: TagFacet, label: &str) -> CatalogTag {
    row(n, facet, label, TagStatus::Active)
}

fn alias(label: &str, target: u128) -> TagAlias {
    let label = TagLabel::parse(label).unwrap();
    TagAlias::restore(label.clone(), label.slug(), tag_id(target)).unwrap()
}

const THO: u128 = 1;
const TAN_VAN: u128 = 2;
const KY_UC: u128 = 11;
const MA: u128 = 21;
const MA_SAC: u128 = 22;
const MA_HUYEN: u128 = 23;
const DO_THI: u128 = 31;
const MUA: u128 = 32;

/// An active catalog snapshot as the adapter returns it (unordered on purpose).
fn snapshot() -> ActiveCatalog {
    ActiveCatalog {
        tags: vec![
            active(MA_HUYEN, TagFacet::Theme, "mà"),
            active(KY_UC, TagFacet::Theme, "Ký ức"),
            active(TAN_VAN, TagFacet::Form, "Tản văn"),
            active(MA, TagFacet::Theme, "ma"),
            active(MUA, TagFacet::Theme, "Mùa"),
            active(THO, TagFacet::Form, "Thơ"),
            active(MA_SAC, TagFacet::Theme, "má"),
        ],
        aliases: vec![alias("Hồi ức", KY_UC), alias("Mạ", MA)],
    }
}

struct Facts {
    catalog: Result<ActiveCatalog, StoreError>,
    slug: Result<SlugFacts, StoreError>,
    reads: AtomicUsize,
}

impl Facts {
    fn new(catalog: Result<ActiveCatalog, StoreError>) -> Self {
        Self {
            catalog,
            slug: Ok(SlugFacts {
                holder: None,
                tag: None,
                successor: None,
            }),
            reads: AtomicUsize::new(0),
        }
    }

    fn slug(slug: SlugFacts) -> Self {
        Self {
            slug: Ok(slug),
            ..Self::new(Ok(snapshot()))
        }
    }
}

impl CatalogReadStore for Facts {
    async fn active_catalog(&self) -> Result<ActiveCatalog, StoreError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.catalog.clone()
    }

    async fn slug_facts(&self, _slug: &TagSlug) -> Result<SlugFacts, StoreError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.slug.clone()
    }
}

/// Panics on any call: proves that validation happens before storage.
struct NoStore;

impl CatalogReadStore for NoStore {
    async fn active_catalog(&self) -> Result<ActiveCatalog, StoreError> {
        panic!("invalid input reached the catalog store")
    }

    async fn slug_facts(&self, _slug: &TagSlug) -> Result<SlugFacts, StoreError> {
        panic!("invalid input reached the catalog store")
    }
}

impl CatalogStore for NoStore {
    async fn change_catalog(
        &self,
        _authority: CuratorAuthority,
        _command: &CatalogCommand,
    ) -> Result<CatalogOutcome, StoreError> {
        panic!("invalid input reached the catalog store")
    }

    async fn catalog_listing(&self) -> Result<CatalogListing, StoreError> {
        panic!("invalid input reached the catalog store")
    }

    async fn catalog_history(&self, _limit: i64) -> Result<Vec<CatalogAuditRecord>, StoreError> {
        panic!("invalid input reached the catalog store")
    }
}

fn search(q: Option<&str>, facet: Option<&str>, limit: Option<i64>) -> SearchTags {
    SearchTags {
        query: q.map(str::to_owned),
        facet: facet.map(str::to_owned),
        limit,
    }
}

fn ids(found: &[super::catalog::TagSuggestion]) -> Vec<TagId> {
    found.iter().map(|s| s.tag.id()).collect()
}

#[tokio::test]
async fn lookup_input_is_validated_before_any_read() {
    let vink = Vink::new(NoStore);
    let long = "a".repeat(41);
    for (input, expected) in [
        (search(None, None, None), CatalogQueryInput::MissingQuery),
        (
            search(Some(" - "), None, None),
            CatalogQueryInput::Query(QueryError::Empty),
        ),
        (
            search(Some(&long), None, None),
            CatalogQueryInput::Query(QueryError::TooLong),
        ),
        (
            search(Some("ma"), Some("genre"), None),
            CatalogQueryInput::Facet,
        ),
        (search(Some("ma"), None, Some(0)), CatalogQueryInput::Limit),
        (search(Some("ma"), None, Some(11)), CatalogQueryInput::Limit),
        (search(Some("ma"), None, Some(-1)), CatalogQueryInput::Limit),
    ] {
        let field = expected.field();
        assert_eq!(
            vink.search_tags(input).await,
            Err(CatalogReadError::Invalid(expected)),
            "{field}"
        );
    }
    assert_eq!(
        vink.catalog_tags(Some("Theme")).await,
        Err(CatalogReadError::Invalid(CatalogQueryInput::Facet))
    );
    // A malformed slug cannot name a tag: not found, without a read.
    for raw in ["", "Ký ức", "ký--ức", "-ký", &"a".repeat(41)] {
        assert_eq!(
            vink.resolve_tag(raw).await,
            Err(CatalogReadError::NotFound),
            "{raw:?}"
        );
    }
}

#[tokio::test]
async fn lookup_ranks_active_tags_with_the_core_and_never_merges_marks() {
    let vink = Vink::new(Facts::new(Ok(snapshot())));
    // `ma`: the exact name first, then every accent-folded candidate (alias `Mạ` names `ma`, which
    // is already suggested), never a merge of the five names.
    let found = vink
        .search_tags(search(Some("ma"), None, None))
        .await
        .unwrap();
    assert_eq!(ids(&found)[0], tag_id(MA));
    assert_eq!(found[0].tier, MatchTier::Exact);
    assert_eq!(found[0].via_alias, None);
    assert_eq!(ids(&found)[1..], [tag_id(MA_HUYEN), tag_id(MA_SAC)]);
    assert!(found[1..].iter().all(|s| s.tier == MatchTier::Folded));
    // `MÁ` in NFD is the same name as `má`.
    let found = vink
        .search_tags(search(Some("MA\u{301}"), None, None))
        .await
        .unwrap();
    assert_eq!(
        (ids(&found)[0], found[0].tier),
        (tag_id(MA_SAC), MatchTier::Exact)
    );
    // An alias prefix suggests the canonical tag and names the alias that matched.
    let found = vink
        .search_tags(search(Some("hồi"), None, None))
        .await
        .unwrap();
    assert_eq!(ids(&found), [tag_id(KY_UC)]);
    assert_eq!(found[0].tier, MatchTier::Prefix);
    assert_eq!(
        found[0].via_alias.as_ref().map(TagLabel::as_str),
        Some("Hồi ức")
    );
    // Facet and limit are applied; the bound is the caller's.
    let found = vink
        .search_tags(search(Some("t"), Some("form"), Some(1)))
        .await
        .unwrap();
    assert_eq!(ids(&found), [tag_id(TAN_VAN)]);
    let found = vink
        .search_tags(search(Some("ma"), Some("form"), None))
        .await
        .unwrap();
    assert!(found.is_empty());
    assert_eq!(vink.store().reads.load(Ordering::SeqCst), 5);
    // A retired tag is never suggested, even by its exact name or an alias.
    let mut catalog = snapshot();
    catalog.tags.push(row(
        DO_THI,
        TagFacet::Theme,
        "Đô thị",
        TagStatus::Retired { successor: None },
    ));
    catalog.aliases.push(alias("Phố", DO_THI));
    let vink = Vink::new(Facts::new(Ok(catalog)));
    for q in ["Đô thị", "phố"] {
        assert!(
            vink.search_tags(search(Some(q), None, None))
                .await
                .unwrap()
                .is_empty(),
            "{q}"
        );
    }
}

#[tokio::test]
async fn catalog_list_is_active_only_ordered_and_bounded() {
    let mut catalog = snapshot();
    catalog.tags.push(row(
        DO_THI,
        TagFacet::Theme,
        "Đô thị",
        TagStatus::Retired { successor: None },
    ));
    let vink = Vink::new(Facts::new(Ok(catalog)));
    let page = vink.catalog_tags(None).await.unwrap();
    let labels: Vec<&str> = page.tags.iter().map(|t| t.label().as_str()).collect();
    // Forms first, then by folded label (ma < mà < má by slug), never the retired tag.
    assert_eq!(labels, ["Tản văn", "Thơ", "Ký ức", "ma", "mà", "má", "Mùa"]);
    assert!(!page.truncated);
    let page = vink.catalog_tags(Some("form")).await.unwrap();
    assert_eq!(page.tags.len(), 2);

    let many = (0..=MAX_LISTED_TAGS as u128)
        .map(|n| active(1000 + n, TagFacet::Theme, &format!("Chủ đề {n:03}")))
        .collect();
    let vink = Vink::new(Facts::new(Ok(ActiveCatalog {
        tags: many,
        aliases: Vec::new(),
    })));
    let page = vink.catalog_tags(Some("theme")).await.unwrap();
    assert_eq!(page.tags.len(), MAX_LISTED_TAGS);
    assert!(
        page.truncated,
        "a longer catalog is flagged, never silently cut"
    );
    assert_eq!(page.tags[0].label().as_str(), "Chủ đề 000");
}

fn holder(n: u128, kind: SlugKind) -> Option<SlugHolder> {
    Some(SlugHolder {
        tag: tag_id(n),
        kind,
    })
}

#[tokio::test]
async fn resolution_follows_the_slug_and_fails_closed() {
    let ky_uc = active(KY_UC, TagFacet::Theme, "Ký ức");
    let canonical = Vink::new(Facts::slug(SlugFacts {
        holder: holder(KY_UC, SlugKind::Canonical),
        tag: Some(ky_uc.clone()),
        successor: None,
    }));
    // NFD and uppercase spell the same slug.
    assert_eq!(
        canonical.resolve_tag("KY\u{301}-ư\u{301}c").await,
        Ok(TagResolution::Canonical(ky_uc.clone()))
    );
    let redirect = Vink::new(Facts::slug(SlugFacts {
        holder: holder(KY_UC, SlugKind::Alias),
        tag: Some(ky_uc.clone()),
        successor: None,
    }));
    assert_eq!(
        redirect.resolve_tag("hồi-ức").await,
        Ok(TagResolution::Redirect(ky_uc.clone()))
    );
    let mua = active(MUA, TagFacet::Theme, "Mùa");
    let retired = row(
        DO_THI,
        TagFacet::Theme,
        "Đô thị",
        TagStatus::Retired {
            successor: Some(tag_id(MUA)),
        },
    );
    let vink = Vink::new(Facts::slug(SlugFacts {
        holder: holder(DO_THI, SlugKind::Former),
        tag: Some(retired.clone()),
        successor: Some(mua.clone()),
    }));
    assert_eq!(
        vink.resolve_tag("phố-thị").await,
        Ok(TagResolution::Retired {
            tag: retired.clone(),
            successor: Some(mua.clone()),
        })
    );
    let missing = Vink::new(Facts::slug(SlugFacts {
        holder: None,
        tag: None,
        successor: None,
    }));
    assert_eq!(
        missing.resolve_tag("biển").await,
        Err(CatalogReadError::NotFound)
    );
    // Facts that contradict each other or the hint are corruption, never a guess.
    for (slug, broken) in [
        // The slug row names another tag than the row read.
        (
            "ký-ức",
            SlugFacts {
                holder: holder(MUA, SlugKind::Canonical),
                tag: Some(ky_uc.clone()),
                successor: None,
            },
        ),
        // The hinted successor's row is missing, or is another tag.
        (
            "đô-thị",
            SlugFacts {
                holder: holder(DO_THI, SlugKind::Canonical),
                tag: Some(retired.clone()),
                successor: None,
            },
        ),
        (
            "đô-thị",
            SlugFacts {
                holder: holder(DO_THI, SlugKind::Canonical),
                tag: Some(retired.clone()),
                successor: Some(ky_uc.clone()),
            },
        ),
    ] {
        let vink = Vink::new(Facts::slug(broken));
        assert_eq!(
            vink.resolve_tag(slug).await,
            Err(CatalogReadError::Internal),
            "{slug}"
        );
    }
}

#[tokio::test]
async fn read_failures_are_reported_not_turned_into_an_empty_catalog() {
    for (error, expected) in [
        (
            StoreError::Unavailable("outage".into()),
            CatalogReadError::Unavailable,
        ),
        (
            StoreError::Integrity("corrupt".into()),
            CatalogReadError::Internal,
        ),
    ] {
        let store = Facts {
            slug: Err(error.clone()),
            ..Facts::new(Err(error))
        };
        let vink = Vink::new(store);
        assert_eq!(vink.catalog_tags(None).await, Err(expected.clone()));
        assert_eq!(
            vink.search_tags(search(Some("ma"), None, None)).await,
            Err(expected.clone())
        );
        assert_eq!(vink.resolve_tag("ma").await, Err(expected));
    }
}

fn change(expected: i64, request: CatalogRequest) -> ChangeCatalog {
    ChangeCatalog {
        expected_version: expected,
        request,
    }
}

#[tokio::test]
async fn operator_input_is_validated_before_any_store_call() {
    let tag = tag_id(KY_UC).to_string();
    let label = |text: &str| text.to_owned();
    for (input, expected) in [
        (
            change(
                0,
                CatalogRequest::Create {
                    facet: "theme".into(),
                    label: label("Biển"),
                },
            ),
            CatalogInput::ExpectedVersion,
        ),
        (
            change(
                1,
                CatalogRequest::Create {
                    facet: "genre".into(),
                    label: label("Biển"),
                },
            ),
            CatalogInput::Facet,
        ),
        (
            change(
                1,
                CatalogRequest::Create {
                    facet: "theme".into(),
                    label: label("海"),
                },
            ),
            CatalogInput::Label(LabelError::ForbiddenCharacter),
        ),
        (
            change(
                1,
                CatalogRequest::Rename {
                    tag: "not-a-tag".into(),
                    label: label("Biển"),
                },
            ),
            CatalogInput::Tag,
        ),
        (
            change(
                1,
                CatalogRequest::Describe {
                    tag: tag.clone(),
                    description: Some("hai\ndòng".into()),
                },
            ),
            CatalogInput::Description(DescriptionError::ForbiddenCharacter),
        ),
        (
            change(
                1,
                CatalogRequest::AddAlias {
                    tag: tag.clone(),
                    label: label("   "),
                },
            ),
            CatalogInput::Label(LabelError::Empty),
        ),
        (
            change(
                1,
                CatalogRequest::Retire {
                    tag: tag.clone(),
                    successor: Some("x".into()),
                },
            ),
            CatalogInput::Successor,
        ),
    ] {
        assert_eq!(input.validate(), Err(expected));
    }
    // Labels are normalized by the domain, and a create gets a server-chosen identity.
    let created = change(
        3,
        CatalogRequest::Create {
            facet: "form".into(),
            label: label("  Truyện   ngắn "),
        },
    )
    .validate()
    .unwrap();
    assert_eq!(created.expected.get(), 3);
    let CatalogChange::Create { facet, label, .. } = &created.change else {
        panic!("{created:?}");
    };
    assert_eq!((*facet, label.as_str()), (TagFacet::Form, "Truyện ngắn"));
    assert_eq!(
        Vink::new(NoStore).catalog_history(0).await,
        Err(CatalogError::Invalid(CatalogInput::Limit))
    );
}

/// In-memory catalog with the adapter's fact loading and the real decision.
#[derive(Debug, Clone)]
struct Catalog {
    version: CatalogVersion,
    tags: Vec<CatalogTag>,
    names: Vec<(TagSlug, TagId, SlugKind)>,
}

struct Curated {
    state: Mutex<Result<Catalog, StoreError>>,
    calls: Mutex<Vec<(CuratorAuthority, CatalogCommand)>>,
}

impl Curated {
    fn new(state: Result<Catalog, StoreError>) -> Self {
        Self {
            state: Mutex::new(state),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn seeded() -> Self {
        let tags = vec![
            active(KY_UC, TagFacet::Theme, "Ký ức"),
            active(MUA, TagFacet::Theme, "Mùa"),
        ];
        let names = tags
            .iter()
            .map(|t| (t.slug().clone(), t.id(), SlugKind::Canonical))
            .collect();
        Self::new(Ok(Catalog {
            version: CatalogVersion::new(3).unwrap(),
            tags,
            names,
        }))
    }

    fn version(&self) -> i64 {
        self.state.lock().unwrap().as_ref().unwrap().version.get()
    }
}

impl CatalogStore for Curated {
    async fn change_catalog(
        &self,
        authority: CuratorAuthority,
        command: &CatalogCommand,
    ) -> Result<CatalogOutcome, StoreError> {
        self.calls
            .lock()
            .unwrap()
            .push((authority, command.clone()));
        let mut guard = self.state.lock().unwrap();
        let state = guard.as_mut().map_err(|e| e.clone())?;
        let change = &command.change;
        let find = |id: TagId| state.tags.iter().find(|t| t.id() == id).cloned();
        let facts = CatalogFacts {
            can_curate: authority == CuratorAuthority::operator(),
            version: state.version,
            subject: find(change.subject()),
            slug_holder: change.claimed_slug().and_then(|slug| {
                state
                    .names
                    .iter()
                    .find(|(s, ..)| *s == slug)
                    .map(|(_, tag, kind)| SlugHolder {
                        tag: *tag,
                        kind: *kind,
                    })
            }),
            successor: change.successor().and_then(find).map(|t| t.facts()),
        };
        let outcome = decide_catalog_change(command, &facts);
        if let CatalogOutcome::Accepted(write) = &outcome {
            state.version = write.version();
            if let Some(tag) = write.tag() {
                state.tags.retain(|t| t.id() != tag.id());
                state.tags.push(tag.clone());
            }
            for slug in write.slugs() {
                state.names.retain(|(s, ..)| *s != slug.slug);
                state
                    .names
                    .push((slug.slug.clone(), slug.tag, slug.kind.slug_kind()));
            }
        }
        Ok(outcome)
    }

    async fn catalog_listing(&self) -> Result<CatalogListing, StoreError> {
        unreachable!("not used by these tests")
    }

    async fn catalog_history(&self, _limit: i64) -> Result<Vec<CatalogAuditRecord>, StoreError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn catalog_changes_pass_one_command_and_the_authority_and_map_the_decision() {
    let store = Curated::seeded();
    let vink = Vink::new(store);
    let create = change(
        3,
        CatalogRequest::Create {
            facet: "theme".into(),
            label: "Gia đình".into(),
        },
    )
    .validate()
    .unwrap();
    let created = vink
        .change_catalog(CuratorAuthority::operator(), create.clone())
        .await
        .unwrap();
    assert_eq!(created.operation, CatalogOperation::Create);
    assert_eq!(created.subject, create.change.subject());
    assert_eq!(created.write.version().get(), 4);
    assert_eq!(
        *vink.store().calls.lock().unwrap(),
        [(CuratorAuthority::operator(), create)]
    );
    // A stale version is a conflict naming the current one; nothing is applied.
    let stale = change(
        3,
        CatalogRequest::AddAlias {
            tag: tag_id(KY_UC).to_string(),
            label: "Hồi ức".into(),
        },
    )
    .validate()
    .unwrap();
    assert_eq!(
        vink.change_catalog(CuratorAuthority::operator(), stale)
            .await,
        Err(CatalogError::Conflict {
            current: CatalogVersion::new(4).unwrap()
        })
    );
    assert_eq!(vink.store().version(), 4);
    // A slug that already names another tag (`mùa` is Mùa's) is refused with its holder.
    let taken = change(
        4,
        CatalogRequest::Rename {
            tag: tag_id(KY_UC).to_string(),
            label: "MÙA".into(),
        },
    )
    .validate()
    .unwrap();
    assert_eq!(
        vink.change_catalog(CuratorAuthority::operator(), taken)
            .await,
        Err(CatalogError::Rejected(CatalogRejection::SlugTaken {
            holder: tag_id(MUA)
        }))
    );
    let retire = change(
        4,
        CatalogRequest::Retire {
            tag: tag_id(KY_UC).to_string(),
            successor: Some(tag_id(KY_UC).to_string()),
        },
    )
    .validate()
    .unwrap();
    assert!(matches!(
        vink.change_catalog(CuratorAuthority::operator(), retire)
            .await,
        Err(CatalogError::Rejected(CatalogRejection::InvalidSuccessor(
            _
        )))
    ));
    assert_eq!(vink.store().version(), 4, "refusals apply nothing");
    assert_eq!(vink.store().calls.lock().unwrap().len(), 4);
}

/// Returns one fixed outcome or error.
struct Scripted(Result<CatalogOutcome, StoreError>);

impl CatalogStore for Scripted {
    async fn change_catalog(
        &self,
        _authority: CuratorAuthority,
        _command: &CatalogCommand,
    ) -> Result<CatalogOutcome, StoreError> {
        self.0.clone()
    }

    async fn catalog_listing(&self) -> Result<CatalogListing, StoreError> {
        Err(StoreError::Unavailable("outage".into()))
    }

    async fn catalog_history(&self, _limit: i64) -> Result<Vec<CatalogAuditRecord>, StoreError> {
        Err(StoreError::Integrity("corrupt".into()))
    }
}

#[tokio::test]
async fn store_outcomes_that_are_not_curator_facing_fail_closed() {
    let command = change(
        1,
        CatalogRequest::Describe {
            tag: tag_id(KY_UC).to_string(),
            description: None,
        },
    )
    .validate()
    .unwrap();
    for (scripted, expected) in [
        (
            Ok(CatalogOutcome::Refused(CatalogRefusal::InvalidFacts)),
            CatalogError::Internal,
        ),
        (
            Ok(CatalogOutcome::Refused(CatalogRefusal::VersionExhausted)),
            CatalogError::Internal,
        ),
        (
            Ok(CatalogOutcome::Refused(CatalogRefusal::NotCurator)),
            CatalogError::Rejected(CatalogRejection::NotCurator),
        ),
        (
            Err(StoreError::Unavailable("outage".into())),
            CatalogError::Unavailable,
        ),
        (
            Err(StoreError::Integrity("corrupt".into())),
            CatalogError::Internal,
        ),
    ] {
        let vink = Vink::new(Scripted(scripted));
        assert_eq!(
            vink.change_catalog(CuratorAuthority::operator(), command.clone())
                .await,
            Err(expected)
        );
    }
    let vink = Vink::new(Scripted(Ok(CatalogOutcome::Conflict {
        current: CatalogVersion::FIRST,
    })));
    assert_eq!(vink.catalog_listing().await, Err(CatalogError::Unavailable));
    assert_eq!(vink.catalog_history(10).await, Err(CatalogError::Internal));
}
