//! The event envelope received from the App relay (contract `services/vink-app/contracts/http/v1/
//! notifications.md` §1.1), parsed strictly and checked against its digest. Pure: no clock, SQL
//! or I/O. The service keeps its own representation; it never imports the App crate.
//!
//! **Digest.** The receiver recomputes SHA-256 over the canonical text of the received object
//! without `payload_digest`: keys in ASCII order at every level, arrays in their given order,
//! compact UTF-8, strings escaped as `serde_json` writes them. A digest that does not match the
//! text is `rejected_invalid`; a valid digest different from the receipt of the same event ID is
//! `payload_mismatch` (decided by the store).
//!
//! **Strictness.** Duplicate keys, unknown fields, a wrong type, non-canonical UUID text,
//! unsorted or duplicate recipients and a field that the event type does not allow are all
//! refused. `admin.notice.reserved` (P-05, `TYPE_ONLY_NOW`) has no producer in v1 and is refused.

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

/// The only envelope schema this binary accepts.
pub const SCHEMA_VERSION: i64 = 1;
/// More recipients than any v1 producer writes (P-01B yields at most two).
pub const MAX_RECIPIENTS: usize = 16;
/// Upper bound of one envelope's text, far above any v1 envelope (~1 KiB).
pub const MAX_ENVELOPE_BYTES: usize = 8 * 1024;

/// Closed set of `event_type` codes (contract §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    CommentRootApproved,
    CommentReplyApproved,
    WorkModerationDecided,
    CommentModerationAdverse,
    WorkThamChanged,
    AdminNoticeReserved,
    Invalidation,
}

impl EventType {
    pub const ALL: [Self; 7] = [
        Self::CommentRootApproved,
        Self::CommentReplyApproved,
        Self::WorkModerationDecided,
        Self::CommentModerationAdverse,
        Self::WorkThamChanged,
        Self::AdminNoticeReserved,
        Self::Invalidation,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::CommentRootApproved => "comment.root.approved",
            Self::CommentReplyApproved => "comment.reply.approved",
            Self::WorkModerationDecided => "work.moderation.decided",
            Self::CommentModerationAdverse => "comment.moderation.adverse",
            Self::WorkThamChanged => "work.endorsement.tham.changed",
            Self::AdminNoticeReserved => "admin.notice.reserved",
            Self::Invalidation => "notification.invalidation",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }

    /// Whether v1 accepts the type at all. The reserved admin type is type-only (P-05).
    pub const fn accepted(self) -> bool {
        !matches!(self, Self::AdminNoticeReserved)
    }

    /// Whether the type can become an inbox row (invalidation and the reserved type cannot).
    pub const fn grouped(self) -> bool {
        !matches!(self, Self::AdminNoticeReserved | Self::Invalidation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticKind {
    Approved,
    Rejected,
    Removed,
    Add,
    Remove,
    Invalidate,
}

impl SemanticKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Removed => "removed",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Invalidate => "invalidate",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        [
            Self::Approved,
            Self::Rejected,
            Self::Removed,
            Self::Add,
            Self::Remove,
            Self::Invalidate,
        ]
        .into_iter()
        .find(|kind| kind.code() == code)
    }
}

/// What a reply answered, as the source still knew it (inventory §4.3 B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyContext {
    /// A direct reply to the root.
    Root,
    /// A reply to another reply whose row still existed.
    Reply(Uuid),
    /// A reply to a reply whose row was purged: grouped by the approved reply itself.
    Purged,
}

/// The typed target (contract §2.4; invalidation kinds from §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    WorkCommentThread {
        work: Uuid,
        comment: Uuid,
        moderation_version: i64,
    },
    WorkCommentReply {
        work: Uuid,
        root: Uuid,
        context: ReplyContext,
        comment: Uuid,
        moderation_version: i64,
    },
    WorkEndorsement {
        work: Uuid,
        revision: Uuid,
    },
    WorkModerationDecision {
        work: Uuid,
        revision: Uuid,
        decision_version: i64,
    },
    CommentModerationDecision {
        work: Uuid,
        comment: Uuid,
        moderation_version: i64,
    },
    InvalidWork {
        work: Uuid,
    },
    InvalidComment {
        work: Uuid,
        comment: Uuid,
    },
    InvalidPrincipal {
        principal: Uuid,
    },
}

