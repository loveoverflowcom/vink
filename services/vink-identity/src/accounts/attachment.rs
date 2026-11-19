//! First verified address; never edits an existing address, password, account or session.

use sqlx::{Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{AccountStore, ChallengeOutcome, ChallengeScope, StoreError, lock_session};
use crate::contract::{ChallengeRejection, CredentialRejection, EmailAttachmentStatus};
use crate::email_code::{ChallengeHandle, Code, EmailAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentRejection {
    CredentialRejected,
    AccountDisabled,
    ContextChanged,
    NoPassword,
    AlreadyAttached,
    InvalidCredentials,
    RateLimited,
    ChallengeClosed,
    InvalidCode,
    AddressUnavailable,
}

type Decision<T> = Result<Result<T, AttachmentRejection>, StoreError>;

fn session_rejected(reason: CredentialRejection) -> AttachmentRejection {
    match reason {
        CredentialRejection::AccountDisabled => AttachmentRejection::AccountDisabled,
        _ => AttachmentRejection::CredentialRejected,
    }
}

async fn password_version(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
) -> Result<Option<i32>, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT version FROM account_passwords WHERE account_id = $1 FOR NO KEY UPDATE",
    )
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await?)
}

async fn has_email(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
) -> Result<bool, StoreError> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM account_emails WHERE account_id = $1)")
            .bind(account_id)
            .fetch_one(&mut **tx)
            .await?,
    )
}

async fn wall_clock(tx: &mut Transaction<'_, Postgres>) -> Result<OffsetDateTime, StoreError> {
    Ok(sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut **tx)
        .await?)
}

impl AccountStore {
    /// Actual binding projection, not a success receipt. No write or mail on readback.
    pub async fn email_attachment_status(
        &self,
        credential: &str,
    ) -> Decision<EmailAttachmentStatus> {
        let session = match self.verify(credential).await? {
            Ok(session) => session,
            Err(reason) => return Ok(Err(session_rejected(reason))),
        };
        let row = sqlx::query(
            "SELECT ap.account_id IS NOT NULL AS password_account, ae.address AS verified_email
               FROM sessions s JOIN accounts a ON a.id = s.account_id
               LEFT JOIN account_passwords ap ON ap.account_id = a.id
               LEFT JOIN account_emails ae ON ae.account_id = a.id
              WHERE s.id = $1 AND s.account_id = $2 AND s.revoked_at IS NULL
                AND s.expires_at > clock_timestamp() AND a.disabled_at IS NULL",
        )
        .bind(session.session_id)
        .bind(session.account_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(Err(AttachmentRejection::CredentialRejected));
        };
        Ok(Ok(EmailAttachmentStatus {
            session_id: session.session_id,
            password_account: row.try_get("password_account")?,
            verified_email: row.try_get("verified_email")?,
        }))
    }

