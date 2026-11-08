//! Kani 0.67.0 harnesses over the production taxonomy rules, compiled in place. Bounds: a
//! universe of four tag identities (enough for three themes plus one too many and a form),
//! symbolic facets/statuses/catalog membership, all `i64` versions, two principals. Labels in
//! the catalog harness are two concrete ASCII spellings, so Unicode rules are NOT proved here
//! (unit/golden tests cover them); nor are SQL uniqueness, locking, I/O or the adapter's facts.
use uuid::Uuid;

use super::*;
use crate::domain::ids::{PrincipalId, TagId};
use crate::domain::principal::Standing;

fn tag(n: u8) -> TagId {
    TagId::from_uuid(Uuid::from_u128(u128::from(n) + 1))
}

fn principal(n: u8) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(u128::from(n) + 100))
}

fn facet(form: bool) -> TagFacet {
    if form {
        TagFacet::Form
    } else {
        TagFacet::Theme
    }
}

#[kani::proof]
fn taxonomy_version_tokens_are_total() {
    let raw: i64 = kani::any();
    assert_eq!(MetadataVersion::new(raw).is_some(), raw >= 1);
    assert_eq!(CatalogVersion::new(raw).is_some(), raw >= 1);
    if let Some(version) = MetadataVersion::new(raw) {
        match version.next() {
            Some(next) => assert_eq!(next.get(), raw + 1),
            None => assert_eq!(raw, i64::MAX),
        }
        kani::cover!(version.next().is_none());
    }
    if let Some(version) = CatalogVersion::new(raw) {
        assert_eq!(version.next().is_none(), raw == i64::MAX);
    }
}

fn status(active: bool) -> TagStatus {
    if active {
        TagStatus::Active
    } else {
        TagStatus::Retired { successor: None }
    }
}

fn shape(form: Option<u8>, themes: &[u8]) -> (Option<TagId>, Vec<TagId>) {
    (form.map(tag), themes.iter().map(|n| tag(*n)).collect())
}

/// Checks one decision for one concrete (selection, current) shape against symbolic facts.
/// Identities are concrete so every collection has a concrete length; facts and versions are
/// symbolic, which is where the branches are.
fn check_tag_change(
    selection: (Option<u8>, &[u8]),
    current: (Option<u8>, &[u8]),
    catalog: &[TagFacts],
    owns: bool,
    standing: Standing,
    version: i64,
    expected: i64,
) {
    let (form, themes) = shape(selection.0, selection.1);
    let selection = TagSelection::new(form, &themes).unwrap();
    let (current_form, current_themes) = shape(current.0, current.1);
    let current = WorkTags::restore(current_form, current_themes).unwrap();
    let facts = WorkTagFacts {
        owner: if owns { principal(1) } else { principal(2) },
        standing,
        version: MetadataVersion::new(version).unwrap(),
        current: current.clone(),
        catalog: catalog.to_vec(),
    };
    let change = TagChange::new(MetadataVersion::new(expected).unwrap(), selection.clone());
    let outcome = decide_tag_change(principal(1), &change, Some(&facts));
    let admitted = owns && standing == Standing::Active;
    let lookup = |id: TagId| catalog.iter().find(|t| t.id == id).copied();
    // Loops and comparisons run over the concrete inputs (`selection`, `current`) with the
    // decision's output on the right-hand side: a value read out of the merged outcome has a
    // path-dependent length, which the verifier cannot use as a loop bound.
    match &outcome {
        TagOutcome::Accepted(write) => {
            assert!(admitted && expected == version);
            assert_eq!(write.version().get(), version + 1);
            let next = write.tags();
            assert_eq!(selection.form(), next.form());
            assert_eq!(selection.themes(), next.themes());
            assert!(selection.themes().len() <= MAX_THEMES_PER_WORK);
            if let Some(id) = selection.form() {
                let found = lookup(id).unwrap();
                assert_eq!(found.facet, TagFacet::Form);
                assert!(found.status.is_active() || current.contains(id));
                assert!(!selection.themes().contains(&id));
            }
            let mut kept_retired = false;
            for id in selection.themes() {
                let found = lookup(*id).unwrap();
                assert_eq!(found.facet, TagFacet::Theme);
                // A retired tag is never introduced; it may only stay.
                assert!(found.status.is_active() || current.contains(*id));
                kept_retired |= !found.status.is_active();
            }
            assert_eq!(write.changed(), !current.matches(&selection));
            kani::cover!(write.changed());
            kani::cover!(!write.changed());
            kani::cover!(kept_retired);
        }
        TagOutcome::Conflict {
            current: observed_version,
            observed,
        } => {
            assert!(admitted && expected != version);
            assert_eq!(observed_version.get(), version);
            assert_eq!(current, *observed);
        }
        TagOutcome::Refused(TagRefusal::NotFound) => assert!(!owns),
        TagOutcome::Refused(TagRefusal::Suspended) => {
            assert!(owns && standing == Standing::Suspended);
        }
        TagOutcome::Refused(refusal) => {
            // Catalog and bound refusals are decided only for the owner at the current version.
            assert!(admitted && expected == version);
            assert_ne!(*refusal, TagRefusal::InvalidFacts);
            if let TagRefusal::RetiredTag { tag, .. } = refusal {
                assert!(!current.contains(*tag) && !lookup(*tag).unwrap().status.is_active());
            }
            if let TagRefusal::UnknownTag(tag) = refusal {
                assert!(lookup(*tag).is_none());
            }
            if *refusal == TagRefusal::TooManyThemes {
                assert!(selection.themes().len() > MAX_THEMES_PER_WORK);
            }
        }
    }
    if admitted && expected != version {
        assert!(matches!(outcome, TagOutcome::Conflict { .. }));
    }
}

