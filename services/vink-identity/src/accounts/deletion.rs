//! Fresh proof for deletion, including disabled accounts. It never issues a session.
use super::{AccountStore, ChallengeOutcome, ChallengeScope, StoreError};
use crate::contract::ChallengeRejection;
use crate::email_code::{ChallengeHandle, Code, EmailAddress};
use crate::outcome::{DeletionReceipt, DeletionRejection, PasswordSignInRejection};
use crate::password::Username;
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

/// Storage row of `account_deletions`; the receipt type stays free of SQL derives.
type ReceiptRow = (Uuid, Uuid, OffsetDateTime);

fn receipt((account_id, operation_id, accepted_at): ReceiptRow) -> DeletionReceipt {
    DeletionReceipt {
        account_id,
        operation_id,
        accepted_at,
    }
}

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
    ) -> Result<Result<DeletionReceipt, DeletionRejection>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let account: Option<Uuid> = sqlx::query_scalar(
            "SELECT account_id FROM email_challenges
              WHERE id = $1 AND purpose = 'delete_account' AND account_id IS NOT NULL",
        )
        .bind(handle.id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(account) = account.filter(|id| expected.is_none_or(|expected| *id == expected))
        else {
            return Ok(Err(DeletionRejection::ChallengeClosed));
        };
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
        .await?;
        let Some(row) = row else {
            return Ok(Err(DeletionRejection::ChallengeClosed));
        };
        if let Some(rejection) = self.attempt(&mut tx, handle, code, &row).await? {
            tx.commit().await?;
            return Ok(Err(match rejection {
                ChallengeRejection::InvalidCode => DeletionRejection::InvalidCode,
                _ => DeletionRejection::ChallengeClosed,
            }));
        }
        // A rejection here drops the transaction, so the accepted code is not spent either.
        let operation = match retire(&mut tx, account, operation_id).await? {
            Ok(operation) => operation,
            Err(rejection) => return Ok(Err(rejection)),
        };
        tx.commit().await?;
        Ok(Ok(operation))
    }

    pub async fn complete_deletion_password(
        &self,
        username: &Username,
        password: &str,
        expected: Option<Uuid>,
        operation_id: Uuid,
    ) -> Result<Result<DeletionReceipt, DeletionRejection>, StoreError> {
        let (account, version, _) = match self.verify_password(username, password).await? {
            Ok(proof) => proof,
            Err(PasswordSignInRejection::RateLimited) => {
                return Ok(Err(DeletionRejection::RateLimited));
            }
            Err(_) => return Ok(Err(DeletionRejection::InvalidCredentials)),
        };
        if expected.is_some_and(|expected| expected != account) {
            return Ok(Err(DeletionRejection::InvalidCredentials));
        }
        let mut tx = self.pool.begin().await?;
        lock_account(&mut tx, account).await?;
        let current: Option<i32> = sqlx::query_scalar(
            "SELECT version FROM account_passwords WHERE account_id = $1 AND username = $2",
        )
        .bind(account)
        .bind(username.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if current != Some(version) {
            return Ok(Err(DeletionRejection::InvalidCredentials));
        }
        let operation = match retire(&mut tx, account, operation_id).await? {
            Ok(operation) => operation,
            Err(rejection) => return Ok(Err(rejection)),
        };
        tx.commit().await?;
        Ok(Ok(operation))
    }

    pub async fn deletion(&self, operation: Uuid) -> Result<Option<DeletionReceipt>, StoreError> {
        let row: Option<ReceiptRow> = sqlx::query_as(
            "SELECT account_id, operation_id, accepted_at FROM account_deletions
              WHERE operation_id = $1",
        )
        .bind(operation)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(receipt))
    }

    pub async fn pending_deletions(
        &self,
        after: Option<Uuid>,
    ) -> Result<Vec<DeletionReceipt>, StoreError> {
        let rows: Vec<ReceiptRow> = sqlx::query_as(
            "SELECT account_id, operation_id, accepted_at FROM account_deletions
              WHERE applied_at IS NULL AND ($1::uuid IS NULL OR operation_id > $1)
              ORDER BY operation_id LIMIT 32",
        )
        .bind(after)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(receipt).collect())
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

