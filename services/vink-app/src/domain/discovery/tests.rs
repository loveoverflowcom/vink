use time::macros::datetime;
use unicode_normalization::UnicodeNormalization;

use super::*;
use crate::domain::taxonomy::{CatalogTag, SlugKind, TagFacet, TagLabel, TagStatus, resolve_slug};

fn tag_id(n: u128) -> TagId {
    TagId::from_uuid(uuid::Uuid::from_u128(
        0xa000_0000_0000_4000_8000_0000_0000_0000 | n,
    ))
}

fn work(n: u128) -> WorkId {
    WorkId::from_uuid(uuid::Uuid::from_u128(
        0x3333_0000_0000_4000_8000_0000_0000_0000 | n,
    ))
}

fn catalog_tag(n: u128, label: &str, status: TagStatus) -> CatalogTag {
    let label = TagLabel::parse(label).unwrap();
    let slug = label.slug();
    CatalogTag::restore(tag_id(n), TagFacet::Theme, label, slug, None, status).unwrap()
}

#[test]
fn blank_queries_are_absent_and_text_is_kept_as_typed() {
    for blank in ["", " ", "\t \n"] {
        assert_eq!(WorkQuery::parse(blank), Ok(None), "{blank:?}");
    }
    let typed = "  Mưa trên PHỐ cũ ";
    let query = WorkQuery::parse(typed).unwrap().unwrap();
    assert_eq!(query.text(), typed, "displayed as typed, untrimmed");
    assert_eq!(query.key().as_str(), "mua tren pho cu");
    // NFC, NFD, uppercase and đ/Đ fold to one key; the text keeps each spelling.
    let nfd: String = "Đường về".nfd().collect();
    let a = WorkQuery::parse(&nfd).unwrap().unwrap();
    let b = WorkQuery::parse("ĐƯỜNG VỀ").unwrap().unwrap();
    assert_eq!(a.key(), b.key());
    assert_eq!(a.key().as_str(), "duong ve");
    assert_eq!(a.text(), nfd);
}

#[test]
fn queries_without_letters_or_over_the_bound_are_refused() {
    for punctuation in ["!!!", " - ", "…", "%_"] {
        assert_eq!(
            WorkQuery::parse(punctuation),
            Err(WorkQueryError::NoLetterOrDigit),
            "{punctuation:?}"
        );
    }
    let longest = "ệ".repeat(MAX_QUERY_CHARS);
    assert!(WorkQuery::parse(&longest).unwrap().is_some());
    // Characters are counted after NFC: 100 decomposed letters are still 100 characters.
    let decomposed: String = longest.nfd().collect();
    assert!(decomposed.chars().count() > MAX_QUERY_CHARS);
    assert!(WorkQuery::parse(&decomposed).unwrap().is_some());
    assert_eq!(
        WorkQuery::parse(&"a".repeat(MAX_QUERY_CHARS + 1)),
        Err(WorkQueryError::TooLong)
    );
    assert_eq!(
        WorkQuery::parse(&" ".repeat(MAX_QUERY_BYTES + 1)),
        Err(WorkQueryError::TooLong),
        "the byte bound applies before anything is normalized"
    );
}

#[test]
fn filters_normalize_deduplicate_and_bound_their_slugs() {
    let nfd: String = "Ký-ức".nfd().collect();
    let filter = TagFilter::parse(["gia-đình", &nfd, "", "ký-ức", "KÝ-ỨC", " "]).unwrap();
    let slugs: Vec<&str> = filter.slugs().iter().map(TagSlug::as_str).collect();
    assert_eq!(
        slugs,
        ["gia-đình", "ký-ức"],
        "first-seen order, blanks ignored"
    );
    assert!(TagFilter::parse(std::iter::empty()).unwrap().is_empty());

    for malformed in ["ký--ức", "-mùa", "mùa-", "mùa đông", "a/b", "ký_ức"] {
        assert_eq!(
            TagFilter::parse([malformed]),
            Err(FilterError::Malformed),
            "{malformed}"
        );
    }
    let four = ["a", "b", "c", "d"];
    assert_eq!(
        TagFilter::parse(four).unwrap().slugs().len(),
        MAX_FILTER_TAGS
    );
    assert_eq!(
        TagFilter::parse(["a", "b", "c", "d", "e"]),
        Err(FilterError::TooMany)
    );
    // The raw bound applies before deduplication, so work stays bounded.
    let refs = ["a"; MAX_FILTER_REFS];
    assert_eq!(
        TagFilter::parse(refs.iter().copied())
            .unwrap()
            .slugs()
            .len(),
        1
    );
    let refs = ["a"; MAX_FILTER_REFS + 1];
    assert_eq!(
        TagFilter::parse(refs.iter().copied()),
        Err(FilterError::TooMany)
    );
}

