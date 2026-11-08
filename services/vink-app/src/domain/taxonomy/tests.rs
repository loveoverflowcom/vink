//! Expected values are written by hand from the documented rules, never computed by calling the
//! function under test; the digest vectors were computed independently from literal bytes.
use uuid::Uuid;

use super::*;
use crate::domain::document::{Document, Title};
use crate::domain::ids::{PrincipalId, TagId};
use crate::domain::principal::Standing;
use crate::domain::revision::ContentHash;

fn id(n: u128) -> TagId {
    TagId::from_uuid(Uuid::from_u128(n))
}

fn label(raw: &str) -> TagLabel {
    TagLabel::parse(raw).unwrap()
}

fn tag(n: u128, facet: TagFacet, raw: &str) -> CatalogTag {
    let label = label(raw);
    CatalogTag::restore(
        id(n),
        facet,
        label.clone(),
        label.slug(),
        None,
        TagStatus::Active,
    )
    .unwrap()
}

fn retired(t: CatalogTag, successor: Option<TagId>) -> CatalogTag {
    CatalogTag::restore(
        t.id(),
        t.facet(),
        t.label().clone(),
        t.slug().clone(),
        None,
        TagStatus::Retired { successor },
    )
    .unwrap()
}

// Example catalog from the ADR TEACH walkthrough (a proposal, not seeded data).
const TAN_VAN: u128 = 1;
const THO: u128 = 2;
const KY_UC: u128 = 11;
const GIA_DINH: u128 = 12;
const DO_THI: u128 = 13;
const MUA: u128 = 14;
const LANG_QUE: u128 = 15;

fn catalog() -> Vec<CatalogTag> {
    vec![
        tag(TAN_VAN, TagFacet::Form, "Tản văn"),
        tag(THO, TagFacet::Form, "Thơ"),
        tag(KY_UC, TagFacet::Theme, "Ký ức"),
        tag(GIA_DINH, TagFacet::Theme, "Gia đình"),
        tag(DO_THI, TagFacet::Theme, "Đô thị"),
        tag(MUA, TagFacet::Theme, "Mùa"),
        tag(LANG_QUE, TagFacet::Theme, "Làng quê"),
    ]
}

// ---------- text ----------

#[test]
fn labels_keep_marks_and_case_and_equate_nfc_nfd_spellings() {
    assert_eq!(label("  Ký   ức ").as_str(), "Ký ức");
    // "Ký ức" typed with combining marks: y + U+0301, u + U+031B + U+0301.
    assert_eq!(label("Ky\u{0301} u\u{031b}\u{0301}c"), label("Ký ức"));
    assert_eq!(label("Đô thị").as_str(), "Đô thị");
    assert_ne!(label("Đô thị"), label("đô thị"));
    for ok in ["Thơ 1-2-3", "O’Neill", "Lục bát", "Hà Nội", "ĐÀ LẠT"] {
        assert!(TagLabel::parse(ok).is_ok(), "{ok}");
    }
}

#[test]
fn ascii_fast_path_agrees_with_full_normalization() {
    use unicode_normalization::UnicodeNormalization;
    let ascii: String = (0u8..128).map(char::from).collect();
    for text in [ascii.as_str(), "Tho tu do", "O'Neill - 1-2-3", ""] {
        assert_eq!(super::text::nfc(text), text.nfc().collect::<String>());
    }
    for c in ascii.chars() {
        // The full rule for an ASCII char: its own NFD base, no marks, one-char lowercase.
        assert_eq!(
            super::text::is_tag_letter(c),
            c.is_ascii_alphabetic(),
            "{c:?}"
        );
    }
    assert_eq!(super::text::nfc("Ky\u{0301}"), "Ký"); // non-ASCII still normalizes
}

#[test]
fn labels_reject_other_scripts_confusables_and_structure() {
    for (raw, error) in [
        ("", LabelError::Empty),
        ("   \t ", LabelError::Empty),
        ("---", LabelError::NoLetterOrDigit),
        ("Thơ!", LabelError::ForbiddenCharacter),
        ("Thơ #1", LabelError::ForbiddenCharacter),
        ("俳句", LabelError::ForbiddenCharacter),
        ("Ｔhơ", LabelError::ForbiddenCharacter), // fullwidth T
        ("Тhơ", LabelError::ForbiddenCharacter),  // Cyrillic Т
        ("ﬁn", LabelError::ForbiddenCharacter),   // ligature
        ("İzmir", LabelError::ForbiddenCharacter), // lowercase is two chars
        ("a\u{0301}\u{0301}\u{0301}", LabelError::ForbiddenCharacter), // stray marks
        ("mưa\u{0000}", LabelError::ForbiddenCharacter),
        ("🌧 mưa", LabelError::ForbiddenCharacter),
    ] {
        assert_eq!(TagLabel::parse(raw), Err(error), "{raw:?}");
    }
}

#[test]
fn label_length_is_counted_in_nfc_characters_at_the_boundary() {
    let at_limit = "ệ".repeat(MAX_LABEL_CHARS); // 40 chars, 120 bytes
    assert_eq!(label(&at_limit).as_str().chars().count(), 40);
    // The same 40 letters fully decomposed (5 bytes each) are still 40 characters.
    let decomposed = "e\u{0323}\u{0302}".repeat(MAX_LABEL_CHARS);
    assert_eq!(label(&decomposed), label(&at_limit));
    assert_eq!(
        TagLabel::parse(&"ệ".repeat(MAX_LABEL_CHARS + 1)),
        Err(LabelError::TooLong)
    );
    assert_eq!(
        TagLabel::parse(&"a".repeat(MAX_RAW_TEXT_BYTES + 1)),
        Err(LabelError::TooLong)
    );
}

