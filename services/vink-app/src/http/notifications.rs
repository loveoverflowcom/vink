//! Notifications through the App Gateway (#348; K5, N4, N6, P-04B, P-08, P-12A, T-06).
//!
//! - **Private** (`/me/notifications…`): badge count, inbox pages, Seen/Read/MarkAllRead and the
//!   private decision details. The inbox owner is the principal resolved from the session; no
//!   client field names it. Every response is `no-store`.
//! - **Public**: the paginated list of people currently Thấm-ing a public revision, read from
//!   Endorsement, never from the inbox. Guests may read it; success carries a strong `ETag`.
//!
//! T-06 CHOICE-20 **A**: the Gateway scans bounded candidate batches from the service, decides
//! each group on one App snapshot per batch and continues with a finite cursor when a scan
//! budget is spent, never concluding the end of the list from a filtered batch.
//!
//! Snapshot and cursor tokens of the service are wrapped in a MAC keyed by the session's
//! credential digest and bound to the principal and purpose: a token of another session,
//! account or purpose is refused before it can reach the service (A → B → A).

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::Sha256;
use time::OffsetDateTime;
use tower_http::set_header::SetResponseHeaderLayer;
use uuid::Uuid;

use super::api::json_body;
use super::conditional::{self, PUBLIC_ERROR_CACHE_CONTROL};
use super::contract::AuthorProjection;
use super::{ApiError, AppState, PrivateSession};
use crate::application::profile::AuthorAttribution;
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};
use crate::domain::notification_inbox::{
    Candidate, GroupKind, InboxTarget, ReadFacts, ReplyContext, Shown, decide,
};
use crate::notification_inbox::{
    InboxCommand, InboxFailure, NotificationInbox, WireGroup, WirePage, parse_snapshot,
};
use crate::persistence::notification_reads::{EndorserRead, MAX_ENDORSER_PAGE};

/// Service groups per candidate batch (the service's own page maximum).
pub const SCAN_BATCH: u8 = 50;
/// Candidate batches one inbox page may scan before answering with a continuation.
pub const SCAN_BATCHES: usize = 3;
/// Unseen candidates one badge count may authorize; beyond it the count is a lower bound.
pub const COUNT_SCAN_BATCHES: usize = 2;
/// How long a verified count may be served as `stale` (P-04B, the service's published bound).
pub const MAX_STALENESS: Duration = Duration::from_secs(60);
const BADGE_CACHE_CAP: usize = 10_000;
const DEFAULT_PAGE: u8 = 20;
const MAX_COMMAND_KEY: usize = 128;

pub(super) fn routes() -> Router<AppState> {
    let private = Router::new()
        .route("/me/notifications", get(list))
        .route("/me/notifications/count", get(count))
        .route("/me/notifications/seen", post(seen))
        .route("/me/notifications/read", post(read))
        .route("/me/notifications/mark-all-read", post(mark_all_read))
        .route(
            "/me/notifications/decisions/works/{work_id}/revisions/{revision_id}/{decision_version}",
            get(work_decision),
        )
        .route(
            "/me/notifications/decisions/comments/{comment_id}/{moderation_version}",
            get(comment_decision),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    let public = Router::new()
        .route(
            "/works/{work_id}/revisions/{revision_id}/endorsers",
            get(endorsers),
        )
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static(PUBLIC_ERROR_CACHE_CONTROL),
        ));
    private.merge(public)
}

// ---------------------------------------------------------------------------------------------
// Session-bound tokens.

fn token_mac(session: &PrivateSession, purpose: &[u8], payload: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(&session.digest)
        .expect("HMAC supports a 32-byte session key");
    mac.update(b"vink:notifications:v1:");
    mac.update(purpose);
    mac.update(session.actor.as_uuid().as_bytes());
    mac.update(payload);
    mac.finalize().into_bytes()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn browser_scope(session: &PrivateSession) -> String {
    token_mac(session, b"browser-scope", b"")
}

/// Older API consumers can omit this header. Web always binds a request to the credential
/// confirmed by SessionOwner, even during a cookie change the tab has not observed yet.
struct NotificationSession(PrivateSession);

impl FromRequestParts<AppState> for NotificationSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, ApiError> {
        let session = PrivateSession::from_request_parts(parts, state).await?;
        if let Some(scope) = parts.headers.get("x-vink-notification-scope")
            && !scope
                .to_str()
                .is_ok_and(|scope| verify(&session, b"browser-scope", b"", scope))
        {
            return Err(ApiError::Unauthenticated);
        }
        Ok(Self(session))
    }
}

