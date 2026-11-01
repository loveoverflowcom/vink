//! Source-text adapter for document schema v1, the reading view and the plain-text export.
//!
//! In the editor's source text, one or more blank lines separate blocks (paragraphs or stanzas)
//! and a single line break stays inside a block. Lines keep their leading spaces. Several blank
//! lines collapse into one block break; that is the only structural normalization here (the
//! server also stores text in NFC). The export keeps the text exactly as written instead.

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

/// The text being written as a UTF-8 `.txt`: the title, a blank line, then the body exactly as
/// the editor holds it (no NFC, blank lines kept), ending with one line break.
pub fn export_text(title: &str, body: &str) -> String {
    let title = title.trim();
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    if !title.is_empty() {
        out.push_str(title);
        out.push_str("\n\n");
    }
    out.push_str(&body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A file name from the title, keeping Vietnamese letters; reserved and control characters
/// become spaces. Untitled drafts get a fixed name.
pub fn export_file_name(title: &str) -> String {
    const MAX_CHARS: usize = 80;
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let name: String = words.join(" ").chars().take(MAX_CHARS).collect();
    let name = name.trim().trim_matches('.').trim();
    if name.is_empty() {
        "ban-nhap-vink.txt".to_string()
    } else {
        format!("{name}.txt")
    }
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
    fn export_keeps_the_text_as_written() {
        let body = "Phố cũ mưa giăng,\n  mái ngói rêu.\n\n\nĐêm nay ai thức?";
        assert_eq!(
            export_text(" Mưa trên phố cũ ", body),
            format!("Mưa trên phố cũ\n\n{body}\n")
        );
        // Combining marks stay as typed; the export is not the server's normalized copy.
        let decomposed = "Vie\u{0302}\u{0323}t";
        assert_eq!(export_text("", decomposed), format!("{decomposed}\n"));
        assert_eq!(export_text("", "một\r\nhai\n"), "một\nhai\n");
        assert_eq!(export_text("", ""), "\n");
    }

    #[test]
    fn export_file_names_keep_vietnamese_and_drop_reserved_characters() {
        assert_eq!(
            export_file_name("Thử nghiệm Bàn viết"),
            "Thử nghiệm Bàn viết.txt"
        );
        assert_eq!(
            export_file_name("  a/b:c*d?\"e<f>g|h\ti  "),
            "a b c d e f g h i.txt"
        );
        assert_eq!(export_file_name("..."), "ban-nhap-vink.txt");
        assert_eq!(export_file_name(""), "ban-nhap-vink.txt");
        let long = "ư".repeat(200);
        assert_eq!(export_file_name(&long).chars().count(), 80 + ".txt".len());
    }

    #[test]
    fn empty_text_is_an_empty_document() {
        assert!(text_to_document("  \n\n").blocks.is_empty());
        assert_eq!(document_to_text(&text_to_document("")), "");
    }
}
