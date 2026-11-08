//! Application wiring of public discovery (TAG-PR-04, #98). `Scripted` returns fixed facts and
//! records each request; `NoStore` panics on any call. They prove validation order, the use of
//! the real pure rules (slug resolution, `filter_tags`, `public_tags`, scope, page cut) and
//! error mapping. They prove nothing about SQL visibility, snapshots or plans: the PostgreSQL
//! tests (`tests/postgres/discovery.rs`) do.
use std::sync::Mutex;

use time::macros::datetime;
use uuid::Uuid;

use super::catalog::TagResolution;
use super::discovery::{AppliedFilter, DiscoverWorks, DiscoveryError, DiscoveryInput, WorkPage};
use super::ports::{
    DiscoveryFacts, DiscoveryRequest, PublicCardFacts, PublicationFacts, PublicationStore,
    SlugFacts, StoreError,
};
use super::*;
use crate::domain::discovery::{
    CursorError, DiscoveryCursor, FilterError, PagePosition, PageSizeError, SearchScope, WorkQuery,
    WorkQueryError,
};
use crate::domain::ids::TagId;
use crate::domain::taxonomy::{
    CatalogTag, MetadataSnapshot, SlugHolder, SlugKind, TagFacet, TagLabel, TagSlug, TagStatus,
    WorkTags,
};

fn tag_id(n: u128) -> TagId {
    TagId::from_uuid(Uuid::from_u128(n))
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

fn tag(n: u128) -> CatalogTag {
    catalog().into_iter().find(|t| t.id() == tag_id(n)).unwrap()
}

fn work(n: u128) -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(
        0x3333_0000_0000_4000_8000_0000_0000_0000 | n,
    ))
}

const NEWEST: OffsetDateTime = datetime!(2026-11-08 12:00:00 UTC);

/// Works newest first, one minute apart.
fn card(n: u128, snapshot: Option<MetadataSnapshot>) -> PublicCardFacts {
    PublicCardFacts {
        work_id: work(n),
        title: Title::parse(&format!("Bài {n}")).unwrap(),
        author_display_name: "Tác giả".into(),
        published_at: NEWEST - time::Duration::minutes(n as i64),
        snapshot,
    }
}

fn snapshot(form: Option<u128>, themes: &[u128]) -> MetadataSnapshot {
    MetadataSnapshot::capture(
        WorkTags::restore(
            form.map(tag_id),
            themes.iter().copied().map(tag_id).collect(),
        )
        .unwrap(),
    )
}

fn canonical(n: u128) -> SlugFacts {
    let tag = tag(n);
    SlugFacts {
        holder: Some(SlugHolder {
            tag: tag.id(),
            kind: SlugKind::Canonical,
        }),
        tag: Some(tag),
        successor: None,
    }
}

fn alias_of(n: u128) -> SlugFacts {
    SlugFacts {
        holder: Some(SlugHolder {
            tag: tag_id(n),
            kind: SlugKind::Alias,
        }),
        ..canonical(n)
    }
}

fn retired_with_successor() -> SlugFacts {
    SlugFacts {
        successor: Some(tag(MUA)),
        ..canonical(DO_THI)
    }
}