    /// Reauthenticate against the signed-in account's current password; never look up by email
    /// or username to choose the account. Reserve the recheck before leaving the transaction,
    /// then perform bounded Argon2 without holding DB locks.
    pub async fn start_email_attachment(
        &self,
        credential: &str,
        expected_session: Uuid,
        address: &EmailAddress,
        raw_password: &str,
    ) -> Decision<ChallengeOutcome> {
        let session = match self.verify(credential).await? {
            Ok(session) => session,
            Err(reason) => return Ok(Err(session_rejected(reason))),
        };
        if session.session_id != expected_session {
            return Ok(Err(AttachmentRejection::ContextChanged));
        }
        let mut tx = self.pool.begin().await?;
        if let Err(reason) = lock_session(&mut tx, &session).await? {
            return Ok(Err(session_rejected(reason)));
        }
        if has_email(&mut tx, session.account_id).await? {
            return Ok(Err(AttachmentRejection::AlreadyAttached));
        }
        let row = sqlx::query(
            "SELECT password_hash, version FROM account_passwords WHERE account_id = $1",
        )
        .bind(session.account_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok(Err(AttachmentRejection::NoPassword));
        };
        let hash: String = row.try_get("password_hash")?;
        let version: i32 = row.try_get("version")?;
        let recent: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_attachment_reauth_attempts
              WHERE account_id = $1 AND created_at > clock_timestamp() - interval '15 minutes'",
        )
        .bind(session.account_id)
        .fetch_one(&mut *tx)
        .await?;
        if recent >= 10 {
            return Ok(Err(AttachmentRejection::RateLimited));
        }
        sqlx::query(
            "INSERT INTO email_attachment_reauth_attempts (id, account_id) VALUES ($1, $2)",
        )
        .bind(Uuid::now_v7())
        .bind(session.account_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        if !crate::password::verify_sign_in(Some(hash), raw_password)
            .await
            .map_err(|_| StoreError::Unavailable)?
        {
            return Ok(Err(AttachmentRejection::InvalidCredentials));
        }
        let mut tx = self.pool.begin().await?;
        let session_expiry = match lock_session(&mut tx, &session).await? {
            Ok(expiry) => expiry,
            Err(reason) => return Ok(Err(session_rejected(reason))),
        };
        if password_version(&mut tx, session.account_id).await? != Some(version) {
            return Ok(Err(AttachmentRejection::InvalidCredentials));
        }
        if has_email(&mut tx, session.account_id).await? {
            return Ok(Err(AttachmentRejection::AlreadyAttached));
        }
        // Same issuance shape and throttles for another account's address; no mail for it.
        // Completion also fences against address ownership changing after issuance.
        let deliver = !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM account_emails WHERE address = $1)",
        )
        .bind(address.as_str())
        .fetch_one(&mut *tx)
        .await?;
        let outcome = self
            .issue_challenge(
                &mut tx,
                address,
                ChallengeScope::AttachEmail {
                    account: session.account_id,
                    session: session.session_id,
                    version,
                },
                deliver,
            )
            .await?;
        if wall_clock(&mut tx).await? >= session_expiry {
            return Ok(Err(AttachmentRejection::CredentialRejected));
        }
        if let ChallengeOutcome::Issued { ref handle, .. } = outcome {
            // A successful resend replaces this session's prior requests only. If the issue
            // fails/throttles, the old request retains its own original validity.
            sqlx::query(
                "UPDATE email_challenges SET consumed_at = clock_timestamp()
                  WHERE purpose = 'attach_email' AND attachment_session_id = $1
                    AND consumed_at IS NULL AND id <> $2",
            )
            .bind(session.session_id)
            .bind(handle.id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.issued(&outcome);
        Ok(Ok(outcome))
    }

    pub async fn complete_email_attachment(
        &self,
        credential: &str,
        handle: &ChallengeHandle,
        code: &Code,
    ) -> Decision<()> {
        let session = match self.verify(credential).await? {
            Ok(session) => session,
            Err(reason) => return Ok(Err(session_rejected(reason))),
        };
        let mut tx = self.pool.begin().await?;
        let session_expiry = match lock_session(&mut tx, &session).await? {
            Ok(expiry) => expiry,
            Err(reason) => return Ok(Err(session_rejected(reason))),
        };
        let Some(version) = password_version(&mut tx, session.account_id).await? else {
            return Ok(Err(AttachmentRejection::NoPassword));
        };
        if has_email(&mut tx, session.account_id).await? {
            return Ok(Err(AttachmentRejection::AlreadyAttached));
        }
        let row = sqlx::query(
            "WITH locked AS MATERIALIZED (
                 SELECT address, code_hash, failed_attempts, expires_at,
                        consumed_at IS NOT NULL AS consumed
                   FROM email_challenges
                  WHERE id = $1 AND purpose = 'attach_email' AND account_id = $2
                    AND attachment_session_id = $3 AND attachment_password_version = $4
                    FOR UPDATE
             )
             SELECT *, expires_at <= clock_timestamp() AS expired,
                    EXISTS (SELECT 1 FROM mail_deliveries WHERE challenge_id = $1
                            AND purpose = 'attach_email') AS real_proof
             FROM locked",
        )
        .bind(handle.id)
        .bind(session.account_id)
        .bind(session.session_id)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await?;
        let closed = Ok(Err(AttachmentRejection::ChallengeClosed));
        let Some(row) = row else {
            return closed;
        };
        if wall_clock(&mut tx).await? >= session_expiry {
            return Ok(Err(AttachmentRejection::CredentialRejected));
        }
        if let Some(rejected) = self.attempt(&mut tx, handle, code, &row).await? {
            tx.commit().await?;
            return Ok(Err(match rejected {
                ChallengeRejection::InvalidCode => AttachmentRejection::InvalidCode,
                _ => AttachmentRejection::ChallengeClosed,
            }));
        }
        // Dummy proof is never eligible, even if a mailbox was released after challenge issuance.
        if !row.try_get::<bool, _>("real_proof")? {
            return closed;
        }
        let address: String = row.try_get("address")?;
        let inserted = sqlx::query(
            "INSERT INTO account_emails (account_id, address, verified_at)
             VALUES ($1, $2, clock_timestamp()) ON CONFLICT (address) DO NOTHING",
        )
        .bind(session.account_id)
        .bind(address)
        .execute(&mut *tx)
        .await?;
        // PostgreSQL's unique index can wait on another transaction. Recheck both deadlines
        // against the DB clock after the wait, and roll the insert and proof back together.
        let now = wall_clock(&mut tx).await?;
        if now >= session_expiry {
            return Ok(Err(AttachmentRejection::CredentialRejected));
        }
        if now >= row.try_get::<OffsetDateTime, _>("expires_at")? {
            return closed;
        }
        if inserted.rows_affected() != 1 {
            // A genuine, valid proof that lost the unique-address race cannot come back later.
            tx.commit().await?;
            return Ok(Err(AttachmentRejection::AddressUnavailable));
        }
        sqlx::query(
            "UPDATE email_challenges SET consumed_at = clock_timestamp()
              WHERE purpose = 'attach_email' AND account_id = $1 AND consumed_at IS NULL",
        )
        .bind(session.account_id)
        .execute(&mut *tx)
        .await?;
        let now = wall_clock(&mut tx).await?;
        if now >= session_expiry {
            return Ok(Err(AttachmentRejection::CredentialRejected));
        }
        if now >= row.try_get::<OffsetDateTime, _>("expires_at")? {
            return closed;
        }
        tx.commit().await?;
        Ok(Ok(()))
    }
}
