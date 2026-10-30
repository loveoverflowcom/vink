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

impl WorkId {
    /// Time-ordered identifier for a newly created work.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}
