//! URL and request identity for Khám phá. No catalog authority or local corpus filter.
use crate::contract::WorkList;
use crate::public_list::work_anchor;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

const COMPONENT: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub fn encode(value: &str) -> String {
    utf8_percent_encode(value, COMPONENT).to_string()
}

/// Only the URL spelling equivalence from TagSlug::parse (NFC + lowercase). This is
/// not accent folding, validation, alias lookup or tag identity; the provider owns those.
pub fn requested_slug(raw: &str) -> String {
    raw.nfc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExploreQuery {
    pub q: String,
    pub tags: Vec<String>,
    pub cursor: Option<String>,
}

impl ExploreQuery {
    /// Normalize representation only. Validation and tag identity remain server-owned.
    pub fn new(q: String, mut tags: Vec<String>, cursor: Option<String>) -> Self {
        tags.retain(|t| !t.is_empty());
        tags.sort();
        tags.dedup();
        Self {
            q,
            tags,
            cursor: cursor.filter(|c| !c.is_empty()),
        }
    }
    pub fn search(&self) -> String {
        let mut pairs = Vec::new();
        if !self.q.is_empty() {
            pairs.push(format!("q={}", encode(&self.q)));
        }
        pairs.extend(self.tags.iter().map(|t| format!("tag={}", encode(t))));
        if let Some(cursor) = &self.cursor {
            pairs.push(format!("cursor={}", encode(cursor)));
        }
        if pairs.is_empty() {
            String::new()
        } else {
            format!("?{}", pairs.join("&"))
        }
    }
    pub fn api_path(&self) -> String {
        format!("/api/v1/works{}", self.search())
    }
    pub fn first(&self) -> Self {
        Self {
            cursor: None,
            ..self.clone()
        }
    }
    pub fn without_tag(&self, slug: &str) -> Self {
        Self::new(
            self.q.clone(),
            self.tags
                .iter()
                .filter(|t| requested_slug(t) != requested_slug(slug))
                .cloned()
                .collect(),
            None,
        )
    }
    pub fn canonical(&self, list: &WorkList) -> Self {
        Self::new(
            list.filters.q.clone().unwrap_or_default(),
            list.filters
                .tags
                .iter()
                .map(|filter| {
                    filter
                        .tag
                        .as_ref()
                        .filter(|t| t.status == "active")
                        .map(|t| t.slug.clone())
                        .unwrap_or_else(|| filter.requested.clone())
                })
                .collect(),
            self.cursor.clone(),
        )
    }
}

/// The route owns the mode, each mode owns its own query and results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExploreMode {
    #[default]
    Works,
    Following,
    Authors,
}

impl ExploreMode {
    pub const FOLLOWING_HREF: &str = "/?mode=following";
    pub const AUTHORS_HREF: &str = "/?mode=authors";

    pub fn from_param(value: Option<&str>) -> Self {
        match value {
            Some("following") => Self::Following,
            Some("authors") => Self::Authors,
            _ => Self::Works,
        }
    }
}

