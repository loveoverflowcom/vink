//! Field disclosure for author profiles. Provisional fields have no value storage.
use serde::{Deserialize, Serialize};

pub const MAX_BIO_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Everyone,
    #[serde(rename = "self")]
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePolicies {
    pub pen_name: Visibility,
    pub avatar: Visibility,
    pub bio: Visibility,
    pub date_of_birth: Visibility,
    pub followers_list: Visibility,
    pub followers_count: Visibility,
    pub following_list: Visibility,
    pub following_count: Visibility,
}

impl Default for ProfilePolicies {
    fn default() -> Self {
        use Visibility::{Everyone, Owner};
        Self {
            pen_name: Everyone,
            avatar: Everyone,
            bio: Everyone,
            date_of_birth: Owner,
            followers_list: Everyone,
            followers_count: Everyone,
            following_list: Owner,
            following_count: Owner,
        }
    }
}

impl ProfilePolicies {
    /// SOC-OPEN-01 leaves the confirmation for widening access unresolved.
    pub fn permits_change(self, next: Self) -> bool {
        use Visibility::{Everyone, Owner};
        next.pen_name == Everyone
            && next.date_of_birth == Owner
            && [
                (self.avatar, next.avatar),
                (self.bio, next.bio),
                (self.followers_list, next.followers_list),
                (self.followers_count, next.followers_count),
                (self.following_list, next.following_list),
                (self.following_count, next.following_count),
            ]
            .into_iter()
            .all(|change| change != (Owner, Everyone))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bio(String);

impl Bio {
    pub fn parse(raw: &str) -> Option<Self> {
        let value = raw.trim().replace("\r\n", "\n");
        let valid = value.chars().count() <= MAX_BIO_CHARS
            && !value.chars().any(|c| c.is_control() && c != '\n');
        valid.then_some(Self(value))
    }

    pub fn value(&self) -> Option<&str> {
        (!self.0.is_empty()).then_some(self.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_narrow_independently_and_hold_widening() {
        use Visibility::{Everyone, Owner};
        let initial = ProfilePolicies::default();
        let narrow = ProfilePolicies {
            bio: Owner,
            followers_list: Owner,
            ..initial
        };
        assert!(initial.permits_change(narrow));
        assert!(!narrow.permits_change(initial));
        assert!(!initial.permits_change(ProfilePolicies {
            pen_name: Owner,
            ..initial
        }));
        assert!(!initial.permits_change(ProfilePolicies {
            date_of_birth: Everyone,
            ..initial
        }));
    }

    #[test]
    fn bio_preserves_vietnamese_and_paragraphs() {
        assert_eq!(
            Bio::parse(" Gió\r\n\r\nMưa ").unwrap().value(),
            Some("Gió\n\nMưa")
        );
        assert_eq!(Bio::parse(" \n ").unwrap().value(), None);
        assert!(Bio::parse("mưa\0gió").is_none());
        assert!(Bio::parse(&"ế".repeat(MAX_BIO_CHARS + 1)).is_none());
    }
}
