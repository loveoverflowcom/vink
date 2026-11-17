//! Canonical literary source, format, version, and capability contract (VDR-DOCUMENT-SOURCE-FORMAT).
//!
//! Boundary status (`AC-05`): `DocumentSource`, `StoredDocument`, and `FormatCapability` define the
//! version-2 format-preserving contract (`schema_version = 2`) alongside backward-compatible reads
//! of schema version 1 (`LegacyDefault`). Production writers (`save_draft` and `capture_snapshot`)
//! still persist [`document::SCHEMA_VERSION`] = `1` until `VDR-DOCUMENT-SOURCE-FORMAT` receives
//! owner signoff.
//!
//! Design invariants:
//! - Raw authored `source` bytes stay lossless in Unicode (NFD stays NFD; emoji ZWJ/FE0F stays
//!   intact), leading indentation/tabs, trailing/multiple blank lines, and whitespace-only lines.
//! - Only line endings are normalized to `\n` in stored `source`, while uniform `\r\n` is recorded in
//!   `line_ending` so authored text can be restored identically.
//! - Schema v1 documents never gain Markdown semantics through content sniffing (`# ` stays plain
//!   text until an author explicitly converts format).
//! - Derived projections (`to_nfc_search_projection`, `to_derived_v1_lossy`) never overwrite `source`.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use super::document::{
    BlockData, Document, DocumentData, DocumentError, MAX_DOCUMENT_CHARS,
    SCHEMA_VERSION as LEGACY_SCHEMA_VERSION,
};

/// Schema version for format-aware lossless source envelopes (`PENDING` exact-revision signoff).
pub const SOURCE_SCHEMA_VERSION: u16 = 2;

/// Defensive UTF-8 byte cap for raw stored `source`, accommodating 200k decomposed Vietnamese
/// scalars and preserved blank lines/indentation without unbounded payload growth.
pub const MAX_RAW_SOURCE_BYTES: usize = 2_097_152;

/// Active selectable content formats in v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    /// Separate title authority + plain multi-line paragraph/stanza text; never sniffed as Markdown.
    Default,
    /// Explicit CommonMark source format; title projection (`FirstH1`) is owned by `#263`.
    Markdown,
}

impl SourceFormat {
    pub const ALL: [Self; 2] = [Self::Default, Self::Markdown];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Markdown => "markdown",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, DocumentSourceError> {
        match raw {
            "default" => Ok(Self::Default),
            "markdown" => Ok(Self::Markdown),
            "asciidoc" => Err(DocumentSourceError::DeferredFormat(raw.to_owned())),
            _ => Err(DocumentSourceError::UnsupportedFormat(raw.to_owned())),
        }
    }
}

impl std::fmt::Display for SourceFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Reserved deferred formats exposed only in capability metadata, never accepted by parser/writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeferredFormat {
    #[serde(rename = "asciidoc")]
    AsciiDoc,
}

impl DeferredFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AsciiDoc => "asciidoc",
        }
    }
}

impl std::fmt::Display for DeferredFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Title-authority contract paired with each declared format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatTitleAuthority {
    SeparateTitle,
    FirstH1Projection,
    DeferredDocumentHeader,
}

/// Machine-readable format capability record establishing the extension point without exposing
/// deferred formats as selectable UI/API options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FormatCapability {
    pub id: &'static str,
    pub display_label_vi: &'static str,
    pub selectable: bool,
    pub deferred: bool,
    pub preserves_raw_source: bool,
    pub requires_explicit_conversion_from_legacy: bool,
    pub title_authority: FormatTitleAuthority,
}

