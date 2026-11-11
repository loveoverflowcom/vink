//! PostgreSQL side of the email-code authority: challenges, accounts, verified addresses and
//! sessions in the Identity database. Every decision that must be atomic (consume a challenge,
//! provision an account, issue a session; change an address and revoke the other sessions;
//! disable an account and revoke all its sessions) runs in one transaction with the rows locked.
//!
//! Time comparisons use the database clock (`now()`), so the server and the stored rows agree.
//!
//! Issuing a challenge writes its delivery intent in the same transaction
//! ([`crate::delivery::enqueue`]); no mail is sent while a transaction is open.

use std::sync::Arc;

use sqlx::{PgPool, Postgres, Row, Transaction};
use time::OffsetDateTime;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::contract::{ChallengeRejection, CredentialRejection};
use crate::delivery;
use crate::email_code::{
    Attempt, ChallengeHandle, ChallengeState, Code, EmailAddress, Policy, RandomUnavailable,
    SessionCredential, decide_attempt,
};
use crate::mail::Purpose;
use crate::password::{Password, Username};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRegistrationError {
    UsernameTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordSignInRejection {
    InvalidCredentials,
    AccountDisabled,
    RateLimited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The database, the random generator or mail delivery failed; nothing was decided.
    Unavailable,
}

impl From<sqlx::Error> for StoreError {
    fn from(_: sqlx::Error) -> Self {
        // The error may quote parameters; it is not logged or returned.
        StoreError::Unavailable
    }
}

impl From<RandomUnavailable> for StoreError {
    fn from(_: RandomUnavailable) -> Self {
        StoreError::Unavailable
    }
}

#[derive(Debug)]
pub enum ChallengeOutcome {
    Issued {
        handle: ChallengeHandle,
        expires_at: OffsetDateTime,
    },
    RateLimited(Throttle),
}

/// Which ceiling refused a challenge. Logged by label; the contract answer is the same for all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Throttle {
    /// Codes per address, any purpose.
    Address,
    /// Verified email changes started per account.
    Account,
    /// Codes across all addresses: caps mail volume when requests rotate addresses and networks.
    Global,
}

impl Throttle {
    pub fn label(self) -> &'static str {
        match self {
            Throttle::Address => "address",
            Throttle::Account => "account",
            Throttle::Global => "global",
        }
    }
}

#[derive(Debug)]
pub enum EmailChangeStart {
    Issued(ChallengeOutcome),
    /// The address is already this account's own. An address of ANOTHER account is not reported:
    /// the answer is `Issued` and no mail is sent, so the operation cannot probe who is registered.
    AddressUnavailable,
    CredentialRejected,
}

/// A session verified now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub authenticated_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct NewSession {
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub credential: SessionCredential,
    pub authenticated_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug)]