fn verify(session: &PrivateSession, purpose: &[u8], payload: &[u8], tag: &str) -> bool {
    // Equal-length hex comparison of two MACs; the expected tag is not secret material.
    let expected = token_mac(session, purpose, payload);
    expected.len() == tag.len()
        && expected
            .bytes()
            .zip(tag.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn snapshot_token(session: &PrivateSession, seq: i64) -> String {
    format!(
        "s1.{seq}.{}",
        token_mac(session, b"snapshot", &seq.to_be_bytes())
    )
}

fn open_snapshot(session: &PrivateSession, token: &str) -> Option<i64> {
    let (seq, tag) = token.strip_prefix("s1.")?.split_once('.')?;
    let seq = parse_snapshot(&format!("s{seq}"))?;
    verify(session, b"snapshot", &seq.to_be_bytes(), tag).then_some(seq)
}

fn cursor_payload(seq: i64, group: Uuid) -> Vec<u8> {
    let mut payload = seq.to_be_bytes().to_vec();
    payload.extend_from_slice(group.as_bytes());
    payload
}

fn cursor_token(session: &PrivateSession, seq: i64, group: Uuid) -> String {
    format!(
        "n1.{seq}.{}.{}",
        group.simple(),
        token_mac(session, b"cursor", &cursor_payload(seq, group))
    )
}

/// The service cursor (`c<seq>.<uuid>`) behind a Gateway cursor of this session.
fn open_cursor(session: &PrivateSession, token: &str) -> Option<String> {
    let mut parts = token.strip_prefix("n1.")?.splitn(3, '.');
    let seq = parse_snapshot(&format!("s{}", parts.next()?))?;
    let raw = parts.next()?;
    let group = Uuid::try_parse(raw).ok()?;
    if group.simple().to_string() != raw {
        return None;
    }
    let tag = parts.next()?;
    verify(session, b"cursor", &cursor_payload(seq, group), tag)
        .then(|| format!("c{seq}.{}", group.hyphenated()))
}

// ---------------------------------------------------------------------------------------------
// Errors and helpers.

fn inbox(state: &AppState) -> Result<&dyn NotificationInbox, ApiError> {
    state
        .notifications
        .as_deref()
        .ok_or(ApiError::DependencyUnavailable)
}

fn token_refused(field: &'static str) -> ApiError {
    ApiError::Validation {
        field,
        message: "unknown, expired or foreign token".into(),
    }
}

impl From<InboxFailure> for ApiError {
    fn from(failure: InboxFailure) -> Self {
        match failure {
            InboxFailure::Unavailable => Self::DependencyUnavailable,
            InboxFailure::InvalidSnapshot => token_refused("up_to_snapshot"),
            InboxFailure::InvalidCursor => token_refused("cursor"),
            InboxFailure::InvalidVersion => Self::Validation {
                field: "observed_group_version",
                message: "the group never had that version".into(),
            },
            InboxFailure::KeyReused => Self::KeyReused,
            InboxFailure::NotFound => Self::NotFound,
        }
    }
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn principal(raw: Uuid) -> PrincipalId {
    PrincipalId::from_uuid(raw)
}

/// Types a service group; `None` (never shown) for an unknown type or a malformed target.
fn candidate(group: &WireGroup) -> Option<Candidate> {
    let kind = GroupKind::from_code(&group.event_type)?;
    let target = InboxTarget::parse(&group.target)?;
    let (contributions, total) = match &group.contributions {
        Some(found) => (
            found
                .newest
                .iter()
                .map(|c| (CommentId::from_uuid(c.comment_id), principal(c.actor_id)))
                .collect(),
            found.total,
        ),
        None => (Vec::new(), 0),
    };
    Candidate::new(
        kind,
        target,
        group.unseen,
        (group.actor_count, group.item_count),
        group
            .actors_preview
            .iter()
            .copied()
            .map(principal)
            .collect(),
        (contributions, total),
        group.publication_epoch,
        group.action_required,
    )
}

type Identities = HashMap<PrincipalId, AuthorAttribution>;

/// One App snapshot for a batch of candidates: a fixed number of statements (no N+1).
async fn facts_for(
    state: &AppState,
    viewer: PrincipalId,
    candidates: &[Option<Candidate>],
    identities: bool,
) -> Result<(ReadFacts, Identities), ApiError> {
    let mut works = BTreeSet::new();
    let mut comments = BTreeSet::new();
    let mut revisions = BTreeSet::new();
    let mut decisions = BTreeSet::new();
    let mut actors = BTreeSet::new();
    for candidate in candidates.iter().flatten() {
        works.insert(candidate.target.work());
        comments.extend(candidate.comments());
        match candidate.target {
            InboxTarget::CommentReply {
                context: ReplyContext::Reply(parent),
                ..
            } if identities => {
                comments.insert(parent);
            }
            InboxTarget::WorkDecision { work, revision, .. } => {
                revisions.insert((work, revision));
            }
            InboxTarget::CommentDecision {
                comment,
                moderation_version,
                ..
            } => {
                decisions.insert((comment, moderation_version));
            }
            _ => {}
        }
        if identities {
            actors.extend(candidate.actors_preview.iter().copied());
            actors.extend(candidate.contributions.iter().map(|(_, actor)| *actor));
        }
    }
    fn collect<T>(set: BTreeSet<T>) -> Vec<T> {
        set.into_iter().collect()
    }
    Ok(state
        .vink
        .store()
        .notification_facts(
            viewer,
            &collect(works),
            &collect(comments),
            &collect(revisions),
            &collect(decisions),
            &collect(actors),
        )
        .await?)
}

fn summary_kind(kind: GroupKind, shown: &Shown) -> &'static str {
    match (kind, shown.outcome) {
        (GroupKind::RootApproved, _) => "comment_root_rollup",
        (GroupKind::ReplyApproved, _) => "comment_reply_rollup",
        (GroupKind::Tham, _) => "work_tham_rollup",
        (GroupKind::Ung, _) => "comment_ung_rollup",
        (GroupKind::WorkDecision, Some("rejected")) => "work_moderation_rejected",
        (GroupKind::WorkDecision, _) => "work_moderation_approved",
        (GroupKind::CommentAdverse, Some("removed")) => "comment_moderation_removed",
        (GroupKind::CommentAdverse, _) => "comment_moderation_rejected",
    }
}

fn item_json(group: &WireGroup, kind: GroupKind, shown: &Shown, identities: &Identities) -> Value {
    let mut target = shown.target.json();
    target["work_title"] = json!(shown.work_title);
    let actors: Vec<AuthorProjection> = shown
        .actors_preview
        .iter()
        .map(|actor| {
            identities.get(actor).map_or_else(
                || AuthorProjection::Unavailable {
                    label: crate::persistence::notification_reads::UNAVAILABLE_LABEL.into(),
                },
                AuthorProjection::from,
            )
        })
        .collect();
    let mut item = json!({
        "group_id": group.group_id,
        "group_version": group.group_version,
        "type": kind.code(),
        "is_read": !group.unread,
        "is_unseen": group.unseen,
        "actor_count": shown.actor_count,
        "item_count": shown.item_count,
        "actors_preview": actors,
        "target": target,
        "summary": {
            "kind": summary_kind(kind, shown),
            "action_required": group.action_required,
        },
        "created_at": rfc3339(group.created_at),
        "updated_at": rfc3339(group.last_signal_at),
        "expires_at": rfc3339(group.expires_at),
    });
    if matches!(kind, GroupKind::RootApproved | GroupKind::ReplyApproved) {
        item["excerpt"] = shown.excerpt.as_ref().map_or(
            Value::Null,
            |excerpt| json!({ "text": excerpt.text, "truncated": excerpt.truncated }),
        );
    }
    item
}

// ---------------------------------------------------------------------------------------------
// Badge count (P-09A, P-04B).

/// Last verified count per principal, kept only to answer `stale` within [`MAX_STALENESS`].
#[derive(Default)]
pub struct BadgeCache {
    entries: Mutex<HashMap<PrincipalId, Known>>,
}

#[derive(Debug, Clone, Copy)]
struct Known {
    count: i64,
    lower_bound: bool,
    snapshot: i64,
    at: Instant,
}

impl BadgeCache {
    fn remember(&self, viewer: PrincipalId, known: Known) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        if entries.len() >= BADGE_CACHE_CAP {
            entries.retain(|_, known| known.at.elapsed() < MAX_STALENESS);
            if entries.len() >= BADGE_CACHE_CAP {
                return;
            }
        }
        entries.insert(viewer, known);
    }

    fn recall(&self, viewer: PrincipalId) -> Option<Known> {
        let entries = self.entries.lock().ok()?;
        entries
            .get(&viewer)
            .copied()
            .filter(|known| known.at.elapsed() < MAX_STALENESS)
    }
}

/// Counts unseen groups the viewer may see now: the service count first (zero answers without
/// any scan), then at most [`COUNT_SCAN_BATCHES`] batches of the newest groups, which hold every
/// unseen candidate first because the inbox is ordered by newest signal.
async fn verified_count(
    state: &AppState,
    viewer: PrincipalId,
) -> Result<(i64, bool, i64), ApiError> {
    let inbox = inbox(state)?;
    let raw = inbox.count(viewer).await?;
    let raw_snapshot =
        parse_snapshot(&raw.as_of_snapshot).ok_or(ApiError::DependencyUnavailable)?;
    if raw.unseen_group_count == 0 {
        return Ok((0, false, raw_snapshot));
    }
    let mut cursor: Option<String> = None;
    let mut visible = 0i64;
    let mut snapshot = None;
    for batch in 0..COUNT_SCAN_BATCHES {
        let page = inbox.page(viewer, SCAN_BATCH, cursor.as_deref()).await?;
        let seen = parse_snapshot(&page.seen_snapshot).ok_or(ApiError::DependencyUnavailable)?;
        let page_snapshot =
            parse_snapshot(&page.inbox_snapshot).ok_or(ApiError::DependencyUnavailable)?;
        snapshot.get_or_insert(page_snapshot);
        let newer: Vec<&WireGroup> = page
            .groups
            .iter()
            .take_while(|group| group.signal_version > seen)
            .collect();
        let reached_seen = newer.len() < page.groups.len();
        let candidates: Vec<Option<Candidate>> = newer
            .iter()
            .filter(|group| group.unseen)
            .map(|group| candidate(group))
            .collect();
        let (facts, _) = facts_for(state, viewer, &candidates, false).await?;
        visible += candidates
            .iter()
            .flatten()
            .filter(|candidate| decide(viewer, candidate, &facts).is_some())
            .count() as i64;
        match (&page.next_cursor, reached_seen) {
            (Some(next), false) => {
                if batch + 1 == COUNT_SCAN_BATCHES {
                    return Ok((visible, true, snapshot.unwrap_or(page_snapshot)));
                }
                cursor = Some(next.clone());
            }
            _ => break,
        }
    }
    Ok((visible, false, snapshot.unwrap_or(raw_snapshot)))
}

async fn count(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
) -> Result<Response, ApiError> {
    let viewer = session.actor;
    match verified_count(&state, viewer).await {
        Ok((count, lower_bound, snapshot)) => {
            state.badges.remember(
                viewer,
                Known {
                    count,
                    lower_bound,
                    snapshot,
                    at: Instant::now(),
                },
            );
            Ok(Json(json!({
                "freshness": {
                    "status": "fresh",
                    "unseen_group_count": count,
                    "count_is_lower_bound": lower_bound,
                    "as_of_snapshot": snapshot_token(&session, snapshot),
                    "max_staleness_seconds": MAX_STALENESS.as_secs(),
                }
            }))
            .into_response())
        }
        Err(ApiError::DependencyUnavailable) => match state.badges.recall(viewer) {
            Some(known) => Ok(Json(json!({
                "freshness": {
                    "status": "stale",
                    "last_known_count": known.count,
                    "count_is_lower_bound": known.lower_bound,
                    "as_of_snapshot": snapshot_token(&session, known.snapshot),
                    "max_staleness_seconds": MAX_STALENESS.as_secs(),
                    "age_seconds": known.at.elapsed().as_secs(),
                    "reason": "dependency_unavailable",
                }
            }))
            .into_response()),
            None => Err(ApiError::DependencyUnavailable),
        },
        Err(other) => Err(other),
    }
}

// ---------------------------------------------------------------------------------------------
// Inbox pages (T-06 A).

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct InboxQuery {
    limit: Option<u8>,
    cursor: Option<String>,
}

/// Measured shape of one page, for tests and logs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanStats {
    pub service_calls: usize,
    pub candidates: usize,
}

