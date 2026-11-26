use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct AccountDeletion {
    pub account_id: Uuid,
    pub operation_id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionError {
    InvalidEmail,
    InvalidCredentials,
    InvalidCode,
    Closed,
    RateLimited,
    Call(CallError),
}

impl IdentityClient {
    pub async fn start_account_deletion(
        &self,
        email: &str,
        expected: Option<Uuid>,
    ) -> Result<IssuedChallenge, DeletionError> {
        let wire: WireChallenge = self
            .deletion_request(
                "/identity/v1/account-deletion/challenges",
                serde_json::json!({ "email": email, "expected": expected }),
            )
            .await?;
        Ok(IssuedChallenge {
            handle: wire.challenge,
            expires_at: wire.expires_at,
        })
    }

    pub async fn confirm_account_deletion(
        &self,
        proof: serde_json::Value,
    ) -> Result<AccountDeletion, DeletionError> {
        self.deletion_request("/identity/v1/account-deletion/confirm", proof)
            .await
    }

    pub async fn account_deletion_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<AccountDeletion>, DeletionError> {
        self.deletion_request(
            "/identity/v1/account-deletion/status",
            serde_json::json!({ "operation_id": operation_id }),
        )
        .await
    }

    pub async fn pending_account_deletions(
        &self,
        after: Option<Uuid>,
    ) -> Result<Vec<AccountDeletion>, DeletionError> {
        let path = "/identity/v1/account-deletion/pending";
        let path = after.map_or_else(
            || path.to_owned(),
            |cursor| format!("{path}?after={cursor}"),
        );
        let (status, bytes) = self
            .call(Method::GET, &path, vec![])
            .await
            .map_err(DeletionError::Call)?;
        decode_deletion(status, &bytes)
    }

    pub async fn finish_account_deletion(&self, operation_id: Uuid) -> Result<(), DeletionError> {
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/account-deletion/finish",
                serde_json::json!({ "operation_id": operation_id })
                    .to_string()
                    .into_bytes(),
            )
            .await
            .map_err(DeletionError::Call)?;
        if status != StatusCode::NO_CONTENT {
            return Err(DeletionError::Call(error_for(status, &bytes)));
        }
        Ok(())
    }

    async fn deletion_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T, DeletionError> {
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                path,
                body.to_string().into_bytes(),
            )
            .await
            .map_err(DeletionError::Call)?;
        decode_deletion(status, &bytes)
    }
}

fn decode_deletion<T: serde::de::DeserializeOwned>(
    status: StatusCode,
    bytes: &[u8],
) -> Result<T, DeletionError> {
    if status.is_success() {
        return serde_json::from_slice(bytes)
            .map_err(|_| DeletionError::Call(CallError::Unavailable(Failure::MalformedResponse)));
    }
    Err(match (status.as_u16(), error_code(bytes).as_str()) {
        (422, "invalid_email") => DeletionError::InvalidEmail,
        (422, "invalid_credentials") => DeletionError::InvalidCredentials,
        (422, "invalid_code") => DeletionError::InvalidCode,
        (409, "challenge_closed") => DeletionError::Closed,
        (429, "rate_limited") => DeletionError::RateLimited,
        _ => DeletionError::Call(error_for(status, bytes)),
    })
}
