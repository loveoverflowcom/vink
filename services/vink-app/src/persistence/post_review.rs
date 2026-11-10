//! Operator producer/result adapter and network-free post-review effect. Lock order:
//! request key advisory -> comment -> NEW ticket/event/delivery; result key advisory ->
//! delivery -> ticket -> result; worker delivery -> ticket -> comment -> receipt. Existing
//! human/remove paths never lock a delivery or ticket; result never locks a comment.
use sqlx::{PgConnection, PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::discussion::state;
use super::outbox::{BoxFuture, EffectHandler, OutboxEvent};
use super::postgres::classify;
use crate::application::ports::{PostReviewStore, StoreError};
use crate::application::post_review::{RequestReview, ReviewAuthority, ReviewResult, ReviewWrite};
use crate::domain::discussion::admission::PostReviewFinding;
use crate::domain::discussion::lifecycle::ModerationVersion;
use crate::domain::discussion::outbox::{DeadReason, HandlerResult, Transient};
use crate::domain::discussion::post_review::{
    self, CONSUMER, EVENT, POLICY, ReviewBinding, ReviewDecision, decide_review, payload_digest,
};
use crate::domain::discussion::submission::PayloadDigest;

pub fn finding_code(finding: PostReviewFinding) -> &'static str {
    match finding {
        PostReviewFinding::Clear => "clear",
        PostReviewFinding::Flagged => "flagged",
        PostReviewFinding::Unresolved => "unresolved",
    }
}

fn finding(raw: &str) -> Option<PostReviewFinding> {
    match raw {
        "clear" => Some(PostReviewFinding::Clear),
        "flagged" => Some(PostReviewFinding::Flagged),
        "unresolved" => Some(PostReviewFinding::Unresolved),
        _ => None,
    }
}

/// Serializes concurrent identical command keys, including cross-ticket result key reuse. The
/// namespace is shared by request/result on purpose. No human path uses these locks.
async fn command_lock(tx: &mut PgConnection, key: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 27115))")
        .bind(key.to_string())
        .execute(tx)
        .await?;
    Ok(())
}