async fn list(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
    query: Result<Query<InboxQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let query = query
        .map(|Query(query)| query)
        .map_err(|rejection| ApiError::Validation {
            field: "query",
            message: rejection.body_text(),
        })?;
    let limit = query.limit.unwrap_or(DEFAULT_PAGE);
    if !(1..=SCAN_BATCH).contains(&limit) {
        return Err(ApiError::Validation {
            field: "limit",
            message: format!("limit must be between 1 and {SCAN_BATCH}"),
        });
    }
    let mut cursor = match query.cursor.as_deref() {
        Some(token) => Some(open_cursor(&session, token).ok_or_else(|| token_refused("cursor"))?),
        None => None,
    };
    let viewer = session.actor;
    let inbox = inbox(&state)?;
    let mut items = Vec::new();
    let mut snapshot: Option<i64> = None;
    let mut next: Option<(i64, Uuid)> = None;
    let mut reason: Option<&'static str> = None;
    let mut stats = ScanStats::default();
    for batch in 0..SCAN_BATCHES {
        let page: WirePage = inbox.page(viewer, SCAN_BATCH, cursor.as_deref()).await?;
        stats.service_calls += 1;
        stats.candidates += page.groups.len();
        snapshot.get_or_insert(
            parse_snapshot(&page.inbox_snapshot).ok_or(ApiError::DependencyUnavailable)?,
        );
        let candidates: Vec<Option<Candidate>> = page.groups.iter().map(candidate).collect();
        let (facts, identities) = facts_for(&state, viewer, &candidates, true).await?;
        let mut consumed: usize = 0;
        for (group, candidate) in page.groups.iter().zip(&candidates) {
            consumed += 1;
            if let Some(candidate) = candidate
                && let Some(shown) = decide(viewer, candidate, &facts)
            {
                let mut item = item_json(group, candidate.kind, &shown, &identities);
                match shown.target {
                    InboxTarget::WorkDecision { revision, .. } => {
                        if let Some(date) = facts.revision_dates.get(&revision) {
                            item["target"]["revision_created_at"] = json!(rfc3339(*date));
                        }
                    }
                    InboxTarget::CommentReply { context, work, .. } => {
                        let direct = match context {
                            ReplyContext::Root => Some(true),
                            ReplyContext::Reply(parent) => facts
                                .comments
                                .get(&parent)
                                .filter(|comment| comment.work == work)
                                .map(|comment| comment.author == viewer),
                            ReplyContext::Purged => None,
                        };
                        if let Some(direct) = direct {
                            item["summary"]["reply_to_viewer"] = json!(direct);
                        }
                    }
                    _ => {}
                }
                items.push(item);
                if items.len() == usize::from(limit) {
                    break;
                }
            }
        }
        let last = page
            .groups
            .get(consumed.saturating_sub(1))
            .map(|group| (group.signal_version, group.group_id));
        if items.len() == usize::from(limit) {
            if consumed < page.groups.len() || page.next_cursor.is_some() {
                next = last;
                reason = Some("more_items");
            }
            break;
        }
        match page.next_cursor {
            None => break,
            Some(service_next) => {
                if batch + 1 == SCAN_BATCHES {
                    next = last;
                    reason = Some("filtered_page_scan_bound");
                } else {
                    cursor = Some(service_next);
                }
            }
        }
    }
    tracing::debug!(
        service_calls = stats.service_calls,
        candidates = stats.candidates,
        shown = items.len(),
        "inbox page"
    );
    let snapshot = snapshot.ok_or(ApiError::DependencyUnavailable)?;
    Ok(Json(json!({
        "inbox_snapshot": snapshot_token(&session, snapshot),
        "items": items,
        "next_cursor": next.map(|(seq, group)| cursor_token(&session, seq, group)),
        "continuation_reason": reason,
    }))
    .into_response())
}