#[kani::proof]
#[kani::unwind(17)]
fn taxonomy_tag_change_respects_owner_version_slots_bounds_and_retired_tags() {
    // Tags 0..=3 are in the catalog with symbolic facet and status; tag 4 is never in it.
    let catalog: [TagFacts; 4] = core::array::from_fn(|n| TagFacts {
        id: tag(n as u8),
        facet: facet(kani::any()),
        status: status(kani::any()),
    });
    let version: i64 = kani::any();
    let expected: i64 = kani::any();
    kani::assume(version >= 1 && expected >= 1);
    let owns: bool = kani::any();
    let standing = if kani::any() {
        Standing::Active
    } else {
        Standing::Suspended
    };
    let cases: [((Option<u8>, &[u8]), (Option<u8>, &[u8])); 7] = [
        ((Some(0), &[1, 2]), (Some(0), &[1, 2])), // same tags: unchanged, still fenced
        ((Some(0), &[1, 2, 3]), (Some(0), &[2])), // three themes; retired ones may stay
        ((None, &[0, 1, 2, 3]), (None, &[1, 3])), // one theme too many
        ((Some(1), &[1]), (None, &[])),           // one identity in both slots
        ((None, &[2, 1, 2]), (None, &[1, 3])),    // deduplicated before the decision
        ((None, &[4]), (None, &[])),              // not in the catalog
        ((Some(3), &[]), (Some(0), &[2])),        // a new form; themes removed
    ];
    for (selection, current) in cases {
        check_tag_change(
            selection, current, &catalog, owns, standing, version, expected,
        );
    }
}

/// What one catalog decision did, as plain values the harness can cover after the outcome is gone.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Seen {
    Accepted { changed: bool, slug_writes: usize },
    Conflict,
    Refused(CatalogRefusal),
}

