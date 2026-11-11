//! Username and password policy and Argon2id hashing (ACC-03, O-12).
//!
//! # Policy O-12
//! - **Username:** 3–30 characters, ASCII alphanumeric (`a–z`, `0–9`), underscore (`_`), dot (`.`).
//!   Case-insensitive (stored canonical lowercase). Never exposed publicly.
//! - **Password:** normalized to Unicode NFKC, then 10–128 characters where each Unicode code
//!   point counts as one character (`O-12@r2`). A Vietnamese letter typed precomposed or with
//!   combining marks therefore counts once and hashes identically. No mandatory character class
//!   rules (per OWASP/NIST SP 800-63B, length over arbitrary complexity). Raw input above
//!   [`MAX_PASSWORD_INPUT_BYTES`] is rejected before normalization.
//! - **Hashing:** Argon2id with random salt and PHC-formatted output.
//!   Dummy verification on missing usernames to mitigate timing side-channels.
//!
//! Secrets and usernames are never printed in clear: [`fmt::Debug`] prints `<redacted>`.

use std::fmt;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use unicode_normalization::UnicodeNormalization;

pub const MIN_USERNAME_LEN: usize = 3;
pub const MAX_USERNAME_LEN: usize = 30;
pub const MIN_PASSWORD_LEN: usize = 10;
pub const MAX_PASSWORD_LEN: usize = 128;
/// Upper bound on the raw request field before normalization. It leaves room for decomposed
/// input (several code points per character) while keeping Argon2 input and NFKC work bounded.
pub const MAX_PASSWORD_INPUT_BYTES: usize = 4096;

/// A canonical username adhering to policy O-12.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Username(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameError {
    TooShort,
    TooLong,
    InvalidCharacter,
}

impl Username {
    pub fn parse(raw: &str) -> Result<Self, UsernameError> {
        let trimmed = raw.trim();
        let bytes = trimmed.as_bytes();
        if bytes.len() < MIN_USERNAME_LEN {
            return Err(UsernameError::TooShort);
        }
        if bytes.len() > MAX_USERNAME_LEN {
            return Err(UsernameError::TooLong);
        }
        let mut canonical = String::with_capacity(bytes.len());
        for &b in bytes {
            match b {
                b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' => canonical.push(b as char),
                b'A'..=b'Z' => canonical.push((b + 32) as char),
                _ => return Err(UsernameError::InvalidCharacter),
            }
        }
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Username(<redacted>)")
    }
}

/// A plaintext password, NFKC-normalized and validated against character length bounds.
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordError {
    TooShort,
    TooLong,
}

impl Password {
    pub fn parse(raw: &str) -> Result<Self, PasswordError> {
        if raw.len() > MAX_PASSWORD_INPUT_BYTES {
            return Err(PasswordError::TooLong);
        }
        let normalized: String = raw.nfkc().collect();
        let len = normalized.chars().count();
        if len < MIN_PASSWORD_LEN {
            return Err(PasswordError::TooShort);
        }
        if len > MAX_PASSWORD_LEN {
            return Err(PasswordError::TooLong);
        }
        Ok(Self(normalized))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashError;

/// Hashes a password using Argon2id with a fresh random salt.
pub fn hash_password(password: &Password) -> Result<String, HashError> {
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.expose().as_bytes())
        .map_err(|_| HashError)?;
    Ok(hash.to_string())
}

/// Verifies a password against a stored PHC Argon2id hash.
pub fn verify_password(stored_hash: &str, password: &Password) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.expose().as_bytes(), &parsed)
        .is_ok()
}

/// Fixed valid PHC Argon2id hash used for constant-time dummy verification
/// when a username is not found.
const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$eW91cmR1bW15c2FsdA$7B9Wf2ZcT28eQx2W+Xm7O3n0gQx9G5Y2vX7Z1p4B0k0";