const FORMAT_CAPABILITIES: [FormatCapability; 3] = [
    FormatCapability {
        id: "default",
        display_label_vi: "Mặc định",
        selectable: true,
        deferred: false,
        preserves_raw_source: true,
        requires_explicit_conversion_from_legacy: true,
        title_authority: FormatTitleAuthority::SeparateTitle,
    },
    FormatCapability {
        id: "markdown",
        display_label_vi: "Markdown",
        selectable: true,
        deferred: false,
        preserves_raw_source: true,
        requires_explicit_conversion_from_legacy: true,
        title_authority: FormatTitleAuthority::FirstH1Projection,
    },
    FormatCapability {
        id: "asciidoc",
        display_label_vi: "AsciiDoc",
        selectable: false,
        deferred: true,
        preserves_raw_source: true,
        requires_explicit_conversion_from_legacy: true,
        title_authority: FormatTitleAuthority::DeferredDocumentHeader,
    },
];

impl FormatCapability {
    pub fn catalog() -> &'static [FormatCapability] {
        &FORMAT_CAPABILITIES
    }

    pub fn active() -> impl Iterator<Item = &'static FormatCapability> {
        FORMAT_CAPABILITIES.iter().filter(|cap| cap.selectable)
    }

    pub fn lookup(id: &str) -> Option<&'static FormatCapability> {
        FORMAT_CAPABILITIES.iter().find(|cap| cap.id == id)
    }
}

/// Original line ending convention recorded so uniform CRLF authored text round-trips cleanly
/// while stored `source` uses canonical LF (`\n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Crlf => "crlf",
        }
    }

    /// Returns [`LineEnding::Crlf`] only when `authored` contains at least one `\r\n` and every line
    /// break uses `\r\n`. Bare `\r`, pure `\n`, empty text, or mixed `\r\n`/`\n` normalize to `Lf`.
    pub fn detect_from_authored(authored: &str) -> Self {
        let bytes = authored.as_bytes();
        let mut saw_crlf = false;
        for (idx, &byte) in bytes.iter().enumerate() {
            if byte == b'\r' {
                if bytes.get(idx + 1) != Some(&b'\n') {
                    return Self::Lf;
                }
                saw_crlf = true;
            } else if byte == b'\n' && (idx == 0 || bytes[idx - 1] != b'\r') {
                return Self::Lf;
            }
        }
        if saw_crlf { Self::Crlf } else { Self::Lf }
    }

    pub fn normalize_authored(authored: &str) -> (String, Self) {
        let detected = Self::detect_from_authored(authored);
        let normalized = if authored.contains('\r') {
            authored.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            authored.to_owned()
        };
        (normalized, detected)
    }

    pub fn restore(self, normalized_lf: &str) -> String {
        match self {
            Self::Lf => normalized_lf.to_owned(),
            Self::Crlf => normalized_lf.replace('\n', "\r\n"),
        }
    }
}

impl std::fmt::Display for LineEnding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Raw JSON/wire envelope for schema v2 documents. Deserializing it validates nothing; pass it to
/// [`DocumentSource::parse`] to cross the validation boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSourceData {
    pub schema_version: u16,
    pub format: SourceFormat,
    pub source: String,
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentSourceError {
    #[error("unsupported source document schema_version {0}")]
    UnsupportedSchemaVersion(u16),
    #[error("format `{0}` is deferred and not selectable in this version")]
    DeferredFormat(String),
    #[error("unsupported document format `{0}`")]
    UnsupportedFormat(String),
    #[error("schema v2 document requires an explicit format")]
    MissingFormat,
    #[error("schema v1 legacy document cannot declare a separate format override")]
    UnexpectedFormatOnLegacyV1,
    #[error("stored source line {line} contains unnormalized CR (`\\r`) line ending")]
    UnnormalizedLineEnding { line: usize },
    #[error("stored source line {line} contains forbidden control or Unicode separator character")]
    ForbiddenCharacter { line: usize },
    #[error("raw source exceeds {max_bytes} UTF-8 bytes (got {bytes})")]
    RawBytesExceeded { bytes: usize, max_bytes: usize },
    #[error("document exceeds {MAX_DOCUMENT_CHARS} normalized content characters")]
    TooLong,
}

