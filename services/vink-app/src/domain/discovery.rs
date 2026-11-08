//! Public discovery (TAG-PR-04, #98): what a reader's search of the public library means,
//! independent of SQL and HTTP. Choices, contract and evidence: docs/development/discovery-api.md.
//!
//! A search has three optional parts: a text query over the public title and the author's
//! display name, a set of catalog tags that must ALL be on the work's public snapshot, and a
//! position after which the next page starts. Every page is finite and newest first
//! (`published_at` descending, then `work_id` ascending); there is no total count.
//!
//! The text query keeps the reader's spelling for display; only its search key (the same
//! [`SearchKey`] as tag lookup: NFC/NFD, `đ`/`Đ`, marks and case folded) decides a match. A tag
//! filter is a list of slugs: each resolves through the catalog (`resolve_slug`) to the one tag
//! it names forever, so an alias, a former name and the canonical slug filter the same works.
//! Only active tags are public facets (retired tags are neither shown nor linked, #95), so a slug
//! that names no tag or a retired tag matches no work: the answer says why, it is not an error.
//!
//! A cursor is the last position of a page plus a digest of the normalized search (query key and
//! canonical tag identities) it belongs to. It is not a capability: forging one only moves the
//! position inside the same public result, which visibility predicates already bound. A cursor
//! from another search is refused rather than silently applied to the wrong list.

use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use unicode_normalization::UnicodeNormalization;

use super::ids::{TagId, WorkId};
use super::taxonomy::{MAX_THEMES_PER_WORK, SearchKey, SlugResolution, TagSlug};

/// Works on one page when the request names no `limit`: the finite home page of ED-01.
pub const DEFAULT_PAGE_SIZE: usize = 12;
/// Most works on one page.
pub const MAX_PAGE_SIZE: usize = 24;
/// Most distinct filter slugs: a work holds one form and at most three themes, so more tags
/// can never all match one work.
pub const MAX_FILTER_TAGS: usize = 1 + MAX_THEMES_PER_WORK;
/// Raw `tag` values accepted before normalizing and removing duplicates (each slug twice).
pub const MAX_FILTER_REFS: usize = 2 * MAX_FILTER_TAGS;
/// Most characters of a text query after NFC: half of the longest title, enough for a title
/// fragment or a pen name.
pub const MAX_QUERY_CHARS: usize = 100;
/// Raw UTF-8 bytes accepted before normalizing (fully decomposed Vietnamese is up to five
/// bytes per letter).
pub const MAX_QUERY_BYTES: usize = 16 * MAX_QUERY_CHARS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkQueryError {
    #[error("q exceeds {MAX_QUERY_CHARS} characters")]
    TooLong,
    #[error("q has no letter or digit")]
    NoLetterOrDigit,
}

/// A reader's text query. `text` is kept exactly as received for display and echo; `key`
/// decides matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkQuery {
    text: String,
    key: SearchKey,
}