impl PostReviewStore for PgStore {
    async fn request_review(
        &self,
        _authority: ReviewAuthority,
        enabled: bool,
        request: &RequestReview,
    ) -> Result<ReviewWrite, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        sqlx::query("SET LOCAL lock_timeout = '2s'")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        sqlx::query("SET LOCAL statement_timeout = '5s'")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        command_lock(&mut tx, request.key).await.map_err(classify)?;
        let existing: Option<(Uuid, Uuid, i64, String)> = sqlx::query_as("SELECT id, comment_id, expected_version, note FROM comment_post_reviews WHERE request_key = $1")
            .bind(request.key).fetch_optional(&mut *tx).await.map_err(classify)?;
        if let Some((id, comment, version, note)) = existing {
            return Ok(
                if comment == request.comment.as_uuid()
                    && version == request.expected.get()
                    && note == request.note.as_str()
                {
                    ReviewWrite::Replayed(id)
                } else {
                    ReviewWrite::Conflict
                },
            );
        }
        if !enabled {
            return Ok(ReviewWrite::Disabled);
        }
        let row: Option<(String, Option<String>, i64, Option<String>)> = sqlx::query_as("SELECT review, removal, moderation_version, body FROM comments WHERE id = $1 FOR UPDATE")
            .bind(request.comment.as_uuid()).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some((review, removal, version, body)) = row else {
            return Ok(ReviewWrite::NotFound);
        };
        let current = state(&review, removal.as_deref(), version)?;
        if !current.is_public() || current.version() != request.expected {
            return Ok(ReviewWrite::Refused);
        }
        let occupied: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM comment_post_reviews WHERE comment_id = $1 AND expected_version = $2)")
            .bind(request.comment.as_uuid()).bind(version).fetch_one(&mut *tx).await.map_err(classify)?;
        if occupied {
            return Ok(ReviewWrite::Conflict);
        }
        let body =
            body.ok_or_else(|| StoreError::Integrity("public comment without text".into()))?;
        let id = Uuid::now_v7();
        sqlx::query(
            "WITH moment AS MATERIALIZED (SELECT clock_timestamp() AS at)
            INSERT INTO comment_post_reviews (id, request_key, comment_id, expected_version,
                payload_digest, origin, policy, note, created_at, deadline_at)
            SELECT $1,$2,$3,$4,$5,'operator_rereview',$6,$7,at,at + make_interval(secs => $8)
            FROM moment",
        )
        .bind(id)
        .bind(request.key)
        .bind(request.comment.as_uuid())
        .bind(version)
        .bind(payload_digest(&body).as_bytes().as_slice())
        .bind(POLICY)
        .bind(request.note.as_str())
        .bind(post_review::DEADLINE_SECONDS as f64)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        sqlx::query("INSERT INTO discussion_events (id, event_type, schema_version, comment_id, source_version) VALUES ($1,$2,1,$3,$4)")
            .bind(id).bind(EVENT).bind(request.comment.as_uuid()).bind(version).execute(&mut *tx).await.map_err(classify)?;
        sqlx::query("INSERT INTO discussion_deliveries (event_id, consumer) VALUES ($1,$2)")
            .bind(id)
            .bind(CONSUMER)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(ReviewWrite::Accepted(id))
    }

    async fn record_review_result(
        &self,
        _authority: ReviewAuthority,
        result: &ReviewResult,
    ) -> Result<ReviewWrite, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        sqlx::query("SET LOCAL lock_timeout = '2s'")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        sqlx::query("SET LOCAL statement_timeout = '5s'")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        command_lock(&mut tx, result.key).await.map_err(classify)?;
        let old: Option<(Uuid, String, String)> = sqlx::query_as("SELECT ticket_id, finding, note FROM comment_post_review_results WHERE command_key = $1")
            .bind(result.key).fetch_optional(&mut *tx).await.map_err(classify)?;
        if let Some((ticket, found, note)) = old {
            return Ok(
                if ticket == result.ticket
                    && found == finding_code(result.finding)
                    && note == result.note.as_str()
                {
                    ReviewWrite::Replayed(ticket)
                } else {
                    ReviewWrite::Conflict
                },
            );
        }
        // Same lock order as complete/requeue. Result submission never steals an active lease.
        let delivery: Option<String> = sqlx::query_scalar("SELECT state FROM discussion_deliveries WHERE event_id = $1 AND consumer = $2 FOR UPDATE")
            .bind(result.ticket).bind(CONSUMER).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some(delivery) = delivery else {
            return Ok(ReviewWrite::NotFound);
        };
        let valid: Option<bool> = sqlx::query_scalar("SELECT deadline_at > clock_timestamp() AND NOT EXISTS (SELECT 1 FROM comment_post_review_receipts WHERE ticket_id = $1) AND NOT EXISTS (SELECT 1 FROM comment_post_review_quarantines WHERE event_id = $1) FROM comment_post_reviews WHERE id = $1 FOR UPDATE")
            .bind(result.ticket).fetch_optional(&mut *tx).await.map_err(classify)?;
        if valid != Some(true) || !matches!(delivery.as_str(), "ready" | "leased") {
            return Ok(ReviewWrite::Refused);
        }
        let inserted = sqlx::query("INSERT INTO comment_post_review_results (ticket_id, command_key, comment_id, expected_version, payload_digest, origin, policy, finding, note) SELECT id,$2,comment_id,expected_version,payload_digest,origin,policy,$3,$4 FROM comment_post_reviews WHERE id = $1 ON CONFLICT (ticket_id) DO NOTHING")
            .bind(result.ticket).bind(result.key).bind(finding_code(result.finding)).bind(result.note.as_str()).execute(&mut *tx).await.map_err(classify)?;
        if inserted.rows_affected() != 1 {
            return Ok(ReviewWrite::Conflict);
        }
        sqlx::query("UPDATE discussion_deliveries SET available_at = now() WHERE event_id = $1 AND consumer = $2 AND state = 'ready'")
            .bind(result.ticket).bind(CONSUMER).execute(&mut *tx).await.map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(ReviewWrite::Accepted(result.ticket))
    }
}