/// The publication stamp pinned at the source commit (COM-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub epoch: i64,
    pub readable: bool,
}

/// A received envelope that passed every structural and per-type check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub event_id: Uuid,
    pub event_type: EventType,
    pub activation_epoch: i64,
    pub source_order: i64,
    pub stamp: Option<Stamp>,
    pub occurred_at: OffsetDateTime,
    pub actor: Option<Uuid>,
    pub recipients: Vec<Uuid>,
    pub target: Target,
    /// The `target` object exactly as received, kept for the Gateway to open the destination.
    pub target_json: Value,
    pub semantic: SemanticKind,
    pub first_selection: Option<bool>,
    pub action_required: bool,
    /// The recomputed digest (equal to the one sent).
    pub payload_digest: String,
}

/// Why an envelope was refused (`rejected_invalid`). Codes only; never echoes input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    TooLarge,
    Malformed,
    DigestMismatch,
    EventIdMismatch,
    UnknownType,
    ReservedType,
    Schema,
    NonCanonical,
    Shape,
}

impl Invalid {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::Malformed => "malformed",
            Self::DigestMismatch => "digest_mismatch",
            Self::EventIdMismatch => "event_id_mismatch",
            Self::UnknownType => "unknown_type",
            Self::ReservedType => "reserved_type",
            Self::Schema => "schema_version",
            Self::NonCanonical => "non_canonical",
            Self::Shape => "shape",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireStamp {
    epoch: i64,
    readable: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    event_id: String,
    event_type: String,
    schema_version: i64,
    source_owner: String,
    activation_epoch: i64,
    source_order: i64,
    #[serde(default)]
    publication_stamp: Option<WireStamp>,
    occurred_at: String,
    actor_id: Option<String>,
    recipients: Vec<String>,
    target: WireTarget,
    semantic_kind: String,
    #[serde(default)]
    first_selection: Option<bool>,
    action_required: bool,
    payload_digest: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum WireTarget {
    #[serde(rename = "work_comment_thread")]
    WorkCommentThread {
        work_id: String,
        comment_id: String,
        moderation_version: i64,
    },
    #[serde(rename = "work_comment_reply")]
    WorkCommentReply {
        work_id: String,
        root_id: String,
        reply_context_kind: String,
        reply_target_id: Option<String>,
        comment_id: String,
        moderation_version: i64,
    },
    #[serde(rename = "work_endorsement")]
    WorkEndorsement {
        work_id: String,
        revision_id: String,
    },
    #[serde(rename = "work_moderation_decision")]
    WorkModerationDecision {
        work_id: String,
        revision_id: String,
        decision_version: i64,
    },
    #[serde(rename = "comment_moderation_decision")]
    CommentModerationDecision {
        work_id: String,
        comment_id: String,
        moderation_version: i64,
    },
    #[serde(rename = "work")]
    Work { work_id: String },
    #[serde(rename = "comment")]
    Comment { work_id: String, comment_id: String },
    #[serde(rename = "principal")]
    Principal { principal_id: String },
}

/// Canonical UUID text only: lowercase, hyphenated, as the App writes it.
fn uuid(raw: &str) -> Result<Uuid, Invalid> {
    let id = Uuid::try_parse(raw).map_err(|_| Invalid::NonCanonical)?;
    (id.hyphenated().to_string() == raw)
        .then_some(id)
        .ok_or(Invalid::NonCanonical)
}

fn version(raw: i64) -> Result<i64, Invalid> {
    (raw >= 0).then_some(raw).ok_or(Invalid::Shape)
}

/// Canonical JSON of the contract: object keys in ASCII order at every level, arrays in their
/// given order, compact. Strings are escaped as `serde_json` does (no escaping of non-ASCII).
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest.as_slice() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Parses one wire envelope sent for `expected_id`. The typed pass refuses duplicate keys,
/// unknown fields and wrong types; the digest is then recomputed over the same object.
pub fn parse(text: &str, expected_id: Uuid) -> Result<Event, Invalid> {
    if text.len() > MAX_ENVELOPE_BYTES {
        return Err(Invalid::TooLarge);
    }
    let wire: Wire = serde_json::from_str(text).map_err(|_| Invalid::Malformed)?;
    let Ok(Value::Object(mut object)) = serde_json::from_str::<Value>(text) else {
        return Err(Invalid::Malformed);
    };
    object.remove("payload_digest");
    let target_json = object.get("target").cloned().ok_or(Invalid::Malformed)?;
    let digest = sha256_hex(canonical(&Value::Object(object)).as_bytes());
    if wire.payload_digest != digest {
        return Err(Invalid::DigestMismatch);
    }
    let event_id = uuid(&wire.event_id)?;
    if event_id != expected_id {
        return Err(Invalid::EventIdMismatch);
    }
    let event_type = EventType::from_code(&wire.event_type).ok_or(Invalid::UnknownType)?;
    if !event_type.accepted() {
        return Err(Invalid::ReservedType);
    }
    if wire.schema_version != SCHEMA_VERSION {
        return Err(Invalid::Schema);
    }
    if wire.activation_epoch < 1 {
        return Err(Invalid::Shape);
    }
    let source_order = version(wire.source_order)?;
    let occurred_at =
        OffsetDateTime::parse(&wire.occurred_at, &Rfc3339).map_err(|_| Invalid::Shape)?;
    let actor = wire.actor_id.as_deref().map(uuid).transpose()?;
    if wire.recipients.len() > MAX_RECIPIENTS {
        return Err(Invalid::Shape);
    }
    let recipients = wire
        .recipients
        .iter()
        .map(|raw| uuid(raw))
        .collect::<Result<Vec<_>, _>>()?;
    if recipients.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Invalid::NonCanonical);
    }
    let semantic = SemanticKind::from_code(&wire.semantic_kind).ok_or(Invalid::Shape)?;
    let target = target(wire.target)?;
    let stamp = wire.publication_stamp.map(|stamp| Stamp {
        epoch: stamp.epoch,
        readable: stamp.readable,
    });
    let event = Event {
        event_id,
        event_type,
        activation_epoch: wire.activation_epoch,
        source_order,
        stamp,
        occurred_at,
        actor,
        recipients,
        target,
        target_json,
        semantic,
        first_selection: wire.first_selection,
        action_required: wire.action_required,
        payload_digest: digest,
    };
    check_type(&event, &wire.source_owner)?;
    Ok(event)
}

