//! Durable delivery of one-time codes: the transactional outbox (`mail_deliveries`), its pure
//! retry policy and the worker that `vink-identity serve` runs beside the contract.
//!
//! **Consistency.** [`enqueue`] runs inside the transaction that inserts the challenge, so a
//! committed challenge always has a delivery intent and a rolled-back one never mails a code. No
//! network call happens inside a transaction: the worker claims due rows (`FOR UPDATE SKIP
//! LOCKED`, so any number of Identity processes share the queue without sending one claim twice
//! at the same time), commits the claim with a lease, sends, then records the outcome only if it
//! still holds the lease token.
//!
//! **Guarantee: at least once, never exactly once.** A crash after the relay accepted a message
//! but before the outcome commits, or a send that outlives its lease, leads to one more attempt
//! of the same code under the same Message-ID. That is harmless: the code still needs the secret
//! that only the requesting client holds. A message the relay accepted can still be lost later
//! (bounce, spam folder); bounce handling is not implemented.
//!
//! **Bounds.** Attempts are capped ([`DeliveryPolicy::max_attempts`]) with exponential backoff and
//! jitter; a `5xx` refusal ends the delivery at once; a code whose challenge expired or was used
//! is cancelled instead of sent. Every terminal state clears the stored code in the same
//! statement.

use std::sync::Arc;
use std::time::Duration;

use sqlx::{PgPool, Postgres, Row, Transaction};
use time::OffsetDateTime;
use tokio::sync::{Notify, watch};
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::email_code::{Code, EmailAddress, random};
use crate::mail::{Failure, Message, Purpose, Transport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryPolicy {
    /// Sends of one code, the first included.
    pub max_attempts: i32,
    pub base_backoff: Duration,
    pub max_backoff: Duration,
    /// How long a claim is exclusive. Longer than one send's bound, so a live worker never loses
    /// its claim to another; after a crash the row is due again when the lease ends.
    pub lease: Duration,
    /// Rows claimed together and sent concurrently.
    pub batch: i64,
    /// Upper bound on how long a due row waits when no wake-up arrives (another process
    /// enqueued it, or a retry came due).
    pub poll_interval: Duration,
}

impl Default for DeliveryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_backoff: Duration::from_secs(5),
            max_backoff: Duration::from_secs(60),
            lease: Duration::from_secs(45),
            batch: 16,
            poll_interval: Duration::from_secs(2),
        }
    }
}

/// What to do with a claimed row before sending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    Send,
    /// The challenge was used or expired: the code is worthless, never send it.
    Cancel,
}

pub fn decide_claim(consumed: bool, expired: bool) -> Claim {
    if consumed || expired {
        Claim::Cancel
    } else {
        Claim::Send
    }
}

/// What to record after one send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    Retry { after: Duration },
    Failed,
}

/// `attempts` counts this send. Pure: `jitter` comes from the caller.
pub fn decide_outcome(
    attempts: i32,
    result: Result<(), Failure>,
    policy: &DeliveryPolicy,
    jitter: Duration,
) -> Outcome {
    match result {
        Ok(()) => Outcome::Sent,
        Err(failure) if failure.is_permanent() => Outcome::Failed,
        Err(_) if attempts >= policy.max_attempts => Outcome::Failed,
        Err(_) => Outcome::Retry {
            after: backoff(attempts, policy) + jitter.min(policy.base_backoff),
        },
    }
}

/// `base · 2^(attempts−1)`, capped at `max_backoff`.
pub fn backoff(attempts: i32, policy: &DeliveryPolicy) -> Duration {
    let exponent = u32::try_from(attempts.saturating_sub(1))
        .unwrap_or(0)
        .min(16);
    policy
        .base_backoff
        .saturating_mul(1u32 << exponent)
        .min(policy.max_backoff)
}

/// Records the delivery intent of a challenge in the caller's transaction.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    challenge: Uuid,
    purpose: Purpose,
    recipient: &EmailAddress,
    code: &Code,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO mail_deliveries (challenge_id, purpose, recipient, code)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(challenge)
    .bind(purpose.label())
    .bind(recipient.as_str())
    .bind(code.expose())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A row this worker holds under `token`.
