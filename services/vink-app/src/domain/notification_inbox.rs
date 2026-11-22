//! What the Gateway may show of a notification group at read time (#348; N4, K4, COM-11, P-12A,
//! #365). Pure: the HTTP layer reads candidate groups from the Notification Service (Thin
//! Pointer: identifiers and versions only) and one App snapshot of facts, and this module decides,
//! group by group, whether the viewer may see it now and with which target, actors and excerpt.
//!
//! The service already drops a comment's contributions when its invalidation arrives. These rules
//! are what holds before that delivery and for the changes the service never projects: a work that
//! is no longer readable or was republished in a new epoch, a comment held or removed while its
//! invalidation is in flight, ownership of private decision targets.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use super::ids::{CommentId, PrincipalId, RevisionId, WorkId};

/// Longest excerpt, in extended grapheme clusters (#365). Clients clamp lines further.
pub const EXCERPT_MAX_GRAPHEMES: usize = 140;
/// Names in a row's preview (N5).
pub const PREVIEW_ACTORS: usize = 3;

/// The inbox types a group can have (`admin.notice.reserved` never reaches an inbox).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupKind {
    RootApproved,
    ReplyApproved,
    WorkDecision,
    CommentAdverse,
    Tham,
}

impl GroupKind {
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "comment.root.approved" => Self::RootApproved,
            "comment.reply.approved" => Self::ReplyApproved,
            "work.moderation.decided" => Self::WorkDecision,
            "comment.moderation.adverse" => Self::CommentAdverse,
            "work.endorsement.tham.changed" => Self::Tham,
            _ => return None,
        })
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::RootApproved => "comment.root.approved",
            Self::ReplyApproved => "comment.reply.approved",
            Self::WorkDecision => "work.moderation.decided",
            Self::CommentAdverse => "comment.moderation.adverse",
            Self::Tham => "work.endorsement.tham.changed",
        }
    }
}

/// Where a reply answered, as the envelope recorded it (§2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyContext {
    Root,
    Reply(CommentId),
    /// The answered reply was purged and its ID is gone.
    Purged,
}

/// The typed target of an inbox row (contract §2.4), without any title or body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxTarget {
    CommentThread {
        work: WorkId,
        comment: CommentId,
        moderation_version: i64,
    },
    CommentReply {
        work: WorkId,
        root: CommentId,
        context: ReplyContext,
        comment: CommentId,
        moderation_version: i64,
    },
    Endorsement {
        work: WorkId,
        revision: RevisionId,
    },
    WorkDecision {
        work: WorkId,
        revision: RevisionId,
        decision_version: i64,
    },
    CommentDecision {
        work: WorkId,
        comment: CommentId,
        moderation_version: i64,
    },
}

fn uuid_field(value: &Value, key: &str) -> Option<Uuid> {
    let raw = value.get(key)?.as_str()?;
    let id = Uuid::try_parse(raw).ok()?;
    (id.hyphenated().to_string() == raw).then_some(id)
}

fn version_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key)?.as_i64().filter(|v| *v >= 0)
}