pub enum SignInOutcome {
    Authenticated(NewSession),
    Rejected(ChallengeRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailChangeOutcome {
    Changed,
    Rejected(ChallengeRejection),
    CredentialRejected,
}

#[derive(Debug, Clone)]
pub struct AccountStore {
    pool: PgPool,
    policy: Policy,
    /// Woken after a challenge commits, so the delivery worker of this process sends at once.
    wake: Arc<Notify>,
}

impl AccountStore {
    pub fn new(pool: PgPool, policy: Policy, wake: Arc<Notify>) -> Self {
        Self { pool, policy, wake }
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Registers a new account with username and password.
    /// Hashes the password outside of database transactions (CPU-bound).
    /// Inserts account and account_passwords, then issues an active session.
    /// If the username is already taken, rolls back without leaving orphan rows.
    pub async fn register_password(
        &self,
        username: &Username,
        password: &Password,
    ) -> Result<Result<NewSession, PasswordRegistrationError>, StoreError> {
        let password_hash =
            crate::password::hash_password(password).map_err(|_| StoreError::Unavailable)?;
        let mut tx = self.pool.begin().await?;
        let account_id = Uuid::now_v7();
        sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
        let insert_result = sqlx::query(
            "INSERT INTO account_passwords (account_id, username, password_hash)
             VALUES ($1, $2, $3)",
        )
        .bind(account_id)
        .bind(username.as_str())
        .bind(&password_hash)
        .execute(&mut *tx)
        .await;

        match insert_result {
            Ok(_) => {}
            Err(error) if is_unique_violation(&error) => {
                tx.rollback().await?;
                return Ok(Err(PasswordRegistrationError::UsernameTaken));
            }
            Err(error) => return Err(error.into()),
        }

        let session = self.issue_session(&mut tx, account_id).await?;
        tx.commit().await?;
        Ok(Ok(session))
    }

    /// Verifies username and password and issues a session.
    /// Hashes/verifies outside row locks; verifies account and credential standing
    /// under commit fence before issuing session.
    pub async fn sign_in_password(
        &self,
        username: &Username,
        password: &Password,
    ) -> Result<Result<NewSession, PasswordSignInRejection>, StoreError> {
        // Throttling per username: check recent failed attempts
        let recent_failures: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM password_login_attempts
              WHERE username = $1 AND created_at > now() - make_interval(secs => 900)",
        )
        .bind(username.as_str())
        .fetch_one(&self.pool)
        .await?;

        if recent_failures >= 10 {
            return Ok(Err(PasswordSignInRejection::RateLimited));
        }

        let row = sqlx::query(
            "SELECT ap.account_id, ap.password_hash, ap.version,
                    a.disabled_at IS NOT NULL AS disabled
               FROM account_passwords ap
               JOIN accounts a ON a.id = ap.account_id
              WHERE ap.username = $1",
        )
        .bind(username.as_str())
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            crate::password::dummy_verify(password);
            let _ =
                sqlx::query("INSERT INTO password_login_attempts (id, username) VALUES ($1, $2)")
                    .bind(Uuid::now_v7())
                    .bind(username.as_str())
                    .execute(&self.pool)
                    .await;
            return Ok(Err(PasswordSignInRejection::InvalidCredentials));
        };

        let stored_hash: String = row.try_get("password_hash")?;
        let account_id: Uuid = row.try_get("account_id")?;
        let expected_version: i32 = row.try_get("version")?;
        let initially_disabled: bool = row.try_get("disabled")?;

        if initially_disabled {
            return Ok(Err(PasswordSignInRejection::AccountDisabled));
        }

        let valid = crate::password::verify_password(&stored_hash, password);
        if !valid {
            let _ =
                sqlx::query("INSERT INTO password_login_attempts (id, username) VALUES ($1, $2)")
                    .bind(Uuid::now_v7())
                    .bind(username.as_str())
                    .execute(&self.pool)
                    .await;
            return Ok(Err(PasswordSignInRejection::InvalidCredentials));
        }

        let mut tx = self.pool.begin().await?;
        let check = sqlx::query(
            "SELECT a.disabled_at IS NOT NULL AS disabled, ap.version
               FROM accounts a
               JOIN account_passwords ap ON ap.account_id = a.id
              WHERE a.id = $1
                FOR UPDATE OF a, ap",
        )
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(check_row) = check else {
            tx.rollback().await?;
            return Ok(Err(PasswordSignInRejection::InvalidCredentials));
        };

        if check_row.try_get::<bool, _>("disabled")? {
            tx.rollback().await?;
            return Ok(Err(PasswordSignInRejection::AccountDisabled));
        }

        if check_row.try_get::<i32, _>("version")? != expected_version {
            tx.rollback().await?;
            return Ok(Err(PasswordSignInRejection::InvalidCredentials));
        }

        let _ = sqlx::query("DELETE FROM password_login_attempts WHERE username = $1")
            .bind(username.as_str())
            .execute(&mut *tx)
            .await;

        let session = self.issue_session(&mut tx, account_id).await?;
        tx.commit().await?;
        Ok(Ok(session))
    }

    fn issued(&self, outcome: &ChallengeOutcome) {
        if matches!(outcome, ChallengeOutcome::Issued { .. }) {
            self.wake.notify_one();
        }
    }

    /// Issues a sign-in challenge for `address`. The account table is not read: the answer and
    /// the mail are the same for a new and an existing address.
    pub async fn start_sign_in(
        &self,
        address: &EmailAddress,
    ) -> Result<ChallengeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let outcome = self
            .issue_challenge(&mut tx, address, Purpose::SignIn, None, true)
            .await?;
        tx.commit().await?;
        self.issued(&outcome);
        Ok(outcome)
    }