fn unknown() -> SlugFacts {
    SlugFacts {
        holder: None,
        tag: None,
        successor: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    query: Option<String>,
    slugs: Vec<String>,
    after: Option<PagePosition>,
    fetch: usize,
}

struct Scripted {
    answer: Result<DiscoveryFacts, StoreError>,
    seen: Mutex<Vec<Seen>>,
}

impl Scripted {
    fn new(answer: Result<DiscoveryFacts, StoreError>) -> Vink<Self> {
        Vink::new(Self {
            answer,
            seen: Mutex::new(Vec::new()),
        })
    }

    fn facts(filters: Vec<SlugFacts>, works: Vec<PublicCardFacts>) -> Vink<Self> {
        Self::new(Ok(DiscoveryFacts {
            filters,
            works,
            catalog: catalog(),
        }))
    }
}

impl PublicationStore for Scripted {
    async fn discover_works(
        &self,
        request: &DiscoveryRequest<'_>,
    ) -> Result<DiscoveryFacts, StoreError> {
        self.seen.lock().unwrap().push(Seen {
            query: request.query.map(|q| q.key().as_str().to_owned()),
            slugs: request
                .filter
                .slugs()
                .iter()
                .map(|s| s.as_str().to_owned())
                .collect(),
            after: request.after,
            fetch: request.size.fetch(),
        });
        self.answer.clone()
    }

    async fn publication(&self, _: WorkId) -> Result<Option<PublicationFacts>, StoreError> {
        panic!("discovery must not read a single publication")
    }
}

struct NoStore;

impl PublicationStore for NoStore {
    async fn discover_works(&self, _: &DiscoveryRequest<'_>) -> Result<DiscoveryFacts, StoreError> {
        panic!("invalid input must not reach the store")
    }

    async fn publication(&self, _: WorkId) -> Result<Option<PublicationFacts>, StoreError> {
        panic!("invalid input must not reach the store")
    }
}

fn search(
    q: Option<&str>,
    tags: &[&str],
    limit: Option<&str>,
    cursor: Option<&str>,
) -> DiscoverWorks {
    DiscoverWorks {
        query: q.map(Into::into),
        tags: tags.iter().map(|t| (*t).into()).collect(),
        limit: limit.map(Into::into),
        cursor: cursor.map(Into::into),
    }
}

fn ids(page: &WorkPage) -> Vec<WorkId> {
    page.works.iter().map(|w| w.work_id).collect()
}

#[tokio::test]
async fn invalid_input_is_refused_before_any_read_in_a_fixed_order() {
    let vink = Vink::new(NoStore);
    let too_many = ["a", "b", "c", "d", "e"];
    let cases: Vec<(DiscoverWorks, DiscoveryInput)> = vec![
        (
            search(Some("!!!"), &["ký--ức"], Some("0"), Some("zz")),
            DiscoveryInput::Query(WorkQueryError::NoLetterOrDigit),
        ),
        (
            search(Some(&"a".repeat(101)), &[], None, None),
            DiscoveryInput::Query(WorkQueryError::TooLong),
        ),
        (
            search(Some("mưa"), &["ký--ức"], Some("0"), Some("zz")),
            DiscoveryInput::Tag(FilterError::Malformed),
        ),
        (
            search(None, &too_many, None, None),
            DiscoveryInput::Tag(FilterError::TooMany),
        ),
        (
            search(None, &["ký-ức"], Some("0"), Some("zz")),
            DiscoveryInput::Limit(PageSizeError::OutOfRange),
        ),
        (
            search(None, &[], Some("25"), None),
            DiscoveryInput::Limit(PageSizeError::OutOfRange),
        ),
        (
            search(None, &[], Some("12.0"), None),
            DiscoveryInput::Limit(PageSizeError::OutOfRange),
        ),
        (
            search(None, &[], Some(" 3"), None),
            DiscoveryInput::Limit(PageSizeError::OutOfRange),
        ),
        (
            search(None, &[], Some("3"), Some("zz")),
            DiscoveryInput::Cursor(CursorError::Malformed),
        ),
    ];
    for (input, expected) in cases {
        let field = expected.field();
        assert_eq!(
            vink.discover_works(input.clone()).await,
            Err(DiscoveryError::Invalid(expected)),
            "{input:?}"
        );
        assert!(["q", "tag", "limit", "cursor"].contains(&field));
    }
}

#[tokio::test]
async fn blank_parameters_are_absent_and_the_default_page_is_the_home_page() {
    let vink = Scripted::facts(vec![], vec![card(1, None)]);
    let page = vink
        .discover_works(search(Some("  "), &["", " "], Some(""), Some(" ")))
        .await
        .unwrap();
    assert_eq!(page.query, None);
    assert!(page.filters.is_empty());
    assert_eq!(ids(&page), [work(1)]);
    assert!(page.next.is_none());
    assert!(page.works[0].tags.is_empty(), "published without metadata");
    let seen = vink.store().seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        [Seen {
            query: None,
            slugs: vec![],
            after: None,
            fetch: 13,
        }]
    );
}

#[tokio::test]
async fn a_page_is_cut_with_a_cursor_bound_to_the_canonical_search() {
    let rows: Vec<PublicCardFacts> = (1..=4)
        .map(|n| card(n, Some(snapshot(None, &[KY_UC]))))
        .collect();
    let vink = Scripted::facts(vec![alias_of(KY_UC)], rows.clone());
    let page = vink
        .discover_works(search(Some("Mưa"), &["hồi-ức"], Some("3"), None))
        .await
        .unwrap();
    assert_eq!(ids(&page), [work(1), work(2), work(3)]);
    let next = page.next.expect("a fourth row was read");
    assert_eq!(
        next.after(),
        PagePosition {
            published_at: rows[2].published_at,
            work_id: work(3)
        }
    );
    let key = WorkQuery::parse("mua").unwrap().unwrap();
    assert_eq!(
        next.scope(),
        SearchScope::of(Some(key.key()), &[tag_id(KY_UC)]),
        "alias resolved before the scope; accents folded"
    );
    assert_eq!(
        page.filters,
        [AppliedFilter {
            requested: TagSlug::parse("hồi-ức").unwrap(),
            resolution: Some(TagResolution::Redirect(tag(KY_UC))),
        }]
    );
    assert_eq!(
        page.query.as_deref(),
        Some("Mưa"),
        "query text kept as typed"
    );

    // The canonical slug and another spelling of the query continue the same list.
    let wire = next.encode();
    let vink = Scripted::facts(vec![canonical(KY_UC)], rows[3..].to_vec());
    let page = vink
        .discover_works(search(Some("MUA"), &["ký-ức"], Some("3"), Some(&wire)))
        .await
        .unwrap();
    assert_eq!(ids(&page), [work(4)]);
    assert!(page.next.is_none());
    assert_eq!(
        vink.store().seen.lock().unwrap()[0].after,
        Some(next.after())
    );

    // Another query or another tag set refuses the cursor instead of applying it elsewhere.
    for (q, tags, filters) in [
        (Some("mưa hè"), vec!["ký-ức"], vec![canonical(KY_UC)]),
        (Some("mưa"), vec![], vec![]),
        (None, vec!["ký-ức"], vec![canonical(KY_UC)]),
        (
            Some("mưa"),
            vec!["ký-ức", "gia-đình"],
            vec![canonical(KY_UC), canonical(GIA_DINH)],
        ),
    ] {
        let vink = Scripted::facts(filters, rows[3..].to_vec());
        assert_eq!(
            vink.discover_works(search(q, &tags, Some("3"), Some(&wire)))
                .await,
            Err(DiscoveryError::Invalid(DiscoveryInput::CursorScope)),
            "{q:?} {tags:?}"
        );
    }
}