impl InboxTarget {
    /// Parses the service's copy of the newest envelope target; `None` for anything unexpected.
    pub fn parse(value: &Value) -> Option<Self> {
        let work = || uuid_field(value, "work_id").map(WorkId::from_uuid);
        let comment = || uuid_field(value, "comment_id").map(CommentId::from_uuid);
        Some(match value.get("kind")?.as_str()? {
            "work_comment_thread" => Self::CommentThread {
                work: work()?,
                comment: comment()?,
                moderation_version: version_field(value, "moderation_version")?,
            },
            "work_comment_reply" => {
                let root = CommentId::from_uuid(uuid_field(value, "root_id")?);
                let context = match (
                    value.get("reply_context_kind")?.as_str()?,
                    value.get("reply_target_id")?,
                ) {
                    ("root", _) => ReplyContext::Root,
                    ("reply", Value::Null) => ReplyContext::Purged,
                    ("reply", _) => ReplyContext::Reply(CommentId::from_uuid(uuid_field(
                        value,
                        "reply_target_id",
                    )?)),
                    _ => return None,
                };
                Self::CommentReply {
                    work: work()?,
                    root,
                    context,
                    comment: comment()?,
                    moderation_version: version_field(value, "moderation_version")?,
                }
            }
            "work_endorsement" => Self::Endorsement {
                work: work()?,
                revision: RevisionId::from_uuid(uuid_field(value, "revision_id")?),
            },
            "work_moderation_decision" => Self::WorkDecision {
                work: work()?,
                revision: RevisionId::from_uuid(uuid_field(value, "revision_id")?),
                decision_version: version_field(value, "decision_version")?,
            },
            "comment_moderation_decision" => Self::CommentDecision {
                work: work()?,
                comment: comment()?,
                moderation_version: version_field(value, "moderation_version")?,
            },
            _ => return None,
        })
    }

    pub const fn work(self) -> WorkId {
        match self {
            Self::CommentThread { work, .. }
            | Self::CommentReply { work, .. }
            | Self::Endorsement { work, .. }
            | Self::WorkDecision { work, .. }
            | Self::CommentDecision { work, .. } => work,
        }
    }

    const fn fits(self, kind: GroupKind) -> bool {
        matches!(
            (kind, self),
            (GroupKind::RootApproved, Self::CommentThread { .. })
                | (GroupKind::ReplyApproved, Self::CommentReply { .. })
                | (GroupKind::Tham, Self::Endorsement { .. })
                | (GroupKind::WorkDecision, Self::WorkDecision { .. })
                | (GroupKind::CommentAdverse, Self::CommentDecision { .. })
        )
    }

    /// The client-facing object (§2.4), before the Gateway adds `work_title`.
    pub fn json(self) -> Value {
        let id = |raw: Uuid| Value::String(raw.hyphenated().to_string());
        match self {
            Self::CommentThread {
                work,
                comment,
                moderation_version,
            } => json!({
                "kind": "work_comment_thread",
                "work_id": id(work.as_uuid()),
                "comment_id": id(comment.as_uuid()),
                "moderation_version": moderation_version,
            }),
            Self::CommentReply {
                work,
                root,
                context,
                comment,
                moderation_version,
            } => {
                let (kind, target) = match context {
                    ReplyContext::Root => ("root", id(root.as_uuid())),
                    ReplyContext::Reply(reply) => ("reply", id(reply.as_uuid())),
                    ReplyContext::Purged => ("reply", Value::Null),
                };
                json!({
                    "kind": "work_comment_reply",
                    "work_id": id(work.as_uuid()),
                    "root_id": id(root.as_uuid()),
                    "reply_context_kind": kind,
                    "reply_target_id": target,
                    "comment_id": id(comment.as_uuid()),
                    "moderation_version": moderation_version,
                })
            }
            Self::Endorsement { work, revision } => json!({
                "kind": "work_endorsement",
                "work_id": id(work.as_uuid()),
                "revision_id": id(revision.as_uuid()),
            }),
            Self::WorkDecision {
                work,
                revision,
                decision_version,
            } => json!({
                "kind": "work_moderation_decision",
                "work_id": id(work.as_uuid()),
                "revision_id": id(revision.as_uuid()),
                "decision_version": decision_version,
            }),
            Self::CommentDecision {
                work,
                comment,
                moderation_version,
            } => json!({
                "kind": "comment_moderation_decision",
                "work_id": id(work.as_uuid()),
                "comment_id": id(comment.as_uuid()),
                "moderation_version": moderation_version,
            }),
        }
    }
}

