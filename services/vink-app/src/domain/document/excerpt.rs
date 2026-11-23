//! Bounded plain-text excerpt derived strictly from a published document body.

use unicode_normalization::UnicodeNormalization;
use vink_markdown_text::{Event, HeadingLevel, Tag, TagEnd, commonmark};

use super::{Document, DocumentFormat};

/// Maximum number of lines (including blank stanza breaks) shown on a discovery card.
pub const MAX_EXCERPT_LINES: usize = 4;

/// Maximum Unicode scalar characters in a public discovery card excerpt.
pub const MAX_EXCERPT_CHARS: usize = 220;

// Upper NFD expansion headroom (1 base letter + multiple combining marks) before NFC pass.
const RAW_CHAR_BUDGET: usize = MAX_EXCERPT_CHARS * 4 + 32;

/// Keep at most a short prefix, even when a Markdown text event holds a complete chapter.
/// Delay whitespace until more text arrives so trailing blanks never produce an ellipsis.
#[derive(Default)]
struct Excerpt {
    text: String,
    raw_chars: usize,
    lines: usize,
    pending_breaks: usize,
    spaces: String,
    space_count: usize,
    truncated: bool,
}

impl Excerpt {
    fn line_break(&mut self, count: usize) {
        self.pending_breaks = count.min(2);
        self.spaces.clear();
        self.space_count = 0;
    }

    fn push_text(&mut self, text: &str) {
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\n' || ch == '\r' {
                if ch == '\r' {
                    chars.next_if_eq(&'\n');
                }
                self.line_break(self.pending_breaks + 1);
            } else if ch.is_whitespace() {
                if !self.text.is_empty() && self.space_count < RAW_CHAR_BUDGET {
                    self.spaces.push(ch);
                    self.space_count += 1;
                }
            } else {
                let breaks = if self.text.is_empty() {
                    0
                } else {
                    self.pending_breaks
                };
                if self.lines + breaks >= MAX_EXCERPT_LINES {
                    self.truncated = true;
                    return;
                }
                if self.raw_chars + breaks + self.space_count + 1 > RAW_CHAR_BUDGET {
                    self.truncated = true;
                    return;
                }
                self.text.extend(std::iter::repeat_n('\n', breaks));
                self.text.push_str(&self.spaces);
                self.text.push(ch);
                self.raw_chars += breaks + self.space_count + 1;
                self.lines += breaks;
                self.line_break(0);
            }
        }
    }

    fn markdown(&mut self, source: &str) {
        let mut skip_title = false;
        let mut found_title = false;
        for event in commonmark(source) {
            if skip_title {
                if matches!(event, Event::End(TagEnd::Heading(HeadingLevel::H1))) {
                    skip_title = false;
                }
                continue;
            }
            match event {
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H1,
                    ..
                }) if !found_title => {
                    found_title = true;
                    skip_title = true;
                }
                Event::Text(text) | Event::Code(text) => self.push_text(&text),
                Event::Start(
                    Tag::Paragraph
                    | Tag::Heading { .. }
                    | Tag::BlockQuote(_)
                    | Tag::CodeBlock(_)
                    | Tag::List(_),
                )
                | Event::End(
                    TagEnd::Paragraph
                    | TagEnd::Heading(_)
                    | TagEnd::BlockQuote(_)
                    | TagEnd::CodeBlock
                    | TagEnd::List(_),
                )
                | Event::Rule => self.line_break(2),
                Event::Start(Tag::Item)
                | Event::End(TagEnd::Item)
                | Event::SoftBreak
                | Event::HardBreak => {
                    self.line_break(self.pending_breaks.max(1));
                }
                _ => {}
            }
            if self.truncated {
                break;
            }
        }
    }
}

/// Project text from the publication's immutable revision. Markdown's first H1 has already
/// supplied the title. Image and link labels are text; their destinations are metadata.
pub fn public_excerpt(document: &Document) -> Option<String> {
    let mut excerpt = Excerpt::default();
    match document.format() {
        DocumentFormat::Default => excerpt.push_text(document.source()),
        DocumentFormat::Markdown => excerpt.markdown(document.source()),
    }
    if excerpt.text.is_empty() {
        return None;
    }
    let text: String = excerpt.text.nfc().collect();
    let len = text.chars().count();
    if len > MAX_EXCERPT_CHARS || (excerpt.truncated && len == MAX_EXCERPT_CHARS) {
        Some(truncate_with_ellipsis(&text, MAX_EXCERPT_CHARS))
    } else if excerpt.truncated && !ends_sentence(&text) {
        Some(format!("{text}…"))
    } else {
        Some(text)
    }
}

fn ends_sentence(text: &str) -> bool {
    text.ends_with(['.', '!', '?', '…'])
}

fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let body_budget = max_chars.saturating_sub(1);
    let cutoff_byte = text
        .char_indices()
        .nth(body_budget)
        .map_or(text.len(), |(idx, _)| idx);
    let prefix = &text[..cutoff_byte];
    let ends_at_word_boundary = text[cutoff_byte..]
        .chars()
        .next()
        .is_none_or(char::is_whitespace);

    let base = if ends_at_word_boundary {
        prefix.trim_end()
    } else if let Some(ws_idx) = prefix.rfind(char::is_whitespace) {
        let candidate = prefix[..ws_idx].trim_end();
        if candidate.is_empty() {
            prefix.trim_end()
        } else {
            candidate
        }
    } else {
        prefix.trim_end()
    };

    let mut out = base.to_string();
    if !ends_sentence(&out) {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_doc(source: &str) -> Document {
        Document::new(DocumentFormat::Default, source.to_string()).unwrap()
    }

    fn markdown_doc(source: &str) -> Document {
        Document::new(DocumentFormat::Markdown, source.to_string()).unwrap()
    }

    #[test]
    fn empty_or_whitespace_only_has_no_excerpt() {
        assert_eq!(public_excerpt(&default_doc("")), None);
        assert_eq!(public_excerpt(&default_doc("   \n\n  \t ")), None);
        assert_eq!(
            public_excerpt(&markdown_doc("# Chỉ có tiêu đề\n\n   ")),
            None
        );
        assert_eq!(
            public_excerpt(&markdown_doc("# Tiêu đề\n\n![](https://example.com/a.png)")),
            None
        );
    }

    #[test]
    fn markdown_keeps_authored_image_labels_without_destinations() {
        assert_eq!(
            public_excerpt(&markdown_doc("# Tên\n\n![Bến cũ](image.jpg)")).as_deref(),
            Some("Bến cũ")
        );
        assert_eq!(
            public_excerpt(&markdown_doc("# Tên\n\nMưa ![rơi](image.jpg) nhẹ")).as_deref(),
            Some("Mưa rơi nhẹ")
        );
    }

    #[test]
    fn default_preserves_stanza_breaks_and_limits_to_four_lines() {
        let poem = "\n\nTrăm năm trong cõi người ta,\nChữ tài chữ mệnh khéo là ghét nhau.\n\nTrải qua một cuộc bể dâu,\nNhững điều trông thấy mà đau đớn lòng.\nLạ gì bỉ sắc tư phong.";
        assert_eq!(
            public_excerpt(&default_doc(poem)).as_deref(),
            Some(
                "Trăm năm trong cõi người ta,\nChữ tài chữ mệnh khéo là ghét nhau.\n\nTrải qua một cuộc bể dâu,…"
            )
        );
    }

    #[test]
    fn default_truncates_on_char_budget_without_cutting_words() {
        let long = "Buổi sáng trên phố cũ, từng giọt mưa rơi khẽ xuống mái ngói rêu phong và những bậc thềm đá ẩm ướt mùi đất mới sau cơn giông đêm qua; người bán hàng rong bước chậm qua ngã tư yên ắng với gánh hoa cúc vàng rực dưới ánh đèn mờ nhạt.";
        let excerpt = public_excerpt(&default_doc(long)).unwrap();
        let prefix = excerpt.strip_suffix('…').unwrap();
        assert!(excerpt.chars().count() <= MAX_EXCERPT_CHARS);
        assert!(long.starts_with(prefix));
        assert!(
            long[prefix.len()..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        );
    }

    #[test]
    fn markdown_skips_first_h1_strips_syntax_and_keeps_verse_breaks() {
        let md = "# Mưa trên **phố** cũ\n\n## Khúc mở\n\nGiọt *mưa* rơi khẽ\nQua hàng [hiên xưa](https://example.com) với `tiếng` lá.\n\nĐoạn kết ngắn.";
        assert_eq!(
            public_excerpt(&markdown_doc(md)).as_deref(),
            Some("Khúc mở\n\nGiọt mưa rơi khẽ\nQua hàng hiên xưa với tiếng lá.")
        );
    }

    #[test]
    fn terminal_punctuation_survives_line_and_character_cutoffs() {
        for ending in [".", "!", "?", "…"] {
            let stanza = format!("Bến cũ\n\nChiều về\nVen sông{ending}");
            let source = format!("{stanza}\nDòng kế").nfd().collect::<String>();
            assert_eq!(public_excerpt(&default_doc(&source)), Some(stanza));

            let prefix = format!("{}{ending}", "a".repeat(MAX_EXCERPT_CHARS - 2));
            let source = format!("{prefix} câu tiếp theo");
            assert_eq!(public_excerpt(&default_doc(&source)), Some(prefix));
        }
    }

    #[test]
    fn normalizes_nfd_vietnamese_to_nfc() {
        let nfd = "Mu\u{031b}a tre\u{0302}n pho\u{0302}\u{0301} cu\u{0303}";
        assert_eq!(
            public_excerpt(&default_doc(nfd)).as_deref(),
            Some("Mưa trên phố cũ")
        );
    }
}