    /// Answers a sign-in challenge. On success the challenge is consumed, the account for the
    /// address is found or created (one account per address even under concurrent first
    /// sign-ins) and a session is issued, all in one transaction.
    pub async fn complete_sign_in(
        &self,
        handle: &ChallengeHandle,
        code: &Code,
    ) -> Result<SignInOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT address, code_hash, failed_attempts, consumed_at IS NOT NULL AS consumed,
                    expires_at <= now() AS expired
               FROM email_challenges WHERE id = $1 AND purpose = 'sign_in' FOR UPDATE",
        )
        .bind(handle.id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok(SignInOutcome::Rejected(ChallengeRejection::ChallengeClosed));
        };
        let address: String = row.try_get("address")?;
        if let Some(rejection) = self.attempt(&mut tx, handle, code, &row).await? {
            tx.commit().await?;
            return Ok(SignInOutcome::Rejected(rejection));
        }
        let account_id = provision_account(&mut tx, &address).await?;
        let disabled: bool =
            sqlx::query_scalar("SELECT disabled_at IS NOT NULL FROM accounts WHERE id = $1")
                .bind(account_id)
                .fetch_one(&mut *tx)
                .await?;
        if disabled {
            // The code is spent either way; a disabled account gets no session.
            tx.commit().await?;
            return Ok(SignInOutcome::Rejected(ChallengeRejection::AccountDisabled));
        }
        let session = self.issue_session(&mut tx, account_id).await?;
        tx.commit().await?;
        Ok(SignInOutcome::Authenticated(session))
    }

    /// The session a credential names, if it is active now. An expired, revoked or disabled
    /// session is reported with its reason; nothing is changed.
    pub async fn verify(
        &self,
        credential: &str,
    ) -> Result<Result<ActiveSession, CredentialRejection>, StoreError> {
        let Some(credential) = SessionCredential::parse(credential) else {
            return Ok(Err(CredentialRejection::Unknown));
        };
        let row = sqlx::query(
            "SELECT s.id, s.account_id, s.created_at, s.expires_at,
                    s.revoked_at IS NOT NULL AS revoked, s.expires_at <= now() AS expired,
                    a.disabled_at IS NOT NULL AS disabled
               FROM sessions s JOIN accounts a ON a.id = s.account_id
              WHERE s.token_hash = $1",
        )
        .bind(credential.hash().as_slice())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(Err(CredentialRejection::Unknown));
        };
        // Disabled first: disabling also revokes, but the account state is the authority.
        if row.try_get::<bool, _>("disabled")? {
            return Ok(Err(CredentialRejection::AccountDisabled));
        }
        if row.try_get::<bool, _>("revoked")? {
            return Ok(Err(CredentialRejection::Revoked));
        }
        if row.try_get::<bool, _>("expired")? {
            return Ok(Err(CredentialRejection::Expired));
        }
        Ok(Ok(ActiveSession {
            session_id: row.try_get("id")?,
            account_id: row.try_get("account_id")?,
            authenticated_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
        }))
    }

    /// Logout. `true` only when an active session was revoked by this call.
    pub async fn revoke(&self, credential: &str) -> Result<bool, StoreError> {
        let Some(credential) = SessionCredential::parse(credential) else {
            return Ok(false);
        };
        let revoked: Option<Uuid> = sqlx::query_scalar(
            "UPDATE sessions SET revoked_at = now(), revoked_reason = 'logout'
              WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()
             RETURNING id",
        )
        .bind(credential.hash().as_slice())
        .fetch_optional(&self.pool)
        .await?;
        Ok(revoked.is_some())
    }

    /// Starts a verified change of the session's account address. The code goes to the new
    /// address; the account keeps its subject.
    pub async fn start_email_change(
        &self,
        credential: &str,
        address: &EmailAddress,
    ) -> Result<EmailChangeStart, StoreError> {
        let Ok(session) = self.verify(credential).await? else {
            return Ok(EmailChangeStart::CredentialRejected);
        };
        let mut tx = self.pool.begin().await?;
        let holder: Option<Uuid> =
            sqlx::query_scalar("SELECT account_id FROM account_emails WHERE address = $1")
                .bind(address.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        if holder == Some(session.account_id) {
            return Ok(EmailChangeStart::AddressUnavailable);
        }
        // Another account's address: same answer and same throttles as a free address, but no
        // mail, so nobody can learn whether an address is registered. Completing it would fail
        // on the unique address anyway.
        let deliver = holder.is_none();
        let outcome = self
            .issue_challenge(
                &mut tx,
                address,
                Purpose::ChangeEmail,
                Some(session.account_id),
                deliver,
            )
            .await?;
        tx.commit().await?;
        self.issued(&outcome);
        Ok(EmailChangeStart::Issued(outcome))
    }

    /// Completes an address change for the session's own account. The other sessions of the
    /// account are revoked (`email_changed`); the presenting session stays.
    pub async fn complete_email_change(
        &self,
        credential: &str,
        handle: &ChallengeHandle,
        code: &Code,
    ) -> Result<EmailChangeOutcome, StoreError> {
        let Ok(session) = self.verify(credential).await? else {
            return Ok(EmailChangeOutcome::CredentialRejected);
        };
        let mut tx = self.pool.begin().await?;
        // Re-check the session under lock: a logout or disable racing this change either commits
        // first (and the change is refused) or waits for it.
        let still_active: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM sessions s JOIN accounts a ON a.id = s.account_id
              WHERE s.id = $1 AND s.revoked_at IS NULL AND s.expires_at > now()
                AND a.disabled_at IS NULL
                FOR UPDATE OF s, a",
        )
        .bind(session.session_id)
        .fetch_optional(&mut *tx)
        .await?;
        if still_active.is_none() {
            return Ok(EmailChangeOutcome::CredentialRejected);
        }
        let row = sqlx::query(
            "SELECT address, code_hash, failed_attempts, consumed_at IS NOT NULL AS consumed,
                    expires_at <= now() AS expired
               FROM email_challenges
              WHERE id = $1 AND purpose = 'change_email' AND account_id = $2 FOR UPDATE",
        )
        .bind(handle.id)
        .bind(session.account_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok(EmailChangeOutcome::Rejected(
                ChallengeRejection::ChallengeClosed,
            ));
        };
        let address: String = row.try_get("address")?;
        if let Some(rejection) = self.attempt(&mut tx, handle, code, &row).await? {
            tx.commit().await?;
            return Ok(EmailChangeOutcome::Rejected(rejection));
        }
        // The address may have been verified by another account since the challenge began.
        sqlx::query("SAVEPOINT address_change")
            .execute(&mut *tx)
            .await?;
        let updated = sqlx::query(
            "UPDATE account_emails SET address = $2, verified_at = now() WHERE account_id = $1",
        )
        .bind(session.account_id)
        .bind(&address)
        .execute(&mut *tx)
        .await;
        match updated {
            Ok(_) => {}
            Err(error) if is_unique_violation(&error) => {
                // Keep the attempt (the code is spent), drop only the failed update.
                sqlx::query("ROLLBACK TO SAVEPOINT address_change")
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(EmailChangeOutcome::Rejected(
                    ChallengeRejection::AddressUnavailable,
                ));
            }
            Err(error) => return Err(error.into()),
        }
        sqlx::query(
            "UPDATE sessions SET revoked_at = now(), revoked_reason = 'email_changed'
              WHERE account_id = $1 AND id <> $2 AND revoked_at IS NULL",
        )
        .bind(session.account_id)
        .bind(session.session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(EmailChangeOutcome::Changed)
    }

    /// Records one attempt on a locked challenge row. `None`: accepted and consumed.
    async fn attempt(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        handle: &ChallengeHandle,
        code: &Code,
        row: &sqlx::postgres::PgRow,
    ) -> Result<Option<ChallengeRejection>, StoreError> {
        let stored: Vec<u8> = row.try_get("code_hash")?;
        let state = ChallengeState {
            consumed: row.try_get("consumed")?,
            expired: row.try_get("expired")?,
            failed_attempts: row.try_get("failed_attempts")?,
        };
        match decide_attempt(state, handle.matches(code, &stored), &self.policy) {
            Attempt::Closed => Ok(Some(ChallengeRejection::ChallengeClosed)),
            Attempt::Wrong { closes } => {
                sqlx::query(
                    "UPDATE email_challenges SET failed_attempts = failed_attempts + 1 WHERE id = $1",
                )
                .bind(handle.id)
                .execute(&mut **tx)
                .await?;
                Ok(Some(if closes {
                    ChallengeRejection::ChallengeClosed
                } else {
                    ChallengeRejection::InvalidCode
                }))
            }
            Attempt::Accept => {
                // The row is locked, so this always matches; the condition is a second fence in
                // case a caller ever reads the row without `FOR UPDATE`.
                let consumed = sqlx::query(
                    "UPDATE email_challenges SET consumed_at = now()
                      WHERE id = $1 AND consumed_at IS NULL",
                )
                .bind(handle.id)
                .execute(&mut **tx)
                .await?;
                Ok((consumed.rows_affected() != 1).then_some(ChallengeRejection::ChallengeClosed))
            }
        }
    }

    /// Applies the ceilings, stores the verifier and, when `deliver`, the delivery intent, all in
    /// the caller's transaction. Nothing is sent here: the worker sends after commit.
    ///
    /// The per-address and per-account ceilings are exact (an advisory lock per address and the
    /// account's session row serialise them). The global ceiling is read without a lock, so
    /// concurrent requests can pass it by at most their number; it bounds mail volume, not a
    /// security property.
    async fn issue_challenge(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        address: &EmailAddress,
        purpose: Purpose,
        account: Option<Uuid>,
        deliver: bool,
    ) -> Result<ChallengeOutcome, StoreError> {
        // Serialise issuance per address so concurrent requests cannot both pass the throttle.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(address.as_str())
            .execute(&mut **tx)
            .await?;
        let recent: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_challenges
              WHERE address = $1 AND created_at > now() - make_interval(secs => $2)",
        )
        .bind(address.as_str())
        .bind(self.policy.throttle_window.as_seconds_f64())
        .fetch_one(&mut **tx)
        .await?;
        if recent >= self.policy.max_challenges_per_window {
            return Ok(ChallengeOutcome::RateLimited(Throttle::Address));
        }
        if let Some(account) = account {
            // Serialise per account too: two changes to different addresses must not both pass.
            // NO KEY UPDATE: a concurrent sign-in of this account (a session insert's key-share
            // lock) is not blocked by it.
            sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR NO KEY UPDATE")
                .bind(account)
                .execute(&mut **tx)
                .await?;
            let changes: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM email_challenges
                  WHERE account_id = $1 AND created_at > now() - make_interval(secs => $2)",
            )
            .bind(account)
            .bind(self.policy.account_window.as_seconds_f64())
            .fetch_one(&mut **tx)
            .await?;
            if changes >= self.policy.max_email_changes_per_window {
                return Ok(ChallengeOutcome::RateLimited(Throttle::Account));
            }
        }
        let everywhere: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_challenges
              WHERE created_at > now() - make_interval(secs => $1)",
        )
        .bind(self.policy.global_window.as_seconds_f64())
        .fetch_one(&mut **tx)
        .await?;
        if everywhere >= self.policy.max_challenges_globally {
            return Ok(ChallengeOutcome::RateLimited(Throttle::Global));
        }
        let handle = ChallengeHandle::generate(Uuid::now_v7())?;
        let code = Code::generate()?;
        let expires_at: OffsetDateTime = sqlx::query_scalar(
            "INSERT INTO email_challenges (id, purpose, address, account_id, code_hash, expires_at)
             VALUES ($1, $2, $3, $4, $5, now() + make_interval(secs => $6))
             RETURNING expires_at",
        )
        .bind(handle.id)
        .bind(purpose.label())
        .bind(address.as_str())
        .bind(account)
        .bind(handle.code_hash(&code).as_slice())
        .bind(self.policy.challenge_ttl.as_seconds_f64())
        .fetch_one(&mut **tx)
        .await?;
        if deliver {
            delivery::enqueue(tx, handle.id, purpose, address, &code).await?;
        }
        Ok(ChallengeOutcome::Issued { handle, expires_at })
    }

    async fn issue_session(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: Uuid,
    ) -> Result<NewSession, StoreError> {
        let credential = SessionCredential::generate()?;
        let session_id = Uuid::now_v7();
        let row = sqlx::query(
            "INSERT INTO sessions (id, account_id, token_hash, expires_at)
             VALUES ($1, $2, $3, now() + make_interval(secs => $4))
             RETURNING created_at, expires_at",
        )
        .bind(session_id)
        .bind(account_id)
        .bind(credential.hash().as_slice())
        .bind(self.policy.session_ttl.as_seconds_f64())
        .fetch_one(&mut **tx)
        .await?;
        Ok(NewSession {
            session_id,
            account_id,
            credential,
            authenticated_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
        })
    }
}

