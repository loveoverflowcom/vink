//! Source-text adapter for document schema v1 and the reading view.
//!
//! In the editor's source text, one or more blank lines separate blocks (paragraphs or stanzas)
//! and a single line break stays inside a block. Lines keep their leading spaces. Several blank
//! lines collapse into one block break; that is the only structural normalization here (the
//! server also stores text in NFC).

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::contract::{BlockDto, DocumentDto};

pub const SCHEMA_VERSION: u16 = 1;
pub const SOURCE_SCHEMA_VERSION: u16 = 2;

pub fn text_to_document(text: &str) -> DocumentDto {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut blocks = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(BlockDto {
                    lines: std::mem::take(&mut current),
                });
            }
        } else {
            current.push(line.to_string());
        }
    }
    if !current.is_empty() {
        blocks.push(BlockDto { lines: current });
    }
    DocumentDto {
        schema_version: SCHEMA_VERSION,
        blocks,
    }
}

pub fn document_to_text(document: &DocumentDto) -> String {
    document
        .blocks
        .iter()
        .map(|block| block.lines.join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Renders blocks as paragraphs; CSS `white-space: pre-wrap` keeps line breaks and indentation.
#[component]
pub fn DocumentView(document: DocumentDto) -> impl IntoView {
    let blocks = document
        .blocks
        .into_iter()
        .map(|block| {
            let class = if block.lines.len() > 1 {
                "vk-block vk-block--lines"
            } else {
                "vk-block"
            };
            view! { <p class=class>{block.lines.join("\n")}</p> }
        })
        .collect_view();
    view! { <div class="vk-document">{blocks}</div> }
}

/// Active v2 source formats (`VDR-DOCUMENT-SOURCE-FORMAT`, `PENDING` owner signoff).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormatDto {
    Default,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEndingDto {
    Lf,
    Crlf,
}

/// Wire shape for v2 lossless source documents. Deserialization rejects unknown fields; production
/// routes still emit/accept v1 [`DocumentDto`] until exact-revision signoff lands (`AC-05`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSourceDto {
    pub schema_version: u16,
    pub format: SourceFormatDto,
    pub source: String,
    pub line_ending: LineEndingDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveFormatDto {
    LegacyDefault,
    Explicit(SourceFormatDto),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredDocumentDto {
    V1(DocumentDto),
    V2(DocumentSourceDto),
}

impl StoredDocumentDto {
    pub fn effective_format(&self) -> EffectiveFormatDto {
        match self {
            Self::V1(_) => EffectiveFormatDto::LegacyDefault,
            Self::V2(src) => EffectiveFormatDto::Explicit(src.format),
        }
    }

    pub fn editor_text(&self) -> String {
        match self {
            Self::V1(doc) => document_to_text(doc),
            Self::V2(src) => src.source.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFormatCapability {
    pub id: &'static str,
    pub label_vi: &'static str,
    pub selectable: bool,
    pub deferred: bool,
}

const WEB_FORMAT_CAPABILITIES: [WebFormatCapability; 3] = [
    WebFormatCapability {
        id: "default",
        label_vi: "Mặc định",
        selectable: true,
        deferred: false,
    },
    WebFormatCapability {
        id: "markdown",
        label_vi: "Markdown",
        selectable: true,
        deferred: false,
    },
    WebFormatCapability {
        id: "asciidoc",
        label_vi: "AsciiDoc",
        selectable: false,
        deferred: true,
    },
];

pub fn format_capabilities() -> &'static [WebFormatCapability] {
    &WEB_FORMAT_CAPABILITIES
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentReadError {
    InvalidJson,
    ExpectedObject,
    MissingSchemaVersion,
    UnsupportedSchemaVersion(u16),
    UnexpectedFormatOnLegacyV1,
    UnknownLegacyField(String),
    DeferredFormat(String),
    UnsupportedFormat(String),
    InvalidSourceCharacter,
}

/// Boundary-isolated dual-version reader adapter (`v1` legacy + `v2` lossless source). Never sniffs
/// Markdown inside `v1` documents even when line 0 begins with `# `.
pub fn read_document_envelope(raw: &str) -> Result<StoredDocumentDto, DocumentReadError> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| DocumentReadError::InvalidJson)?;
    let map = value.as_object().ok_or(DocumentReadError::ExpectedObject)?;
    let version = map
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .and_then(|v| u16::try_from(v).ok())
        .ok_or(DocumentReadError::MissingSchemaVersion)?;

    match version {
        SCHEMA_VERSION => {
            if map.contains_key("format") {
                return Err(DocumentReadError::UnexpectedFormatOnLegacyV1);
            }
            for key in map.keys() {
                if key != "schema_version" && key != "blocks" {
                    return Err(DocumentReadError::UnknownLegacyField(key.clone()));
                }
            }
            let doc: DocumentDto =
                serde_json::from_value(value).map_err(|_| DocumentReadError::InvalidJson)?;
            Ok(StoredDocumentDto::V1(doc))
        }
        SOURCE_SCHEMA_VERSION => {
            if let Some(raw_fmt) = map.get("format").and_then(|v| v.as_str()) {
                match raw_fmt {
                    "default" | "markdown" => {}
                    "asciidoc" => {
                        return Err(DocumentReadError::DeferredFormat(raw_fmt.to_owned()));
                    }
                    other => return Err(DocumentReadError::UnsupportedFormat(other.to_owned())),
                }
            }
            let src: DocumentSourceDto =
                serde_json::from_value(value).map_err(|_| DocumentReadError::InvalidJson)?;
            if src.source.chars().any(|c| {
                c == '\r'
                    || (c.is_control() && c != '\n' && c != '\t')
                    || c == '\u{2028}'
                    || c == '\u{2029}'
            }) {
                return Err(DocumentReadError::InvalidSourceCharacter);
            }
            Ok(StoredDocumentDto::V2(src))
        }
        other => Err(DocumentReadError::UnsupportedSchemaVersion(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_sequences_survive_document_without_markup() {
        let source = "Tiếng Việt ❤️‍🔥 👍🏽 👩‍👩‍👧‍👦 🇻🇳 1️⃣\n  vẫn nguyên FE0F và ZWJ\n\n❤ và ❤️ cùng ở đây.";
        assert_eq!(document_to_text(&text_to_document(source)), source);
    }

    #[test]
    fn stanzas_lines_and_indentation_round_trip() {
        let text = "Phố cũ mưa giăng sợi mỏng,\n  mái ngói rêu xanh lặng thầm.\n\nĐêm nay ai thức cùng trăng?";
        let document = text_to_document(text);
        assert_eq!(document.blocks.len(), 2);
        assert_eq!(
            document.blocks[0].lines[1],
            "  mái ngói rêu xanh lặng thầm."
        );
        assert_eq!(document_to_text(&document), text);
    }

    #[test]
    fn windows_line_endings_and_extra_blank_lines_normalize() {
        let document = text_to_document("\r\n\r\nmột\r\nhai\r\n \r\n\r\n\tba\n\n");
        assert_eq!(
            document.blocks,
            vec![
                BlockDto {
                    lines: vec!["một".into(), "hai".into()]
                },
                BlockDto {
                    lines: vec!["\tba".into()]
                },
            ]
        );
        assert_eq!(document_to_text(&document), "một\nhai\n\n\tba");
    }

    #[test]
    fn empty_text_is_an_empty_document() {
        assert!(text_to_document("  \n\n").blocks.is_empty());
        assert_eq!(document_to_text(&text_to_document("")), "");
    }

    #[test]
    fn dual_version_reader_treats_v1_as_legacy_default_and_preserves_v2_source() {
        let legacy = read_document_envelope(
            r##"{"schema_version":1,"blocks":[{"lines":["# Không sniff thành H1"]}]}"##,
        )
        .unwrap();
        assert_eq!(legacy.effective_format(), EffectiveFormatDto::LegacyDefault);
        assert_eq!(legacy.editor_text(), "# Không sniff thành H1");

        let nfd_md = "  # Tie\u{0302}\u{0301}ng Vie\u{0323}\u{0302}t\n\n\n\t❤️\u{200d}🔥\n   \n";
        let v2_raw = serde_json::to_string(&DocumentSourceDto {
            schema_version: SOURCE_SCHEMA_VERSION,
            format: SourceFormatDto::Markdown,
            source: nfd_md.to_owned(),
            line_ending: LineEndingDto::Lf,
        })
        .unwrap();
        let v2 = read_document_envelope(&v2_raw).unwrap();
        assert_eq!(
            v2.effective_format(),
            EffectiveFormatDto::Explicit(SourceFormatDto::Markdown)
        );
        assert_eq!(v2.editor_text(), nfd_md);

        assert_eq!(
            read_document_envelope(
                r#"{"schema_version":2,"format":"asciidoc","source":"= X","line_ending":"lf"}"#,
            ),
            Err(DocumentReadError::DeferredFormat("asciidoc".to_owned()))
        );
        assert_eq!(
            read_document_envelope(r#"{"schema_version":1,"format":"markdown","blocks":[]}"#),
            Err(DocumentReadError::UnexpectedFormatOnLegacyV1)
        );
        assert_eq!(
            format_capabilities()
                .iter()
                .filter(|cap| cap.selectable)
                .map(|cap| cap.id)
                .collect::<Vec<_>>(),
            vec!["default", "markdown"]
        );
    }
}
