//! Moderation authority: admins, grants and groups (ADMIN-MODERATION-20261111, MOD-09, MOD-11).
//!
//! The directory is loaded by an adapter and re-read at every read or decision; nothing here is
//! cached authority. [`ModerationDirectory::resolve`] answers whether one actor may review one
//! exact target *now*: an active admin, or at least one unrevoked, unexpired grant of the right
//! capability and scope held directly or through a group the actor is a member of *now*. A
//! suspended account holds nothing. Reviewers recuse from content they wrote and from comments on
//! their own works, admins included.
//!
//! Comment grants created by the existing operator command (`vink-app moderator grant`) are
//! inherited as global [`ReviewCapability::Comments`] grants ([`GrantOrigin::InheritedComment`]);
//! they never confer work review.
//!
//! A [`VerifiedAuthority`] is evidence of one check and has no public constructor:
//! ```compile_fail
//! use vink_app::domain::moderation::authority::VerifiedAuthority;
//! let authority: VerifiedAuthority = serde_json::from_str("{}").unwrap();
//! ```
//! Lawful twin (stable rustdoc ignores compile-fail error codes):
//! ```
//! use vink_app::domain::ids::{PrincipalId, WorkId};
//! use vink_app::domain::moderation::authority::{
//!     DirectoryFacts, ModerationDirectory, ReviewCapability, ReviewTarget,
//! };
//! let raw: serde_json::Value = serde_json::from_str("{}").unwrap();
//! let admin = PrincipalId::parse("44444444-4444-4444-8444-444444444444").unwrap();
//! let author = PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap();
//! let work = WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap();
//! let directory = ModerationDirectory::restore(DirectoryFacts {
//!     admins: vec![admin],
//!     suspended: vec![],
//!     grants: vec![],
//!     groups: vec![],
//! })
//! .unwrap();
//! let target = ReviewTarget::work(work, author);
//! let now = time::OffsetDateTime::UNIX_EPOCH;
//! assert_eq!(directory.resolve(admin, &target, now).unwrap().actor(), admin);
//! assert!(raw.is_object());
//! ```

use time::OffsetDateTime;

use crate::domain::ids::{GrantId, GroupId, PrincipalId, WorkId};

/// What a grant lets its holder review. Work and comment review are independent (MOD-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewCapability {
    Works,
    Comments,
}

impl ReviewCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Works => "works",
            Self::Comments => "comments",
        }
    }
}

/// Where a grant applies: all of Vink, or one work (its revisions or the comments on it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantScope {
    Global,
    Work(WorkId),
}

impl GrantScope {
    const fn covers(self, work: WorkId) -> bool {
        match self {
            Self::Global => true,
            Self::Work(scoped) => scoped.as_uuid().as_u128() == work.as_uuid().as_u128(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grantee {
    Principal(PrincipalId),
    Group(GroupId),
}

/// An admin-issued grant, or a comment moderator grant inherited from the operator command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantOrigin {
    Admin,
    InheritedComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantFacts {
    pub id: GrantId,
    pub grantee: Grantee,
    pub capability: ReviewCapability,
    pub scope: GrantScope,
    pub expires_at: Option<OffsetDateTime>,
    pub revoked: bool,
    pub origin: GrantOrigin,
}

impl GrantFacts {
    fn live(&self, now: OffsetDateTime) -> bool {
        !self.revoked && self.expires_at.is_none_or(|at| now < at)
    }
}

/// A group managed by an admin; membership is what it is at the moment of the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupFacts {
    pub id: GroupId,
    pub members: Vec<PrincipalId>,
}

/// Untrusted row-loading shape; validated by [`ModerationDirectory::restore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryFacts {
    pub admins: Vec<PrincipalId>,
    pub suspended: Vec<PrincipalId>,
    pub grants: Vec<GrantFacts>,
    pub groups: Vec<GroupFacts>,
}

/// One exact thing to review: a work revision (owner = author) or a comment (owner = writer) on
/// a work whose author is `work_owner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewTarget {
    pub capability: ReviewCapability,
    pub work: WorkId,
    pub content_owner: PrincipalId,
    pub work_owner: PrincipalId,
}

impl ReviewTarget {
    pub const fn work(work: WorkId, author: PrincipalId) -> Self {
        Self {
            capability: ReviewCapability::Works,
            work,
            content_owner: author,
            work_owner: author,
        }
    }

    pub const fn comment(work: WorkId, writer: PrincipalId, work_author: PrincipalId) -> Self {
        Self {
            capability: ReviewCapability::Comments,
            work,
            content_owner: writer,
            work_owner: work_author,
        }
    }

    const fn conflicted(&self, actor: PrincipalId) -> bool {
        actor.as_uuid().as_u128() == self.content_owner.as_uuid().as_u128()
            || actor.as_uuid().as_u128() == self.work_owner.as_uuid().as_u128()
    }
}

/// Recorded with every conclusion: why this actor could decide (MOD-11, audit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityBasis {
    Admin,
    Grant(GrantId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AuthorityRefusal {
    #[error("no live admin seat or grant covers this target")]
    NotAuthorized,
    #[error("account is suspended")]
    Suspended,
    #[error("reviewer must recuse from their own content or their own work")]
    Recused,
    #[error("admin authority required")]
    NotAdmin,
}

impl AuthorityRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotAuthorized => "not_authorized",
            Self::Suspended => "suspended",
            Self::Recused => "recused",
            Self::NotAdmin => "not_admin",
        }
    }
}

