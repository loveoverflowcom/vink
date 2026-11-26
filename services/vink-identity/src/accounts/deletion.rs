//! Fresh proof for deletion, including disabled accounts. It never issues a session.
use super::{AccountStore, ChallengeOutcome, ChallengeScope, StoreError};
use crate::contract::{AccountDeletion, ContractError};
use crate::email_code::{ChallengeHandle, Code, EmailAddress};
use crate::password::Username;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

impl AccountStore {
    pub async fn start_deletion(
        &self,
        email: &EmailAddress,
        expected: Option<Uuid>,
    ) -> Result<ChallengeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let account = lock_email_owner(&mut tx, email.as_str()).await?;
        let target = account.filter(|id| expected.is_none_or(|expected| *id == expected));
        let outcome = self
            .issue_challenge(
                &mut tx,
                email,
                ChallengeScope::DeleteAccount(target),
                target.is_some(),
            )
            .await?;
        tx.commit().await?;
        self.issued(&outcome);
        Ok(outcome)
    }

    pub async fn complete_deletion_code(
        &self,
        handle: &ChallengeHandle,
        code: &Code,
        expected: Option<Uuid>,
        operation_id: Uuid,
    ) -> Result<AccountDeletion, ContractError> {
        let mut tx = self.pool.begin().await.map_err(StoreError::from)?;
        let account: Option<Uuid> = sqlx::query_scalar(
            "SELECT account_id FROM email_challenges
              WHERE id = $1 AND purpose = 'delete_account' AND account_id IS NOT NULL",
        )
        .bind(handle.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        let account = account
            .filter(|id| expected.is_none_or(|expected| *id == expected))
            .ok_or(ContractError::ChallengeClosed)?;
        lock_account(&mut tx, account).await?;
        let row = sqlx::query(
            "SELECT c.code_hash, c.failed_attempts, c.consumed_at IS NOT NULL AS consumed,
                    c.expires_at <= clock_timestamp() AS expired
               FROM email_challenges c JOIN account_emails e USING (account_id)
              WHERE c.id = $1 AND c.purpose = 'delete_account' AND c.account_id = $2
                AND c.address = e.address
              FOR UPDATE OF c",
        )
        .bind(handle.id)
        .bind(account)
        .fetch_optional(&mut *tx)
        .await
        .map_err(StoreError::from)?
        .ok_or(ContractError::ChallengeClosed)?;
        if let Some(rejection) = self.attempt(&mut tx, handle, code, &row).await? {
            tx.commit().await.map_err(StoreError::from)?;
            return Err(match rejection {
                crate::contract::ChallengeRejection::InvalidCode => ContractError::InvalidCode,
                _ => ContractError::ChallengeClosed,
            });
        }
        let operation = retire(&mut tx, account, operation_id).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(operation)
    }

    pub async fn complete_deletion_password(
        &self,
        username: &Username,
        password: &str,
        expected: Option<Uuid>,
        operation_id: Uuid,
    ) -> Result<AccountDeletion, ContractError> {
        let (account, version, _) =
            self.verify_password(username, password)
                .await?
                .map_err(|error| match error {
                    super::PasswordSignInRejection::RateLimited => ContractError::RateLimited,
                    _ => ContractError::InvalidCredentials,
                })?;
        if expected.is_some_and(|expected| expected != account) {
            return Err(ContractError::InvalidCredentials);
        }
        let mut tx = self.pool.begin().await.map_err(StoreError::from)?;
        lock_account(&mut tx, account).await?;
        let current: Option<i32> = sqlx::query_scalar(
            "SELECT version FROM account_passwords WHERE account_id = $1 AND username = $2",
        )
        .bind(account)
        .bind(username.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if current != Some(version) {
            return Err(ContractError::InvalidCredentials);
        }
        let operation = retire(&mut tx, account, operation_id).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(operation)
    }

    pub async fn deletion(&self, operation: Uuid) -> Result<Option<AccountDeletion>, StoreError> {
        Ok(sqlx::query_as(
            "SELECT account_id, operation_id, accepted_at FROM account_deletions
              WHERE operation_id = $1",
        )
        .bind(operation)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn pending_deletions(
        &self,
        after: Option<Uuid>,
    ) -> Result<Vec<AccountDeletion>, StoreError> {
        Ok(sqlx::query_as(
            "SELECT account_id, operation_id, accepted_at FROM account_deletions
              WHERE applied_at IS NULL AND ($1::uuid IS NULL OR operation_id > $1)
              ORDER BY operation_id LIMIT 32",
        )
        .bind(after)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn finish_deletion(&self, operation: Uuid) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE account_deletions SET applied_at = clock_timestamp()
              WHERE operation_id = $1 AND applied_at IS NULL",
        )
        .bind(operation)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn lock_account(tx: &mut Transaction<'_, Postgres>, account: Uuid) -> Result<(), StoreError> {
    sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE")
        .bind(account)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

// Read address ownership again after any account lock wait.
pub(super) async fn lock_email_owner(
    tx: &mut Transaction<'_, Postgres>,
    address: &str,
) -> Result<Option<Uuid>, StoreError> {
    let owner: Option<Uuid> =
        sqlx::query_scalar("SELECT account_id FROM account_emails WHERE address = $1")
            .bind(address)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(owner) = owner {
        lock_account(tx, owner).await?;
        return Ok(sqlx::query_scalar(
            "SELECT account_id FROM account_emails WHERE address = $1 AND account_id = $2",
        )
        .bind(address)
        .bind(owner)
        .fetch_optional(&mut **tx)
        .await?);
    }
    Ok(None)
}

async fn retire(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    operation_id: Uuid,
) -> Result<AccountDeletion, ContractError> {
    if operation_id.is_nil() {
        return Err(ContractError::MalformedRequest);
    }
    let operation = sqlx::query_as(
        "INSERT INTO account_deletions (account_id, operation_id)
         VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING account_id, operation_id, accepted_at",
    )
    .bind(account)
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(StoreError::from)?
    .ok_or(ContractError::ChallengeClosed)?;
    sqlx::query("UPDATE accounts SET disabled_at = coalesce(disabled_at, now()) WHERE id = $1")
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(StoreError::from)?;

    // The account's old sign-in challenges must close before its address can be reused.
    // Its mail outbox rows cascade from the challenges.
    for statement in [
        "SELECT pg_advisory_xact_lock(hashtextextended(address, 0))
           FROM account_emails WHERE account_id = $1",
        "DELETE FROM email_challenges WHERE account_id = $1 OR address IN
           (SELECT address FROM account_emails WHERE account_id = $1)",
        "DELETE FROM email_attachment_reauth_attempts WHERE account_id = $1",
        "DELETE FROM sessions WHERE account_id = $1",
        "DELETE FROM password_login_attempts WHERE username IN
           (SELECT username FROM account_passwords WHERE account_id = $1)",
        "DELETE FROM account_passwords WHERE account_id = $1",
        "DELETE FROM account_emails WHERE account_id = $1",
    ] {
        sqlx::query(statement)
            .bind(account)
            .execute(&mut **tx)
            .await
            .map_err(StoreError::from)?;
    }
    Ok(operation)
}