/// Checks one catalog decision for one concrete change against symbolic facts: the subject's
/// facet and status, the holder of the claimed slug, the successor's facts, authority and
/// versions. The subject's label is "a"; changes use "a", "A" (same slug) or "b".
fn check_catalog_change(change: CatalogChange, subject_present: bool) -> Seen {
    let subject_id = tag(0);
    let other_id = tag(1);
    let a = TagLabel::parse("a").unwrap();
    let subject = if subject_present {
        CatalogTag::restore(
            subject_id,
            facet(kani::any()),
            a.clone(),
            a.slug(),
            None,
            status(kani::any()),
        )
    } else {
        None
    };
    let claimed = change.claimed_slug();
    let slug_holder = if claimed.is_some() && kani::any() {
        let kind = match kani::any::<u8>() % 3 {
            0 => SlugKind::Canonical,
            1 => SlugKind::Alias,
            _ => SlugKind::Former,
        };
        Some(SlugHolder {
            tag: if kani::any() { subject_id } else { other_id },
            kind,
        })
    } else {
        None
    };
    let successor = change.successor().and_then(|id| {
        kani::any::<bool>().then(|| TagFacts {
            id,
            facet: facet(kani::any()),
            status: status(kani::any()),
        })
    });
    let version: i64 = kani::any();
    let expected: i64 = kani::any();
    kani::assume(version >= 1 && expected >= 1);
    let can_curate: bool = kani::any();
    let facts = CatalogFacts {
        can_curate,
        version: CatalogVersion::new(version).unwrap(),
        subject: subject.clone(),
        slug_holder,
        successor,
    };
    let command = CatalogCommand {
        expected: CatalogVersion::new(expected).unwrap(),
        change: change.clone(),
    };
    let outcome = decide_catalog_change(&command, &facts);
    let subject_active = subject.as_ref().is_some_and(|t| t.status().is_active());
    match &outcome {
        CatalogOutcome::Accepted(write) => {
            assert!(can_curate && expected == version);
            assert_eq!(write.version().get(), version + 1);
            let slugs = write.slugs();
            assert!(slugs.len() <= 2);
            for i in 0..2 {
                if i >= slugs.len() {
                    break;
                }
                let slug = &slugs[i];
                // No write names another tag.
                assert_eq!(subject_id, slug.tag);
                if i == 0 {
                    // The first write claims the change's slug: a new row, or a row that
                    // already named this same tag (never its canonical one).
                    assert_eq!(claimed.as_ref(), Some(&slug.slug));
                    match slug_holder {
                        None => assert!(slug.replaces.is_none()),
                        Some(holder) => {
                            assert_eq!(subject_id, holder.tag);
                            assert_ne!(SlugKind::Canonical, holder.kind);
                            assert_eq!(Some(holder.kind), slug.replaces);
                        }
                    }
                } else {
                    // The second write only demotes the subject's previous canonical slug.
                    assert_eq!(a.slug(), slug.slug);
                    assert_eq!(NameKind::Former, slug.kind);
                    assert_eq!(Some(SlugKind::Canonical), slug.replaces);
                }
            }
            match &change {
                CatalogChange::Create { .. } => {
                    assert!(subject.is_none() && slug_holder.is_none());
                }
                CatalogChange::AddAlias { label, .. } => {
                    assert!(subject_active);
                    assert_ne!(a.slug(), label.slug());
                }
                CatalogChange::Retire { successor, .. } => {
                    assert!(subject_active);
                    if let Some(named) = successor {
                        let next = facts.successor.unwrap();
                        assert!(*named != subject_id && next.status.is_active());
                        assert_eq!(subject.as_ref().unwrap().facet(), next.facet);
                    }
                }
                CatalogChange::Rename { .. } | CatalogChange::Describe { .. } => {
                    assert!(subject_active);
                }
            }
        }
        CatalogOutcome::Conflict { current } => {
            assert!(can_curate && expected != version);
            assert_eq!(current.get(), version);
        }
        CatalogOutcome::Refused(CatalogRefusal::NotCurator) => assert!(!can_curate),
        CatalogOutcome::Refused(refusal) => {
            assert!(can_curate);
            if let CatalogRefusal::SlugTaken { holder } = refusal {
                assert_eq!(slug_holder.map(|h| h.tag), Some(*holder));
            }
        }
    }
    let seen = match &outcome {
        CatalogOutcome::Accepted(write) => Seen::Accepted {
            changed: write.changed(),
            slug_writes: write.slugs().len(),
        },
        CatalogOutcome::Conflict { .. } => Seen::Conflict,
        CatalogOutcome::Refused(refusal) => Seen::Refused(*refusal),
    };
    // The outcome's slug list has a path-dependent length; dropping it would make the verifier
    // unroll the element destructor to the bound. Leaking it in a proof changes no assertion.
    core::mem::forget(outcome);
    seen
}