#[derive(Debug)]
pub struct Claimed {
    pub token: Uuid,
    pub attempts: i32,
    pub claim: Claim,
    pub message: Option<Message>,
    pub challenge: Uuid,
}

/// Claims up to `policy.batch` due rows and commits the claim (attempt counted, lease set).
pub async fn claim(pool: &PgPool, policy: &DeliveryPolicy) -> Result<Vec<Claimed>, sqlx::Error> {
    let token = Uuid::now_v7();
    let rows = sqlx::query(
        "WITH due AS (
             SELECT challenge_id FROM mail_deliveries
              WHERE state = 'pending' AND next_attempt_at <= now()
              ORDER BY next_attempt_at
              LIMIT $1
                FOR UPDATE SKIP LOCKED)
         UPDATE mail_deliveries d
            SET attempts = d.attempts + 1,
                next_attempt_at = now() + make_interval(secs => $2),
                lease_token = $3
           FROM due, email_challenges c
          WHERE d.challenge_id = due.challenge_id AND c.id = d.challenge_id
      RETURNING d.challenge_id, d.purpose, d.recipient, d.code, d.attempts, c.expires_at,
                c.consumed_at IS NOT NULL AS consumed, c.expires_at <= now() AS expired",
    )
    .bind(policy.batch)
    .bind(policy.lease.as_secs_f64())
    .bind(token)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            let challenge: Uuid = row.try_get("challenge_id")?;
            let claim = decide_claim(row.try_get("consumed")?, row.try_get("expired")?);
            let purpose: String = row.try_get("purpose")?;
            let recipient: String = row.try_get("recipient")?;
            let code: Option<String> = row.try_get("code")?;
            let expires_at: OffsetDateTime = row.try_get("expires_at")?;
            // A row the schema allows but this version cannot read is failed, never guessed.
            let message = match (
                Purpose::from_label(&purpose),
                EmailAddress::parse(&recipient),
                code.as_deref().and_then(Code::parse),
            ) {
                (Some(purpose), Some(to), Some(code)) => Some(Message {
                    delivery: challenge,
                    to,
                    code,
                    purpose,
                    expires_at,
                }),
                _ => None,
            };
            Ok(Claimed {
                token,
                attempts: row.try_get("attempts")?,
                claim,
                message,
                challenge,
            })
        })
        .collect()
}

/// Terminal states. Each clears the code and the lease in the same statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Final {
    Sent,
    Failed(Option<Failure>),
    Cancelled,
}

/// `false`: the lease was lost (another claim owns the row now); nothing was written.
pub async fn finish(pool: &PgPool, claimed: &Claimed, outcome: Final) -> Result<bool, sqlx::Error> {
    let (state, error, status) = match outcome {
        Final::Sent => ("sent", None, None),
        Final::Failed(failure) => (
            "failed",
            Some(failure.map_or("permanent", Failure::label)),
            failure.and_then(Failure::status),
        ),
        Final::Cancelled => ("cancelled", Some("expired"), None),
    };
    let updated = sqlx::query(
        "UPDATE mail_deliveries
            SET state = $3, code = NULL, lease_token = NULL, finished_at = now(),
                last_error = COALESCE($4, last_error), last_status = COALESCE($5, last_status)
          WHERE challenge_id = $1 AND lease_token = $2 AND state = 'pending'",
    )
    .bind(claimed.challenge)
    .bind(claimed.token)
    .bind(state)
    .bind(error)
    .bind(status.map(|s| s as i16))
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

pub async fn reschedule(
    pool: &PgPool,
    claimed: &Claimed,
    after: Duration,
    failure: Failure,
) -> Result<bool, sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE mail_deliveries
            SET next_attempt_at = now() + make_interval(secs => $3), lease_token = NULL,
                last_error = $4, last_status = $5
          WHERE challenge_id = $1 AND lease_token = $2 AND state = 'pending'",
    )
    .bind(claimed.challenge)
    .bind(claimed.token)
    .bind(after.as_secs_f64())
    .bind(failure.label())
    .bind(failure.status().map(|s| s as i16))
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// Queue health for readiness and operators. Counts only; no address or code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backlog {
    pub pending: i64,
    pub oldest_pending_seconds: Option<i64>,
    pub failed_last_hour: i64,
}