#[test]
fn slugs_keep_vietnamese_marks_and_join_words_with_hyphens() {
    for (raw, expected) in [
        ("Tản văn", "tản-văn"),
        ("Ký ức", "ký-ức"),
        ("Gia đình", "gia-đình"),
        ("ĐÀ LẠT", "đà-lạt"),
        ("Thơ 1-2-3", "thơ-1-2-3"),
        ("O’Neill", "o-neill"),
        ("Ký - ức", "ký-ức"),
        ("-Ký ức-", "ký-ức"),
    ] {
        assert_eq!(label(raw).slug().as_str(), expected, "{raw}");
    }
}

#[test]
fn ma_family_shares_a_search_key_but_never_a_slug() {
    let family = ["ma", "má", "mà", "mả", "mã", "mạ"];
    let slugs: Vec<TagSlug> = family.iter().map(|raw| label(raw).slug()).collect();
    for (i, slug) in slugs.iter().enumerate() {
        assert_eq!(slug.as_str(), family[i]);
        assert_eq!(label(family[i]).search_key().as_str(), "ma");
        assert!(!slugs[..i].contains(slug), "{} collides", family[i]);
    }
    // NFD spellings land on the same slugs.
    assert_eq!(label("ma\u{0301}").slug(), slugs[1]);
    assert_eq!(label("ma\u{0323}").slug(), slugs[5]);
    // Đ is a letter of its own for identity, folded to d only for search.
    assert_ne!(label("Đa").slug(), label("Da").slug());
    assert_eq!(label("Đa").search_key(), label("Da").search_key());
}

#[test]
fn slug_parsing_normalizes_case_and_nfd_and_rejects_malformed_paths() {
    assert_eq!(TagSlug::parse("Ký-Ức").unwrap().as_str(), "ký-ức");
    assert_eq!(
        TagSlug::parse("ky\u{0301}-u\u{031b}\u{0301}c")
            .unwrap()
            .as_str(),
        "ký-ức"
    );
    assert_eq!(TagSlug::parse("ky-uc").unwrap().as_str(), "ky-uc"); // valid, different slug
    for (raw, error) in [
        ("", SlugError::Empty),
        ("-ký", SlugError::Malformed),
        ("ký-", SlugError::Malformed),
        ("ký--ức", SlugError::Malformed),
        ("ký ức", SlugError::Malformed),
        ("ký/ức", SlugError::Malformed),
        ("..", SlugError::Malformed),
        ("тhơ", SlugError::Malformed),
    ] {
        assert_eq!(TagSlug::parse(raw), Err(error), "{raw:?}");
    }
    assert_eq!(
        TagSlug::parse(&"ơ".repeat(MAX_LABEL_CHARS + 1)),
        Err(SlugError::TooLong)
    );
    // Every label's derived slug parses back to itself.
    for raw in [
        "Tản văn",
        "ĐÀ LẠT",
        "Thơ 1-2-3",
        "mạ",
        &"Ệ".repeat(MAX_LABEL_CHARS),
    ] {
        let slug = label(raw).slug();
        assert_eq!(TagSlug::parse(slug.as_str()), Ok(slug));
    }
}

#[test]
fn search_keys_fold_marks_case_d_and_separators() {
    for (raw, expected) in [
        ("Ký ức", "ky uc"),
        ("ký-ức", "ky uc"),
        ("KÝ  ỨC", "ky uc"),
        ("Ky\u{0301} u\u{031b}\u{0301}c", "ky uc"),
        ("ĐÀ LẠT", "da lat"),
        ("  Thơ   tự-do ", "tho tu do"),
    ] {
        assert_eq!(SearchKey::of(raw).as_str(), expected, "{raw:?}");
    }
    let key = SearchKey::of("Ký ức gia đình");
    assert!(key.has_word_prefix(&SearchKey::of("gia")));
    assert!(key.has_word_prefix(&SearchKey::of("ky u")));
    assert!(!key.has_word_prefix(&SearchKey::of("ia")));
    assert!(!key.has_word_prefix(&SearchKey::of("")));
}

#[test]
fn descriptions_and_queries_are_bounded() {
    assert_eq!(
        TagDescription::parse("  Văn xuôi ngắn, giàu cảm xúc. ")
            .unwrap()
            .as_str(),
        "Văn xuôi ngắn, giàu cảm xúc."
    );
    assert_eq!(TagDescription::parse(" "), Err(DescriptionError::Empty));
    assert_eq!(
        TagDescription::parse("hai\ndòng"),
        Err(DescriptionError::ForbiddenCharacter)
    );
    assert!(TagDescription::parse(&"ơ".repeat(MAX_DESCRIPTION_CHARS)).is_ok());
    assert_eq!(
        TagDescription::parse(&"ơ".repeat(MAX_DESCRIPTION_CHARS + 1)),
        Err(DescriptionError::TooLong)
    );
    let query = TagQuery::parse("Má").unwrap();
    assert_eq!(query.exact().unwrap().as_str(), "má");
    assert_eq!(query.key().as_str(), "ma");
    assert_eq!(TagQuery::parse("má?").unwrap().exact(), None); // still searchable
    assert_eq!(TagQuery::parse(" ?! "), Err(QueryError::Empty));
    assert_eq!(
        TagQuery::parse(&"a".repeat(MAX_QUERY_CHARS + 1)),
        Err(QueryError::TooLong)
    );
}

// ---------- catalog ----------

fn facts(version: i64) -> CatalogFacts {
    CatalogFacts {
        can_curate: true,
        version: CatalogVersion::new(version).unwrap(),
        subject: None,
        slug_holder: None,
        successor: None,
    }
}

fn command(expected: i64, change: CatalogChange) -> CatalogCommand {
    CatalogCommand {
        expected: CatalogVersion::new(expected).unwrap(),
        change,
    }
}

