//! Structural checks at an explicit publication attempt. The editor's source is never rewritten.
use super::{Event, HeadingLevel, Tag, TagEnd, commonmark};
use pulldown_cmark::LinkType;
use std::borrow::Cow;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SubmissionIssues {
    pub title_missing: bool,
    pub body_missing: bool,
}

impl SubmissionIssues {
    pub fn is_empty(self) -> bool {
        !self.title_missing && !self.body_missing
    }
}

/// The caller supplies the authoritative format-specific title, never a display fallback.
pub fn check_submission(title: &str, body: SubmissionBody<'_>) -> SubmissionIssues {
    SubmissionIssues {
        title_missing: !title.nfc().any(|c| !invisible(c) && !is_combining_mark(c)),
        body_missing: !match body {
            SubmissionBody::Literal(text) => has_body_text(text),
            SubmissionBody::Markdown(source) => markdown_body_has_text(source),
        },
    }
}

pub enum SubmissionBody<'a> {
    Literal(&'a str),
    Markdown(&'a str),
}

/// Comments may consist of emoji or punctuation. A sequence of invisible/combining marks alone
/// has no independently visible content.
pub fn has_visible_comment_text(text: &str) -> bool {
    text.chars().any(|c| !invisible(c) && !is_combining_mark(c))
}

fn invisible(c: char) -> bool {
    c.is_whitespace()
        || c.is_control()
        || matches!(c,
        '\u{ad}' | '\u{34f}' | '\u{61c}' | '\u{115f}' | '\u{1160}' | '\u{17b4}'..='\u{17b5}' |
        '\u{180b}'..='\u{180f}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' |
        '\u{2060}'..='\u{206f}' | '\u{2800}' | '\u{3164}' | '\u{fe00}'..='\u{fe0f}' |
        '\u{feff}' | '\u{ffa0}' | '\u{fff0}'..='\u{fff8}' | '\u{1bca0}'..='\u{1bca3}' |
        '\u{1d173}'..='\u{1d17a}' | '\u{e0000}'..='\u{e0fff}')
}

fn url_token(token: &str) -> bool {
    let token = token.trim_start_matches(|c: char| !c.is_alphanumeric() && c != '/');
    if token.starts_with("//") {
        return true;
    }
    if let Some((scheme, _)) = token.split_once("://") {
        return scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'));
    }
    ["mailto:", "www."].iter().any(|prefix| {
        token
            .get(..prefix.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
    })
}

fn has_body_text(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let token: Cow<'_, str> = if token.chars().any(invisible) {
            token
                .chars()
                .filter(|&c| !invisible(c))
                .collect::<String>()
                .into()
        } else {
            Cow::Borrowed(token)
        };
        !url_token(&token)
            && token
                .nfc()
                .any(|c| c.is_alphanumeric() && !is_combining_mark(c))
    })
}

fn markdown_body_has_text(source: &str) -> bool {
    let mut saw_title = false;
    let mut skip_depth = 0usize;
    let mut line = String::new();
    for event in commonmark(source) {
        if skip_depth != 0 {
            match event {
                Event::Start(_) => skip_depth += 1,
                Event::End(_) => skip_depth -= 1,
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) if !saw_title => {
                saw_title = true;
                skip_depth = 1;
            }
            Event::Start(
                Tag::Image { .. }
                | Tag::Link {
                    link_type: LinkType::Autolink | LinkType::Email,
                    ..
                },
            ) => {
                line.push(' ');
                skip_depth = 1;
            }
            Event::Text(text) | Event::Code(text) => line.push_str(&text),
            Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::Item
                | TagEnd::BlockQuote(_)
                | TagEnd::Link,
            ) => {
                if has_body_text(&line) {
                    return true;
                }
                line.clear();
            }
            Event::Start(Tag::Link { .. }) => line.push(' '),
            _ => {}
        }
    }
    has_body_text(&line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_projection_uses_visible_body_and_first_heading_only() {
        use SubmissionBody::*;
        for source in [
            "# Mưa",
            "> Mưa\n> ===",
            "# Mưa\n\n---\n\n> \n\n```rust\n```",
            "# Mưa\n\n<!-- prose -->\n\n<div>hidden</div>",
            "# Mưa\n\n![Hoa](flowers.jpg)",
            "# Mưa\n\n[https://a.vn](https://b.vn) <https://b.vn> <x@y.vn>",
            "# Mưa\n\nhttps://a.vn/?q=x&amp;y=z",
        ] {
            assert!(
                check_submission("Mưa", Markdown(source)).body_missing,
                "{source:?}"
            );
        }
        for source in [
            "Mưa\n\n# Đêm",
            "# Đêm\n\n> Ơ",
            "# Đêm\n\n## Mưa",
            "# Đêm\n\n# Sau",
            "# Đêm\n\n[ơ](https://a.vn)",
            "# Đêm\n\n`a`",
            "💧\n\n> Đêm\n> ===\n\nu\u{31b}\u{301}",
        ] {
            assert!(
                !check_submission("Đêm", Markdown(source)).body_missing,
                "{source:?}"
            );
        }
        for source in [
            "",
            " \r\n",
            "\u{200b}\u{200c}\u{200d}\u{feff}\u{2060}\u{a0}\u{2800}\u{3164}\u{115f}",
            "\u{301}… 💫",
            "(HTTPS://a.vn) www.example.com",
            "h\u{200b}ttps://a.vn",
            "//a.vn/đường",
        ] {
            assert!(check_submission("Chưa đặt tên", Literal(source)).body_missing);
        }
        assert!(check_submission("\u{3164}", Literal("Mưa")).title_missing);
        assert!(check_submission("🌙", Literal("# Mưa")).is_empty());
        assert!(check_submission("Chưa đặt tên", Literal("ơ")).is_empty());
    }
}