/// Validated, lossless schema v2 source document. Only [`DocumentSourceData`] deserializes directly;
/// callers must cross [`DocumentSource::parse`] or [`DocumentSource::from_authored`].
/// ```compile_fail
/// use vink_app::domain::source::DocumentSource;
/// let src: DocumentSource = serde_json::from_str(
///     r#"{"schema_version":2,"format":"default","source":"a","line_ending":"lf"}"#,
/// )
/// .unwrap();
/// ```
/// Lawful twin: deserialize [`DocumentSourceData`] and validate via [`DocumentSource::parse`].
/// ```
/// use vink_app::domain::source::{DocumentSource, DocumentSourceData, SourceFormat};
/// let data: DocumentSourceData = serde_json::from_str(
///     r#"{"schema_version":2,"format":"default","source":"a","line_ending":"lf"}"#,
/// )
/// .unwrap();
/// let src = DocumentSource::parse(data).unwrap();
/// assert_eq!(src.format(), SourceFormat::Default);
/// assert_eq!(src.source(), "a");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSource {
    format: SourceFormat,
    source: String,
    line_ending: LineEnding,
}

impl DocumentSource {
    pub fn parse(data: DocumentSourceData) -> Result<Self, DocumentSourceError> {
        if data.schema_version != SOURCE_SCHEMA_VERSION {
            return Err(DocumentSourceError::UnsupportedSchemaVersion(
                data.schema_version,
            ));
        }
        validate_raw_source(&data.source)?;
        Ok(Self {
            format: data.format,
            source: data.source,
            line_ending: data.line_ending,
        })
    }

    pub fn from_authored(
        format: SourceFormat,
        authored: &str,
    ) -> Result<Self, DocumentSourceError> {
        let (source, line_ending) = LineEnding::normalize_authored(authored);
        Self::parse(DocumentSourceData {
            schema_version: SOURCE_SCHEMA_VERSION,
            format,
            source,
            line_ending,
        })
    }

    pub fn format(&self) -> SourceFormat {
        self.format
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Exact stored UTF-8 text with `\n` line breaks. Preserves NFD, emoji modifiers, tabs, spaces,
    /// blank lines, and Markdown/plain syntax verbatim.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn to_data(&self) -> DocumentSourceData {
        DocumentSourceData {
            schema_version: SOURCE_SCHEMA_VERSION,
            format: self.format,
            source: self.source.clone(),
            line_ending: self.line_ending,
        }
    }

    /// Text ready for browser/CodeMirror 6 editor buffers (which always operate over `\n`).
    pub fn to_editor_text(&self) -> &str {
        &self.source
    }

    /// Reconstructs uniform CRLF when `line_ending == Crlf`; keeps `\n` otherwise.
    pub fn to_authored_text(&self) -> String {
        self.line_ending.restore(&self.source)
    }