impl WorkQuery {
    /// `Ok(None)` for blank input: an empty search box submits `q=`, which means "no query".
    pub fn parse(raw: &str) -> Result<Option<Self>, WorkQueryError> {
        if raw.len() > MAX_QUERY_BYTES {
            return Err(WorkQueryError::TooLong);
        }
        if raw.trim().is_empty() {
            return Ok(None);
        }
        let normalized: String = raw.nfc().collect();
        if normalized.chars().count() > MAX_QUERY_CHARS {
            return Err(WorkQueryError::TooLong);
        }
        let key = SearchKey::of(&normalized);
        if key.is_empty() {
            return Err(WorkQueryError::NoLetterOrDigit);
        }
        Ok(Some(Self {
            text: raw.to_owned(),
            key,
        }))
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn key(&self) -> &SearchKey {
        &self.key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FilterError {
    #[error("at most {MAX_FILTER_TAGS} distinct tags can filter one search")]
    TooMany,
    #[error("tag is not a slug")]
    Malformed,
}

/// Requested filter slugs: normalized (NFC, lowercase), blank values ignored, duplicates
/// removed in first-seen order. Two slugs may still name one tag (an alias and its tag); the
/// catalog decides that, not spelling.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagFilter(Vec<TagSlug>);

impl TagFilter {
    pub fn parse<'a>(raw: impl IntoIterator<Item = &'a str>) -> Result<Self, FilterError> {
        let mut slugs: Vec<TagSlug> = Vec::new();
        for (index, value) in raw.into_iter().enumerate() {
            if index >= MAX_FILTER_REFS {
                return Err(FilterError::TooMany);
            }
            if value.trim().is_empty() {
                continue;
            }
            let slug = TagSlug::parse(value).map_err(|_| FilterError::Malformed)?;
            if !slugs.contains(&slug) {
                slugs.push(slug);
            }
        }
        if slugs.len() > MAX_FILTER_TAGS {
            return Err(FilterError::TooMany);
        }
        Ok(Self(slugs))
    }

    pub fn slugs(&self) -> &[TagSlug] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The tags a search is constrained to (sorted, distinct), or `None` when no public work can
/// match: some slug names no tag or a retired one. `resolutions` are those of the filter's
/// slugs, read in the same snapshot as the search.
pub fn filter_tags(resolutions: &[SlugResolution<'_>]) -> Option<Vec<TagId>> {
    let mut tags = Vec::with_capacity(resolutions.len());
    for resolution in resolutions {
        match resolution {
            SlugResolution::Canonical(tag) | SlugResolution::Redirect(tag) => tags.push(tag.id()),
            SlugResolution::Retired(_) | SlugResolution::NotFound => return None,
        }
    }
    tags.sort();
    tags.dedup();
    Some(tags)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageSizeError {
    #[error("limit must be between 1 and {MAX_PAGE_SIZE}")]
    OutOfRange,
}

/// Works per page, `1..=MAX_PAGE_SIZE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSize(usize);

impl PageSize {
    pub const DEFAULT: Self = Self(DEFAULT_PAGE_SIZE);

    pub fn new(raw: i64) -> Result<Self, PageSizeError> {
        usize::try_from(raw)
            .ok()
            .filter(|n| (1..=MAX_PAGE_SIZE).contains(n))
            .map(Self)
            .ok_or(PageSizeError::OutOfRange)
    }

    pub const fn get(self) -> usize {
        self.0
    }

    /// Rows to read: one more than a page tells whether another page exists.
    pub const fn fetch(self) -> usize {
        self.0 + 1
    }
}

/// Where a work sits in the newest-first order. Storage keeps microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePosition {
    pub published_at: OffsetDateTime,
    pub work_id: WorkId,
}

/// Digest of a normalized search: the query key and the canonical tag identities. Two requests
/// with the same digest list the same works in the same order, whatever spelling, alias or
/// order of filters they used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchScope([u8; 8]);

impl SearchScope {
    /// `tags` as returned by [`filter_tags`] (sorted, distinct).
    pub fn of(query: Option<&SearchKey>, tags: &[TagId]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-discovery-scope-v1");
        match query {
            None => hasher.update([0]),
            Some(key) => {
                hasher.update([1]);
                hasher.update((key.as_str().len() as u64).to_be_bytes());
                hasher.update(key.as_str().as_bytes());
            }
        }
        hasher.update((tags.len() as u64).to_be_bytes());
        for tag in tags {
            hasher.update(tag.as_uuid().as_bytes());
        }
        let digest = hasher.finalize();
        let mut scope = [0; 8];
        scope.copy_from_slice(&digest[..8]);
        Self(scope)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CursorError {
    #[error("cursor is malformed")]
    Malformed,
}

const CURSOR_VERSION: u8 = 1;
const CURSOR_BYTES: usize = 1 + 8 + 16 + 8;

/// Position after which the next page starts, bound to the search it came from. Wire form:
/// lowercase hex of version (1), `published_at` as Unix microseconds (i64 big-endian),
/// `work_id` (16 bytes) and the [`SearchScope`] (8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryCursor {
    after: PagePosition,
    scope: SearchScope,
}

impl DiscoveryCursor {
    pub fn new(after: PagePosition, scope: SearchScope) -> Self {
        Self { after, scope }
    }

    pub const fn after(&self) -> PagePosition {
        self.after
    }

    pub const fn scope(&self) -> SearchScope {
        self.scope
    }

    pub fn encode(&self) -> String {
        let micros = self
            .after
            .published_at
            .unix_timestamp_nanos()
            .div_euclid(1000);
        // Storage dates are within i64 microseconds by a wide margin; saturate rather than wrap.
        let micros = i64::try_from(micros).unwrap_or(if micros < 0 { i64::MIN } else { i64::MAX });
        let mut bytes = Vec::with_capacity(CURSOR_BYTES);
        bytes.push(CURSOR_VERSION);
        bytes.extend_from_slice(&micros.to_be_bytes());
        bytes.extend_from_slice(self.after.work_id.as_uuid().as_bytes());
        bytes.extend_from_slice(&self.scope.0);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(raw: &str) -> Result<Self, CursorError> {
        if raw.len() != 2 * CURSOR_BYTES {
            return Err(CursorError::Malformed);
        }
        let mut bytes = [0u8; CURSOR_BYTES];
        for (index, pair) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[index] = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
        }
        if bytes[0] != CURSOR_VERSION {
            return Err(CursorError::Malformed);
        }
        let micros =
            i64::from_be_bytes(bytes[1..9].try_into().map_err(|_| CursorError::Malformed)?);
        let published_at = OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1000)
            .map_err(|_| CursorError::Malformed)?;
        let work_id = WorkId::from_uuid(uuid::Uuid::from_bytes(
            bytes[9..25]
                .try_into()
                .map_err(|_| CursorError::Malformed)?,
        ));
        let scope = SearchScope(bytes[25..].try_into().map_err(|_| CursorError::Malformed)?);
        Ok(Self {
            after: PagePosition {
                published_at,
                work_id,
            },
            scope,
        })
    }
}

/// One canonical spelling: lowercase hex only.
fn hex_digit(byte: u8) -> Result<u8, CursorError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(CursorError::Malformed),
    }
}

/// Cuts `rows` (read with [`PageSize::fetch`]) to one page and, when a further row was read,
/// names the cursor after the page's last work. `rows` are already newest first.
pub fn cut_page<T>(
    mut rows: Vec<T>,
    size: PageSize,
    scope: SearchScope,
    position: impl Fn(&T) -> PagePosition,
) -> (Vec<T>, Option<DiscoveryCursor>) {
    if rows.len() <= size.get() {
        return (rows, None);
    }
    rows.truncate(size.get());
    let next = rows
        .last()
        .map(|last| DiscoveryCursor::new(position(last), scope));
    (rows, next)
}

#[cfg(test)]
mod tests;