/// One group as read from the service, already typed. Built by the HTTP layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: GroupKind,
    pub target: InboxTarget,
    pub unseen: bool,
    pub actor_count: i32,
    pub item_count: i32,
    pub actors_preview: Vec<PrincipalId>,
    /// Comment groups: active contributions, newest first, and how many are active in total.
    pub contributions: Vec<(CommentId, PrincipalId)>,
    pub contribution_total: i64,
    pub publication_epoch: Option<i64>,
    pub action_required: bool,
}

impl Candidate {
    /// `None` when the target does not belong to the type (a contract violation: never shown).
    /// One argument per wire field group, so a caller cannot build a mistyped candidate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: GroupKind,
        target: InboxTarget,
        unseen: bool,
        counts: (i32, i32),
        actors_preview: Vec<PrincipalId>,
        contributions: (Vec<(CommentId, PrincipalId)>, i64),
        publication_epoch: Option<i64>,
        action_required: bool,
    ) -> Option<Self> {
        target.fits(kind).then_some(Self {
            kind,
            target,
            unseen,
            actor_count: counts.0,
            item_count: counts.1,
            actors_preview,
            contributions: contributions.0,
            contribution_total: contributions.1,
            publication_epoch,
            action_required,
        })
    }

    /// Comments whose current visibility the Gateway must read.
    pub fn comments(&self) -> impl Iterator<Item = CommentId> + '_ {
        let decision = match self.target {
            InboxTarget::CommentDecision { comment, .. } => Some(comment),
            _ => None,
        };
        self.contributions
            .iter()
            .map(|(comment, _)| *comment)
            .chain(decision)
    }
}

/// The readable public state of a work right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readable {
    pub revision: RevisionId,
    pub epoch: i64,
    /// Title of the readable revision.
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkFacts {
    pub owner: PrincipalId,
    pub readable: Option<Readable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentFacts {
    pub work: WorkId,
    pub author: PrincipalId,
    /// Present, approved and shown on the public discussion of a readable work right now.
    pub public: bool,
    pub moderation_version: i64,
    /// The body, only when `public` (never read for a hidden, held or removed comment).
    pub body: Option<String>,
}

/// One App snapshot of everything the candidates of a page need (batch-read: no N+1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadFacts {
    pub works: HashMap<WorkId, WorkFacts>,
    pub comments: HashMap<CommentId, CommentFacts>,
    /// Owner-visible titles of the revisions named by work decisions.
    pub revision_titles: HashMap<RevisionId, String>,
    /// Adverse comment decisions named by inbox targets: `true` when the comment was removed,
    /// `false` when rejected. Read only for the comment's writer.
    pub comment_decisions: HashMap<(CommentId, i64), bool>,
}

/// A short excerpt of a comment (#365): NFC, whitespace collapsed, cut on a grapheme boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    pub text: String,
    pub truncated: bool,
}

impl Excerpt {
    /// `None` for a body with nothing to show.
    pub fn of(body: &str) -> Option<Self> {
        let normalized: String = body.nfc().collect();
        let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            return None;
        }
        let mut graphemes = collapsed.graphemes(true);
        let text: String = graphemes
            .by_ref()
            .take(EXCERPT_MAX_GRAPHEMES)
            .collect::<String>();
        let truncated = graphemes.next().is_some();
        let text = if truncated {
            text.trim_end().to_owned()
        } else {
            text
        };
        Some(Self { text, truncated })
    }
}

/// What the viewer may see of one group now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub target: InboxTarget,
    pub work_title: Option<String>,
    pub actors_preview: Vec<PrincipalId>,
    pub actor_count: i32,
    pub item_count: i32,
    pub excerpt: Option<Excerpt>,
    /// Decision rows only: `approved`/`rejected` (work) or `rejected`/`removed` (comment).
    pub outcome: Option<&'static str>,
}