    /// Derived NFC projection for indexing/search/title matching without mutating `source`.
    pub fn to_nfc_search_projection(&self) -> String {
        self.source
            .split('\n')
            .map(|line| line.nfc().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Projects a lossless v2 document down to schema v1 `DocumentData` while explicitly reporting
    /// what semantics/whitespace/Unicode detail v1 cannot represent.
    pub fn to_derived_v1_lossy(&self) -> Result<(Document, LegacyLossSummary), DocumentError> {
        let mut blocks = Vec::new();
        let mut current: Vec<String> = Vec::new();
        let mut non_nfc_normalized = false;
        let mut leading_blank_lines = 0usize;
        let mut blank_run = 0usize;
        let mut collapsed_internal_blank_lines = 0usize;
        let mut dropped_whitespace_only_lines = 0usize;
        let mut saw_content_block = false;

        if !self.source.is_empty() {
            for line in self.source.split('\n') {
                if !non_nfc_normalized && line.nfc().ne(line.chars()) {
                    non_nfc_normalized = true;
                }
                if line.trim().is_empty() {
                    if !line.is_empty() {
                        dropped_whitespace_only_lines += 1;
                    }
                    if !saw_content_block {
                        leading_blank_lines += 1;
                    } else {
                        if !current.is_empty() {
                            blocks.push(BlockData {
                                lines: std::mem::take(&mut current),
                            });
                        }
                        blank_run += 1;
                    }
                } else {
                    if saw_content_block && blank_run > 1 {
                        collapsed_internal_blank_lines += blank_run - 1;
                    }
                    blank_run = 0;
                    saw_content_block = true;
                    current.push(line.to_owned());
                }
            }
            if !current.is_empty() {
                blocks.push(BlockData { lines: current });
            }
        }

        let trailing_blank_lines = if saw_content_block { blank_run } else { 0 };
        let doc = Document::parse(DocumentData {
            schema_version: LEGACY_SCHEMA_VERSION,
            blocks,
        })?;
        let summary = LegacyLossSummary {
            format: self.format,
            crlf_normalized: self.line_ending == LineEnding::Crlf,
            non_nfc_normalized,
            leading_blank_lines,
            trailing_blank_lines,
            collapsed_internal_blank_lines,
            dropped_whitespace_only_lines,
        };
        Ok((doc, summary))
    }
}

/// Exact accounting of information erased when a v2 `DocumentSource` is flattened into a v1
/// `Document` (`blocks` only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLossSummary {
    pub format: SourceFormat,
    pub crlf_normalized: bool,
    pub non_nfc_normalized: bool,
    pub leading_blank_lines: usize,
    pub trailing_blank_lines: usize,
    pub collapsed_internal_blank_lines: usize,
    pub dropped_whitespace_only_lines: usize,
}

impl LegacyLossSummary {
    pub fn is_lossless_text(&self) -> bool {
        !self.crlf_normalized
            && !self.non_nfc_normalized
            && self.leading_blank_lines == 0
            && self.trailing_blank_lines == 0
            && self.collapsed_internal_blank_lines == 0
            && self.dropped_whitespace_only_lines == 0
    }

    pub fn is_lossless(&self) -> bool {
        self.format == SourceFormat::Default && self.is_lossless_text()
    }
}

/// Effective interpretation format when reading stored documents across versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveFormat {
    /// Legacy v1 document with no explicit `format` field: always plain text/stanzas, NEVER Markdown.
    LegacyDefault,
    Explicit(SourceFormat),
}

impl EffectiveFormat {
    pub fn resolve_legacy_compatible(
        schema_version: u16,
        raw_format: Option<&str>,
    ) -> Result<Self, DocumentSourceError> {
        match (schema_version, raw_format) {
            (LEGACY_SCHEMA_VERSION, None) => Ok(Self::LegacyDefault),
            (LEGACY_SCHEMA_VERSION, Some(_)) => {
                Err(DocumentSourceError::UnexpectedFormatOnLegacyV1)
            }
            (SOURCE_SCHEMA_VERSION, None) => Err(DocumentSourceError::MissingFormat),
            (SOURCE_SCHEMA_VERSION, Some(raw)) => SourceFormat::parse(raw).map(Self::Explicit),
            (other, _) => Err(DocumentSourceError::UnsupportedSchemaVersion(other)),
        }
    }

    pub const fn is_legacy_default(self) -> bool {
        matches!(self, Self::LegacyDefault)
    }

