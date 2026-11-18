//! The Web side of the one canonical document (`format` + exact `source`).
//!
//! The editor sends its text verbatim: no blank-line collapsing, no line-ending or Unicode
//! rewriting. Reading paragraphs out of a default document is a presentation projection only.

use leptos::prelude::*;
use unicode_normalization::UnicodeNormalization;

use crate::contract::{DocumentDto, DocumentFormatDto};

/// The desk's text as a default document, byte for byte.
pub fn default_document(text: &str) -> DocumentDto {
    DocumentDto {
        format: DocumentFormatDto::Default,
        source: text.to_owned(),
    }
}

/// The desk's text as a Markdown document, byte for byte.
pub fn markdown_document(text: &str) -> DocumentDto {
    DocumentDto {
        format: DocumentFormatDto::Markdown,
        source: text.to_owned(),
    }
}

/// Why the desk's CodeMirror editor cannot open a document without changing it on the next save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotEditable {
    /// CodeMirror normalizes `\r\n`/`\r` to `\n`; saving would silently change line endings.
    CarriageReturn,
}

/// The desk opens a document only when saving its buffer would return the same source bytes.
pub fn desk_editable(document: &DocumentDto) -> Result<(), NotEditable> {
    if document.source.contains('\r') {
        Err(NotEditable::CarriageReturn)
    } else {
        Ok(())
    }
}

/// Whether the source holds any visible text (blank and whitespace-only lines do not count).
pub fn has_text(document: &DocumentDto) -> bool {
    !document.source.trim().is_empty()
}

/// Result of previewing an intentional format conversion (#262).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionPreview {
    pub target_format: DocumentFormatDto,
    pub title: String,
    pub source: String,
    pub note: String,
}