/// Retires `account` under `operation_id` in the caller's transaction. A rejection leaves the
/// transaction for the caller to drop: nothing it wrote may be committed.
async fn retire(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    operation_id: Uuid,
) -> Result<Result<DeletionReceipt, DeletionRejection>, StoreError> {
    if operation_id.is_nil() {
        return Ok(Err(DeletionRejection::MalformedOperation));
    }
    let operation: Option<ReceiptRow> = sqlx::query_as(
        "INSERT INTO account_deletions (account_id, operation_id)
         VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING account_id, operation_id, accepted_at",
    )
    .bind(account)
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(operation) = operation.map(receipt) else {
        return Ok(Err(DeletionRejection::ChallengeClosed));
    };
    sqlx::query("UPDATE accounts SET disabled_at = coalesce(disabled_at, now()) WHERE id = $1")
        .bind(account)
        .execute(&mut **tx)
        .await?;

    // The account's old sign-in challenges must close before its address can be reused.
    // Its mail outbox rows cascade from the challenges.
    for statement in [
        "SELECT pg_advisory_xact_lock(hashtextextended(address, 0))
           FROM account_emails WHERE account_id = $1",
        "DELETE FROM email_challenges WHERE account_id = $1 OR address IN
           (SELECT address FROM account_emails WHERE account_id = $1)",
        "DELETE FROM sessions WHERE account_id = $1",
        "DELETE FROM password_login_attempts WHERE username IN
           (SELECT username FROM account_passwords WHERE account_id = $1)",
        "DELETE FROM account_passwords WHERE account_id = $1",
        "DELETE FROM account_emails WHERE account_id = $1",
    ] {
        sqlx::query(statement)
            .bind(account)
            .execute(&mut **tx)
            .await?;
    }
    Ok(Ok(operation))
}

/// Why the offline legacy-address review stopped. Nothing of the failing batch was committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyReviewError {
    /// The database could not answer.
    Unavailable,
    /// `account_emails` lacks the constraints that make retirement by address unambiguous.
    UnexpectedSchema,
    /// Retirement was refused for an account (a receipt for its operation already exists).
    Refused(DeletionRejection),
}

impl From<StoreError> for LegacyReviewError {
    fn from(StoreError::Unavailable: StoreError) -> Self {
        LegacyReviewError::Unavailable
    }
}

impl From<sqlx::Error> for LegacyReviewError {
    fn from(error: sqlx::Error) -> Self {
        StoreError::from(error).into()
    }
}

/// Offline cutover on the previous schema. Inspect the address using the same Rust policy
/// as issuance. Keep the legacy primary key, NOT NULL and unique constraints as prerequisites.
/// A batch locks both writers before reading, so ownership cannot change between inventory
/// and retirement. Each committed receipt is independently replayable by the App deletion reconciler.
pub async fn review_legacy_email_accounts(
    pool: &sqlx::PgPool,
    apply: bool,
) -> Result<(u64, u64), LegacyReviewError> {
    let mut cursor: Option<Uuid> = None;
    let mut examined = 0;
    let mut invalid = 0;
    loop {
        let mut tx = pool.begin().await?;
        if apply {
            sqlx::query("LOCK TABLE accounts, account_emails, account_deletions IN SHARE ROW EXCLUSIVE MODE")
                .execute(&mut *tx).await?;
        }
        // A divergent schema can make deleting by address ambiguous. Never run retirement
        // against such a database. The baseline also proves every binding has a verification time.
        let expected: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT FROM pg_constraint WHERE conrelid='account_emails'::regclass
                 AND contype='p' AND pg_get_constraintdef(oid)='PRIMARY KEY (account_id)')
             AND EXISTS (SELECT FROM pg_constraint WHERE conrelid='account_emails'::regclass
                 AND contype='u' AND pg_get_constraintdef(oid)='UNIQUE (address)')
             AND 2=(SELECT count(*) FROM pg_attribute WHERE attrelid='account_emails'::regclass
                 AND attname IN ('address','verified_at') AND attnotnull AND NOT attisdropped)",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !expected {
            return Err(LegacyReviewError::UnexpectedSchema);
        }
        let rows: Vec<(Uuid, Option<String>)> = sqlx::query_as(
            "SELECT a.id, e.address FROM accounts a
             LEFT JOIN account_emails e ON e.account_id = a.id
             WHERE NOT EXISTS (SELECT 1 FROM account_deletions d WHERE d.account_id = a.id)
               AND ($1::uuid IS NULL OR a.id > $1)
             ORDER BY a.id LIMIT 128",
        )
        .bind(cursor)
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            return Ok((examined, invalid));
        }
        for (account, address) in rows {
            cursor = Some(account);
            examined += 1;
            if address.as_deref().is_some_and(|raw| {
                EmailAddress::parse(raw).is_some_and(|parsed| parsed.as_str() == raw)
            }) {
                continue;
            }
            invalid += 1;
            if apply {
                retire(&mut tx, account, Uuid::now_v7())
                    .await?
                    .map_err(LegacyReviewError::Refused)?;
            }
        }
        tx.commit().await?;
    }
}