fn target(wire: WireTarget) -> Result<Target, Invalid> {
    Ok(match wire {
        WireTarget::WorkCommentThread {
            work_id,
            comment_id,
            moderation_version,
        } => Target::WorkCommentThread {
            work: uuid(&work_id)?,
            comment: uuid(&comment_id)?,
            moderation_version: version(moderation_version)?,
        },
        WireTarget::WorkCommentReply {
            work_id,
            root_id,
            reply_context_kind,
            reply_target_id,
            comment_id,
            moderation_version,
        } => {
            let root = uuid(&root_id)?;
            let context = match (reply_context_kind.as_str(), reply_target_id.as_deref()) {
                ("root", Some(raw)) if uuid(raw)? == root => ReplyContext::Root,
                ("reply", Some(raw)) => ReplyContext::Reply(uuid(raw)?),
                ("reply", None) => ReplyContext::Purged,
                _ => return Err(Invalid::Shape),
            };
            Target::WorkCommentReply {
                work: uuid(&work_id)?,
                root,
                context,
                comment: uuid(&comment_id)?,
                moderation_version: version(moderation_version)?,
            }
        }
        WireTarget::WorkEndorsement {
            work_id,
            revision_id,
        } => Target::WorkEndorsement {
            work: uuid(&work_id)?,
            revision: uuid(&revision_id)?,
        },
        WireTarget::WorkModerationDecision {
            work_id,
            revision_id,
            decision_version,
        } => Target::WorkModerationDecision {
            work: uuid(&work_id)?,
            revision: uuid(&revision_id)?,
            decision_version: version(decision_version)?,
        },
        WireTarget::CommentModerationDecision {
            work_id,
            comment_id,
            moderation_version,
        } => Target::CommentModerationDecision {
            work: uuid(&work_id)?,
            comment: uuid(&comment_id)?,
            moderation_version: version(moderation_version)?,
        },
        WireTarget::Work { work_id } => Target::InvalidWork {
            work: uuid(&work_id)?,
        },
        WireTarget::Comment {
            work_id,
            comment_id,
        } => Target::InvalidComment {
            work: uuid(&work_id)?,
            comment: uuid(&comment_id)?,
        },
        WireTarget::Principal { principal_id } => Target::InvalidPrincipal {
            principal: uuid(&principal_id)?,
        },
    })
}