fn escape_leading_atx_hashes(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for (idx, line) in source.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let trimmed_start = line.trim_start_matches([' ', '\t']);
        let indent_len = line.len() - trimmed_start.len();
        if indent_len <= 3 && trimmed_start.starts_with('#') {
            out.push_str(&line[..indent_len]);
            out.push('\\');
            out.push_str(trimmed_start);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Computes an explicit, lossless conversion preview between `default` and `markdown` (#262).
pub fn preview_format_conversion(
    from: DocumentFormatDto,
    to: DocumentFormatDto,
    default_title: &str,
    markdown_h1_title: Option<&str>,
    source: &str,
) -> ConversionPreview {
    if from == to {
        return ConversionPreview {
            target_format: to,
            title: match to {
                DocumentFormatDto::Default => default_title.trim().nfc().collect(),
                DocumentFormatDto::Markdown => markdown_h1_title
                    .map(str::to_string)
                    .unwrap_or_else(|| markdown_first_h1(source)),
            },
            source: source.to_owned(),
            note: "Giữ nguyên định dạng hiện tại.".into(),
        };
    }
    match (from, to) {
        (DocumentFormatDto::Default, DocumentFormatDto::Markdown) => {
            let clean_title: String = default_title.trim().nfc().collect();
            let escaped = escape_leading_atx_hashes(source);
            let next_source = if clean_title.is_empty() {
                escaped
            } else if escaped.trim().is_empty() {
                format!("# {clean_title}")
            } else {
                format!("# {clean_title}\n\n{escaped}")
            };
            let note = if clean_title.is_empty() {
                "Giữ nguyên chữ nguồn sang Markdown; các dấu # ở đầu dòng được escape (\\#) để không đổi nghĩa."
            } else {
                "Tiêu đề riêng được đặt làm H1 đầu tiên (# …); các dấu # ở đầu dòng có sẵn được escape (\\#)."
            };
            ConversionPreview {
                target_format: DocumentFormatDto::Markdown,
                title: clean_title,
                source: next_source,
                note: note.into(),
            }
        }
        (DocumentFormatDto::Markdown, DocumentFormatDto::Default) => {
            let extracted: String = markdown_h1_title
                .map(|t| t.trim().nfc().collect())
                .unwrap_or_else(|| markdown_first_h1(source));
            ConversionPreview {
                target_format: DocumentFormatDto::Default,
                title: extracted,
                source: source.to_owned(),
                note: "H1 đầu tiên được dùng làm tiêu đề riêng; toàn bộ chữ nguồn được giữ nguyên không bị mất ký tự.".into(),
            }
        }
        _ => unreachable!(),
    }
}

/// Lightweight CommonMark first-H1 extractor used when the CodeMirror AST is not mounted yet.
pub fn markdown_first_h1(source: &str) -> String {
    let lines: Vec<&str> = source.lines().flat_map(|line| line.split('\r')).collect();
    let mut fence: Option<(char, usize)> = None;
    let mut prev_line: Option<&str> = None;

    for line in lines {
        let trimmed = line.trim_start_matches(' ');
        let leading_spaces = line.len() - trimmed.len();
        if let Some((fc, flen)) = fence {
            if leading_spaces <= 3
                && trimmed.starts_with(fc)
                && trimmed.chars().take_while(|&c| c == fc).count() >= flen
                && trimmed.trim_start_matches(fc).trim().is_empty()
            {
                fence = None;
            }
            prev_line = None;
            continue;
        }
        if leading_spaces <= 3 {
            for fc in ['`', '~'] {
                let count = trimmed.chars().take_while(|&c| c == fc).count();
                if count >= 3 && (fc == '~' || !trimmed[count..].contains('`')) {
                    fence = Some((fc, count));
                    prev_line = None;
                    break;
                }
            }
            if fence.is_some() {
                continue;
            }
        }
        if leading_spaces >= 4 || line.starts_with('\t') {
            prev_line = None;
            continue;
        }
        let unquoted = strip_quote_list_prefix(trimmed);
        if let Some(rest) = unquoted.strip_prefix('#')
            && (rest.is_empty() || rest.starts_with([' ', '\t']))
        {
            let heading = strip_atx_closing_hashes(rest.trim());
            return strip_basic_inline_markdown(heading).trim().nfc().collect();
        }
        if let Some(prev) = prev_line
            && !prev.trim().is_empty()
            && !unquoted.is_empty()
            && unquoted.trim_end().chars().all(|c| c == '=')
        {
            return strip_basic_inline_markdown(prev.trim())
                .trim()
                .nfc()
                .collect();
        }
        prev_line = if unquoted.trim().is_empty() {
            None
        } else {
            Some(unquoted)
        };
    }
    String::new()
}

fn strip_quote_list_prefix(mut line: &str) -> &str {
    loop {
        let trimmed = line.trim_start_matches(' ');
        if let Some(rest) = trimmed.strip_prefix('>') {
            line = rest.strip_prefix(' ').unwrap_or(rest);
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            line = rest;
            continue;
        }
        return trimmed;
    }
}

fn strip_atx_closing_hashes(text: &str) -> &str {
    let trimmed = text.trim_end_matches([' ', '\t']);
    let without_hashes = trimmed.trim_end_matches('#');
    if without_hashes.len() < trimmed.len()
        && (without_hashes.is_empty() || without_hashes.ends_with([' ', '\t']))
    {
        without_hashes.trim_end_matches([' ', '\t'])
    } else {
        trimmed
    }
}

fn strip_basic_inline_markdown(input: &str) -> String {
    input
        .replace("\\#", "#")
        .replace("\\*", "*")
        .replace("**", "")
        .replace(['*', '`'], "")
}

/// Paragraphs and stanzas of a default document for reading: a run of blank or whitespace-only
/// lines separates blocks, a single line break stays inside one, and indentation is kept. LF, CRLF and
/// CR each count as one break. The source itself is never changed.
pub fn paragraphs(source: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in source.lines().flat_map(|line| line.split('\r')) {
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

enum PreviewBlock {
    Heading { level: usize, text: String },
    Quote(Vec<String>),
    Code(Vec<String>),
    Paragraph(Vec<String>),
}

fn markdown_preview_blocks(source: &str) -> Vec<PreviewBlock> {
    let mut blocks = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut quote: Vec<String> = Vec::new();
    let mut code: Vec<String> = Vec::new();
    let mut fence: Option<char> = None;

    let flush_para = |blocks: &mut Vec<PreviewBlock>, para: &mut Vec<String>| {
        if !para.is_empty() {
            blocks.push(PreviewBlock::Paragraph(std::mem::take(para)));
        }
    };
    let flush_quote = |blocks: &mut Vec<PreviewBlock>, quote: &mut Vec<String>| {
        if !quote.is_empty() {
            blocks.push(PreviewBlock::Quote(std::mem::take(quote)));
        }
    };

    for line in source.lines().flat_map(|line| line.split('\r')) {
        let trimmed = line.trim_start_matches(' ');
        if let Some(fc) = fence {
            if trimmed.starts_with(fc) && trimmed.chars().take_while(|&c| c == fc).count() >= 3 {
                blocks.push(PreviewBlock::Code(std::mem::take(&mut code)));
                fence = None;
            } else {
                code.push(line.to_owned());
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            flush_para(&mut blocks, &mut para);
            flush_quote(&mut blocks, &mut quote);
            fence = trimmed.chars().next();
            continue;
        }
        if line.trim().is_empty() {
            flush_para(&mut blocks, &mut para);
            flush_quote(&mut blocks, &mut quote);
            continue;
        }
        if let Some(q) = trimmed.strip_prefix('>') {
            flush_para(&mut blocks, &mut para);
            quote.push(q.strip_prefix(' ').unwrap_or(q).to_owned());
            continue;
        }
        flush_quote(&mut blocks, &mut quote);
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) {
            let rest = &trimmed[hashes..];
            if rest.is_empty() || rest.starts_with([' ', '\t']) {
                flush_para(&mut blocks, &mut para);
                blocks.push(PreviewBlock::Heading {
                    level: hashes,
                    text: strip_atx_closing_hashes(rest.trim()).to_owned(),
                });
                continue;
            }
        }
        para.push(line.to_owned());
    }
    if fence.is_some() {
        blocks.push(PreviewBlock::Code(code));
    }
    flush_para(&mut blocks, &mut para);
    flush_quote(&mut blocks, &mut quote);
    blocks
}

/// Default documents render as paragraphs; CSS `white-space: pre-wrap` keeps line breaks and
/// indentation. Markdown in the private draft preview renders safe escaped blocks without HTML execution.
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
        DocumentFormatDto::Markdown => {
            let blocks = markdown_preview_blocks(&document.source)
                .into_iter()
                .map(|block| match block {
                    PreviewBlock::Heading { level, text } => view! {
                        <p class=format!("vk-block vk-preview-md-h{level}")>{text}</p>
                    }
                    .into_any(),
                    PreviewBlock::Quote(lines) => view! {
                        <blockquote class="vk-block vk-block--lines vk-preview-md-quote">
                            {lines.join("\n")}
                        </blockquote>
                    }
                    .into_any(),
                    PreviewBlock::Code(lines) => view! {
                        <pre class="vk-block vk-preview-md-code"><code>{lines.join("\n")}</code></pre>
                    }
                    .into_any(),
                    PreviewBlock::Paragraph(lines) => {
                        let class = if lines.len() > 1 {
                            "vk-block vk-block--lines"
                        } else {
                            "vk-block"
                        };
                        view! { <p class=class>{lines.join("\n")}</p> }.into_any()
                    }
                })
                .collect_view();
            view! {
                <div class="vk-document" data-format="markdown">
                    {blocks}
                </div>
            }
            .into_any()
        }
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
        assert_eq!(
            paragraphs("a\rb\r\rc\r\n d"),
            vec![vec!["a", "b"], vec!["c", " d"]]
        );
        assert!(paragraphs("  \n\n").is_empty());
        assert!(!has_text(&default_document(" \n\t\n")));
        assert!(has_text(&default_document("\n# x")));
    }

    #[test]
    fn the_desk_opens_default_and_markdown_without_carriage_returns() {
        assert_eq!(desk_editable(&default_document("  Thơ\n\n\nKhổ\t")), Ok(()));
        assert_eq!(
            desk_editable(&default_document("Một\r\nHai")),
            Err(NotEditable::CarriageReturn)
        );
        let markdown = markdown_document("# Mưa");
        assert_eq!(desk_editable(&markdown), Ok(()));
        assert_eq!(
            desk_editable(&markdown_document("# Mưa\r\nThân")),
            Err(NotEditable::CarriageReturn)
        );
    }

    #[test]
    fn conversion_preview_preserves_text_and_escapes_hashes() {
        let to_md = preview_format_conversion(
            DocumentFormatDto::Default,
            DocumentFormatDto::Markdown,
            "Tiếng mưa",
            None,
            "# Không phải tiêu đề\nĐoạn thường.",
        );
        assert_eq!(to_md.target_format, DocumentFormatDto::Markdown);
        assert_eq!(to_md.title, "Tiếng mưa");
        assert_eq!(
            to_md.source,
            "# Tiếng mưa\n\n\\# Không phải tiêu đề\nĐoạn thường."
        );

        let to_default = preview_format_conversion(
            DocumentFormatDto::Markdown,
            DocumentFormatDto::Default,
            "",
            Some("Tiếng mưa"),
            "# Tiếng mưa\n\n## Hiên nhà\nNội dung.",
        );
        assert_eq!(to_default.target_format, DocumentFormatDto::Default);
        assert_eq!(to_default.title, "Tiếng mưa");
        assert_eq!(to_default.source, "# Tiếng mưa\n\n## Hiên nhà\nNội dung.");
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