fn accepted(outcome: CatalogOutcome) -> CatalogWrite {
    match outcome {
        CatalogOutcome::Accepted(write) => write,
        other => panic!("expected acceptance, got {other:?}"),
    }
}

fn holder(n: u128, kind: SlugKind) -> Option<SlugHolder> {
    Some(SlugHolder { tag: id(n), kind })
}

#[test]
fn creating_tags_claims_one_canonical_slug_and_refuses_duplicates() {
    let create = |raw: &str| CatalogChange::Create {
        id: id(MUA),
        facet: TagFacet::Theme,
        label: label(raw),
        description: None,
    };
    let write = accepted(decide_catalog_change(&command(4, create("Mùa")), &facts(4)));
    assert_eq!(write.version().get(), 5);
    assert_eq!(write.tag(), Some(&tag(MUA, TagFacet::Theme, "Mùa")));
    assert_eq!(
        write.slugs(),
        [SlugWrite {
            slug: label("mùa").slug(),
            tag: id(MUA),
            kind: NameKind::Canonical,
            replaces: None,
        }]
    );
    // "MÙA" and an NFD spelling are the same name: the slug is taken.
    for raw in ["MÙA", "mu\u{0300}a"] {
        let taken = CatalogFacts {
            slug_holder: holder(99, SlugKind::Canonical),
            ..facts(4)
        };
        assert_eq!(
            decide_catalog_change(&command(4, create(raw)), &taken),
            CatalogOutcome::Refused(CatalogRefusal::SlugTaken { holder: id(99) })
        );
    }
    // A former name of another tag is never reassigned either.
    let former = CatalogFacts {
        slug_holder: holder(99, SlugKind::Former),
        ..facts(4)
    };
    assert_eq!(
        decide_catalog_change(&command(4, create("Mùa")), &former),
        CatalogOutcome::Refused(CatalogRefusal::SlugTaken { holder: id(99) })
    );
    let id_taken = CatalogFacts {
        subject: Some(tag(MUA, TagFacet::Theme, "Mưa")),
        ..facts(4)
    };
    assert_eq!(
        decide_catalog_change(&command(4, create("Mùa")), &id_taken),
        CatalogOutcome::Refused(CatalogRefusal::TagIdTaken)
    );
}

#[test]
fn every_member_of_the_ma_family_can_be_its_own_tag() {
    // Each create sees no holder for its own slug, because the slugs differ.
    for (n, raw) in ["ma", "má", "mà", "mả", "mã", "mạ"].iter().enumerate() {
        let change = CatalogChange::Create {
            id: id(100 + n as u128),
            facet: TagFacet::Theme,
            label: label(raw),
            description: None,
        };
        let write = accepted(decide_catalog_change(&command(1, change), &facts(1)));
        assert_eq!(write.slugs()[0].slug.as_str(), *raw);
    }
}

#[test]
fn renaming_keeps_identity_and_turns_the_old_slug_into_a_redirect() {
    let original = tag(DO_THI, TagFacet::Theme, "Đô thị");
    let rename = |raw: &str| CatalogChange::Rename {
        id: id(DO_THI),
        label: label(raw),
    };
    let base = CatalogFacts {
        subject: Some(original.clone()),
        ..facts(7)
    };
    let write = accepted(decide_catalog_change(&command(7, rename("Phố thị")), &base));
    let renamed = write.tag().unwrap();
    assert_eq!(renamed.id(), id(DO_THI));
    assert_eq!(renamed.slug().as_str(), "phố-thị");
    assert_eq!(
        write.slugs(),
        [
            SlugWrite {
                slug: label("Phố thị").slug(),
                tag: id(DO_THI),
                kind: NameKind::Canonical,
                replaces: None,
            },
            SlugWrite {
                slug: original.slug().clone(),
                tag: id(DO_THI),
                kind: NameKind::Former,
                replaces: Some(SlugKind::Canonical),
            },
        ]
    );
    // The old URL now redirects to the renamed tag.
    assert_eq!(
        resolve_slug(
            original.slug(),
            holder(DO_THI, SlugKind::Former),
            Some(renamed)
        ),
        Ok(SlugResolution::Redirect(renamed))
    );
    // Renaming back takes the tag's own former slug; another tag's slug is refused.
    let back = CatalogFacts {
        subject: Some(renamed.clone()),
        slug_holder: holder(DO_THI, SlugKind::Former),
        ..facts(8)
    };
    let write = accepted(decide_catalog_change(&command(8, rename("Đô thị")), &back));
    assert_eq!(write.slugs()[0].replaces, Some(SlugKind::Former));
    let theirs = CatalogFacts {
        slug_holder: holder(KY_UC, SlugKind::Alias),
        ..base.clone()
    };
    assert_eq!(
        decide_catalog_change(&command(7, rename("Hồi ức")), &theirs),
        CatalogOutcome::Refused(CatalogRefusal::SlugTaken { holder: id(KY_UC) })
    );
    // Case-only rename: same slug, label changes, no slug rows.
    let same_slug = CatalogFacts {
        slug_holder: holder(DO_THI, SlugKind::Canonical),
        ..base.clone()
    };
    let write = accepted(decide_catalog_change(
        &command(7, rename("ĐÔ THỊ")),
        &same_slug,
    ));
    assert_eq!(write.tag().unwrap().label().as_str(), "ĐÔ THỊ");
    assert!(write.slugs().is_empty());
    // Same label: accepted, version advances, nothing changes.
    let write = accepted(decide_catalog_change(
        &command(7, rename("Đô thị")),
        &same_slug,
    ));
    assert!(!write.changed());
    assert_eq!(write.version().get(), 8);
}