/// Performs dummy password verification so that missing username requests do not exit early.
pub fn dummy_verify(password: &Password) {
    let _ = verify_password(DUMMY_HASH, password);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_policy_validation() {
        assert_eq!(Username::parse("abc").unwrap().as_str(), "abc");
        assert_eq!(
            Username::parse("User_01.Dev").unwrap().as_str(),
            "user_01.dev"
        );
        assert_eq!(
            Username::parse("  name_123  ").unwrap().as_str(),
            "name_123"
        );
        assert_eq!(Username::parse("ab"), Err(UsernameError::TooShort));
        let too_long = "a".repeat(31);
        assert_eq!(Username::parse(&too_long), Err(UsernameError::TooLong));
        assert_eq!(
            Username::parse("user-name"),
            Err(UsernameError::InvalidCharacter)
        );
        assert_eq!(
            Username::parse("user@name"),
            Err(UsernameError::InvalidCharacter)
        );
        assert_eq!(
            Username::parse("user name"),
            Err(UsernameError::InvalidCharacter)
        );
        assert_eq!(
            Username::parse("người_dùng"),
            Err(UsernameError::InvalidCharacter)
        );
    }

    #[test]
    fn password_policy_validation() {
        assert!(Password::parse("1234567890").is_ok());
        assert!(Password::parse("a-very-strong-passphrase").is_ok());
        assert_eq!(Password::parse("123456789"), Err(PasswordError::TooShort));
        let too_long = "x".repeat(129);
        assert_eq!(Password::parse(&too_long), Err(PasswordError::TooLong));
        assert!(Password::parse(&"x".repeat(128)).is_ok());
    }

    #[test]
    fn password_length_counts_characters_after_nfkc() {
        // Nine Vietnamese letters: 9 characters but 19 bytes precomposed, so still too short.
        let precomposed = "mậtkhẩuạ\u{1ec7}";
        assert_eq!(precomposed.chars().count(), 9);
        assert!(precomposed.len() >= 10);
        assert_eq!(Password::parse(precomposed), Err(PasswordError::TooShort));
        // Ten letters typed with combining marks count as ten and match the precomposed form.
        let decomposed: String = "mậtkhẩuđẹp".nfd().collect();
        assert!(decomposed.chars().count() > 10);
        let from_nfd = Password::parse(&decomposed).unwrap();
        let from_nfc = Password::parse("mậtkhẩuđẹp").unwrap();
        assert_eq!(from_nfd.expose(), from_nfc.expose());
        let hash = hash_password(&from_nfc).unwrap();
        assert!(verify_password(&hash, &from_nfd));
        // 128 multi-byte characters are allowed; byte length no longer decides.
        assert!(Password::parse(&"ệ".repeat(128)).is_ok());
        assert_eq!(
            Password::parse(&"ệ".repeat(129)),
            Err(PasswordError::TooLong)
        );
        // Compatibility forms fold (full-width digits become ASCII digits).
        assert_eq!(
            Password::parse("１２３４５６７８９０").unwrap().expose(),
            "1234567890"
        );
        assert_eq!(
            Password::parse(&"a".repeat(MAX_PASSWORD_INPUT_BYTES + 1)),
            Err(PasswordError::TooLong)
        );
    }

    #[test]
    fn hashing_and_verification() {
        let pass = Password::parse("correct-horse-battery-staple").unwrap();
        let wrong = Password::parse("incorrect-horse-battery-staple").unwrap();
        let hash = hash_password(&pass).expect("hash success");
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(&hash, &pass));
        assert!(!verify_password(&hash, &wrong));
        assert!(!verify_password("invalid_phc_string", &pass));
    }

    #[test]
    fn redaction() {
        let user = Username::parse("secret_user").unwrap();
        let pass = Password::parse("secret_password_123").unwrap();
        assert_eq!(format!("{user:?}"), "Username(<redacted>)");
        assert_eq!(format!("{pass:?}"), "Password(<redacted>)");
    }
}