async fn quarantine(
    tx: &mut PgConnection,
    event: Uuid,
    reason: &str,
) -> Result<HandlerResult, sqlx::Error> {
    sqlx::query("INSERT INTO comment_post_review_quarantines (event_id, reason) VALUES ($1,$2) ON CONFLICT DO NOTHING")
        .bind(event).bind(reason).execute(tx).await?;
    Ok(HandlerResult::Fail(DeadReason::InvalidPayload))
}

pub struct OperatorPostReview;
impl EffectHandler for OperatorPostReview {
    fn consumer(&self) -> &str {
        CONSUMER
    }
    fn apply<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        event: &'a OutboxEvent,
    ) -> BoxFuture<'a, Result<HandlerResult, sqlx::Error>> {
        Box::pin(apply(tx, event, false))
    }
}

/// Caller MUST already hold and fence the delivery. Used at normal completion AND at every
/// dead transition (claim, complete, record_failure). Errors roll back the delivery too.
pub(super) async fn apply(
    tx: &mut PgConnection,
    event: &OutboxEvent,
    exhausted: bool,
) -> Result<HandlerResult, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM comment_post_reviews WHERE id = $1 FOR UPDATE")
        .bind(event.id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        return quarantine(tx, event.id, "missing_ticket").await;
    };
    let comment: Uuid = row.try_get("comment_id")?;
    let version: i64 = row.try_get("expected_version")?;
    let origin: String = row.try_get("origin")?;
    let policy: String = row.try_get("policy")?;
    if event.event_type != EVENT || event.schema_version != 1 {
        return quarantine(tx, event.id, "unsupported_event").await;
    }
    if comment != event.comment_id
        || version != event.source_version
        || origin != "operator_rereview"
        || policy != POLICY
    {
        return quarantine(tx, event.id, "invalid_binding").await;
    }
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM comment_post_review_receipts WHERE ticket_id = $1)",
    )
    .bind(event.id)
    .fetch_one(&mut *tx)
    .await?;
    if already {
        return Ok(HandlerResult::AlreadyApplied);
    }
    let raw_digest: Vec<u8> = row.try_get("payload_digest")?;
    let (Some(payload), Some(version)) = (
        PayloadDigest::from_bytes(&raw_digest),
        ModerationVersion::new(version),
    ) else {
        return quarantine(tx, event.id, "invalid_binding").await;
    };
    let target = sqlx::query(
        "SELECT review, removal, moderation_version, body FROM comments WHERE id = $1 FOR UPDATE",
    )
    .bind(comment)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(target) = target else {
        return quarantine(tx, event.id, "invalid_binding").await;
    };
    let current = state(
        target.try_get("review")?,
        target.try_get::<Option<&str>, _>("removal")?,
        target.try_get("moderation_version")?,
    )
    .map_err(|_| sqlx::Error::Protocol("invalid post-review state".into()))?;
    let body: Option<String> = target.try_get("body")?;
    // Author removal purges text. No content decision is possible or needed after removal.
    let current_digest = if current.is_present() {
        let Some(body) = body else {
            return quarantine(tx, event.id, "invalid_binding").await;
        };
        payload_digest(&body)
    } else {
        payload
    };
    let raw_result: Option<String> =
        sqlx::query_scalar("SELECT finding FROM comment_post_review_results WHERE ticket_id = $1")
            .bind(event.id)
            .fetch_optional(&mut *tx)
            .await?;
    let result = raw_result.as_deref().and_then(finding);
    if raw_result.is_some() && result.is_none() {
        return quarantine(tx, event.id, "invalid_binding").await;
    }
    let now: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    let deadline: OffsetDateTime = row.try_get("deadline_at")?;
    let micros =
        |at: OffsetDateTime| i64::try_from(at.unix_timestamp_nanos() / 1000).unwrap_or(i64::MAX);
    let decided = decide_review(
        ReviewBinding {
            version,
            payload,
            deadline_micros: micros(deadline),
        },
        current,
        current_digest,
        result,
        micros(now),
        exhausted,
    )
    .map_err(|_| sqlx::Error::Protocol("post-review version exhausted".into()))?;
    let (outcome, next) = match decided {
        ReviewDecision::AwaitResult => return Ok(HandlerResult::Retry(Transient::Timeout)),
        ReviewDecision::InvalidBinding => return quarantine(tx, event.id, "invalid_binding").await,
        ReviewDecision::Keep => ("kept", current),
        ReviewDecision::Hold(next) => {
            sqlx::query("UPDATE comments SET review = 'pending', moderation_version = $2, updated_at = now() WHERE id = $1")
                .bind(comment).bind(next.version().get()).execute(&mut *tx).await?;
            ("held", next)
        }
        ReviewDecision::Superseded => ("superseded", current),
        ReviewDecision::Removed => ("removed", current),
    };
    let effective_finding = if exhausted || now >= deadline {
        Some(PostReviewFinding::Unresolved)
    } else {
        result
    };
    sqlx::query("INSERT INTO comment_post_review_receipts (ticket_id, outcome, observed_version, resulting_version, finding) VALUES ($1,$2,$3,$4,$5)")
        .bind(event.id).bind(outcome).bind(current.version().get()).bind(next.version().get()).bind(effective_finding.map(finding_code)).execute(tx).await?;
    Ok(HandlerResult::Applied)
}