pub(crate) fn author_search_href(q: &str, cursor: Option<&str>) -> String {
    let mut href = ExploreMode::AUTHORS_HREF.to_owned();
    if !q.trim().is_empty() {
        href.push_str("&q=");
        href.push_str(&encode(q));
    }
    if let Some(cursor) = cursor {
        href.push_str("&cursor=");
        href.push_str(&encode(cursor));
    }
    href
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreContext {
    #[serde(default)]
    pub mode: ExploreMode,
    pub query: ExploreQuery,
    pub tag_page: Option<String>,
}

impl ExploreContext {
    pub fn href(&self, work: Option<&str>) -> String {
        let mut query = self.query.clone();
        let base = match &self.tag_page {
            Some(slug) => {
                query
                    .tags
                    .retain(|tag| requested_slug(tag) != requested_slug(slug));
                format!("/the/{}", encode(slug))
            }
            None => "/".into(),
        };
        let mut href = match self.mode {
            ExploreMode::Works => format!("{base}{}", query.search()),
            ExploreMode::Following => ExploreMode::FOLLOWING_HREF.to_owned(),
            ExploreMode::Authors => author_search_href(&query.q, query.cursor.as_deref()),
        };
        if let Some(work) = work {
            href.push('#');
            href.push_str(&work_anchor(work));
        }
        href
    }
    /// Only the supported mode and its public URL fields carry over to Reader.
    pub fn reader_href(&self, id: &str) -> String {
        if self.mode == ExploreMode::Following {
            return format!("/tac-pham/{}?mode=following", encode(id));
        }
        let mut query = self.query.search();
        if let Some(slug) = &self.tag_page {
            query.push(if query.is_empty() { '?' } else { '&' });
            query.push_str(&format!("from_tag={}", encode(slug)));
        }
        format!("/tac-pham/{}{query}", encode(id))
    }
    pub fn canonical(&self, list: &WorkList) -> Self {
        let tag_page = self.tag_page.as_ref().map(|slug| {
            list.filters
                .tags
                .iter()
                .find(|f| f.requested == requested_slug(slug))
                .and_then(|f| f.tag.as_ref())
                .filter(|t| t.status == "active")
                .map(|t| t.slug.clone())
                .unwrap_or_else(|| requested_slug(slug))
        });
        Self {
            mode: self.mode,
            query: self.query.canonical(list),
            tag_page,
        }
    }
}

pub fn tag_href(slug: &str) -> String {
    format!("/the/{}", encode(slug))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_context_encodes_once_preserves_all_filters_and_page_and_stays_local() {
        let context = ExploreContext {
            mode: ExploreMode::Works,
            query: ExploreQuery::new(
                "Đ & 100% + #".into(),
                vec!["ky-uc".into(), "tho".into()],
                Some("opaque+/?".into()),
            ),
            tag_page: Some("tho".into()),
        };
        assert_eq!(
            context.href(None),
            "/the/tho?q=%C4%90%20%26%20100%25%20%2B%20%23&tag=ky-uc&cursor=opaque%2B%2F%3F"
        );
        assert!(context.reader_href("id").ends_with("&from_tag=tho"));
        assert!(context.href(Some("id")).ends_with("#work-id"));
        let hostile = ExploreContext {
            tag_page: Some("//outside/a?x=y".into()),
            ..context
        };
        assert!(
            hostile
                .href(None)
                .starts_with("/the/%2F%2Foutside%2Fa%3Fx%3Dy?")
        );
        assert!(!hostile.query.first().search().contains("cursor="));
        assert_eq!(hostile.query.without_tag("tho").tags, ["ky-uc"]);
    }
    #[test]
    fn following_mode_round_trips_without_public_query_or_private_cursor() {
        let context = ExploreContext {
            mode: ExploreMode::from_param(Some("following")),
            query: ExploreQuery::new("mưa".into(), vec!["tho".into()], Some("opaque".into())),
            tag_page: Some("tho".into()),
        };
        assert_eq!(context.href(Some("abc")), "/?mode=following#work-abc");
        assert_eq!(context.reader_href("abc"), "/tac-pham/abc?mode=following");
    }

    #[test]
    fn keys_differ_for_query_filter_and_page_and_are_order_independent() {
        let base = ExploreQuery::default();
        let a = ExploreQuery::new("mưa".into(), vec!["ky-uc".into(), "tho".into()], None);
        let b = ExploreQuery::new(
            "mưa".into(),
            vec!["tho".into(), "ky-uc".into(), "tho".into()],
            None,
        );
        assert_eq!(a, b);
        assert_ne!(base.api_path(), a.api_path());
        assert_ne!(
            a.api_path(),
            ExploreQuery {
                cursor: Some("next".into()),
                ..a.clone()
            }
            .api_path()
        );
        assert_ne!(a.api_path(), a.without_tag("tho").api_path());
    }
    #[test]
    fn authoritative_removal_fences_old_responses_in_every_retained_search() {
        use crate::freshness::{Applied, Outcome, Policy, Store, Trigger};
        let a = ExploreQuery::new("a".into(), vec!["thơ".into()], None);
        let b = ExploreQuery::new("b".into(), vec![], Some("page".into()));
        let mut store = Store::new(Policy::V1);
        store.seed(&a, vec!["withdrawn", "a-only"], 0.0, 0.0);
        store.seed(&b, vec!["withdrawn", "b-only"], 0.0, 0.0);
        let old_a = store.begin(&a, Trigger::Manual, 1.0).unwrap();
        let old_b = store.begin(&b, Trigger::Manual, 1.0).unwrap();
        let c = ExploreQuery::new("not-loaded-yet".into(), vec![], None);
        let crate::freshness::Load::Fetch(old_c) = store.load(&c, 1.0) else {
            panic!("must fetch");
        };
        store.mark_stale_where(|_| true);
        store.edit_all(|_, works| works.retain(|id| *id != "withdrawn"));
        assert_eq!(
            store.complete(&c, old_c, Outcome::Fresh(vec!["withdrawn"]), 2.0, 0.0),
            Applied::Discarded
        );
        assert_eq!(
            store.complete(&a, old_a, Outcome::Fresh(vec!["withdrawn"]), 2.0, 0.0),
            Applied::Discarded
        );
        assert_eq!(
            store.complete(&b, old_b, Outcome::Fresh(vec!["withdrawn"]), 2.0, 0.0),
            Applied::Discarded
        );
        assert_eq!(store.status(&a).unwrap().data.unwrap(), ["a-only"]);
        assert_eq!(store.status(&b).unwrap().data.unwrap(), ["b-only"]);
    }
    #[test]
    fn uppercase_nfd_route_uses_the_providers_canonical_identity_without_folding_accents() {
        use crate::contract::{TagFilter, TagList, WorkFilters};
        let catalog: TagList = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/tags.list.json"
        ))
        .unwrap();
        let tag = catalog.tags.into_iter().find(|t| t.label == "Thơ").unwrap();
        let raw: String = "THƠ".nfd().collect();
        let context = ExploreContext {
            mode: ExploreMode::Works,
            query: ExploreQuery::new(String::new(), vec![raw.clone()], None),
            tag_page: Some(raw.clone()),
        };
        let list = WorkList {
            works: vec![],
            next_cursor: None,
            filters: WorkFilters {
                q: None,
                tags: vec![TagFilter {
                    requested: "thơ".into(),
                    resolution: "canonical".into(),
                    tag: Some(tag),
                }],
            },
        };
        assert_eq!(context.canonical(&list).href(None), "/the/th%C6%A1");
        assert!(context.query.without_tag("thơ").tags.is_empty());
        assert_ne!(requested_slug("má"), requested_slug("ma"));
        assert_eq!(requested_slug(&"KÝ-ỨC".nfd().collect::<String>()), "ký-ức");
    }
}
