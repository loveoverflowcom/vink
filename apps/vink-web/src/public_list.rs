//! Presentation rules for the finite public collection; no new reads or access authority.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use crate::contract::WorkCard;
use crate::search::search_key;

// RFC 3986 unreserved characters; identical in SSR, WASM and native tests.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub fn visible_works(works: &[WorkCard], query: &str) -> Vec<WorkCard> {
    let query = search_key(query);
    works
        .iter()
        .filter(|work| {
            search_key(&work.title).contains(&query)
                || search_key(&work.author_display_name).contains(&query)
        })
        .cloned()
        .collect()
}

fn query_string(query: &str) -> String {
    if query.is_empty() {
        String::new()
    } else {
        format!("?q={}", utf8_percent_encode(query, COMPONENT))
    }
}

pub fn work_anchor(work_id: &str) -> String {
    if !work_id.is_empty()
        && work_id.len() <= 64
        && work_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        format!("work-{work_id}")
    } else {
        "community-stream".to_string()
    }
}

/// Construct only a local Explore URL. There is deliberately no arbitrary return target.
pub fn explore_href(query: &str, work_id: Option<&str>) -> String {
    let mut href = format!("/{}", query_string(query));
    if let Some(id) = work_id {
        href.push('#');
        href.push_str(&work_anchor(id));
    }
    href
}

pub fn reader_href(work_id: &str, query: &str) -> String {
    format!(
        "/tac-pham/{}{}",
        utf8_percent_encode(work_id, COMPONENT),
        query_string(query)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_normalization::UnicodeNormalization;

    fn card(id: &str, title: &str, author: &str) -> WorkCard {
        WorkCard {
            work_id: id.into(),
            title: title.into(),
            author_display_name: author.into(),
            published_at: "2026-11-01T08:30:00Z".into(),
        }
    }

    #[test]
    fn public_search_matches_real_title_or_author_and_preserves_order_and_text() {
        let works = vec![
            card("b", "Đường về có nắng", "Nguyễn An"),
            card("a", "Một ngày", &"ĐẶNG THÙY".nfd().collect::<String>()),
            card(
                "c",
                &"ĐƯỜNG VỀ CÓ MƯA".nfd().collect::<String>(),
                "Đặng Thy",
            ),
        ];
        let found = visible_works(&works, "  duong   ve ");
        assert_eq!(
            found.iter().map(|w| w.work_id.as_str()).collect::<Vec<_>>(),
            ["b", "c"]
        );
        assert_eq!(found[1], works[2]);
        assert_eq!(visible_works(&works, "đặng thuỳ"), [works[1].clone()]);
        assert_eq!(
            visible_works(&works, &"NGUYỄN AN".nfd().collect::<String>()),
            [works[0].clone()]
        );
        assert_eq!(visible_works(&works, " \n "), works);
        assert!(visible_works(&works, "thể loại chưa có").is_empty());
        assert!(visible_works(&[], "Đường").is_empty());
    }

    #[test]
    fn context_links_encode_query_once_and_cannot_redirect_outside_explore() {
        let id = "33333333-3333-4333-8333-333333333333";
        assert_eq!(explore_href("", None), "/");
        assert_eq!(
            explore_href("Đ & 100% + #", Some(id)),
            format!("/?q=%C4%90%20%26%20100%25%20%2B%20%23#work-{id}")
        );
        let reader = reader_href(id, "%2F?return=https://outside.example/");
        assert!(reader.starts_with(&format!("/tac-pham/{id}")));
        assert!(reader.ends_with("?q=%252F%3Freturn%3Dhttps%3A%2F%2Foutside.example%2F"));
        assert_eq!(
            explore_href("//outside.example", Some("//outside.example/#")),
            "/?q=%2F%2Foutside.example#community-stream"
        );
    }

    /// The router's own server-side URL parser reads back exactly what these links encode.
    #[cfg(feature = "ssr")]
    #[test]
    fn return_context_round_trips_through_the_router_and_malformed_input_stays_local() {
        use leptos_router::location::RequestUrl;

        let id = "33333333-3333-4333-8333-333333333333";
        let decomposed = "Đường về".nfd().collect::<String>();
        for query in [
            "",
            "duong ve",
            "Đ & 100% + #",
            "a=b&q=c",
            "?returnTo=https://outside.example/",
            "/tac-pham/x?y#z",
            decomposed.as_str(),
        ] {
            for (href, path) in [
                (explore_href(query, Some(id)), "/".to_string()),
                (reader_href(id, query), format!("/tac-pham/{id}")),
            ] {
                let url = RequestUrl::new(&href).parse().expect(&href);
                assert_eq!(url.path(), path, "{href}");
                assert_eq!(
                    url.search_params().get_str("q").unwrap_or_default(),
                    query,
                    "{href}"
                );
                assert_eq!(url.search_params().get_str("returnTo"), None, "{href}");
            }
        }
        // Malformed or hostile deep links decode without panicking and build only local links.
        for raw in [
            "/?q=%E0%A4%A",
            "/?q=%",
            "/?q=%2F%2Foutside.example",
            "/?q=a%00b",
        ] {
            let url = RequestUrl::new(raw).parse().expect(raw);
            let query = url.search_params().get("q").unwrap_or_default();
            let back = explore_href(&query, Some("//outside.example"));
            assert!(
                back.starts_with("/?q=") && !back.starts_with("//"),
                "{raw} → {back}"
            );
            assert!(back.ends_with("#community-stream"), "{back}");
        }
    }
}
