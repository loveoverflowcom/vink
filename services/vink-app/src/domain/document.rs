//! Minimal literary document, schema version 1.
//!
//! A document is a list of blocks; a block is a list of lines. Prose paragraphs are one-line
//! blocks and poem stanzas are multi-line blocks, so stanza breaks and line breaks survive
//! save → store → render. This is the bootstrap schema for the textarea desk, not the long-term
//! editor decision (S-01). Lines are normalized to Unicode NFC so that visually identical
//! Vietnamese text typed as precomposed or combining marks stores and hashes the same way.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

pub const SCHEMA_VERSION: u16 = 1;
pub const MAX_DOCUMENT_CHARS: usize = 200_000;
pub const MAX_TITLE_CHARS: usize = 200;

/// Canonical stored and wire shape. Deserializing it validates nothing; use [`Document::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentData {
    pub schema_version: u16,
    pub blocks: Vec<BlockData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockData {
    pub lines: Vec<String>,
}

/// A validated, NFC-normalized document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    blocks: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("unsupported document schema_version {0}")]
    UnsupportedSchemaVersion(u16),
    #[error("block {block} has no lines")]
    EmptyBlock { block: usize },
    #[error("block {block} line {line} is blank; blank lines separate blocks")]
    BlankLine { block: usize, line: usize },
    #[error("block {block} line {line} contains a line break or control character")]
    ForbiddenCharacter { block: usize, line: usize },
    #[error("document exceeds {MAX_DOCUMENT_CHARS} characters")]
    TooLong,
}

impl Document {
    pub fn empty() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn parse(data: DocumentData) -> Result<Self, DocumentError> {
        if data.schema_version != SCHEMA_VERSION {
            return Err(DocumentError::UnsupportedSchemaVersion(data.schema_version));
        }
        let mut total = 0usize;
        let mut blocks = Vec::with_capacity(data.blocks.len());
        for (block_index, block) in data.blocks.into_iter().enumerate() {
            if block.lines.is_empty() {
                return Err(DocumentError::EmptyBlock { block: block_index });
            }
            let mut lines = Vec::with_capacity(block.lines.len());
            for (line_index, raw) in block.lines.into_iter().enumerate() {
                let line: String = raw.nfc().collect();
                if line.chars().any(is_forbidden) {
                    return Err(DocumentError::ForbiddenCharacter {
                        block: block_index,
                        line: line_index,
                    });
                }
                if line.trim().is_empty() {
                    return Err(DocumentError::BlankLine {
                        block: block_index,
                        line: line_index,
                    });
                }
                total += line.chars().count();
                if total > MAX_DOCUMENT_CHARS {
                    return Err(DocumentError::TooLong);
                }
                lines.push(line);
            }
            blocks.push(lines);
        }
        Ok(Self { blocks })
    }

    pub fn blocks(&self) -> impl ExactSizeIterator<Item = &[String]> {
        self.blocks.iter().map(Vec::as_slice)
    }

    pub fn first_line(&self) -> Option<&str> {
        self.blocks
            .first()
            .and_then(|b| b.first())
            .map(String::as_str)
    }

    pub fn to_data(&self) -> DocumentData {
        DocumentData {
            schema_version: SCHEMA_VERSION,
            blocks: self
                .blocks
                .iter()
                .map(|lines| BlockData {
                    lines: lines.clone(),
                })
                .collect(),
        }
    }
}

/// A work title. Working drafts may be untitled; revisions require a title.
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
    pub fn parse(raw: &str) -> Result<Self, TitleError> {
        let title: String = raw.trim().nfc().collect();
        if title.chars().any(|c| c == '\t' || is_forbidden(c)) {
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

/// Control characters and Unicode line/paragraph separators would smuggle structure into a line.
fn is_forbidden(c: char) -> bool {
    (c.is_control() && c != '\t') || c == '\u{2028}' || c == '\u{2029}'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(blocks: &[&[&str]]) -> DocumentData {
        DocumentData {
            schema_version: SCHEMA_VERSION,
            blocks: blocks
                .iter()
                .map(|lines| BlockData {
                    lines: lines.iter().map(|l| l.to_string()).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn keeps_stanzas_lines_indentation_and_vietnamese_marks() {
        let input = data(&[
            &[
                "Phố cũ mưa giăng sợi mỏng,",
                "  mái ngói rêu xanh lặng thầm.",
            ],
            &["Đêm nay ai thức cùng trăng?"],
        ]);
        let doc = Document::parse(input.clone()).unwrap();
        assert_eq!(doc.to_data(), input);
        assert_eq!(doc.blocks().len(), 2);
    }

    #[test]
    fn normalizes_combining_marks_to_nfc() {
        // "Việt" typed with combining marks: e + U+0323 (dot below) + U+0302 (circumflex).
        let decomposed = "Vie\u{0323}\u{0302}t";
        let doc = Document::parse(data(&[&[decomposed]])).unwrap();
        assert_eq!(doc.first_line(), Some("Việt"));
        assert_eq!(Title::parse(decomposed).unwrap().as_str(), "Việt");
    }

    #[test]
    fn rejects_structure_smuggled_into_lines() {
        assert_eq!(
            Document::parse(data(&[&["một\nhai"]])),
            Err(DocumentError::ForbiddenCharacter { block: 0, line: 0 })
        );
        assert_eq!(
            Document::parse(data(&[&["một", "   "]])),
            Err(DocumentError::BlankLine { block: 0, line: 1 })
        );
        assert_eq!(
            Document::parse(data(&[&[]])),
            Err(DocumentError::EmptyBlock { block: 0 })
        );
        assert_eq!(
            Document::parse(data(&[&["a\u{2028}b"]])),
            Err(DocumentError::ForbiddenCharacter { block: 0, line: 0 })
        );
    }

    #[test]
    fn rejects_unknown_schema_and_oversized_documents() {
        let mut unknown = data(&[&["x"]]);
        unknown.schema_version = 2;
        assert_eq!(
            Document::parse(unknown),
            Err(DocumentError::UnsupportedSchemaVersion(2))
        );
        let long = "ư".repeat(MAX_DOCUMENT_CHARS + 1);
        assert_eq!(
            Document::parse(data(&[&[long.as_str()]])),
            Err(DocumentError::TooLong)
        );
    }

    #[test]
    fn title_is_trimmed_single_line_and_bounded() {
        assert_eq!(Title::parse("  Mưa  ").unwrap().as_str(), "Mưa");
        assert!(Title::parse("").unwrap().is_empty());
        assert_eq!(Title::parse("a\nb"), Err(TitleError::ForbiddenCharacter));
        assert_eq!(
            Title::parse(&"ơ".repeat(MAX_TITLE_CHARS + 1)),
            Err(TitleError::TooLong)
        );
    }

    #[test]
    fn stored_shape_rejects_unknown_fields() {
        let raw = r#"{"schema_version":1,"blocks":[],"html":"<b>x</b>"}"#;
        assert!(serde_json::from_str::<DocumentData>(raw).is_err());
    }
}