/// The account that verified `address`, created if none has. Two first sign-ins for one address
/// race on `account_emails_address_key`: the loser removes its unreferenced account row and
/// reads the winner's, so exactly one account (and later one Vink principal) exists.
async fn provision_account(
    tx: &mut Transaction<'_, Postgres>,
    address: &str,
) -> Result<Uuid, StoreError> {
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT account_id FROM account_emails WHERE address = $1")
            .bind(address)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(account) = existing {
        return Ok(account);
    }
    let candidate = Uuid::now_v7();
    sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
        .bind(candidate)
        .execute(&mut **tx)
        .await?;
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO account_emails (account_id, address, verified_at) VALUES ($1, $2, now())
         ON CONFLICT (address) DO NOTHING RETURNING account_id",
    )
    .bind(candidate)
    .bind(address)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(account) = inserted {
        return Ok(account);
    }
    sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(candidate)
        .execute(&mut **tx)
        .await?;
    Ok(
        sqlx::query_scalar("SELECT account_id FROM account_emails WHERE address = $1")
            .bind(address)
            .fetch_one(&mut **tx)
            .await?,
    )
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// Operator result of `vink-identity account disable|enable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountChange {
    Changed { sessions_revoked: u64 },
    Unchanged,
    UnknownAccount,
}

