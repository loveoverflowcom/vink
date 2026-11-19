//! Pure Social Follow Graph rules (#294): desired-state transitions with accepted-order
//! fencing and durable receipts (CHOICE-02 A), policy evaluation for lists/counts/Mutual,
//! global principal lock ordering, and opaque scoped cursors.
//!
//! Initial absent state is `(following=false, version=0, order=0)`. Every accepted command
//! advances `order` (including same-state intents) to fence ABA replays; `version` advances only
//! when `following` flips, so reachable states preserve `following == (version % 2 == 1)`.
//!
//! ```compile_fail
//! use vink_app::domain::follow::{FollowOrder, FollowVersion};
//! let version = FollowVersion::FIRST;
//! let expected: FollowOrder = version;
//! ```
//! ```compile_fail
//! use vink_app::domain::follow::FollowState;
//! let mut state = FollowState::INITIAL;
//! state.following = true;
//! ```
//! Lawful twin: the same public setup compiles; token substitution and field mutation do not.
//! ```
//! use vink_app::domain::follow::{FollowOrder, FollowState, FollowVersion};
//! let version = FollowVersion::FIRST;
//! let expected = FollowOrder::INITIAL;
//! let state = FollowState::INITIAL;
//! assert_eq!(version.get(), expected.get());
//! assert!(!state.following());
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use super::ids::PrincipalId;
use super::profile::{ProfilePolicies, Visibility};

pub const DEFAULT_FOLLOW_PAGE_SIZE: usize = 12;
pub const MAX_FOLLOW_PAGE_SIZE: usize = 24;
pub const MAX_MEMBERSHIP_BATCH: usize = 50;
pub const MAX_STORE_MEMBERSHIP_CANDIDATES: usize = 100;

macro_rules! non_negative_token {
    ($name:ident, $first:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i64);
        impl $name {
            pub const INITIAL: Self = Self(0);
            pub const $first: Self = Self(0);
            pub fn new(raw: i64) -> Option<Self> {
                (raw >= 0).then_some(Self(raw))
            }
            pub const fn get(self) -> i64 {
                self.0
            }
            pub fn next(self) -> Option<Self> {
                self.0.checked_add(1).map(Self)
            }
        }
    };
}

non_negative_token!(FollowVersion, FIRST);
non_negative_token!(FollowOrder, START);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FollowCommandId(Uuid);

