//! The browser-facing deletion ceremony. A proof never yields an ordinary session.
use super::*;
use crate::identity::client::{AccountDeletion, DeletionError};

const DELETE_COOKIE: &str = "__Host-vink_delete_account";
const ISSUE_LIMIT: RateLimit = RateLimit {
    bucket: "account_delete.challenge",
    ..SIGN_IN_CHALLENGE
};
const CODE_LIMIT: RateLimit = RateLimit {
    bucket: "account_delete.code",
    ..SIGN_IN_COMPLETE
};
const STATUS_LIMIT: RateLimit = RateLimit {
    bucket: "account_delete.status",
    ..SIGN_IN_COMPLETE
};
const PASSWORD_LIMIT: RateLimit = RateLimit {
    bucket: "account_delete.password",
    ..PASSWORD_LOGIN
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Start {
    email: String,
}

#[derive(Serialize)]
struct Sent {
    challenge_id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Confirm {
    Password {
        operation_id: Uuid,
        username: String,
        password: String,
    },
    EmailCode {
        operation_id: Uuid,
        challenge_id: Uuid,
        code: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Operation {
    operation_id: Uuid,
}

fn refusal(error: DeletionError) -> ApiError {
    match error {
        DeletionError::InvalidEmail => ApiError::Validation {
            field: "email",
            message: "invalid email address".into(),
        },
        DeletionError::InvalidCredentials => ApiError::InvalidCredentials,
        DeletionError::InvalidCode => ApiError::CodeRejected,
        DeletionError::Closed => ApiError::ChallengeClosed,
        DeletionError::RateLimited => ApiError::RateLimited {
            retry_after_seconds: None,
        },
        DeletionError::Call(error) => unavailable(error),
    }
}

/// The signed-in ceremony can only affect the presenting account. An expired/revoked cookie
/// cannot authorize anything, so disabled users use the same dedicated proof as a guest.
async fn expected_account(
    client: &IdentityClient,
    parts: &Parts,
) -> Result<Option<Uuid>, ApiError> {
    let Some(credential) = presented_credential(parts) else {
        return Ok(None);
    };
    match client.verify(credential).await.map_err(unavailable)? {
        Verification::Authenticated { authority, subject } if authority == "vink-identity" => Ok(
            Some(Uuid::parse_str(&subject).map_err(|_| ApiError::DependencyUnavailable)?),
        ),
        Verification::Rejected => Ok(None),
        Verification::Authenticated { .. } => Err(ApiError::AuthorityNotConfigured),
    }
}

pub(super) async fn start(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let client = sign_in_client(&state)?;
    let (parts, body) = request.into_parts();
    admit(&state, &parts, &ISSUE_LIMIT).await?;
    let expected = expected_account(client, &parts).await?;
    let request: Start = json(parts, body).await?;
    if request.email.len() > 254 {
        return Err(refusal(DeletionError::InvalidEmail));
    }
    let issued = client
        .start_account_deletion(&request.email, expected)
        .await
        .map_err(refusal)?;
    let challenge_id =
        attachment_handle_id(&issued.handle).ok_or(ApiError::DependencyUnavailable)?;
    let cookie = cookie(
        DELETE_COOKIE,
        &issued.handle,
        "Strict",
        seconds_until(issued.expires_at),
    )
    .ok_or(ApiError::DependencyUnavailable)?;
    Ok((
        StatusCode::ACCEPTED,
        [(header::SET_COOKIE, cookie)],
        Json(Sent {
            challenge_id,
            expires_at: issued.expires_at,
        }),
    )
        .into_response())
}

pub(super) async fn confirm(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let client = sign_in_client(&state)?;
    let (parts, body) = request.into_parts();
    let expected = expected_account(client, &parts).await?;
    let challenge = single_cookie(&parts.headers, DELETE_COOKIE)
        .filter(|handle| handle.len() <= 256)
        .map(str::to_owned);
    let net = network(&state, &parts).ok_or(ApiError::DependencyUnavailable)?;
    let request: Confirm = json(parts, body).await?;
    let (limit, proof) = match request {
        Confirm::Password {
            operation_id,
            username,
            password,
        } if !operation_id.is_nil()
            && username.len() <= 64
            && password.len() <= MAX_CREDENTIAL_BYTES =>
        {
            (
                &PASSWORD_LIMIT,
                serde_json::json!({ "method": "password", "operation_id": operation_id,
                "expected": expected, "username": username, "password": password }),
            )
        }
        Confirm::Password { .. } => return Err(ApiError::InvalidCredentials),
        Confirm::EmailCode {
            operation_id,
            challenge_id,
            code,
        } if !operation_id.is_nil() && code.len() <= 16 => {
            let handle = challenge
                .filter(|handle| attachment_handle_id(handle) == Some(challenge_id))
                .ok_or(ApiError::ChallengeClosed)?;
            (
                &CODE_LIMIT,
                serde_json::json!({ "method": "email_code", "operation_id": operation_id,
                "expected": expected, "challenge": handle, "code": code }),
            )
        }
        Confirm::EmailCode { .. } => return Err(ApiError::ChallengeClosed),
    };
    admit_network(&state, net, limit).await?;
    let operation = client
        .confirm_account_deletion(proof)
        .await
        .map_err(refusal)?;

    // Identity has revoked the proof's sessions already. Let the receipt's background consumer
    // finish if App is temporarily slow, the request disconnects, or this process restarts.
    let applied = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        state.vink.store().apply_account_deletion(operation),
    )
    .await
    .is_ok_and(|result| result.is_ok());
    Ok(receipt(applied))
}

pub(super) async fn status(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let client = state
        .sign_in
        .as_ref()
        .ok_or(ApiError::AuthorityNotConfigured)?;
    let (parts, body) = request.into_parts();
    let expected = expected_account(client, &parts).await?;
    admit(&state, &parts, &STATUS_LIMIT).await?;
    let request: Operation = json(parts, body).await?;
    let operation: AccountDeletion = client
        .account_deletion_status(request.operation_id)
        .await
        .map_err(refusal)?
        .filter(|op| expected.is_none_or(|expected| expected == op.account_id))
        .ok_or(ApiError::ChallengeClosed)?;
    let applied = state
        .vink
        .store()
        .account_deletion_applied(operation.operation_id)
        .await
        .unwrap_or(false);
    Ok(receipt(applied))
}

fn receipt(applied: bool) -> Response {
    let (status, stage) = if applied {
        (StatusCode::OK, "deleted")
    } else {
        (StatusCode::ACCEPTED, "accepted")
    };
    let mut response = (status, Json(serde_json::json!({ "state": stage }))).into_response();
    for name in [
        SESSION_COOKIE,
        DELETE_COOKIE,
        CHALLENGE_COOKIE,
        ENROLL_COOKIE,
        ATTACH_COOKIE,
    ] {
        response
            .headers_mut()
            .append(header::SET_COOKIE, clear(name));
    }
    response
}