/// The rules of N4/COM-11/P-12A for one group; `None` hides it.
pub fn decide(viewer: PrincipalId, candidate: &Candidate, facts: &ReadFacts) -> Option<Shown> {
    let work = facts.works.get(&candidate.target.work())?;
    match candidate.kind {
        GroupKind::RootApproved | GroupKind::ReplyApproved => {
            interaction(viewer, candidate, work, facts)
        }
        GroupKind::Tham => {
            let readable = current_epoch(candidate, work)?;
            (work.owner == viewer).then(|| Shown {
                target: candidate.target,
                work_title: Some(readable.title.clone()),
                actors_preview: candidate.actors_preview.clone(),
                actor_count: candidate.actor_count,
                item_count: candidate.item_count,
                excerpt: None,
                outcome: None,
            })
        }
        GroupKind::WorkDecision => {
            let InboxTarget::WorkDecision { revision, .. } = candidate.target else {
                return None;
            };
            (work.owner == viewer).then(|| Shown {
                target: candidate.target,
                work_title: facts.revision_titles.get(&revision).cloned(),
                actors_preview: Vec::new(),
                actor_count: 0,
                item_count: candidate.item_count,
                excerpt: None,
                // The producer sets `action_required` exactly for rejections (P-11a).
                outcome: Some(if candidate_action_required(candidate) {
                    "rejected"
                } else {
                    "approved"
                }),
            })
        }
        GroupKind::CommentAdverse => {
            let InboxTarget::CommentDecision {
                comment,
                moderation_version,
                ..
            } = candidate.target
            else {
                return None;
            };
            let removed = *facts
                .comment_decisions
                .get(&(comment, moderation_version))?;
            let facts = facts.comments.get(&comment)?;
            (facts.author == viewer && facts.work == candidate.target.work()).then(|| Shown {
                target: candidate.target,
                // The writer is not the work's owner: a title only while the work is public.
                work_title: work.readable.as_ref().map(|r| r.title.clone()),
                actors_preview: Vec::new(),
                actor_count: 0,
                item_count: candidate.item_count,
                excerpt: None,
                outcome: Some(if removed { "removed" } else { "rejected" }),
            })
        }
    }
}

fn candidate_action_required(candidate: &Candidate) -> bool {
    candidate.action_required
}

/// COM-11: an interaction is shown only while the work is readable in the epoch it was stamped
/// with; after a republication, earlier signals stay hidden (`should_notify_author`).
fn current_epoch<'a>(candidate: &Candidate, work: &'a WorkFacts) -> Option<&'a Readable> {
    let readable = work.readable.as_ref()?;
    (candidate.publication_epoch == Some(readable.epoch)).then_some(readable)
}