impl FollowCommandId {
    pub const fn from_uuid(raw: Uuid) -> Self {
        Self(raw)
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCount(i64);

impl FollowCount {
    pub const ZERO: Self = Self(0);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 0).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowIntent {
    Follow,
    Unfollow,
}

impl FollowIntent {
    pub const fn following(self) -> bool {
        matches!(self, Self::Follow)
    }

    pub const fn from_following(following: bool) -> Self {
        if following {
            Self::Follow
        } else {
            Self::Unfollow
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowDirection {
    Followers,
    Following,
}

impl FollowDirection {
    pub const fn web_segment(self) -> &'static str {
        match self {
            Self::Followers => "nguoi-theo-doi",
            Self::Following => "dang-theo-doi",
        }
    }

    pub const fn api_segment(self) -> &'static str {
        match self {
            Self::Followers => "followers",
            Self::Following => "following",
        }
    }

    pub fn parse_web(raw: &str) -> Option<Self> {
        match raw {
            "nguoi-theo-doi" => Some(Self::Followers),
            "dang-theo-doi" => Some(Self::Following),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCommand {
    pub command_id: FollowCommandId,
    pub target: PrincipalId,
    pub expected_order: FollowOrder,
    pub intent: FollowIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowState {
    following: bool,
    version: FollowVersion,
    order: FollowOrder,
}

impl FollowState {
    pub const INITIAL: Self = Self {
        following: false,
        version: FollowVersion::INITIAL,
        order: FollowOrder::INITIAL,
    };

    /// Reconstructs durable follow state, rejecting parity or ordering violations.
    pub fn restore(following: bool, version: i64, order: i64) -> Option<Self> {
        let version = FollowVersion::new(version)?;
        let order = FollowOrder::new(order)?;
        (version.get() <= order.get() && following == (version.get() % 2 == 1)).then_some(Self {
            following,
            version,
            order,
        })
    }

    pub const fn following(self) -> bool {
        self.following
    }

    pub const fn version(self) -> FollowVersion {
        self.version
    }

    pub const fn order(self) -> FollowOrder {
        self.order
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredFollow {
    actor: PrincipalId,
    target: PrincipalId,
    state: FollowState,
}

impl StoredFollow {
    pub const fn restore(actor: PrincipalId, target: PrincipalId, state: FollowState) -> Self {
        Self {
            actor,
            target,
            state,
        }
    }

    pub const fn actor(self) -> PrincipalId {
        self.actor
    }

    pub const fn target(self) -> PrincipalId {
        self.target
    }

    pub const fn state(self) -> FollowState {
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowReceipt {
    actor: PrincipalId,
    command: FollowCommand,
    committed: FollowState,
}

impl FollowReceipt {
    pub fn restore(
        actor: PrincipalId,
        command: FollowCommand,
        committed: FollowState,
    ) -> Option<Self> {
        (actor != command.target
            && command.expected_order.next() == Some(committed.order())
            && committed.following() == command.intent.following())
        .then_some(Self {
            actor,
            command,
            committed,
        })
    }

    pub const fn actor(self) -> PrincipalId {
        self.actor
    }

    pub const fn command(self) -> FollowCommand {
        self.command
    }

    pub const fn committed(self) -> FollowState {
        self.committed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowFacts {
    pub actor_eligible: bool,
    pub target_eligible: bool,
    pub current: Option<StoredFollow>,
    pub receipt: Option<FollowReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowDelta {
    None,
    Add,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowWrite {
    next: StoredFollow,
    delta: FollowDelta,
    receipt: FollowReceipt,
}

impl FollowWrite {
    pub const fn next(self) -> StoredFollow {
        self.next
    }

    pub const fn delta(self) -> FollowDelta {
        self.delta
    }

    pub const fn receipt(self) -> FollowReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowRefusal {
    SelfFollow,
    ActorUnavailable,
    TargetUnavailable,
    InvalidFacts,
    KeyReused,
    TokenExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowOutcome {
    Accepted(FollowWrite),
    Replay {
        committed: FollowState,
        observed: FollowState,
    },
    Conflict {
        observed: FollowState,
    },
    Refused(FollowRefusal),
}

/// Pure production decision for Follow/Unfollow desired-state transitions.
pub fn decide_follow(
    actor: PrincipalId,
    command: FollowCommand,
    facts: FollowFacts,
) -> FollowOutcome {
    use FollowOutcome::{Accepted, Conflict, Refused, Replay};
    use FollowRefusal::*;

    if actor == command.target {
        return Refused(SelfFollow);
    }
    if !facts.actor_eligible {
        return Refused(ActorUnavailable);
    }
    if facts
        .current
        .is_some_and(|s| s.actor != actor || s.target != command.target)
    {
        return Refused(InvalidFacts);
    }
    if !facts.target_eligible {
        return Refused(TargetUnavailable);
    }
    let current = facts.current.map_or(FollowState::INITIAL, |s| s.state);
    if let Some(receipt) = facts.receipt {
        if receipt.actor != actor || receipt.command.command_id != command.command_id {
            return Refused(InvalidFacts);
        }
        if receipt.command != command {
            return Refused(KeyReused);
        }
        if receipt.committed.order() > current.order()
            || (receipt.committed.order() == current.order() && receipt.committed != current)
        {
            return Refused(InvalidFacts);
        }
        return Replay {
            committed: receipt.committed,
            observed: current,
        };
    }
    if command.expected_order != current.order() {
        return Conflict { observed: current };
    }
    let Some(order) = current.order().next() else {
        return Refused(TokenExhausted);
    };
    let following = command.intent.following();
    let changed = following != current.following();
    let version = if changed {
        let Some(v) = current.version().next() else {
            return Refused(TokenExhausted);
        };
        v
    } else {
        current.version()
    };
    let next = FollowState {
        following,
        version,
        order,
    };
    let delta = if !changed {
        FollowDelta::None
    } else if following {
        FollowDelta::Add
    } else {
        FollowDelta::Remove
    };
    Accepted(FollowWrite {
        next: StoredFollow::restore(actor, command.target, next),
        delta,
        receipt: FollowReceipt {
            actor,
            command,
            committed: next,
        },
    })
}

pub fn can_read_list(
    policies: ProfilePolicies,
    direction: FollowDirection,
    is_owner: bool,
) -> bool {
    if is_owner {
        return true;
    }
    match direction {
        FollowDirection::Followers => policies.followers_list == Visibility::Everyone,
        FollowDirection::Following => policies.following_list == Visibility::Everyone,
    }
}

pub fn can_read_count(
    policies: ProfilePolicies,
    direction: FollowDirection,
    is_owner: bool,
) -> bool {
    if is_owner {
        return true;
    }
    match direction {
        FollowDirection::Followers => policies.followers_count == Visibility::Everyone,
        FollowDirection::Following => policies.following_count == Visibility::Everyone,
    }
}

/// Mutual status is disclosed only to the two distinct participants in the pair.
pub fn mutual_visible(
    viewer: Option<PrincipalId>,
    target: PrincipalId,
    viewer_follows_target: bool,
    target_follows_viewer: bool,
) -> Option<bool> {
    viewer
        .filter(|&v| v != target)
        .map(|_| viewer_follows_target && target_follows_viewer)
}

/// Canonical ascending PrincipalId order so opposite-direction writes never deadlock.
pub fn principal_lock_pair(a: PrincipalId, b: PrincipalId) -> Option<(PrincipalId, PrincipalId)> {
    if a == b {
        None
    } else if a < b {
        Some((a, b))
    } else {
        Some((b, a))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowPagePosition {
    pub active_since: OffsetDateTime,
    pub cursor_key: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCursorScope([u8; 32]);

impl FollowCursorScope {
    pub fn public(target: PrincipalId, direction: FollowDirection) -> Self {
        Self::of(target, direction, None)
    }

    pub fn own(actor: PrincipalId, direction: FollowDirection, session: [u8; 32]) -> Self {
        Self::of(actor, direction, Some(session))
    }

    fn of(target: PrincipalId, direction: FollowDirection, session: Option<[u8; 32]>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-follow-cursor-v2");
        hasher.update(target.as_uuid().as_bytes());
        hasher.update([match direction {
            FollowDirection::Followers => 0,
            FollowDirection::Following => 1,
        }]);
        match session {
            Some(digest) => {
                hasher.update([1]);
                hasher.update(digest);
            }
            None => hasher.update([0]),
        }
        Self(hasher.finalize().into())
    }

    fn mask_anchor(self, mut anchor: [u8; 16]) -> [u8; 16] {
        // Separate the transmitted scope tag from the mask; own tokens cannot be linked
        // to public tokens by the raw stable edge key.
        for (byte, mask) in anchor.iter_mut().zip(&self.0[16..]) {
            *byte ^= mask;
        }
        anchor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowCursorError {
    Malformed,
}

const CURSOR_VERSION: u8 = 2;
const CURSOR_BYTES: usize = 1 + 16 + 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCursor {
    masked_anchor: [u8; 16],
    scope_tag: [u8; 16],
}

impl FollowCursor {
    pub fn new(anchor: Uuid, scope: FollowCursorScope) -> Self {
        let mut scope_tag = [0u8; 16];
        scope_tag.copy_from_slice(&scope.0[..16]);
        Self {
            masked_anchor: scope.mask_anchor(*anchor.as_bytes()),
            scope_tag,
        }
    }

    pub fn anchor(&self, scope: FollowCursorScope) -> Option<Uuid> {
        (self.scope_tag == scope.0[..16])
            .then(|| Uuid::from_bytes(scope.mask_anchor(self.masked_anchor)))
    }

    pub fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(CURSOR_BYTES);
        bytes.push(CURSOR_VERSION);
        bytes.extend_from_slice(&self.masked_anchor);
        bytes.extend_from_slice(&self.scope_tag);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(raw: &str) -> Result<Self, FollowCursorError> {
        if raw.len() != 2 * CURSOR_BYTES {
            return Err(FollowCursorError::Malformed);
        }
        let mut bytes = [0u8; CURSOR_BYTES];
        for (index, pair) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[index] = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
        }
        if bytes[0] != CURSOR_VERSION {
            return Err(FollowCursorError::Malformed);
        }
        Ok(Self {
            masked_anchor: bytes[1..17]
                .try_into()
                .map_err(|_| FollowCursorError::Malformed)?,
            scope_tag: bytes[17..]
                .try_into()
                .map_err(|_| FollowCursorError::Malformed)?,
        })
    }
}

fn hex_digit(byte: u8) -> Result<u8, FollowCursorError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(FollowCursorError::Malformed),
    }
}

pub fn cut_follow_page<T>(
    mut rows: Vec<T>,
    page_size: usize,
    scope: FollowCursorScope,
    anchor: impl Fn(&T) -> Uuid,
) -> (Vec<T>, Option<FollowCursor>) {
    if rows.len() <= page_size {
        return (rows, None);
    }
    rows.truncate(page_size);
    let next = rows
        .last()
        .map(|last| FollowCursor::new(anchor(last), scope));
    (rows, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(n))
    }

    fn key(n: u128) -> FollowCommandId {
        FollowCommandId::from_uuid(Uuid::from_u128(0x1000 + n))
    }

    fn cmd(
        command_id: u128,
        target: PrincipalId,
        expected_order: i64,
        intent: FollowIntent,
    ) -> FollowCommand {
        FollowCommand {
            command_id: key(command_id),
            target,
            expected_order: FollowOrder::new(expected_order).unwrap(),
            intent,
        }
    }

    #[test]
    fn self_follow_and_ineligibility_are_refused() {
        let a = p(1);
        let b = p(2);
        let c = cmd(1, a, 0, FollowIntent::Follow);
        assert_eq!(
            decide_follow(
                a,
                c,
                FollowFacts {
                    actor_eligible: true,
                    target_eligible: true,
                    current: None,
                    receipt: None,
                },
            ),
            FollowOutcome::Refused(FollowRefusal::SelfFollow)
        );
        let c_b = cmd(2, b, 0, FollowIntent::Follow);
        assert_eq!(
            decide_follow(
                a,
                c_b,
                FollowFacts {
                    actor_eligible: false,
                    target_eligible: true,
                    current: None,
                    receipt: None,
                },
            ),
            FollowOutcome::Refused(FollowRefusal::ActorUnavailable)
        );
        assert_eq!(
            decide_follow(
                a,
                c_b,
                FollowFacts {
                    actor_eligible: true,
                    target_eligible: false,
                    current: None,
                    receipt: None,
                },
            ),
            FollowOutcome::Refused(FollowRefusal::TargetUnavailable)
        );
    }

    #[test]
    fn aba_lost_ack_unfollow_and_retry_cannot_resurrect_edge() {
        let a = p(1);
        let b = p(2);

        // Initial Follow K1 at expected_order=0 -> (following=true, version=1, order=1)
        let k1 = cmd(1, b, 0, FollowIntent::Follow);
        let FollowOutcome::Accepted(w1) = decide_follow(
            a,
            k1,
            FollowFacts {
                actor_eligible: true,
                target_eligible: true,
                current: None,
                receipt: None,
            },
        ) else {
            panic!("expected k1 accepted");
        };
        assert_eq!(w1.delta(), FollowDelta::Add);
        assert!(w1.next().state().following());
        assert_eq!(w1.next().state().version().get(), 1);
        assert_eq!(w1.next().state().order().get(), 1);

        // Actor reconciles and sends Unfollow K2 at expected_order=1 -> (false, v2, order 2)
        let k2 = cmd(2, b, 1, FollowIntent::Unfollow);
        let FollowOutcome::Accepted(w2) = decide_follow(
            a,
            k2,
            FollowFacts {
                actor_eligible: true,
                target_eligible: true,
                current: Some(w1.next()),
                receipt: None,
            },
        ) else {
            panic!("expected k2 accepted");
        };
        assert_eq!(w2.delta(), FollowDelta::Remove);
        assert!(!w2.next().state().following());
        assert_eq!(w2.next().state().version().get(), 2);
        assert_eq!(w2.next().state().order().get(), 2);

        // Retrying K1 returns Replay with historical committed v1/order1 and current observed false/v2/order2
        let outcome = decide_follow(
            a,
            k1,
            FollowFacts {
                actor_eligible: true,
                target_eligible: true,
                current: Some(w2.next()),
                receipt: Some(w1.receipt()),
            },
        );
        assert_eq!(
            outcome,
            FollowOutcome::Replay {
                committed: w1.next().state(),
                observed: w2.next().state(),
            }
        );

        // Reusing K1 with different payload or expected_order is refused
        let reused_intent = cmd(1, b, 0, FollowIntent::Unfollow);
        assert_eq!(
            decide_follow(
                a,
                reused_intent,
                FollowFacts {
                    actor_eligible: true,
                    target_eligible: true,
                    current: Some(w2.next()),
                    receipt: Some(w1.receipt()),
                },
            ),
            FollowOutcome::Refused(FollowRefusal::KeyReused)
        );
        let reused_target = cmd(1, p(3), 0, FollowIntent::Follow);
        assert_eq!(
            decide_follow(
                a,
                reused_target,
                FollowFacts {
                    actor_eligible: true,
                    target_eligible: true,
                    current: None,
                    receipt: Some(w1.receipt()),
                },
            ),
            FollowOutcome::Refused(FollowRefusal::KeyReused)
        );

        // Stale new command at order 1 conflicts with observed order 2
        let stale = cmd(3, b, 1, FollowIntent::Follow);
        assert_eq!(
            decide_follow(
                a,
                stale,
                FollowFacts {
                    actor_eligible: true,
                    target_eligible: true,
                    current: Some(w2.next()),
                    receipt: None,
                },
            ),
            FollowOutcome::Conflict {
                observed: w2.next().state(),
            }
        );
    }

    #[test]
    fn noop_command_advances_order_without_flipping_version_or_count() {
        let a = p(1);
        let b = p(2);
        let k1 = cmd(1, b, 0, FollowIntent::Unfollow);
        let FollowOutcome::Accepted(w1) = decide_follow(
            a,
            k1,
            FollowFacts {
                actor_eligible: true,
                target_eligible: true,
                current: None,
                receipt: None,
            },
        ) else {
            panic!("expected noop unfollow accepted");
        };
        assert_eq!(w1.delta(), FollowDelta::None);
        assert!(!w1.next().state().following());
        assert_eq!(w1.next().state().version().get(), 0);
        assert_eq!(w1.next().state().order().get(), 1);
    }

    #[test]
    fn state_and_receipt_restore_enforce_parity_and_bounds() {
        assert_eq!(
            FollowState::restore(false, 0, 0),
            Some(FollowState::INITIAL)
        );
        assert!(FollowState::restore(true, 1, 1).is_some());
        assert!(FollowState::restore(false, 2, 2).is_some());
        assert!(FollowState::restore(false, 0, 1).is_some());
        assert!(FollowState::restore(true, 0, 1).is_none());
        assert!(FollowState::restore(false, 1, 1).is_none());
        assert!(FollowState::restore(true, 2, 1).is_none());
        assert!(FollowState::restore(false, -1, 1).is_none());
        assert!(FollowState::restore(false, 0, -1).is_none());
    }

    #[test]
    fn policy_matrix_mutual_and_lock_order_are_exact() {
        let a = p(10);
        let b = p(20);
        assert_eq!(principal_lock_pair(a, a), None);
        assert_eq!(principal_lock_pair(a, b), Some((a, b)));
        assert_eq!(principal_lock_pair(b, a), Some((a, b)));

        assert_eq!(mutual_visible(None, a, true, true), None);
        assert_eq!(mutual_visible(Some(a), a, true, true), None);
        assert_eq!(mutual_visible(Some(b), a, true, true), Some(true));
        assert_eq!(mutual_visible(Some(b), a, true, false), Some(false));

        let custom = ProfilePolicies {
            followers_list: Visibility::Owner,
            followers_count: Visibility::Everyone,
            following_list: Visibility::Everyone,
            following_count: Visibility::Owner,
            ..ProfilePolicies::default()
        };
        for is_owner in [false, true] {
            assert_eq!(
                can_read_list(custom, FollowDirection::Followers, is_owner),
                is_owner
            );
            assert!(can_read_count(custom, FollowDirection::Followers, is_owner));
            assert!(can_read_list(custom, FollowDirection::Following, is_owner));
            assert_eq!(
                can_read_count(custom, FollowDirection::Following, is_owner),
                is_owner
            );
        }
    }

    #[test]
    fn follow_cursor_round_trips_and_binds_target_direction_and_session() {
        let target = p(1);
        let direction = FollowDirection::Followers;
        let public = FollowCursorScope::public(target, direction);
        let own = FollowCursorScope::own(target, direction, [1; 32]);
        assert_ne!(public, own);
        assert_ne!(own, FollowCursorScope::own(target, direction, [2; 32]));
        assert_ne!(own, FollowCursorScope::own(p(3), direction, [1; 32]));
        assert_ne!(public, FollowCursorScope::public(p(3), direction));
        assert_ne!(
            public,
            FollowCursorScope::public(target, FollowDirection::Following)
        );

        let anchor = Uuid::from_u128(0xdeadbeef);
        let encoded = FollowCursor::new(anchor, own).encode();
        assert_eq!(encoded.len(), 66);
        let decoded = FollowCursor::decode(&encoded).unwrap();
        assert_eq!(decoded.anchor(own), Some(anchor));
        assert_eq!(decoded.anchor(public), None);
        assert!(!encoded.contains(&anchor.simple().to_string()));
        assert!(FollowCursor::decode("zz").is_err());
        assert!(FollowCursor::decode(&encoded.to_uppercase()).is_err());
    }
}
