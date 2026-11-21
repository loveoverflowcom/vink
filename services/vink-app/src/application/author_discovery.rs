//! Public author search input, paging, and the shared public author reference.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::discovery::{DiscoveryInput, page_size};
use super::handle::AuthorRef;
use crate::domain::discovery::{CursorError, PageSize, WorkQuery};
use crate::domain::handle;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverAuthors {
    pub q: Option<String>,
    pub limit: Option<String>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthorPage {
    pub authors: Vec<AuthorRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

pub(crate) struct AuthorSearch {
    pub name: Option<WorkQuery>,
    pub handle_pattern: Option<String>,
    pub size: PageSize,
    cursor: Option<String>,
}

impl AuthorSearch {
    pub fn parse(input: DiscoverAuthors) -> Result<Self, DiscoveryInput> {
        let query = input.q.as_deref().unwrap_or_default();
        let query = WorkQuery::parse(query).map_err(DiscoveryInput::Query)?;
        let size = page_size(input.limit.as_deref().filter(|raw| !raw.is_empty()))?;
        let cursor = input.cursor.filter(|raw| !raw.is_empty());
        let (name, handle_pattern) = match query {
            None => (None, Some("%".into())),
            Some(query) => {
                let trimmed = query.text().trim();
                if let Some(address) = trimmed.strip_prefix('@') {
                    let handle = address.to_ascii_lowercase();
                    (None, handle::valid(&handle).then_some(handle))
                } else {
                    let prefix = trimmed.to_ascii_lowercase();
                    let handle_pattern = (prefix.len() <= handle::MAX_HANDLE_BYTES
                        && prefix
                            .bytes()
                            .all(|chr| chr.is_ascii_alphanumeric() || chr == b'-'))
                    .then(|| format!("{prefix}%"));
                    (Some(query), handle_pattern)
                }
            }
        };
        Ok(Self {
            name,
            handle_pattern,
            size,
            cursor,
        })
    }

    pub fn scope(&self, name_pattern: Option<&str>) -> AuthorSearchScope {
        let mut hash = Sha256::new();
        // Both patterns contain public search terms only. The folded name has no NUL.
        hash.update(b"authors-v1\0");
        hash.update(name_pattern.unwrap_or_default());
        hash.update([0]);
        hash.update(self.handle_pattern.as_deref().unwrap_or_default());
        AuthorSearchScope(
            hash.finalize()[..8]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        )
    }

    pub fn after(&self, scope: &AuthorSearchScope) -> Result<&str, DiscoveryInput> {
        let Some(raw) = self.cursor.as_deref() else {
            return Ok("");
        };
        let Some((head, handle)) = raw
            .strip_prefix("authors-v1.")
            .and_then(|raw| raw.split_once('.'))
            .filter(|(head, handle)| {
                head.len() == 16
                    && head.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && handle::valid(handle)
            })
        else {
            return Err(DiscoveryInput::Cursor(CursorError::Malformed));
        };
        if head != scope.0 {
            return Err(DiscoveryInput::CursorScope);
        }
        Ok(handle)
    }
}

pub(crate) struct AuthorSearchScope(String);

impl AuthorSearchScope {
    pub fn page(&self, mut authors: Vec<AuthorRef>, size: PageSize) -> AuthorPage {
        let next_cursor = if authors.len() > size.get() {
            authors.truncate(size.get());
            authors
                .last()
                .map(|author| format!("authors-v1.{}.{}", self.0, author.handle))
        } else {
            None
        };
        AuthorPage {
            authors,
            next_cursor,
        }
    }
}
