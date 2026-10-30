//! Textarea adapter for document schema v1, and the reading view.
//!
//! In a textarea, one or more blank lines separate blocks (paragraphs or stanzas) and a single
//! line break stays inside a block. Lines keep their leading spaces. Several blank lines
//! collapse into one block break; that is the only structural normalization.

use leptos::prelude::*;

use crate::contract::{BlockDto, DocumentDto};

pub const SCHEMA_VERSION: u16 = 1;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