#[test]
fn only_active_tags_constrain_a_search_and_aliases_collapse() {
    let memory = catalog_tag(1, "Ký ức", TagStatus::Active);
    let family = catalog_tag(2, "Gia đình", TagStatus::Active);
    let city = catalog_tag(3, "Đô thị", TagStatus::Retired { successor: None });
    let slug = |raw: &str| TagSlug::parse(raw).unwrap();
    let alias = resolve_slug(
        &slug("hồi-ức"),
        Some(holder(&memory, SlugKind::Alias)),
        Some(&memory),
    )
    .unwrap();
    let unknown = resolve_slug(&slug("không-có"), None, None).unwrap();

    // Independent truth table: (resolutions, expected constraint).
    let cases: Vec<(Vec<SlugResolution<'_>>, Option<Vec<TagId>>)> = vec![
        (vec![], Some(vec![])),
        (vec![canonical(&memory)], Some(vec![tag_id(1)])),
        (vec![alias], Some(vec![tag_id(1)])),
        (vec![canonical(&memory), alias], Some(vec![tag_id(1)])),
        (
            vec![canonical(&family), canonical(&memory)],
            Some(vec![tag_id(1), tag_id(2)]),
        ),
        (vec![canonical(&memory), canonical(&city)], None),
        (vec![unknown, canonical(&memory)], None),
    ];
    for (resolutions, expected) in cases {
        assert_eq!(filter_tags(&resolutions), expected, "{resolutions:?}");
    }
}

fn canonical(tag: &CatalogTag) -> SlugResolution<'_> {
    resolve_slug(
        tag.slug(),
        Some(holder(tag, SlugKind::Canonical)),
        Some(tag),
    )
    .unwrap()
}

fn holder(tag: &CatalogTag, kind: SlugKind) -> crate::domain::taxonomy::SlugHolder {
    crate::domain::taxonomy::SlugHolder {
        tag: tag.id(),
        kind,
    }
}

#[test]
fn page_sizes_are_bounded() {
    assert_eq!(PageSize::DEFAULT.get(), 12);
    assert_eq!(PageSize::new(1).unwrap().get(), 1);
    assert_eq!(PageSize::new(24).unwrap().fetch(), 25);
    for out in [0, -1, 25, i64::MAX, i64::MIN] {
        assert_eq!(PageSize::new(out), Err(PageSizeError::OutOfRange), "{out}");
    }
}

#[test]
fn search_scopes_follow_the_normalized_search_only() {
    let key = |raw: &str| WorkQuery::parse(raw).unwrap().unwrap().key().clone();
    let base = SearchScope::of(Some(&key("mưa")), &[tag_id(1), tag_id(2)]);
    assert_eq!(
        base,
        SearchScope::of(Some(&key("MƯA")), &[tag_id(1), tag_id(2)])
    );
    assert_eq!(
        base,
        SearchScope::of(
            Some(&key(&"mưa".nfd().collect::<String>())),
            &[tag_id(1), tag_id(2)]
        )
    );
    for other in [
        SearchScope::of(None, &[tag_id(1), tag_id(2)]),
        SearchScope::of(Some(&key("mua he")), &[tag_id(1), tag_id(2)]),
        SearchScope::of(Some(&key("mưa")), &[tag_id(1)]),
        SearchScope::of(Some(&key("mưa")), &[]),
        SearchScope::of(Some(&key("mưa")), &[tag_id(1), tag_id(3)]),
    ] {
        assert_ne!(base, other);
    }
    assert_ne!(
        SearchScope::of(None, &[]),
        SearchScope::of(None, &[tag_id(1)])
    );
}

#[test]
fn cursors_round_trip_and_have_one_spelling() {
    let at = datetime!(2026-11-08 09:30:15.123456 UTC);
    let scope = SearchScope::of(None, &[]);
    let cursor = DiscoveryCursor::new(
        PagePosition {
            published_at: at,
            work_id: work(7),
        },
        scope,
    );
    let wire = cursor.encode();
    // Independent layout: version, microseconds big-endian, work id, scope.
    let micros = 1_794_130_215_123_456_i64;
    assert_eq!(at.unix_timestamp_nanos(), i128::from(micros) * 1000);
    let expected_prefix = format!("01{micros:016x}33330000000040008000000000000007");
    assert!(wire.starts_with(&expected_prefix), "{wire}");
    assert_eq!(wire.len(), 66);
    assert_eq!(DiscoveryCursor::decode(&wire), Ok(cursor));

    let malformed = [
        String::new(),
        wire[..64].to_string(),
        format!("{wire}00"),
        wire.to_uppercase(),
        format!("02{}", &wire[2..]),
        format!("zz{}", &wire[2..]),
        format!(" {}", &wire[1..]),
        "é".repeat(33),
    ];
    for raw in malformed {
        assert_eq!(
            DiscoveryCursor::decode(&raw),
            Err(CursorError::Malformed),
            "{raw:?}"
        );
    }
}

#[test]
fn a_page_names_a_cursor_only_when_another_row_was_read() {
    let size = PageSize::new(2).unwrap();
    let scope = SearchScope::of(None, &[]);
    let at = datetime!(2026-11-01 08:00 UTC);
    let position = |n: &u128| PagePosition {
        published_at: at,
        work_id: work(*n),
    };
    let (rows, next) = cut_page(vec![1, 2], size, scope, position);
    assert_eq!((rows, next), (vec![1, 2], None));
    let (rows, next) = cut_page(vec![1, 2, 3], size, scope, position);
    assert_eq!(rows, vec![1, 2]);
    assert_eq!(
        next.map(|c| c.after()),
        Some(PagePosition {
            published_at: at,
            work_id: work(2)
        })
    );
    let (rows, next) = cut_page(Vec::<u128>::new(), size, scope, position);
    assert!(rows.is_empty() && next.is_none());
}