#[test]
fn aliases_point_only_at_active_canonical_tags() {
    let ky_uc = tag(KY_UC, TagFacet::Theme, "Ký ức");
    let add = |raw: &str| CatalogChange::AddAlias {
        target: id(KY_UC),
        label: label(raw),
    };
    let base = CatalogFacts {
        subject: Some(ky_uc.clone()),
        ..facts(3)
    };
    let write = accepted(decide_catalog_change(&command(3, add("Hồi ức")), &base));
    assert_eq!(write.tag(), None);
    assert_eq!(
        write.slugs(),
        [SlugWrite {
            slug: label("hồi-ức").slug(),
            tag: id(KY_UC),
            kind: NameKind::Alias(label("Hồi ức")),
            replaces: None,
        }]
    );
    // Self alias: the target's own name.
    let own = CatalogFacts {
        slug_holder: holder(KY_UC, SlugKind::Canonical),
        ..base.clone()
    };
    assert_eq!(
        decide_catalog_change(&command(3, add("KÝ ỨC")), &own),
        CatalogOutcome::Refused(CatalogRefusal::AliasIsCanonicalName)
    );
    // Dangling target and retired target.
    assert_eq!(
        decide_catalog_change(&command(3, add("Hồi ức")), &facts(3)),
        CatalogOutcome::Refused(CatalogRefusal::UnknownTag)
    );
    let gone = CatalogFacts {
        subject: Some(retired(ky_uc.clone(), None)),
        ..facts(3)
    };
    assert_eq!(
        decide_catalog_change(&command(3, add("Hồi ức")), &gone),
        CatalogOutcome::Refused(CatalogRefusal::Retired)
    );
    // An alias of another tag cannot be re-aimed (no chains, no reassignment).
    let other = CatalogFacts {
        slug_holder: holder(GIA_DINH, SlugKind::Alias),
        ..base
    };
    assert_eq!(
        decide_catalog_change(&command(3, add("Hồi ức")), &other),
        CatalogOutcome::Refused(CatalogRefusal::SlugTaken {
            holder: id(GIA_DINH)
        })
    );
}

#[test]
fn retiring_keeps_references_and_validates_the_successor_hint() {
    let do_thi = tag(DO_THI, TagFacet::Theme, "Đô thị");
    let retire = |successor| CatalogChange::Retire {
        id: id(DO_THI),
        successor,
    };
    let base = CatalogFacts {
        subject: Some(do_thi.clone()),
        ..facts(2)
    };
    let write = accepted(decide_catalog_change(&command(2, retire(None)), &base));
    assert_eq!(
        write.tag().unwrap().status(),
        TagStatus::Retired { successor: None }
    );
    assert!(write.slugs().is_empty());
    let with = |t: CatalogTag| CatalogFacts {
        successor: Some(t.facts()),
        ..base.clone()
    };
    let ok = accepted(decide_catalog_change(
        &command(2, retire(Some(id(LANG_QUE)))),
        &with(tag(LANG_QUE, TagFacet::Theme, "Làng quê")),
    ));
    assert_eq!(
        ok.tag().unwrap().status(),
        TagStatus::Retired {
            successor: Some(id(LANG_QUE))
        }
    );
    for (successor, found, error) in [
        (
            id(DO_THI),
            Some(do_thi.clone()),
            SuccessorError::SelfReference,
        ),
        (id(LANG_QUE), None, SuccessorError::Unknown),
        (
            id(LANG_QUE),
            Some(retired(tag(LANG_QUE, TagFacet::Theme, "Làng quê"), None)),
            SuccessorError::Retired,
        ),
        (
            id(THO),
            Some(tag(THO, TagFacet::Form, "Thơ")),
            SuccessorError::OtherFacet,
        ),
    ] {
        let facts = CatalogFacts {
            successor: found.map(|t| t.facts()),
            ..base.clone()
        };
        assert_eq!(
            decide_catalog_change(&command(2, retire(Some(successor))), &facts),
            CatalogOutcome::Refused(CatalogRefusal::InvalidSuccessor(error))
        );
    }
    let again = CatalogFacts {
        subject: Some(retired(do_thi, None)),
        ..facts(2)
    };
    assert_eq!(
        decide_catalog_change(&command(2, retire(None)), &again),
        CatalogOutcome::Refused(CatalogRefusal::Retired)
    );
}

#[test]
fn catalog_authority_version_and_fact_ownership_come_first() {
    let describe = CatalogChange::Describe {
        id: id(MUA),
        description: TagDescription::parse("Bốn mùa, thời tiết.").ok(),
    };
    let base = CatalogFacts {
        subject: Some(tag(MUA, TagFacet::Theme, "Mùa")),
        ..facts(5)
    };
    let not_curator = CatalogFacts {
        can_curate: false,
        version: CatalogVersion::FIRST,
        ..base.clone()
    };
    // A non-curator learns nothing, not even that the version moved.
    assert_eq!(
        decide_catalog_change(&command(5, describe.clone()), &not_curator),
        CatalogOutcome::Refused(CatalogRefusal::NotCurator)
    );
    assert_eq!(
        decide_catalog_change(&command(4, describe.clone()), &base),
        CatalogOutcome::Conflict {
            current: CatalogVersion::new(5).unwrap()
        }
    );
    let foreign = CatalogFacts {
        subject: Some(tag(KY_UC, TagFacet::Theme, "Ký ức")),
        ..base.clone()
    };
    let stray = CatalogFacts {
        slug_holder: holder(MUA, SlugKind::Canonical),
        ..base.clone()
    };
    for bad in [foreign, stray] {
        assert_eq!(
            decide_catalog_change(&command(5, describe.clone()), &bad),
            CatalogOutcome::Refused(CatalogRefusal::InvalidFacts)
        );
    }
    let exhausted = CatalogFacts {
        version: CatalogVersion::new(i64::MAX).unwrap(),
        ..base.clone()
    };
    assert_eq!(
        decide_catalog_change(&command(i64::MAX, describe.clone()), &exhausted),
        CatalogOutcome::Refused(CatalogRefusal::VersionExhausted)
    );
    let write = accepted(decide_catalog_change(&command(5, describe), &base));
    assert_eq!(
        write.tag().unwrap().description().unwrap().as_str(),
        "Bốn mùa, thời tiết."
    );
}

