//! The one canonical literary document: an explicit format and the author's exact source text.
//!
//! There is a single, unversioned representation (`VDR-DOCUMENT-SOURCE-FORMAT@r2`, product
//! decision 17/11/2026 "Replace hoàn toàn"). `source` keeps every byte the author wrote: Unicode
//! form (NFD stays NFD), emoji sequences, indentation, tabs, blank and whitespace-only lines and
//! line endings. Normalization only ever builds derived projections (title, search keys); it
//! never rewrites `source`.
//!
//! The format decides who owns the title ([`TitledDocument`]):
//! - [`DocumentFormat::Default`]: a separate, explicit title; the body is literal text and a
//!   leading `#` is never read as Markdown.
//! - [`DocumentFormat::Markdown`]: the title is the first H1 of the CommonMark AST
//!   ([`markdown::first_h1_projection`]); there is no writable title field.
//!
//! AsciiDoc is `DEFERRED`: the name is recognized only to refuse it explicitly.

pub mod markdown;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

/// Content budget in Unicode scalar values after NFC, line breaks (`\n`, `\r`) excluded.
pub const MAX_DOCUMENT_CHARS: usize = 200_000;
/// Defensive byte cap on the raw source: 200 000 decomposed Vietnamese characters plus preserved
/// blank lines and indentation fit; unbounded payloads do not.
pub const MAX_SOURCE_BYTES: usize = 2_097_152;
pub const MAX_TITLE_CHARS: usize = 200;

/// The formats a document can have. Unknown names are refused, never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentFormat {
    Default,
    Markdown,
}

impl DocumentFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Markdown => "markdown",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, DocumentError> {
        match raw {
            "default" => Ok(Self::Default),
            "markdown" => Ok(Self::Markdown),
            "asciidoc" => Err(DocumentError::DeferredFormat(raw.to_owned())),
            _ => Err(DocumentError::UnsupportedFormat(raw.to_owned())),
        }
    }
}

impl std::fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Canonical stored (PostgreSQL `jsonb`) and wire shape. Deserializing it validates nothing; use
/// [`Document::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentData {
    pub format: String,
    pub source: String,
}

/// A validated document. Only the raw [`DocumentData`] shape deserializes; callers must use
/// [`Document::parse`] or [`Document::new`] to cross that trust boundary.
/// ```compile_fail
/// use vink_app::domain::document::Document;
/// let document: Document = serde_json::from_str(r#"{"format":"default","source":""}"#).unwrap();
/// ```
/// Lawful twin (stable rustdoc does not check `compile_fail` error codes): the raw shape
/// deserializes and only [`Document::parse`] turns it into a document.
/// ```
/// use vink_app::domain::document::{Document, DocumentData, DocumentFormat};
/// let data: DocumentData = serde_json::from_str(r#"{"format":"default","source":""}"#).unwrap();
/// let document: Document = Document::parse(data).unwrap();
/// assert_eq!(document, Document::empty(DocumentFormat::Default));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    format: DocumentFormat,
    source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("unsupported document format `{0}`")]
    UnsupportedFormat(String),
    #[error("document format `{0}` is deferred and cannot be written yet")]
    DeferredFormat(String),
    #[error("line {line} contains a control or Unicode line/paragraph separator character")]
    ForbiddenCharacter { line: usize },
    #[error("document exceeds {MAX_DOCUMENT_CHARS} characters")]
    TooLong,
    #[error("document source exceeds {MAX_SOURCE_BYTES} bytes")]
    TooManyBytes,
}

impl Document {
    pub fn empty(format: DocumentFormat) -> Self {
        Self {
            format,
            source: String::new(),
        }
    }

    pub fn new(format: DocumentFormat, source: impl Into<String>) -> Result<Self, DocumentError> {
        let source = source.into();
        validate_source(&source)?;
        Ok(Self { format, source })
    }

    pub fn parse(data: DocumentData) -> Result<Self, DocumentError> {
        Self::new(DocumentFormat::parse(&data.format)?, data.source)
    }

    pub fn format(&self) -> DocumentFormat {
        self.format
    }

    /// The exact authored text, byte for byte.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn to_data(&self) -> DocumentData {
        DocumentData {
            format: self.format.as_str().to_owned(),
            source: self.source.clone(),
        }
    }
}

