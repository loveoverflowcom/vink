//! Pure policy of the native email-code authority (D-05/S-03 spike): address normalisation,
//! one-time codes, challenge handles, session credentials and their hashes, and the decision for
//! one code attempt. No I/O except the operating system's random generator.
//!
//! The authority proves *control of a mailbox* when the person signs in. That is the assurance of
//! a password account with email reset, without a password to store or reuse; it is not an
//! authenticator in the NIST SP 800-63B sense (email "SHALL NOT be used for out-of-band
//! authentication"). Phishing-resistant factors (passkeys) are a later, separate credential type
//! on the same account. See `docs/architecture/auth-authority.md`.
//!
//! Secrets never reach `Debug`, logs or error messages: every secret type prints `<redacted>`.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::Duration;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// The authority identifier in every subject this service issues. It names the native account
/// system, not a factor: a later passkey or password on the same account keeps the same subject.
pub const AUTHORITY: &str = "vink-identity";

const SECRET_BYTES: usize = 32;
const CODE_DIGITS: usize = 6;
const SESSION_PREFIX: &str = "vink_s1_";
const CHALLENGE_PREFIX: &str = "vink_c1.";

/// Limits of the spike. Values follow the cited guidance where one exists (NIST SP 800-63B-4:
/// 30 days overall at AAL1; at least 64 bits of session secret) and are otherwise provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub challenge_ttl: Duration,
    /// Wrong codes allowed per challenge; the next wrong code closes it.
    pub max_attempts: i32,
    /// Challenges per address within [`Policy::throttle_window`], any purpose.
    pub max_challenges_per_window: i64,
    pub throttle_window: Duration,
    /// Verified email changes an account may start within [`Policy::account_window`].
    pub max_email_changes_per_window: i64,
    pub account_window: Duration,
    /// Challenges across all addresses within [`Policy::global_window`]: a ceiling on mail volume
    /// (and cost) when requests rotate addresses and networks. Operators tune it
    /// (`VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT`); reaching it refuses new codes for everyone, which
    /// is the intended degradation under a flood.
    pub max_challenges_globally: i64,
    pub global_window: Duration,
    /// Absolute session lifetime; there is no sliding renewal.
    pub session_ttl: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            challenge_ttl: Duration::minutes(10),
            max_attempts: 5,
            max_challenges_per_window: 3,
            throttle_window: Duration::minutes(15),
            max_email_changes_per_window: 3,
            account_window: Duration::hours(1),
            max_challenges_globally: 200,
            global_window: Duration::minutes(10),
            session_ttl: Duration::days(30),
        }
    }
}

