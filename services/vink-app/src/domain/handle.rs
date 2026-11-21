//! Public addresses are separate from identity credentials and display names.
use super::principal::PenName;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub const MAX_HANDLE_BYTES: usize = 40;

const WORDS: [&str; 8] = ["may", "truc", "sen", "gio", "la", "mua", "song", "lua"];
const RESERVED: &[&str] = &[
    "admin",
    "api",
    "vink",
    "support",
    "moderator",
    "toi",
    "ban-viet",
    "kiem-duyet",
];

pub fn valid(name: &str) -> bool {
    (1..=MAX_HANDLE_BYTES).contains(&name.len())
        && !RESERVED.contains(&name)
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// Only the pen name and independent randomness enter the generator.
pub fn candidate(pen: &PenName, entropy: u32) -> String {
    let mut stem = String::new();
    for c in pen.as_str().nfd().filter(|c| !is_combining_mark(*c)) {
        let c = match c {
            'đ' | 'Đ' => 'd',
            c => c.to_ascii_lowercase(),
        };
        if c.is_ascii_alphanumeric() {
            stem.push(c);
        } else if !stem.is_empty() && !stem.ends_with('-') {
            stem.push('-');
        }
        if stem.len() >= 27 {
            break;
        }
    }
    let stem = stem.trim_end_matches('-');
    let stem = if stem.is_empty() { "tho" } else { stem };
    format!(
        "{stem}-{}-{:06x}",
        WORDS[(entropy >> 24) as usize % WORDS.len()],
        entropy & 0xffffff
    )
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Setup,
    Rename,
}
impl Intent {
    pub fn key(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Rename => "rename",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChangeError {
    WrongIntent,
    Exhausted,
}

/// None is a confirmed no-op. Setup always advances, including keeping the suggestion.
pub fn change(
    setup_complete: bool,
    remaining: i16,
    intent: Intent,
    same_name: bool,
) -> Result<Option<i16>, ChangeError> {
    match (setup_complete, intent, same_name) {
        (false, Intent::Setup, _) => Ok(Some(2)),
        (true, Intent::Rename, true) => Ok(None),
        (true, Intent::Rename, false) if remaining > 0 => Ok(Some(remaining - 1)),
        (true, Intent::Rename, false) => Err(ChangeError::Exhausted),
        _ => Err(ChangeError::WrongIntent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handle_grammar_and_independent_suggestions() {
        for bad in [
            "",
            "Admin",
            "admin",
            "-a",
            "a-",
            "a--b",
            "a.b",
            "tên",
            "a/b",
            &"a".repeat(41),
        ] {
            assert!(!valid(bad), "{bad}");
        }
        assert!(valid(&"a".repeat(40)));
        for pen in ["Đặng Thùy", "🌱", &"Ỹ".repeat(80)] {
            for e in [0, 0xffffffff] {
                assert!(valid(&candidate(&PenName::parse(pen).unwrap(), e)));
            }
        }
        assert_eq!(
            candidate(&PenName::parse("Đặng Thùy").unwrap(), 15),
            "dang-thuy-may-00000f"
        );
    }
}