/// Bounded status listing; never returns comment text, internal reason, email or provider data.
pub async fn status(pool: &PgPool) -> Result<Vec<(String, i64, Option<i64>)>, StoreError> {
    sqlx::query_as(
        "WITH states AS (
            SELECT CASE
                WHEN r.outcome IS NOT NULL THEN r.outcome
                WHEN q.event_id IS NOT NULL THEN 'quarantined'
                WHEN t.deadline_at <= now() OR d.state = 'dead' OR d.attempts >= 8 THEN 'finalization_due'
                WHEN a.ticket_id IS NOT NULL THEN 'result_ready'
                ELSE 'awaiting_result' END AS state, t.created_at
            FROM comment_post_reviews t
            LEFT JOIN comment_post_review_receipts r ON r.ticket_id = t.id
            LEFT JOIN comment_post_review_quarantines q ON q.event_id = t.id
            LEFT JOIN comment_post_review_results a ON a.ticket_id = t.id
            LEFT JOIN discussion_deliveries d ON d.event_id = t.id AND d.consumer = 'operator_post_review.v1'
            UNION ALL
            SELECT 'quarantined', q.created_at FROM comment_post_review_quarantines q
            WHERE NOT EXISTS (SELECT 1 FROM comment_post_reviews t WHERE t.id = q.event_id)
            UNION ALL
            SELECT 'overdue', t.created_at FROM comment_post_reviews t
            WHERE t.deadline_at <= now() AND NOT EXISTS (
                SELECT 1 FROM comment_post_review_receipts r WHERE r.ticket_id = t.id)
        ) SELECT state, count(*), max(EXTRACT(EPOCH FROM now() - created_at)::bigint)
          FROM states GROUP BY state ORDER BY state",
    ).fetch_all(pool).await.map_err(classify)
}

#[derive(Debug, sqlx::FromRow)]
pub struct PendingReview {
    pub ticket: Uuid,
    pub comment: Uuid,
    pub version: i64,
    pub deadline: OffsetDateTime,
    pub overdue: bool,
}

pub async fn pending(pool: &PgPool, limit: i64) -> Result<Vec<PendingReview>, StoreError> {
    sqlx::query_as(
        "SELECT t.id AS ticket, t.comment_id AS comment, t.expected_version AS version,
                t.deadline_at AS deadline, t.deadline_at <= now() AS overdue
           FROM comment_post_reviews t
          WHERE NOT EXISTS (SELECT 1 FROM comment_post_review_receipts r WHERE r.ticket_id = t.id)
          ORDER BY t.created_at LIMIT $1",
    )
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await
    .map_err(classify)
}

pub async fn quarantines(pool: &PgPool, limit: i64) -> Result<Vec<(Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT q.event_id, q.reason FROM comment_post_review_quarantines q
          WHERE NOT EXISTS (SELECT 1 FROM comment_post_review_receipts r WHERE r.ticket_id = q.event_id)
          ORDER BY q.created_at LIMIT $1",
    )
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await
    .map_err(classify)
}