#[tokio::test]
async fn unknown_or_retired_filters_match_nothing_and_say_why() {
    for (facts, expected) in [
        (unknown(), None),
        (
            retired_with_successor(),
            Some(TagResolution::Retired {
                tag: tag(DO_THI),
                successor: Some(tag(MUA)),
            }),
        ),
    ] {
        let vink = Scripted::facts(vec![canonical(KY_UC), facts], vec![]);
        let cursor = DiscoveryCursor::new(
            PagePosition {
                published_at: NEWEST,
                work_id: work(9),
            },
            SearchScope::of(None, &[]),
        )
        .encode();
        // A cursor cannot make an impossible filter match; the page is empty and final.
        let page = vink
            .discover_works(search(None, &["ký-ức", "đô-thị"], None, Some(&cursor)))
            .await
            .unwrap();
        assert!(page.works.is_empty() && page.next.is_none());
        assert_eq!(page.filters[1].resolution, expected);
        assert_eq!(
            page.filters[0].resolution,
            Some(TagResolution::Canonical(tag(KY_UC)))
        );
    }
    // Rows for an impossible filter mean the adapter disagreed with the rule.
    let vink = Scripted::facts(vec![unknown()], vec![card(1, None)]);
    assert_eq!(
        vink.discover_works(search(None, &["không-có"], None, None))
            .await,
        Err(DiscoveryError::Internal)
    );
}

#[tokio::test]
async fn cards_show_active_tags_in_snapshot_order_and_corruption_is_not_empty() {
    let vink = Scripted::facts(
        vec![],
        vec![card(
            1,
            Some(snapshot(Some(TAN_VAN), &[GIA_DINH, DO_THI, KY_UC])),
        )],
    );
    let page = vink.discover_works(DiscoverWorks::default()).await.unwrap();
    let shown: Vec<TagId> = page.works[0].tags.iter().map(CatalogTag::id).collect();
    assert_eq!(
        shown,
        [tag_id(TAN_VAN), tag_id(GIA_DINH), tag_id(KY_UC)],
        "retired Đô thị stays in the snapshot but is not shown"
    );

    for broken in [
        // A snapshot tag without its catalog row.
        DiscoveryFacts {
            filters: vec![],
            works: vec![card(1, Some(snapshot(None, &[99])))],
            catalog: catalog(),
        },
        // One slug fact for two requested slugs.
        DiscoveryFacts {
            filters: vec![canonical(KY_UC)],
            works: vec![],
            catalog: catalog(),
        },
        // A holder that names another tag than the row read for it.
        DiscoveryFacts {
            filters: vec![
                SlugFacts {
                    tag: Some(tag(MUA)),
                    ..canonical(KY_UC)
                },
                canonical(GIA_DINH),
            ],
            works: vec![],
            catalog: catalog(),
        },
    ] {
        let tags = if broken.filters.is_empty() {
            vec![]
        } else {
            vec!["ký-ức", "gia-đình"]
        };
        let vink = Scripted::new(Ok(broken));
        assert_eq!(
            vink.discover_works(search(None, &tags, None, None)).await,
            Err(DiscoveryError::Internal)
        );
    }
}

#[tokio::test]
async fn outages_and_integrity_errors_are_never_an_empty_page() {
    for (error, expected) in [
        (
            StoreError::Unavailable("down".into()),
            DiscoveryError::Unavailable,
        ),
        (
            StoreError::Integrity("bad".into()),
            DiscoveryError::Internal,
        ),
    ] {
        let vink = Scripted::new(Err(error));
        assert_eq!(
            vink.discover_works(DiscoverWorks::default()).await,
            Err(expected)
        );
    }
}