/// Per-type rules of the inventory (§4.1) and contract (§1.1): which owner, target, semantic,
/// actor, recipients, stamp and `first_selection` each type carries.
fn check_type(event: &Event, owner: &str) -> Result<(), Invalid> {
    use EventType as T;
    use SemanticKind as S;
    let shape = |ok: bool| ok.then_some(()).ok_or(Invalid::Shape);
    let has_recipients = !event.recipients.is_empty();
    let not_self = event
        .actor
        .is_none_or(|actor| !event.recipients.contains(&actor));
    match event.event_type {
        T::CommentRootApproved => shape(
            owner == "discussion"
                && matches!(event.target, Target::WorkCommentThread { moderation_version, .. }
                    if moderation_version == event.source_order)
                && event.semantic == S::Approved
                && event.actor.is_some()
                && has_recipients
                && not_self
                && event.stamp.is_some()
                && event.first_selection.is_none()
                && !event.action_required,
        ),
        T::CommentReplyApproved => shape(
            owner == "discussion"
                && matches!(event.target, Target::WorkCommentReply { moderation_version, .. }
                    if moderation_version == event.source_order)
                && event.semantic == S::Approved
                && event.actor.is_some()
                && has_recipients
                && not_self
                && event.stamp.is_some()
                && event.first_selection.is_none()
                && !event.action_required,
        ),
        T::WorkModerationDecided => shape(
            owner == "publication_moderation"
                && matches!(event.target, Target::WorkModerationDecision { decision_version, .. }
                    if decision_version == event.source_order)
                && matches!(event.semantic, S::Approved | S::Rejected)
                && event.actor.is_none()
                && has_recipients
                && event.stamp.is_none()
                && event.first_selection.is_none(),
        ),
        T::CommentModerationAdverse => shape(
            owner == "discussion"
                && matches!(event.target, Target::CommentModerationDecision { moderation_version, .. }
                    if moderation_version == event.source_order)
                && matches!(event.semantic, S::Rejected | S::Removed)
                && event.actor.is_none()
                && has_recipients
                && event.stamp.is_none()
                && event.first_selection.is_none()
                && !event.action_required,
        ),
        T::WorkThamChanged => shape(
            owner == "endorsement"
                && matches!(event.target, Target::WorkEndorsement { .. })
                && matches!(event.semantic, S::Add | S::Remove)
                && event.actor.is_some()
                && has_recipients
                && not_self
                && event.stamp.is_some()
                && (event.semantic == S::Add) == event.first_selection.is_some()
                && !event.action_required,
        ),
        T::Invalidation => shape(
            matches!(
                (owner, event.target),
                ("discussion", Target::InvalidComment { .. })
                    | ("publication_moderation", Target::InvalidWork { .. })
                    | ("principal", Target::InvalidPrincipal { .. })
            ) && event.semantic == S::Invalidate
                && event.actor.is_none()
                && !has_recipients
                && event.stamp.is_none()
                && event.first_selection.is_none()
                && !event.action_required,
        ),
        T::AdminNoticeReserved => Err(Invalid::ReservedType),
    }
}

#[cfg(test)]
mod tests;
