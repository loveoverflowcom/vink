//! Text forms of a tag. Four things that are easy to confuse stay separate types:
//!
//! | Form | Type | Unique? | Example for "Ký ức" |
//! | --- | --- | --- | --- |
//! | Canonical identity | [`crate::domain::ids::TagId`] | yes, forever | UUID |
//! | Display label | [`TagLabel`] | via its slug | `Ký ức` (case and marks kept) |
//! | URL slug | [`TagSlug`] | yes, never reassigned | `ký-ức` (marks kept, lowercase) |
//! | Search key | [`SearchKey`] | **no** | `ky uc` (marks folded) |
//!
//! NFC and NFD spellings are the same text everywhere. Folding marks away is a search aid only:
//! `ma`, `má`, `mà`, `mã` and `mạ` share the search key `ma` but are five distinct slugs, so five
//! possible tags. No rule here rewrites a manuscript; only curated tag text passes through it.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Most characters (Unicode scalar values) in a label after trimming, NFC and whitespace
/// collapse. A tag names a form or theme in a few words; 40 holds "Thơ lục bát biến thể" three
/// times over while keeping a chip or a URL readable. A slug never has more characters than
/// its label, so the same bound applies to slugs.
pub const MAX_LABEL_CHARS: usize = 40;
/// Most characters in a curator's one-paragraph description of a tag.
pub const MAX_DESCRIPTION_CHARS: usize = 280;
/// Most characters in a lookup query after NFC; a query longer than any label cannot match one.
pub const MAX_QUERY_CHARS: usize = MAX_LABEL_CHARS;
/// Raw UTF-8 bytes accepted before normalizing anything. Generous enough for fully decomposed
/// Vietnamese (`ệ` is five bytes in NFD) at the character bound, small enough to bound work.
pub const MAX_RAW_TEXT_BYTES: usize = 16 * MAX_LABEL_CHARS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    #[error("label is empty")]
    Empty,
    #[error("label exceeds {MAX_LABEL_CHARS} characters")]
    TooLong,
    #[error(
        "label may hold only Vietnamese/Latin letters, digits, spaces, hyphens and apostrophes"
    )]
    ForbiddenCharacter,
    #[error("label has no letter or digit")]
    NoLetterOrDigit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SlugError {
    #[error("slug is empty")]
    Empty,
    #[error("slug exceeds {MAX_LABEL_CHARS} characters")]
    TooLong,
    #[error("slug is not lowercase letters/digits joined by single hyphens")]
    Malformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DescriptionError {
    #[error("description is empty")]
    Empty,
    #[error("description exceeds {MAX_DESCRIPTION_CHARS} characters")]
    TooLong,
    #[error("description contains a line break or control character")]
    ForbiddenCharacter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QueryError {
    #[error("query has no letter or digit")]
    Empty,
    #[error("query exceeds {MAX_QUERY_CHARS} characters")]
    TooLong,
}

/// A letter a pilot tag may use: an ASCII letter, optionally with combining marks (every
/// Vietnamese letter, NFC or not), or `đ`/`Đ`. Other scripts, compatibility forms (`ﬁ`, `Ａ`)
/// and letters whose lowercase is longer than one character (`İ`) are rejected, which keeps
/// slugs confusable-free and lets a label always derive a valid slug.
pub(super) fn is_tag_letter(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphabetic();
    }
    if c == 'đ' || c == 'Đ' {
        return true;
    }
    let mut parts = c.nfd();
    let base_ok = parts.next().is_some_and(|b| b.is_ascii_alphabetic());
    base_ok && parts.all(is_combining_mark) && c.to_lowercase().count() == 1
}

/// NFC of `text`. ASCII text is already NFC, so it skips the normalizer; besides saving work,
/// this keeps the ASCII labels of the Kani harness out of the Unicode tables.
pub(super) fn nfc(text: &str) -> String {
    if text.is_ascii() {
        text.to_owned()
    } else {
        text.nfc().collect()
    }
}

fn is_label_char(c: char) -> bool {
    is_tag_letter(c) || c.is_ascii_digit() || matches!(c, ' ' | '-' | '\'' | '\u{2019}')
}

/// Display text of a tag: trimmed, NFC, internal whitespace collapsed to one space, case and
/// marks kept. Curated, so the character set is deliberately narrow (see [`LabelError`]).
/// ```compile_fail
/// use vink_app::domain::taxonomy::TagLabel;
/// let label: TagLabel = serde_json::from_str(r#""Ký ức""#).unwrap();
/// ```
/// Lawful twin (stable rustdoc does not check `compile_fail` error codes): raw text must pass
/// [`TagLabel::parse`].
/// ```
/// use vink_app::domain::taxonomy::TagLabel;
/// let raw: String = serde_json::from_str(r#""Ký ức""#).unwrap();
/// let label: TagLabel = TagLabel::parse(&raw).unwrap();
/// assert_eq!(label.as_str(), "Ký ức");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagLabel(String);

impl TagLabel {
    pub fn parse(raw: &str) -> Result<Self, LabelError> {
        if raw.len() > MAX_RAW_TEXT_BYTES {
            return Err(LabelError::TooLong);
        }
        let label = nfc(raw).split_whitespace().collect::<Vec<_>>().join(" ");
        if label.is_empty() {
            return Err(LabelError::Empty);
        }
        if label.chars().count() > MAX_LABEL_CHARS {
            return Err(LabelError::TooLong);
        }
        if !label.chars().all(is_label_char) {
            return Err(LabelError::ForbiddenCharacter);
        }
        if !label
            .chars()
            .any(|c| is_tag_letter(c) || c.is_ascii_digit())
        {
            return Err(LabelError::NoLetterOrDigit);
        }
        Ok(Self(label))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The label's slug: what makes two labels the same name.
    pub fn slug(&self) -> TagSlug {
        TagSlug::from_label(self)
    }

    pub fn search_key(&self) -> SearchKey {
        SearchKey::of(&self.0)
    }
}

/// URL and duplicate key: lowercase NFC letters/digits joined by single hyphens, Vietnamese
/// marks kept (`ký-ức`, not `ky-uc`). Two labels with the same slug name the same thing
/// (`Ký ức`, `ký-ức`, `KÝ  ỨC`); labels that differ only by marks do not (`má` ≠ `mà`).
/// The catalog never reassigns a slug to another tag (see `catalog`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagSlug(String);

impl TagSlug {
    /// Derives the slug of a label: lowercase each letter, keep digits, turn every run of
    /// spaces/hyphens/apostrophes into one hyphen. Total: a label always has a letter or digit.
    pub fn from_label(label: &TagLabel) -> Self {
        let mut slug = String::with_capacity(label.0.len());
        let mut separator = false;
        for c in label.0.chars() {
            if is_tag_letter(c) || c.is_ascii_digit() {
                if separator && !slug.is_empty() {
                    slug.push('-');
                }
                separator = false;
                slug.extend(c.to_lowercase());
            } else {
                separator = true;
            }
        }
        Self(nfc(&slug))
    }

    /// Parses a slug from a URL path segment (already percent-decoded). NFD and uppercase
    /// spellings normalize to the canonical slug; the caller may redirect when they differ.
    pub fn parse(raw: &str) -> Result<Self, SlugError> {
        if raw.len() > MAX_RAW_TEXT_BYTES {
            return Err(SlugError::TooLong);
        }
        let lowered: String = nfc(raw).chars().flat_map(char::to_lowercase).collect();
        let slug = nfc(&lowered);
        if slug.is_empty() {
            return Err(SlugError::Empty);
        }
        if slug.chars().count() > MAX_LABEL_CHARS {
            return Err(SlugError::TooLong);
        }
        let well_formed = !slug.starts_with('-')
            && !slug.ends_with('-')
            && !slug.contains("--")
            && slug
                .chars()
                .all(|c| c == '-' || c.is_ascii_digit() || (is_tag_letter(c) && !c.is_uppercase()));
        if !well_formed {
            return Err(SlugError::Malformed);
        }
        Ok(Self(slug))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn search_key(&self) -> SearchKey {
        SearchKey::of(&self.0)
    }
}

/// Accent- and case-insensitive comparison key: NFD, combining marks dropped, `đ` → `d`,
/// lowercase, every run of non-alphanumeric characters one space. Many tags may share one key;
/// it finds candidates and never decides identity, synonymy or an alias.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SearchKey(String);

impl SearchKey {
    pub fn of(text: &str) -> Self {
        let mut key = String::with_capacity(text.len());
        let mut separator = false;
        for c in text.nfd() {
            if is_combining_mark(c) {
                continue;
            }
            let c = if c == 'đ' || c == 'Đ' { 'd' } else { c };
            if c.is_alphanumeric() {
                if separator && !key.is_empty() {
                    key.push(' ');
                }
                separator = false;
                key.extend(c.to_lowercase());
            } else {
                separator = true;
            }
        }
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether `prefix` starts this key or one of its words (`gia` matches `ky uc gia dinh`).
    pub fn has_word_prefix(&self, prefix: &SearchKey) -> bool {
        !prefix.is_empty()
            && (self.0.starts_with(&prefix.0)
                || self
                    .0
                    .match_indices(' ')
                    .any(|(i, _)| self.0[i + 1..].starts_with(&prefix.0)))
    }
}

/// A curator's one-paragraph explanation of when to use a tag. Trimmed, NFC, no line breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDescription(String);

impl TagDescription {
    pub fn parse(raw: &str) -> Result<Self, DescriptionError> {
        if raw.len() > 16 * MAX_DESCRIPTION_CHARS {
            return Err(DescriptionError::TooLong);
        }
        let text = nfc(raw.trim());
        if text.is_empty() {
            return Err(DescriptionError::Empty);
        }
        if text
            .chars()
            .any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
        {
            return Err(DescriptionError::ForbiddenCharacter);
        }
        if text.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(DescriptionError::TooLong);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What someone typed to find a tag. `exact` is the slug of the query when the query is itself
/// a valid label (`Má` → `má`), so a marked query can rank the exact name first; `key` finds
/// every accent-folded candidate (`ma` → `ma`, `má`, `mà`, `mã`, `mạ`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagQuery {
    exact: Option<TagSlug>,
    key: SearchKey,
}

impl TagQuery {
    pub fn parse(raw: &str) -> Result<Self, QueryError> {
        if raw.len() > MAX_RAW_TEXT_BYTES {
            return Err(QueryError::TooLong);
        }
        let text = nfc(raw);
        if text.chars().count() > MAX_QUERY_CHARS {
            return Err(QueryError::TooLong);
        }
        let key = SearchKey::of(&text);
        if key.is_empty() {
            return Err(QueryError::Empty);
        }
        let exact = TagLabel::parse(&text).ok().map(|label| label.slug());
        Ok(Self { exact, key })
    }

    pub fn exact(&self) -> Option<&TagSlug> {
        self.exact.as_ref()
    }

    pub fn key(&self) -> &SearchKey {
        &self.key
    }
}