fn validate_source(source: &str) -> Result<(), DocumentError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(DocumentError::TooManyBytes);
    }
    let mut line = 0usize;
    let mut after_cr = false;
    for c in source.chars() {
        match c {
            '\r' => line += 1,
            '\n' if !after_cr => line += 1,
            c if is_forbidden_in_source(c) => {
                return Err(DocumentError::ForbiddenCharacter { line });
            }
            _ => {}
        }
        after_cr = c == '\r';
    }
    let counted = source
        .nfc()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(MAX_DOCUMENT_CHARS + 1)
        .count();
    if counted > MAX_DOCUMENT_CHARS {
        return Err(DocumentError::TooLong);
    }
    Ok(())
}

/// Tabs and line endings (`\n`, `\r`) are text; other control characters and the Unicode
/// line/paragraph separators would smuggle invisible structure into the source.
fn is_forbidden_in_source(c: char) -> bool {
    (c.is_control() && !matches!(c, '\t' | '\n' | '\r')) || c == '\u{2028}' || c == '\u{2029}'
}

/// A work title. Working drafts may be untitled; revisions require a title.
/// ```compile_fail
/// use vink_app::domain::document::Title;
/// let title: Title = serde_json::from_str(r#""a\nb""#).unwrap();
/// ```
/// Lawful twin: the same raw text must pass [`Title::parse`], which rejects the line break.
/// ```
/// use vink_app::domain::document::Title;
/// let raw: String = serde_json::from_str(r#""a\nb""#).unwrap();
/// assert!(Title::parse(&raw).is_err());
/// let _title: Title = Title::parse("Mưa").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TitleError {
    #[error("title contains a line break or control character")]
    ForbiddenCharacter,
    #[error("title exceeds {MAX_TITLE_CHARS} characters")]
    TooLong,
}