    pub const fn renders_markdown(self) -> bool {
        matches!(self, Self::Explicit(SourceFormat::Markdown))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoredDocumentError {
    #[error("stored document JSON must be an object")]
    ExpectedJsonObject,
    #[error("stored document is missing `schema_version`")]
    MissingSchemaVersion,
    #[error("stored document `schema_version` must be an unsigned 16-bit integer")]
    InvalidSchemaVersion,
    #[error("unsupported stored document schema_version {0}")]
    UnsupportedSchemaVersion(u16),
    #[error("invalid document envelope JSON: {0}")]
    InvalidEnvelope(String),
    #[error(transparent)]
    LegacyV1(#[from] DocumentError),
    #[error(transparent)]
    Source(#[from] DocumentSourceError),
}

/// Version-dispatched validated stored document (`V1` legacy or `V2` lossless source). Does not
/// implement `Deserialize` directly; parse via [`StoredDocument::parse_json_str`] or
/// [`StoredDocument::parse_json_value`].
/// ```compile_fail
/// use vink_app::domain::source::StoredDocument;
/// let stored: StoredDocument = serde_json::from_str(r#"{"schema_version":1,"blocks":[]}"#).unwrap();
/// ```
/// Lawful twin: dispatch both v1 and v2 JSON envelopes through [`StoredDocument::parse_json_str`].
/// ```
/// use vink_app::domain::source::{EffectiveFormat, StoredDocument};
/// let legacy = StoredDocument::parse_json_str(r#"{"schema_version":1,"blocks":[]}"#).unwrap();
/// assert_eq!(legacy.effective_format(), EffectiveFormat::LegacyDefault);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredDocument {
    V1(Document),
    V2(DocumentSource),
}

impl StoredDocument {
    pub fn parse_json_str(raw: &str) -> Result<Self, StoredDocumentError> {
        let value: serde_json::Value = serde_json::from_str(raw)
            .map_err(|err| StoredDocumentError::InvalidEnvelope(err.to_string()))?;
        Self::parse_json_value(value)
    }

    pub fn parse_json_value(value: serde_json::Value) -> Result<Self, StoredDocumentError> {
        let map = value
            .as_object()
            .ok_or(StoredDocumentError::ExpectedJsonObject)?;
        let version_value = map
            .get("schema_version")
            .ok_or(StoredDocumentError::MissingSchemaVersion)?;
        let version_u64 = version_value
            .as_u64()
            .ok_or(StoredDocumentError::InvalidSchemaVersion)?;
        let schema_version =
            u16::try_from(version_u64).map_err(|_| StoredDocumentError::InvalidSchemaVersion)?;

        match schema_version {
            LEGACY_SCHEMA_VERSION => {
                let data: DocumentData = serde_json::from_value(value)
                    .map_err(|err| StoredDocumentError::InvalidEnvelope(err.to_string()))?;
                Ok(Self::V1(Document::parse(data)?))
            }
            SOURCE_SCHEMA_VERSION => {
                if let Some(raw_fmt) = map.get("format").and_then(|v| v.as_str()) {
                    SourceFormat::parse(raw_fmt)?;
                }
                let data: DocumentSourceData = serde_json::from_value(value)
                    .map_err(|err| StoredDocumentError::InvalidEnvelope(err.to_string()))?;
                Ok(Self::V2(DocumentSource::parse(data)?))
            }
            other => Err(StoredDocumentError::UnsupportedSchemaVersion(other)),
        }
    }

    pub fn schema_version(&self) -> u16 {
        match self {
            Self::V1(_) => LEGACY_SCHEMA_VERSION,
            Self::V2(_) => SOURCE_SCHEMA_VERSION,
        }
    }

    pub fn effective_format(&self) -> EffectiveFormat {
        match self {
            Self::V1(_) => EffectiveFormat::LegacyDefault,
            Self::V2(src) => EffectiveFormat::Explicit(src.format()),
        }
    }

    pub fn to_editor_text(&self) -> String {
        match self {
            Self::V1(doc) => doc
                .blocks()
                .map(|lines| lines.join("\n"))
                .collect::<Vec<_>>()
                .join("\n\n"),
            Self::V2(src) => src.to_editor_text().to_owned(),
        }
    }

    pub fn to_nfc_search_text(&self) -> String {
        match self {
            Self::V1(_) => self.to_editor_text(),
            Self::V2(src) => src.to_nfc_search_projection(),
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::V1(doc) => {
                serde_json::to_value(doc.to_data()).expect("DocumentData serializes infallibly")
            }
            Self::V2(src) => serde_json::to_value(src.to_data())
                .expect("DocumentSourceData serializes infallibly"),
        }
    }

    pub fn to_derived_v1_document(
        &self,
    ) -> Result<(Document, Option<LegacyLossSummary>), DocumentError> {
        match self {
            Self::V1(doc) => Ok((doc.clone(), None)),
            Self::V2(src) => {
                let (doc, loss) = src.to_derived_v1_lossy()?;
                Ok((doc, Some(loss)))
            }
        }
    }
}

fn validate_raw_source(source: &str) -> Result<(), DocumentSourceError> {
    if source.len() > MAX_RAW_SOURCE_BYTES {
        return Err(DocumentSourceError::RawBytesExceeded {
            bytes: source.len(),
            max_bytes: MAX_RAW_SOURCE_BYTES,
        });
    }

    let mut normalized_chars = 0usize;
    for (line_idx, line) in source.split('\n').enumerate() {
        for ch in line.chars() {
            if ch == '\r' {
                return Err(DocumentSourceError::UnnormalizedLineEnding { line: line_idx });
            }
            if is_forbidden_source_char(ch) {
                return Err(DocumentSourceError::ForbiddenCharacter { line: line_idx });
            }
        }
        normalized_chars = normalized_chars.saturating_add(line.nfc().count());
        if normalized_chars > MAX_DOCUMENT_CHARS {
            return Err(DocumentSourceError::TooLong);
        }
    }
    Ok(())
}

fn is_forbidden_source_char(c: char) -> bool {
    (c.is_control() && c != '\t') || c == '\u{2028}' || c == '\u{2029}'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_catalog_exposes_default_and_markdown_and_keeps_asciidoc_deferred() {
        let active: Vec<&str> = FormatCapability::active().map(|cap| cap.id).collect();
        assert_eq!(active, vec!["default", "markdown"]);

        let asciidoc = FormatCapability::lookup("asciidoc").expect("asciidoc reserved capability");
        assert!(asciidoc.deferred);
        assert!(!asciidoc.selectable);
        assert_eq!(
            asciidoc.title_authority,
            FormatTitleAuthority::DeferredDocumentHeader
        );
        assert_eq!(
            SourceFormat::parse("asciidoc"),
            Err(DocumentSourceError::DeferredFormat("asciidoc".to_owned()))
        );
    }

    #[test]
    fn source_preserves_nfd_emoji_indentation_tabs_and_blank_lines_without_mutation() {
        let authored = concat!(
            "\n\n",
            "  \tTie\u{0302}\u{0301}ng Vie\u{0323}\u{0302}t NFD\n",
            "    \n",
            "\n",
            "\n",
            "Tho\u{031b} lu\u{0323}c ba\u{0301}t \u{1f9d1}\u{200d}\u{1f33e} \u{2764}\u{fe0f} \u{270d}\u{fe0f}\u{1f3fd}\n",
            "   \t\n"
        );
        let src = DocumentSource::from_authored(SourceFormat::Default, authored).unwrap();
        assert_eq!(src.line_ending(), LineEnding::Lf);
        // Raw source must remain byte-for-byte identical, NOT NFC-folded or trimmed.
        assert_eq!(src.source(), authored);
        assert_eq!(src.to_editor_text(), authored);
        assert_eq!(src.to_authored_text(), authored);
        assert_ne!(src.to_nfc_search_projection(), authored);
        assert!(src.to_nfc_search_projection().contains("Tiếng Việt NFD"));
        // Raw source itself stays untouched after projection calls.
        assert_eq!(src.source(), authored);
    }

    #[test]
    fn uniform_crlf_round_trips_byte_identically_while_mixed_crlf_lf_normalizes_to_lf() {
        let uniform_crlf = "# Tiêu đề\r\n\r\n\r\n  Dòng thơ một\r\nDòng thơ hai\r\n";
        let src_crlf = DocumentSource::from_authored(SourceFormat::Markdown, uniform_crlf).unwrap();
        assert_eq!(src_crlf.line_ending(), LineEnding::Crlf);
        assert!(!src_crlf.source().contains('\r'));
        assert_eq!(src_crlf.to_authored_text(), uniform_crlf);

        let mixed = "Dòng 1\r\nDòng 2\nDòng 3\rDòng 4";
        let src_mixed = DocumentSource::from_authored(SourceFormat::Default, mixed).unwrap();
        assert_eq!(src_mixed.line_ending(), LineEnding::Lf);
        assert_eq!(src_mixed.source(), "Dòng 1\nDòng 2\nDòng 3\nDòng 4");
        assert_eq!(
            src_mixed.to_authored_text(),
            "Dòng 1\nDòng 2\nDòng 3\nDòng 4"
        );
    }

    #[test]
    fn stored_document_v1_and_default_starting_with_hash_never_sniff_as_markdown() {
        let v1_json = r##"{
            "schema_version": 1,
            "blocks": [
                {"lines": ["# Không phải Markdown"]},
                {"lines": ["Dòng 1", "Dòng 2"]}
            ]
        }"##;
        let v1 = StoredDocument::parse_json_str(v1_json).unwrap();
        assert_eq!(v1.schema_version(), 1);
        assert_eq!(v1.effective_format(), EffectiveFormat::LegacyDefault);
        assert!(!v1.effective_format().renders_markdown());
        assert_eq!(
            v1.to_editor_text(),
            "# Không phải Markdown\n\nDòng 1\nDòng 2"
        );

        let v2_default = DocumentSource::from_authored(
            SourceFormat::Default,
            "# Vẫn là chữ thường\n\n**không in đậm**",
        )
        .unwrap();
        let stored_v2_default =
            StoredDocument::parse_json_value(StoredDocument::V2(v2_default).to_json_value())
                .unwrap();
        assert_eq!(
            stored_v2_default.effective_format(),
            EffectiveFormat::Explicit(SourceFormat::Default)
        );
        assert!(!stored_v2_default.effective_format().renders_markdown());

        let v2_md = DocumentSource::from_authored(SourceFormat::Markdown, "# Tiêu đề").unwrap();
        assert!(EffectiveFormat::Explicit(v2_md.format()).renders_markdown());
    }

    #[test]
    fn derived_v1_projection_accounts_accurately_for_lost_structure_and_unicode() {
        let src = DocumentSource::from_authored(
            SourceFormat::Markdown,
            "\r\n# Tie\u{0302}\u{0301}u đề\r\n   \r\n\r\n\r\nĐoạn cuối\r\n",
        )
        .unwrap();
        let (doc, loss) = src.to_derived_v1_lossy().unwrap();
        assert!(!loss.is_lossless());
        assert!(!loss.is_lossless_text());
        assert_eq!(loss.format, SourceFormat::Markdown);
        assert!(loss.crlf_normalized);
        assert!(loss.non_nfc_normalized);
        assert_eq!(loss.leading_blank_lines, 1);
        assert_eq!(loss.trailing_blank_lines, 1);
        assert_eq!(loss.collapsed_internal_blank_lines, 2);
        assert_eq!(loss.dropped_whitespace_only_lines, 1);
        assert_eq!(doc.first_line(), Some("# Tiếu đề"));
        // Exact source stays unmutated even after generating the lossy v1 projection.
        assert!(src.source().contains("Tie\u{0302}\u{0301}u"));
    }

    #[test]
    fn derived_v1_lossy_is_exact_only_for_plain_nfc_lf_with_single_block_breaks() {
        let src =
            DocumentSource::from_authored(SourceFormat::Default, "Khổ một\n\nKhổ hai").unwrap();
        let (doc, loss) = src.to_derived_v1_lossy().unwrap();
        assert!(loss.is_lossless());
        assert_eq!(doc.blocks().len(), 2);

        let empty = DocumentSource::from_authored(SourceFormat::Default, "").unwrap();
        let (empty_doc, empty_loss) = empty.to_derived_v1_lossy().unwrap();
        assert!(empty_loss.is_lossless());
        assert_eq!(empty_doc, Document::empty());
    }

    #[test]
    fn parse_rejects_unknown_fields_deferred_or_unknown_formats_and_raw_cr() {
        let err_asciidoc = StoredDocument::parse_json_str(
            r#"{"schema_version":2,"format":"asciidoc","source":"= Title","line_ending":"lf"}"#,
        );
        assert_eq!(
            err_asciidoc,
            Err(StoredDocumentError::Source(
                DocumentSourceError::DeferredFormat("asciidoc".to_owned())
            ))
        );

        let err_unknown_fmt = StoredDocument::parse_json_str(
            r#"{"schema_version":2,"format":"org","source":"* Title","line_ending":"lf"}"#,
        );
        assert_eq!(
            err_unknown_fmt,
            Err(StoredDocumentError::Source(
                DocumentSourceError::UnsupportedFormat("org".to_owned())
            ))
        );

        let err_extra_field = StoredDocument::parse_json_str(
            r#"{"schema_version":2,"format":"default","source":"ok","line_ending":"lf","extra":1}"#,
        );
        assert!(matches!(
            err_extra_field,
            Err(StoredDocumentError::InvalidEnvelope(_))
        ));

        let err_v1_format = StoredDocument::parse_json_str(
            r#"{"schema_version":1,"format":"markdown","blocks":[]}"#,
        );
        assert!(matches!(
            err_v1_format,
            Err(StoredDocumentError::InvalidEnvelope(_))
        ));

        let err_raw_cr = DocumentSource::parse(DocumentSourceData {
            schema_version: SOURCE_SCHEMA_VERSION,
            format: SourceFormat::Default,
            source: "a\r\nb".to_owned(),
            line_ending: LineEnding::Lf,
        });
        assert_eq!(
            err_raw_cr,
            Err(DocumentSourceError::UnnormalizedLineEnding { line: 0 })
        );

        assert_eq!(
            EffectiveFormat::resolve_legacy_compatible(1, None),
            Ok(EffectiveFormat::LegacyDefault)
        );
        assert_eq!(
            EffectiveFormat::resolve_legacy_compatible(2, None),
            Err(DocumentSourceError::MissingFormat)
        );
    }

    #[test]
    fn accepts_200k_nfd_characters_inside_raw_byte_cap_and_rejects_excess() {
        // One Vietnamese char in NFD (`e\u{0323}\u{0302}` = 3 scalars, 5 UTF-8 bytes): 200,000 NFC chars
        // fit inside MAX_DOCUMENT_CHARS (200,000) and MAX_RAW_SOURCE_BYTES (2 MiB).
        let nfd_char = "e\u{0323}\u{0302}";
        let at_limit = nfd_char.repeat(MAX_DOCUMENT_CHARS);
        assert!(at_limit.len() < MAX_RAW_SOURCE_BYTES);
        let parsed = DocumentSource::from_authored(SourceFormat::Default, &at_limit).unwrap();
        assert_eq!(parsed.source().len(), at_limit.len());

        let over_chars = format!("{at_limit}a");
        assert_eq!(
            DocumentSource::from_authored(SourceFormat::Default, &over_chars),
            Err(DocumentSourceError::TooLong)
        );

        let raw_bytes_flood = "\n".repeat(MAX_RAW_SOURCE_BYTES + 1);
        assert!(matches!(
            DocumentSource::from_authored(SourceFormat::Default, &raw_bytes_flood),
            Err(DocumentSourceError::RawBytesExceeded { .. })
        ));
    }
}