#[test]
fn stored_catalog_rows_fail_closed_when_inconsistent() {
    let label = label("Ký ức");
    assert!(
        CatalogTag::restore(
            id(KY_UC),
            TagFacet::Theme,
            label.clone(),
            TagSlug::parse("ky-uc").unwrap(),
            None,
            TagStatus::Active
        )
        .is_none()
    );
    assert!(
        CatalogTag::restore(
            id(KY_UC),
            TagFacet::Theme,
            label.clone(),
            label.slug(),
            None,
            TagStatus::Retired {
                successor: Some(id(KY_UC))
            }
        )
        .is_none()
    );
    assert!(TagAlias::restore(label.clone(), TagSlug::parse("ky-uc").unwrap(), id(1)).is_none());
    let t = tag(KY_UC, TagFacet::Theme, "Ký ức");
    for (holder, found) in [
        (None, Some(&t)),
        (holder(GIA_DINH, SlugKind::Canonical), Some(&t)),
        (holder(KY_UC, SlugKind::Alias), Some(&t)), // canonical slug filed as alias
        (holder(KY_UC, SlugKind::Canonical), None),
    ] {
        assert_eq!(resolve_slug(t.slug(), holder, found), Err(CatalogIntegrity));
    }
}

#[test]
fn slugs_resolve_to_canonical_redirect_retired_or_not_found() {
    let t = tag(KY_UC, TagFacet::Theme, "Ký ức");
    let alias = label("Hồi ức").slug();
    assert_eq!(
        resolve_slug(t.slug(), holder(KY_UC, SlugKind::Canonical), Some(&t)),
        Ok(SlugResolution::Canonical(&t))
    );
    assert_eq!(
        resolve_slug(&alias, holder(KY_UC, SlugKind::Alias), Some(&t)),
        Ok(SlugResolution::Redirect(&t))
    );
    let gone = retired(t.clone(), Some(id(GIA_DINH)));
    assert_eq!(
        resolve_slug(&alias, holder(KY_UC, SlugKind::Alias), Some(&gone)),
        Ok(SlugResolution::Retired(&gone))
    );
    // The folded spelling is a different slug; with no holder it is simply not found.
    assert_eq!(
        resolve_slug(&TagSlug::parse("ky-uc").unwrap(), None, None),
        Ok(SlugResolution::NotFound)
    );
}

#[test]
fn lookup_ranks_exact_marks_before_folded_candidates_and_resolves_aliases() {
    let mut tags: Vec<CatalogTag> = ["mạ", "má", "ma", "mà", "mã"]
        .iter()
        .enumerate()
        .map(|(n, raw)| tag(200 + n as u128, TagFacet::Theme, raw))
        .collect();
    tags.push(tag(KY_UC, TagFacet::Theme, "Ký ức"));
    tags.push(retired(tag(DO_THI, TagFacet::Theme, "Mạch ngầm"), None));
    let aliases = vec![
        TagAlias::restore(label("Hồi ức"), label("Hồi ức").slug(), id(KY_UC)).unwrap(),
        TagAlias::restore(label("Ma quái"), label("Ma quái").slug(), id(DO_THI)).unwrap(),
    ];
    let names = |raw: &str| -> Vec<(String, MatchTier, Option<String>)> {
        rank_matches(
            &TagQuery::parse(raw).unwrap(),
            &tags,
            &aliases,
            MAX_SUGGESTIONS,
        )
        .into_iter()
        .map(|m| {
            (
                m.tag.label().as_str().to_owned(),
                m.tier,
                m.via_alias.map(|a| a.label().as_str().to_owned()),
            )
        })
        .collect()
    };
    let only_labels = |found: Vec<(String, MatchTier, Option<String>)>| -> Vec<String> {
        found.into_iter().map(|(l, _, _)| l).collect()
    };
    // Exact "má" first; the rest of the family are folded candidates; "Mạch ngầm" is retired
    // (and its alias "Ma quái" resolves to a retired tag), so neither is offered.
    let marked = names("MÁ");
    assert_eq!(marked[0], ("má".into(), MatchTier::Exact, None));
    assert!(
        marked[1..]
            .iter()
            .all(|(_, tier, _)| *tier == MatchTier::Folded)
    );
    assert_eq!(marked.len(), 5);
    let unmarked = only_labels(names("ma"));
    assert_eq!(unmarked[0], "ma");
    assert_eq!(unmarked.len(), 5);
    assert!(!unmarked.contains(&"Mạch ngầm".to_string()));
    // NFD query, folded query and an alias all reach the same canonical tag.
    assert_eq!(
        names("Ky\u{0301} u\u{031b}\u{0301}c")[0].1,
        MatchTier::Exact
    );
    assert_eq!(names("ky uc")[0], ("Ký ức".into(), MatchTier::Folded, None));
    assert_eq!(
        names("hồi"),
        vec![("Ký ức".into(), MatchTier::Prefix, Some("Hồi ức".into()))]
    );
    assert_eq!(names("hoi uc")[0].1, MatchTier::Folded);
    assert_eq!(
        rank_matches(&TagQuery::parse("ma").unwrap(), &tags, &aliases, 2).len(),
        2
    );
}

// ---------- assignment ----------

fn actor() -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(0xA))
}

fn other() -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(0xB))
}