/// Fills `buf` from the operating system. An error means no secret can be made: callers answer
/// "unavailable", never a weaker value.
pub fn random(buf: &mut [u8]) -> Result<(), RandomUnavailable> {
    getrandom::fill(buf).map_err(|_| RandomUnavailable)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomUnavailable;

/// A mailbox address normalised for comparison: trimmed, NFC, lower case. It is a login handle
/// and a delivery target, never an identity key (the subject is the account id).
#[derive(Clone, PartialEq, Eq)]
pub struct EmailAddress(String);

impl EmailAddress {
    /// Accepts `local@domain` with a non-empty local part (≤ 64 bytes), a dotted domain without
    /// empty labels (≤ 253 bytes), no whitespace or control characters and ≤ 254 bytes in total.
    /// Provider-specific rewriting (dots, `+tags`) is not applied: two spellings are two mailboxes.
    pub fn parse(raw: &str) -> Option<Self> {
        let normalized: String = raw.trim().to_lowercase().nfc().collect();
        if normalized.len() > 254
            || normalized
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return None;
        }
        let (local, domain) = normalized.split_once('@')?;
        let labels_ok = domain.split('.').count() >= 2
            && domain
                .split('.')
                .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'));
        let valid = !local.is_empty()
            && local.len() <= 64
            && !domain.contains('@')
            && domain.len() <= 253
            && labels_ok;
        valid.then_some(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EmailAddress(<redacted>)")
    }
}

/// A six-digit one-time code as the person typed it.
#[derive(Clone, PartialEq, Eq)]
pub struct Code([u8; CODE_DIGITS]);

impl Code {
    /// Exactly six ASCII digits; the App strips spaces before sending.
    pub fn parse(raw: &str) -> Option<Self> {
        let bytes = raw.as_bytes();
        (bytes.len() == CODE_DIGITS && bytes.iter().all(u8::is_ascii_digit)).then(|| {
            let mut code = [0; CODE_DIGITS];
            code.copy_from_slice(bytes);
            Self(code)
        })
    }

    /// Uniform over 000000–999999 (rejection sampling, no modulo bias).
    pub fn generate() -> Result<Self, RandomUnavailable> {
        const RANGE: u32 = 1_000_000;
        let limit = u32::MAX - (u32::MAX % RANGE);
        loop {
            let mut raw = [0u8; 4];
            random(&mut raw)?;
            let value = u32::from_le_bytes(raw);
            if value < limit {
                let text = format!("{:06}", value % RANGE);
                return Ok(Self::parse(&text).expect("six digits"));
            }
        }
    }

    /// The digits, for the mail body only.
    pub fn expose(&self) -> &str {
        std::str::from_utf8(&self.0).expect("ASCII digits")
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Code(<redacted>)")
    }
}

/// What the requesting client keeps between asking for a code and typing it: the challenge id
/// and a secret that only the client holds. The database stores `H(secret, code)`, so neither a
/// leaked row nor a leaked mail is enough to answer the challenge, and a code typed into another
/// browser that lacks the handle is useless.
#[derive(Clone, PartialEq, Eq)]
pub struct ChallengeHandle {
    pub id: Uuid,
    secret: [u8; SECRET_BYTES],
}

impl ChallengeHandle {
    pub fn generate(id: Uuid) -> Result<Self, RandomUnavailable> {
        let mut secret = [0; SECRET_BYTES];
        random(&mut secret)?;
        Ok(Self { id, secret })
    }

    /// `vink_c1.<uuid simple>.<base64url secret>`; anything else is not a handle.
    pub fn parse(raw: &str) -> Option<Self> {
        let rest = raw.strip_prefix(CHALLENGE_PREFIX)?;
        let (id, secret) = rest.split_once('.')?;
        if id.len() != 32 {
            return None;
        }
        let id = Uuid::try_parse(id).ok()?;
        let bytes = URL_SAFE_NO_PAD.decode(secret).ok()?;
        let secret: [u8; SECRET_BYTES] = bytes.try_into().ok()?;
        Some(Self { id, secret })
    }

    pub fn encode(&self) -> String {
        format!(
            "{CHALLENGE_PREFIX}{}.{}",
            self.id.simple(),
            URL_SAFE_NO_PAD.encode(self.secret)
        )
    }

    /// The stored verifier for `code` under this handle's secret.
    pub fn code_hash(&self, code: &Code) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-identity/email-code/v1\0");
        hasher.update(self.secret);
        hasher.update(code.0);
        hasher.finalize().into()
    }

    /// Constant-time comparison with a stored verifier.
    pub fn matches(&self, code: &Code, stored: &[u8]) -> bool {
        stored.len() == 32 && bool::from(self.code_hash(code).ct_eq(stored))
    }
}

impl fmt::Debug for ChallengeHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChallengeHandle")
            .field("id", &self.id)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// An opaque session credential: 256 random bits. Only its hash is stored; the value exists in
/// the response that issues it and in the client's HttpOnly cookie or secure storage.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionCredential([u8; SECRET_BYTES]);

impl SessionCredential {
    pub fn generate() -> Result<Self, RandomUnavailable> {
        let mut bytes = [0; SECRET_BYTES];
        random(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// `vink_s1_<base64url>`; any other string is not a credential this authority issued.
    pub fn parse(raw: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD
            .decode(raw.strip_prefix(SESSION_PREFIX)?)
            .ok()?;
        Some(Self(bytes.try_into().ok()?))
    }

    /// The value handed to the caller exactly once.
    pub fn encode(&self) -> String {
        format!("{SESSION_PREFIX}{}", URL_SAFE_NO_PAD.encode(self.0))
    }

    /// Lookup key in `sessions.token_hash`. The input has 256 bits of entropy, so an unsalted
    /// hash is not guessable; it keeps a database leak from yielding usable credentials.
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-identity/session/v1\0");
        hasher.update(self.0);
        hasher.finalize().into()
    }
}

impl fmt::Debug for SessionCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionCredential(<redacted>)")
    }
}

/// The stored state of a challenge when a code arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeState {
    pub consumed: bool,
    pub expired: bool,
    pub failed_attempts: i32,
}

/// Decision for one attempt. `Accept` consumes the challenge; `Wrong` records a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    Accept,
    /// Wrong code; `closes` when this failure uses the last allowed attempt.
    Wrong {
        closes: bool,
    },
    /// Expired, consumed or out of attempts: nothing is recorded and the person must restart.
    Closed,
}