fn label(raw: &str) -> TagLabel {
    TagLabel::parse(raw).unwrap()
}

fn taken(seen: Seen) -> bool {
    matches!(seen, Seen::Refused(CatalogRefusal::SlugTaken { .. }))
}

// One harness per kind of change keeps each verifier run small (a single harness over all nine
// changes needed 36 minutes and 10.9 GB).

#[kani::proof]
#[kani::unwind(17)]
fn taxonomy_catalog_create_and_describe_claim_only_free_slugs() {
    let id = tag(0);
    let create = check_catalog_change(
        CatalogChange::Create {
            id,
            facet: facet(kani::any()),
            label: label("a"),
            description: None,
        },
        kani::any(),
    );
    kani::cover!(
        create
            == Seen::Accepted {
                changed: true,
                slug_writes: 1
            }
    );
    kani::cover!(taken(create));
    kani::cover!(create == Seen::Refused(CatalogRefusal::TagIdTaken));
    let describe = check_catalog_change(
        CatalogChange::Describe {
            id,
            description: None,
        },
        kani::any(),
    );
    kani::cover!(matches!(describe, Seen::Accepted { changed: false, .. }));
    kani::cover!(describe == Seen::Refused(CatalogRefusal::UnknownTag));
}

#[kani::proof]
#[kani::unwind(17)]
fn taxonomy_catalog_rename_keeps_identity_and_never_takes_another_tags_slug() {
    let id = tag(0);
    let renamed = check_catalog_change(
        CatalogChange::Rename {
            id,
            label: label("b"),
        },
        true,
    );
    kani::cover!(
        renamed
            == Seen::Accepted {
                changed: true,
                slug_writes: 2
            }
    );
    kani::cover!(taken(renamed));
    let respelled = check_catalog_change(
        CatalogChange::Rename {
            id,
            label: label("A"),
        },
        true,
    );
    kani::cover!(
        respelled
            == Seen::Accepted {
                changed: true,
                slug_writes: 0
            }
    );
    kani::cover!(respelled == Seen::Refused(CatalogRefusal::Retired));
}

#[kani::proof]
#[kani::unwind(17)]
fn taxonomy_catalog_aliases_point_only_at_active_canonical_tags() {
    let target = tag(0);
    let alias = check_catalog_change(
        CatalogChange::AddAlias {
            target,
            label: label("b"),
        },
        kani::any(),
    );
    kani::cover!(
        alias
            == Seen::Accepted {
                changed: true,
                slug_writes: 1
            }
    );
    kani::cover!(taken(alias));
    kani::cover!(alias == Seen::Refused(CatalogRefusal::UnknownTag));
    let own_name = check_catalog_change(
        CatalogChange::AddAlias {
            target,
            label: label("A"),
        },
        true,
    );
    kani::cover!(own_name == Seen::Refused(CatalogRefusal::AliasIsCanonicalName));
}

#[kani::proof]
#[kani::unwind(17)]
fn taxonomy_catalog_retirement_validates_the_successor_hint() {
    let id = tag(0);
    let plain = check_catalog_change(
        CatalogChange::Retire {
            id,
            successor: None,
        },
        true,
    );
    kani::cover!(
        plain
            == Seen::Accepted {
                changed: true,
                slug_writes: 0
            }
    );
    let own = check_catalog_change(
        CatalogChange::Retire {
            id,
            successor: Some(id),
        },
        true,
    );
    kani::cover!(
        own == Seen::Refused(CatalogRefusal::InvalidSuccessor(
            SuccessorError::SelfReference
        ))
    );
    let other = check_catalog_change(
        CatalogChange::Retire {
            id,
            successor: Some(tag(1)),
        },
        true,
    );
    kani::cover!(matches!(other, Seen::Accepted { .. }));
    kani::cover!(
        other == Seen::Refused(CatalogRefusal::InvalidSuccessor(SuccessorError::OtherFacet))
    );
    kani::cover!(other == Seen::Refused(CatalogRefusal::InvalidSuccessor(SuccessorError::Unknown)));
}