impl Title {
    /// Trims, normalizes to NFC and validates. The limit counts Unicode scalar values after NFC.
    pub fn parse(raw: &str) -> Result<Self, TitleError> {
        let title: String = raw.trim().nfc().collect();
        if title.chars().any(is_forbidden_in_title) {
            return Err(TitleError::ForbiddenCharacter);
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(TitleError::TooLong);
        }
        Ok(Self(title))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn is_forbidden_in_title(c: char) -> bool {
    c.is_control() || c == '\u{2028}' || c == '\u{2029}'
}

/// A document paired with the title its format authorizes. This is the only constructor of that
/// pair, so source and title cannot drift apart within one save, revision or read.
///
/// ```compile_fail
/// use vink_app::domain::document::{Document, DocumentFormat, Title, TitledDocument};
/// let content = TitledDocument { title: Title::parse("x").unwrap(),
///     document: Document::empty(DocumentFormat::Markdown) };
/// ```
/// Lawful twin: Markdown takes its title from the source and refuses a separate one.
/// ```
/// use vink_app::domain::document::{Document, DocumentFormat, TitleAuthorityError, TitledDocument};
/// let markdown = Document::new(DocumentFormat::Markdown, "# Mưa\n\nThân bài.").unwrap();
/// let content = TitledDocument::author(markdown.clone(), None).unwrap();
/// assert_eq!(content.title().as_str(), "Mưa");
/// assert_eq!(
///     TitledDocument::author(markdown, Some("Tên khác")),
///     Err(TitleAuthorityError::TitleNotWritable),
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitledDocument {
    title: Title,
    document: Document,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TitleAuthorityError {
    #[error("title: {0}")]
    Title(#[from] TitleError),
    #[error("a default document needs its separate title")]
    MissingTitle,
    #[error("a markdown document takes its title from its first H1; a separate title is refused")]
    TitleNotWritable,
    #[error("stored title differs from the title the source authorizes")]
    StoredTitleMismatch,
}

impl TitledDocument {
    /// The authoring boundary. Default documents need `title` (it may be blank in a draft);
    /// Markdown documents must not send one: the first H1 is the only title source.
    pub fn author(document: Document, title: Option<&str>) -> Result<Self, TitleAuthorityError> {
        let title = match (document.format(), title) {
            (DocumentFormat::Default, Some(raw)) => Title::parse(raw)?,
            (DocumentFormat::Default, None) => return Err(TitleAuthorityError::MissingTitle),
            (DocumentFormat::Markdown, None) => markdown_title(document.source())?,
            (DocumentFormat::Markdown, Some(_)) => {
                return Err(TitleAuthorityError::TitleNotWritable);
            }
        };
        Ok(Self { title, document })
    }

    /// The stored boundary: a default title is the stored column; a Markdown title is derived again
    /// and must equal the stored projection, so a drifted row is an integrity error, not a rename.
    pub fn from_stored(
        document: Document,
        stored_title: &str,
    ) -> Result<Self, TitleAuthorityError> {
        let stored = Title::parse(stored_title)?;
        if stored.as_str() != stored_title {
            return Err(TitleAuthorityError::StoredTitleMismatch);
        }
        if document.format() == DocumentFormat::Markdown
            && markdown_title(document.source())? != stored
        {
            return Err(TitleAuthorityError::StoredTitleMismatch);
        }
        Ok(Self {
            title: stored,
            document,
        })
    }

    pub fn title(&self) -> &Title {
        &self.title
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn format(&self) -> DocumentFormat {
        self.document.format()
    }
}

fn markdown_title(source: &str) -> Result<Title, TitleError> {
    Title::parse(&markdown::first_h1_projection(source))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default(source: &str) -> Document {
        Document::new(DocumentFormat::Default, source).unwrap()
    }

    #[test]
    fn source_is_kept_byte_for_byte() {
        let authored = "  Phố cũ mưa giăng,\r\n\tmái ngói rêu.\r\n\r\n\r\n   \n\
                        Vie\u{0323}\u{0302}t ❤️‍🔥 👩🏽‍💻\n\n";
        let document = default(authored);
        assert_eq!(document.source(), authored);
        assert_eq!(document.to_data().source, authored);
        assert_eq!(Document::parse(document.to_data()).unwrap(), document);
    }

    #[test]
    fn formats_are_explicit_and_unknown_or_deferred_names_are_refused() {
        for format in [DocumentFormat::Default, DocumentFormat::Markdown] {
            assert_eq!(DocumentFormat::parse(format.as_str()), Ok(format));
        }
        assert_eq!(
            DocumentFormat::parse("asciidoc"),
            Err(DocumentError::DeferredFormat("asciidoc".into()))
        );
        for unknown in ["", "Default", "md", "html", "plain"] {
            assert_eq!(
                DocumentFormat::parse(unknown),
                Err(DocumentError::UnsupportedFormat(unknown.into()))
            );
        }
    }

    #[test]
    fn leading_hash_does_not_make_a_default_document_markdown() {
        let document = default("# Không phải tên\n\nThân bài.");
        assert_eq!(document.format(), DocumentFormat::Default);
        let content = TitledDocument::author(document, Some("Tên riêng")).unwrap();
        assert_eq!(content.title().as_str(), "Tên riêng");
    }

    #[test]
    fn forbidden_characters_report_their_line() {
        assert_eq!(
            Document::new(DocumentFormat::Default, "một\nhai\u{0}"),
            Err(DocumentError::ForbiddenCharacter { line: 1 })
        );
        assert_eq!(
            Document::new(DocumentFormat::Markdown, "a\u{2028}b"),
            Err(DocumentError::ForbiddenCharacter { line: 0 })
        );
        assert_eq!(
            Document::new(DocumentFormat::Default, "\n\n\u{7f}"),
            Err(DocumentError::ForbiddenCharacter { line: 2 })
        );
    }

    #[test]
    fn size_limits_count_nfc_characters_without_line_breaks_and_cap_bytes() {
        let at_limit = "đ".repeat(MAX_DOCUMENT_CHARS);
        assert!(Document::new(DocumentFormat::Default, at_limit.clone()).is_ok());
        let with_breaks = format!("{at_limit}\r\n\n\n");
        assert!(Document::new(DocumentFormat::Default, with_breaks).is_ok());
        assert_eq!(
            Document::new(DocumentFormat::Default, format!("{at_limit}x")),
            Err(DocumentError::TooLong)
        );
        // 200 000 decomposed characters (two scalars each) are 200 000 characters after NFC.
        let nfd = "e\u{0302}".repeat(MAX_DOCUMENT_CHARS);
        assert!(Document::new(DocumentFormat::Default, nfd).is_ok());
        let bytes = " ".repeat(MAX_SOURCE_BYTES + 1);
        assert_eq!(
            Document::new(DocumentFormat::Default, bytes),
            Err(DocumentError::TooManyBytes)
        );
    }

    #[test]
    fn stored_shape_rejects_unknown_and_missing_fields() {
        for raw in [
            r#"{"format":"default","source":"","html":"<b>x</b>"}"#,
            r#"{"format":"default"}"#,
            r#"{"source":""}"#,
            r#"{"schema_version":1,"blocks":[]}"#,
            r#"{"format":"default","source":"","schema_version":2}"#,
        ] {
            assert!(serde_json::from_str::<DocumentData>(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn forbidden_character_reports_the_line_across_mixed_endings() {
        assert_eq!(
            validate_source("a\rb\r\nc\nd\u{2028}"),
            Err(DocumentError::ForbiddenCharacter { line: 3 })
        );
    }

    #[test]
    fn title_is_trimmed_nfc_single_line_and_bounded() {
        assert_eq!(Title::parse("  Mưa  ").unwrap().as_str(), "Mưa");
        assert_eq!(
            Title::parse("Vie\u{0323}\u{0302}t").unwrap().as_str(),
            "Việt"
        );
        assert!(Title::parse("").unwrap().is_empty());
        assert_eq!(Title::parse("a\nb"), Err(TitleError::ForbiddenCharacter));
        assert_eq!(Title::parse("a\tb"), Err(TitleError::ForbiddenCharacter));
        let at_limit = "Đ".repeat(MAX_TITLE_CHARS);
        assert_eq!(Title::parse(&at_limit).unwrap().as_str(), at_limit);
        assert_eq!(
            Title::parse(&"ơ".repeat(MAX_TITLE_CHARS + 1)),
            Err(TitleError::TooLong)
        );
        assert_ne!(Title::parse("Đ").unwrap(), Title::parse("đ").unwrap());
    }

    #[test]
    fn title_authority_follows_the_format() {
        let body = default("Thân bài.");
        assert_eq!(
            TitledDocument::author(body.clone(), None),
            Err(TitleAuthorityError::MissingTitle)
        );
        let untitled = TitledDocument::author(body, Some("  ")).unwrap();
        assert!(untitled.title().is_empty());

        let markdown = Document::new(DocumentFormat::Markdown, "Thân.\n\n# Tên\n\n# Sau").unwrap();
        let content = TitledDocument::author(markdown.clone(), None).unwrap();
        assert_eq!(content.title().as_str(), "Tên");
        assert_eq!(content.document().source(), "Thân.\n\n# Tên\n\n# Sau");
        assert_eq!(
            TitledDocument::author(markdown.clone(), Some("")),
            Err(TitleAuthorityError::TitleNotWritable)
        );
        let long = Document::new(
            DocumentFormat::Markdown,
            format!("# {}", "ơ".repeat(MAX_TITLE_CHARS + 1)),
        )
        .unwrap();
        assert_eq!(
            TitledDocument::author(long, None),
            Err(TitleAuthorityError::Title(TitleError::TooLong))
        );
    }

    #[test]
    fn stored_titles_must_match_their_authority() {
        let markdown = Document::new(DocumentFormat::Markdown, "# Mưa").unwrap();
        assert!(TitledDocument::from_stored(markdown.clone(), "Mưa").is_ok());
        assert_eq!(
            TitledDocument::from_stored(markdown, "Nắng"),
            Err(TitleAuthorityError::StoredTitleMismatch)
        );
        // A stored default title is already canonical; an untrimmed column is drift.
        assert_eq!(
            TitledDocument::from_stored(default("x"), " Mưa"),
            Err(TitleAuthorityError::StoredTitleMismatch)
        );
        assert_eq!(
            TitledDocument::from_stored(default("# Mưa"), "Tên riêng")
                .unwrap()
                .title()
                .as_str(),
            "Tên riêng"
        );
    }
}