fn work_facts(version: i64, current: WorkTags) -> WorkTagFacts {
    WorkTagFacts {
        owner: actor(),
        standing: Standing::Active,
        version: MetadataVersion::new(version).unwrap(),
        current,
        catalog: catalog().iter().map(CatalogTag::facts).collect(),
    }
}

fn select(form: Option<u128>, themes: &[u128]) -> TagSelection {
    let themes: Vec<TagId> = themes.iter().map(|n| id(*n)).collect();
    TagSelection::new(form.map(id), &themes).unwrap()
}

fn change(expected: i64, form: Option<u128>, themes: &[u128]) -> TagChange {
    TagChange::new(
        MetadataVersion::new(expected).unwrap(),
        select(form, themes),
    )
}

fn tags(form: Option<u128>, themes: &[u128]) -> WorkTags {
    WorkTags::restore(form.map(id), themes.iter().map(|n| id(*n)).collect()).unwrap()
}

fn write(outcome: TagOutcome) -> TagWrite {
    match outcome {
        TagOutcome::Accepted(write) => write,
        other => panic!("expected acceptance, got {other:?}"),
    }
}

#[test]
fn teach_example_tan_van_ky_uc_gia_dinh_is_accepted_in_author_order() {
    let facts = work_facts(1, WorkTags::EMPTY);
    let saved = write(decide_tag_change(
        actor(),
        &change(1, Some(TAN_VAN), &[KY_UC, GIA_DINH]),
        Some(&facts),
    ));
    assert_eq!(saved.version().get(), 2);
    assert!(saved.changed());
    assert_eq!(
        saved.tags().ids().collect::<Vec<_>>(),
        [id(TAN_VAN), id(KY_UC), id(GIA_DINH)]
    );
    let catalog = catalog();
    let shown: Vec<&str> = draft_tags(saved.tags(), &catalog)
        .unwrap()
        .iter()
        .map(|t| t.label().as_str())
        .collect();
    assert_eq!(shown, ["Tản văn", "Ký ức", "Gia đình"]);
}

#[test]
fn selections_are_bounded_before_parsing_and_deduplicated_in_order() {
    // Picking "Ký ức" directly and again through its alias yields one identity.
    let s = select(None, &[KY_UC, GIA_DINH, KY_UC]);
    assert_eq!(s.themes(), [id(KY_UC), id(GIA_DINH)]);
    let raw = |n: usize| -> Vec<String> {
        (0..n)
            .map(|_| "b2222222-2222-4222-8222-222222222222".into())
            .collect()
    };
    assert_eq!(
        TagSelection::parse(None, &raw(MAX_THEME_REFS))
            .unwrap()
            .themes()
            .len(),
        1
    );
    assert_eq!(
        TagSelection::parse(None, &raw(MAX_THEME_REFS + 1)),
        Err(SelectionError::TooManyThemeRefs)
    );
    assert_eq!(
        TagSelection::parse(Some("thơ"), &[]),
        Err(SelectionError::Form)
    );
    let mut bad = raw(2);
    bad[1] = "ký-ức".into();
    assert_eq!(
        TagSelection::parse(None, &bad),
        Err(SelectionError::Theme { index: 1 })
    );
}

/// Bounded-exhaustive instead of Kani: every sequence over three identities of length 0..=7
/// (3 280 inputs). The oracle is a set-based restatement, not the production loop.
#[test]
fn selection_dedup_is_exhaustively_first_seen_up_to_one_past_the_bound() {
    let ids = [id(1), id(2), id(3)];
    let mut checked = 0;
    for len in 0..=MAX_THEME_REFS + 1 {
        for code in 0..3usize.pow(len as u32) {
            let raw: Vec<TagId> = (0..len)
                .map(|i| ids[code / 3usize.pow(i as u32) % 3])
                .collect();
            let result = TagSelection::new(None, &raw);
            checked += 1;
            if len > MAX_THEME_REFS {
                assert_eq!(result, Err(SelectionError::TooManyThemeRefs));
                continue;
            }
            let themes = result.unwrap().themes().to_vec();
            let mut seen = std::collections::HashSet::new();
            let expected: Vec<TagId> = raw.iter().copied().filter(|t| seen.insert(*t)).collect();
            assert_eq!(themes, expected, "{raw:?}");
        }
    }
    assert_eq!(checked, (0..=7).map(|k| 3usize.pow(k)).sum::<usize>());
}

#[test]
fn theme_bound_is_three_distinct_tags_and_the_form_slot_is_typed() {
    let facts = work_facts(1, WorkTags::EMPTY);
    let three = change(1, Some(THO), &[KY_UC, GIA_DINH, MUA]);
    assert_eq!(
        write(decide_tag_change(actor(), &three, Some(&facts)))
            .tags()
            .themes()
            .len(),
        3
    );
    let four = change(1, Some(THO), &[KY_UC, GIA_DINH, MUA, LANG_QUE]);
    assert_eq!(
        decide_tag_change(actor(), &four, Some(&facts)),
        TagOutcome::Refused(TagRefusal::TooManyThemes)
    );
    // Duplicates do not count against the bound.
    let repeated = change(1, None, &[KY_UC, GIA_DINH, MUA, KY_UC, MUA]);
    assert!(matches!(
        decide_tag_change(actor(), &repeated, Some(&facts)),
        TagOutcome::Accepted(_)
    ));
    for (selection, tag, slot) in [
        (change(1, Some(KY_UC), &[]), KY_UC, TagFacet::Form),
        (change(1, None, &[THO]), THO, TagFacet::Theme),
        (change(1, Some(THO), &[THO]), THO, TagFacet::Theme),
    ] {
        assert_eq!(
            decide_tag_change(actor(), &selection, Some(&facts)),
            TagOutcome::Refused(TagRefusal::WrongFacet { tag: id(tag), slot })
        );
    }
    assert!(WorkTags::restore(None, vec![id(1), id(2), id(3), id(4)]).is_none());
    assert!(WorkTags::restore(Some(id(1)), vec![id(1)]).is_none());
    assert!(WorkTags::restore(None, vec![id(1), id(1)]).is_none());
}