pub async fn backlog(pool: &PgPool) -> Result<Backlog, sqlx::Error> {
    let row = sqlx::query(
        "SELECT count(*) FILTER (WHERE state = 'pending') AS pending,
                EXTRACT(EPOCH FROM now() - min(created_at) FILTER (WHERE state = 'pending'))::bigint
                    AS oldest,
                count(*) FILTER (WHERE state = 'failed' AND finished_at > now() - interval '1 hour')
                    AS failed
           FROM mail_deliveries",
    )
    .fetch_one(pool)
    .await?;
    Ok(Backlog {
        pending: row.try_get("pending")?,
        oldest_pending_seconds: row.try_get("oldest")?,
        failed_last_hour: row.try_get("failed")?,
    })
}

/// Sends due codes until `shutdown` turns true. Owns no request path: a slow or failing relay
/// never delays a contract call.
pub struct Worker {
    pool: PgPool,
    transport: Transport,
    policy: DeliveryPolicy,
    wake: Arc<Notify>,
}

impl Worker {
    pub fn new(
        pool: PgPool,
        transport: Transport,
        policy: DeliveryPolicy,
        wake: Arc<Notify>,
    ) -> Self {
        Self {
            pool,
            transport,
            policy,
            wake,
        }
    }

    /// Claims and sends one batch. Returns how many rows were claimed.
    pub async fn run_once(&self) -> Result<usize, sqlx::Error> {
        let batch = claim(&self.pool, &self.policy).await?;
        let claimed = batch.len();
        let mut sends = JoinSet::new();
        for row in batch {
            let pool = self.pool.clone();
            let transport = self.transport.clone();
            let policy = self.policy;
            sends.spawn(async move { deliver(&pool, &transport, &policy, row).await });
        }
        while let Some(joined) = sends.join_next().await {
            if let Ok(Err(_)) = joined {
                // The outcome could not be recorded; the lease expires and the row is retried.
                tracing::warn!("delivery outcome not recorded; the code will be retried");
            }
        }
        Ok(claimed)
    }

    /// The loop of `serve`. On shutdown it stops claiming, lets the batch in flight finish
    /// (bounded by the transport timeout) and returns; rows it did not finish are retried by
    /// any process after their lease.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        tracing::info!(
            transport = self.transport.label(),
            "mail delivery worker started"
        );
        let mut idle_backoff = self.policy.poll_interval;
        loop {
            if *shutdown.borrow() {
                break;
            }
            match self.run_once().await {
                Ok(claimed) if claimed > 0 => {
                    idle_backoff = self.policy.poll_interval;
                    continue;
                }
                Ok(_) => idle_backoff = self.policy.poll_interval,
                Err(_) => {
                    tracing::warn!("mail delivery worker cannot reach the database");
                    idle_backoff = (idle_backoff * 2).min(Duration::from_secs(30));
                }
            }
            tokio::select! {
                () = self.wake.notified() => {}
                () = tokio::time::sleep(idle_backoff) => {}
                _ = shutdown.changed() => {}
            }
        }
        tracing::info!("mail delivery worker stopped");
    }
}

fn jitter(policy: &DeliveryPolicy) -> Duration {
    let mut raw = [0u8; 4];
    if random(&mut raw).is_err() {
        return Duration::ZERO;
    }
    let millis = policy.base_backoff.as_millis().max(1) as u64;
    Duration::from_millis(u64::from(u32::from_le_bytes(raw)) % millis)
}

