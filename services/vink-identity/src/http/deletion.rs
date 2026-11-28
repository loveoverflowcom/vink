use super::*;
use crate::contract::{AccountDeletion, Bounded};
use crate::outcome::{DeletionReceipt, DeletionRejection};
use axum::extract::Query;
use uuid::Uuid;

fn wire(receipt: DeletionReceipt) -> AccountDeletion {
    AccountDeletion {
        account_id: receipt.account_id,
        operation_id: receipt.operation_id,
        accepted_at: receipt.accepted_at,
    }
}

impl From<DeletionRejection> for ContractError {
    fn from(rejection: DeletionRejection) -> Self {
        match rejection {
            DeletionRejection::MalformedOperation => ContractError::MalformedRequest,
            DeletionRejection::InvalidCode => ContractError::InvalidCode,
            DeletionRejection::ChallengeClosed => ContractError::ChallengeClosed,
            DeletionRejection::InvalidCredentials => ContractError::InvalidCredentials,
            DeletionRejection::RateLimited => ContractError::RateLimited,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Start {
    email: Bounded<254>,
    expected: Option<Uuid>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Confirm {
    Password {
        username: Bounded<64>,
        password: Bounded<{ crate::contract::MAX_PASSWORD_FIELD_BYTES }>,
        expected: Option<Uuid>,
        operation_id: Uuid,
    },
    EmailCode {
        challenge: Bounded<256>,
        code: Bounded<16>,
        expected: Option<Uuid>,
        operation_id: Uuid,
    },
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Operation {
    operation_id: Uuid,
}

pub(super) async fn start(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Response, ContractError> {
    let request: Start = read_json(body).await?;
    let authority = &state.inner.authority;
    authority.require(Method::EmailCode)?;
    let address = EmailAddress::parse(request.email.expose()).ok_or(ContractError::InvalidEmail)?;
    match authority.start_deletion(&address, request.expected).await? {
        ChallengeOutcome::Issued { handle, expires_at } => Ok(issued(&handle, expires_at)),
        ChallengeOutcome::RateLimited(_) => Err(ContractError::RateLimited),
    }
}

pub(super) async fn confirm(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<AccountDeletion>, ContractError> {
    let request: Confirm = read_json_with_limit(body, MAX_PASSWORD_BODY_BYTES).await?;
    let authority = &state.inner.authority;
    let receipt = match request {
        Confirm::Password {
            username,
            password,
            expected,
            operation_id,
        } => {
            let username = Username::parse(username.expose())
                .map_err(|_| ContractError::InvalidCredentials)?;
            authority
                .confirm_deletion_password(&username, password.expose(), expected, operation_id)
                .await?
        }
        Confirm::EmailCode {
            challenge,
            code,
            expected,
            operation_id,
        } => {
            let handle =
                ChallengeHandle::parse(challenge.expose()).ok_or(ContractError::ChallengeClosed)?;
            let code = Code::parse(code.expose()).ok_or(ContractError::InvalidCode)?;
            authority
                .confirm_deletion_code(&handle, &code, expected, operation_id)
                .await?
        }
    };
    Ok(Json(wire(receipt?)))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pending {
    after: Option<Uuid>,
}

pub(super) async fn pending(
    State(state): State<IdentityState>,
    query: Result<Query<Pending>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Vec<AccountDeletion>>, ContractError> {
    let Query(query) = query.map_err(|_| ContractError::MalformedRequest)?;
    let receipts = state.inner.authority.pending_deletions(query.after).await?;
    Ok(Json(receipts.into_iter().map(wire).collect()))
}

pub(super) async fn status(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<Option<AccountDeletion>>, ContractError> {
    let request: Operation = read_json(body).await?;
    let receipt = state.inner.authority.deletion(request.operation_id).await?;
    Ok(Json(receipt.map(wire)))
}

pub(super) async fn finish(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<StatusCode, ContractError> {
    let request: Operation = read_json(body).await?;
    state
        .inner
        .authority
        .finish_deletion(request.operation_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