/// Evidence that `actor` could review `target` at `checked_at`. Not reusable as a standing
/// permission: every later command checks again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedAuthority {
    actor: PrincipalId,
    basis: AuthorityBasis,
    target: ReviewTarget,
    checked_at: OffsetDateTime,
}

impl VerifiedAuthority {
    pub const fn actor(self) -> PrincipalId {
        self.actor
    }

    pub const fn basis(self) -> AuthorityBasis {
        self.basis
    }

    pub const fn target(self) -> ReviewTarget {
        self.target
    }

    pub const fn checked_at(self) -> OffsetDateTime {
        self.checked_at
    }

    pub const fn is_admin(self) -> bool {
        matches!(self.basis, AuthorityBasis::Admin)
    }
}

/// Evidence that `actor` held a live admin seat at `checked_at` (admin-only commands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminSeat {
    actor: PrincipalId,
    checked_at: OffsetDateTime,
}

impl AdminSeat {
    pub const fn actor(self) -> PrincipalId {
        self.actor
    }

    pub const fn checked_at(self) -> OffsetDateTime {
        self.checked_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DirectoryError {
    #[error("stored directory facts are inconsistent")]
    InvalidFacts,
    #[error(transparent)]
    Refused(#[from] AuthorityRefusal),
    #[error("grant identity already exists")]
    DuplicateGrant,
    #[error("unknown grant")]
    UnknownGrant,
    #[error("unknown group")]
    UnknownGroup,
    #[error("an inherited comment grant is only global comment review for one principal")]
    InvalidInheritance,
    #[error("the requested change already holds")]
    Unchanged,
}

/// Admin command to create a grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantSpec {
    pub grantee: Grantee,
    pub capability: ReviewCapability,
    pub scope: GrantScope,
    pub expires_at: Option<OffsetDateTime>,
}

/// Audit row of an accepted directory change (actor, change, time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryChange {
    Granted(GrantId),
    Revoked(GrantId),
    Joined { group: GroupId, member: PrincipalId },
    Left { group: GroupId, member: PrincipalId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectoryAudit {
    pub admin: PrincipalId,
    pub change: DirectoryChange,
    pub at: OffsetDateTime,
}

/// Validated moderation directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationDirectory(DirectoryFacts);

impl ModerationDirectory {
    pub fn restore(facts: DirectoryFacts) -> Result<Self, DirectoryError> {
        for (i, grant) in facts.grants.iter().enumerate() {
            if facts.grants[..i].iter().any(|g| g.id == grant.id) {
                return Err(DirectoryError::InvalidFacts);
            }
            if let Grantee::Group(group) = grant.grantee
                && !facts.groups.iter().any(|g| g.id == group)
            {
                return Err(DirectoryError::InvalidFacts);
            }
            if grant.origin == GrantOrigin::InheritedComment
                && (grant.capability != ReviewCapability::Comments
                    || grant.scope != GrantScope::Global
                    || !matches!(grant.grantee, Grantee::Principal(_)))
            {
                return Err(DirectoryError::InvalidInheritance);
            }
        }
        for (i, group) in facts.groups.iter().enumerate() {
            if facts.groups[..i].iter().any(|g| g.id == group.id) {
                return Err(DirectoryError::InvalidFacts);
            }
            for (j, member) in group.members.iter().enumerate() {
                if group.members[..j].contains(member) {
                    return Err(DirectoryError::InvalidFacts);
                }
            }
        }
        Ok(Self(facts))
    }

    pub fn facts(&self) -> &DirectoryFacts {
        &self.0
    }

    fn suspended(&self, actor: PrincipalId) -> bool {
        self.0.suspended.contains(&actor)
    }

    fn holds(&self, actor: PrincipalId, grantee: Grantee) -> bool {
        match grantee {
            Grantee::Principal(holder) => holder == actor,
            Grantee::Group(group) => self
                .0
                .groups
                .iter()
                .any(|g| g.id == group && g.members.contains(&actor)),
        }
    }

    /// Every basis this actor has for this target now, admin first, then grants in stored order.
    fn bases(
        &self,
        actor: PrincipalId,
        target: &ReviewTarget,
        now: OffsetDateTime,
    ) -> Vec<AuthorityBasis> {
        let mut bases = Vec::new();
        if self.0.admins.contains(&actor) {
            bases.push(AuthorityBasis::Admin);
        }
        bases.extend(
            self.0
                .grants
                .iter()
                .filter(|g| {
                    g.live(now)
                        && g.capability == target.capability
                        && g.scope.covers(target.work)
                        && self.holds(actor, g.grantee)
                })
                .map(|g| AuthorityBasis::Grant(g.id)),
        );
        bases
    }

    /// Fresh check for one actor and one target (MOD-11).
    pub fn resolve(
        &self,
        actor: PrincipalId,
        target: &ReviewTarget,
        now: OffsetDateTime,
    ) -> Result<VerifiedAuthority, AuthorityRefusal> {
        self.resolve_preferring(actor, target, now, None)
    }

    /// Like [`Self::resolve`], keeping `prefer` as the basis when it is still live. Used when a
    /// claimed case is re-checked: a moderator who lost one grant keeps the case through another
    /// valid one (MOD-12).
    pub fn resolve_preferring(
        &self,
        actor: PrincipalId,
        target: &ReviewTarget,
        now: OffsetDateTime,
        prefer: Option<AuthorityBasis>,
    ) -> Result<VerifiedAuthority, AuthorityRefusal> {
        if self.suspended(actor) {
            return Err(AuthorityRefusal::Suspended);
        }
        let bases = self.bases(actor, target, now);
        if bases.is_empty() {
            return Err(AuthorityRefusal::NotAuthorized);
        }
        if target.conflicted(actor) {
            return Err(AuthorityRefusal::Recused);
        }
        let basis = prefer.filter(|p| bases.contains(p)).unwrap_or(bases[0]);
        Ok(VerifiedAuthority {
            actor,
            basis,
            target: *target,
            checked_at: now,
        })
    }

    pub fn require_admin(
        &self,
        actor: PrincipalId,
        now: OffsetDateTime,
    ) -> Result<AdminSeat, AuthorityRefusal> {
        if self.suspended(actor) {
            return Err(AuthorityRefusal::Suspended);
        }
        if !self.0.admins.contains(&actor) {
            return Err(AuthorityRefusal::NotAdmin);
        }
        Ok(AdminSeat {
            actor,
            checked_at: now,
        })
    }

    /// Whether this actor has at least one active grant for this capability, or is an admin.
    pub fn has_any_capability(
        &self,
        actor: PrincipalId,
        capability: ReviewCapability,
        now: OffsetDateTime,
    ) -> bool {
        if self.suspended(actor) {
            return false;
        }
        if self.0.admins.contains(&actor) {
            return true;
        }
        self.0
            .grants
            .iter()
            .any(|g| g.live(now) && g.capability == capability && self.holds(actor, g.grantee))
    }

    fn audit(seat: AdminSeat, change: DirectoryChange) -> DirectoryAudit {
        DirectoryAudit {
            admin: seat.actor,
            change,
            at: seat.checked_at,
        }
    }

    /// Admin creates a grant. Inherited comment grants come only from the existing operator
    /// store; an admin-created grant is always [`GrantOrigin::Admin`].
    pub fn grant(
        &self,
        seat: AdminSeat,
        id: GrantId,
        spec: GrantSpec,
    ) -> Result<(Self, DirectoryAudit), DirectoryError> {
        self.require_admin(seat.actor, seat.checked_at)?;
        if self.0.grants.iter().any(|g| g.id == id) {
            return Err(DirectoryError::DuplicateGrant);
        }
        if let Grantee::Group(group) = spec.grantee
            && !self.0.groups.iter().any(|g| g.id == group)
        {
            return Err(DirectoryError::UnknownGroup);
        }
        let mut next = self.clone();
        next.0.grants.push(GrantFacts {
            id,
            grantee: spec.grantee,
            capability: spec.capability,
            scope: spec.scope,
            expires_at: spec.expires_at,
            revoked: false,
            origin: GrantOrigin::Admin,
        });
        Ok((next, Self::audit(seat, DirectoryChange::Granted(id))))
    }

    /// Admin revokes a grant; the row stays for audit.
    pub fn revoke(
        &self,
        seat: AdminSeat,
        id: GrantId,
    ) -> Result<(Self, DirectoryAudit), DirectoryError> {
        self.require_admin(seat.actor, seat.checked_at)?;
        let mut next = self.clone();
        let grant = next
            .0
            .grants
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or(DirectoryError::UnknownGrant)?;
        if grant.revoked {
            return Err(DirectoryError::Unchanged);
        }
        grant.revoked = true;
        Ok((next, Self::audit(seat, DirectoryChange::Revoked(id))))
    }

    /// Admin adds or removes a group member.
    pub fn set_membership(
        &self,
        seat: AdminSeat,
        group: GroupId,
        member: PrincipalId,
        present: bool,
    ) -> Result<(Self, DirectoryAudit), DirectoryError> {
        self.require_admin(seat.actor, seat.checked_at)?;
        let mut next = self.clone();
        let slot = next
            .0
            .groups
            .iter_mut()
            .find(|g| g.id == group)
            .ok_or(DirectoryError::UnknownGroup)?;
        let is_member = slot.members.contains(&member);
        if is_member == present {
            return Err(DirectoryError::Unchanged);
        }
        if present {
            slot.members.push(member);
        } else {
            slot.members.retain(|m| *m != member);
        }
        let change = if present {
            DirectoryChange::Joined { group, member }
        } else {
            DirectoryChange::Left { group, member }
        };
        Ok((next, Self::audit(seat, change)))
    }
}
