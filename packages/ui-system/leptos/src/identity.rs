//! `avatar-identity-v1`: initials and avatar tone from caller-supplied facts.
//!
//! Pure functions shared by contract with CMP (`VinkIdentity.kt`); both renderers test the same
//! golden vectors from `shared/fixtures/identity.json`. Nothing here looks up a profile, an email
//! or an image, and a list position never chooses a colour.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub use crate::identity_vectors::{TONES, VERSION};

/// Letters (BMP, not marks) and ASCII digits start an initial; emoji and punctuation never do.
/// Supplementary-plane letters are excluded so Rust and Kotlin (UTF-16 `Char`) agree.
fn starts_initial(c: char) -> bool {
    c.is_ascii_digit() || (c.is_alphabetic() && !is_combining_mark(c) && (c as u32) <= 0xFFFF)
}

/// The first letter or digit of a word, with any combining marks NFC left after it.
fn initial(word: &str) -> Option<String> {
    let mut chars = word.chars().skip_while(|c| !starts_initial(*c));
    let first = chars.next()?;
    let mut out = String::from(first);
    out.extend(chars.take_while(|c| is_combining_mark(*c)));
    Some(out)
}

/// First and last word of the NFC display name, uppercased; `None` means show the neutral mark.
/// "An Nhiên" → "AN", "Nguyễn" → "N", blank or emoji-only → `None`.
pub fn initials(display_name: Option<&str>) -> Option<String> {
    let name: String = display_name?.nfc().collect();
    let words: Vec<String> = name.split_whitespace().filter_map(initial).collect();
    let first = words.first()?;
    let mut letters = first.clone();
    if words.len() > 1 {
        letters.push_str(words.last()?);
    }
    Some(letters.to_uppercase())
}

/// 32-bit FNV-1a; small, specified and identical on every platform.
fn fnv1a(bytes: impl IntoIterator<Item = u8>) -> u32 {
    bytes.into_iter().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// Tone `1..=TONES` from a stable identity key (an ID, never an email or a list index);
/// `None` for a missing or empty key, which renders the neutral tone.
pub fn tone(identity_key: Option<&str>) -> Option<u8> {
    let key = identity_key.filter(|k| !k.is_empty())?;
    let hash = fnv1a(key.nfc().collect::<String>().into_bytes());
    Some(1 + (hash % u32::from(TONES)) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity_vectors::{INITIALS, PEOPLE, TONE_VECTORS};

    #[test]
    fn initials_match_the_shared_golden_vectors() {
        for (name, expected) in INITIALS {
            assert_eq!(initials(*name).as_deref(), *expected, "{name:?}");
        }
    }

    #[test]
    fn tones_match_the_shared_golden_vectors_and_ignore_composition() {
        for (key, expected) in TONE_VECTORS {
            assert_eq!(tone(*key), *expected, "{key:?}");
        }
        assert_eq!(tone(Some("Nguyễn")), tone(Some("Nguye\u{0302}\u{0303}n")));
    }

    #[test]
    fn same_initials_can_differ_by_key_and_order_never_matters() {
        let people: Vec<_> = PEOPLE.iter().filter(|(k, _)| k.is_some()).collect();
        let forward: Vec<_> = people.iter().map(|(k, _)| tone(*k)).collect();
        let mut backward: Vec<_> = people.iter().rev().map(|(k, _)| tone(*k)).collect();
        backward.reverse();
        assert_eq!(forward, backward);
        assert_eq!(initials(Some("Trần Linh")), initials(Some("Trần Lâm")));
        assert_ne!(tone(Some("acct-001")), tone(Some("acct-002")));
    }
}