fn interaction(
    viewer: PrincipalId,
    candidate: &Candidate,
    work: &WorkFacts,
    facts: &ReadFacts,
) -> Option<Shown> {
    let readable = current_epoch(candidate, work)?;
    if candidate.kind == GroupKind::RootApproved && work.owner != viewer {
        return None;
    }
    let target_work = candidate.target.work();
    let valid = |(comment, actor): &(CommentId, PrincipalId)| {
        facts.comments.get(comment).is_some_and(|facts| {
            facts.public && facts.work == target_work && facts.author == *actor
        })
    };
    let (chosen, chosen_author) = *candidate.contributions.iter().find(|c| valid(c))?;
    let chosen_facts = facts.comments.get(&chosen)?;
    let target = match candidate.target {
        InboxTarget::CommentThread { work, .. } => InboxTarget::CommentThread {
            work,
            comment: chosen,
            moderation_version: chosen_facts.moderation_version,
        },
        InboxTarget::CommentReply {
            work,
            root,
            context,
            ..
        } => InboxTarget::CommentReply {
            work,
            root,
            context,
            comment: chosen,
            moderation_version: chosen_facts.moderation_version,
        },
        _ => return None,
    };
    let invalid: Vec<&(CommentId, PrincipalId)> = candidate
        .contributions
        .iter()
        .filter(|c| !valid(c))
        .collect();
    let complete = candidate.contribution_total <= candidate.contributions.len() as i64;
    let mut valid_actors: Vec<PrincipalId> = Vec::new();
    for (_, actor) in candidate.contributions.iter().filter(|c| valid(c)) {
        if !valid_actors.contains(actor) {
            valid_actors.push(*actor);
        }
    }
    let (preview, count) = if invalid.is_empty() {
        (candidate.actors_preview.clone(), candidate.actor_count)
    } else if complete {
        (
            valid_actors.iter().copied().take(PREVIEW_ACTORS).collect(),
            i32::try_from(valid_actors.len()).unwrap_or(i32::MAX),
        )
    } else {
        // Older contributions were not returned: drop only actors whose every returned
        // contribution is invalid. Bounded; the service converges once invalidations arrive.
        let valid_set: HashSet<PrincipalId> = valid_actors.iter().copied().collect();
        let gone: HashSet<PrincipalId> = invalid
            .iter()
            .map(|(_, actor)| *actor)
            .filter(|actor| !valid_set.contains(actor))
            .collect();
        let preview = candidate
            .actors_preview
            .iter()
            .copied()
            .filter(|actor| !gone.contains(actor))
            .take(PREVIEW_ACTORS)
            .collect();
        let gone = i32::try_from(gone.len()).unwrap_or(i32::MAX);
        (preview, (candidate.actor_count - gone).max(1))
    };
    let mut preview: Vec<PrincipalId> = preview;
    if !preview.contains(&chosen_author) {
        preview.insert(0, chosen_author);
        preview.truncate(PREVIEW_ACTORS);
    }
    Some(Shown {
        target,
        work_title: Some(readable.title.clone()),
        actors_preview: preview,
        actor_count: count.max(1),
        item_count: candidate.item_count,
        excerpt: chosen_facts.body.as_deref().and_then(Excerpt::of),
        outcome: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(0x1000_0000_0000_4000_8000_0000_0000_0000 + n)
    }
    fn p(n: u128) -> PrincipalId {
        PrincipalId::from_uuid(id(n))
    }
    fn c(n: u128) -> CommentId {
        CommentId::from_uuid(id(100 + n))
    }
    const OWNER: u128 = 1;
    fn work() -> WorkId {
        WorkId::from_uuid(id(900))
    }
    fn revision() -> RevisionId {
        RevisionId::from_uuid(id(901))
    }

    fn facts(readable_epoch: Option<i64>) -> ReadFacts {
        let mut facts = ReadFacts::default();
        facts.works.insert(
            work(),
            WorkFacts {
                owner: p(OWNER),
                readable: readable_epoch.map(|epoch| Readable {
                    revision: revision(),
                    epoch,
                    title: "Chiều qua bến cũ".into(),
                }),
            },
        );
        facts
    }

    fn comment(facts: &mut ReadFacts, n: u128, author: u128, public: bool, body: &str) {
        facts.comments.insert(
            c(n),
            CommentFacts {
                work: work(),
                author: p(author),
                public,
                moderation_version: 2,
                body: public.then(|| body.to_owned()),
            },
        );
    }

    fn root_group(contributions: &[(u128, u128)], actor_count: i32, total: i64) -> Candidate {
        let newest = contributions[0];
        Candidate::new(
            GroupKind::RootApproved,
            InboxTarget::CommentThread {
                work: work(),
                comment: c(newest.0),
                moderation_version: 2,
            },
            true,
            (actor_count, 0),
            contributions
                .iter()
                .map(|(_, a)| p(*a))
                .fold(Vec::new(), |mut v, a| {
                    if !v.contains(&a) {
                        v.push(a);
                    }
                    v
                })
                .into_iter()
                .take(3)
                .collect(),
            (
                contributions
                    .iter()
                    .map(|(cm, a)| (c(*cm), p(*a)))
                    .collect(),
                total,
            ),
            Some(1),
            false,
        )
        .unwrap()
    }

    #[test]
    fn a_target_of_another_type_is_never_a_candidate() {
        let target = InboxTarget::Endorsement {
            work: work(),
            revision: revision(),
        };
        assert!(
            Candidate::new(
                GroupKind::RootApproved,
                target,
                true,
                (1, 0),
                vec![],
                (vec![], 0),
                Some(1),
                false
            )
            .is_none()
        );
    }

    #[test]
    fn targets_round_trip_through_the_contract_shape() {
        for target in [
            InboxTarget::CommentReply {
                work: work(),
                root: c(1),
                context: ReplyContext::Purged,
                comment: c(2),
                moderation_version: 3,
            },
            InboxTarget::CommentReply {
                work: work(),
                root: c(1),
                context: ReplyContext::Reply(c(5)),
                comment: c(2),
                moderation_version: 3,
            },
            InboxTarget::WorkDecision {
                work: work(),
                revision: revision(),
                decision_version: 4,
            },
        ] {
            assert_eq!(InboxTarget::parse(&target.json()), Some(target));
        }
        let mut upper = InboxTarget::Endorsement {
            work: work(),
            revision: revision(),
        }
        .json();
        upper["work_id"] = json!("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA");
        assert_eq!(
            InboxTarget::parse(&upper),
            None,
            "only the canonical lowercase spelling"
        );
        upper["work_id"] = json!("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
        assert!(InboxTarget::parse(&upper).is_some());
        let reserved = json!({"kind": "admin_notice_reserved", "notice_id": id(1).to_string()});
        assert_eq!(InboxTarget::parse(&reserved), None);
    }

    #[test]
    fn the_newest_valid_comment_is_the_target_when_the_newest_lost_visibility() {
        let mut facts = facts(Some(1));
        // B wrote c1 then c2; c2 is held (no longer public) before its invalidation arrived.
        comment(&mut facts, 1, 2, true, "  Một   câu\nbình luận  ");
        comment(&mut facts, 2, 2, false, "đang giữ");
        comment(&mut facts, 3, 3, true, "khác");
        let group = root_group(&[(2, 2), (3, 3), (1, 2)], 2, 3);
        let shown = decide(p(OWNER), &group, &facts).unwrap();
        assert_eq!(
            shown.target,
            InboxTarget::CommentThread {
                work: work(),
                comment: c(3),
                moderation_version: 2
            }
        );
        assert_eq!(shown.excerpt.unwrap().text, "khác");
        // B still counts once through c1.
        assert_eq!(shown.actor_count, 2);
        assert_eq!(shown.actors_preview, vec![p(3), p(2)]);
    }

    #[test]
    fn a_group_without_any_valid_contribution_is_hidden() {
        let mut facts = facts(Some(1));
        comment(&mut facts, 1, 2, false, "gỡ");
        assert_eq!(decide(p(OWNER), &root_group(&[(1, 2)], 1, 1), &facts), None);
        // A comment the App no longer knows (purged) is not valid either.
        assert_eq!(decide(p(OWNER), &root_group(&[(9, 2)], 1, 1), &facts), None);
    }

    #[test]
    fn an_actor_whose_comments_all_lost_visibility_leaves_preview_and_count() {
        let mut facts = facts(Some(1));
        comment(&mut facts, 1, 2, false, "x");
        comment(&mut facts, 2, 3, true, "y");
        comment(&mut facts, 3, 4, true, "z");
        let shown = decide(
            p(OWNER),
            &root_group(&[(1, 2), (2, 3), (3, 4)], 3, 3),
            &facts,
        )
        .unwrap();
        assert_eq!(shown.actor_count, 2);
        assert_eq!(shown.actors_preview, vec![p(3), p(4)]);
        // Incomplete list: only actors without any valid returned contribution are dropped.
        let shown = decide(p(OWNER), &root_group(&[(1, 2), (2, 3)], 9, 40), &facts).unwrap();
        assert_eq!(shown.actor_count, 8);
        assert!(!shown.actors_preview.contains(&p(2)));
    }

    #[test]
    fn interactions_need_a_readable_work_in_the_stamped_epoch() {
        let mut readable = facts(Some(1));
        comment(&mut readable, 1, 2, true, "x");
        let group = root_group(&[(1, 2)], 1, 1);
        assert!(decide(p(OWNER), &group, &readable).is_some());
        // Hidden or private now: no title, actor or excerpt leaks.
        let mut hidden = facts(None);
        comment(&mut hidden, 1, 2, true, "x");
        assert_eq!(decide(p(OWNER), &group, &hidden), None);
        // Republished in a new epoch: earlier signals stay hidden (COM-11).
        let mut republished = facts(Some(2));
        comment(&mut republished, 1, 2, true, "x");
        assert_eq!(decide(p(OWNER), &group, &republished), None);
        // A root group belongs to the work's owner only.
        assert_eq!(decide(p(7), &group, &readable), None);
    }

    #[test]
    fn decisions_are_private_to_the_owner_or_writer_whatever_the_publication() {
        let mut facts = facts(None);
        facts.revision_titles.insert(revision(), "Bản riêng".into());
        let decision = Candidate::new(
            GroupKind::WorkDecision,
            InboxTarget::WorkDecision {
                work: work(),
                revision: revision(),
                decision_version: 3,
            },
            true,
            (0, 1),
            vec![],
            (vec![], 0),
            None,
            true,
        )
        .unwrap();
        let shown = decide(p(OWNER), &decision, &facts).unwrap();
        assert_eq!(shown.outcome, Some("rejected"));
        assert_eq!(shown.work_title.as_deref(), Some("Bản riêng"));
        assert_eq!(decide(p(2), &decision, &facts), None);
        comment(&mut facts, 1, 2, false, "đã gỡ");
        let adverse = Candidate::new(
            GroupKind::CommentAdverse,
            InboxTarget::CommentDecision {
                work: work(),
                comment: c(1),
                moderation_version: 4,
            },
            true,
            (0, 1),
            vec![],
            (vec![], 0),
            None,
            false,
        )
        .unwrap();
        // The decision row must exist at that version.
        assert_eq!(decide(p(2), &adverse, &facts), None);
        facts.comment_decisions.insert((c(1), 4), true);
        let shown = decide(p(2), &adverse, &facts).unwrap();
        assert_eq!(shown.outcome, Some("removed"));
        assert_eq!(
            shown.work_title, None,
            "the writer sees no title of a hidden work"
        );
        assert_eq!(shown.excerpt, None, "never a body");
        // The work's owner is not the writer of a reader's comment.
        assert_eq!(decide(p(OWNER), &adverse, &facts), None);
    }

    #[test]
    fn excerpts_cut_on_grapheme_boundaries_after_nfc() {
        // "ệ" written as e + combining circumflex + combining dot below (NFD).
        let decomposed = "Tie\u{0302}\u{0323}ng Vie\u{0323}\u{0302}t";
        assert_eq!(Excerpt::of(decomposed).unwrap().text, "Tiệng Việt");
        let long: String = "ố".repeat(EXCERPT_MAX_GRAPHEMES + 5);
        let excerpt = Excerpt::of(&long).unwrap();
        assert!(excerpt.truncated);
        assert_eq!(excerpt.text.graphemes(true).count(), EXCERPT_MAX_GRAPHEMES);
        // A family emoji is one grapheme and never split.
        let family = "👨‍👩‍👧";
        let text = format!("{}{family}", "a".repeat(EXCERPT_MAX_GRAPHEMES - 1));
        let excerpt = Excerpt::of(&text).unwrap();
        assert!(!excerpt.truncated);
        assert!(excerpt.text.ends_with(family));
        assert_eq!(Excerpt::of(" \n\t "), None);
    }
}
