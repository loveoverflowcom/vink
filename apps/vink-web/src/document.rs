//! The Web side of the one canonical document (`format` + exact `source`).
//!
//! The editor sends its text verbatim: no blank-line collapsing, no line-ending or Unicode
//! rewriting. Reading paragraphs out of a default document is a presentation projection only.

use leptos::prelude::*;

use crate::contract::{DocumentDto, DocumentFormatDto};

/// The desk's text as a default document, byte for byte.
pub fn default_document(text: &str) -> DocumentDto {
    DocumentDto {
        format: DocumentFormatDto::Default,
        source: text.to_owned(),
    }
}

/// Why the desk's CodeMirror editor cannot open a document without changing it on the next save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotEditable {
    /// Markdown needs its own editor and title authority (#256/#262).
    Markdown,
    /// CodeMirror normalizes `\r\n`/`\r` to `\n`; saving would silently change line endings.
    CarriageReturn,
}

/// The desk opens a document only when saving its buffer would return the same source bytes.
pub fn desk_editable(document: &DocumentDto) -> Result<(), NotEditable> {
    match document.format {
        DocumentFormatDto::Markdown => Err(NotEditable::Markdown),
        DocumentFormatDto::Default if document.source.contains('\r') => {
            Err(NotEditable::CarriageReturn)
        }
        DocumentFormatDto::Default => Ok(()),
    }
}

/// Whether the source holds any visible text (blank and whitespace-only lines do not count).
pub fn has_text(document: &DocumentDto) -> bool {
    !document.source.trim().is_empty()
}

/// Paragraphs and stanzas of a default document for reading: a run of blank or whitespace-only
/// lines separates blocks, a single line break stays inside one, and indentation is kept. A line
/// ending's `\r` is not shown. The source itself is never changed.
pub fn paragraphs(source: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in source.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

/// Default documents render as paragraphs; CSS `white-space: pre-wrap` keeps line breaks and
/// indentation. Markdown has no Reader yet (#264, and publication refuses it): its source is shown
/// verbatim as escaped text, never interpreted as HTML.
#[component]
pub fn DocumentView(document: DocumentDto) -> impl IntoView {
    match document.format {
        DocumentFormatDto::Default => {
            let blocks = paragraphs(&document.source)
                .into_iter()
                .map(|lines| {
                    let class = if lines.len() > 1 {
                        "vk-block vk-block--lines"
                    } else {
                        "vk-block"
                    };
                    view! { <p class=class>{lines.join("\n")}</p> }
                })
                .collect_view();
            view! { <div class="vk-document">{blocks}</div> }.into_any()
        }
        DocumentFormatDto::Markdown => view! {
            <div class="vk-document" data-format="markdown">
                <p class="vk-block vk-block--lines">{document.source}</p>
            </div>
        }
        .into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_text_is_sent_exactly() {
        let text = "\r\n  Tiếng Việt ❤️‍🔥 👍🏽 👩‍👩‍👧‍👦 🇻🇳 1️⃣\n\n\n   \n\tVie\u{0323}\u{0302}t\r\n";
        let document = default_document(text);
        assert_eq!(document.format, DocumentFormatDto::Default);
        assert_eq!(document.source, text);
        let wire = serde_json::to_string(&document).unwrap();
        let back: DocumentDto = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, document);
    }

    #[test]
    fn paragraphs_are_a_reading_projection() {
        let source = "Phố cũ mưa giăng,\r\n  mái ngói rêu.\n\n\n \t \nĐêm nay ai thức?\n";
        assert_eq!(
            paragraphs(source),
            vec![
                vec!["Phố cũ mưa giăng,", "  mái ngói rêu."],
                vec!["Đêm nay ai thức?"],
            ]
        );
        assert!(paragraphs("  \n\n").is_empty());
        assert!(!has_text(&default_document(" \n\t\n")));
        assert!(has_text(&default_document("\n# x")));
    }

    #[test]
    fn the_desk_opens_only_documents_it_saves_back_unchanged() {
        assert_eq!(desk_editable(&default_document("  Thơ\n\n\nKhổ\t")), Ok(()));
        assert_eq!(
            desk_editable(&default_document("Một\r\nHai")),
            Err(NotEditable::CarriageReturn)
        );
        let markdown = DocumentDto {
            format: DocumentFormatDto::Markdown,
            source: "# Mưa".into(),
        };
        assert_eq!(desk_editable(&markdown), Err(NotEditable::Markdown));
    }

    #[test]
    fn unknown_formats_and_old_shapes_do_not_decode() {
        for raw in [
            r#"{"format":"asciidoc","source":"= X"}"#,
            r#"{"format":"html","source":"<b>x</b>"}"#,
            r#"{"schema_version":1,"blocks":[]}"#,
            r#"{"format":"default","source":"","schema_version":2}"#,
        ] {
            assert!(serde_json::from_str::<DocumentDto>(raw).is_err(), "{raw}");
        }
    }
}
