//! Nominal identities: a principal, work and revision cannot substitute for one another.
//! Raw text is parsed at its boundary; SQL UUIDs are already structurally valid. No UUID
//! version restriction is imposed (existing fixtures use v4 and new works use v7).
//!
//! ```compile_fail
//! use vink_app::domain::ids::{PrincipalId, WorkId};
//! let actor = PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap();
//! let work: WorkId = actor;
//! ```
//! ```compile_fail
//! use vink_app::domain::ids::{RevisionId, WorkId};
//! let work = WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap();
//! let revision: RevisionId = work;
//! ```
//! Lawful twin of both blocks: stable rustdoc ignores a `compile_fail` error code, so a typo in
//! their setup would also "fail to compile". The same setup must compile, leaving the
//! substitution as the only error.
//! ```
//! use vink_app::domain::ids::{PrincipalId, RevisionId, WorkId};
//! let actor = PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap();
//! let work = WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap();
//! let (_actor, _work): (PrincipalId, WorkId) = (actor, work);
//! let _revision: RevisionId = RevisionId::from_uuid(work.as_uuid());
//! ```

use uuid::Uuid;

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub fn parse(raw: &str) -> Option<Self> {
                Uuid::parse_str(raw).ok().map(Self)
            }

            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(
    /// Stable Vink principal (`vink_user_id`), independent of username, email or a provider subject.
    PrincipalId
);
id_type!(
    /// Stable identity of a work; drafts, revisions and the public pointer hang off it.
    WorkId
);
id_type!(
    /// Identity of one immutable revision snapshot.
    RevisionId
);
id_type!(
    /// Discussion scope. Its binding to a work/chapter belongs to the future owner adapter.
    ThreadId
);
id_type!(
    /// Stable identity of a root comment or a flat reply, not a tree path.
    CommentId
);
id_type!(
    /// Canonical identity of a catalog tag (#95). Its label, slug and search key may change or
    /// collide; this identity never does, and assignments and snapshots reference only it.
    TagId
);

impl WorkId {
    /// Time-ordered identifier for a newly created work.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}

impl ThreadId {
    /// Proposed identity for a work's discussion thread. The store keeps the existing thread when
    /// one is already bound to the work, so a generated ID is a candidate, not an assertion.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}

impl CommentId {
    /// Server-chosen identity for a new comment; clients never pick comment identities.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}

impl TagId {
    /// Server-chosen identity for a new catalog tag; authors and curators never pick one.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_external_ids_are_rejected_for_every_kind() {
        for raw in [
            "",
            "not-a-uuid",
            "../33333333",
            "33333333-3333-4333-8333-33333333333g",
        ] {
            assert_eq!(PrincipalId::parse(raw), None);
            assert_eq!(WorkId::parse(raw), None);
            assert_eq!(RevisionId::parse(raw), None);
            assert_eq!(ThreadId::parse(raw), None);
            assert_eq!(CommentId::parse(raw), None);
            assert_eq!(TagId::parse(raw), None);
        }
    }

    #[test]
    fn existing_uuid_values_round_trip_without_a_new_version_policy() {
        let raw = "33333333-3333-4333-8333-333333333333";
        let id = WorkId::parse(raw).unwrap();
        assert_eq!(id.to_string(), raw);
        assert_eq!(WorkId::from_uuid(id.as_uuid()), id);
        assert_eq!(WorkId::generate().as_uuid().get_version_num(), 7);
    }
}