// ---------------------------------------------------------------------------------------------
// ACK commands (P-09A, P-10A).

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotRequest {
    command_key: String,
    up_to_snapshot: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadRequest {
    command_key: String,
    group_id: Uuid,
    observed_group_version: i64,
}

fn command_key(raw: &str) -> Result<&str, ApiError> {
    if (1..=MAX_COMMAND_KEY).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_graphic()) {
        Ok(raw)
    } else {
        Err(ApiError::Validation {
            field: "command_key",
            message: format!("1..{MAX_COMMAND_KEY} printable ASCII characters"),
        })
    }
}

/// Origin is checked by the API middleware; standing is fenced here, at the App boundary, right
/// before the command reaches the Notification Service.
async fn run(
    state: &AppState,
    viewer: PrincipalId,
    key: &str,
    command: InboxCommand,
) -> Result<Value, ApiError> {
    let inbox = inbox(state)?;
    state.vink.store().require_active_viewer(viewer).await?;
    Ok(inbox.command(viewer, key, &command).await?)
}

fn answered_snapshot(
    session: &PrivateSession,
    answer: &Value,
    field: &str,
) -> Result<String, ApiError> {
    let seq = answer[field]
        .as_str()
        .and_then(parse_snapshot)
        .ok_or(ApiError::DependencyUnavailable)?;
    Ok(snapshot_token(session, seq))
}