async fn deliver(
    pool: &PgPool,
    transport: &Transport,
    policy: &DeliveryPolicy,
    row: Claimed,
) -> Result<(), sqlx::Error> {
    let delivery = row.challenge.simple();
    let Some(message) = row.message.as_ref() else {
        tracing::error!(%delivery, "unreadable delivery row failed without sending");
        finish(pool, &row, Final::Failed(None)).await?;
        return Ok(());
    };
    if row.claim == Claim::Cancel {
        finish(pool, &row, Final::Cancelled).await?;
        tracing::info!(%delivery, "code not sent: its challenge is used or expired");
        return Ok(());
    }
    let result = transport.send(message).await;
    let recorded = match decide_outcome(row.attempts, result, policy, jitter(policy)) {
        Outcome::Sent => {
            let kept = finish(pool, &row, Final::Sent).await?;
            tracing::info!(%delivery, attempts = row.attempts, transport = transport.label(), "code delivered");
            kept
        }
        Outcome::Retry { after } => {
            let failure = result.err().unwrap_or(Failure::Transient { status: None });
            tracing::warn!(
                %delivery,
                attempts = row.attempts,
                failure = failure.label(),
                status = ?failure.status(),
                retry_in_seconds = after.as_secs(),
                "code delivery failed; retrying"
            );
            reschedule(pool, &row, after, failure).await?
        }
        Outcome::Failed => {
            let failure = result.err();
            tracing::error!(
                %delivery,
                attempts = row.attempts,
                failure = failure.map_or("unknown", Failure::label),
                status = ?failure.and_then(Failure::status),
                "code delivery failed permanently"
            );
            finish(pool, &row, Final::Failed(failure)).await?
        }
    };
    if !recorded {
        tracing::warn!(%delivery, "delivery lease was lost; another claim owns the outcome");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_or_expired_codes_are_never_sent() {
        assert_eq!(decide_claim(false, false), Claim::Send);
        assert_eq!(decide_claim(true, false), Claim::Cancel);
        assert_eq!(decide_claim(false, true), Claim::Cancel);
        assert_eq!(decide_claim(true, true), Claim::Cancel);
    }

    /// Every attempt count up to past the bound × every failure category: the delivery ends on
    /// success, on a permanent refusal or on the last attempt, and otherwise waits a bounded,
    /// non-decreasing time.
    #[test]
    fn outcomes_are_bounded_and_backoff_grows_to_its_cap() {
        let policy = DeliveryPolicy::default();
        let failures = [
            Failure::Timeout,
            Failure::Connect,
            Failure::Tls,
            Failure::Configuration,
            Failure::Transient { status: Some(451) },
            Failure::Permanent { status: Some(550) },
        ];
        let mut previous = Duration::ZERO;
        for attempts in 1..=policy.max_attempts + 2 {
            assert_eq!(
                decide_outcome(attempts, Ok(()), &policy, Duration::ZERO),
                Outcome::Sent
            );
            let wait = backoff(attempts, &policy);
            assert!(wait >= previous && wait <= policy.max_backoff, "{attempts}");
            previous = wait;
            for failure in failures {
                let outcome = decide_outcome(attempts, Err(failure), &policy, Duration::MAX);
                match outcome {
                    Outcome::Failed => {
                        assert!(failure.is_permanent() || attempts >= policy.max_attempts)
                    }
                    Outcome::Retry { after } => {
                        assert!(!failure.is_permanent() && attempts < policy.max_attempts);
                        assert!(after <= policy.max_backoff + policy.base_backoff);
                        assert!(after >= wait);
                    }
                    Outcome::Sent => panic!("a failure is never sent"),
                }
            }
        }
        assert_eq!(backoff(1, &policy), Duration::from_secs(5));
        assert_eq!(backoff(2, &policy), Duration::from_secs(10));
        assert_eq!(backoff(10, &policy), Duration::from_secs(60));
        assert_eq!(backoff(i32::MAX, &policy), Duration::from_secs(60));
        // The whole retry schedule fits well inside a code's 10-minute lifetime.
        let total: Duration = (1..policy.max_attempts)
            .map(|n| backoff(n, &policy) + policy.base_backoff)
            .sum();
        assert!(total < Duration::from_secs(300), "{total:?}");
    }
}
