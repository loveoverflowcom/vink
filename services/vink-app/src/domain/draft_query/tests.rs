use super::*;
use crate::domain::document::Title;
use crate::domain::draft::DraftVersion;
use time::macros::datetime;

fn owner() -> PrincipalId {
    PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap()
}
fn card(title: &str) -> DraftSummary {
    DraftSummary {
        work_id: WorkId::generate(),
        title: Title::parse(title).unwrap(),
        version: DraftVersion::FIRST,
        updated_at: datetime!(2026-11-15 00:00:00.123456 UTC),
    }
}

#[test]
fn draft_search_keeps_web_semantics_including_punctuation_and_untitled() {
    for (raw, key) in [
        ("  ĐƯỜNG\u{a0}về  ", "duong ve"),
        ("Đường về".nfd().collect::<String>().as_str(), "duong ve"),
        ("a_%—b", "a_%—b"),
        ("ΟΣ", "οσ"),
        ("\u{345}\u{1ab0}", ""),
    ] {
        assert_eq!(search_key(raw), key);
    }
    assert_eq!(title_key(""), "chua dat ten");
    let original = card("Đường về");
    assert_eq!(original.title.as_str(), "Đường về");
}

#[test]
fn draft_page_is_finite_and_cursor_bound_to_owner_query_sort_limit_and_version() {
    for sort in ["recent", "title"] {
        let query = DraftSearch::parse(owner(), "Đường", Some(sort), Some("1"), None).unwrap();
        let a = card("Đường về");
        assert!(query.page(vec![a.clone()]).next_cursor.is_none());
        let page = query.page(vec![a.clone(), card("Đường đi")]);
        assert_eq!(page.drafts.as_slice(), std::slice::from_ref(&a));
        let cursor = page.next_cursor.unwrap();
        let continued =
            DraftSearch::parse(owner(), "duong", Some(sort), Some("1"), Some(&cursor)).unwrap();
        match continued.after().unwrap() {
            DraftPosition::Recent {
                updated_at,
                work_id,
            } => {
                assert_eq!(*updated_at, a.updated_at);
                assert_eq!(*work_id, a.work_id);
            }
            DraftPosition::Title { key, work_id } => {
                assert_eq!(key, "duong ve");
                assert_eq!(*work_id, a.work_id);
            }
        }
        for (actor, q, s, l) in [
            (
                PrincipalId::from_uuid(uuid::Uuid::nil()),
                "duong",
                sort,
                "1",
            ),
            (owner(), "khac", sort, "1"),
            (
                owner(),
                "duong",
                if sort == "title" { "recent" } else { "title" },
                "1",
            ),
            (owner(), "duong", sort, "2"),
        ] {
            assert_eq!(
                DraftSearch::parse(actor, q, Some(s), Some(l), Some(&cursor)),
                Err(DraftQueryError::Cursor)
            );
        }
        for bad in [
            format!("02{}", &cursor[2..]),
            format!("{}ff", cursor),
            cursor[..99].to_string(),
            "x".repeat(MAX_CURSOR_BYTES + 1),
        ] {
            assert_eq!(
                DraftSearch::parse(owner(), "duong", Some(sort), Some("1"), Some(&bad)),
                Err(DraftQueryError::Cursor)
            );
        }
    }
}

#[test]
fn draft_query_limits_validate_before_store() {
    assert_eq!(
        DraftSearch::parse(owner(), "", None, None, None)
            .unwrap()
            .size(),
        100
    );
    for raw in ["0", "101", "-1", "+1", "", "1.5", "18446744073709551616"] {
        assert_eq!(
            DraftSearch::parse(owner(), "", None, Some(raw), None),
            Err(DraftQueryError::Limit)
        );
    }
    for raw in [
        "x".repeat(201),
        "\0".into(),
        "\u{301}".repeat(MAX_QUERY_BYTES),
    ] {
        assert_eq!(
            DraftSearch::parse(owner(), &raw, None, None, None),
            Err(DraftQueryError::Query)
        );
    }
    assert_eq!(
        DraftSearch::parse(owner(), "", Some("Title"), None, None),
        Err(DraftQueryError::Sort)
    );
}
