//! Comment text. The only way to obtain a [`CommentBody`] is [`CommentBody::parse`], so request
//! JSON and stored rows both pass the same rules.
//!
//! Text is NFC-normalized like literary documents, line endings become `\n` and surrounding
//! whitespace is trimmed; interior blank lines (a stanza break in a quoted poem) are kept. Inline
//! emoji stay as their source code points, including ZWJ sequences and variation selectors
//! (D2T-INV-11); stickers are references to a manifest (#25), never text.
//!
//! ```compile_fail
//! use vink_app::domain::discussion::body::CommentBody;
//! let body: CommentBody = serde_json::from_str("\"x\"").unwrap();
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::body::CommentBody;
//! let raw: String = serde_json::from_str("\"x\"").unwrap();
//! let body: CommentBody = CommentBody::parse(&raw).unwrap();
//! assert_eq!(body.as_str(), "x");
//! ```

use unicode_normalization::UnicodeNormalization;

/// Upper bound in Unicode scalar values after normalization; the database checks the same bound.
pub const MAX_BODY_CHARS: usize = 4_000;
pub const MAX_BODY_LINES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentBody(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BodyError {
    #[error("comment is empty")]
    Empty,
    #[error("comment is longer than {MAX_BODY_CHARS} characters")]
    TooLong,
    #[error("comment has more than {MAX_BODY_LINES} lines")]
    TooManyLines,
    #[error("comment contains a control character")]
    ControlCharacter,
    /// Embedding, override and isolate controls can make displayed text differ from stored text.
    #[error("comment contains a bidirectional formatting control")]
    BidiControl,
}

impl CommentBody {
    pub fn parse(raw: &str) -> Result<Self, BodyError> {
        let unified = raw.replace("\r\n", "\n").replace('\r', "\n");
        let text: String = unified.trim().nfc().collect();
        if text.is_empty() {
            return Err(BodyError::Empty);
        }
        if text.chars().count() > MAX_BODY_CHARS {
            return Err(BodyError::TooLong);
        }
        if text.lines().count() > MAX_BODY_LINES {
            return Err(BodyError::TooManyLines);
        }
        for c in text.chars() {
            if c.is_control() && c != '\n' && c != '\t' {
                return Err(BodyError::ControlCharacter);
            }
            if matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}') {
                return Err(BodyError::BidiControl);
            }
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vietnamese_text_is_nfc_and_line_endings_are_unified() {
        let nfd = "Ca\u{0300} phe\u{0302}\u{0301}\r\nso\u{031B}\u{0301}m";
        let body = CommentBody::parse(nfd).unwrap();
        assert_eq!(body.as_str(), "Cà phế\nsớm");
        assert_eq!(body, CommentBody::parse("Cà phế\nsớm").unwrap());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_but_stanza_breaks_stay() {
        let body = CommentBody::parse("\n  Khổ một,\ndòng hai.\n\nKhổ hai.  \n").unwrap();
        assert_eq!(body.as_str(), "Khổ một,\ndòng hai.\n\nKhổ hai.");
    }

    #[test]
    fn emoji_sequences_keep_their_source_code_points() {
        for raw in ["👍🏽", "👩‍💻", "❤️", "🇻🇳 đẹp quá"] {
            assert_eq!(CommentBody::parse(raw).unwrap().as_str(), raw);
        }
    }

    #[test]
    fn empty_oversized_and_unsafe_text_is_rejected() {
        assert_eq!(CommentBody::parse(" \n\t "), Err(BodyError::Empty));
        let longest = "ạ".repeat(MAX_BODY_CHARS);
        assert!(CommentBody::parse(&longest).is_ok());
        assert_eq!(
            CommentBody::parse(&format!("{longest}a")),
            Err(BodyError::TooLong)
        );
        let many_lines = vec!["x"; MAX_BODY_LINES + 1].join("\n");
        assert_eq!(
            CommentBody::parse(&many_lines),
            Err(BodyError::TooManyLines)
        );
        assert_eq!(
            CommentBody::parse("a\u{0007}b"),
            Err(BodyError::ControlCharacter)
        );
        assert_eq!(
            CommentBody::parse("abc\u{202E}def"),
            Err(BodyError::BidiControl)
        );
        assert!(CommentBody::parse("cột\tbảng").is_ok());
    }
}
