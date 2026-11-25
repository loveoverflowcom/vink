//! The Web side of the one canonical document (`format` + exact `source`).
//!
//! The editor sends its text verbatim: no blank-line collapsing, no line-ending or Unicode
//! rewriting. Reading paragraphs out of a default document is a presentation projection only.

use leptos::prelude::*;
use unicode_normalization::UnicodeNormalization;

use crate::contract::{DocumentDto, DocumentFormatDto};

mod markdown;

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
    source: &str,
) -> ConversionPreview {
    if from == to {
        return ConversionPreview {
            target_format: to,
            title: match to {
                DocumentFormatDto::Default => default_title.trim().nfc().collect(),
                DocumentFormatDto::Markdown => markdown_first_h1(source),
            },
            source: source.to_owned(),
            note: "Giữ nguyên định dạng hiện tại.".into(),
        };
    }
    match (from, to) {
        (DocumentFormatDto::Default, DocumentFormatDto::Markdown) => {
            let clean_title: String = default_title.trim().nfc().collect();
            let escaped = escape_leading_atx_hashes(source);
            // A plain title must remain literal, even when it contains a link, entity or closing #.
            let mut heading = String::new();
            for c in clean_title.chars() {
                if c.is_ascii_punctuation() {
                    heading.push('\\');
                }
                heading.push(c);
            }
            let next_source = if clean_title.is_empty() {
                escaped
            } else if escaped.is_empty() {
                format!("# {heading}")
            } else {
                format!("# {heading}\n\n{escaped}")
            };
            let note = if clean_title.is_empty() {
                "Giữ nguyên chữ nguồn sang Markdown; các dấu # ở đầu dòng được escape (\\#) để không đổi nghĩa."
            } else {
                "Tiêu đề riêng được đặt làm H1 đầu tiên (# …); các dấu # ở đầu dòng có sẵn được escape (\\#)."
            };
            ConversionPreview {
                target_format: DocumentFormatDto::Markdown,
                title: markdown_first_h1(&next_source),
                source: next_source,
                note: note.into(),
            }
        }
        (DocumentFormatDto::Markdown, DocumentFormatDto::Default) => {
            let extracted: String = markdown_first_h1(source);
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

/// The same CommonMark projection that the server materializes for this exact source.
pub fn markdown_first_h1(source: &str) -> String {
    vink_markdown_text::first_h1_projection(source)
        .trim()
        .nfc()
        .collect()
}

impl ConversionPreview {
    pub fn title_too_long(&self) -> bool {
        self.title.chars().count() > 200
    }
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

/// Ephemeral reading projection. Only the source DTO is stored or cached.
pub(crate) struct ReadingDocument<'a> {
    body: ReadingBody<'a>,
}

enum ReadingBody<'a> {
    Default(Vec<Vec<&'a str>>),
    Markdown(markdown::Markdown),
}

impl<'a> ReadingDocument<'a> {
    pub(crate) fn new(document: &'a DocumentDto) -> Self {
        let body = match document.format {
            DocumentFormatDto::Default => ReadingBody::Default(paragraphs(&document.source)),
            DocumentFormatDto::Markdown => {
                ReadingBody::Markdown(markdown::Markdown::parse(&document.source))
            }
        };
        Self { body }
    }

    /// A Markdown H1 occupies its source position. The enclosing page only supplies a
    /// separate heading for Default or for a draft that has no H1 yet.
    pub(crate) fn title_in_body(&self) -> bool {
        match &self.body {
            ReadingBody::Default(_) => false,
            ReadingBody::Markdown(markdown) => markdown.has_h1,
        }
    }

    pub(crate) fn description(&self) -> String {
        match &self.body {
            ReadingBody::Default(blocks) => blocks
                .first()
                .and_then(|lines| lines.first())
                .map(|line| (*line).to_owned())
                .unwrap_or_default(),
            ReadingBody::Markdown(markdown) => markdown.description(),
        }
    }

    pub(crate) fn render(self, title: &str) -> AnyView {
        match self.body {
            ReadingBody::Default(blocks) => {
                let blocks = blocks
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
                view! { <div class="vk-document" data-format="default">{blocks}</div> }.into_any()
            }
            ReadingBody::Markdown(markdown) => markdown.render(title),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "ssr")]
    #[test]
    fn shared_render_corpus_uses_the_closed_commonmark_projection() {
        use leptos::tachys::view::RenderHtml;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Case {
            id: String,
            format: DocumentFormatDto,
            source: String,
            title: String,
            first_h1: bool,
            description: String,
            html_tags: Vec<String>,
            literals: Vec<String>,
        }

        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../services/vink-app/tests/fixtures/title-authority/render.json"
        ))
        .unwrap();
        for case in cases {
            let document = DocumentDto {
                format: case.format,
                source: case.source,
            };
            let reading = ReadingDocument::new(&document);
            assert_eq!(reading.title_in_body(), case.first_h1, "{}", case.id);
            assert_eq!(reading.description(), case.description, "{}", case.id);
            if document.format == DocumentFormatDto::Markdown {
                assert_eq!(
                    markdown_first_h1(&document.source),
                    case.title,
                    "{}",
                    case.id
                );
            }
            let html = Owner::new()
                .with(|| {
                    reading
                        .render(&crate::draft_list::display_title(&case.title))
                        .to_html()
                })
                .replace("<!>", "");
            for tag in &case.html_tags {
                assert!(
                    html.contains(&format!("<{tag}>")) || html.contains(&format!("<{tag} ")),
                    "{} missing {tag}: {html}",
                    case.id
                );
            }
            for text in case.literals {
                assert!(html.contains(&text), "{} missing {text}: {html}", case.id);
            }
            for tag in [
                "img",
                "script",
                "vink-inline",
                "vink-raw",
                "table",
                "del",
                "input",
            ] {
                assert!(
                    !html.contains(&format!("<{tag}")),
                    "{} unexpected {tag}",
                    case.id
                );
            }
            if case.id == "network_and_html" {
                assert_eq!(html.matches("href=").count(), 3, "{html}");
                assert!(html.contains("href=\"https://preview-network.invalid/link\""));
                assert_eq!(html.matches("src=").count(), 1, "{html}");
                assert!(html.contains("&lt;img src="));
                assert!(!html.contains("href=\"javascript:"));
                assert!(!html.contains("href=\"data:"));
                assert!(!html.contains("href=\"//"));
            }
            if case.id == "default_poetry" {
                assert_eq!(html.matches("<p ").count(), 2);
                assert!(!html.contains("<h1"));
            }
            if case.id == "empty_first_h1" {
                assert!(html.contains("Chưa đặt tên"));
            }
        }
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn markdown_projection_reaches_the_end_of_a_large_document_and_bounds_nesting() {
        use leptos::tachys::view::RenderHtml;

        let start = "# Bản dài\n\n";
        let end = "\n\n## Kết thúc";
        let source = format!(
            "{start}{}{end}",
            "ơ".repeat(200_000 - (start.chars().count() + end.chars().count()))
        );
        let document = markdown_document(&source);
        let html =
            Owner::new().with(|| ReadingDocument::new(&document).render("Bản dài").to_html());
        assert!(html.contains("id=\"muc-2\">Kết thúc"));
        assert_eq!(document.source, source);

        let source = format!("{}# Một tên\n\nPhần cuối.", "> ".repeat(48));
        let document = markdown_document(&source);
        let reading = ReadingDocument::new(&document);
        assert!(reading.title_in_body());
        let html = Owner::new().with(|| reading.render("Một tên").to_html());
        assert!(html.contains("id=\"muc-1\""));
        assert_eq!(html.matches("Một tên").count(), 1);
        assert!(!html.contains("<blockquote"));
        assert!(html.contains("Phần cuối."));
    }

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
            "# Tiếng mưa\n\n## Hiên nhà\nNội dung.",
        );
        assert_eq!(to_default.target_format, DocumentFormatDto::Default);
        assert_eq!(to_default.title, "Tiếng mưa");
        assert_eq!(to_default.source, "# Tiếng mưa\n\n## Hiên nhà\nNội dung.");
    }

    #[test]
    fn conversion_uses_the_canonical_title_and_preserves_blank_source() {
        use DocumentFormatDto::{Default, Markdown};
        let title = "[mưa](url) &eacute; *gió* #";
        let converted = preview_format_conversion(Default, Markdown, title, " \n\t");
        assert_eq!(converted.title, title);
        assert!(converted.source.ends_with("\n\n \n\t"));
        assert_eq!(
            preview_format_conversion(Default, Markdown, "", "Mưa\n===").title,
            "Mưa"
        );
        assert_eq!(
            preview_format_conversion(Markdown, Default, "", "# <https://x.vn> &eacute;").title,
            "https://x.vn é"
        );
        for length in [200, 201] {
            let title = "ế".repeat(length);
            assert_eq!(
                preview_format_conversion(Default, Markdown, &title, "body").title_too_long(),
                length > 200
            );
            assert_eq!(
                preview_format_conversion(Markdown, Default, "", &format!("# {title}"))
                    .title_too_long(),
                length > 200
            );
        }
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
