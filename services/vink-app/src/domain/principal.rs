//! Who acts, independent of how they signed in (D-05/S-03 spike).
//!
//! A [`SubjectKey`] is what an authority asserts (`authority`, `subject id`); it is never a
//! principal. The App binds it to exactly one [`crate::domain::ids::PrincipalId`] and decides
//! [`Standing`]. A [`PenName`] is display text: it changes freely and identifies nobody.

use unicode_normalization::UnicodeNormalization;

const MAX_KEY_CHARS: usize = 200;
const MAX_PEN_NAME_CHARS: usize = 80;
const MAX_REASON_CHARS: usize = 200;

/// `(authority, subject id)` exactly as the identity provider reported them, bounded. No
/// normalisation: two spellings are two subjects, and nothing is derived from an email address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectKey {
    authority: String,
    id: String,
}

impl SubjectKey {
    pub fn new(authority: &str, id: &str) -> Option<Self> {
        let ok = |s: &str| {
            !s.is_empty()
                && s.chars().count() <= MAX_KEY_CHARS
                && !s.chars().any(|c| c.is_control() || c.is_whitespace())
        };
        (ok(authority) && ok(id)).then(|| Self {
            authority: authority.to_string(),
            id: id.to_string(),
        })
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// The name shown on works and comments (`principals.display_name`). Trimmed, NFC, 1–80
/// characters, no control characters. Not unique and not a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PenName(String);

impl PenName {
    pub fn parse(raw: &str) -> Option<Self> {
        let text: String = raw.trim().nfc().collect();
        let valid = !text.is_empty()
            && text.chars().count() <= MAX_PEN_NAME_CHARS
            && !text.chars().any(char::is_control);
        valid.then_some(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Operator's one-line reason for a suspension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionReason(String);

impl SuspensionReason {
    pub fn parse(raw: &str) -> Option<Self> {
        let text: String = raw.trim().nfc().collect();
        let valid = !text.is_empty()
            && text.chars().count() <= MAX_REASON_CHARS
            && !text.chars().any(char::is_control);
        valid.then_some(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether a bound principal may act now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Active,
    Suspended,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_keys_are_bounded_and_never_rewritten() {
        let key = SubjectKey::new("vink-identity", "018f3c2e-0000-7000-8000-00000000a001");
        assert!(key.is_some());
        assert_ne!(
            SubjectKey::new("vink-identity", "ABC"),
            SubjectKey::new("vink-identity", "abc")
        );
        for (authority, id) in [
            ("", "x"),
            ("x", ""),
            ("vink identity", "x"),
            ("x", "a\u{0000}b"),
            ("x", &"s".repeat(201)),
        ] {
            assert_eq!(SubjectKey::new(authority, id), None, "{authority:?}/{id:?}");
        }
    }

    #[test]
    fn pen_names_are_display_text_in_nfc() {
        let composed = PenName::parse("  Nguyễn Thị Thơ ").unwrap();
        assert_eq!(composed.as_str(), "Nguyễn Thị Thơ");
        let decomposed = PenName::parse("Nguye\u{0302}\u{0303}n Thi\u{0323} Tho\u{031b}").unwrap();
        assert_eq!(decomposed, composed);
        assert!(PenName::parse(&"ơ".repeat(80)).is_some());
        for bad in ["", "   ", "a\nb", &"ơ".repeat(81)] {
            assert_eq!(PenName::parse(bad), None, "{bad:?}");
        }
        assert!(SuspensionReason::parse("Spam lặp lại, báo cáo #12").is_some());
        assert!(SuspensionReason::parse("hai\ndòng").is_none());
    }
}