async fn seen(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
    body: Result<Json<SnapshotRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let request = json_body(body)?;
    let key = command_key(&request.command_key)?;
    let up_to = open_snapshot(&session, &request.up_to_snapshot)
        .ok_or_else(|| token_refused("up_to_snapshot"))?;
    let answer = run(&state, session.actor, key, InboxCommand::Seen { up_to }).await?;
    Ok(Json(json!({
        "seen_snapshot": answered_snapshot(&session, &answer, "seen_snapshot")?,
    }))
    .into_response())
}

async fn read(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
    body: Result<Json<ReadRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let request = json_body(body)?;
    let key = command_key(&request.command_key)?;
    if request.observed_group_version < 1 {
        return Err(InboxFailure::InvalidVersion.into());
    }
    let command = InboxCommand::Read {
        group_id: request.group_id,
        observed_version: request.observed_group_version,
    };
    let answer = run(&state, session.actor, key, command).await?;
    let (Some(group_version), Some(unread)) =
        (answer["group_version"].as_i64(), answer["unread"].as_bool())
    else {
        return Err(ApiError::DependencyUnavailable);
    };
    Ok(Json(json!({
        "group_id": request.group_id,
        "group_version": group_version,
        "is_read": !unread,
    }))
    .into_response())
}

async fn mark_all_read(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
    body: Result<Json<SnapshotRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let request = json_body(body)?;
    let key = command_key(&request.command_key)?;
    let up_to = open_snapshot(&session, &request.up_to_snapshot)
        .ok_or_else(|| token_refused("up_to_snapshot"))?;
    let answer = run(
        &state,
        session.actor,
        key,
        InboxCommand::MarkAllRead { up_to },
    )
    .await?;
    Ok(Json(json!({
        "up_to_snapshot": answered_snapshot(&session, &answer, "up_to_snapshot")?,
        "groups_marked": answer["groups_marked"].as_i64().ok_or(ApiError::DependencyUnavailable)?,
    }))
    .into_response())
}