/// A closed challenge never accepts, even with the right code; a consumed one is never reused.
pub fn decide_attempt(state: ChallengeState, code_matches: bool, policy: &Policy) -> Attempt {
    if state.consumed || state.expired || state.failed_attempts >= policy.max_attempts {
        Attempt::Closed
    } else if code_matches {
        Attempt::Accept
    } else {
        Attempt::Wrong {
            closes: state.failed_attempts + 1 >= policy.max_attempts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_normalised_but_not_rewritten() {
        let a = EmailAddress::parse("  Tac.Gia+Tho@Vi-Du.VN ").unwrap();
        assert_eq!(a.as_str(), "tac.gia+tho@vi-du.vn");
        // NFC: a decomposed "ư" + combining hook equals the precomposed spelling.
        let composed = EmailAddress::parse("người@ví-dụ.vn").unwrap();
        let decomposed =
            EmailAddress::parse("ngu\u{031b}o\u{031b}\u{0300}i@vi\u{0301}-du\u{0323}.vn");
        assert_eq!(Some(composed.clone()), decomposed);
        // Provider-specific rewriting is not applied.
        assert_ne!(
            EmailAddress::parse("tacgia@vi-du.vn"),
            EmailAddress::parse("tac.gia@vi-du.vn")
        );
        assert!(!format!("{composed:?}").contains("người"));
        for bad in [
            "",
            "khong-co-a-cong",
            "@vi-du.vn",
            "a@",
            "a@localhost",
            "a@b..vn",
            "a@-b.vn",
            "a b@vi-du.vn",
            "a@b@vi-du.vn",
            "a\u{0007}@vi-du.vn",
            &format!("{}@vi-du.vn", "a".repeat(65)),
            &format!("a@{}.vn", "b".repeat(254)),
        ] {
            assert_eq!(EmailAddress::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn codes_are_six_digits_and_uniformly_spread() {
        for bad in ["", "12345", "1234567", "12345a", "١٢٣٤٥٦", " 123456"] {
            assert_eq!(Code::parse(bad), None, "{bad:?}");
        }
        let mut first_digit = [0u32; 10];
        for _ in 0..5_000 {
            let code = Code::generate().unwrap();
            assert_eq!(Code::parse(code.expose()), Some(code.clone()));
            first_digit[usize::from(code.0[0] - b'0')] += 1;
        }
        // A coarse sanity bound (expected 500 each), not a statistical proof of uniformity.
        assert!(
            first_digit.iter().all(|n| (300..700).contains(n)),
            "{first_digit:?}"
        );
        assert_eq!(
            format!("{:?}", Code::parse("123456").unwrap()),
            "Code(<redacted>)"
        );
    }

    #[test]
    fn handles_round_trip_and_bind_the_code_to_the_secret() {
        let handle = ChallengeHandle::generate(Uuid::now_v7()).unwrap();
        let encoded = handle.encode();
        assert_eq!(ChallengeHandle::parse(&encoded), Some(handle.clone()));
        let code = Code::parse("042917").unwrap();
        let stored = handle.code_hash(&code);
        assert!(handle.matches(&code, &stored));
        assert!(!handle.matches(&Code::parse("042918").unwrap(), &stored));
        assert!(!handle.matches(&code, &stored[..31]));
        // Same id and code, different secret: the stored verifier does not match.
        let other = ChallengeHandle::generate(handle.id).unwrap();
        assert!(!other.matches(&code, &stored));
        let secret_text = encoded.rsplit('.').next().unwrap();
        assert!(!format!("{handle:?}").contains(secret_text));
        for bad in [
            "",
            "vink_c1.",
            &encoded.replace("vink_c1.", "vink_c2."),
            &format!("{encoded}x"),
            &encoded[..encoded.len() - 2],
            &format!("vink_c1.{}.{}", handle.id.hyphenated(), secret_text),
        ] {
            assert_eq!(ChallengeHandle::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn session_credentials_are_opaque_and_hashed() {
        let credential = SessionCredential::generate().unwrap();
        let encoded = credential.encode();
        assert!(encoded.starts_with("vink_s1_"));
        assert_eq!(encoded.len(), "vink_s1_".len() + 43);
        assert_eq!(SessionCredential::parse(&encoded), Some(credential.clone()));
        assert_ne!(
            credential.hash(),
            SessionCredential::generate().unwrap().hash()
        );
        assert!(!format!("{credential:?}").contains(&encoded[8..]));
        for bad in [
            "",
            "fixture-credential-accepted",
            "vink_s1_",
            &encoded[..encoded.len() - 1],
            &encoded.replace("vink_s1_", "vink_s2_"),
        ] {
            assert_eq!(SessionCredential::parse(bad), None, "{bad:?}");
        }
    }

    /// Every reachable state × answer within the attempt bound (a finite exhaustive check).
    #[test]
    fn attempts_never_accept_a_closed_challenge_and_close_on_the_last_failure() {
        let policy = Policy::default();
        for consumed in [false, true] {
            for expired in [false, true] {
                for failed_attempts in 0..=policy.max_attempts + 1 {
                    for code_matches in [false, true] {
                        let state = ChallengeState {
                            consumed,
                            expired,
                            failed_attempts,
                        };
                        let decision = decide_attempt(state, code_matches, &policy);
                        let open = !consumed && !expired && failed_attempts < policy.max_attempts;
                        match decision {
                            Attempt::Accept => assert!(open && code_matches, "{state:?}"),
                            Attempt::Wrong { closes } => {
                                assert!(open && !code_matches, "{state:?}");
                                assert_eq!(closes, failed_attempts + 1 == policy.max_attempts);
                            }
                            Attempt::Closed => assert!(!open, "{state:?}"),
                        }
                    }
                }
            }
        }
    }
}
