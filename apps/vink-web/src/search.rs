//! Shared comparison keys for already-loaded cards. Never rewrite authored text.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Accept Vietnamese accents in NFC/NFD, Đ/đ, case and repeated whitespace.
pub fn search_key(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'đ' { 'd' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