// ---------------------------------------------------------------------------------------------
// Private decision details (P-12A).

fn version(raw: &str) -> Result<i64, ApiError> {
    raw.parse::<i64>()
        .ok()
        .filter(|v| *v >= 1)
        .ok_or(ApiError::NotFound)
}

async fn work_decision(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
    Path((work, revision, decision_version)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    let work = WorkId::parse(&work).ok_or(ApiError::NotFound)?;
    let revision = RevisionId::parse(&revision).ok_or(ApiError::NotFound)?;
    let decision_version = version(&decision_version)?;
    let detail = state
        .vink
        .store()
        .work_decision_detail(session.actor, work, revision, decision_version)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(json!({
        "kind": "work_moderation_decision",
        "work_id": work.as_uuid(),
        "work_title": detail.title,
        "revision_id": revision.as_uuid(),
        "decision_version": decision_version,
        "outcome": detail.outcome,
        "action_required": detail.action_required,
        "reason": { "code": detail.reason_code, "message": detail.reason_message },
        "can_appeal": detail.can_appeal,
        "is_current": detail.is_current,
        "decided_at": rfc3339(detail.decided_at),
    }))
    .into_response())
}

async fn comment_decision(
    State(state): State<AppState>,
    NotificationSession(session): NotificationSession,
    Path((comment, moderation_version)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let comment = CommentId::parse(&comment).ok_or(ApiError::NotFound)?;
    let moderation_version = version(&moderation_version)?;
    let detail = state
        .vink
        .store()
        .comment_decision_detail(session.actor, comment, moderation_version)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(json!({
        "kind": "comment_moderation_decision",
        "work_id": detail.work.as_uuid(),
        "work_title": detail.work_title,
        "comment_id": comment.as_uuid(),
        "moderation_version": moderation_version,
        "outcome": detail.outcome,
        "action_required": false,
        "comment_body": detail.comment_body,
        "body_format": detail.body_format,
        "reason": { "code": Value::Null, "message": detail.reason_message },
        "is_current": detail.is_current,
        "decided_at": rfc3339(detail.decided_at),
    }))
    .into_response())
}

// ---------------------------------------------------------------------------------------------
// Public Thấm list (P-08 R3).

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct EndorserQuery {
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(serde::Serialize)]
struct EndorserItem {
    author: AuthorProjection,
}

#[derive(serde::Serialize)]
struct EndorserList {
    work_id: Uuid,
    revision_id: Uuid,
    total_count: i64,
    items: Vec<EndorserItem>,
    next_cursor: Option<String>,
}