#[test]
fn unknown_and_newly_chosen_retired_tags_are_refused_but_existing_ones_stay() {
    let mut facts = work_facts(3, tags(Some(TAN_VAN), &[DO_THI]));
    // The curator retired "Đô thị" with the successor hint "Mùa".
    for t in &mut facts.catalog {
        if t.id == id(DO_THI) {
            t.status = TagStatus::Retired {
                successor: Some(id(MUA)),
            };
        }
    }
    // Editing another theme keeps the retired one: no silent rewrite of the author's choice.
    let kept = write(decide_tag_change(
        actor(),
        &change(3, Some(TAN_VAN), &[DO_THI, KY_UC]),
        Some(&facts),
    ));
    assert_eq!(kept.tags().themes(), [id(DO_THI), id(KY_UC)]);
    // A draft that does not already hold it cannot add it.
    let fresh = WorkTagFacts {
        current: WorkTags::EMPTY,
        ..facts.clone()
    };
    assert_eq!(
        decide_tag_change(actor(), &change(3, None, &[DO_THI]), Some(&fresh)),
        TagOutcome::Refused(TagRefusal::RetiredTag {
            tag: id(DO_THI),
            successor: Some(id(MUA))
        })
    );
    // "Chưa có thẻ này": an identity the catalog does not know.
    assert_eq!(
        decide_tag_change(actor(), &change(3, None, &[999]), Some(&facts)),
        TagOutcome::Refused(TagRefusal::UnknownTag(id(999)))
    );
    // Removing the retired tag is always allowed.
    let removed = write(decide_tag_change(
        actor(),
        &change(3, Some(TAN_VAN), &[]),
        Some(&facts),
    ));
    assert!(!removed.tags().contains(id(DO_THI)));
}

#[test]
fn stale_tab_gets_a_conflict_with_the_observed_tags_and_writes_nothing() {
    // Tab A and tab B both rendered version 4 with [Ký ức].
    let mut facts = work_facts(4, tags(None, &[KY_UC]));
    let tab_a = change(4, None, &[KY_UC, GIA_DINH]);
    let a = write(decide_tag_change(actor(), &tab_a, Some(&facts)));
    facts.version = a.version();
    facts.current = a.tags().clone();
    // Tab B's older command arrives after A committed.
    let tab_b = change(4, None, &[MUA]);
    let b = decide_tag_change(actor(), &tab_b, Some(&facts));
    assert_eq!(
        b,
        TagOutcome::Conflict {
            current: MetadataVersion::new(5).unwrap(),
            observed: tags(None, &[KY_UC, GIA_DINH]),
        }
    );
    // Conflict precedes validation: even an invalid stale command reports the conflict.
    assert!(matches!(
        decide_tag_change(actor(), &change(4, None, &[999]), Some(&facts)),
        TagOutcome::Conflict { .. }
    ));
    // Tab A's lost acknowledgement: its retry is a conflict, and the observed state shows
    // that A's desired tags hold now (not which command produced them).
    let TagOutcome::Conflict { observed, .. } = decide_tag_change(actor(), &tab_a, Some(&facts))
    else {
        panic!("retry with the old version must conflict");
    };
    assert!(observed.matches(tab_a.selection()));
    assert!(!observed.matches(tab_b.selection()));
}

#[test]
fn unchanged_save_still_advances_the_version_and_fences_older_commands() {
    let mut facts = work_facts(9, tags(None, &[KY_UC]));
    // The author picks Gia đình (cmd1) then reverts to [Ký ức] (cmd2), both against v9.
    let cmd1 = change(9, None, &[KY_UC, GIA_DINH]);
    let cmd2 = change(9, None, &[KY_UC]);
    // cmd2 arrives first: same tags, accepted, version advances.
    let w2 = write(decide_tag_change(actor(), &cmd2, Some(&facts)));
    assert!(!w2.changed());
    assert_eq!(w2.version().get(), 10);
    facts.version = w2.version();
    // The delayed cmd1 can no longer resurrect the older intent.
    assert!(matches!(
        decide_tag_change(actor(), &cmd1, Some(&facts)),
        TagOutcome::Conflict { .. }
    ));
}

#[test]
fn other_owners_missing_works_suspension_and_bad_facts_write_nothing() {
    let facts = work_facts(1, WorkTags::EMPTY);
    let c = change(1, None, &[KY_UC]);
    assert_eq!(
        decide_tag_change(other(), &c, Some(&facts)),
        TagOutcome::Refused(TagRefusal::NotFound)
    );
    assert_eq!(
        decide_tag_change(actor(), &c, None),
        TagOutcome::Refused(TagRefusal::NotFound)
    );
    let suspended = WorkTagFacts {
        standing: Standing::Suspended,
        ..facts.clone()
    };
    assert_eq!(
        decide_tag_change(actor(), &c, Some(&suspended)),
        TagOutcome::Refused(TagRefusal::Suspended)
    );
    // A suspended non-owner still learns nothing about the work.
    assert_eq!(
        decide_tag_change(other(), &c, Some(&suspended)),
        TagOutcome::Refused(TagRefusal::NotFound)
    );
    let mut duplicated = facts.clone();
    duplicated.catalog.push(duplicated.catalog[0]);
    assert_eq!(
        decide_tag_change(actor(), &c, Some(&duplicated)),
        TagOutcome::Refused(TagRefusal::InvalidFacts)
    );
    let exhausted = WorkTagFacts {
        version: MetadataVersion::new(i64::MAX).unwrap(),
        ..facts
    };
    assert_eq!(
        decide_tag_change(
            actor(),
            &TagChange::new(exhausted.version, select(None, &[KY_UC])),
            Some(&exhausted)
        ),
        TagOutcome::Refused(TagRefusal::VersionExhausted)
    );
}