/// Disables an account and revokes every open session in one transaction. Already disabled:
/// unchanged. Re-enabling never restores a revoked session.
pub async fn disable_account(pool: &PgPool, account: Uuid) -> Result<AccountChange, StoreError> {
    let mut tx = pool.begin().await?;
    let exists: Option<bool> =
        sqlx::query_scalar("SELECT disabled_at IS NOT NULL FROM accounts WHERE id = $1 FOR UPDATE")
            .bind(account)
            .fetch_optional(&mut *tx)
            .await?;
    let change = match exists {
        None => AccountChange::UnknownAccount,
        Some(true) => AccountChange::Unchanged,
        Some(false) => {
            sqlx::query("UPDATE accounts SET disabled_at = now() WHERE id = $1")
                .bind(account)
                .execute(&mut *tx)
                .await?;
            let revoked = sqlx::query(
                "UPDATE sessions SET revoked_at = now(), revoked_reason = 'account_disabled'
                  WHERE account_id = $1 AND revoked_at IS NULL",
            )
            .bind(account)
            .execute(&mut *tx)
            .await?;
            AccountChange::Changed {
                sessions_revoked: revoked.rows_affected(),
            }
        }
    };
    tx.commit().await?;
    Ok(change)
}

pub async fn enable_account(pool: &PgPool, account: Uuid) -> Result<AccountChange, StoreError> {
    let updated = sqlx::query(
        "UPDATE accounts SET disabled_at = NULL WHERE id = $1 AND disabled_at IS NOT NULL",
    )
    .bind(account)
    .execute(pool)
    .await?;
    if updated.rows_affected() == 1 {
        return Ok(AccountChange::Changed {
            sessions_revoked: 0,
        });
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM accounts WHERE id = $1)")
        .bind(account)
        .fetch_one(pool)
        .await?;
    Ok(if exists {
        AccountChange::Unchanged
    } else {
        AccountChange::UnknownAccount
    })
}