/// `e1.<revision>.<position>`: public and not secret, but bound to its revision so a cursor of
/// an older revision is told apart (`409 revision_changed`) from a malformed one.
fn endorser_cursor(revision: RevisionId, position: Uuid) -> String {
    format!("e1.{}.{}", revision.as_uuid().simple(), position.simple())
}

fn open_endorser_cursor(token: &str) -> Option<(Uuid, Uuid)> {
    let (revision, position) = token.strip_prefix("e1.")?.split_once('.')?;
    let parse = |raw: &str| {
        let id = Uuid::try_parse(raw).ok()?;
        (id.simple().to_string() == raw).then_some(id)
    };
    Some((parse(revision)?, parse(position)?))
}

async fn endorsers(
    State(state): State<AppState>,
    Path((work, revision)): Path<(String, String)>,
    query: Result<Query<EndorserQuery>, QueryRejection>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let work = WorkId::parse(&work).ok_or(ApiError::NotFound)?;
    let revision = RevisionId::parse(&revision).ok_or(ApiError::NotFound)?;
    let query = query
        .map(|Query(query)| query)
        .map_err(|rejection| ApiError::Validation {
            field: "query",
            message: rejection.body_text(),
        })?;
    let limit = query.limit.unwrap_or(i64::from(DEFAULT_PAGE));
    if !(1..=MAX_ENDORSER_PAGE).contains(&limit) {
        return Err(ApiError::Validation {
            field: "limit",
            message: format!("limit must be between 1 and {MAX_ENDORSER_PAGE}"),
        });
    }
    let after = match query.cursor.as_deref() {
        None => None,
        Some(token) => {
            let (cursor_revision, position) =
                open_endorser_cursor(token).ok_or_else(|| token_refused("cursor"))?;
            if cursor_revision != revision.as_uuid() {
                return Err(token_refused("cursor"));
            }
            Some(position)
        }
    };
    match state
        .vink
        .store()
        .public_endorsers(work, revision, after, limit)
        .await?
    {
        EndorserRead::NotFound => Err(ApiError::NotFound),
        EndorserRead::RevisionChanged(published) => Err(ApiError::RevisionChanged { published }),
        EndorserRead::Page(page) => conditional::respond(
            &headers,
            &EndorserList {
                work_id: work.as_uuid(),
                revision_id: revision.as_uuid(),
                total_count: page.total_count,
                items: page
                    .items
                    .iter()
                    .map(|author| EndorserItem {
                        author: AuthorProjection::from(author),
                    })
                    .collect(),
                next_cursor: page
                    .next
                    .map(|position| endorser_cursor(revision, position)),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(actor: u128, key: u8) -> PrivateSession {
        PrivateSession {
            actor: PrincipalId::from_uuid(Uuid::from_u128(actor)),
            digest: [key; 32],
        }
    }

    /// #348 AC-06: the Web-independent fixture is exactly what the Gateway serializes and accepts:
    /// an inbox item field by field, the Seen/Read request bodies, and the token shapes.
    #[test]
    fn the_gateway_fixture_is_what_the_runtime_serializes() {
        use crate::application::handle::AuthorRef;
        use crate::domain::notification_inbox::Excerpt;
        use time::format_description::well_known::Rfc3339;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../contracts/http/v1/fixtures/notifications-gateway.json"
        ))
        .unwrap();
        let expected = &fixture["response"]["items"][0];
        let uuid = |key: &str| Uuid::parse_str(expected["target"][key].as_str().unwrap()).unwrap();
        let at =
            |key: &str| OffsetDateTime::parse(expected[key].as_str().unwrap(), &Rfc3339).unwrap();
        let (linked, unavailable) = (
            PrincipalId::from_uuid(Uuid::from_u128(1)),
            PrincipalId::from_uuid(Uuid::from_u128(2)),
        );
        let (work, comment) = (
            WorkId::from_uuid(uuid("work_id")),
            CommentId::from_uuid(uuid("comment_id")),
        );
        let group = WireGroup {
            group_id: Uuid::parse_str(expected["group_id"].as_str().unwrap()).unwrap(),
            event_type: "comment.root.approved".into(),
            target: json!({"kind": "work_comment_thread", "work_id": work.to_string(),
                           "comment_id": comment.to_string(), "moderation_version": 2}),
            group_version: 12,
            signal_version: 12,
            unread: true,
            unseen: true,
            actor_count: 2,
            item_count: 0,
            actors_preview: vec![linked.as_uuid(), unavailable.as_uuid()],
            contributions: None,
            action_required: false,
            publication_epoch: Some(1),
            created_at: at("created_at"),
            last_signal_at: at("updated_at"),
            expires_at: at("expires_at"),
        };
        let shown = Shown {
            target: InboxTarget::CommentThread {
                work,
                comment,
                moderation_version: 2,
            },
            work_title: Some("Chiều qua bến cũ".into()),
            actors_preview: vec![linked, unavailable],
            actor_count: 2,
            item_count: 0,
            excerpt: Excerpt::of("Câu thơ thứ hai làm tôi nhớ bến sông quê"),
            outcome: None,
        };
        let identities = HashMap::from([(
            linked,
            AuthorAttribution::Linked(AuthorRef::new("Minh Khôi".into(), "minh-khoi-08".into())),
        )]);
        assert_eq!(
            item_json(&group, GroupKind::RootApproved, &shown, &identities),
            *expected
        );

        // Request bodies deserialize with the runtime's strict types.
        let seen: SnapshotRequest =
            serde_json::from_value(fixture["requests"]["seen"].clone()).unwrap();
        assert_eq!(seen.up_to_snapshot, fixture["response"]["inbox_snapshot"]);
        let read: ReadRequest =
            serde_json::from_value(fixture["requests"]["read"].clone()).unwrap();
        assert_eq!(
            (read.group_id, read.observed_group_version),
            (group.group_id, 12)
        );

        // Token shapes: `s1.<seq>.<32 hex>` and `n1.<seq>.<group hex>.<32 hex>`, like the fixture.
        let session = session(7, 3);
        let shape = |token: &str| {
            token
                .split('.')
                .map(|part| (part.len(), part.bytes().all(|b| b.is_ascii_hexdigit())))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            shape(&snapshot_token(&session, 45)),
            shape(fixture["response"]["inbox_snapshot"].as_str().unwrap())
        );
        assert_eq!(
            shape(&cursor_token(&session, 40, group.group_id)),
            shape(fixture["response"]["next_cursor"].as_str().unwrap())
        );
        assert_eq!(fixture["response"]["continuation_reason"], "more_items");
    }

    #[test]
    fn snapshot_and_cursor_tokens_belong_to_one_session_and_principal() {
        let a = session(1, 7);
        let token = snapshot_token(&a, 42);
        assert_eq!(open_snapshot(&a, &token), Some(42));
        // Another session of the same principal, another principal, another purpose: refused.
        assert_eq!(open_snapshot(&session(1, 8), &token), None);
        assert_eq!(open_snapshot(&session(2, 7), &token), None);
        assert_eq!(open_snapshot(&a, &token.replace("s1.42", "s1.43")), None);
        let group = Uuid::from_u128(0x019a_9b20_1111_7000_8000_0000_0000_0002);
        let cursor = cursor_token(&a, 41, group);
        assert_eq!(
            open_cursor(&a, &cursor).as_deref(),
            Some("c41.019a9b20-1111-7000-8000-000000000002")
        );
        assert_eq!(open_cursor(&session(2, 7), &cursor), None);
        assert_eq!(open_snapshot(&a, &cursor), None);
        for bad in ["", "s42", "c41.x", "n1.41", "s1.42.zz"] {
            assert_eq!(open_snapshot(&a, bad), None, "{bad}");
            assert_eq!(open_cursor(&a, bad), None, "{bad}");
        }
    }

    #[test]
    fn endorser_cursors_name_their_revision() {
        let revision = RevisionId::from_uuid(Uuid::from_u128(5));
        let position = Uuid::from_u128(9);
        let token = endorser_cursor(revision, position);
        assert_eq!(
            open_endorser_cursor(&token),
            Some((revision.as_uuid(), position))
        );
        assert_eq!(open_endorser_cursor("e1.x.y"), None);
    }

    #[test]
    fn a_stale_count_is_served_only_within_its_bound() {
        let cache = BadgeCache::default();
        let viewer = PrincipalId::from_uuid(Uuid::from_u128(3));
        assert!(cache.recall(viewer).is_none());
        cache.remember(
            viewer,
            Known {
                count: 4,
                lower_bound: false,
                snapshot: 9,
                at: Instant::now(),
            },
        );
        assert_eq!(cache.recall(viewer).map(|k| k.count), Some(4));
        cache.remember(
            viewer,
            Known {
                count: 4,
                lower_bound: false,
                snapshot: 9,
                at: Instant::now()
                    .checked_sub(MAX_STALENESS + Duration::from_secs(1))
                    .unwrap_or_else(Instant::now),
            },
        );
        assert!(
            cache.recall(viewer).is_none(),
            "older than the bound: 503, never a number"
        );
    }
}