#[test]
fn metadata_digest_matches_independent_v1_vectors() {
    // Computed with Python hashlib from the documented byte layout, not with this code.
    let form = TagId::parse("a1111111-1111-4111-8111-111111111111").unwrap();
    let t1 = TagId::parse("b2222222-2222-4222-8222-222222222222").unwrap();
    let t2 = TagId::parse("b3333333-3333-4333-8333-333333333333").unwrap();
    let full = WorkTags::restore(Some(form), vec![t1, t2]).unwrap();
    let swapped = WorkTags::restore(Some(form), vec![t2, t1]).unwrap();
    assert_eq!(
        MetadataDigest::of(&full).as_str(),
        "3caaa54dcf51e0f2ad79f574b6d1ddfe6890a3f55b9604ddffde8882f4798f8a"
    );
    assert_eq!(
        MetadataDigest::of(&swapped).as_str(),
        "373e9f343ff0395bb03624b6720cd6f1f49176b327eee45ac756e05760feaf37"
    );
    assert_eq!(
        MetadataDigest::of(&WorkTags::EMPTY).as_str(),
        "9be68828b1f3703c6923321d34c21a29a7f6913220699f9fa9468f1684094c57"
    );
    let snapshot = MetadataSnapshot::capture(full.clone());
    assert_eq!(
        MetadataSnapshot::restore(full.clone(), snapshot.digest().as_str()),
        Some(snapshot)
    );
    assert_eq!(
        MetadataSnapshot::restore(swapped, MetadataDigest::of(&full).as_str()),
        None
    );
}

#[test]
fn tags_never_change_the_content_hash_v1_of_a_revision() {
    // Same golden vector as domain::revision: tags are not part of the content identity.
    let title = Title::parse("Việt Đ/đ").unwrap();
    let document = Document::parse(crate::domain::document::DocumentData {
        schema_version: crate::domain::document::SCHEMA_VERSION,
        blocks: vec![
            crate::domain::document::BlockData {
                lines: vec!["Đêm xuống,".into(), "  đợi trăng".into()],
            },
            crate::domain::document::BlockData {
                lines: vec!["đường xa.".into()],
            },
        ],
    })
    .unwrap();
    let before = ContentHash::of(&title, &document);
    let facts = work_facts(1, WorkTags::EMPTY);
    let _ = decide_tag_change(actor(), &change(1, Some(TAN_VAN), &[KY_UC]), Some(&facts));
    assert_eq!(
        ContentHash::of(&title, &document).as_str(),
        "88bbbf2e34c3174e82d6a49388dddb70882a216e7ec3ba6b76eea165a49c36dd"
    );
    assert_eq!(ContentHash::of(&title, &document), before);
}

#[test]
fn public_projection_reads_only_the_bound_snapshot() {
    let mut catalog = catalog();
    // Published: Tản văn / Ký ức / Gia đình.
    let published = MetadataSnapshot::capture(tags(Some(TAN_VAN), &[KY_UC, GIA_DINH]));
    let public = |catalog: &[CatalogTag]| -> Vec<String> {
        public_tags(&published, catalog)
            .unwrap()
            .iter()
            .map(|t| t.label().as_str().to_owned())
            .collect()
    };
    assert_eq!(public(&catalog), ["Tản văn", "Ký ức", "Gia đình"]);
    // The author edits the draft to Thơ / Mùa; the public snapshot is untouched.
    let facts = work_facts(2, published.tags().clone());
    let draft = write(decide_tag_change(
        actor(),
        &change(2, Some(THO), &[MUA]),
        Some(&facts),
    ));
    assert_ne!(draft.tags(), published.tags());
    assert_eq!(public(&catalog), ["Tản văn", "Ký ức", "Gia đình"]);
    // A rename shows everywhere (identity is stable); a retired tag is no longer shown.
    let i = catalog.iter().position(|t| t.id() == id(KY_UC)).unwrap();
    catalog[i] = tag(KY_UC, TagFacet::Theme, "Hồi ức");
    let j = catalog.iter().position(|t| t.id() == id(GIA_DINH)).unwrap();
    catalog[j] = retired(catalog[j].clone(), None);
    assert_eq!(public(&catalog), ["Tản văn", "Hồi ức"]);
    // The author still sees the retired tag on a draft that holds it.
    assert_eq!(draft_tags(published.tags(), &catalog).unwrap().len(), 3);
    // A missing catalog row or a changed facet is corruption, not an empty list.
    catalog.remove(i);
    assert_eq!(public_tags(&published, &catalog), Err(CatalogIntegrity));
    let wrong_facet = vec![tag(TAN_VAN, TagFacet::Theme, "Tản văn")];
    let only_form = MetadataSnapshot::capture(tags(Some(TAN_VAN), &[]));
    assert_eq!(public_tags(&only_form, &wrong_facet), Err(CatalogIntegrity));
}

#[test]
fn version_tokens_start_at_one_and_refuse_to_wrap() {
    for raw in [0, -1, i64::MIN] {
        assert_eq!(MetadataVersion::new(raw), None);
        assert_eq!(CatalogVersion::new(raw), None);
    }
    assert_eq!(MetadataVersion::new(1), Some(MetadataVersion::FIRST));
    assert_eq!(MetadataVersion::FIRST.next().unwrap().get(), 2);
    assert_eq!(CatalogVersion::new(i64::MAX).unwrap().next(), None);
}