/// How long closed records are kept before [`prune`] deletes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// After a challenge expires (its delivery row goes with it).
    pub challenges: time::Duration,
    /// After a session closed: expiry or revocation, whichever is later.
    pub sessions: time::Duration,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            challenges: time::Duration::days(7),
            sessions: time::Duration::days(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pruned {
    pub challenges: u64,
    pub sessions: u64,
}

/// Deletes expired challenges (and their deliveries) and long-closed sessions. Never touches an
/// account, a verified address, an open session or a challenge that could still be answered.
/// Idempotent; safe to run from several processes at once.
pub async fn prune(pool: &PgPool, retention: Retention) -> Result<Pruned, StoreError> {
    let challenges = sqlx::query(
        "DELETE FROM email_challenges WHERE expires_at < now() - make_interval(secs => $1)",
    )
    .bind(retention.challenges.as_seconds_f64())
    .execute(pool)
    .await?
    .rows_affected();
    let sessions = sqlx::query(
        "DELETE FROM sessions
          WHERE GREATEST(expires_at, COALESCE(revoked_at, expires_at))
                < now() - make_interval(secs => $1)",
    )
    .bind(retention.sessions.as_seconds_f64())
    .execute(pool)
    .await?
    .rows_affected();
    let _ = sqlx::query(
        "DELETE FROM password_login_attempts WHERE created_at < now() - INTERVAL '1 day'",
    )
    .execute(pool)
    .await;
    Ok(Pruned {
        challenges,
        sessions,
    })
}
